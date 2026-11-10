use crate::i18n::{message, t};
use crate::model::{self, Connection, DataConfig, FileSystem, WriteInput, address, hex, number};
use anyhow::{Context, Result, anyhow, bail, ensure};
use std::{
    collections::HashSet,
    ffi::{OsStr, OsString},
    fs,
    io::{BufRead, BufReader, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::Sender,
    },
    thread,
    time::Duration,
};
use walkdir::WalkDir;

#[derive(Clone)]
pub struct Tools {
    pub platform: PathBuf,
    pub shared: PathBuf,
}

impl Tools {
    pub fn locate() -> Self {
        let target = format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH);
        let exe = std::env::current_exe().ok();
        let parent = exe
            .as_deref()
            .and_then(Path::parent)
            .unwrap_or_else(|| Path::new("."));
        let bundled = if cfg!(target_os = "macos") {
            parent.join("../Resources/tools")
        } else {
            parent.join("tools")
        };
        let local = Path::new(env!("CARGO_MANIFEST_DIR")).join("tools");
        let root = if bundled.join(&target).is_dir() {
            bundled
        } else {
            local
        };
        Self {
            platform: root.join(target),
            shared: root.join("shared"),
        }
    }

    pub fn tool(&self, name: &str) -> PathBuf {
        let filename = format!("{name}{}", std::env::consts::EXE_SUFFIX);
        if cfg!(windows) && name == "mklittlefs-legacy" {
            let isolated = self.platform.join("littlefs-legacy").join(&filename);
            if isolated.is_file() {
                return isolated;
            }
        }
        self.platform.join(filename)
    }

    fn image_python(&self) -> PathBuf {
        self.platform.join(if cfg!(windows) {
            "python/python.exe"
        } else {
            "python/bin/python3.12"
        })
    }

    fn image_app(&self) -> PathBuf {
        self.shared.join("idf-image.pyz")
    }

    fn image_generator(&self) -> Invocation {
        let native = self.tool("idf-image");
        if native.is_file() {
            Invocation::new(native)
        } else {
            let mut command = Invocation::new(self.image_python());
            command.args(["-I", "-B", "-X", "utf8"]);
            command.arg(self.image_app());
            command
        }
    }

    pub fn available(&self) -> bool {
        ["esptool", "mklittlefs", "mklittlefs-legacy"]
            .into_iter()
            .all(|name| self.tool(name).is_file())
            && (self.tool("idf-image").is_file()
                || (self.image_python().is_file() && self.image_app().is_file()))
    }
}

pub enum Event {
    Log(String),
    Progress(f32),
    Finished(Result<String>),
}

pub struct JobContext {
    pub events: Sender<Event>,
    pub cancel: Arc<AtomicBool>,
    pub repaint: eframe::egui::Context,
}

impl JobContext {
    pub fn send(&self, event: Event) {
        let _ = self.events.send(event);
        self.repaint.request_repaint();
    }

    fn log(&self, text: impl Into<String>) {
        self.send(Event::Log(text.into()));
    }

    pub fn check_cancel(&self) -> Result<()> {
        ensure!(
            !self.cancel.load(Ordering::Relaxed),
            t("operation_cancelled")
        );
        Ok(())
    }
}

#[derive(Clone)]
pub struct Invocation {
    pub program: PathBuf,
    pub args: Vec<OsString>,
}

impl Invocation {
    fn new(program: PathBuf) -> Self {
        Self {
            program,
            args: Vec::new(),
        }
    }

    fn arg(&mut self, arg: impl AsRef<OsStr>) {
        self.args.push(arg.as_ref().into());
    }

    fn args(&mut self, args: impl IntoIterator<Item = impl AsRef<OsStr>>) {
        for arg in args {
            self.arg(arg);
        }
    }

    pub fn display(&self) -> String {
        let arguments = std::iter::once(self.program.as_os_str())
            .chain(self.args.iter().map(OsString::as_os_str))
            .map(|arg| {
                let arg = arg.to_string_lossy();
                let escaped = if cfg!(windows) {
                    arg.replace('\'', "''")
                } else {
                    arg.replace('\'', "'\\''")
                };
                format!("'{escaped}'")
            })
            .collect::<Vec<_>>()
            .join(" ");
        if cfg!(windows) {
            format!("& {arguments}")
        } else {
            arguments
        }
    }

