mod chrome;
mod icons;
mod progress;
mod style;
mod tree;

pub use chrome::install_github_icon;
pub use style::install as install_style;

use crate::{
    app::{self, App},
    i18n::{message, t},
    model::{self, FileSystem, Mode, Segment},
};
use eframe::egui::{
    self, Align, Button, ComboBox, Context, CornerRadius, Frame, Layout, Margin, Rect, RichText,
    ScrollArea, Stroke, TextEdit, TextStyle, Ui, Vec2,
};
use egui_extras::{Column, TableBuilder};
use icons::Icon;
use std::path::Path;
use style::{CONTROL_HEIGHT, CONTROL_RADIUS, input, palette, primary};

const CHIPS: &[&str] = &[
    "auto", "esp32", "esp32s2", "esp32s3", "esp32s31", "esp32c2", "esp32c3", "esp32c5", "esp32c6",
    "esp32c61", "esp32e22", "esp32h2", "esp32h21", "esp32h4", "esp32p4", "esp8266",
];
const RATES: &[&str] = &[
    "115200", "230400", "460800", "500000", "921600", "1000000", "1500000", "2000000", "3000000",
];
const FLASH_SIZES: &[&str] = &[
    "keep", "detect", "256KB", "512KB", "1MB", "2MB", "2MB-c1", "4MB", "4MB-c1", "8MB", "16MB",
    "32MB", "64MB", "128MB",
];
const FLASH_FREQS: &[&str] = &[
    "keep", "80m", "64m", "60m", "48m", "40m", "32m", "30m", "26m", "24m", "20m", "16m", "15m",
    "12m",
];

pub fn show(app: &mut App, ctx: &Context) {
    let p = palette(ctx);
    let radius = style::window_radius(ctx);
    egui::CentralPanel::default()
        .frame(Frame::NONE)
        .show(ctx, |root| {
            let bounds = root.max_rect();
            root.painter().rect_filled(bounds, radius, p.canvas);
            let top = root.painter().add(egui::Shape::Noop);
            chrome::title_bar(app, root);
            workspace_bar(app, root);
            root.painter().set(
                top,
                egui::epaint::RectShape::filled(
                    Rect::from_min_max(bounds.min, egui::pos2(bounds.right(), root.cursor().top())),
                    CornerRadius {
                        nw: radius,
                        ne: radius,
                        sw: 0,
                        se: 0,
                    },
                    p.panel,
                ),
            );
            status_panel(app, root, radius);
            let width = (root.available_width() * 0.33).clamp(263.0, 350.0);
            egui::SidePanel::right("actions")
                .exact_width(width)
                .show_separator_line(false)
                .resizable(false)
                .frame(Frame::new().fill(p.canvas).inner_margin(Margin {
                    left: 4,
                    right: 20,
                    top: 18,
                    bottom: 14,
                }))
                .show_inside(root, |ui| {
                    ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| actions(app, ctx, ui));
                });
            egui::CentralPanel::default()
                .frame(Frame::new().fill(p.canvas).inner_margin(Margin {
                    left: 20,
                    right: 12,
                    top: 18,
                    bottom: 14,
                }))
                .show_inside(root, |ui| {
                    ui.add_enabled_ui(app.job.is_none(), |ui| {
                        ScrollArea::vertical()
                            .id_salt(mode_keys(app.project.mode).0)
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                let (title, description, guide) = mode_keys(app.project.mode);
                                ui.heading(t(title));
                                hint(ui, t(description));
                                if app.preferences.guide {
                                    Frame::new()
                                        .fill(p.tint)
                                        .corner_radius(9)
                                        .inner_margin(Margin::same(10))
                                        .show(ui, |ui| {
                                            ui.set_min_width(ui.available_width());
                                            ui.label(RichText::new(t(guide)).color(p.accent));
                                        });
                                }
                                ui.add_space(4.0);
                                match app.project.mode {
                                    Mode::Firmware => firmware(app, ui),
                                    Mode::Partitions => partitions(app, ui),
                                    Mode::Data => data(app, ui),
                                }
                            });
                    });
                });
        });
    dialogs(app, ctx);
    chrome::about(app, ctx);
    progress::monitor(app, ctx);
    chrome::resize_edges(ctx);
}

fn mode_keys(mode: Mode) -> (&'static str, &'static str, &'static str) {
    match mode {
        Mode::Firmware => ("firmware", "firmware_desc", "firmware_guide"),
        Mode::Partitions => ("partitions", "partitions_desc", "partitions_guide"),
        Mode::Data => ("data", "data_desc", "data_guide"),
    }
}

