//! The virtual MIDI device a host application connects to.

use std::marker::PhantomData;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use midir::os::unix::{VirtualInput, VirtualOutput};
use midir::{MidiInput, MidiInputConnection, MidiOutput, MidiOutputConnection};

use crate::message::{HostMessage, Interaction};
use crate::{Clock, DeviceSpec, Error, Surface};

/// A MIDI output shared with the callback that writes to it.
type SharedOutput = Arc<Mutex<Option<MidiOutputConnection>>>;

/// Replies queued for a host that has no MIDI port to receive them on.
type Replies = Arc<Mutex<Vec<Vec<u8>>>>;

/// A Launchpad an application can drive, over MIDI or in process.
///
/// [`Emulator::new`] publishes a pair of virtual MIDI ports for an external host to connect to, and
/// dropping the emulator removes them. [`Emulator::in_process`] publishes nothing and is driven by
/// [`Emulator::feed`] instead.
pub struct Emulator<S: DeviceSpec> {
    /// Receives what the host sends; named from the host's point of view.
    _from_host: Option<MidiInputConnection<()>>,
    /// Sends what the emulator reports; named from the host's point of view.
    to_host: SharedOutput,
    /// Interactions waiting to be drained when there is no host port to write to.
    reported: Vec<Interaction>,
    /// Replies waiting to be drained when there is no host port to write to.
    replies: Replies,
    hardware_out: SharedOutput,
    hardware_in: Option<MidiInputConnection<()>>,
    host_messages: Receiver<HostMessage>,
    hardware_interactions: Receiver<Interaction>,
    hardware_sink: Sender<Interaction>,
    surface: Arc<Mutex<Surface>>,
    clock: Arc<Mutex<Clock>>,
    message_sink: Sender<HostMessage>,
    /// Name of our own virtual ports, which must never be mistaken for hardware.
    port_name: Option<String>,
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
        let hardware_out: SharedOutput = Arc::new(Mutex::new(None));
        let to_host: SharedOutput = Arc::new(Mutex::new(None));
        let (message_sink, host_messages) = channel();
        let (hardware_sink, hardware_interactions) = channel();
        let replies: Replies = Arc::new(Mutex::new(Vec::new()));

        let sink = message_sink.clone();
        let shared_surface = Arc::clone(&surface);
        let shared_clock = Arc::clone(&clock);
        let shared_hardware = Arc::clone(&hardware_out);
        let shared_to_host = Arc::clone(&to_host);
        let shared_replies = Arc::clone(&replies);
        let from_host = MidiInput::new(port_name)?
            .create_virtual(
                port_name,
                move |_timestamp, bytes, ()| {
                    ingest::<S>(
                        bytes,
                        &shared_surface,
                        &shared_clock,
                        &shared_hardware,
                        &shared_to_host,
                        &shared_replies,
                        &sink,
                    );
                },
                (),
            )
            .map_err(|_| Error::VirtualPort {
                name: port_name.into(),
            })?;

        let port = MidiOutput::new(port_name)?
            .create_virtual(port_name)
            .map_err(|_| Error::VirtualPort {
                name: port_name.into(),
            })?;
        *to_host.lock().map_err(|_| Error::Poisoned)? = Some(port);

