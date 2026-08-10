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
    use crate::DeviceSpec;
    use crate::message::HostMessage;

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
