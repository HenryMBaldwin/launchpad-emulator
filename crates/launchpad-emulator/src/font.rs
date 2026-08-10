//! The glyphs used to draw scrolling text.

/// Rows in a glyph, matching the eight rows a scroll occupies.
pub const HEIGHT: usize = 8;

/// Columns in a glyph.
pub const WIDTH: usize = 5;

/// Blank columns left between glyphs.
pub const SPACING: usize = 1;

/// Rows of a glyph, top first, each holding [`WIDTH`] pixels in its low bits.
type Glyph = [u8; 7];

/// A glyph for every character the emulator can draw, sorted for lookup by byte.
///
/// Lowercase is drawn with the uppercase glyph, and anything unknown is drawn as a blank.
#[rustfmt::skip]
const GLYPHS: &[(u8, Glyph)] = &[
    (b' ', [0, 0, 0, 0, 0, 0, 0]),
    (b'!', [0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00000, 0b00100]),
    (b'"', [0b01010, 0b01010, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000]),
    (b'#', [0b01010, 0b11111, 0b01010, 0b01010, 0b11111, 0b01010, 0b00000]),
    (b'\'', [0b00100, 0b00100, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000]),
    (b'(', [0b00010, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00010]),
    (b')', [0b01000, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01000]),
    (b'+', [0b00000, 0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0b00000]),
    (b',', [0b00000, 0b00000, 0b00000, 0b00000, 0b00110, 0b00100, 0b01000]),
    (b'-', [0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000]),
    (b'.', [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00110, 0b00110]),
    (b'/', [0b00001, 0b00010, 0b00010, 0b00100, 0b01000, 0b01000, 0b10000]),
    (b'0', [0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110]),
    (b'1', [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110]),
    (b'2', [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111]),
    (b'3', [0b11111, 0b00010, 0b00100, 0b00010, 0b00001, 0b10001, 0b01110]),
    (b'4', [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010]),
    (b'5', [0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110]),
    (b'6', [0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110]),
    (b'7', [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000]),
    (b'8', [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110]),
    (b'9', [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100]),
    (b':', [0b00000, 0b00110, 0b00110, 0b00000, 0b00110, 0b00110, 0b00000]),
    (b'<', [0b00010, 0b00100, 0b01000, 0b10000, 0b01000, 0b00100, 0b00010]),
    (b'=', [0b00000, 0b00000, 0b11111, 0b00000, 0b11111, 0b00000, 0b00000]),
    (b'>', [0b01000, 0b00100, 0b00010, 0b00001, 0b00010, 0b00100, 0b01000]),
    (b'?', [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b00000, 0b00100]),
    (b'A', [0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001]),
    (b'B', [0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110]),
    (b'C', [0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110]),
    (b'D', [0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110]),
    (b'E', [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111]),
    (b'F', [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000]),
    (b'G', [0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01110]),
    (b'H', [0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001]),
    (b'I', [0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110]),
    (b'J', [0b00111, 0b00010, 0b00010, 0b00010, 0b00010, 0b10010, 0b01100]),
    (b'K', [0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001]),
    (b'L', [0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111]),
    (b'M', [0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001]),
    (b'N', [0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001]),
    (b'O', [0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110]),
    (b'P', [0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000]),
    (b'Q', [0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101]),
    (b'R', [0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001]),
    (b'S', [0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110]),
    (b'T', [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100]),
    (b'U', [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110]),
    (b'V', [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100]),
    (b'W', [0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b11011, 0b10001]),
    (b'X', [0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001]),
    (b'Y', [0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100]),
    (b'Z', [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111]),
    (b'_', [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b11111]),
];

/// Whether a glyph has a pixel at `column` of `row`.
fn pixel(glyph: Glyph, column: usize, row: usize) -> bool {
    let Some(bits) = glyph.get(row) else {
        return false;
    };
    let shift = WIDTH - 1 - column.min(WIDTH - 1);
    (bits >> shift) & 1 == 1
}

/// The glyph for a byte, folding lowercase onto uppercase.
fn glyph(byte: u8) -> Option<Glyph> {
    let byte = byte.to_ascii_uppercase();
    GLYPHS
        .iter()
        .find(|(candidate, _)| *candidate == byte)
        .map(|(_, glyph)| *glyph)
}

/// Renders `text` into columns of pixels, each column holding [`HEIGHT`] rows in its low bits.
///
/// Bit 0 of a column is the top row. Unknown characters are skipped.
#[must_use]
pub fn columns(text: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len() * (WIDTH + SPACING));
    for &byte in text {
        let Some(glyph) = glyph(byte) else {
            continue;
        };
        for column in 0..WIDTH {
            let mut bits = 0u8;
            for row in 0..HEIGHT {
                if pixel(glyph, column, row) {
                    bits |= 1 << row;
                }
            }
            out.push(bits);
        }
        out.extend(std::iter::repeat_n(0, SPACING));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Draws columns back out as rows of text, for eyeballing a glyph
    fn art(columns: &[u8]) -> String {
        (0..HEIGHT)
            .map(|row| {
                columns
                    .iter()
                    .map(|bits| if (bits >> row) & 1 == 1 { '#' } else { '.' })
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn a_glyph_is_five_columns_and_a_gap() {
        assert_eq!(columns(b"A").len(), WIDTH + SPACING);
        assert_eq!(columns(b"AB").len(), 2 * (WIDTH + SPACING));
    }

    #[test]
    fn letters_render_the_shape_they_are_written_as() {
        assert_eq!(
            art(&columns(b"A")),
            ".###..\n\
             #...#.\n\
             #...#.\n\
             #####.\n\
             #...#.\n\
             #...#.\n\
             #...#.\n\
             ......"
        );
        assert_eq!(
            art(&columns(b"1")),
            "..#...\n\
             .##...\n\
             ..#...\n\
             ..#...\n\
             ..#...\n\
             ..#...\n\
             .###..\n\
             ......"
        );
    }

    #[test]
    fn lowercase_uses_the_uppercase_glyph() {
        assert_eq!(columns(b"a"), columns(b"A"));
        assert_eq!(columns(b"hello"), columns(b"HELLO"));
    }

    #[test]
    fn a_space_is_blank_and_unknown_characters_are_skipped() {
        assert!(columns(b" ").iter().all(|bits| *bits == 0));
        assert!(columns(&[0x01, 0x7F]).is_empty());
    }

    #[test]
    fn every_glyph_fits_the_grid() {
        for (byte, glyph) in GLYPHS {
            for row in glyph {
                assert!(
                    *row < (1 << WIDTH),
                    "{:?} has a row wider than {WIDTH}",
                    *byte as char
                );
            }
        }
    }

    #[test]
    fn the_table_covers_letters_digits_and_a_space() {
        for byte in b'A'..=b'Z' {
            assert!(glyph(byte).is_some(), "missing {:?}", byte as char);
        }
        for byte in b'0'..=b'9' {
            assert!(glyph(byte).is_some(), "missing {:?}", byte as char);
        }
        assert!(glyph(b' ').is_some());
    }
}