fn workspace_bar(app: &mut App, root: &mut Ui) {
    let p = palette(root.ctx());
    egui::TopBottomPanel::top("workspace_bar")
        .show_separator_line(false)
        .frame(Frame::NONE)
        .show_inside(root, |ui| {
            ui.add_enabled_ui(app.job.is_none(), |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                let baseline = ui.painter().add(egui::Shape::Noop);
                let tabs = ui.horizontal(|ui| {
                    ui.add_space(20.0 - ui.spacing().item_spacing.x);
                    ui.spacing_mut().item_spacing.x = 4.0;
                    for (mode, icon) in [
                        (Mode::Firmware, Icon::Chip),
                        (Mode::Partitions, Icon::Layers),
                        (Mode::Data, Icon::Folder),
                    ] {
                        let selected = app.project.mode == mode;
                        let label = t(mode_keys(mode).0);
                        let background = ui.painter().add(egui::Shape::Noop);
                        let seam = ui.painter().add(egui::Shape::Noop);
                        let color = if selected { p.accent } else { p.muted };
                        let mut tab = icon.button(RichText::new(label).color(color));
                        tab.color = Some(color);
                        // Let the translated label size the tab, with extra space around it.
                        tab.button = tab
                            .button
                            .fill(egui::Color32::TRANSPARENT)
                            .stroke(Stroke::NONE)
                            .min_size(Vec2::new(144.0, 40.0));
                        let response = ui.add(tab);
                        response.widget_info(|| {
                            egui::WidgetInfo::selected(
                                egui::WidgetType::SelectableLabel,
                                ui.is_enabled(),
                                selected,
                                label,
                            )
                        });
                        if selected || response.hovered() || response.has_focus() {
                            let rect = response.rect;
                            let corners = CornerRadius {
                                nw: 9,
                                ne: 9,
                                sw: 0,
                                se: 0,
                            };
                            let (fill, border) = if selected {
                                (p.canvas, p.border)
                            } else if response.has_focus() {
                                (p.tint, p.accent)
                            } else {
                                (p.tint, p.tint)
                            };
                            ui.painter().set(
                                background,
                                egui::epaint::RectShape::new(
                                    rect,
                                    corners,
                                    fill,
                                    Stroke::new(1.0, border),
                                    egui::StrokeKind::Inside,
                                ),
                            );
                            if selected {
                                ui.painter().set(
                                    seam,
                                    egui::epaint::RectShape::filled(
                                        Rect::from_min_max(
                                            egui::pos2(rect.left() + 1.0, rect.bottom() - 2.0),
                                            egui::pos2(rect.right() - 1.0, rect.bottom()),
                                        ),
                                        0,
                                        p.canvas,
                                    ),
                                );
                            }
                        }
                        if response.clicked() {
                            app.project.mode = mode;
                        }
                    }
                });
                ui.painter().set(
                    baseline,
                    egui::Shape::line_segment(
                        [
                            egui::pos2(ui.max_rect().left(), tabs.response.rect.bottom() - 0.5),
                            egui::pos2(ui.max_rect().right(), tabs.response.rect.bottom() - 0.5),
                        ],
                        Stroke::new(1.0, p.border),
                    ),
                );
            });
        });
}

fn firmware(app: &mut App, ui: &mut Ui) {
    let firmware = &mut app.project.firmware;
    card(ui, |ui| {
        heading(ui, "firmware_file");
        path_field(
            ui,
            &mut firmware.path,
            "no_file",
            "browse_file",
            app::pick_binary,
        );
        if let Some(bytes) = file_size(&firmware.path) {
            hint(ui, &model::human_size(bytes));
        }
        if app.preferences.advanced {
            field(ui, "start_address", &mut firmware.address, "0x000000");
        } else {
            ui.collapsing(message("write_at", &[&firmware.address]), |ui| {
                field(ui, "start_address", &mut firmware.address, "0x000000");
            });
        }
    });
}

fn partitions(app: &mut App, ui: &mut Ui) {
    card(ui, |ui| {
        heading(ui, "partition_files");
        ui.horizontal_wrapped(|ui| {
            csv_buttons(app, ui);
            if ui.add(Icon::Plus.button(t("add_partition"))).clicked() {
                app.project.segments.push(Segment {
                    enabled: false,
                    ..Default::default()
                });
            }
        });
        if app.project.segments.is_empty() {
            hint(ui, t("empty_partitions"));
        } else {
            partition_table(ui, &mut app.project.segments, app.preferences.advanced);
        }
        if app.preferences.advanced || !app.project.csv_path.is_empty() {
            details(ui, "csv_settings", app.preferences.advanced, |ui| {
                csv_settings(app, ui)
            });
        }
    });
}

fn csv_buttons(app: &mut App, ui: &mut Ui) {
    if ui.add(Icon::File.button(t("import_csv"))).clicked() {
        app.choose_csv();
    }
    if let Some(name) = Path::new(&app.project.csv_path).file_name() {
        let reload = ui
            .add(Icon::Refresh.button(t("reload_csv")))
            .on_hover_text(name.to_string_lossy());
        if reload.clicked() {
            app.load_csv();
        }
    }
}

fn csv_settings(app: &mut App, ui: &mut Ui) {
    field(ui, "table_offset", &mut app.project.table_offset, "0x8000");
    field(
        ui,
        "boot_offset",
        &mut app.project.boot_offset,
        t("by_chip"),
    );
    field(
        ui,
        "recovery_offset",
        &mut app.project.recovery_offset,
        t("optional"),
    );
    hint(ui, t("csv_hint"));
}