        Ok(Self {
            _from_host: Some(from_host),
            to_host,
            reported: Vec::new(),
            hardware_out,
            hardware_in: None,
            host_messages,
            hardware_interactions,
            hardware_sink,
            surface,
            clock,
            message_sink,
            replies,
            port_name: Some(port_name.to_owned()),
            device: PhantomData,
        })
    }

    /// Creates the virtual ports under [`DeviceSpec::PORT_NAME`], which hosts can discover.
    ///
    /// # Errors
    ///
    /// Fails if a MIDI backend cannot be initialised or a virtual port cannot be created.
    pub fn with_default_name() -> Result<Self, Error> {
        Self::new(S::PORT_NAME)
    }

    /// Creates an emulator that publishes no MIDI ports.
    ///
    /// Drive it with [`Self::feed`] and collect what it reports with [`Self::reported`]. Use this
    /// when embedding the emulator in the application that also plays it, so nothing appears in the
    /// system's MIDI port list.
    #[must_use]
    pub fn in_process() -> Self {
        let (message_sink, host_messages) = channel();
        let (hardware_sink, hardware_interactions) = channel();
        Self {
            _from_host: None,
            to_host: Arc::new(Mutex::new(None)),
            reported: Vec::new(),
            replies: Arc::new(Mutex::new(Vec::new())),
            hardware_out: Arc::new(Mutex::new(None)),
            hardware_in: None,
            host_messages,
            hardware_interactions,
            hardware_sink,
            surface: Arc::new(Mutex::new(Surface::new::<S>())),
            clock: Arc::new(Mutex::new(Clock::new(Instant::now()))),
            message_sink,
            port_name: None,
            device: PhantomData,
        }
    }

    /// Applies MIDI bytes as though a host had sent them.
    ///
    /// Mirrors them to attached hardware and updates the surface, exactly as the virtual port does.
    pub fn feed(&mut self, bytes: &[u8]) {
        ingest::<S>(
            bytes,
            &self.surface,
            &self.clock,
            &self.hardware_out,
            &self.to_host,
            &self.replies,
            &self.message_sink,
        );
    }

    /// Takes the replies the device produced since the last call.
    ///
    /// Only fills up when there is no host port to write to, so this is empty for an emulator made
    /// by [`Self::new`], which sends replies straight down the port.
    ///
    /// # Errors
    ///
    /// Fails if a thread holding the reply lock panicked.
    pub fn replies(&mut self) -> Result<Vec<Vec<u8>>, Error> {
        self.replies
            .lock()
            .map(|mut queued| std::mem::take(&mut *queued))
            .map_err(|_| Error::Poisoned)
    }

    /// Takes the interactions reported since the last call.
    ///
    /// Only fills up when there is no host port to write to, so this is empty for an emulator made
    /// by [`Self::new`].
    pub fn reported(&mut self) -> Vec<Interaction> {
        std::mem::take(&mut self.reported)
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

        let ours = self.port_name.as_deref();
        let output = MidiOutput::new("launchpad-emulator")?;
        let port = find_port(&output, S::HARDWARE_KEYWORD, ours).ok_or_else(not_found)?;
        let connection = output
            .connect(&port, "launchpad-emulator-out")
            .map_err(|e| Error::Connect(e.to_string()))?;
        *self.hardware_out.lock().map_err(|_| Error::Poisoned)? = Some(connection);

        let input = MidiInput::new("launchpad-emulator")?;
        let port = find_port(&input, S::HARDWARE_KEYWORD, ours).ok_or_else(not_found)?;
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

    /// Whether this emulator publishes virtual MIDI ports for an external host.
    #[must_use]
    pub fn publishes_ports(&self) -> bool {
        self.to_host.lock().is_ok_and(|port| port.is_some())
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
        match self.to_host.lock().map_err(|_| Error::Poisoned)?.as_mut() {
            Some(port) => port.send(&bytes)?,
            None => self.reported.push(interaction),
        }
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
        let mut port = self.to_host.lock().map_err(|_| Error::Poisoned)?;
        for interaction in &pending {
            match port.as_mut() {
                Some(port) => port.send(&S::encode(*interaction))?,
                None => self.reported.push(*interaction),
            }
        }
        drop(port);
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

/// Finds the first MIDI port whose name contains `keyword`, skipping our own by exact name.
///
/// The comparison with `ours` is exact because hardware port names extend the emulator's, so
/// `Launchpad X LPX MIDI In` must still match while `Launchpad X LPX MIDI` is skipped.
fn find_port<T: midir::MidiIO>(io: &T, keyword: &str, ours: Option<&str>) -> Option<T::Port> {
    io.ports().into_iter().find(|port| {
        io.port_name(port)
            .is_ok_and(|name| name.contains(keyword) && Some(name.as_str()) != ours)
    })
}

/// Applies host bytes to the surface, the clock, attached hardware and the message log.
fn ingest<S: DeviceSpec>(
    bytes: &[u8],
    surface: &Arc<Mutex<Surface>>,
    clock: &Arc<Mutex<Clock>>,
    hardware: &SharedOutput,
    to_host: &SharedOutput,
    replies: &Replies,
    sink: &Sender<HostMessage>,
) {
    if let Ok(mut hardware) = hardware.lock()
        && let Some(output) = hardware.as_mut()
    {
        let _ = output.send(bytes);
    }
    for message in S::decode(bytes) {
        if let Ok(mut surface) = surface.lock() {
            surface.apply(&message);
        }
        if matches!(message, HostMessage::Clock)
            && let Ok(mut clock) = clock.lock()
        {
            clock.tick(Instant::now());
        }
        // The hardware answers immediately, so replies go out before the next message
        let reply = surface
            .lock()
            .ok()
            .and_then(|surface| S::encode_reply(&message, &surface));
        if let Some(reply) = reply {
            match to_host.lock().as_deref_mut() {
                Ok(Some(port)) => {
                    let _ = port.send(&reply);
                }
                Ok(None) => {
                    if let Ok(mut queued) = replies.lock() {
                        queued.push(reply);
                    }
                }
                Err(_) => {}
            }
        }
        // A closed receiver only means nothing is reading the log
        let _ = sink.send(message);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::devices::LaunchpadX;
    use crate::{Lighting, Pad, Rgb};

    const RED: Rgb = Rgb { r: 255, g: 0, b: 0 };

    #[test]
    fn an_in_process_emulator_publishes_no_ports() {
        let emulator = Emulator::<LaunchpadX>::in_process();
        assert!(!emulator.publishes_ports());
        assert!(!emulator.hardware_attached());
    }

    #[test]
    fn fed_bytes_light_the_surface() -> Result<(), Error> {
        let mut emulator = Emulator::<LaunchpadX>::in_process();
        emulator.feed(&[0x90, 11, 5]);
        let surface = emulator.surface()?;
        assert_eq!(
            surface.lighting(Pad::new(0, 8)),
            Lighting::Static(RED),
            "a note on should light the bottom left pad"
        );
        Ok(())
    }

    #[test]
    fn fed_bytes_are_logged_and_drive_the_clock() -> Result<(), Error> {
        let mut emulator = Emulator::<LaunchpadX>::in_process();
        emulator.feed(&[0xF8]);
        assert_eq!(emulator.poll(), vec![HostMessage::Clock]);
        assert!(emulator.bpm()? > 0.0);
        Ok(())
    }

    #[test]
    fn interactions_queue_up_when_there_is_no_host_port() -> Result<(), Error> {
        let mut emulator = Emulator::<LaunchpadX>::in_process();
        let press = Interaction::Press {
            pad: Pad::new(0, 8),
            velocity: 100,
        };
        emulator.send(press)?;
        assert_eq!(emulator.reported(), vec![press]);
        assert!(
            emulator.reported().is_empty(),
            "draining should consume them"
        );
        Ok(())
    }

    #[test]
    fn our_own_port_is_never_mistaken_for_hardware() {
        // The hardware's names extend ours, so only an exact match may be skipped
        let ours = Some("Launchpad X LPX MIDI");
        let keyword = LaunchpadX::HARDWARE_KEYWORD;
        for (name, is_hardware) in [
            ("Launchpad X LPX MIDI", false),
            ("Launchpad X LPX MIDI In", true),
            ("Launchpad X LPX MIDI Out", true),
        ] {
            let matched = name.contains(keyword) && Some(name) != ours;
            assert_eq!(matched, is_hardware, "for {name:?}");
        }
    }

    #[test]
    fn a_pad_off_the_surface_is_refused() {
        let mut emulator = Emulator::<LaunchpadX>::in_process();
        let result = emulator.send(Interaction::Release {
            pad: Pad::new(20, 20),
        });
        assert!(matches!(result, Err(Error::OffSurface { .. })));
    }
}