    fn command(&self) -> Command {
        let mut command = Command::new(&self.program);
        command
            .args(&self.args)
            .stdin(Stdio::null())
            .env("PYTHONUTF8", "1");
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        command
    }

    pub fn capture(&self) -> Result<String> {
        let output = self
            .command()
            .env("COLUMNS", "106")
            .output()
            .with_context(|| message("tool_start_failed", &[&self.program.display()]))?;
        ensure!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }

    pub fn run(&self, ctx: &JobContext) -> Result<()> {
        ctx.check_cancel()?;
        ctx.log(format!("› {}", self.display()));
        let mut command = self.command();
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }
        let mut child = command
            .spawn()
            .with_context(|| message("tool_start_failed", &[&self.program.display()]))?;
        let stdout = child.stdout.take().context(t("worker_error"))?;
        let stderr = child.stderr.take().context(t("worker_error"))?;
        thread::scope(|scope| {
            scope.spawn(|| forward(stdout, ctx));
            scope.spawn(|| forward(stderr, ctx));
            let status = loop {
                if let Some(status) = child.try_wait()? {
                    break status;
                }
                if ctx.cancel.load(Ordering::Relaxed) {
                    terminate(&mut child);
                    let _ = child.wait();
                    bail!(t("write_cancelled"));
                }
                thread::sleep(Duration::from_millis(35));
            };
            ensure!(
                status.success(),
                message(
                    "tool_failed",
                    &[
                        &self
                            .program
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy(),
                        &status
                    ]
                )
            );
            Ok(())
        })
    }
}

fn terminate(child: &mut Child) {
    #[cfg(unix)]
    {
        // The helper and its PyInstaller child share a process group.
        unsafe { libc::kill(-(child.id() as i32), libc::SIGKILL) };
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let _ = Command::new("taskkill")
            .args(["/PID", &child.id().to_string(), "/T", "/F"])
            .creation_flags(0x08000000)
            .output();
    }
    let _ = child.kill();
}

fn forward(pipe: impl Read, ctx: &JobContext) {
    let mut reader = BufReader::new(pipe);
    let mut line = Vec::new();
    while let Ok(buffer) = reader.fill_buf() {
        if buffer.is_empty() {
            break;
        }
        let consumed = buffer.len();
        for &byte in buffer {
            if (byte == b'\r' || byte == b'\n' || line.len() >= 32768) && !line.is_empty() {
                emit_line(&line, ctx);
                line.clear();
            }
            if byte != b'\r' && byte != b'\n' {
                line.push(byte);
            }
        }
        reader.consume(consumed);
    }
    if !line.is_empty() {
        emit_line(&line, ctx);
    }
}

fn emit_line(line: &[u8], ctx: &JobContext) {
    let line = String::from_utf8_lossy(line).into_owned();
    if let Some((prefix, _)) = line.rsplit_once('%') {
        let digits: String = prefix
            .trim_end()
            .chars()
            .rev()
            .take_while(|c| c.is_ascii_digit() || *c == '.')
            .collect();
        let number: String = digits.chars().rev().collect();
        if let Ok(value) = number.parse::<f32>() {
            ctx.send(Event::Progress((value / 100.0).clamp(0.0, 1.0)));
        }
    }
    ctx.log(line);
}

#[derive(Clone, Copy)]
enum OptionScope {
    Global,
    Write,
    Merge,
}

fn add_extra(command: &mut Invocation, text: &str, scope: OptionScope) -> Result<()> {
    let mut args = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .peekable();
    let mut seen = HashSet::new();
    while let Some(flag) = args.next() {
        let count = match (scope, flag) {
            (
                OptionScope::Global,
                "--port-filter" | "--override-vddsdio" | "--connect-attempts",
            )
            | (OptionScope::Write, "--spi-connection")
            | (OptionScope::Merge, "--target-offset" | "--pad-to-size" | "--chunk-size") => 1,
            (OptionScope::Write, "--diff-with") => usize::MAX,
            (OptionScope::Global, "--verbose" | "--silent")
            | (
                OptionScope::Write,
                "--no-progress"
                | "--ignore-flash-enc-efuse"
                | "--trust-flash-content"
                | "--skip-flashed"
                | "--force",
            )
            | (OptionScope::Merge, "--md5-disable") => 0,
            _ => bail!(message("invalid_option", &[&flag])),
        };
        ensure!(
            flag == "--port-filter" || seen.insert(flag),
            message("invalid_option", &[&flag])
        );
        command.arg(flag);
        if count != 0 {
            let value = args
                .next()
                .with_context(|| message("invalid_option", &[&flag]))?;
            ensure!(
                !value.starts_with("--"),
                message("invalid_option", &[&flag])
            );
            command.arg(value);
            if count == usize::MAX {
                while let Some(value) = args.next_if(|arg| !arg.starts_with("--")) {
                    command.arg(value);
                }
            }
        }
    }
    Ok(())
}

