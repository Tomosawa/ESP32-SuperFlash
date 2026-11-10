use crate::engine::Entry;
use crate::model::human_size;
use eframe::egui::{
    self, Align2, Frame, Margin, Rect, RichText, ScrollArea, Sense, Stroke, TextStyle,
    TextWrapMode, Ui, Vec2, WidgetText,
};
use std::path::Path;

use super::style::palette;

pub fn show(ui: &mut Ui, entries: &[Entry], uppercase: bool, folder: &str) {
    let p = palette(ui.ctx());
    let mut hidden = None;
    let mut visible = Vec::new();
    for (index, entry) in entries.iter().enumerate() {
        if hidden.is_some_and(|path: &Path| entry.path.starts_with(path)) {
            continue;
        }
        hidden = None;
        let expandable = entry.directory
            && entries
                .get(index + 1)
                .is_some_and(|next| next.path.starts_with(&entry.path));
        let depth = entry.path.components().count().saturating_sub(1);
        let id = ui.make_persistent_id(("tree", folder, &entry.path));
        let expanded = expandable
            && ui
                .data(|data| data.get_temp::<bool>(id))
                .unwrap_or(depth == 0);
        visible.push((entry, depth, id, expandable, expanded));
        if expandable && !expanded {
            hidden = Some(entry.path.as_path());
        }
    }
    Frame::new()
        .fill(p.field)
        .stroke(Stroke::new(1.0, p.border))
        .corner_radius(9)
        .inner_margin(Margin::same(4))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            let height = 29.0;
            let viewport_height = (height * visible.len() as f32).min(height * 7.0);
            ScrollArea::vertical()
                .id_salt("tree_scroll")
                .auto_shrink([false, false])
                .min_scrolled_height(viewport_height)
                .max_height(viewport_height)
                .show_rows(ui, height, visible.len(), |ui, range| {
                    for (entry, depth, id, expandable, expanded) in &visible[range] {
                        let (rect, _) = ui.allocate_exact_size(
                            Vec2::new(ui.available_width(), height),
                            Sense::hover(),
                        );
                        let name = entry.path.file_name().unwrap_or_default().to_string_lossy();
                        let name = if uppercase {
                            name.to_uppercase()
                        } else {
                            name.into_owned()
                        };
                        let response = ui
                            .interact(
                                rect,
                                *id,
                                if *expandable {
                                    Sense::click()
                                } else {
                                    Sense::hover()
                                },
                            )
                            .on_hover_text(entry.path.to_string_lossy());
                        response.widget_info(|| {
                            if *expandable {
                                egui::WidgetInfo::selected(
                                    egui::WidgetType::CollapsingHeader,
                                    ui.is_enabled(),
                                    *expanded,
                                    &name,
                                )
                            } else {
                                egui::WidgetInfo::labeled(
                                    egui::WidgetType::Label,
                                    ui.is_enabled(),
                                    &name,
                                )
                            }
                        });
                        if response.hovered() || response.has_focus() {
                            ui.painter().rect_filled(rect, 5, p.tint);
                        }
                        if *expandable && response.clicked() {
                            ui.data_mut(|data| data.insert_temp(*id, !expanded));
                        }

                        let x = rect.left() + 11.0 + (*depth as f32 * 15.0).min(110.0);
                        let y = rect.center().y;
                        let painter = ui.painter();
                        if *expandable {
                            let points = if *expanded {
                                [(-3.5, -1.5), (0.0, 2.0), (3.5, -1.5)]
                            } else {
                                [(-1.5, -3.5), (2.0, 0.0), (-1.5, 3.5)]
                            };
                            painter.line(
                                points.map(|(dx, dy)| egui::pos2(x + dx, y + dy)).to_vec(),
                                Stroke::new(1.3, p.muted),
                            );
                        }
                        let c = egui::pos2(x + 18.0, y);
                        let icon_stroke =
                            Stroke::new(1.2, if entry.directory { p.accent } else { p.muted });
                        if entry.directory {
                            let points = [
                                (-7., -5.),
                                (-2., -5.),
                                (0., -3.),
                                (7., -3.),
                                (7., 5.),
                                (-7., 5.),
                                (-7., -5.),
                            ];
                            painter.line(
                                points.map(|(dx, dy)| c + Vec2::new(dx, dy)).to_vec(),
                                icon_stroke,
                            );
                        } else {
                            let points = [
                                (-5., -6.),
                                (1., -6.),
                                (5., -2.),
                                (5., 6.),
                                (-5., 6.),
                                (-5., -6.),
                            ];
                            painter.line(
                                points.map(|(dx, dy)| c + Vec2::new(dx, dy)).to_vec(),
                                icon_stroke,
                            );
                            painter.line(
                                vec![
                                    c + Vec2::new(1., -6.),
                                    c + Vec2::new(1., -2.),
                                    c + Vec2::new(5., -2.),
                                ],
                                icon_stroke,
                            );
                        }
                        let mut text_right = rect.right() - 9.0;
                        if !entry.directory {
                            let size = painter.text(
                                egui::pos2(text_right, y),
                                Align2::RIGHT_CENTER,
                                human_size(entry.size),
                                TextStyle::Small.resolve(ui.style()),
                                p.muted,
                            );
                            text_right = size.left() - 12.0;
                        }
                        let left = c.x + 13.0;
                        let galley = WidgetText::from(RichText::new(name)).into_galley(
                            ui,
                            Some(TextWrapMode::Truncate),
                            (text_right - left).max(0.0),
                            TextStyle::Body,
                        );
                        let clip = Rect::from_min_max(
                            egui::pos2(left, rect.top()),
                            egui::pos2(text_right.max(left), rect.bottom()),
                        );
                        ui.painter().with_clip_rect(clip).galley(
                            egui::pos2(left, y - galley.size().y / 2.0),
                            galley,
                            p.text,
                        );
                    }
                });
        });
}
