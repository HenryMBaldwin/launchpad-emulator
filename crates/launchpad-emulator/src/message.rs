//! Device-independent messages crossing between a host application and a surface.

use crate::pad::Pad;
use crate::surface::{Lighting, TextScroll};

/// A message sent from a host application to a device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostMessage {
    /// Light a single pad.
    Lighting {
        /// The pad being lit.
        pad: Pad,
        /// How it should be lit.
        lighting: Lighting,
    },
    /// Set the overall LED brightness.
    Brightness(u8),
    /// Switch the LEDs off or back on.
    Sleep(bool),
    /// Enter Programmer mode, or leave it for Live mode.
    ProgrammerMode(bool),
    /// Begin a text scroll, or reconfigure the running one.
    StartScroll(TextScroll),
    /// End any running text scroll.
    StopScroll,
    /// Change the velocity curve and the fixed velocity that goes with it.
    SetVelocityCurve {
        /// Which curve to use.
        curve: u8,
        /// Velocity reported while the curve is the fixed one.
        fixed_velocity: u8,
    },
    /// Change how held pads report pressure.
    SetAftertouch {
        /// Report per pad or once for the whole grid.
        mode: u8,
        /// Pressure needed before reporting starts.
        threshold: u8,
    },
    /// Something the host wants reported back.
    Query(Query),
    /// A MIDI beat clock tick, 24 per beat.
    Clock,
    /// Bytes that did not parse as anything the surface reacts to.
    Unrecognised(Vec<u8>),
}

/// A request for the device to report something about itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Query {
    /// Identity and firmware version, asked with a universal device inquiry.
    DeviceInquiry,
    /// Which layout is selected.
    Layout,
    /// The velocity curve and fixed velocity.
    VelocityCurve,
    /// The aftertouch mode and threshold.
    Aftertouch,
    /// The overall LED brightness.
    Brightness,
    /// Whether the LEDs are switched off.
    Sleep,
}

/// An interaction reported from a device back to the host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Interaction {
    /// A pad was struck, with a velocity in `1..=127`.
    Press {
        /// The pad that was struck.
        pad: Pad,
        /// How hard it was struck.
        velocity: u8,
    },
    /// A pad was let go.
    Release {
        /// The pad that was let go.
        pad: Pad,
    },
    /// The pressure on a held pad changed, from 0 to 127.
    Aftertouch {
        /// The pad being held.
        pad: Pad,
        /// The pressure applied.
        pressure: u8,
    },
}

impl Interaction {
    /// The pad this interaction refers to.
    #[must_use]
    pub const fn pad(self) -> Pad {
        match self {
            Self::Press { pad, .. } | Self::Release { pad } | Self::Aftertouch { pad, .. } => pad,
        }
    }
}
