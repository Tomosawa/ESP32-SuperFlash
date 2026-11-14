use super::{
    icons::Icon,
    style::{self, CONTROL_HEIGHT, palette},
};
use crate::{
    app::{App, Appearance},
    i18n::{Language, t},
};
use eframe::egui::{
    self, Align, Button, Color32, ComboBox, Context, Frame, Layout, Margin, Rect, RichText, Sense,
    Stroke, StrokeKind, Ui, Vec2, ViewportCommand,
};

const GITHUB: &str = "https://github.com/tomosawa/esp32-superflash";

// Component, license, copyright holder(s), and upstream source.
const THIRD_PARTY: &[(&str, &str, &str, &str)] = &[
    (
        "esptool 5.6.0",
        "GPL-2.0-or-later",
        "Fredrik Ahlberg, Angus Gratton, Espressif Systems (Shanghai) CO LTD and contributors",
        "https://github.com/espressif/esptool/tree/v5.6.0",
    ),
    (
        "mklittlefs 4.1.0 / 4.0.2",
        "MIT",
        "Ivan Grokhotkov",
        "https://github.com/earlephilhower/mklittlefs/releases",
    ),
    (
        "LittleFS 2.11.1 / 2.9.3",
        "BSD-3-Clause",
        "The littlefs authors and Arm Limited",
        "https://github.com/littlefs-project/littlefs/tags",
    ),
    (
        "mkspiffs 0.2.3",
        "MIT",
        "Ivan Grokhotkov",
        "https://github.com/igrr/mkspiffs/releases/tag/0.2.3",
    ),
    (
        "SPIFFS",
        "MIT",
        "Peter Andersson",
        "https://github.com/pellepl/spiffs/tree/f5e26c4",
    ),
    (
        "ESP-IDF 6.1 · FatFS / SPIFFS",
        "Apache-2.0",
        "Espressif Systems (Shanghai) CO LTD",
        "https://github.com/espressif/esp-idf/tree/v6.1/components",
    ),
    (
        "Construct 2.10.70",
        "MIT",
        "Arkadiusz Bulski, Tomer Filiba and Corbin Simpson",
        "https://pypi.org/project/construct/2.10.70/#files",
    ),
    (
        "CPython 3.12",
        "PSF-2.0 + historical terms",
        "Python Software Foundation and contributors",
        "https://github.com/python/cpython/tree/3.12",
    ),
    (
        "Python Build Standalone",
        "Runtime notices",
        "Python Software Foundation and contributors",
        "https://github.com/astral-sh/python-build-standalone/releases/tag/20261026",
    ),
    (
        "MinGW-w64 winpthreads (Windows)",
        "Upstream COPYING",
        "MinGW-w64 project and contributors",
        "https://github.com/mingw-w64/mingw-w64/blob/v12.0.0/mingw-w64-libraries/winpthreads/COPYING",
    ),
    (
        "Noto Sans CJK",
        "OFL-1.1",
        "Adobe",
        "https://github.com/notofonts/noto-cjk",
    ),
    (
        "Material Design Icons · GitHub",
        "Apache-2.0",
        "Pictogrammers",
        "https://pictogrammers.com/library/mdi/icon/github/",
    ),
];

pub fn install_github_icon(ctx: &Context) -> egui::TextureHandle {
    let data = eframe::icon_data::from_png_bytes(include_bytes!("../../assets/github.png"))
        .expect("GitHub icon");
    ctx.load_texture(
        "mdi-github",
        egui::ColorImage::from_rgba_unmultiplied(
            [data.width as usize, data.height as usize],
            &data.rgba,
        ),
        egui::TextureOptions::LINEAR,
    )
}

