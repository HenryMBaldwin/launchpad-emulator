//! The codec shared by the MK3 generation of Launchpads.
//!
//! The Launchpad X and the Launchpad Mini MK3 share a 9x9 layout, a palette and a `SysEx` grammar,
//! and differ only in the device ID byte and in whether the grid is velocity sensitive.

use super::palette;
use crate::Rgb;
use crate::message::{HostMessage, Interaction};
use crate::pad::Pad;
use crate::surface::{Lighting, TextScroll};

/// Manufacturer prefix every Novation `SysEx` message carries.
const NOVATION: [u8; 5] = [0xF0, 0x00, 0x20, 0x29, 0x02];

const SYSEX_END: u8 = 0xF7;
const CMD_LED: u8 = 0x03;
const CMD_TEXT: u8 = 0x07;
const CMD_BRIGHTNESS: u8 = 0x08;
const CMD_SLEEP: u8 = 0x09;
const CMD_MODE: u8 = 0x0E;
const CLOCK: u8 = 0xF8;

/// Surface width and height in pads.
pub const SIZE: u8 = 9;

/// The `SysEx` header for a device, including its device ID.
#[must_use]
pub fn sysex_header(device_id: u8) -> [u8; 6] {
    [
        NOVATION[0],
        NOVATION[1],
        NOVATION[2],
        NOVATION[3],
        NOVATION[4],
        device_id,
    ]
}

/// Converts a Programmer-mode note or control change number into a pad.
///
/// Valid numbers are `row * 10 + column` with both row and column in `1..=9`.
#[must_use]
pub const fn pad_from_midi(number: u8) -> Option<Pad> {
    let (row, column) = (number / 10, number % 10);
    if row == 0 || row > SIZE || column == 0 || column > SIZE {
        return None;
    }
    Some(Pad {
        x: column - 1,
        y: SIZE - row,
    })
}

/// The Programmer-mode note or control change number for a pad.
#[must_use]
pub const fn pad_to_midi(pad: Pad) -> Option<u8> {
    if pad.x >= SIZE || pad.y >= SIZE {
        return None;
    }
    Some((SIZE - pad.y) * 10 + pad.x + 1)
}

/// Whether a pad is one of the central 8x8 grid pads, which report notes.
#[must_use]
pub const fn is_grid(pad: Pad) -> bool {
    pad.x < SIZE - 1 && pad.y > 0
}

/// Whether a pad is the logo, which lights but never reports a press.
#[must_use]
pub const fn is_logo(pad: Pad) -> bool {
    pad.x == SIZE - 1 && pad.y == 0
}

/// Encodes an interaction as the hardware would report it in Programmer mode.
#[must_use]
pub fn encode(interaction: Interaction) -> Vec<u8> {
    let Some(number) = pad_to_midi(interaction.pad()) else {
        return Vec::new();
    };
    match interaction {
        Interaction::Press { pad, velocity } => {
            let status = if is_grid(pad) { 0x90 } else { 0xB0 };
            vec![status, number, velocity.clamp(1, 127)]
        }
        Interaction::Release { pad } => {
            let status = if is_grid(pad) { 0x90 } else { 0xB0 };
            vec![status, number, 0]
        }
        Interaction::Aftertouch { pressure, .. } => vec![0xA0, number, pressure.min(127)],
    }
}

/// Reads a Programmer-mode press, release or aftertouch reported by the hardware.
#[must_use]
pub fn decode_interaction(bytes: &[u8]) -> Option<Interaction> {
    let &[status, number, value] = bytes else {
        return None;
    };
    let pad = pad_from_midi(number)?;
    match (status, value) {
        (0x90 | 0xB0, 0) | (0x80, _) => Some(Interaction::Release { pad }),
        (0x90 | 0xB0, velocity) => Some(Interaction::Press { pad, velocity }),
        (0xA0, pressure) => Some(Interaction::Aftertouch { pad, pressure }),
        _ => None,
    }
}

/// Decodes one MIDI message from the host into the host messages it carries.
#[must_use]
pub fn decode(device_id: u8, bytes: &[u8]) -> Vec<HostMessage> {
    if bytes == [CLOCK] {
        return vec![HostMessage::Clock];
    }
    if let Some(message) = decode_channel_voice(bytes) {
        return vec![message];
    }
    if let Some(messages) = strip_sysex(device_id, bytes).and_then(decode_sysex) {
        return messages;
    }
    vec![HostMessage::Unrecognised(bytes.to_vec())]
}

