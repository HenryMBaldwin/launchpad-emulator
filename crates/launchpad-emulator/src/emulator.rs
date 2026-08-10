//! The virtual MIDI device a host application connects to.

use std::marker::PhantomData;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use midir::os::unix::{VirtualInput, VirtualOutput};
use midir::{MidiInput, MidiInputConnection, MidiOutput, MidiOutputConnection};

use crate::message::{HostMessage, Interaction};
use crate::{Clock, DeviceSpec, Error, Surface};

/// Hardware connection shared with the callback that mirrors host traffic onto it.
type Hardware = Arc<Mutex<Option<MidiOutputConnection>>>;

/// A Launchpad presented to host applications as a pair of virtual MIDI ports.
///
/// Dropping the emulator removes the ports.
pub struct Emulator<S: DeviceSpec> {
    /// Receives what the host sends; named from the host's point of view.
    _from_host: MidiInputConnection<()>,
    /// Sends what the emulator reports; named from the host's point of view.
    to_host: MidiOutputConnection,
    hardware_out: Hardware,
    hardware_in: Option<MidiInputConnection<()>>,
    host_messages: Receiver<HostMessage>,
    hardware_interactions: Receiver<Interaction>,
    hardware_sink: Sender<Interaction>,
    surface: Arc<Mutex<Surface>>,
    clock: Arc<Mutex<Clock>>,
    device: PhantomData<S>,
}

impl<S: DeviceSpec> std::fmt::Debug for Emulator<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Emulator")
            .field("device", &S::NAME)
            .field("hardware_attached", &self.hardware_attached())
            .finish_non_exhaustive()
    }
}

impl<S: DeviceSpec> Emulator<S> {
    /// Creates the virtual ports a host application connects to.
    ///
    /// `port_name` appears in the host's MIDI port list. A name containing
    /// [`DeviceSpec::HARDWARE_KEYWORD`] is discoverable by libraries that find hardware by name.
    ///
    /// # Errors
    ///
    /// Fails if a MIDI backend cannot be initialised or a virtual port cannot be created.
    pub fn new(port_name: &str) -> Result<Self, Error> {
        let surface = Arc::new(Mutex::new(Surface::new::<S>()));
        let clock = Arc::new(Mutex::new(Clock::new(Instant::now())));
        let hardware_out: Hardware = Arc::new(Mutex::new(None));
        let (message_sink, host_messages) = channel();
        let (hardware_sink, hardware_interactions) = channel();

        let shared_surface = Arc::clone(&surface);
        let shared_clock = Arc::clone(&clock);
        let shared_hardware = Arc::clone(&hardware_out);
        let from_host = MidiInput::new(port_name)?
            .create_virtual(
                port_name,
                move |_timestamp, bytes, ()| {
                    if let Ok(mut hardware) = shared_hardware.lock()
                        && let Some(output) = hardware.as_mut()
                    {
                        let _ = output.send(bytes);
                    }
                    for message in S::decode(bytes) {
                        if let Ok(mut surface) = shared_surface.lock() {
                            surface.apply(&message);
                        }
                        if matches!(message, HostMessage::Clock)
                            && let Ok(mut clock) = shared_clock.lock()
                        {
                            clock.tick(Instant::now());
                        }
                        // A closed receiver only means nothing is reading the log
                        let _ = message_sink.send(message);
                    }
                },
                (),
            )
            .map_err(|_| Error::VirtualPort {
                name: port_name.into(),
            })?;

        let to_host = MidiOutput::new(port_name)?
            .create_virtual(port_name)
            .map_err(|_| Error::VirtualPort {
                name: port_name.into(),
            })?;

        Ok(Self {
            _from_host: from_host,
            to_host,
            hardware_out,
            hardware_in: None,
            host_messages,
            hardware_interactions,
            hardware_sink,
            surface,
            clock,
            device: PhantomData,
        })
    }

    /// Creates the virtual ports under the device's own name.
    ///
    /// # Errors
    ///
    /// Fails if a MIDI backend cannot be initialised or a virtual port cannot be created.
    pub fn with_default_name() -> Result<Self, Error> {
        Self::new(S::NAME)
    }

    /// The lighting state built from everything the host has sent.
    ///
    /// # Errors
    ///
    /// Fails if a thread holding the surface lock panicked.
    pub fn surface(&self) -> Result<Surface, Error> {
        self.surface
            .lock()
            .map(|surface| surface.clone())
            .map_err(|_| Error::Poisoned)
    }