pub fn title_bar(app: &mut App, root: &mut Ui) {
    let ctx = &root.ctx().clone();
    egui::TopBottomPanel::top("title_bar")
        .show_separator_line(false)
        .frame(Frame::new().inner_margin(Margin::symmetric(14, 8)))
        .show_inside(root, |ui| {
            ui.horizontal(|ui| {
                if cfg!(target_os = "macos") {
                    window_controls(ui, ctx);
                    ui.add_space(9.0);
                }
                let brand = ui
                    .horizontal(|ui| {
                        brand_mark(ui);
                        ui.label(RichText::new("ESP32 SuperFlash").size(17.0).strong());
                    })
                    .response
                    .rect;
                ui.separator();

                let menu_style = |style: &mut egui::Style| {
                    egui::containers::menu::menu_style(style);
                    style.spacing.button_padding.x = 10.0;
                    style.spacing.item_spacing.x = 4.0;
                };
                egui::MenuBar::new().style(menu_style).ui(ui, |ui| {
                    menus(app, ui);
                    let menu_end = ui.cursor().left();
                    let right = ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if !cfg!(target_os = "macos") {
                            window_controls(ui, ctx);
                            ui.add_space(9.0);
                        }
                        github_button(ui, app);
                        ui.add_enabled_ui(app.job.is_none(), |ui| {
                            let mut language = app.preferences.language();
                            let response = ComboBox::from_id_salt("language")
                                .selected_text(language.name())
                                .width(98.0)
                                .height(400.0)
                                .show_ui(ui, |ui| {
                                    for choice in Language::ALL {
                                        if ui
                                            .selectable_value(&mut language, choice, choice.name())
                                            .clicked()
                                        {
                                            ui.close();
                                        }
                                    }
                                })
                                .response
                                .on_hover_text(t("language"));
                            app.change_language(ctx, language);
                            response.rect.left()
                        })
                        .inner
                    });
                    let spacer = Rect::from_x_y_ranges(
                        menu_end..=right.inner,
                        right.response.rect.y_range(),
                    );
                    for (id, rect) in [("brand_drag", brand), ("spacer_drag", spacer)] {
                        if !rect.is_positive() {
                            continue;
                        }
                        let handle = ui.interact(rect, ui.id().with(id), Sense::click_and_drag());
                        if handle.double_clicked() {
                            maximize(ctx);
                        } else if handle.drag_started_by(egui::PointerButton::Primary) {
                            ctx.send_viewport_cmd(ViewportCommand::StartDrag);
                        }
                    }
                });
            });
        });
}

fn menus(app: &mut App, ui: &mut Ui) {
    ui.add_enabled_ui(app.job.is_none(), |ui| {
        ui.menu_button(t("project"), |ui| {
            ui.set_max_width(280.0);
            if ui.add(Icon::Folder.button(t("open_project"))).clicked() {
                ui.close();
                app.import_project();
            }
            if ui.add(Icon::Save.button(t("save_project"))).clicked() {
                ui.close();
                app.export_project();
            }
            ui.separator();
            super::hint(ui, t("workspace_autosave"));
        });
    });

    ui.menu_button(t("view_menu"), |ui| {
        if ui.checkbox(&mut app.log_open, t("show_log")).changed() {
            ui.close();
        }
        if let Some(monitor) = &mut app.flash_monitor
            && ui.add(Icon::Up.button(t("flash_monitor"))).clicked()
        {
            monitor.visible = true;
            ui.close();
        }
    });

    ui.add_enabled_ui(app.job.is_none(), |ui| {
        ui.menu_button(t("options_menu"), |ui| {
            if ui
                .checkbox(&mut app.preferences.advanced, t("advanced"))
                .changed()
            {
                app.save_preferences();
                ui.close();
            }
            ui.separator();
            let mut appearance = app.preferences.appearance;
            ui.menu_button(t("appearance"), |ui| {
                for choice in Appearance::ALL {
                    if ui
                        .selectable_value(&mut appearance, choice, t(choice.key()))
                        .clicked()
                    {
                        ui.close_kind(egui::UiKind::Menu);
                    }
                }
            });
            app.change_appearance(ui.ctx(), appearance);
        });
    });

    ui.menu_button(t("help_menu"), |ui| {
        if ui
            .add(Button::new(t("about")).wrap_mode(egui::TextWrapMode::Extend))
            .clicked()
        {
            app.about_open = true;
            ui.close();
        }
    });
}

