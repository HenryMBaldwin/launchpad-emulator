//! The Novation Launchpad Mini MK3.

use super::{mk3_family as family, palette};
use crate::message::{HostMessage, Interaction};
use crate::pad::Pad;
use crate::{DeviceSpec, PadRole, Rgb};

/// Device ID this model answers to in `SysEx` messages.
pub const DEVICE_ID: u8 = 0x0D;

/// A Novation Launchpad Mini MK3, whose pads are not velocity sensitive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LaunchpadMiniMk3;

impl DeviceSpec for LaunchpadMiniMk3 {
    const NAME: &'static str = "Launchpad Mini MK3";
    const WIDTH: u8 = family::SIZE;
    const HEIGHT: u8 = family::SIZE;
    const HARDWARE_KEYWORD: &'static str = "Launchpad Mini MK3 LPMiniMK3 MI";
    const PORT_NAME: &'static str = "Launchpad Mini MK3 LPMiniMK3 MIDI";
    const VELOCITY_SENSITIVE: bool = false;

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
    fn the_port_name_is_discoverable_as_this_model() {
        assert!(LaunchpadMiniMk3::PORT_NAME.contains(LaunchpadMiniMk3::HARDWARE_KEYWORD));
    }

    #[test]
    fn the_pads_are_switches() {
        const { assert!(!LaunchpadMiniMk3::VELOCITY_SENSITIVE) };
    }
}
