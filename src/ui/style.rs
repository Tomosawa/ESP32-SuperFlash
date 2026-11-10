use eframe::egui::{
    self, Color32, Context, FontId, Frame, Margin, RichText, Stroke, TextStyle, Theme, Vec2,
};

use super::icons::{Icon, IconButton};

pub const CONTROL_HEIGHT: f32 = 34.0;
pub const CONTROL_RADIUS: u8 = 8;
pub const FIELD_PADDING: i8 = 9;

pub struct Palette {
    pub canvas: Color32,
    pub panel: Color32,
    pub field: Color32,
    pub border: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub placeholder: Color32,
    pub accent: Color32,
    pub action: Color32,
    pub tint: Color32,
    pub warning: Color32,
    pub error: Color32,
    pub error_bg: Color32,
    pub categories: [Color32; 4],
}

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

const LIGHT: Palette = Palette {
    canvas: rgb(0xF3F6F8),
    panel: rgb(0xFFFFFF),
    field: rgb(0xFFFFFF),
    border: rgb(0xD6E0E6),
    text: rgb(0x17232D),
    muted: rgb(0x516371),
    placeholder: rgb(0x65778A),
    accent: rgb(0x006854),
    action: rgb(0x076B58),
    tint: rgb(0xE5F3EF),
    warning: rgb(0x885316),
    error: rgb(0xA23039),
    error_bg: rgb(0xFDEEEF),
    categories: [rgb(0x187653), rgb(0x2B64A5), rgb(0x7654A0), rgb(0x935C1C)],
};

const DARK: Palette = Palette {
    canvas: rgb(0x10171E),
    panel: rgb(0x18222B),
    field: rgb(0x121B23),
    border: rgb(0x344451),
    text: rgb(0xE6EFF6),
    muted: rgb(0xA8BAC9),
    placeholder: rgb(0x8193A6),
    accent: rgb(0x73DFC0),
    action: rgb(0x18745E),
    tint: rgb(0x203C39),
    warning: rgb(0xF0BE75),
    error: rgb(0xFFABB2),
    error_bg: rgb(0x47272D),
    categories: [rgb(0x67D3A6), rgb(0x8FBBFD), rgb(0xC3A6F1), rgb(0xEEBC79)],
};

pub fn palette(ctx: &Context) -> &'static Palette {
    for_theme(ctx.theme())
}

fn for_theme(theme: Theme) -> &'static Palette {
    match theme {
        Theme::Light => &LIGHT,
        Theme::Dark => &DARK,
    }
}

pub fn install(ctx: &Context) {
    ctx.options_mut(|options| options.fallback_theme = Theme::Light);
    for theme in [Theme::Light, Theme::Dark] {
        let p = for_theme(theme);
        ctx.style_mut_of(theme, |style| {
            for (kind, size) in [
                (TextStyle::Body, 14.0),
                (TextStyle::Button, 14.0),
                (TextStyle::Small, 12.0),
                (TextStyle::Heading, 24.0),
            ] {
                style.text_styles.insert(kind, FontId::proportional(size));
            }
            style
                .text_styles
                .insert(TextStyle::Monospace, FontId::monospace(13.0));
            style.spacing.item_spacing = Vec2::new(8.0, 8.0);
            style.spacing.interact_size = Vec2::new(CONTROL_HEIGHT, CONTROL_HEIGHT);
            style.spacing.button_padding = Vec2::new(11.0, 4.0);
            style.spacing.window_margin = Margin::same(22);
            style.spacing.menu_margin = Margin::same(10);
            let v = &mut style.visuals;
            v.override_text_color = Some(p.text);
            v.weak_text_color = Some(p.muted);
            v.panel_fill = p.canvas;
            v.window_fill = p.panel;
            v.window_stroke = Stroke::new(1.0, p.border);
            v.window_corner_radius = 12.into();
            v.extreme_bg_color = p.field;
            v.text_edit_bg_color = Some(p.field);
            v.faint_bg_color = p.canvas;
            v.code_bg_color = p.field;
            v.selection.bg_fill = p.tint;
            v.selection.stroke = Stroke::new(1.0, p.accent);
            v.hyperlink_color = p.accent;
            v.error_fg_color = p.error;
            v.warn_fg_color = p.warning;
            v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.border);
            for (widget, fill, border) in [
                (&mut v.widgets.inactive, p.field, p.border),
                (&mut v.widgets.hovered, p.canvas, p.accent),
                (&mut v.widgets.active, p.tint, p.accent),
                (&mut v.widgets.open, p.tint, p.accent),
            ] {
                widget.bg_fill = fill;
                widget.weak_bg_fill = fill;
                widget.bg_stroke = Stroke::new(1.0, border);
                widget.fg_stroke = Stroke::new(1.0, p.text);
                widget.corner_radius = CONTROL_RADIUS.into();
                widget.expansion = 0.0;
            }
        });
    }
    watch_system_theme(ctx);
}

// winit supplies native appearance updates on macOS and Windows. The Linux
// backend leaves the system theme unset, so read and watch the desktop portal.
#[cfg(target_os = "linux")]
fn watch_system_theme(ctx: &Context) {
    let ctx = ctx.clone();
    std::thread::spawn(move || {
        let apply = |mode| {
            let theme = match mode {
                dark_light::Mode::Dark => Theme::Dark,
                _ => Theme::Light,
            };
            ctx.options_mut(|options| options.fallback_theme = theme);
            ctx.request_repaint();
        };
        if let Ok(mode) = dark_light::detect() {
            apply(mode);
        }
        if let Ok(watcher) = dark_light::subscribe() {
            for mode in watcher.iter() {
                apply(mode);
            }
        }
    });
}

#[cfg(not(target_os = "linux"))]
fn watch_system_theme(_: &Context) {}

pub fn input<'a>(ui: &egui::Ui, value: &'a mut String, hint: &str) -> egui::TextEdit<'a> {
    egui::TextEdit::singleline(value)
        .hint_text(RichText::new(hint).color(palette(ui.ctx()).placeholder))
        .min_size(Vec2::new(0.0, CONTROL_HEIGHT))
        .margin(Margin::symmetric(FIELD_PADDING, 3))
        .vertical_align(egui::Align::Center)
}

pub fn primary(ui: &egui::Ui, icon: Icon, text: &str, width: f32) -> IconButton<'static> {
    let mut action = icon.button(egui::RichText::new(text).strong().color(Color32::WHITE));
    action.color = Some(Color32::WHITE);
    action.button = action
        .button
        .fill(palette(ui.ctx()).action)
        .stroke(Stroke::NONE)
        .min_size(Vec2::new(width, CONTROL_HEIGHT));
    action
}

pub fn window_radius(ctx: &Context) -> u8 {
    let fills_screen = ctx.input(|input| {
        input.viewport().maximized == Some(true) || input.viewport().fullscreen == Some(true)
    });
    if cfg!(target_os = "macos") && !fills_screen {
        14
    } else {
        0
    }
}

pub fn modal(ctx: &Context, id: &'static str) -> egui::Modal {
    let modal = egui::Modal::new(egui::Id::new(id));
    // Keep the dimming layer inside the rounded window silhouette.
    ctx.layer_painter(modal.area.layer()).rect_filled(
        ctx.content_rect(),
        window_radius(ctx),
        modal.backdrop_color,
    );
    modal
        .backdrop_color(Color32::TRANSPARENT)
        .frame(Frame::window(&ctx.style()))
}