/// Returns the `SysEx` payload between the device header and the terminator.
fn strip_sysex(device_id: u8, bytes: &[u8]) -> Option<&[u8]> {
    let body = bytes.strip_prefix(&sysex_header(device_id))?;
    match body.split_last() {
        Some((&SYSEX_END, rest)) => Some(rest),
        _ => None,
    }
}

/// Decodes the note and control change messages that light one pad each.
///
/// Channel 1 sets a static colour, channel 2 flashing and channel 3 pulsing. In Programmer mode
/// every pad accepts both a note and a control change.
fn decode_channel_voice(bytes: &[u8]) -> Option<HostMessage> {
    let &[status, number, value] = bytes else {
        return None;
    };
    let kind = status & 0xF0;
    if kind != 0x90 && kind != 0xB0 {
        return None;
    }
    let pad = pad_from_midi(number)?;
    let color = palette::get(value)?;
    let lighting = match status & 0x0F {
        0 => Lighting::Static(color),
        1 => Lighting::Flashing {
            a: color,
            b: Rgb::BLACK,
        },
        2 => Lighting::Pulsing(color),
        _ => return None,
    };
    Some(HostMessage::Lighting { pad, lighting })
}

/// Decodes the payload of a device `SysEx` message.
fn decode_sysex(body: &[u8]) -> Option<Vec<HostMessage>> {
    let (&command, rest) = body.split_first()?;
    match (command, rest) {
        (CMD_LED, _) => decode_lighting_specs(rest),
        (CMD_BRIGHTNESS, &[level]) => Some(vec![HostMessage::Brightness(level)]),
        (CMD_SLEEP, &[state]) => Some(vec![HostMessage::Sleep(state == 0)]),
        (CMD_MODE, &[mode]) => Some(vec![HostMessage::ProgrammerMode(mode == 1)]),
        (CMD_TEXT, _) => Some(vec![decode_text(rest)?]),
        _ => None,
    }
}

/// Decodes a run of colour specifications from the lighting `SysEx`.
fn decode_lighting_specs(mut rest: &[u8]) -> Option<Vec<HostMessage>> {
    let mut out = Vec::new();
    while !rest.is_empty() {
        let (&kind, tail) = rest.split_first()?;
        let (&number, tail) = tail.split_first()?;
        let (lighting, tail) = match kind {
            0 => {
                let (&entry, tail) = tail.split_first()?;
                (Lighting::Static(palette::get(entry)?), tail)
            }
            1 => {
                let (&b, tail) = tail.split_first()?;
                let (&a, tail) = tail.split_first()?;
                (
                    Lighting::Flashing {
                        a: palette::get(a)?,
                        b: palette::get(b)?,
                    },
                    tail,
                )
            }
            2 => {
                let (&entry, tail) = tail.split_first()?;
                (Lighting::Pulsing(palette::get(entry)?), tail)
            }
            3 => {
                let (rgb, tail) = tail.split_at_checked(3)?;
                (Lighting::Static(Rgb::from_midi(rgb)?), tail)
            }
            _ => return None,
        };
        rest = tail;
        if let Some(pad) = pad_from_midi(number) {
            out.push(HostMessage::Lighting { pad, lighting });
        }
    }
    Some(out)
}

/// Decodes the text scroll `SysEx`, whose trailing fields may all be omitted.
fn decode_text(rest: &[u8]) -> Option<HostMessage> {
    let Some((&loop_flag, tail)) = rest.split_first() else {
        return Some(HostMessage::StopScroll);
    };
    let (&speed, tail) = tail.split_first()?;
    let (color, text) = match tail.split_first() {
        Some((0, tail)) => {
            let (&entry, text) = tail.split_first()?;
            (palette::get(entry)?, text)
        }
        Some((1, tail)) => {
            let (rgb, text) = tail.split_at_checked(3)?;
            (Rgb::from_midi(rgb)?, text)
        }
        _ => return None,
    };
    Some(HostMessage::StartScroll(TextScroll {
        looping: loop_flag == 1,
        speed: decode_speed(speed),
        color,
        text: text.to_vec(),
    }))
}