pub fn esptool_help(tools: &Tools, subcommand: &str) -> Result<String> {
    let mut command = Invocation::new(tools.tool("esptool"));
    if !subcommand.is_empty() {
        command.arg(subcommand);
    }
    command.arg("--help");
    command.capture()
}

pub fn flash_command(
    tools: &Tools,
    options: &Connection,
    inputs: &[WriteInput],
    data: bool,
) -> Result<Invocation> {
    let mut command = Invocation::new(tools.tool("esptool"));
    let baud: u32 = options.baud.trim().parse().context(t("invalid_baud"))?;
    ensure!(baud > 0, t("invalid_baud"));
    ensure!(
        options.auto_port || !options.port.trim().is_empty(),
        t("select_port_hint")
    );
    command.args([
        "--chip",
        &options.chip,
        "--baud",
        &options.baud,
        "--before",
        &options.before,
        "--after",
        &options.after,
    ]);
    if !options.auto_port {
        command.args(["--port", options.port.trim()]);
    }
    if options.no_stub {
        command.arg("--no-stub");
    }
    if options.trace {
        command.arg("--trace");
    }
    add_extra(&mut command, &options.globals, OptionScope::Global)?;
    command.arg("write-flash");
    if !data {
        flash_header(&mut command, options);
    } else if options.flash_size == "detect" {
        command.args(["--flash-size", "detect"]);
    }
    match options.compression.as_str() {
        "compress" => command.arg("--compress"),
        "no-compress" => command.arg("--no-compress"),
        _ => {}
    }
    if options.erase_all {
        ensure!(!data, t("data_erase_block"));
        command.arg("--erase-all");
    }
    let selected = inputs.iter().filter(|input| input.encrypt).count();
    ensure!(selected == 0 || !options.encrypt, t("encryption_conflict"));
    if options.encrypt || selected == inputs.len() {
        command.arg("--encrypt");
    }
    add_extra(&mut command, &options.write_options, OptionScope::Write)?;
    if selected > 0 && selected < inputs.len() {
        command.arg("--encrypt-files");
        append_inputs(&mut command, inputs.iter().filter(|input| input.encrypt));
        command.arg("--");
        append_inputs(&mut command, inputs.iter().filter(|input| !input.encrypt));
    } else {
        command.arg("--");
        append_inputs(&mut command, inputs);
    }
    Ok(command)
}

fn flash_header(command: &mut Invocation, options: &Connection) {
    command.args([
        "--flash-mode",
        &options.flash_mode,
        "--flash-size",
        &options.flash_size,
        "--flash-freq",
        &options.flash_freq,
    ]);
}

fn append_inputs<'a>(command: &mut Invocation, inputs: impl IntoIterator<Item = &'a WriteInput>) {
    for input in inputs {
        command.arg(hex(input.address));
        command.arg(&input.path);
    }
}

pub fn merge_command(
    tools: &Tools,
    options: &Connection,
    inputs: &[WriteInput],
    format: &str,
    output: &Path,
) -> Result<Invocation> {
    ensure!(
        !options.chip.trim().is_empty() && options.chip != "auto",
        t("merge_chip")
    );
    ensure!(matches!(format, "raw" | "hex" | "uf2"), t("invalid_format"));
    let mut command = Invocation::new(tools.tool("esptool"));
    command.args([
        "--chip",
        &options.chip,
        "merge-bin",
        "--format",
        format,
        "--output",
    ]);
    command.arg(output);
    let mut header = options.clone();
    if header.flash_size == "detect" {
        header.flash_size = "keep".into();
    }
    flash_header(&mut command, &header);
    add_extra(&mut command, &options.merge_options, OptionScope::Merge)?;
    command.arg("--");
    append_inputs(&mut command, inputs);
    Ok(command)
}

