//! Drives the emulator over MIDI as a host would and prints the surface it builds.
//!
//! Run with a Launchpad X attached to also check that mirroring reaches the hardware.

use std::error::Error;
use std::fmt::Write as _;
use std::thread::sleep;
use std::time::Duration;

use launchpad_emulator::devices::{LaunchpadX, mk3_family, x};
use launchpad_emulator::{DeviceSpec, Emulator, Interaction, Pad, Surface};
use midir::MidiOutput;

const PORT: &str = "Launchpad X Emulator";

fn main() -> Result<(), Box<dyn Error>> {
    let mut emulator = Emulator::<LaunchpadX>::new(PORT)?;
    match emulator.attach_hardware() {
        Ok(()) => println!("hardware attached, mirroring is on"),
        Err(e) => println!("running without hardware: {e}"),
    }

    let output = MidiOutput::new("smoke-host")?;
    let port = output
        .ports()
        .into_iter()
        .find(|p| output.port_name(p).is_ok_and(|n| n.contains(PORT)))
        .ok_or("the emulator's port did not appear")?;
    let mut host = output.connect(&port, "smoke-host")?;

    let sysex = |body: &[u8]| {
        let mut bytes = mk3_family::sysex_header(x::DEVICE_ID).to_vec();
        bytes.extend_from_slice(body);
        bytes.push(0xF7);
        bytes
    };

    host.send(&sysex(&[0x0E, 0x01]))?;

    // A red diagonal by note, a green column by the lighting SysEx, and a pulsing blue logo
    for i in 0..8 {
        let pad = Pad::new(i, i + 1);
        host.send(&[0x90, LaunchpadX::pad_to_midi(pad).ok_or("off surface")?, 5])?;
    }
    let mut body = vec![0x03];
    for y in 1..9 {
        body.extend_from_slice(&[
            0,
            LaunchpadX::pad_to_midi(Pad::new(8, y)).ok_or("off surface")?,
            21,
        ]);
    }
    body.extend_from_slice(&[
        2,
        LaunchpadX::pad_to_midi(Pad::new(8, 0)).ok_or("off surface")?,
        45,
    ]);
    host.send(&sysex(&body))?;
    host.send(&sysex(&[0x03, 3, 91, 127, 0, 127]))?;

    sleep(Duration::from_millis(300));

    let messages = emulator.poll();
    println!("decoded {} messages from the host", messages.len());
    draw(&emulator.surface()?);

    println!("\nsending a press to the host, and reporting hardware presses for 15s");
    emulator.send(Interaction::Press {
        pad: Pad::new(0, 8),
        velocity: 100,
    })?;

    for _ in 0..150 {
        for interaction in emulator.pump_hardware()? {
            println!("  hardware -> host: {interaction:?}");
        }
        sleep(Duration::from_millis(100));
    }
    Ok(())
}

/// Prints a surface as truecolor blocks, with the logo separated from the grid.
fn draw(surface: &Surface) {
    println!(
        "programmer_mode={} brightness={} asleep={}",
        surface.is_programmer_mode(),
        surface.brightness(),
        surface.is_asleep()
    );
    for y in 0..surface.height() {
        let mut line = String::new();
        for x in 0..surface.width() {
            let c = surface.color_at(Pad::new(x, y), 0.25);
            let _ = write!(line, "\x1b[38;2;{};{};{}m██\x1b[0m", c.r, c.g, c.b);
        }
        println!("{line}");
    }
}
