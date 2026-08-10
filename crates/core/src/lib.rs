//! Novation Launchpad emulators that present themselves as MIDI devices.
//!
//! An [`Emulator`] creates a pair of virtual MIDI ports. A host application connects to those
//! instead of the hardware, and everything it sends to light the surface is decoded into a
//! [`Surface`] that a front end can draw. Interactions go the other way: [`Emulator::send`] reports
//! a press to the host as though a pad had been struck.
//!
//! Calling [`Emulator::attach_hardware`] mirrors both directions, so a real Launchpad and the drawn
//! surface stay in step.
//!
//! Device differences live behind [`DeviceSpec`], so the emulator, the surface and any front end are
//! written once. [`devices`] holds the implementations.
//!
//! ```no_run
//! use launchpad_emulator::{devices::LaunchpadX, Emulator, Interaction, Pad};
//!
//! let mut emulator = Emulator::<LaunchpadX>::with_default_name()?;
//! emulator.attach_hardware()?;
//!
//! for message in emulator.poll() {
//!     println!("{message:?}");
//! }
//!
//! emulator.send(Interaction::Press {
//!     pad: Pad::new(0, 8),
//!     velocity: 100,
//! })?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! Virtual MIDI ports are unavailable on Windows, so this crate does not build there.

mod clock;
mod color;
pub mod devices;
mod emulator;
mod message;
mod pad;
mod role;
mod surface;

pub use clock::{Clock, DEFAULT_BPM, TICKS_PER_BEAT};
pub use color::Rgb;
pub use emulator::Emulator;
pub use message::{HostMessage, Interaction, Query};
pub use pad::Pad;
pub use role::PadRole;
pub use surface::{Lighting, MAX_BRIGHTNESS, Settings, Surface, TextScroll};

/// The behaviour that differs between Launchpad models, implemented by a marker per device.
pub trait DeviceSpec {
    /// Human readable model name, also used as the default virtual port name.
    const NAME: &'static str;
    /// Surface width in pads.
    const WIDTH: u8;
    /// Surface height in pads.
    const HEIGHT: u8;
    /// Substring identifying the MIDI ports of this model's hardware.
    const HARDWARE_KEYWORD: &'static str;
    /// Name to create virtual ports under.
    ///
    /// Contains [`Self::HARDWARE_KEYWORD`], so a host that discovers hardware by name finds the
    /// emulator too. With real hardware also attached the match is ambiguous.
    const PORT_NAME: &'static str;
    /// Whether the pads report how hard they were struck.
    const VELOCITY_SENSITIVE: bool;
    /// Family code this model reports in a device inquiry response.
    const FAMILY_CODE: u8;
    /// Firmware version this emulator reports for itself.
    const FIRMWARE_VERSION: [u8; 4];

    /// Converts a note or control change number into a pad.
    fn pad_from_midi(number: u8) -> Option<Pad>;

    /// The note or control change number addressing a pad, or `None` when it lies off the surface.
    fn pad_to_midi(pad: Pad) -> Option<u8>;

    /// What kind of control occupies a position.
    fn role(pad: Pad) -> PadRole;

    /// Whether a pad can be pressed, as opposed to only lit.
    fn is_button(pad: Pad) -> bool {
        Self::role(pad).is_button()
    }

    /// Resolves a palette entry to a colour.
    fn palette(entry: u8) -> Option<Rgb>;

    /// Decodes one MIDI message from the host into the host messages it carries.
    fn decode(bytes: &[u8]) -> Vec<HostMessage>;

    /// Reads an interaction reported by this model's hardware.
    fn decode_interaction(bytes: &[u8]) -> Option<Interaction>;

    /// Encodes an interaction as this model's hardware would report it.
    fn encode(interaction: Interaction) -> Vec<u8>;

    /// Builds the bytes this model's hardware would send in answer to a message from the host.
    ///
    /// Returns `None` for messages the hardware does not answer.
    fn encode_reply(message: &HostMessage, surface: &Surface) -> Option<Vec<u8>>;
}

/// Errors from setting up or driving an emulator.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A MIDI backend could not be initialised.
    #[error("could not initialise MIDI: {0}")]
    Init(#[from] midir::InitError),
    /// A virtual port could not be created.
    #[error("could not create the virtual MIDI port {name:?}")]
    VirtualPort {
        /// Name the port was requested under.
        name: String,
    },
    /// Sending MIDI failed.
    #[error("could not send MIDI: {0}")]
    Send(#[from] midir::SendError),
    /// No hardware matching the device's port name was found.
    #[error("no MIDI port matching {keyword:?} was found")]
    HardwareNotFound {
        /// Substring that was searched for.
        keyword: &'static str,
    },
    /// Connecting to the hardware failed.
    #[error("could not connect to the hardware: {0}")]
    Connect(String),
    /// A pad outside the device's surface was addressed.
    #[error("pad ({}, {}) is off the surface", pad.x, pad.y)]
    OffSurface {
        /// The pad that was addressed.
        pad: Pad,
    },
    /// A thread holding one of the emulator's locks panicked.
    #[error("an emulator lock was poisoned")]
    Poisoned,
}