fn partition_table(ui: &mut Ui, segments: &mut Vec<Segment>, advanced: bool) {
    let p = palette(ui.ctx());
    let mut remove = None;
    let (name_width, address_width, size_width) = (72.0, 86.0, 86.0);
    let fixed_width = 23.0
        + name_width
        + address_width
        + size_width
        + CONTROL_HEIGHT
        + 5.0 * ui.spacing().item_spacing.x
        + if advanced {
            CONTROL_HEIGHT + ui.spacing().item_spacing.x
        } else {
            0.0
        };
    let file_width = (ui.available_width() - fixed_width).max(126.0);
    ScrollArea::horizontal()
        .id_salt("partitions")
        .show(ui, |ui| {
            ui.set_width(fixed_width + file_width);
            let table = TableBuilder::new(ui)
                .striped(true)
                .vscroll(false)
                .cell_layout(Layout::left_to_right(Align::Center))
                .column(Column::exact(23.0))
                .column(Column::exact(name_width))
                .column(Column::exact(address_width))
                .column(Column::exact(size_width));
            let table = if advanced {
                table.column(Column::exact(CONTROL_HEIGHT))
            } else {
                table
            };
            table
                .column(Column::exact(file_width))
                .column(Column::exact(CONTROL_HEIGHT))
                .header(24.0, |mut head| {
                    let columns: &[&str] = if advanced {
                        &["", "partition", "address", "size", "encrypt", "file", ""]
                    } else {
                        &["", "partition", "address", "size", "file", ""]
                    };
                    for &key in columns {
                        head.col(|ui| {
                            if !key.is_empty() {
                                ui.add(
                                    egui::Label::new(RichText::new(t(key)).small().color(p.muted))
                                        .truncate(),
                                );
                            }
                        });
                    }
                })
                .body(|mut body| {
                    for (index, row) in segments.iter_mut().enumerate() {
                        body.row(CONTROL_HEIGHT + 5.0, |mut cell| {
                            cell.col(|ui| {
                                let has_file = !row.path.trim().is_empty();
                                let mut included = row.enabled && has_file;
                                let response = ui.add_enabled(
                                    has_file,
                                    egui::Checkbox::without_text(&mut included),
                                );
                                if response.changed() {
                                    row.enabled = included;
                                }
                                response.on_hover_text(t("include_partition"));
                            });
                            cell.col(|ui| {
                                inline(ui, &mut row.name, name_width, "—").on_hover_text(&row.kind);
                            });
                            cell.col(|ui| {
                                inline(ui, &mut row.address, address_width, "0x000000");
                            });
                            cell.col(|ui| {
                                let capacity = model::number(&row.size)
                                    .ok()
                                    .map(model::human_size)
                                    .unwrap_or_default();
                                inline(ui, &mut row.size, size_width, "—").on_hover_text(capacity);
                            });
                            if advanced {
                                cell.col(|ui| {
                                    ui.checkbox(&mut row.encrypt, "")
                                        .on_hover_text(t("encrypt_file"));
                                });
                            }
                            cell.col(|ui| {
                                let field_width =
                                    file_width - CONTROL_HEIGHT - ui.spacing().item_spacing.x;
                                let response = ui.add(
                                    input(ui, &mut row.path, "—")
                                        .desired_width((field_width - 18.0).max(24.0)),
                                );
                                if response.changed() {
                                    row.enabled = !row.path.is_empty();
                                }
                                response.on_hover_text(&row.path);
                                if ui.button("…").on_hover_text(t("browse_file")).clicked()
                                    && app::pick_binary(&mut row.path)
                                {
                                    row.enabled = true;
                                }
                            });
                            cell.col(|ui| {
                                if ui
                                    .button("×")
                                    .on_hover_text(t("remove_partition"))
                                    .clicked()
                                {
                                    remove = Some(index);
                                }
                            });
                        });
                    }
                });
        });
    if let Some(index) = remove {
        segments.remove(index);
    }
}

fn data(app: &mut App, ui: &mut Ui) {
    card(ui, |ui| {
        heading(ui, "source_folder");
        let mut folder = std::mem::take(&mut app.project.data.folder);
        let mut browse = false;
        path_field(ui, &mut folder, "no_folder", "browse_folder", |_| {
            browse = true;
            false
        });
        app.project.data.folder = folder;
        if browse {
            app.choose_folder();
        }
        ui.horizontal_wrapped(|ui| {
            for format in [FileSystem::LittleFs, FileSystem::FatFs, FileSystem::Spiffs] {
                if ui
                    .add(Button::new(format.name()).selected(app.project.data.format == format))
                    .clicked()
                {
                    app.project.data.format = format;
                }
            }
        });
        ui.horizontal_wrapped(|ui| {
            field(
                ui,
                "partition_size",
                &mut app.project.data.size,
                model::DEFAULT_DATA_SIZE,
            );
            field(
                ui,
                "address",
                &mut app.project.data.address,
                model::DEFAULT_DATA_ADDRESS,
            );
        });
        hint(ui, t("data_address_hint"));
        ui.horizontal_wrapped(|ui| {
            csv_buttons(app, ui);
            let config = &mut app.project.data;
            ComboBox::from_id_salt("data_partition")
                .selected_text(t("use_partition"))
                .show_ui(ui, |ui| {
                    let mut found = false;
                    for row in &app.project.segments {
                        if model::data_partition(config.format, &row.kind) {
                            found = true;
                            if ui
                                .selectable_label(
                                    config.address == row.address,
                                    format!("{} · {} · {}", row.name, row.address, row.size),
                                )
                                .clicked()
                            {
                                config.address.clone_from(&row.address);
                                config.size.clone_from(&row.size);
                            }
                        }
                    }
                    if !found {
                        hint(ui, t("no_partition_match"));
                    }
                });
        });
        if app.preferences.advanced {
            details(ui, "csv_settings", app.preferences.advanced, |ui| {
                csv_settings(app, ui)
            });
        }
    });
    if !app.project.data.folder.is_empty() {
        storage(app, ui);
    }
    card(ui, |ui| {
        details(ui, "fs_settings", app.preferences.advanced, |ui| {
            ui.checkbox(&mut app.project.data.ignore_system, t("ignore_system"));
            fs_settings(ui, &mut app.project.data);
        });
    });
}

