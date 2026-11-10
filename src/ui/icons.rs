use eframe::egui::{
    self, Atom, Button, Color32, Id, Painter, Pos2, Rect, Stroke, StrokeKind, Ui, Vec2, WidgetText,
    vec2,
};

#[derive(Clone, Copy)]
pub enum Icon {
    Chip,
    Layers,
    Folder,
    File,
    Plus,
    Refresh,
    Flash,
    Save,
    Terminal,
    Copy,
    Trash,
    Stop,
    Down,
    Up,
}

impl Icon {
    pub fn paint(self, painter: &Painter, center: Pos2, color: Color32) {
        // All icons share a 16-point grid and the same stroke weight.
        let origin = center - Vec2::splat(8.0);
        let stroke = Stroke::new(1.45, color);
        let point = |(x, y): (f32, f32)| origin + vec2(x, y);
        let path = |points: &[(f32, f32)]| {
            painter.line(points.iter().copied().map(point).collect(), stroke);
        };
        let outline = |min, max, radius| {
            painter.rect_stroke(
                Rect::from_min_max(point(min), point(max)),
                radius,
                stroke,
                StrokeKind::Middle,
            );
        };
        match self {
            Self::Chip => {
                outline((3., 3.), (13., 13.), 2);
                outline((6., 6.), (10., 10.), 1);
                for i in [5.5, 10.5] {
                    path(&[(i, 1.), (i, 3.)]);
                    path(&[(i, 13.), (i, 15.)]);
                    path(&[(1., i), (3., i)]);
                    path(&[(13., i), (15., i)]);
                }
            }
            Self::Layers => {
                path(&[(1., 5.), (8., 1.5), (15., 5.), (8., 8.5), (1., 5.)]);
                path(&[(1., 8.5), (8., 12.), (15., 8.5)]);
                path(&[(1., 12.), (8., 15.5), (15., 12.)]);
            }
            Self::Folder => path(&[
                (1., 4.),
                (2., 3.),
                (6., 3.),
                (8., 5.),
                (14., 5.),
                (15., 6.),
                (15., 13.),
                (14., 14.),
                (2., 14.),
                (1., 13.),
                (1., 4.),
            ]),
            Self::File => {
                path(&[
                    (3., 14.),
                    (3., 2.),
                    (9., 2.),
                    (13., 6.),
                    (13., 14.),
                    (3., 14.),
                ]);
                path(&[(9., 2.), (9., 6.), (13., 6.)]);
            }
            Self::Plus => {
                path(&[(3., 8.), (13., 8.)]);
                path(&[(8., 3.), (8., 13.)]);
            }
            Self::Refresh => {
                let on_arc = |degrees: f32| {
                    let angle = degrees.to_radians();
                    (8. + 5.5 * angle.cos(), 8. + 5.5 * angle.sin())
                };
                let arc: [_; 25] = std::array::from_fn(|step| on_arc(45. + step as f32 * 12.5));
                let (x, y) = arc[24];
                path(&arc);
                path(&[(x - 3.4, y), (x, y), (x, y - 3.4)]);
            }
            Self::Flash => path(&[
                (9., 1.),
                (3., 9.),
                (7., 9.),
                (6.5, 15.),
                (13., 7.),
                (9., 7.),
                (9., 1.),
            ]),
            Self::Save => {
                path(&[(8., 2.), (8., 10.)]);
                path(&[(5., 7.), (8., 10.), (11., 7.)]);
                path(&[(2., 10.), (2., 14.), (14., 14.), (14., 10.)]);
            }
            Self::Terminal => {
                outline((1., 2.5), (15., 13.5), 2);
                path(&[(4., 5.5), (6.5, 8.), (4., 10.5)]);
                path(&[(8.5, 10.5), (12., 10.5)]);
            }
            Self::Copy => {
                path(&[(2., 11.), (2., 2.), (11., 2.)]);
                outline((5., 5.), (14., 14.), 1);
            }
            Self::Trash => {
                path(&[(2., 4.5), (14., 4.5)]);
                path(&[(6., 4.5), (6., 2.), (10., 2.), (10., 4.5)]);
                path(&[(4., 4.5), (5., 14.), (11., 14.), (12., 4.5)]);
            }
            Self::Stop => outline((3.5, 3.5), (12.5, 12.5), 2),
            Self::Down => path(&[(4., 6.), (8., 10.), (12., 6.)]),
            Self::Up => path(&[(4., 10.), (8., 6.), (12., 10.)]),
        }
    }

    pub fn button<'a>(self, label: impl Into<WidgetText>) -> IconButton<'a> {
        IconButton {
            icon: self,
            button: Button::new((Atom::custom(icon_slot(), Vec2::splat(16.0)), label.into())),
            color: None,
        }
    }
}

fn icon_slot() -> Id {
    Id::new("icon")
}

pub struct IconButton<'a> {
    icon: Icon,
    pub button: Button<'a>,
    pub color: Option<Color32>,
}

impl egui::Widget for IconButton<'_> {
    fn ui(self, ui: &mut Ui) -> egui::Response {
        let result = self.button.atom_ui(ui);
        if let Some(rect) = result.rect(icon_slot()) {
            let color = self
                .color
                .unwrap_or_else(|| ui.style().interact(&result.response).text_color());
            self.icon.paint(ui.painter(), rect.center(), color);
        }
        result.response
    }
}
