//! The devices the emulator can present itself as.
//!
//! One module per model, each holding a marker type implementing [`DeviceSpec`](crate::DeviceSpec).
//! Models that share a protocol delegate to a `_family` module rather than repeating it.

pub mod mini_mk3;
pub mod mk3_family;
pub mod palette;
pub mod x;

pub use mini_mk3::LaunchpadMiniMk3;
pub use x::LaunchpadX;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::HostMessage;
    use crate::{DeviceSpec, Pad};

    /// A brightness message addressed to a given device ID.
    fn brightness(device_id: u8) -> Vec<u8> {
        let mut bytes = mk3_family::sysex_header(device_id).to_vec();
        bytes.extend_from_slice(&[0x08, 64, 0xF7]);
        bytes
    }

    #[test]
    fn the_two_devices_share_a_surface() {
        assert_eq!(LaunchpadX::WIDTH, LaunchpadMiniMk3::WIDTH);
        assert_eq!(LaunchpadX::HEIGHT, LaunchpadMiniMk3::HEIGHT);
        assert_ne!(
            LaunchpadX::HARDWARE_KEYWORD,
            LaunchpadMiniMk3::HARDWARE_KEYWORD
        );
    }

    #[test]
    fn both_devices_name_every_button_but_no_grid_pad() {
        for (top_left, sixth) in [
            (
                LaunchpadX::printed_name(Pad::new(0, 0)),
                LaunchpadX::printed_name(Pad::new(5, 0)),
            ),
            (
                LaunchpadMiniMk3::printed_name(Pad::new(0, 0)),
                LaunchpadMiniMk3::printed_name(Pad::new(5, 0)),
            ),
        ] {
            assert_eq!(top_left, Some("Up"));
            assert!(sixth.is_some(), "the sixth top button is named");
        }
        // The models differ where their printing differs
        assert_eq!(LaunchpadX::printed_name(Pad::new(5, 0)), Some("Note"));
        assert_eq!(
            LaunchpadMiniMk3::printed_name(Pad::new(5, 0)),
            Some("Drums")
        );
        assert_eq!(LaunchpadX::printed_name(Pad::new(3, 4)), None);
        assert_eq!(LaunchpadX::printed_name(Pad::new(8, 0)), Some("Logo"));
    }

    #[test]
    fn each_device_only_accepts_its_own_sysex() {
        assert_eq!(
            LaunchpadX::decode(&brightness(x::DEVICE_ID)),
            vec![HostMessage::Brightness(64)]
        );
        assert_eq!(
            LaunchpadMiniMk3::decode(&brightness(mini_mk3::DEVICE_ID)),
            vec![HostMessage::Brightness(64)]
        );
        assert!(matches!(
            LaunchpadX::decode(&brightness(mini_mk3::DEVICE_ID)).as_slice(),
            [HostMessage::Unrecognised(_)]
        ));
        assert!(matches!(
            LaunchpadMiniMk3::decode(&brightness(x::DEVICE_ID)).as_slice(),
            [HostMessage::Unrecognised(_)]
        ));
    }
}