fn fs_settings(ui: &mut Ui, config: &mut model::DataConfig) {
    match config.format {
        FileSystem::LittleFs => {
            ui.horizontal_wrapped(|ui| {
                ui.selectable_value(&mut config.littlefs_legacy, true, "LittleFS 2.9.3");
                ui.selectable_value(&mut config.littlefs_legacy, false, "LittleFS 2.11.1");
            });
            hint(ui, t("littlefs_hint"));
            ui.horizontal_wrapped(|ui| {
                field(ui, "block_size", &mut config.block, "4096");
                field(ui, "page_size", &mut config.littlefs_page, "256");
            });
            ui.collapsing(t("custom_littlefs"), |ui| {
                path_field(
                    ui,
                    &mut config.custom_littlefs,
                    "built_in",
                    "browse_file",
                    |path| {
                        let Some(chosen) = rfd::FileDialog::new().pick_file() else {
                            return false;
                        };
                        *path = chosen.to_string_lossy().into_owned();
                        true
                    },
                );
            });
        }
        FileSystem::FatFs => {
            combo(
                ui,
                "layout",
                &mut config.fat_layout,
                &["wl", "safe", "raw"],
                |value| match value {
                    "wl" => t("fat_wl"),
                    "safe" => t("fat_safe"),
                    _ => t("fat_raw"),
                },
            );
            if config.fat_layout == "wl" {
                options(ui, "sector_size", &mut config.fat_sector, &["4096", "512"]);
            }
            options(
                ui,
                "cluster_sectors",
                &mut config.fat_clusters,
                &["1", "2", "4", "8", "16", "32", "64", "128"],
            );
            options(ui, "fat_tables", &mut config.fat_count, &["2", "1"]);
            ui.checkbox(&mut config.fat_lfn, t("fat_lfn"));
        }
        FileSystem::Spiffs => {
            ui.horizontal_wrapped(|ui| {
                field(ui, "block_size", &mut config.block, "4096");
                field(ui, "page_size", &mut config.page, "256");
                field(ui, "path_limit", &mut config.spiffs_name, "32");
                field(ui, "metadata", &mut config.spiffs_meta, "4");
            });
            ui.horizontal_wrapped(|ui| {
                ui.checkbox(&mut config.spiffs_magic, "Magic");
                ui.add_enabled_ui(config.spiffs_magic, |ui| {
                    ui.checkbox(&mut config.spiffs_magic_len, "Magic Length")
                });
                ui.checkbox(&mut config.spiffs_aligned, t("align_index"));
            });
            hint(ui, t("spiffs_hint"));
        }
    }
}

fn storage(app: &mut App, ui: &mut Ui) {
    let p = palette(ui.ctx());
    card(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            heading(ui, "folder_preview");
            if ui.add(Icon::Refresh.button(t("rescan"))).clicked() {
                app.rescan();
            }
        });
        match &app.preview {
            None => hint(ui, t("scanning")),
            Some(Err(error)) => {
                ui.colored_label(p.error, error);
            }
            Some(Ok(preview)) => {
                let bytes: u64 = preview.sizes.iter().sum();
                let limit = model::number(&app.project.data.size)
                    .ok()
                    .filter(|&n| n > 0);
                let free = limit.map(|n| model::human_size(n.saturating_sub(bytes)));
                let size = model::human_size(bytes);
                hint(
                    ui,
                    &message(
                        "file_count",
                        &[&preview.file_count, &size, &preview.skipped],
                    ),
                );
                if let Some(limit) = limit {
                    let (bar, _) = ui.allocate_exact_size(
                        Vec2::new(ui.available_width(), 12.0),
                        egui::Sense::hover(),
                    );
                    ui.painter().rect_filled(bar, 5, p.border);
                    let mut x = bar.left();
                    for (index, size) in preview.sizes.iter().enumerate() {
                        let width = bar.width() * (*size as f64 / limit.max(bytes) as f64) as f32;
                        let segment = egui::Rect::from_min_size(
                            egui::pos2(x, bar.top()),
                            Vec2::new(width, bar.height()),
                        );
                        ui.painter().with_clip_rect(bar).rect_filled(
                            segment,
                            0,
                            p.categories[index],
                        );
                        x += width;
                    }
                }
                ui.horizontal_wrapped(|ui| {
                    for (index, (category, color)) in
                        ["web", "media", "config_files", "other_files"]
                            .into_iter()
                            .zip(p.categories)
                            .enumerate()
                    {
                        if preview.sizes[index] != 0 {
                            ui.label(
                                RichText::new(format!(
                                    "● {} {}",
                                    t(category),
                                    model::human_size(preview.sizes[index])
                                ))
                                .small()
                                .color(color),
                            );
                        }
                    }
                    if let Some(free) = free {
                        hint(ui, &message("remaining", &[&free]));
                    }
                });
                if limit.is_some_and(|n| bytes > n) {
                    ui.colored_label(p.error, t("over_capacity"));
                } else if app.project.data.format == FileSystem::Spiffs
                    && limit.is_some_and(|n| bytes > n / 4 * 3)
                {
                    ui.colored_label(p.warning, t("spiffs_space"));
                }
                if !preview.entries.is_empty() {
                    tree::show(
                        ui,
                        &preview.entries,
                        app.project.data.format == FileSystem::FatFs && !app.project.data.fat_lfn,
                        &app.project.data.folder,
                    );
                }
                hint(ui, t("estimate"));
            }
        }
    });
}