/// Reads a scroll speed, where 0x40 and above means scrolling left to right.
const fn decode_speed(speed: u8) -> i8 {
    if speed >= 0x40 {
        speed.wrapping_sub(0x80).cast_signed()
    } else {
        speed.cast_signed()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    /// The Launchpad X device ID, used to exercise the shared codec.
    const ID: u8 = 0x0C;
    const RED: Rgb = Rgb { r: 255, g: 0, b: 0 };

    fn sysex(body: &[u8]) -> Vec<u8> {
        let mut bytes = sysex_header(ID).to_vec();
        bytes.extend_from_slice(body);
        bytes.push(SYSEX_END);
        bytes
    }

    #[test]
    fn corners_match_the_programmer_layout() {
        assert_eq!(pad_to_midi(Pad::new(0, 0)), Some(91));
        assert_eq!(pad_to_midi(Pad::new(7, 0)), Some(98));
        assert_eq!(pad_to_midi(Pad::new(8, 0)), Some(99));
        assert_eq!(pad_to_midi(Pad::new(0, 8)), Some(11));
        assert_eq!(pad_to_midi(Pad::new(7, 8)), Some(18));
        assert_eq!(pad_to_midi(Pad::new(8, 8)), Some(19));
        assert_eq!(pad_to_midi(Pad::new(0, 1)), Some(81));
    }

    #[test]
    fn midi_numbers_round_trip() {
        for pad in Pad::all(SIZE, SIZE) {
            assert_eq!(pad_from_midi(pad_to_midi(pad).unwrap()), Some(pad));
        }
    }

    #[test]
    fn numbers_and_pads_off_the_surface_are_rejected() {
        for number in [0, 9, 10, 20, 90, 100, 127, 255] {
            assert!(pad_from_midi(number).is_none(), "accepted {number}");
        }
        assert!(pad_to_midi(Pad::new(9, 0)).is_none());
    }

    #[test]
    fn only_the_top_right_corner_is_the_logo() {
        assert!(is_logo(Pad::new(8, 0)));
        assert_eq!(Pad::all(SIZE, SIZE).filter(|p| is_logo(*p)).count(), 1);
        assert_eq!(Pad::all(SIZE, SIZE).filter(|p| is_grid(*p)).count(), 64);
    }

    #[test]
    fn note_and_control_change_light_one_pad_each() {
        assert_eq!(
            decode(ID, &[0x90, 11, 5]),
            vec![HostMessage::Lighting {
                pad: Pad::new(0, 8),
                lighting: Lighting::Static(RED),
            }]
        );
        assert_eq!(
            decode(ID, &[0xB0, 91, 5]),
            vec![HostMessage::Lighting {
                pad: Pad::new(0, 0),
                lighting: Lighting::Static(RED),
            }]
        );
    }

    #[test]
    fn channels_two_and_three_flash_and_pulse() {
        assert_eq!(
            decode(ID, &[0x91, 11, 5]),
            vec![HostMessage::Lighting {
                pad: Pad::new(0, 8),
                lighting: Lighting::Flashing {
                    a: RED,
                    b: Rgb::BLACK
                },
            }]
        );
        assert_eq!(
            decode(ID, &[0x92, 11, 5]),
            vec![HostMessage::Lighting {
                pad: Pad::new(0, 8),
                lighting: Lighting::Pulsing(RED),
            }]
        );
    }

    /// The worked example from the Programmer's Reference Manual.
    #[test]
    fn manual_lighting_example_decodes() {
        let decoded = decode(ID, &sysex(&[CMD_LED, 0, 11, 13, 1, 12, 21, 23, 2, 13, 37]));
        assert_eq!(
            decoded,
            vec![
                HostMessage::Lighting {
                    pad: Pad::new(0, 8),
                    lighting: Lighting::Static(palette::get(13).unwrap()),
                },
                HostMessage::Lighting {
                    pad: Pad::new(1, 8),
                    lighting: Lighting::Flashing {
                        a: palette::get(23).unwrap(),
                        b: palette::get(21).unwrap(),
                    },
                },
                HostMessage::Lighting {
                    pad: Pad::new(2, 8),
                    lighting: Lighting::Pulsing(palette::get(37).unwrap()),
                },
            ]
        );
    }

    #[test]
    fn rgb_lighting_spec_widens_to_eight_bits() {
        assert_eq!(
            decode(ID, &sysex(&[CMD_LED, 3, 11, 127, 0, 0])),
            vec![HostMessage::Lighting {
                pad: Pad::new(0, 8),
                lighting: Lighting::Static(RED),
            }]
        );
    }

    #[test]
    fn a_lighting_sysex_can_address_the_whole_surface() {
        let mut body = vec![CMD_LED];
        for pad in Pad::all(SIZE, SIZE) {
            body.extend_from_slice(&[0, pad_to_midi(pad).unwrap(), 5]);
        }
        assert_eq!(decode(ID, &sysex(&body)).len(), 81);
    }

    #[test]
    fn brightness_sleep_and_mode_decode() {
        assert_eq!(
            decode(ID, &sysex(&[CMD_BRIGHTNESS, 64])),
            vec![HostMessage::Brightness(64)]
        );
        assert_eq!(
            decode(ID, &sysex(&[CMD_SLEEP, 0])),
            vec![HostMessage::Sleep(true)]
        );
        assert_eq!(
            decode(ID, &sysex(&[CMD_SLEEP, 1])),
            vec![HostMessage::Sleep(false)]
        );
        assert_eq!(
            decode(ID, &sysex(&[CMD_MODE, 1])),
            vec![HostMessage::ProgrammerMode(true)]
        );
    }

    #[test]
    fn text_scrolls_start_and_stop() {
        assert_eq!(
            decode(ID, &sysex(&[CMD_TEXT])),
            vec![HostMessage::StopScroll]
        );
        assert_eq!(
            decode(ID, &sysex(&[CMD_TEXT, 1, 10, 0, 5, b'H', b'i'])),
            vec![HostMessage::StartScroll(TextScroll {
                looping: true,
                speed: 10,
                color: RED,
                text: b"Hi".to_vec(),
            })]
        );
    }

    #[test]
    fn high_scroll_speeds_are_negative() {
        assert_eq!(decode_speed(0x00), 0);
        assert_eq!(decode_speed(0x10), 16);
        assert_eq!(decode_speed(0x40), -64);
        assert_eq!(decode_speed(0x7F), -1);
    }

    #[test]
    fn another_devices_sysex_is_not_accepted() {
        let mut bytes = sysex_header(0x0D).to_vec();
        bytes.extend_from_slice(&[CMD_BRIGHTNESS, 64, SYSEX_END]);
        assert!(matches!(
            decode(ID, &bytes).as_slice(),
            [HostMessage::Unrecognised(_)]
        ));
    }

    #[test]
    fn unknown_input_is_reported_rather_than_dropped() {
        for bytes in [vec![0x90, 0x00, 0x00], vec![0xFF], vec![]] {
            assert_eq!(
                decode(ID, &bytes),
                vec![HostMessage::Unrecognised(bytes.clone())],
                "for {bytes:02X?}"
            );
        }
    }

    #[test]
    fn a_truncated_lighting_spec_does_not_partially_apply() {
        assert!(matches!(
            decode(ID, &sysex(&[CMD_LED, 0, 11])).as_slice(),
            [HostMessage::Unrecognised(_)]
        ));
    }

    #[test]
    fn grid_presses_are_notes_and_edges_are_control_changes() {
        assert_eq!(
            encode(Interaction::Press {
                pad: Pad::new(0, 8),
                velocity: 100
            }),
            vec![0x90, 11, 100]
        );
        assert_eq!(
            encode(Interaction::Press {
                pad: Pad::new(0, 0),
                velocity: 127
            }),
            vec![0xB0, 91, 127]
        );
        assert_eq!(
            encode(Interaction::Release {
                pad: Pad::new(0, 8)
            }),
            vec![0x90, 11, 0]
        );
        assert_eq!(
            encode(Interaction::Aftertouch {
                pad: Pad::new(0, 8),
                pressure: 64
            }),
            vec![0xA0, 11, 64]
        );
    }

    #[test]
    fn a_press_never_encodes_as_a_release() {
        let bytes = encode(Interaction::Press {
            pad: Pad::new(0, 8),
            velocity: 0,
        });
        assert_eq!(bytes[2], 1);
    }

    #[test]
    fn encoded_interactions_decode_back() {
        for pad in Pad::all(SIZE, SIZE) {
            let press = Interaction::Press { pad, velocity: 127 };
            assert_eq!(decode_interaction(&encode(press)), Some(press));
            let release = Interaction::Release { pad };
            assert_eq!(decode_interaction(&encode(release)), Some(release));
        }
    }
}
