//! The lighting state a front end draws.

use crate::message::HostMessage;
use crate::pad::Pad;
use crate::{DeviceSpec, Rgb};

/// Full brightness, as every supported device reports it.
pub const MAX_BRIGHTNESS: u8 = 127;

/// How a single LED is lit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lighting {
    /// A constant colour.
    Static(Rgb),
    /// Alternating between two colours at 50% duty cycle, one period per beat.
    Flashing {
        /// The colour held for the first half of the period.
        a: Rgb,
        /// The colour held for the second half of the period.
        b: Rgb,
    },
    /// Ramping between a colour and unlit, one period per beat.
    Pulsing(Rgb),
}

impl Lighting {
    /// The colour to draw at `phase` through the current beat, in `0.0..1.0`.
    #[must_use]
    pub fn color_at(self, phase: f32) -> Rgb {
        match self {
            Self::Static(color) => color,
            Self::Flashing { a, b } => {
                if phase < 0.5 {
                    a
                } else {
                    b
                }
            }
            Self::Pulsing(color) => color.scaled(1.0 - (phase * 2.0 - 1.0).abs()),
        }
    }
}

impl Default for Lighting {
    fn default() -> Self {
        Self::Static(Rgb::BLACK)
    }
}

/// Settings a host can change and read back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Settings {
    /// Which curve maps striking force onto velocity.
    pub velocity_curve: u8,
    /// Velocity reported for every pad while the curve is the fixed one.
    pub fixed_velocity: u8,
    /// Whether held pads report pressure per pad or once for the whole grid.
    pub aftertouch_mode: u8,
    /// Pressure needed before a held pad starts reporting.
    pub aftertouch_threshold: u8,
}

impl Default for Settings {
    /// The values the hardware powers on with.
    fn default() -> Self {
        Self {
            velocity_curve: 1,
            fixed_velocity: 127,
            aftertouch_mode: 0,
            aftertouch_threshold: 1,
        }
    }
}

/// A text scroll running across the surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextScroll {
    /// Whether the text restarts once it has scrolled out.
    pub looping: bool,
    /// Speed in pads per second, negative when the text scrolls left to right.
    pub speed: i8,
    /// Colour the text is drawn in.
    pub color: Rgb,
    /// The bytes to display.
    pub text: Vec<u8>,
}

/// The lighting state of a whole surface, sized for the device that produced it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Surface {
    width: u8,
    height: u8,
    leds: Vec<Lighting>,
    brightness: u8,
    asleep: bool,
    programmer_mode: bool,
    scroll: Option<TextScroll>,
    settings: Settings,
}

impl Surface {
    /// Creates an unlit surface for a device.
    #[must_use]
    pub fn new<S: DeviceSpec>() -> Self {
        Self::with_size(S::WIDTH, S::HEIGHT)
    }