fn actions(app: &mut App, ctx: &Context, ui: &mut Ui) {
    let p = palette(ctx);
    ui.add_enabled_ui(app.job.is_none(), |ui| {
        card(ui, |ui| {
            heading(ui, "device");
            if app.preferences.guide && app.project.mode != Mode::Firmware {
                hint(ui, t("usb_hint"));
            }
            ui.horizontal(|ui| {
                ui.label(t("serial_port"));
                if ui.add(Icon::Refresh.button(t("refresh"))).clicked() {
                    app.refresh_ports();
                }
            });
            let config = &mut app.project.connection;
            let port_text = if config.auto_port {
                t("auto_port")
            } else if config.port.is_empty() {
                t("select_port")
            } else {
                &config.port
            };
            ComboBox::from_id_salt("ports")
                .selected_text(port_text)
                .truncate()
                .width((ui.available_width() - 19.0).max(100.0))
                .show_ui(ui, |ui| {
                    for (port, description) in &app.ports {
                        if ui
                            .selectable_label(!config.auto_port && config.port == *port, port)
                            .on_hover_text(description)
                            .clicked()
                        {
                            config.port.clone_from(port);
                            config.auto_port = false;
                        }
                    }
                    if app.ports.is_empty() {
                        hint(ui, t("no_ports"));
                    }
                });
            if !app.port_error.is_empty() {
                ui.colored_label(p.error, &app.port_error);
            }
            combo(ui, "chip", &mut config.chip, CHIPS, |value| {
                if value == "auto" {
                    t("auto_chip")
                } else {
                    value
                }
            });
            ui.add_space(4.0);
            let ready = source_selected(app, true)
                && (app.project.connection.auto_port
                    || !app.project.connection.port.trim().is_empty());
            if ui
                .add_enabled(
                    app.tools_available && ready,
                    primary(ui, Icon::Flash, t("flash_button"), ui.available_width()),
                )
                .clicked()
            {
                app.request_flash(false);
            }
            if !ready {
                hint(
                    ui,
                    t(if source_selected(app, true) {
                        "select_port_hint"
                    } else {
                        "prepare_files"
                    }),
                );
            }
            if app.preferences.advanced
                && ui
                    .add(Icon::Terminal.button(t("preview_command")))
                    .clicked()
            {
                app.request_flash(true);
            }
        });
        if app.error {
            Frame::new()
                .fill(p.error_bg)
                .corner_radius(8)
                .inner_margin(Margin::same(9))
                .show(ui, |ui| {
                    ui.colored_label(p.error, &app.activity);
                });
        }
        if app.project.mode != Mode::Firmware {
            export_card(app, ctx, ui);
        }
        card(ui, |ui| {
            details(ui, "flash_options", app.preferences.advanced, |ui| {
                connection_options(app, ctx, ui)
            });
        });
    });
}

fn source_selected(app: &App, flash: bool) -> bool {
    match app.project.mode {
        Mode::Firmware => !app.project.firmware.path.trim().is_empty(),
        Mode::Partitions => app
            .project
            .segments
            .iter()
            .any(|row| row.enabled && !row.path.trim().is_empty()),
        Mode::Data => {
            let data = &app.project.data;
            !data.folder.trim().is_empty()
                && !data.size.trim().is_empty()
                && (!flash || !data.address.trim().is_empty())
        }
    }
}

fn export_card(app: &mut App, ctx: &Context, ui: &mut Ui) {
    card(ui, |ui| {
        let merge = app.project.mode == Mode::Partitions;
        heading(ui, if merge { "merge" } else { "build_image" });
        hint(ui, t("offline_hint"));
        if merge {
            combo(
                ui,
                "file_format",
                &mut app.project.merge_format,
                &["raw", "hex", "uf2"],
                |format| match format {
                    "raw" => "BIN",
                    "hex" => "HEX",
                    _ => "UF2",
                },
            );
        }
        let output = if merge {
            &app.project.merge_path
        } else {
            &app.project.image_path
        };
        if !output.is_empty() {
            ui.add(egui::Label::new(output).truncate())
                .on_hover_text(output);
            if ui.add(Icon::Folder.button(t("change_output"))).clicked() {
                app.choose_output(merge);
            }
        }
        let chip_selected = !merge || app.project.connection.chip != "auto";
        let enabled = source_selected(app, false) && app.tools_available && chip_selected;
        if !chip_selected {
            hint(ui, t("merge_chip"));
        }
        let mut export = Icon::Save.button(t("export_button"));
        export.button = export
            .button
            .min_size(Vec2::new(ui.available_width(), CONTROL_HEIGHT));
        if ui.add_enabled(enabled, export).clicked() {
            app.request_output(ctx, merge);
        }
    });
}

