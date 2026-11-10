use crate::{
    engine::{self, Event, JobContext, Preview, Tools},
    i18n::{self, Language, message, t},
    model::{self, Mode, Project, Segment},
};
use anyhow::{Context as _, Result};
use eframe::egui::{self, Context};
use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
    },
    thread,
    time::{Duration, Instant},
};

pub struct App {
    pub project: Project,
    pub preferences: Preferences,
    pub tools: Tools,
    pub tools_available: bool,
    pub ports: Vec<(String, String)>,
    pub activity: String,
    pub error: bool,
    pub log: VecDeque<String>,
    pub log_open: bool,
    pub log_height: f32,
    pub preview: Option<Result<Preview, String>>,
    pub confirm: Option<Plan>,
    pub command_preview: Option<String>,
    pub help: Option<(String, Receiver<String>)>,
    pub help_text: String,
    pub about_open: bool,
    pub port_error: String,
    pub autosave_error: String,
    pub job: Option<Job>,
    pub flash_monitor: Option<FlashMonitor>,
    pub github_icon: egui::TextureHandle,
    log_sequence: u64,
    profile: PathBuf,
    preferences_path: PathBuf,
    saved: Project,
    dirty_since: Option<Instant>,
    scan: Option<Scan>,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Preferences {
    language: String,
    pub guide: bool,
    pub advanced: bool,
    pub appearance: Appearance,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            language: String::new(),
            guide: true,
            advanced: false,
            appearance: Appearance::System,
        }
    }
}

impl Preferences {
    pub fn language(&self) -> Language {
        Language::from_tag(&self.language).unwrap_or_else(Language::system)
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Appearance {
    #[default]
    System,
    Light,
    Dark,
}

impl Appearance {
    pub const ALL: [Self; 3] = [Self::System, Self::Light, Self::Dark];

    pub fn key(self) -> &'static str {
        match self {
            Self::System => "theme_system",
            Self::Light => "theme_light",
            Self::Dark => "theme_dark",
        }
    }

    fn apply(self, ctx: &Context) {
        ctx.set_theme(match self {
            Self::System => egui::ThemePreference::System,
            Self::Light => egui::ThemePreference::Light,
            Self::Dark => egui::ThemePreference::Dark,
        });
    }
}

pub struct FlashMonitor {
    pub summary: String,
    pub visible: bool,
    pub outcome: Option<Result<String, String>>,
    first_line: u64,
}

struct Scan {
    key: (String, model::FileSystem, bool, bool),
    cancel: Arc<AtomicBool>,
    rx: Receiver<Result<Preview, String>>,
}

pub struct Job {
    pub title: &'static str,
    pub progress: Option<f32>,
    pub cancel: Arc<AtomicBool>,
    rx: Receiver<Event>,
}

pub enum Plan {
    Flash {
        command: engine::Invocation,
        summary: String,
    },
    DataFlash {
        config: Box<model::DataConfig>,
        connection: model::Connection,
        display: String,
        summary: String,
    },
    Merge {
        connection: model::Connection,
        inputs: Vec<model::WriteInput>,
        format: String,
        output: PathBuf,
    },
    Build {
        config: model::DataConfig,
        output: PathBuf,
    },
}

impl Plan {
    fn title(&self) -> &'static str {
        match self {
            Self::Flash { .. } => t("flash_firmware"),
            Self::DataFlash { .. } => t("flash_data"),
            Self::Merge { .. } => t("merge"),
            Self::Build { .. } => t("build_image"),
        }
    }

    pub fn review(&self) -> (&str, String) {
        match self {
            Self::Flash { command, summary } => (summary, command.display()),
            Self::DataFlash {
                display, summary, ..
            } => (summary, display.clone()),
            _ => ("", String::new()),
        }
    }

