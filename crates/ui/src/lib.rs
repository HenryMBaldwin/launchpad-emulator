//! An egui widget that draws a Launchpad surface and turns clicks into interactions.
//!
//! The widget is not generic over the device. It draws whatever [`Surface`] it is handed, using a
//! [`Layout`] to know which positions are pads, round buttons or the logo.
//!
//! ```no_run
//! use launchpad_emulator::devices::LaunchpadX;
//! use launchpad_emulator_ui::{Layout, LaunchpadUi};
//!
//! # fn demo(ui: &mut egui::Ui, surface: &launchpad_emulator::Surface) {
//! let layout = Layout::for_device::<LaunchpadX>();
//! let mut widget = LaunchpadUi::new();
//! for interaction in widget.show(ui, &layout, surface, 0.0).inner {
//!     println!("{interaction:?}");
//! }
//! # }
//! ```

use egui::{Color32, CornerRadius, InnerResponse, Pos2, Rect, Response, Sense, Ui, Vec2};
use launchpad_emulator::{DeviceSpec, Interaction, Pad, PadRole, Rgb, Surface};

/// Colour of the chassis behind the pads.
const BODY: Color32 = Color32::from_rgb(28, 28, 32);

/// Colour of a pad that is not lit.
const UNLIT: Color32 = Color32::from_rgb(58, 58, 64);

/// Fraction of a cell left as a gap between neighbours.
const GAP: f32 = 0.16;

/// Size of an edge button relative to a grid pad.
const CONTROL_SCALE: f32 = 0.82;

/// Corner radius of a grid pad, as a fraction of a cell.
const PAD_CORNER: f32 = 0.16;

/// Corner radius of an edge button, as a fraction of a cell.
const CONTROL_CORNER: f32 = 0.26;

/// Height of the logo bar relative to a grid pad.
const LOGO_HEIGHT: f32 = 0.30;

/// Outline drawn around the pad under the pointer.
const PRESSED: Color32 = Color32::from_rgb(240, 240, 245);

/// Width of that outline, as a fraction of a cell.
const PRESSED_WIDTH: f32 = 0.055;

/// Seconds a pad must be held for the aftertouch ramp to reach full pressure.
const RAMP_SECONDS: f64 = 1.0;

/// Which positions of a surface hold pads, round buttons or the logo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    width: u8,
    height: u8,
    roles: Vec<PadRole>,
}

impl Layout {
    /// Builds the layout of a device.
    #[must_use]
    pub fn for_device<S: DeviceSpec>() -> Self {
        Self {
            width: S::WIDTH,
            height: S::HEIGHT,
            roles: Pad::all(S::WIDTH, S::HEIGHT).map(S::role).collect(),
        }
    }

    /// Surface width in pads.
    #[must_use]
    pub const fn width(&self) -> u8 {
        self.width
    }

    /// Surface height in pads.
    #[must_use]
    pub const fn height(&self) -> u8 {
        self.height
    }

    /// The role of a position, [`PadRole::Absent`] outside the surface.
    #[must_use]
    pub fn role(&self, pad: Pad) -> PadRole {
        if pad.x >= self.width || pad.y >= self.height {
            return PadRole::Absent;
        }
        let index = usize::from(pad.y) * usize::from(self.width) + usize::from(pad.x);
        self.roles.get(index).copied().unwrap_or(PadRole::Absent)
    }

    /// Every position of the layout, in row-major order.
    pub fn pads(&self) -> impl Iterator<Item = Pad> + use<> {
        Pad::all(self.width, self.height)
    }
}

/// A pad currently held down by the pointer.
#[derive(Debug, Clone, Copy)]
struct Held {
    pad: Pad,
    since: f64,
    pressure: u8,
}

/// Draws a Launchpad surface and reports the interactions the pointer produces.
#[derive(Debug, Clone)]
pub struct LaunchpadUi {
    velocity: u8,
    aftertouch_on_hold: bool,
    held: Option<Held>,
}

impl Default for LaunchpadUi {
    fn default() -> Self {
        Self::new()
    }
}