fn connection_options(app: &mut App, ctx: &Context, ui: &mut Ui) {
    let p = palette(ctx);
    let config = &mut app.project.connection;
    ui.checkbox(&mut config.auto_port, t("auto_port"));
    ui.add_enabled_ui(!config.auto_port, |ui| {
        field(
            ui,
            "manual_port",
            &mut config.port,
            if cfg!(windows) { "COM3" } else { "/dev/…" },
        )
    });
    baud_picker(ui, &mut config.baud);
    ui.add_enabled_ui(app.project.mode != Mode::Data, |ui| {
        options(
            ui,
            "flash_mode",
            &mut config.flash_mode,
            &["keep", "qio", "qout", "dio", "dout"],
        );
        options(ui, "flash_freq", &mut config.flash_freq, FLASH_FREQS);
    });
    options(ui, "flash_size", &mut config.flash_size, FLASH_SIZES);
    options(
        ui,
        "before_reset",
        &mut config.before,
        &["default-reset", "usb-reset", "no-reset", "no-reset-no-sync"],
    );
    options(
        ui,
        "after_reset",
        &mut config.after,
        &[
            "hard-reset",
            "soft-reset",
            "no-reset",
            "no-reset-stub",
            "watchdog-reset",
        ],
    );
    options(
        ui,
        "compression",
        &mut config.compression,
        &["default", "compress", "no-compress"],
    );
    ui.checkbox(&mut config.no_stub, t("rom_only"));
    ui.checkbox(&mut config.trace, t("trace"));
    ui.checkbox(&mut config.encrypt, t("encrypt_all"));
    ui.checkbox(
        &mut config.erase_all,
        RichText::new(t("erase_all")).color(p.warning),
    );
    if config.erase_all && app.project.mode == Mode::Data {
        ui.colored_label(p.error, t("data_erase_block"));
    }
    let mut requested_help = None;
    details(ui, "extra_options", app.preferences.advanced, |ui| {
        hint(ui, t("extra_hint"));
        for (label, command, field, flags) in [
            ("global_args", "", &mut config.globals, GLOBAL_OPTIONS),
            (
                "write_args",
                "write-flash",
                &mut config.write_options,
                WRITE_OPTIONS,
            ),
            (
                "merge_args",
                "merge-bin",
                &mut config.merge_options,
                MERGE_OPTIONS,
            ),
        ] {
            ui.horizontal_wrapped(|ui| {
                ui.label(t(label));
                if ui.button(t("tool_help")).clicked() {
                    requested_help = Some((label, command));
                }
            });
            ComboBox::from_id_salt(label)
                .selected_text(t("add_option"))
                .show_ui(ui, |ui| {
                    for (flag, value) in flags {
                        if ui.selectable_label(false, *flag).clicked() {
                            if !field.is_empty() && !field.ends_with('\n') {
                                field.push('\n');
                            }
                            field.push_str(flag);
                            field.push('\n');
                            if !value.is_empty() {
                                field.push_str(value);
                                field.push('\n');
                            }
                        }
                    }
                });
            ui.add(
                TextEdit::multiline(field)
                    .font(TextStyle::Monospace)
                    .desired_rows(2)
                    .desired_width(ui.available_width()),
            );
        }
    });
    if let Some((label, command)) = requested_help {
        app.show_help(ctx, t(label), command);
    }
}

const GLOBAL_OPTIONS: &[(&str, &str)] = &[
    ("--port-filter", "vid=0x303a"),
    ("--verbose", ""),
    ("--silent", ""),
    ("--override-vddsdio", "1.8V"),
    ("--connect-attempts", "7"),
];
const WRITE_OPTIONS: &[(&str, &str)] = &[
    ("--spi-connection", "SPI"),
    ("--no-progress", ""),
    ("--ignore-flash-enc-efuse", ""),
    ("--diff-with", "skip"),
    ("--trust-flash-content", ""),
    ("--skip-flashed", ""),
    ("--force", ""),
];
const MERGE_OPTIONS: &[(&str, &str)] = &[
    ("--target-offset", "0x0"),
    ("--pad-to-size", "4MB"),
    ("--chunk-size", "256"),
    ("--md5-disable", ""),
];

