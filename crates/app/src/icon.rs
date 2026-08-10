//! The window icon, drawn in code so the app carries no image files.

use eframe::egui::IconData;

/// Width and height of the icon in pixels.
const SIZE: u16 = 64;

/// Pads across the icon.
const PADS: u16 = 3;

/// Colour behind the pads.
const BODY: [u8; 3] = [28, 28, 32];

/// The pads, read left to right and top to bottom, from the device's own palette.
const PADS_RGB: [[u8; 3]; 9] = [
    [255, 0, 0],
    [255, 132, 0],
    [255, 255, 0],
    [0, 255, 0],
    [0, 228, 255],
    [0, 0, 255],
    [132, 0, 255],
    [255, 0, 255],
    [255, 255, 255],
];

/// Builds the window icon.
#[must_use]
pub fn build() -> IconData {
    let side = usize::from(SIZE);
    let mut rgba = Vec::with_capacity(side * side * 4);
    let cell = f32::from(SIZE) / f32::from(PADS);
    let inset = cell * 0.14;
    let radius = cell * 0.22;

    for y in 0..side {
        for x in 0..side {
            let color = pad_at(x, y, cell, inset, radius).unwrap_or(BODY);
            rgba.extend_from_slice(&[color[0], color[1], color[2], 0xFF]);
        }
    }
    IconData {
        rgba,
        width: u32::from(SIZE),
        height: u32::from(SIZE),
    }
}

/// The colour of the pad covering a pixel, if one does.
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
fn pad_at(x: usize, y: usize, cell: f32, inset: f32, radius: f32) -> Option<[u8; 3]> {
    let (column, row) = (x as f32 / cell, y as f32 / cell);
    let (index_x, index_y) = (column as usize, row as usize);
    if index_x >= usize::from(PADS) || index_y >= usize::from(PADS) {
        return None;
    }

    // Distance from the pad's edges, so the corners can be rounded off
    let left = x as f32 - index_x as f32 * cell;
    let top = y as f32 - index_y as f32 * cell;
    let inner = cell - 2.0 * inset;
    let (px, py) = (left - inset, top - inset);
    if px < 0.0 || py < 0.0 || px > inner || py > inner {
        return None;
    }
    let corner_x = (radius - px).max(px - (inner - radius)).max(0.0);
    let corner_y = (radius - py).max(py - (inner - radius)).max(0.0);
    if corner_x * corner_x + corner_y * corner_y > radius * radius {
        return None;
    }
    PADS_RGB.get(index_y * usize::from(PADS) + index_x).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_icon_is_a_full_rgba_square() {
        let icon = build();
        let side = usize::from(SIZE);
        assert_eq!(icon.width, u32::from(SIZE));
        assert_eq!(icon.height, u32::from(SIZE));
        assert_eq!(icon.rgba.len(), side * side * 4);
        assert!(icon.rgba.chunks(4).all(|pixel| pixel[3] == 0xFF));
    }

    #[test]
    fn the_middle_of_each_pad_carries_its_colour() {
        let cell = f32::from(SIZE) / f32::from(PADS);
        let inset = cell * 0.14;
        let radius = cell * 0.22;
        for row in 0..usize::from(PADS) {
            for column in 0..usize::from(PADS) {
                let middle = |index: usize| {
                    #[allow(
                        clippy::cast_precision_loss,
                        clippy::cast_possible_truncation,
                        clippy::cast_sign_loss
                    )]
                    let value = (index as f32 * cell + cell / 2.0) as usize;
                    value
                };
                assert_eq!(
                    pad_at(middle(column), middle(row), cell, inset, radius),
                    Some(PADS_RGB[row * usize::from(PADS) + column]),
                    "pad {column},{row}"
                );
            }
        }
    }

    #[test]
    fn the_corners_fall_outside_the_pads() {
        let cell = f32::from(SIZE) / f32::from(PADS);
        assert_eq!(pad_at(0, 0, cell, cell * 0.14, cell * 0.22), None);
    }
}