fn brand_mark(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(27.0), Sense::hover());
    let p = palette(ui.ctx());
    ui.painter().rect_filled(rect, 7, p.action);
    let points = [
        (15., 4.),
        (7., 15.),
        (13., 15.),
        (11., 23.),
        (21., 11.),
        (14., 11.),
    ];
    ui.painter().add(egui::Shape::convex_polygon(
        points.map(|(x, y)| rect.min + Vec2::new(x, y)).to_vec(),
        Color32::WHITE,
        Stroke::NONE,
    ));
}

fn github_button(ui: &mut Ui, app: &App) -> egui::Response {
    let image = egui::Image::from_texture(egui::load::SizedTexture::from_handle(&app.github_icon))
        .fit_to_exact_size(Vec2::splat(20.0))
        .tint(palette(ui.ctx()).text);
    let github = ui
        .add(
            Button::image(image)
                .frame(false)
                .min_size(Vec2::splat(CONTROL_HEIGHT)),
        )
        .on_hover_text("GitHub");
    github.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "GitHub"));
    if github.clicked() {
        ui.ctx().open_url(egui::OpenUrl::new_tab(GITHUB));
    }
    github
}

pub fn about(app: &mut App, ctx: &Context) {
    if !app.about_open {
        return;
    }
    let dialog = style::modal(ctx, "about").show(ctx, |ui| {
        ui.set_width(520.0);
        ui.heading(t("about"));
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            brand_mark(ui);
            ui.vertical(|ui| {
                ui.label(RichText::new("ESP32 SuperFlash").size(17.0).strong());
                super::hint(ui, concat!("v", env!("CARGO_PKG_VERSION")));
            });
        });
        ui.label(t("about_description"));
        ui.add_space(6.0);
        ui.separator();
        ui.label(RichText::new(t("third_party_notices")).strong());
        egui::ScrollArea::vertical()
            .max_height((ctx.content_rect().height() * 0.39).min(265.0))
            .show(ui, |ui| {
                ui.spacing_mut().interact_size.y = 20.0;
                ui.spacing_mut().item_spacing.y = 3.0;
                for (name, license, copyright, source) in THIRD_PARTY {
                    ui.horizontal_wrapped(|ui| {
                        ui.hyperlink_to(*name, *source);
                        super::hint(ui, license);
                    });
                    super::hint(ui, &format!("© {copyright}"));
                    ui.add_space(5.0);
                }
            });
        ui.separator();
        super::hint(ui, t("third_party_notice_files"));
        ui.horizontal(|ui| {
            github_button(ui, app);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.button(t("close")).clicked()
            })
            .inner
        })
        .inner
    });
    if dialog.inner || dialog.should_close() {
        app.about_open = false;
    }
}

fn maximize(ctx: &Context) {
    let maximized = ctx.input(|input| input.viewport().maximized.unwrap_or(false));
    ctx.send_viewport_cmd(ViewportCommand::Maximized(!maximized));
}

fn window_controls(ui: &mut Ui, ctx: &Context) {
    let p = palette(ctx);
    let controls = if cfg!(target_os = "macos") {
        ["close", "minimize", "maximize"]
    } else {
        ["close", "maximize", "minimize"]
    };
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        for control in controls {
            let (rect, response) =
                ui.allocate_exact_size(Vec2::new(25.0, CONTROL_HEIGHT), Sense::click());
            let maximized = ctx.input(|input| input.viewport().maximized.unwrap_or(false));
            let label = t(if control == "maximize" && maximized {
                "restore_window"
            } else {
                control
            });
            let response = response.on_hover_text(label);
            response
                .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
            let center = rect.center();
            let paint = ui.painter();
            let mut color = p.text;
            if cfg!(target_os = "macos") {
                let fill = match control {
                    "close" => Color32::from_rgb(243, 99, 94),
                    "minimize" => Color32::from_rgb(242, 184, 66),
                    _ => Color32::from_rgb(70, 192, 101),
                };
                paint.circle_filled(center, 6.5, fill);
                if !response.hovered() && !response.has_focus() {
                    continue;
                }
                color = Color32::from_rgb(49, 36, 31);
            } else if response.hovered() || response.has_focus() {
                paint.rect_filled(
                    rect.shrink(2.0),
                    6,
                    if control == "close" {
                        p.error_bg
                    } else {
                        p.tint
                    },
                );
            }
            let stroke = Stroke::new(1.3, color);
            let size = if cfg!(target_os = "macos") { 3.0 } else { 4.0 };
            match control {
                "close" => {
                    paint.line_segment(
                        [
                            center + Vec2::new(-size, -size),
                            center + Vec2::new(size, size),
                        ],
                        stroke,
                    );
                    paint.line_segment(
                        [
                            center + Vec2::new(-size, size),
                            center + Vec2::new(size, -size),
                        ],
                        stroke,
                    );
                }
                "minimize" => {
                    paint.line_segment(
                        [center - Vec2::new(size, 0.0), center + Vec2::new(size, 0.0)],
                        stroke,
                    );
                }
                _ => {
                    let icon = Rect::from_center_size(center, Vec2::splat(size * 2.0));
                    paint.rect_stroke(icon, 1, stroke, StrokeKind::Inside);
                    if maximized {
                        paint.line_segment(
                            [
                                icon.left_top() + Vec2::new(2.0, -2.0),
                                icon.right_top() + Vec2::new(2.0, -2.0),
                            ],
                            stroke,
                        );
                    }
                }
            }
            if response.clicked() {
                match control {
                    "close" => ctx.send_viewport_cmd(ViewportCommand::Close),
                    "minimize" => ctx.send_viewport_cmd(ViewportCommand::Minimized(true)),
                    _ => maximize(ctx),
                }
            }
        }
    });
}