impl LaunchpadUi {
    /// Creates a widget reporting full velocity presses.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            velocity: 127,
            aftertouch_on_hold: true,
            held: None,
        }
    }

    /// Velocity reported for a click, in `1..=127`.
    #[must_use]
    pub const fn velocity(&self) -> u8 {
        self.velocity
    }

    /// Sets the velocity reported for a click, clamped to `1..=127`.
    pub fn set_velocity(&mut self, velocity: u8) {
        self.velocity = velocity.clamp(1, 127);
    }

    /// Whether holding a grid pad ramps aftertouch pressure.
    #[must_use]
    pub const fn aftertouch_on_hold(&self) -> bool {
        self.aftertouch_on_hold
    }

    /// Sets whether holding a grid pad ramps aftertouch pressure.
    pub const fn set_aftertouch_on_hold(&mut self, on: bool) {
        self.aftertouch_on_hold = on;
    }

    /// Draws the surface at `phase` through the current beat and returns any interactions.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        layout: &Layout,
        surface: &Surface,
        phase: f32,
    ) -> InnerResponse<Vec<Interaction>> {
        let side = ui.available_size().min_elem().max(180.0);
        let (response, painter) = ui.allocate_painter(Vec2::splat(side), Sense::click_and_drag());
        let board = response.rect;
        let cell = side / f32::from(layout.width().max(layout.height()).max(1));

        // Resolve the pointer first so a press outlines its pad on the same frame
        let interactions = self.interactions(ui, layout, &response, board, cell);

        painter.rect_filled(board, CornerRadius::same(12), BODY);
        for pad in layout.pads() {
            let role = layout.role(pad);
            if !role.is_lit() {
                continue;
            }
            let color = to_color32(surface.color_at(pad, phase));
            let rect = cell_rect(board, cell, pad);
            draw_pad(&painter, rect, cell, role, color);
        }
        if let Some(held) = self.held {
            outline(
                &painter,
                cell_rect(board, cell, held.pad),
                cell,
                layout.role(held.pad),
            );
        }

        InnerResponse::new(interactions, response)
    }

    /// Translates pointer state into presses, releases and aftertouch.
    fn interactions(
        &mut self,
        ui: &Ui,
        layout: &Layout,
        response: &Response,
        board: Rect,
        cell: f32,
    ) -> Vec<Interaction> {
        let (now, primary) = ui.input(|i| (i.time, i.pointer.primary_down()));
        // Only the primary button plays; secondary is left for a host's own menu
        let down = primary && response.is_pointer_button_down_on();
        let over = response
            .interact_pointer_pos()
            .and_then(|pos| pad_at(layout, board, cell, pos))
            .filter(|pad| layout.role(*pad).is_button());

        let mut out = Vec::new();
        match (down.then_some(over).flatten(), self.held) {
            // Dragged off the pad, or the button was let go
            (None, Some(held)) => {
                out.push(Interaction::Release { pad: held.pad });
                self.held = None;
            }
            (Some(pad), None) => {
                out.push(Interaction::Press {
                    pad,
                    velocity: self.velocity,
                });
                self.held = Some(Held {
                    pad,
                    since: now,
                    pressure: 0,
                });
            }
            (Some(pad), Some(held)) if held.pad != pad => {
                out.push(Interaction::Release { pad: held.pad });
                out.push(Interaction::Press {
                    pad,
                    velocity: self.velocity,
                });
                self.held = Some(Held {
                    pad,
                    since: now,
                    pressure: 0,
                });
            }
            (Some(pad), Some(mut held)) => {
                if self.aftertouch_on_hold && layout.role(pad) == PadRole::Grid {
                    let pressure = ramp(now - held.since);
                    if pressure != held.pressure {
                        held.pressure = pressure;
                        self.held = Some(held);
                        out.push(Interaction::Aftertouch { pad, pressure });
                    }
                }
            }
            (None, None) => {}
        }
        out
    }
}

/// Pressure reached after holding for `seconds`.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn ramp(seconds: f64) -> u8 {
    let level = (seconds / RAMP_SECONDS).clamp(0.0, 1.0);
    (level * 127.0).round() as u8
}

/// The square a pad occupies on the board.
fn cell_rect(board: Rect, cell: f32, pad: Pad) -> Rect {
    let min = board.min + Vec2::new(f32::from(pad.x) * cell, f32::from(pad.y) * cell);
    Rect::from_min_size(min, Vec2::splat(cell)).shrink(cell * GAP * 0.5)
}

/// The pad under a position, if any.
fn pad_at(layout: &Layout, board: Rect, cell: f32, pos: Pos2) -> Option<Pad> {
    layout
        .pads()
        .find(|pad| cell_rect(board, cell, *pad).contains(pos))
}

