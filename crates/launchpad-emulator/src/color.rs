//! Colour representation shared by the palette and the surface.

/// An 8-bit RGB colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Rgb {
    /// Red channel.
    pub r: u8,
    /// Green channel.
    pub g: u8,
    /// Blue channel.
    pub b: u8,
}

impl Rgb {
    /// An unlit LED.
    pub const BLACK: Self = Self { r: 0, g: 0, b: 0 };

    /// Reads the three 7-bit channels a Launchpad X RGB message carries.
    ///
    /// Channel values run from 0 to 127 and are scaled up to the full 8-bit range.
    #[must_use]
    pub fn from_midi(channels: &[u8]) -> Option<Self> {
        let &[r, g, b] = channels else {
            return None;
        };
        if r > 127 || g > 127 || b > 127 {
            return None;
        }
        Some(Self {
            r: scale_up(r),
            g: scale_up(g),
            b: scale_up(b),
        })
    }

    /// The three 7-bit channels a Launchpad X RGB message carries.
    #[must_use]
    pub const fn to_midi(self) -> [u8; 3] {
        [narrow(self.r), narrow(self.g), narrow(self.b)]
    }

    /// This colour with every channel multiplied by `level`, which is clamped to `0.0..=1.0`.
    #[must_use]
    pub fn scaled(self, level: f32) -> Self {
        let level = level.clamp(0.0, 1.0);
        Self {
            r: scale(self.r, level),
            g: scale(self.g, level),
            b: scale(self.b, level),
        }
    }
}

/// Widens a 7-bit channel so that 127 maps to 255, exactly reversing [`narrow`].
const fn scale_up(value: u8) -> u8 {
    (value << 1) | (value >> 6)
}

/// Narrows an 8-bit channel so that 255 maps to 127.
const fn narrow(value: u8) -> u8 {
    value >> 1
}

/// Multiplies a channel by a level already clamped to `0.0..=1.0`.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn scale(value: u8, level: f32) -> u8 {
    (f32::from(value) * level).round() as u8
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn seven_bit_channels_widen_to_the_full_range() {
        assert_eq!(
            Rgb::from_midi(&[127, 0, 0]),
            Some(Rgb { r: 255, g: 0, b: 0 })
        );
        assert_eq!(Rgb::from_midi(&[0, 0, 0]), Some(Rgb::BLACK));
    }

    #[test]
    fn channels_survive_a_round_trip_to_the_wire() {
        for channel in [0u8, 1, 63, 64, 126, 127] {
            let color = Rgb::from_midi(&[channel, channel, channel]).unwrap_or(Rgb::BLACK);
            assert_eq!(
                color.to_midi(),
                [channel, channel, channel],
                "for {channel}"
            );
        }
    }

    #[test]
    fn out_of_range_channels_are_rejected() {
        assert!(Rgb::from_midi(&[128, 0, 0]).is_none());
        assert!(Rgb::from_midi(&[0, 0]).is_none());
        assert!(Rgb::from_midi(&[0, 0, 0, 0]).is_none());
    }

    #[test]
    fn scaling_is_clamped_at_both_ends() {
        let white = Rgb {
            r: 255,
            g: 255,
            b: 255,
        };
        assert_eq!(white.scaled(0.0), Rgb::BLACK);
        assert_eq!(white.scaled(1.0), white);
        assert_eq!(white.scaled(2.0), white);
        assert_eq!(white.scaled(-1.0), Rgb::BLACK);
    }
}