fn status_panel(app: &mut App, root: &mut Ui, radius: u8) {
    let p = palette(root.ctx());
    let status = egui::TopBottomPanel::bottom("status")
        .show_separator_line(false)
        .frame(
            Frame::new()
                .fill(p.panel)
                .corner_radius(CornerRadius {
                    sw: radius,
                    se: radius,
                    nw: 0,
                    ne: 0,
                })
                .inner_margin(Margin::symmetric(16, 7)),
        )
        .show_inside(root, |ui| {
            ui.horizontal(|ui| {
                let log_label = if app.log_open { "hide_log" } else { "show_log" };
                if ui.add(Icon::Terminal.button(t(log_label))).clicked() {
                    app.log_open = !app.log_open;
                }
                let mut status = app.activity.as_str();
                let mut detail = status;
                let mut error = app.error;
                if let Some(monitor) = &mut app.flash_monitor {
                    if !monitor.visible && ui.add(Icon::Up.button(t("flash_monitor"))).clicked() {
                        monitor.visible = true;
                    }
                    if let Some(result) = &monitor.outcome {
                        (status, error) = progress::outcome_status(result);
                        detail = status;
                    }
                    let width = (ui.available_width() * 0.30).clamp(100.0, 200.0);
                    progress::bar(
                        ui,
                        progress::percent(app.job.as_ref(), monitor),
                        width,
                        app.job.is_some(),
                    );
                } else if let Some(job) = &app.job {
                    ui.spinner().on_hover_text(job.title);
                    progress::stop_button(ui, job);
                }
                if !app.autosave_error.is_empty() {
                    status = t("autosave_error");
                    detail = &app.autosave_error;
                    error = true;
                }
                let color = if error { p.error } else { p.muted };
                ui.add(egui::Label::new(RichText::new(status).small().color(color)).truncate())
                    .on_hover_text(detail);
            });
        });
    root.painter().hline(
        status.response.rect.x_range(),
        status.response.rect.top() + 0.5,
        Stroke::new(1.0, p.border),
    );

    if app.log_open {
        let min_height = 130.0;
        let max_height = (root.max_rect().height() * 0.56)
            .min(root.available_height() - 96.0)
            .max(min_height);
        let bottom = root.available_rect_before_wrap().bottom();
        egui::TopBottomPanel::bottom("workbench_log_panel")
            .exact_height(app.log_height.clamp(min_height, max_height))
            .frame(Frame::new().fill(p.panel).inner_margin(Margin {
                left: 16,
                right: 16,
                top: 0,
                bottom: 8,
            }))
            .show_inside(root, |ui| {
                let (handle, drag) = ui.allocate_exact_size(
                    Vec2::new(ui.available_width(), 12.0),
                    egui::Sense::drag(),
                );
                let drag = drag.on_hover_cursor(egui::CursorIcon::ResizeVertical);
                if (drag.dragged() || drag.drag_stopped())
                    && let Some(pointer) = drag.interact_pointer_pos()
                {
                    app.log_height =
                        (bottom - pointer.y + handle.height() * 0.5).clamp(min_height, max_height);
                    ui.ctx().request_repaint();
                }
                ui.painter().rect_filled(
                    Rect::from_center_size(handle.center(), Vec2::new(36.0, 3.0)),
                    2,
                    if drag.hovered() || drag.dragged() {
                        p.accent
                    } else {
                        p.muted
                    },
                );
                ui.horizontal_wrapped(|ui| {
                    if ui.add(Icon::Copy.button(t("copy_log"))).clicked() {
                        ui.ctx().copy_text(app.log_text());
                    }
                    // The progress monitor uses the same log buffer.
                    if ui
                        .add_enabled(app.job.is_none(), Icon::Trash.button(t("clear_log")))
                        .clicked()
                    {
                        app.log.clear();
                    }
                    hint(ui, t("resize_log"));
                });
                progress::log_view(ui, &app.log, 0, ui.available_height(), "workbench_log");
            });
    }
}

fn dialogs(app: &mut App, ctx: &Context) {
    let p = palette(ctx);
    if let Some(plan) = &app.confirm {
        let (summary, command) = plan.review();
        let result = style::modal(ctx, "confirm_flash").show(ctx, |ui| {
            ui.set_width(442.0);
            ui.heading(t("confirm_title"));
            ui.add_space(4.0);
            ui.colored_label(p.warning, t("overwrite_warning"));
            ScrollArea::vertical().max_height(210.0).show(ui, |ui| {
                ui.label(summary);
            });
            ui.collapsing(t("preview_command"), |ui| {
                ui.code(&command);
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                (
                    ui.button(t("cancel")).clicked(),
                    ui.add(primary(ui, Icon::Flash, t("confirm_flash"), 110.0))
                        .clicked(),
                )
            })
            .inner
        });
        if result.inner.1
            && let Some(plan) = app.confirm.take()
        {
            app.start(ctx, plan);
        } else if result.inner.0 || result.should_close() {
            app.confirm = None;
        }
    }
    if let Some(command) = &app.command_preview {
        let mut open = true;
        egui::Window::new(t("preview_command"))
            .open(&mut open)
            .default_width(520.0)
            .show(ctx, |ui| {
                ScrollArea::vertical().max_height(310.0).show(ui, |ui| {
                    ui.code(command);
                });
                if ui.add(Icon::Copy.button(t("copy_command"))).clicked() {
                    ctx.copy_text(command.clone());
                }
            });
        if !open {
            app.command_preview = None;
        }
    }
    if let Some((title, _)) = &app.help {
        let mut open = true;
        egui::Window::new(format!("esptool · {title}"))
            .open(&mut open)
            .default_width(710.0)
            .show(ctx, |ui| {
                ScrollArea::both().max_height(450.0).show(ui, |ui| {
                    ui.add(egui::Label::new(RichText::new(&app.help_text).monospace()).extend());
                });
            });
        if !open {
            app.help = None;
        }
    }
}