    /// How far through the current beat we are, in `0.0..1.0`.
    ///
    /// Pass this to [`Surface::color_at`] so flashing and pulsing follow the host's tempo.
    ///
    /// # Errors
    ///
    /// Fails if a thread holding the clock lock panicked.
    pub fn phase(&self) -> Result<f32, Error> {
        self.clock
            .lock()
            .map(|clock| clock.phase(Instant::now()))
            .map_err(|_| Error::Poisoned)
    }

    /// Tempo in beats per minute, as the host's clock defines it.
    ///
    /// # Errors
    ///
    /// Fails if a thread holding the clock lock panicked.
    pub fn bpm(&self) -> Result<f32, Error> {
        self.clock
            .lock()
            .map(|clock| clock.bpm())
            .map_err(|_| Error::Poisoned)
    }

    /// Connects real hardware, passing host traffic through to it unchanged.
    ///
    /// Hardware presses reach the host through [`Self::pump_hardware`].
    ///
    /// # Errors
    ///
    /// Fails if no matching hardware port is found, it cannot be opened, or a lock is poisoned.
    pub fn attach_hardware(&mut self) -> Result<(), Error> {
        let not_found = || Error::HardwareNotFound {
            keyword: S::HARDWARE_KEYWORD,
        };

        let output = MidiOutput::new("launchpad-emulator")?;
        let port = find_port(&output, S::HARDWARE_KEYWORD).ok_or_else(not_found)?;
        let connection = output
            .connect(&port, "launchpad-emulator-out")
            .map_err(|e| Error::Connect(e.to_string()))?;
        *self.hardware_out.lock().map_err(|_| Error::Poisoned)? = Some(connection);

        let input = MidiInput::new("launchpad-emulator")?;
        let port = find_port(&input, S::HARDWARE_KEYWORD).ok_or_else(not_found)?;
        let sink = self.hardware_sink.clone();
        self.hardware_in = Some(
            input
                .connect(
                    &port,
                    "launchpad-emulator-in",
                    move |_timestamp, bytes, ()| {
                        if let Some(interaction) = S::decode_interaction(bytes) {
                            let _ = sink.send(interaction);
                        }
                    },
                    (),
                )
                .map_err(|e| Error::Connect(e.to_string()))?,
        );
        Ok(())
    }

    /// Whether real hardware is attached.
    #[must_use]
    pub fn hardware_attached(&self) -> bool {
        self.hardware_out
            .lock()
            .is_ok_and(|hardware| hardware.is_some())
    }

    /// Reports an interaction to the host as though a pad had been touched.
    ///
    /// # Errors
    ///
    /// Fails if the pad lies off the surface or the MIDI write fails.
    pub fn send(&mut self, interaction: Interaction) -> Result<(), Error> {
        let bytes = S::encode(interaction);
        if bytes.is_empty() {
            return Err(Error::OffSurface {
                pad: interaction.pad(),
            });
        }
        self.to_host.send(&bytes)?;
        Ok(())
    }

    /// Everything the host has sent since the last call, oldest first.
    pub fn poll(&self) -> Vec<HostMessage> {
        self.host_messages.try_iter().collect()
    }

    /// Drains interactions from attached hardware and reports them to the host.
    ///
    /// # Errors
    ///
    /// Fails if the MIDI write to the host fails.
    pub fn pump_hardware(&mut self) -> Result<Vec<Interaction>, Error> {
        let pending: Vec<Interaction> = self.hardware_interactions.try_iter().collect();
        for interaction in &pending {
            self.to_host.send(&S::encode(*interaction))?;
        }
        Ok(pending)
    }

    /// Sends raw bytes to attached hardware, doing nothing when none is attached.
    ///
    /// # Errors
    ///
    /// Fails if the MIDI write fails or a lock is poisoned.
    pub fn send_to_hardware(&mut self, bytes: &[u8]) -> Result<(), Error> {
        if let Some(output) = self
            .hardware_out
            .lock()
            .map_err(|_| Error::Poisoned)?
            .as_mut()
        {
            output.send(bytes)?;
        }
        Ok(())
    }
}

/// Finds the first MIDI port whose name contains `keyword`.
fn find_port<T: midir::MidiIO>(io: &T, keyword: &str) -> Option<T::Port> {
    io.ports()
        .into_iter()
        .find(|port| io.port_name(port).is_ok_and(|name| name.contains(keyword)))
}