pub fn save_output(
    output: &Path,
    inputs: &[WriteInput],
    make: impl FnOnce(&Path) -> Result<()>,
) -> Result<()> {
    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    if let Ok(target) = output.canonicalize() {
        for input in inputs {
            ensure!(input.path.canonicalize()? != target, t("same_output"));
        }
    }
    let mut file = tempfile::NamedTempFile::new_in(parent)
        .context(message("save_failed", &[&output.display()]))?;
    make(file.path())?;
    ensure!(file.path().metadata()?.len() > 0, t("output_invalid"));
    file.flush()?;
    file.persist(output)
        .with_context(|| message("save_failed", &[&output.display()]))?;
    Ok(())
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Category {
    Web,
    Media,
    Config,
    Other,
}

impl Category {
    fn of(path: &Path) -> Self {
        match path
            .extension()
            .and_then(OsStr::to_str)
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str()
        {
            "html" | "htm" | "css" | "js" | "wasm" | "svg" => Self::Web,
            "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp" | "ico" | "wav" | "mp3" | "ogg"
            | "mp4" => Self::Media,
            "json" | "yaml" | "yml" | "txt" | "md" | "ini" | "toml" | "cfg" | "csv" => Self::Config,
            _ => Self::Other,
        }
    }
}

#[derive(Clone)]
pub struct Entry {
    pub path: PathBuf,
    pub directory: bool,
    pub size: u64,
}

pub struct Preview {
    pub entries: Vec<Entry>,
    pub sizes: [u64; 4],
    pub file_count: usize,
    pub skipped: usize,
}

pub fn scan(config: &DataConfig, cancelled: impl Fn() -> bool) -> Result<Preview> {
    let root = Path::new(&config.folder);
    ensure!(
        !config.folder.trim().is_empty() && root.is_dir(),
        t("folder_invalid")
    );
    let mut preview = Preview {
        entries: Vec::new(),
        sizes: [0; 4],
        file_count: 0,
        skipped: 0,
    };
    let mut folded_paths = HashSet::new();
    let mut entries = WalkDir::new(root)
        .min_depth(1)
        .sort_by_file_name()
        .follow_links(false)
        .into_iter();
    while let Some(entry) = entries.next() {
        ensure!(!cancelled(), t("operation_cancelled"));
        let entry = entry.context(t("folder_failed"))?;
        let name = entry
            .file_name()
            .to_str()
            .context(message("filename_invalid", &[&entry.path().display()]))?;
        if config.ignore_system
            && (matches!(name, ".DS_Store" | "__MACOSX" | "Thumbs.db" | "desktop.ini")
                || name.starts_with("._"))
        {
            if entry.file_type().is_dir() {
                entries.skip_current_dir();
            }
            preview.skipped += 1;
            continue;
        }
        let path = entry.path().strip_prefix(root)?.to_owned();
        let file_type = entry.file_type();
        if !file_type.is_file() && !file_type.is_dir() {
            preview.skipped += 1;
            continue;
        }
        if config.format == FileSystem::FatFs {
            validate_fat_name(name, file_type.is_dir(), config.fat_lfn)?;
            ensure!(
                folded_paths.insert(path.to_string_lossy().to_uppercase()),
                message("filename_invalid", &[&path.display()])
            );
        }
        let size = if file_type.is_file() {
            let bytes = entry.metadata()?.len();
            preview.sizes[Category::of(&path) as usize] += bytes;
            preview.file_count += 1;
            bytes
        } else {
            0
        };
        preview.entries.push(Entry {
            path,
            directory: file_type.is_dir(),
            size,
        });
    }
    // SPIFFS and the bundled mklittlefs only add directories represented by files.
    if config.format != FileSystem::FatFs {
        let populated: HashSet<_> = preview
            .entries
            .iter()
            .filter(|e| !e.directory)
            .flat_map(|entry| entry.path.ancestors().skip(1).map(Path::to_path_buf))
            .collect();
        preview.entries.retain(|entry| {
            let keep = !entry.directory || populated.contains(&entry.path);
            preview.skipped += usize::from(!keep);
            keep
        });
    }
    Ok(preview)
}

fn validate_fat_name(name: &str, directory: bool, lfn: bool) -> Result<()> {
    ensure!(
        !name.starts_with(' ')
            && !name.ends_with([' ', '.'])
            && !name
                .chars()
                .any(|c| c.is_control() || "\"*/:<>?\\|".contains(c))
            && name.len() <= 255
            && name.chars().all(|c| u32::from(c) <= 0xffff),
        message("filename_invalid", &[&name])
    );
    if !lfn {
        let (base, ext) = if directory {
            (name, "")
        } else {
            name.rsplit_once('.').unwrap_or((name, ""))
        };
        let valid = |part: &str| {
            part.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"!#$%&'()-@^_`{}~".contains(&b))
        };
        ensure!(
            !base.is_empty() && base.len() <= 8 && ext.len() <= 3 && valid(base) && valid(ext),
            message("filename_short", &[&name])
        );
    }
    Ok(())
}