    fn run(self, tools: &Tools, ctx: &JobContext) -> Result<String> {
        match self {
            Self::Flash { command, .. } => {
                command.run(ctx)?;
                Ok(t("flash_complete").into())
            }
            Self::DataFlash {
                config, connection, ..
            } => {
                let dir = tempfile::tempdir()?;
                let image = dir.path().join("filesystem.bin");
                let size = engine::generate_image(tools, &config, &image, ctx)?;
                let input = engine::data_input(&config, image, size, &connection)?;
                engine::flash_command(tools, &connection, &[input], true)?.run(ctx)?;
                Ok(message("data_complete", &[&config.format.name()]))
            }
            Self::Merge {
                connection,
                inputs,
                format,
                output,
            } => {
                engine::save_output(&output, &inputs, |tmp| {
                    engine::merge_command(tools, &connection, &inputs, &format, tmp)?.run(ctx)
                })?;
                Ok(message("output_saved", &[&output.display()]))
            }
            Self::Build { config, output } => {
                engine::validate_image_destination(&output, Path::new(&config.folder))?;
                engine::save_output(&output, &[], |tmp| {
                    engine::generate_image(tools, &config, tmp, ctx)?;
                    ctx.check_cancel()
                })?;
                Ok(message("output_saved", &[&output.display()]))
            }
        }
    }
}

impl App {
    pub fn new(ctx: &Context) -> Self {
        let tools = Tools::locate();
        let profile = directories::ProjectDirs::from("", "SuperFlash", "ESP32-SuperFlash")
            .map(|dirs| dirs.config_dir().join("last-project.json"))
            .unwrap_or_else(|| PathBuf::from("./.superflash/last-project.json"));
        let preferences_path = profile.with_file_name("preferences.json");
        let mut preferences: Preferences = std::fs::read(&preferences_path)
            .ok()
            .and_then(|data| serde_json::from_slice(&data).ok())
            .unwrap_or_default();
        let language = preferences.language();
        preferences.language = language.code().into();
        i18n::activate(language);
        crate::fonts::install(ctx, &tools, language);
        crate::ui::install_style(ctx);
        preferences.appearance.apply(ctx);
        let (project, autosave_error) = match model::read_project(&profile) {
            Ok(project) => (project, String::new()),
            Err(err) if profile.exists() => (Project::default(), engine::tool_error(&err)),
            Err(_) => (Project::default(), String::new()),
        };
        let tools_available = tools.available();
        let saved = project.clone();
        let mut app = Self {
            tools,
            tools_available,
            profile,
            preferences_path,
            preferences,
            saved,
            project,
            autosave_error,
            ports: Vec::new(),
            port_error: String::new(),
            activity: t("ready").into(),
            error: false,
            log: VecDeque::new(),
            log_open: false,
            log_height: 216.0,
            preview: None,
            scan: None,
            job: None,
            flash_monitor: None,
            github_icon: crate::ui::install_github_icon(ctx),
            log_sequence: 0,
            dirty_since: None,
            confirm: None,
            command_preview: None,
            help: None,
            help_text: String::new(),
            about_open: false,
        };
        app.refresh_ports();
        if !tools_available {
            app.report_error(t("missing_tools"));
        }
        app
    }

    pub fn change_language(&mut self, ctx: &Context, language: Language) {
        if language == self.preferences.language() {
            return;
        }
        i18n::activate(language);
        self.preferences.language = language.code().into();
        crate::fonts::install(ctx, &self.tools, language);
        self.activity = t("ready").into();
        self.error = false;
        self.rescan();
        self.save_preferences();
    }

    pub fn change_appearance(&mut self, ctx: &Context, appearance: Appearance) {
        if appearance != self.preferences.appearance {
            self.preferences.appearance = appearance;
            appearance.apply(ctx);
            self.save_preferences();
        }
    }

    pub fn save_preferences(&mut self) {
        if let Err(error) = model::save_json(&self.preferences_path, &self.preferences) {
            self.autosave_error = engine::tool_error(&error);
        }
    }

    pub fn refresh_ports(&mut self) {
        match engine::port_names() {
            Ok(ports) => {
                self.ports = ports;
                self.port_error.clear();
            }
            Err(err) => self.port_error = engine::tool_error(&err),
        }
    }

    pub fn report_error(&mut self, error: impl Into<String>) {
        self.set_activity(error.into(), true);
    }

    fn set_activity(&mut self, text: String, error: bool) {
        self.error = error;
        self.push_log(text.clone());
        self.activity = text;
    }

    fn push_log(&mut self, line: String) {
        if self.log.len() >= 1000 {
            self.log.pop_front();
        }
        self.log.push_back(line);
        self.log_sequence += 1;
    }