fn card(ui: &mut Ui, show: impl FnOnce(&mut Ui)) {
    let p = palette(ui.ctx());
    Frame::new()
        .fill(p.panel)
        .stroke(Stroke::new(1.0, p.border))
        .corner_radius(11)
        .inner_margin(Margin::same(13))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            show(ui);
        });
    ui.add_space(2.0);
}

fn heading(ui: &mut Ui, key: &'static str) {
    ui.label(RichText::new(t(key)).size(16.0).strong());
}

fn hint(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).small().color(palette(ui.ctx()).muted));
}

fn details(ui: &mut Ui, key: &'static str, expanded: bool, show: impl FnOnce(&mut Ui)) {
    if expanded {
        heading(ui, key);
        show(ui);
    } else {
        ui.collapsing(t(key), show);
    }
}

fn field(ui: &mut Ui, key: &'static str, value: &mut String, placeholder: &str) {
    ui.horizontal_wrapped(|ui| {
        ui.label(t(key));
        inline(ui, value, 92.0, placeholder);
    });
}

fn inline(ui: &mut Ui, value: &mut String, width: f32, hint: &str) -> egui::Response {
    ui.add(
        input(ui, value, hint)
            .desired_width((width - 18.0).max(24.0))
            .font(TextStyle::Monospace),
    )
}

fn options(ui: &mut Ui, key: &'static str, value: &mut String, choices: &[&str]) {
    combo(ui, key, value, choices, |value| value);
}

fn combo(
    ui: &mut Ui,
    key: &'static str,
    value: &mut String,
    choices: &[&str],
    label: impl for<'a> Fn(&'a str) -> &'a str,
) {
    ui.horizontal_wrapped(|ui| {
        ui.label(t(key));
        ComboBox::from_id_salt(key)
            .selected_text(label(value))
            .show_ui(ui, |ui| {
                for choice in choices {
                    ui.selectable_value(value, (*choice).into(), label(choice));
                }
            });
    });
}

fn path_field(
    ui: &mut Ui,
    value: &mut String,
    placeholder: &'static str,
    browse_label: &'static str,
    browse: impl FnOnce(&mut String) -> bool,
) {
    ui.horizontal(|ui| {
        let button = ui.add(Icon::Folder.button(t(browse_label)));
        if button.clicked() {
            browse(value);
        }
        ui.add(input(ui, value, t(placeholder)).desired_width(ui.available_width()))
            .on_hover_text(value.as_str());
    });
}

fn file_size(path: &str) -> Option<u64> {
    std::fs::metadata(path)
        .ok()
        .filter(std::fs::Metadata::is_file)
        .map(|meta| meta.len())
}

fn baud_picker(ui: &mut Ui, baud: &mut String) {
    let p = palette(ui.ctx());
    ui.horizontal_wrapped(|ui| {
        let label = ui.label(t("baud"));
        let width = 131.0;
        let stroke_index = ui.painter().add(egui::Shape::Noop);
        let row = ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            let field = ui
                .add(
                    input(ui, baud, "115200")
                        .desired_width(width - CONTROL_HEIGHT - 18.0)
                        .font(TextStyle::Monospace)
                        .frame(false),
                )
                .labelled_by(label.id);
            let (rect, trigger) =
                ui.allocate_exact_size(Vec2::splat(CONTROL_HEIGHT), egui::Sense::click());
            let trigger = trigger.on_hover_text(t("baud_presets"));
            trigger.widget_info(|| {
                egui::WidgetInfo::labeled(
                    egui::WidgetType::ComboBox,
                    ui.is_enabled(),
                    t("baud_presets"),
                )
            });
            let c = rect.center();
            ui.painter().line(
                vec![
                    c + Vec2::new(-4.0, -2.0),
                    c + Vec2::new(0.0, 2.0),
                    c + Vec2::new(4.0, -2.0),
                ],
                Stroke::new(1.4, if trigger.hovered() { p.accent } else { p.muted }),
            );
            (field, trigger)
        });
        let (field, trigger) = row.inner;
        let open_from_field =
            field.has_focus() && ui.input(|input| input.key_pressed(egui::Key::ArrowDown));
        let popup = egui::Popup::menu(&trigger)
            .anchor(row.response.rect)
            .width(width)
            .gap(4.0);
        let popup = if open_from_field {
            popup.open_memory(Some(egui::SetOpenCommand::Bool(true)))
        } else {
            popup
        };
        popup.show(|ui| {
            for rate in RATES {
                if ui.selectable_label(baud == rate, *rate).clicked() {
                    *baud = (*rate).into();
                    ui.close();
                }
            }
        });
        let border = if field.has_focus() || field.hovered() || trigger.hovered() {
            p.accent
        } else {
            p.border
        };
        ui.painter().set(
            stroke_index,
            egui::epaint::RectShape::new(
                row.response.rect,
                CONTROL_RADIUS,
                p.field,
                Stroke::new(1.0, border),
                egui::StrokeKind::Inside,
            ),
        );
    });
}