fn populate_fat(
    config: &DataConfig,
    preview: &Preview,
    staged: &Path,
    output: &Path,
    ctx: &JobContext,
) -> Result<()> {
    // ESP-IDF WL starts with one 4 KiB dummy sector. The FAT boot sector
    // declares its own length before the two state copies and the config sector.
    let start = if config.fat_layout == "raw" { 0 } else { 4096 };
    let mut image = fs::OpenOptions::new().read(true).write(true).open(output)?;
    image.seek(SeekFrom::Start(start))?;
    let mut bpb = [0u8; 36];
    image.read_exact(&mut bpb)?;
    let sector = u64::from(u16::from_le_bytes([bpb[11], bpb[12]]));
    let short_count = u64::from(u16::from_le_bytes([bpb[19], bpb[20]]));
    let long_count = u64::from(u32::from_le_bytes([bpb[32], bpb[33], bpb[34], bpb[35]]));
    let sectors = if short_count == 0 {
        long_count
    } else {
        short_count
    };
    let end = start + sectors * sector;
    ensure!(
        matches!(sector, 512 | 4096) && sectors > 0 && end <= image.metadata()?.len(),
        t("filesystem_geometry")
    );
    let stream = fscommon::StreamSlice::new(image, start, end)?;
    let fat = fatfs::FileSystem::new(fscommon::BufStream::new(stream), fatfs::FsOptions::new())
        .context(message("read_failed", &[&output.display()]))?;
    {
        let root = fat.root_dir();
        for entry in &preview.entries {
            ctx.check_cancel()?;
            let name = entry
                .path
                .components()
                .map(|component| component.as_os_str().to_str().unwrap_or_default())
                .collect::<Vec<_>>()
                .join("/");
            if entry.directory {
                root.create_dir(&name)
                    .with_context(|| message("save_failed", &[&name]))?;
            } else {
                let mut source = fs::File::open(staged.join(&entry.path))?;
                let mut file = root
                    .create_file(&name)
                    .with_context(|| message("save_failed", &[&name]))?;
                std::io::copy(&mut source, &mut file)
                    .with_context(|| message("save_failed", &[&name]))?;
                file.flush()?;
            }
        }
    }
    let stats = fat.stats()?;
    ctx.log(message(
        "remaining",
        &[&model::human_size(
            u64::from(stats.free_clusters()) * u64::from(stats.cluster_size()),
        )],
    ));
    fat.unmount()?;
    Ok(())
}

pub fn data_size(config: &DataConfig) -> Result<u64> {
    let size = number(&config.size).context(t("invalid_capacity"))?;
    ensure!(
        (8192..=256 * 1024 * 1024).contains(&size) && size % 4096 == 0,
        t("capacity_alignment")
    );
    Ok(size)
}

fn geometry(config: &DataConfig, size: u64) -> Result<(u64, u64)> {
    let block = number(&config.block)?;
    let page = number(if config.format == FileSystem::LittleFs {
        &config.littlefs_page
    } else {
        &config.page
    })?;
    ensure!(
        block.is_power_of_two()
            && page.is_power_of_two()
            && page >= 16
            && block / 2 >= page
            && size.is_multiple_of(block)
            && size / block >= 2,
        t("filesystem_geometry")
    );
    Ok((block, page))
}

