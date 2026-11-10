use crate::{
    app::{App, FlashMonitor, Job},
    i18n::t,
};
use eframe::egui::{self, Context, Frame, Margin, Rect, RichText, ScrollArea, TextStyle, Ui, Vec2};
use std::{collections::VecDeque, sync::atomic::Ordering};

use super::{
    icons::Icon,
    style::{self, palette},
};

pub fn monitor(app: &mut App, ctx: &Context) {
    let Some(monitor) = app.flash_monitor.as_ref().filter(|monitor| monitor.visible) else {
        return;
    };
    let p = palette(ctx);
    let first = app.flash_log_start(monitor);
    let result = style::modal(ctx, "flash_monitor").show(ctx, |ui| {
        let size = ctx.content_rect().size();
        ui.set_width((size.x - 110.0).min(860.0));
        let hide = ui
            .horizontal(|ui| {
                ui.heading(t("flash_monitor"));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add(Icon::Down.button(t("hide_monitor"))).clicked()
                })
                .inner
            })
            .inner;
        let (status, failed) = monitor
            .outcome
            .as_ref()
            .map(outcome_status)
            .unwrap_or((&app.activity, false));
        ui.colored_label(if failed { p.error } else { p.accent }, status);
        ScrollArea::vertical()
            .id_salt("target")
            .max_height(64.0)
            .show(ui, |ui| {
                ui.label(RichText::new(&monitor.summary).small().color(p.muted));
            });
        ui.horizontal(|ui| {
            bar(
                ui,
                percent(app.job.as_ref(), monitor),
                ui.available_width() - 56.0,
                app.job.is_some(),
            );
        });
        ui.add_space(4.0);
        ui.label(RichText::new(t("flash_output")).strong());
        log_view(
            ui,
            &app.log,
            first,
            (size.y - 335.0).clamp(180.0, 420.0),
            "flash_log",
        );
        ui.add_space(4.0);
        let close = ui
            .horizontal(|ui| {
                if ui.add(Icon::Copy.button(t("copy_log"))).clicked() {
                    ctx.copy_text(
                        app.log
                            .iter()
                            .skip(first)
                            .map(String::as_str)
                            .collect::<Vec<_>>()
                            .join("\n"),
                    );
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(job) = &app.job {
                        stop_button(ui, job);
                        false
                    } else {
                        ui.button(t("close")).clicked()
                    }
                })
                .inner
            })
            .inner;
        (hide, close)
    });
    let (hide, close) = result.inner;
    if app.job.is_none() && (close || result.should_close()) {
        app.flash_monitor = None;
    } else if hide && let Some(monitor) = &mut app.flash_monitor {
        monitor.visible = false;
    }
}

pub fn outcome_status(outcome: &Result<String, String>) -> (&str, bool) {
    match outcome {
        Ok(status) => (status, false),
        Err(status) => (status, true),
    }
}

pub fn percent(job: Option<&Job>, monitor: &FlashMonitor) -> Option<f32> {
    if monitor.outcome.as_ref().is_some_and(Result::is_ok) {
        Some(1.0)
    } else {
        job.and_then(|job| job.progress)
    }
}

pub fn bar(ui: &mut Ui, percent: Option<f32>, width: f32, running: bool) {
    let p = palette(ui.ctx());
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(width.max(48.0), 8.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, 4, p.border);
    let fill = match percent {
        Some(fraction) if fraction > 0.0 => Rect::from_min_size(
            rect.min,
            Vec2::new(rect.width() * fraction.clamp(0.0, 1.0), rect.height()),
        ),
        None if running => {
            let segment = rect.width() * 0.24;
            let time = ui.input(|input| input.time) as f32;
            let travel =
                (1.0 - (2.0 * (time * 0.7).fract() - 1.0).abs()) * (rect.width() - segment);
            Rect::from_min_size(
                rect.min + egui::vec2(travel, 0.0),
                egui::vec2(segment, rect.height()),
            )
        }
        _ => Rect::NOTHING,
    };
    if fill.is_positive() {
        ui.painter().rect_filled(fill, 4, p.action);
    }
    response.widget_info(|| {
        let mut info = egui::WidgetInfo::labeled(
            egui::WidgetType::ProgressIndicator,
            true,
            t("flash_monitor"),
        );
        info.value = percent.map(|fraction| f64::from((fraction * 100.0).round()));
        info
    });
    if let Some(fraction) = percent {
        ui.label(
            RichText::new(format!("{:.0}%", fraction * 100.0))
                .small()
                .color(p.muted),
        );
    }
}

pub fn stop_button(ui: &mut Ui, job: &Job) {
    let stopping = job.cancel.load(Ordering::Relaxed);
    if ui
        .add_enabled(
            !stopping,
            Icon::Stop.button(t(if stopping { "stopping" } else { "stop" })),
        )
        .clicked()
    {
        job.cancel.store(true, Ordering::Relaxed);
    }
}

pub fn log_view(ui: &mut Ui, log: &VecDeque<String>, first: usize, height: f32, id: &'static str) {
    let p = palette(ui.ctx());
    let height = (height - 18.0).max(28.0);
    Frame::new()
        .fill(p.field)
        .stroke(egui::Stroke::new(1.0, p.border))
        .corner_radius(8)
        .inner_margin(Margin::same(8))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.set_min_height(height);
            let row_height = ui.text_style_height(&TextStyle::Monospace);
            ScrollArea::both()
                .id_salt(id)
                .max_height(height)
                .stick_to_bottom(true)
                .show_rows(
                    ui,
                    row_height,
                    log.len().saturating_sub(first),
                    |ui, rows| {
                        for index in rows {
                            ui.add(
                                egui::Label::new(RichText::new(&log[first + index]).monospace())
                                    .extend(),
                            );
                        }
                    },
                );
        });
}
