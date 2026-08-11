//! The lighting state a front end draws.

use crate::message::HostMessage;
use crate::pad::Pad;
use crate::{DeviceSpec, Rgb, font};

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

/// Dimmest a pulsing colour goes, as the manual's waveform shows.
const PULSE_FLOOR: f32 = 0.25;

/// Fraction of a pulse period spent rising, half a beat of the two.
const PULSE_RISE: f32 = 0.25;

/// Beats the pulse is shifted by to peak where the hardware's does.
const PULSE_OFFSET: f32 = 1.0;

impl Lighting {
    /// The colour to draw at `beats` elapsed.
    ///
    /// Flashing has a period of one beat and pulsing two, matching the hardware.
    #[must_use]
    pub fn color_at(self, beats: f32) -> Rgb {
        match self {
            Self::Static(color) => color,
            Self::Flashing { a, b } => {
                if beats.rem_euclid(1.0) < 0.5 {
                    a
                } else {
                    b
                }
            }
            Self::Pulsing(color) => color.scaled(pulse_level(beats)),
        }
    }
}

/// Brightness of a pulse at `beats` elapsed, rising quickly then falling away.
fn pulse_level(beats: f32) -> f32 {
    let phase = ((beats + PULSE_OFFSET) * 0.5).rem_euclid(1.0);
    let climb = if phase < PULSE_RISE {
        phase / PULSE_RISE
    } else {
        1.0 - (phase - PULSE_RISE) / (1.0 - PULSE_RISE)
    };
    PULSE_FLOOR + (1.0 - PULSE_FLOOR) * climb
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
#[derive(Debug, Clone, PartialEq)]
pub struct Surface {
    width: u8,
    height: u8,
    leds: Vec<Lighting>,
    brightness: u8,
    asleep: bool,
    programmer_mode: bool,
    scroll: Option<TextScroll>,
    /// Pixels of the running scroll, one byte per column.
    scroll_columns: Vec<u8>,
    /// Columns the scroll has travelled, counting from off the surface.
    scroll_offset: f32,
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
            scroll_columns: Vec::new(),
            scroll_offset: 0.0,
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

    /// The colour to draw for a pad at `beats` elapsed.
    ///
    /// Returns black while the surface is asleep, and scales everything else by the brightness.
    #[must_use]
    pub fn color_at(&self, pad: Pad, beats: f32) -> Rgb {
        if self.asleep {
            return Rgb::BLACK;
        }
        let level = f32::from(self.brightness) / f32::from(MAX_BRIGHTNESS);
        // A scroll covers what is underneath until it stops or finishes
        let color = match self.scroll_color(pad) {
            Some(color) => color,
            None => self.lighting(pad).color_at(beats),
        };
        color.scaled(level)
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
            HostMessage::StartScroll(scroll) => self.start_scroll(scroll.clone()),
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

    /// Begins a scroll, placing the text just off the edge it enters from.
    #[allow(clippy::cast_precision_loss)]
    fn start_scroll(&mut self, scroll: TextScroll) {
        self.scroll_columns = font::columns(&scroll.text);
        self.scroll_offset = if scroll.speed < 0 {
            self.scroll_columns.len() as f32
        } else {
            -f32::from(self.width)
        };
        self.scroll = Some(scroll);
    }

    /// Moves a running scroll on by `seconds`, ending or looping it once the text has left.
    ///
    /// Speed is in pads per second, negative when the text travels left to right.
    #[allow(clippy::cast_precision_loss)]
    pub fn advance(&mut self, seconds: f32) {
        let Some(scroll) = self.scroll.as_ref() else {
            return;
        };
        let travel = f32::from(scroll.speed) * seconds;
        let text = self.scroll_columns.len() as f32;
        let start = -f32::from(self.width);
        self.scroll_offset += travel;

        let gone = if scroll.speed < 0 {
            self.scroll_offset <= start
        } else {
            self.scroll_offset >= text
        };
        if gone {
            if scroll.looping {
                self.scroll_offset = if scroll.speed < 0 { text } else { start };
            } else {
                self.scroll = None;
                self.scroll_columns.clear();
            }
        }
    }

    /// The colour the running scroll paints a pad, if it covers it.
    ///
    /// The top row and anything past the grid are left alone, as on the hardware.
    #[allow(clippy::cast_possible_truncation)]
    fn scroll_color(&self, pad: Pad) -> Option<Rgb> {
        let scroll = self.scroll.as_ref()?;
        if pad.y == 0 || pad.y > font::HEIGHT as u8 || pad.x >= self.width {
            return None;
        }
        let column = self.scroll_offset.floor() + f32::from(pad.x);
        let lit = usize::try_from(column as i64)
            .ok()
            .and_then(|index| self.scroll_columns.get(index))
            .is_some_and(|bits| (bits >> (pad.y - 1)) & 1 == 1);
        Some(if lit { scroll.color } else { Rgb::BLACK })
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

    fn scrolling(text: &[u8], speed: i8, looping: bool) -> Surface {
        let mut surface = Surface::new::<LaunchpadX>();
        surface.apply(&HostMessage::StartScroll(TextScroll {
            looping,
            speed,
            color: RED,
            text: text.to_vec(),
        }));
        surface
    }

    /// The lit columns of the scroll region, as the text stands now
    fn lit_columns(surface: &Surface) -> Vec<u8> {
        (0..surface.width())
            .filter(|x| (1..9).any(|y| surface.color_at(Pad::new(*x, y), 0.0) != Rgb::BLACK))
            .collect()
    }

    #[test]
    fn a_scroll_enters_from_the_right_and_leaves_at_the_left() {
        let mut surface = scrolling(b"A", 9, false);
        assert!(lit_columns(&surface).is_empty(), "starts off the surface");
        // Two pads' worth of travel, so only the leading columns have arrived
        surface.advance(0.2);
        let entered = lit_columns(&surface);
        assert!(!entered.is_empty(), "the text has started to appear");
        assert!(
            entered.iter().all(|x| *x >= 6),
            "it enters at the right edge, got {entered:?}"
        );
    }

    #[test]
    fn a_scroll_covers_the_grid_but_not_the_top_row_or_beyond() {
        let mut surface = scrolling(b"HELLO", 9, true);
        surface.apply(&HostMessage::Lighting {
            pad: Pad::new(0, 0),
            lighting: Lighting::Static(RED),
        });
        surface.advance(0.5);
        assert_eq!(
            surface.color_at(Pad::new(0, 0), 0.0),
            RED,
            "the top row keeps its own colour"
        );
    }

    #[test]
    fn a_scroll_that_does_not_loop_ends_and_gives_the_leds_back() {
        let mut surface = scrolling(b"A", 9, false);
        surface.apply(&HostMessage::Lighting {
            pad: Pad::new(0, 4),
            lighting: Lighting::Static(RED),
        });
        surface.advance(10.0);
        assert!(
            surface.scroll().is_none(),
            "the scroll should have finished"
        );
        assert_eq!(
            surface.color_at(Pad::new(0, 4), 0.0),
            RED,
            "lighting covered by the scroll returns"
        );
    }

    #[test]
    fn a_looping_scroll_keeps_going() {
        let mut surface = scrolling(b"A", 9, true);
        surface.advance(60.0);
        assert!(surface.scroll().is_some());
    }

    #[test]
    fn a_negative_speed_scrolls_the_other_way() {
        let mut surface = scrolling(b"A", -9, false);
        surface.advance(0.2);
        let entered = lit_columns(&surface);
        assert!(!entered.is_empty());
        assert!(
            entered.iter().all(|x| *x <= 2),
            "it enters at the left edge, got {entered:?}"
        );
    }

    #[test]
    fn stopping_a_scroll_reveals_what_was_under_it() {
        let mut surface = scrolling(b"HELLO", 9, true);
        surface.apply(&HostMessage::Lighting {
            pad: Pad::new(3, 4),
            lighting: Lighting::Static(RED),
        });
        surface.advance(0.5);
        surface.apply(&HostMessage::StopScroll);
        assert_eq!(surface.color_at(Pad::new(3, 4), 0.0), RED);
    }

    #[test]
    fn flashing_alternates_once_a_beat() {
        let flashing = Lighting::Flashing {
            a: RED,
            b: Rgb::BLACK,
        };
        assert_eq!(flashing.color_at(0.0), RED);
        assert_eq!(flashing.color_at(0.75), Rgb::BLACK);
        assert_eq!(flashing.color_at(1.25), RED, "the period is one beat");
    }

    /// The manual's waveform: two beats long, from a quarter brightness up to full
    #[test]
    fn pulsing_spans_two_beats_and_never_goes_dark() {
        assert!(
            (pulse_level(1.0) - PULSE_FLOOR).abs() < 0.001,
            "dimmest on the beat"
        );
        assert!(
            (pulse_level(1.5) - 1.0).abs() < 0.001,
            "peaks half a beat later"
        );
        assert!(
            (pulse_level(3.0) - PULSE_FLOOR).abs() < 0.001,
            "and again two beats on"
        );
        assert!(
            (pulse_level(3.5) - 1.0).abs() < 0.001,
            "repeating every two beats"
        );
        for beats in [0.0, 0.3, 0.7, 1.1, 1.9, 2.4, 3.8] {
            let level = pulse_level(beats);
            assert!(
                (PULSE_FLOOR..=1.0).contains(&level),
                "level {level} at {beats} beats"
            );
        }
        assert_eq!(Lighting::Pulsing(RED).color_at(1.5), RED);
        assert_ne!(Lighting::Pulsing(RED).color_at(1.0), Rgb::BLACK);
    }
}