    /// Creates an unlit surface of the given size.
    #[must_use]
    pub fn with_size(width: u8, height: u8) -> Self {
        Self {
            width,
            height,
            leds: vec![Lighting::default(); usize::from(width) * usize::from(height)],
            brightness: MAX_BRIGHTNESS,
            asleep: false,
            programmer_mode: false,
            scroll: None,
            settings: Settings::default(),
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

    /// Every pad of this surface, in row-major order.
    pub fn pads(&self) -> impl Iterator<Item = Pad> + use<> {
        Pad::all(self.width, self.height)
    }

    /// How a pad is currently lit, ignoring brightness and sleep.
    #[must_use]
    pub fn lighting(&self, pad: Pad) -> Lighting {
        self.index(pad)
            .and_then(|i| self.leds.get(i).copied())
            .unwrap_or_default()
    }

    /// The colour to draw for a pad at `phase` through the current beat, in `0.0..1.0`.
    ///
    /// Returns black while the surface is asleep, and scales everything else by the brightness.
    #[must_use]
    pub fn color_at(&self, pad: Pad, phase: f32) -> Rgb {
        if self.asleep {
            return Rgb::BLACK;
        }
        let level = f32::from(self.brightness) / f32::from(MAX_BRIGHTNESS);
        self.lighting(pad).color_at(phase).scaled(level)
    }

    /// Overall LED brightness, from 0 to [`MAX_BRIGHTNESS`].
    #[must_use]
    pub const fn brightness(&self) -> u8 {
        self.brightness
    }

    /// Whether the LEDs have been switched off.
    #[must_use]
    pub const fn is_asleep(&self) -> bool {
        self.asleep
    }

    /// Whether the host has put the surface into Programmer mode.
    #[must_use]
    pub const fn is_programmer_mode(&self) -> bool {
        self.programmer_mode
    }

    /// Settings the host has changed, which it can also read back.
    #[must_use]
    pub const fn settings(&self) -> Settings {
        self.settings
    }

    /// The running text scroll, if any.
    #[must_use]
    pub const fn scroll(&self) -> Option<&TextScroll> {
        self.scroll.as_ref()
    }

    /// Unlights every pad.
    pub fn clear(&mut self) {
        self.leds.fill(Lighting::default());
    }

    /// Applies a message from the host.
    pub fn apply(&mut self, message: &HostMessage) {
        match message {
            HostMessage::Lighting { pad, lighting } => {
                if let Some(i) = self.index(*pad)
                    && let Some(led) = self.leds.get_mut(i)
                {
                    *led = *lighting;
                }
            }
            HostMessage::Brightness(level) => self.brightness = (*level).min(MAX_BRIGHTNESS),
            HostMessage::Sleep(asleep) => self.asleep = *asleep,
            HostMessage::ProgrammerMode(on) => self.programmer_mode = *on,
            HostMessage::StartScroll(scroll) => self.scroll = Some(scroll.clone()),
            HostMessage::StopScroll => self.scroll = None,
            HostMessage::SetVelocityCurve {
                curve,
                fixed_velocity,
            } => {
                self.settings.velocity_curve = *curve;
                self.settings.fixed_velocity = *fixed_velocity;
            }
            HostMessage::SetAftertouch { mode, threshold } => {
                self.settings.aftertouch_mode = *mode;
                self.settings.aftertouch_threshold = *threshold;
            }
            HostMessage::Query(_) | HostMessage::Clock | HostMessage::Unrecognised(_) => {}
        }
    }

    /// Row-major index of a pad, or `None` when it lies off the surface.
    fn index(&self, pad: Pad) -> Option<usize> {
        (pad.x < self.width && pad.y < self.height)
            .then(|| usize::from(pad.y) * usize::from(self.width) + usize::from(pad.x))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::devices::LaunchpadX;

    const RED: Rgb = Rgb { r: 255, g: 0, b: 0 };
    const PAD: Pad = Pad::new(3, 4);

    fn lit() -> Surface {
        let mut surface = Surface::new::<LaunchpadX>();
        surface.apply(&HostMessage::Lighting {
            pad: PAD,
            lighting: Lighting::Static(RED),
        });
        surface
    }

    #[test]
    fn a_new_surface_is_unlit_and_sized_for_its_device() {
        let surface = Surface::new::<LaunchpadX>();
        assert_eq!((surface.width(), surface.height()), (9, 9));
        assert_eq!(surface.pads().count(), 81);
        assert!(
            surface
                .pads()
                .all(|p| surface.color_at(p, 0.0) == Rgb::BLACK)
        );
    }

    #[test]
    fn lighting_a_pad_is_visible() {
        assert_eq!(lit().color_at(PAD, 0.0), RED);
    }

    #[test]
    fn sleep_hides_lit_pads_without_forgetting_them() {
        let mut surface = lit();
        surface.apply(&HostMessage::Sleep(true));
        assert_eq!(surface.color_at(PAD, 0.0), Rgb::BLACK);
        surface.apply(&HostMessage::Sleep(false));
        assert_eq!(surface.color_at(PAD, 0.0), RED);
    }

    #[test]
    fn brightness_scales_the_drawn_colour_and_is_clamped() {
        let mut surface = lit();
        surface.apply(&HostMessage::Brightness(64));
        assert_eq!(surface.color_at(PAD, 0.0).r, 129);
        surface.apply(&HostMessage::Brightness(0));
        assert_eq!(surface.color_at(PAD, 0.0), Rgb::BLACK);
        surface.apply(&HostMessage::Brightness(MAX_BRIGHTNESS));
        assert_eq!(surface.color_at(PAD, 0.0), RED);
        surface.apply(&HostMessage::Brightness(200));
        assert_eq!(surface.brightness(), MAX_BRIGHTNESS);
    }

    #[test]
    fn pads_off_the_surface_are_ignored() {
        let mut surface = Surface::new::<LaunchpadX>();
        surface.apply(&HostMessage::Lighting {
            pad: Pad::new(50, 50),
            lighting: Lighting::Static(RED),
        });
        assert_eq!(surface.lighting(Pad::new(50, 50)), Lighting::default());
    }

    #[test]
    fn flashing_alternates_and_pulsing_peaks_at_mid_beat() {
        let flashing = Lighting::Flashing {
            a: RED,
            b: Rgb::BLACK,
        };
        assert_eq!(flashing.color_at(0.0), RED);
        assert_eq!(flashing.color_at(0.75), Rgb::BLACK);
        assert_eq!(Lighting::Pulsing(RED).color_at(0.0), Rgb::BLACK);
        assert_eq!(Lighting::Pulsing(RED).color_at(0.5), RED);
    }
}
