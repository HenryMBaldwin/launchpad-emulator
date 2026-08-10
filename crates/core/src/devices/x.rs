//! The Novation Launchpad X.

use super::{mk3_family as family, palette};
use crate::message::{HostMessage, Interaction};
use crate::pad::Pad;
use crate::{DeviceSpec, PadRole, Rgb};

/// Device ID this model answers to in `SysEx` messages.
pub const DEVICE_ID: u8 = 0x0C;

/// A Novation Launchpad X.
///
/// The central 8x8 grid is velocity and pressure sensitive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LaunchpadX;

impl DeviceSpec for LaunchpadX {
    const NAME: &'static str = "Launchpad X";
    const WIDTH: u8 = family::SIZE;
    const HEIGHT: u8 = family::SIZE;
    const HARDWARE_KEYWORD: &'static str = "Launchpad X LPX MI";
    const VELOCITY_SENSITIVE: bool = true;

    fn pad_from_midi(number: u8) -> Option<Pad> {
        family::pad_from_midi(number)
    }

    fn pad_to_midi(pad: Pad) -> Option<u8> {
        family::pad_to_midi(pad)
    }

    fn role(pad: Pad) -> PadRole {
        family::role(pad)
    }

    fn palette(entry: u8) -> Option<Rgb> {
        palette::get(entry)
    }

    fn decode(bytes: &[u8]) -> Vec<HostMessage> {
        family::decode(DEVICE_ID, bytes)
    }

    fn decode_interaction(bytes: &[u8]) -> Option<Interaction> {
        family::decode_interaction(bytes)
    }

    fn encode(interaction: Interaction) -> Vec<u8> {
        family::encode(interaction)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_logo_lights_but_is_not_a_button() {
        assert!(!LaunchpadX::is_button(Pad::new(8, 0)));
        assert!(LaunchpadX::is_button(Pad::new(0, 0)));
        assert!(LaunchpadX::pad_to_midi(Pad::new(8, 0)).is_some());
    }

    #[test]
    fn the_grid_reports_how_hard_it_was_struck() {
        const { assert!(LaunchpadX::VELOCITY_SENSITIVE) };
    }
}