/// Draws one pad as a flat rounded rectangle.
fn draw_pad(painter: &egui::Painter, rect: Rect, cell: f32, role: PadRole, color: Color32) {
    let fill = if color == Color32::BLACK {
        UNLIT
    } else {
        color
    };
    match role {
        PadRole::Grid => {
            painter.rect_filled(rect, corner_radius(cell * PAD_CORNER), fill);
        }
        PadRole::Control => {
            let button = Rect::from_center_size(rect.center(), rect.size() * CONTROL_SCALE);
            painter.rect_filled(button, corner_radius(cell * CONTROL_CORNER), fill);
        }
        // The logo is a bar across the top right, never a button
        PadRole::Logo => {
            let bar = Rect::from_center_size(
                rect.center(),
                Vec2::new(rect.width() * CONTROL_SCALE, cell * LOGO_HEIGHT),
            );
            painter.rect_filled(bar, corner_radius(cell * LOGO_HEIGHT * 0.5), fill);
        }
        PadRole::Absent => {}
    }
}

/// Outlines the pad being held, matching the shape it was drawn with.
fn outline(painter: &egui::Painter, rect: Rect, cell: f32, role: PadRole) {
    let stroke = egui::Stroke::new(cell * PRESSED_WIDTH, PRESSED);
    let (shape, radius) = match role {
        PadRole::Grid => (rect, cell * PAD_CORNER),
        PadRole::Control => (
            Rect::from_center_size(rect.center(), rect.size() * CONTROL_SCALE),
            cell * CONTROL_CORNER,
        ),
        PadRole::Logo | PadRole::Absent => return,
    };
    painter.rect_stroke(
        shape,
        corner_radius(radius),
        stroke,
        egui::StrokeKind::Inside,
    );
}

/// Converts a length in points into a corner radius.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn corner_radius(points: f32) -> CornerRadius {
    CornerRadius::same(points.clamp(0.0, 255.0) as u8)
}

/// Converts an emulator colour into an egui colour.
fn to_color32(color: Rgb) -> Color32 {
    Color32::from_rgb(color.r, color.g, color.b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use launchpad_emulator::devices::LaunchpadX;

    #[test]
    fn a_device_layout_matches_its_surface() {
        let layout = Layout::for_device::<LaunchpadX>();
        assert_eq!((layout.width(), layout.height()), (9, 9));
        assert_eq!(layout.pads().count(), 81);
        assert_eq!(layout.role(Pad::new(8, 0)), PadRole::Logo);
        assert_eq!(layout.role(Pad::new(0, 0)), PadRole::Control);
        assert_eq!(layout.role(Pad::new(0, 1)), PadRole::Grid);
        assert_eq!(layout.role(Pad::new(20, 20)), PadRole::Absent);
    }

    #[test]
    fn cells_tile_the_board_without_overlapping() {
        let board = Rect::from_min_size(Pos2::ZERO, Vec2::splat(90.0));
        let a = cell_rect(board, 10.0, Pad::new(0, 0));
        let b = cell_rect(board, 10.0, Pad::new(1, 0));
        assert!(a.max.x <= b.min.x, "{a:?} overlaps {b:?}");
        assert!(board.contains_rect(a));
        assert!(board.contains_rect(cell_rect(board, 10.0, Pad::new(8, 8))));
    }

    #[test]
    fn positions_map_back_to_the_pad_under_them() {
        let layout = Layout::for_device::<LaunchpadX>();
        let board = Rect::from_min_size(Pos2::ZERO, Vec2::splat(90.0));
        for pad in layout.pads() {
            let center = cell_rect(board, 10.0, pad).center();
            assert_eq!(pad_at(&layout, board, 10.0, center), Some(pad));
        }
        assert_eq!(pad_at(&layout, board, 10.0, Pos2::new(500.0, 500.0)), None);
    }

    #[test]
    fn the_aftertouch_ramp_spans_the_full_range() {
        assert_eq!(ramp(0.0), 0);
        assert_eq!(ramp(RAMP_SECONDS / 2.0), 64);
        assert_eq!(ramp(RAMP_SECONDS), 127);
        assert_eq!(ramp(10.0), 127);
    }

    #[test]
    fn velocity_is_clamped_to_a_real_press() {
        let mut widget = LaunchpadUi::new();
        widget.set_velocity(0);
        assert_eq!(widget.velocity(), 1);
        widget.set_velocity(200);
        assert_eq!(widget.velocity(), 127);
    }
}