#[cfg(target_os = "macos")]
pub fn resize_edges(_: &Context) {}

#[cfg(not(target_os = "macos"))]
pub fn resize_edges(ctx: &Context) {
    use egui::{CursorIcon, ResizeDirection};
    if ctx.input(|i| {
        i.viewport().maximized.unwrap_or(false) || i.viewport().fullscreen.unwrap_or(false)
    }) {
        return;
    }
    let r = ctx.content_rect();
    let margin = 5.0;
    let x0 = r.left();
    let x1 = x0 + margin;
    let x2 = r.right() - margin;
    let x3 = r.right();
    let y0 = r.top();
    let y1 = y0 + margin;
    let y2 = r.bottom() - margin;
    let y3 = r.bottom();
    for (index, (x0, y0, x1, y1, dir, cursor)) in [
        (
            x0,
            y0,
            x1,
            y1,
            ResizeDirection::NorthWest,
            CursorIcon::ResizeNwSe,
        ),
        (
            x1,
            y0,
            x2,
            y1,
            ResizeDirection::North,
            CursorIcon::ResizeVertical,
        ),
        (
            x2,
            y0,
            x3,
            y1,
            ResizeDirection::NorthEast,
            CursorIcon::ResizeNeSw,
        ),
        (
            x0,
            y1,
            x1,
            y2,
            ResizeDirection::West,
            CursorIcon::ResizeHorizontal,
        ),
        (
            x2,
            y1,
            x3,
            y2,
            ResizeDirection::East,
            CursorIcon::ResizeHorizontal,
        ),
        (
            x0,
            y2,
            x1,
            y3,
            ResizeDirection::SouthWest,
            CursorIcon::ResizeNeSw,
        ),
        (
            x1,
            y2,
            x2,
            y3,
            ResizeDirection::South,
            CursorIcon::ResizeVertical,
        ),
        (
            x2,
            y2,
            x3,
            y3,
            ResizeDirection::SouthEast,
            CursorIcon::ResizeNwSe,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let rect = Rect::from_min_max(egui::pos2(x0, y0), egui::pos2(x1, y1));
        egui::Area::new(egui::Id::new(("resize", index)))
            .order(egui::Order::Foreground)
            .fixed_pos(rect.min)
            .constrain(false)
            .show(ctx, |ui| {
                let (_, response) = ui.allocate_exact_size(rect.size(), Sense::drag());
                let response = response
                    .on_hover_cursor(cursor)
                    .on_hover_and_drag_cursor(cursor);
                if response.is_pointer_button_down_on()
                    && ui.input(|i| i.pointer.button_pressed(egui::PointerButton::Primary))
                {
                    ctx.send_viewport_cmd(ViewportCommand::BeginResize(dir));
                }
            });
    }
}