    pub fn log_text(&self) -> String {
        self.log.iter().cloned().collect::<Vec<_>>().join("\n")
    }

    pub fn flash_log_start(&self, monitor: &FlashMonitor) -> usize {
        let received = (self.log_sequence - monitor.first_line) as usize;
        self.log.len().saturating_sub(received)
    }

    pub fn save(&mut self) {
        if self.saved != self.project {
            match model::save_project(&self.profile, &self.project) {
                Ok(()) => {
                    self.saved = self.project.clone();
                    self.autosave_error.clear();
                }
                Err(err) => self.autosave_error = engine::tool_error(&err),
            }
        }
        self.dirty_since = None;
    }

    pub fn import_project(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("SuperFlash", &["json"])
            .pick_file()
        {
            match model::read_project(&path) {
                Ok(project) => {
                    self.project = project;
                    self.rescan();
                    self.set_activity(message("project_loaded", &[&path.display()]), false);
                }
                Err(err) => self.report_error(engine::tool_error(&err)),
            }
        }
    }

    pub fn export_project(&mut self) {
        let dialog = rfd::FileDialog::new()
            .add_filter("SuperFlash", &["json"])
            .set_file_name("superflash.json");
        if let Some(path) = dialog.save_file() {
            match model::save_project(&path, &self.project) {
                Ok(()) => self.set_activity(message("output_saved", &[&path.display()]), false),
                Err(err) => self.report_error(engine::tool_error(&err)),
            }
        }
    }

    pub fn choose_csv(&mut self) {
        let dialog =
            dialog_at(&self.project.csv_path).add_filter("ESP-IDF partition CSV", &["csv"]);
        if let Some(path) = dialog.pick_file() {
            self.project.csv_path = path.to_string_lossy().into_owned();
            self.load_csv();
        }
    }

    pub fn load_csv(&mut self) {
        match model::read_partitions(Path::new(&self.project.csv_path), &self.project) {
            Ok(rows) => {
                let imported = rows.len();
                self.project.segments.retain(|row| row.kind.is_empty());
                self.project.segments.extend(rows);
                self.set_activity(message("partitions_imported", &[&imported]), false);
            }
            Err(err) => self.report_error(engine::tool_error(&err)),
        }
    }

    pub fn choose_folder(&mut self) {
        let mut dialog = rfd::FileDialog::new();
        if Path::new(&self.project.data.folder).is_dir() {
            dialog = dialog.set_directory(&self.project.data.folder);
        }
        if let Some(path) = dialog.pick_folder() {
            self.project.data.folder = path.to_string_lossy().into_owned();
            self.rescan();
        }
    }

    pub fn rescan(&mut self) {
        if let Some(scan) = self.scan.take() {
            scan.cancel.store(true, Ordering::Relaxed);
        }
        self.preview = None;
    }

    pub fn choose_output(&mut self, merge: bool) {
        let (path, extension, name) = if merge {
            let extension = if self.project.merge_format == "raw" {
                "bin"
            } else {
                &self.project.merge_format
            };
            (
                &mut self.project.merge_path,
                extension,
                format!("firmware.{extension}"),
            )
        } else {
            (
                &mut self.project.image_path,
                "bin",
                format!(
                    "{}.bin",
                    self.project.data.format.name().to_ascii_lowercase()
                ),
            )
        };
        let dialog = dialog_at(path)
            .add_filter("Image", &[extension])
            .set_file_name(&name);
        if let Some(new_path) = dialog.save_file() {
            *path = new_path.to_string_lossy().into_owned();
        }
    }

