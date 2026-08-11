//! Drives a Launchpad, real or emulated, as a host application would.
//!
//! Pass a substring of the MIDI port to drive, defaulting to the Launchpad X emulator's own name.

use std::error::Error;
use std::thread::sleep;
use std::time::Duration;

use launchpad_emulator::DeviceSpec;
use launchpad_emulator::devices::{LaunchpadX, mk3_family, x};
use midir::{MidiInput, MidiOutput, MidiOutputConnection};

fn main() -> Result<(), Box<dyn Error>> {
    // Exactly, not by substring: the hardware's own ports extend the emulator's name and come
    // first in the list, so a substring would drive the hardware instead
    let wanted = std::env::args()
        .nth(1)
        .unwrap_or_else(|| LaunchpadX::PORT_NAME.to_owned());
    let output = MidiOutput::new("drive")?;
    let port = output
        .ports()
        .into_iter()
        .find(|p| output.port_name(p).is_ok_and(|n| n == wanted))
        .ok_or_else(|| format!("no MIDI port matching {wanted:?}"))?;
    println!("driving {:?}", output.port_name(&port)?);
    let mut host = output.connect(&port, "drive")?;

    // Listen the way a host would, so presses coming back are visible
    let input = MidiInput::new("drive")?;
    let source = input
        .ports()
        .into_iter()
        .find(|p| input.port_name(p).is_ok_and(|n| n == wanted))
        .ok_or_else(|| format!("no MIDI source matching {wanted:?}"))?;
    let listener = input.connect(
        &source,
        "drive-in",
        |_timestamp, bytes, ()| match LaunchpadX::decode_interaction(bytes) {
            Some(interaction) => println!("  received {interaction:?}"),
            None => println!("  received {bytes:02X?}"),
        },
        (),
    )?;

    host.send(&sysex(&[0x0E, 0x01]))?;
    paint(&mut host)?;

    // Loop some text so the scroll can be seen going
    let mut scroll = sysex(&[0x07, 1, 7, 0, 0x25]);
    scroll.pop();
    scroll.extend_from_slice(b"HELLO 123");
    scroll.push(0xF7);
    host.send(&scroll)?;

    // 24 ticks per beat at 120 BPM, so flashing and pulsing have a tempo to follow
    let tick = Duration::from_secs_f64(60.0 / 120.0 / 24.0);
    for _ in 0..(24 * 120) {
        host.send(&[0xF8])?;
        sleep(tick);
    }
    drop(listener);
    Ok(())
}

/// Wraps a command in the Launchpad X `SysEx` header.
fn sysex(body: &[u8]) -> Vec<u8> {
    let mut bytes = mk3_family::sysex_header(x::DEVICE_ID).to_vec();
    bytes.extend_from_slice(body);
    bytes.push(0xF7);
    bytes
}

/// Lights a pattern covering every lighting mode the surface supports.
fn paint(host: &mut MidiOutputConnection) -> Result<(), Box<dyn Error>> {
    let mut body = vec![0x03];
    let mut push = |pad, spec: &[u8]| {
        if let Some(number) = LaunchpadX::pad_to_midi(pad) {
            body.push(spec[0]);
            body.push(number);
            body.extend_from_slice(&spec[1..]);
        }
    };

    // A palette gradient across the grid
    for y in 1..9 {
        for x in 0..8 {
            let entry = 4 + (y - 1) * 8 + x;
            push(launchpad_emulator::Pad::new(x, y), &[0, entry]);
        }
    }
    // Round buttons flash, the right-hand column pulses, the logo is a plain RGB white
    for x in 0..8 {
        push(launchpad_emulator::Pad::new(x, 0), &[1, 0, 5]);
    }
    for y in 1..9 {
        push(launchpad_emulator::Pad::new(8, y), &[2, 45]);
    }
    push(launchpad_emulator::Pad::new(8, 0), &[3, 127, 127, 127]);

    host.send(&sysex(&body))?;
    Ok(())
}