pub fn image_command(
    tools: &Tools,
    config: &DataConfig,
    size: u64,
    staged: &Path,
    output: &Path,
) -> Result<Invocation> {
    match config.format {
        FileSystem::LittleFs => {
            let (block, page) = geometry(config, size)?;
            let program = if config.custom_littlefs.is_empty() {
                tools.tool(if config.littlefs_legacy {
                    "mklittlefs-legacy"
                } else {
                    "mklittlefs"
                })
            } else {
                PathBuf::from(&config.custom_littlefs)
            };
            let mut command = Invocation::new(program);
            command.args(["-a", "-c"]);
            command.arg(staged);
            command.args([
                "-s",
                &size.to_string(),
                "-b",
                &block.to_string(),
                "-p",
                &page.to_string(),
            ]);
            command.arg(output);
            Ok(command)
        }
        FileSystem::Spiffs => {
            let (block, page) = geometry(config, size)?;
            let name = number(&config.spiffs_name)?;
            let meta = number(&config.spiffs_meta)?;
            ensure!(
                name > 1
                    && name
                        .checked_add(meta)
                        .is_some_and(|sum| sum <= page.saturating_sub(64)),
                t("filesystem_geometry")
            );
            let mut command = tools.image_generator();
            command.args(["spiffs", &size.to_string()]);
            command.arg(staged);
            command.arg(output);
            command.args([
                "--block-size",
                &block.to_string(),
                "--page-size",
                &page.to_string(),
                "--obj-name-len",
                &name.to_string(),
                "--meta-len",
                &meta.to_string(),
            ]);
            command.arg(if config.spiffs_magic {
                "--use-magic"
            } else {
                "--no-magic"
            });
            command.arg(if config.spiffs_magic && config.spiffs_magic_len {
                "--use-magic-len"
            } else {
                "--no-magic-len"
            });
            if config.spiffs_aligned {
                command.arg("--aligned-obj-ix-tables");
            }
            Ok(command)
        }
        FileSystem::FatFs => {
            let raw = config.fat_layout == "raw";
            let sector = if config.fat_layout == "safe" {
                "512"
            } else if raw {
                "4096"
            } else {
                &config.fat_sector
            };
            let min = if raw {
                16
            } else if config.fat_layout == "safe" {
                28
            } else if sector == "512" {
                20
            } else {
                32
            };
            ensure!(size >= min * 1024, message("fat_minimum", &[&min]));
            let mut command = tools.image_generator();
            command.arg(if raw { "fatfs" } else { "fatfs-wl" });
            command.arg(staged);
            command.arg("--output_file");
            command.arg(output);
            command.args([
                "--partition_size",
                &size.to_string(),
                "--sector_size",
                sector,
                "--sectors_per_cluster",
                &config.fat_clusters,
                "--fat_count",
                &config.fat_count,
                "--use_default_datetime",
            ]);
            if !raw && sector == "512" {
                command.args([
                    "--wl_mode",
                    if config.fat_layout == "safe" {
                        "safe"
                    } else {
                        "perf"
                    },
                ]);
            }
            if config.fat_lfn {
                command.arg("--long_name_support");
            }
            Ok(command)
        }
    }
}