    pub fn flash_plan(&self) -> Result<Plan> {
        let port = if self.project.connection.auto_port {
            t("auto_port").into()
        } else {
            self.project.connection.port.clone()
        };
        let erase = if self.project.connection.erase_all {
            t("erase_warning")
        } else {
            ""
        };
        let chip = &self.project.connection.chip;
        let options = &self.project.connection;
        if self.project.mode == Mode::Data {
            let config = self.project.data.clone();
            let size = engine::data_size(&config)?;
            let preview = self
                .preview
                .as_ref()
                .context(t("scanning"))?
                .as_ref()
                .map_err(|err| anyhow::anyhow!("{err}"))?;
            anyhow::ensure!(
                preview.sizes.iter().sum::<u64>() <= size,
                "{}",
                t("over_capacity")
            );
            let placeholder = PathBuf::from("<filesystem.bin>");
            engine::image_command(
                &self.tools,
                &config,
                size,
                Path::new("<staged-folder>"),
                &placeholder,
            )?;
            let input = engine::data_input(&config, placeholder, size, options)?;
            let summary = format!(
                "{} {chip} · {port}\n{} · {} · {}",
                t("target"),
                config.format.name(),
                model::hex(input.address),
                model::human_size(size)
            );
            let display = engine::flash_command(&self.tools, options, &[input], true)?.display();
            Ok(Plan::DataFlash {
                config: Box::new(config),
                connection: options.clone(),
                display,
                summary,
            })
        } else {
            let inputs = if self.project.mode == Mode::Firmware {
                model::inputs([&self.project.firmware], options, true)?
            } else {
                model::inputs(self.partition_files(), options, true)?
            };
            let mut summary = format!("{} {chip} · {port}\n{erase}", t("target"));
            for input in &inputs {
                let name = input.path.file_name().unwrap_or_default().to_string_lossy();
                summary.push_str(&format!(
                    "\n{}    {name}    {}",
                    model::hex(input.address),
                    model::human_size(input.size)
                ));
            }
            let command = engine::flash_command(&self.tools, options, &inputs, false)?;
            Ok(Plan::Flash { command, summary })
        }
    }

    pub fn request_flash(&mut self, preview: bool) {
        match self.flash_plan() {
            Ok(plan) if preview => self.command_preview = Some(plan.review().1),
            Ok(plan) => self.confirm = Some(plan),
            Err(err) => self.report_error(engine::tool_error(&err)),
        }
    }

    fn partition_files(&self) -> impl Iterator<Item = &Segment> {
        self.project
            .segments
            .iter()
            .filter(|row| !row.path.trim().is_empty())
    }

    fn output_plan(&self, merge: bool) -> Result<Plan> {
        if merge {
            let output = engine::open_output_path(&self.project.merge_path)?;
            let connection = self.project.connection.clone();
            let inputs = model::inputs(self.partition_files(), &connection, true)?;
            let format = self.project.merge_format.clone();
            engine::merge_command(&self.tools, &connection, &inputs, &format, &output)?;
            Ok(Plan::Merge {
                connection,
                inputs,
                format,
                output,
            })
        } else {
            let output = engine::open_output_path(&self.project.image_path)?;
            let config = self.project.data.clone();
            let size = engine::data_size(&config)?;
            engine::validate_image_destination(&output, Path::new(&config.folder))?;
            engine::image_command(
                &self.tools,
                &config,
                size,
                Path::new(&config.folder),
                &output,
            )?;
            Ok(Plan::Build { config, output })
        }
    }

    pub fn request_output(&mut self, ctx: &Context, merge: bool) {
        if if merge {
            self.project.merge_path.is_empty()
        } else {
            self.project.image_path.is_empty()
        } {
            self.choose_output(merge);
            if if merge {
                self.project.merge_path.is_empty()
            } else {
                self.project.image_path.is_empty()
            } {
                return;
            }
        }
        match self.output_plan(merge) {
            Ok(plan) => self.start(ctx, plan),
            Err(err) => self.report_error(engine::tool_error(&err)),
        }
    }

    pub fn start(&mut self, ctx: &Context, plan: Plan) {
        if self.job.is_some() {
            return;
        }
        let title = plan.title();
        self.flash_monitor = match &plan {
            Plan::Flash { summary, .. } | Plan::DataFlash { summary, .. } => Some(FlashMonitor {
                summary: summary.clone(),
                visible: true,
                outcome: None,
                first_line: self.log_sequence,
            }),
            _ => None,
        };
        self.set_activity(message("working", &[&title]), false);
        self.save();
        let cancel = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel();
        let language = self.preferences.language();
        let worker = JobContext {
            events: tx,
            cancel: cancel.clone(),
            repaint: ctx.clone(),
        };
        let tools = self.tools.clone();
        self.job = Some(Job {
            title,
            progress: None,
            cancel,
            rx,
        });
        thread::spawn(move || {
            i18n::activate(language);
            worker.send(Event::Finished(plan.run(&tools, &worker)));
        });
    }

