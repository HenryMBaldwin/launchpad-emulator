//! The devices the emulator can present itself as.

pub mod mk3;
pub mod palette;

use crate::message::{HostMessage, Interaction};
use crate::pad::Pad;
use crate::{DeviceSpec, Rgb};

/// A Novation Launchpad X.
///
/// The central 8x8 grid is velocity and pressure sensitive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LaunchpadX;

/// A Novation Launchpad Mini MK3, whose pads are not velocity sensitive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LaunchpadMiniMk3;

impl DeviceSpec for LaunchpadX {
    const NAME: &'static str = "Launchpad X";
    const WIDTH: u8 = mk3::SIZE;
    const HEIGHT: u8 = mk3::SIZE;
    const HARDWARE_KEYWORD: &'static str = "Launchpad X LPX MI";
    const VELOCITY_SENSITIVE: bool = true;

    fn pad_from_midi(number: u8) -> Option<Pad> {
        mk3::pad_from_midi(number)
    }

    fn pad_to_midi(pad: Pad) -> Option<u8> {
        mk3::pad_to_midi(pad)
    }

    fn is_button(pad: Pad) -> bool {
        !mk3::is_logo(pad) && mk3::pad_to_midi(pad).is_some()
    }

    fn palette(entry: u8) -> Option<Rgb> {
        palette::get(entry)
    }

    fn decode(bytes: &[u8]) -> Vec<HostMessage> {
        mk3::decode(0x0C, bytes)
    }

    fn decode_interaction(bytes: &[u8]) -> Option<Interaction> {
        mk3::decode_interaction(bytes)
    }

    fn encode(interaction: Interaction) -> Vec<u8> {
        mk3::encode(interaction)
    }
}

impl DeviceSpec for LaunchpadMiniMk3 {
    const NAME: &'static str = "Launchpad Mini MK3";
    const WIDTH: u8 = mk3::SIZE;
    const HEIGHT: u8 = mk3::SIZE;
    const HARDWARE_KEYWORD: &'static str = "Launchpad Mini MK3 LPMiniMK3 MI";
    const VELOCITY_SENSITIVE: bool = false;

    fn pad_from_midi(number: u8) -> Option<Pad> {
        mk3::pad_from_midi(number)
    }

    fn pad_to_midi(pad: Pad) -> Option<u8> {
        mk3::pad_to_midi(pad)
    }

    fn is_button(pad: Pad) -> bool {
        !mk3::is_logo(pad) && mk3::pad_to_midi(pad).is_some()
    }

    fn palette(entry: u8) -> Option<Rgb> {
        palette::get(entry)
    }

    fn decode(bytes: &[u8]) -> Vec<HostMessage> {
        mk3::decode(0x0D, bytes)
    }

    fn decode_interaction(bytes: &[u8]) -> Option<Interaction> {
        mk3::decode_interaction(bytes)
    }

    fn encode(interaction: Interaction) -> Vec<u8> {
        mk3::encode(interaction)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_two_devices_differ_only_where_expected() {
        assert_eq!(LaunchpadX::WIDTH, LaunchpadMiniMk3::WIDTH);
        const { assert!(LaunchpadX::VELOCITY_SENSITIVE) };
        const { assert!(!LaunchpadMiniMk3::VELOCITY_SENSITIVE) };
        assert_ne!(
            LaunchpadX::HARDWARE_KEYWORD,
            LaunchpadMiniMk3::HARDWARE_KEYWORD
        );
    }

    #[test]
    fn each_device_only_accepts_its_own_sysex() {
        let brightness = |id: u8| {
            let mut bytes = mk3::sysex_header(id).to_vec();
            bytes.extend_from_slice(&[0x08, 64, 0xF7]);
            bytes
        };
        assert_eq!(
            LaunchpadX::decode(&brightness(0x0C)),
            vec![HostMessage::Brightness(64)]
        );
        assert!(matches!(
            LaunchpadX::decode(&brightness(0x0D)).as_slice(),
            [HostMessage::Unrecognised(_)]
        ));
        assert_eq!(
            LaunchpadMiniMk3::decode(&brightness(0x0D)),
            vec![HostMessage::Brightness(64)]
        );
    }

    #[test]
    fn the_logo_lights_but_is_not_a_button() {
        assert!(!LaunchpadX::is_button(Pad::new(8, 0)));
        assert!(LaunchpadX::is_button(Pad::new(0, 0)));
        assert!(LaunchpadX::pad_to_midi(Pad::new(8, 0)).is_some());
    }
}