pub fn generate_image(
    tools: &Tools,
    config: &DataConfig,
    output: &Path,
    ctx: &JobContext,
) -> Result<u64> {
    let size = data_size(config)?;
    ctx.log(format!(
        "{} · {} · {}",
        t("build_image"),
        config.format.name(),
        model::human_size(size)
    ));
    let preview = scan(config, || ctx.cancel.load(Ordering::Relaxed))?;
    let bytes: u64 = preview.sizes.iter().sum();
    ensure!(bytes <= size, t("over_capacity"));
    if config.format == FileSystem::Spiffs {
        let max = number(&config.spiffs_name)?;
        for entry in preview.entries.iter().filter(|entry| !entry.directory) {
            let len = entry.path.to_string_lossy().len() as u64;
            ensure!(
                len + 1 < max,
                message("spiffs_path", &[&entry.path.display()])
            );
        }
    }
    let staged = tempfile::tempdir().context(t("worker_error"))?;
    ctx.log(message(
        "file_count",
        &[
            &preview.file_count,
            &model::human_size(bytes),
            &preview.skipped,
        ],
    ));
    for entry in &preview.entries {
        ctx.check_cancel()?;
        let source = Path::new(&config.folder).join(&entry.path);
        let destination = staged.path().join(&entry.path);
        if entry.directory {
            fs::create_dir(&destination)?;
        } else {
            let metadata = fs::symlink_metadata(&source)?;
            ensure!(
                metadata.is_file() && metadata.len() == entry.size,
                message("source_changed", &[&source.display()])
            );
            let copied = fs::copy(&source, destination)
                .with_context(|| message("read_failed", &[&source.display()]))?;
            ensure!(
                copied == entry.size,
                message("source_changed", &[&source.display()])
            );
        }
    }
    if config.format == FileSystem::FatFs && config.fat_lfn {
        // ESP-IDF initializes the exact partition geometry and wear-level metadata.
        // Populate its blank FAT volume using Unicode LFN entries and valid ASCII aliases.
        let empty = tempfile::tempdir()?;
        image_command(tools, config, size, empty.path(), output)?.run(ctx)?;
        populate_fat(config, &preview, staged.path(), output, ctx)?;
    } else {
        image_command(tools, config, size, staged.path(), output)?.run(ctx)?;
    }
    let actual = output.metadata()?.len();
    ensure!(
        actual == size,
        format!("{} ({actual} B ≠ {size} B)", t("output_invalid"))
    );
    if config.format == FileSystem::LittleFs {
        let mut verify = image_command(tools, config, size, staged.path(), output)?;
        verify.args.clear();
        let (block, page) = geometry(config, size)?;
        verify.args([
            "-l",
            "-s",
            &size.to_string(),
            "-b",
            &block.to_string(),
            "-p",
            &page.to_string(),
        ]);
        verify.arg(output);
        let listing = verify
            .capture()
            .context(message("read_failed", &[&output.display()]))?;
        // mklittlefs reports each stored file with its byte count.
        let mut sizes: Vec<u64> = listing
            .lines()
            .filter_map(|line| line.split_whitespace().next()?.parse().ok())
            .collect();
        let mut expected: Vec<u64> = preview
            .entries
            .iter()
            .filter(|entry| !entry.directory)
            .map(|entry| entry.size)
            .collect();
        sizes.sort_unstable();
        expected.sort_unstable();
        ensure!(sizes == expected, t("output_invalid"));
    }
    Ok(actual)
}

pub fn data_input(
    config: &DataConfig,
    path: PathBuf,
    size: u64,
    options: &Connection,
) -> Result<WriteInput> {
    let address = address(&config.address).context(t("invalid_address"))?;
    ensure!(address % 4096 == 0, t("align_4k"));
    let limit = number(options.flash_size.trim_end_matches("-c1"))
        .unwrap_or(u32::MAX as u64 + 1)
        .min(u32::MAX as u64 + 1);
    ensure!(address + size <= limit, t("flash_capacity"));
    Ok(WriteInput {
        address,
        path,
        size,
        encrypt: false,
    })
}

pub fn validate_image_destination(output: &Path, folder: &Path) -> Result<()> {
    let parent = output.parent().context(t("invalid_path"))?.canonicalize()?;
    ensure!(
        !parent.starts_with(folder.canonicalize()?),
        t("image_outside")
    );
    Ok(())
}

pub fn port_names() -> Result<Vec<(String, String)>> {
    let mut ports: Vec<_> = serialport::available_ports()?
        .into_iter()
        .map(|port| {
            let description = match port.port_type {
                serialport::SerialPortType::UsbPort(usb) => usb
                    .product
                    .unwrap_or_else(|| format!("USB {:04X}:{:04X}", usb.vid, usb.pid)),
                serialport::SerialPortType::BluetoothPort => "Bluetooth".to_owned(),
                _ => String::new(),
            };
            (port.port_name, description)
        })
        .collect();
    if cfg!(target_os = "macos") {
        let names: HashSet<_> = ports.iter().map(|port| port.0.clone()).collect();
        ports.retain(|(name, _)| {
            !name.starts_with("/dev/tty.")
                || !names.contains(&name.replacen("/dev/tty.", "/dev/cu.", 1))
        });
    }
    ports.sort_by_key(|(name, description)| {
        (
            description == "Bluetooth" || !name.contains("usb"),
            name.clone(),
        )
    });
    Ok(ports)
}

pub fn tool_error(error: &anyhow::Error) -> String {
    format!("{error:#}")
}

pub fn open_output_path(path: &str) -> Result<PathBuf> {
    let output = PathBuf::from(path);
    ensure!(output.file_name().is_some(), t("choose_output"));
    if output.is_absolute() {
        Ok(output)
    } else {
        Ok(std::env::current_dir()
            .map_err(|err| anyhow!("{} {err}", t("invalid_path")))?
            .join(output))
    }
}