    pub fn show_help(&mut self, ctx: &Context, label: &str, command: &'static str) {
        let (tx, rx) = mpsc::channel();
        let tools = self.tools.clone();
        let ctx = ctx.clone();
        self.help = Some((label.into(), rx));
        self.help_text = t("loading_help").into();
        thread::spawn(move || {
            let output = engine::esptool_help(&tools, command)
                .unwrap_or_else(|err| engine::tool_error(&err));
            let _ = tx.send(output);
            ctx.request_repaint();
        });
    }

    pub fn tick(&mut self, ctx: &Context) {
        if self.saved != self.project {
            let started = self.dirty_since.get_or_insert_with(Instant::now);
            if started.elapsed() >= Duration::from_millis(450) {
                self.save();
            } else {
                ctx.request_repaint_after(Duration::from_millis(450));
            }
        }
        let mut outcome = None;
        for _ in 0..512 {
            let Some(job) = &mut self.job else { break };
            match job.rx.try_recv() {
                Ok(Event::Log(line)) => self.push_log(line),
                Ok(Event::Progress(progress)) => job.progress = Some(progress),
                Ok(Event::Finished(result)) => {
                    outcome = Some(result);
                    break;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    outcome = Some(Err(anyhow::anyhow!("{}", t("worker_error"))));
                    break;
                }
                Err(mpsc::TryRecvError::Empty) => break,
            }
        }
        if let Some(result) = outcome {
            self.job = None;
            match result {
                Ok(message) => self.set_activity(message, false),
                Err(err) => self.report_error(engine::tool_error(&err)),
            }
            if let Some(monitor) = &mut self.flash_monitor {
                monitor.outcome = Some(if self.error {
                    Err(self.activity.clone())
                } else {
                    Ok(self.activity.clone())
                });
            }
        }
        if self.job.is_some() {
            ctx.request_repaint_after(Duration::from_millis(60));
        }
        if let Some((_, rx)) = &self.help
            && let Ok(text) = rx.try_recv()
        {
            self.help_text = text;
        }
        if self.project.mode == Mode::Data {
            self.tick_scan(ctx);
        }
    }

    fn tick_scan(&mut self, ctx: &Context) {
        let config = &self.project.data;
        let key = (
            config.folder.clone(),
            config.format,
            config.ignore_system,
            config.fat_lfn,
        );
        if self.scan.as_ref().is_none_or(|scan| scan.key != key) {
            self.rescan();
            if !key.0.is_empty() {
                let config = self.project.data.clone();
                let cancel = Arc::new(AtomicBool::new(false));
                let worker_cancel = cancel.clone();
                let (tx, rx) = mpsc::channel();
                let repaint = ctx.clone();
                let language = self.preferences.language();
                self.scan = Some(Scan { key, cancel, rx });
                thread::spawn(move || {
                    i18n::activate(language);
                    let result = engine::scan(&config, || worker_cancel.load(Ordering::Relaxed))
                        .map_err(|err| engine::tool_error(&err));
                    let _ = tx.send(result);
                    repaint.request_repaint();
                });
            }
        }
        if let Some(scan) = &self.scan
            && let Ok(preview) = scan.rx.try_recv()
        {
            self.preview = Some(preview);
        }
    }
}

impl eframe::App for App {
    fn clear_color(&self, _: &egui::Visuals) -> [f32; 4] {
        [0.0; 4]
    }

    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.tick(ctx);
        if ctx.input(|input| input.viewport().close_requested()) {
            if self.job.is_some() {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.report_error(t("close_while_running"));
            } else {
                self.save();
            }
        }
        crate::ui::show(self, ctx);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.save();
    }
}

pub fn dialog_at(path: &str) -> rfd::FileDialog {
    let mut dialog = rfd::FileDialog::new();
    if let Some(parent) = Path::new(path).parent().filter(|p| p.is_dir()) {
        dialog = dialog.set_directory(parent);
    }
    dialog
}

pub fn pick_binary(path: &mut String) -> bool {
    if let Some(chosen) = dialog_at(path)
        .add_filter("Raw binary", &["bin", "img"])
        .pick_file()
    {
        *path = chosen.to_string_lossy().into_owned();
        true
    } else {
        false
    }
}
