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
    /// A MIDI beat clock tick, 24 per beat.
    Clock,
    /// Bytes that did not parse as anything the surface reacts to.
    Unrecognised(Vec<u8>),
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
