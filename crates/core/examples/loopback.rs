//! Checks that an interaction reported by the emulator arrives at a host application.
//!
//! Creates an emulator, connects to its ports the way a host would, sends a press and a release,
//! and reports what came back.

use std::error::Error;
use std::sync::mpsc::channel;
use std::thread::sleep;
use std::time::Duration;

use launchpad_emulator::devices::{LaunchpadX, mk3_family, x};
use launchpad_emulator::{DeviceSpec, Emulator, Interaction, Pad};
use midir::{MidiInput, MidiOutput};

const PORT: &str = "Launchpad X Loopback";

fn main() -> Result<(), Box<dyn Error>> {
    let mut emulator = Emulator::<LaunchpadX>::new(PORT)?;

    let input = MidiInput::new("loopback-host")?;
    let source = input
        .ports()
        .into_iter()
        .find(|p| input.port_name(p).is_ok_and(|n| n.contains(PORT)))
        .ok_or("the emulator's port did not appear")?;
    let (sender, received) = channel();
    let _listener = input.connect(
        &source,
        "loopback-host",
        move |_timestamp, bytes, ()| {
            let _ = sender.send((bytes.to_vec(), LaunchpadX::decode_interaction(bytes)));
        },
        (),
    )?;

    // A host identifies the device by asking it who it is
    let output = MidiOutput::new("loopback-host")?;
    let destination = output
        .ports()
        .into_iter()
        .find(|p| output.port_name(p).is_ok_and(|n| n.contains(PORT)))
        .ok_or("the emulator's destination did not appear")?;
    let mut host = output.connect(&destination, "loopback-host")?;
    host.send(&[0xF0, 0x7E, 0x7F, 0x06, 0x01, 0xF7])?;
    host.send(&sysex(&[0x08]))?;
    sleep(Duration::from_millis(200));

    let pad = Pad::new(3, 4);
    let sent = [
        Interaction::Press { pad, velocity: 100 },
        Interaction::Aftertouch { pad, pressure: 64 },
        Interaction::Release { pad },
    ];
    for interaction in sent {
        emulator.send(interaction)?;
        sleep(Duration::from_millis(50));
    }
    sleep(Duration::from_millis(200));

    let (replies, interactions): (Vec<_>, Vec<_>) = received
        .try_iter()
        .partition(|(_, decoded)| decoded.is_none());
    println!("replies to queries:");
    for (bytes, _) in &replies {
        println!("  {bytes:02X?}");
    }
    let inquiry_answered = replies.iter().any(|(bytes, _)| {
        bytes.starts_with(&[
            0xF0,
            0x7E,
            0x00,
            0x06,
            0x02,
            0x00,
            0x20,
            0x29,
            LaunchpadX::FAMILY_CODE,
        ])
    });
    let brightness_answered = replies
        .iter()
        .any(|(bytes, _)| bytes.starts_with(&mk3_family::sysex_header(x::DEVICE_ID)));
    println!("device inquiry answered: {inquiry_answered}");
    println!("brightness query answered: {brightness_answered}");

    let got: Vec<_> = interactions.into_iter().filter_map(|(_, d)| d).collect();
    println!("sent     {sent:?}");
    println!("received {got:?}");
    if got != sent {
        return Err("interactions did not survive the round trip".into());
    }
    if !inquiry_answered || !brightness_answered {
        return Err("the emulator did not answer a query".into());
    }
    println!("PASS: interactions round trip and queries are answered");
    Ok(())
}

/// Wraps a command in the Launchpad X `SysEx` header.
fn sysex(body: &[u8]) -> Vec<u8> {
    let mut bytes = mk3_family::sysex_header(x::DEVICE_ID).to_vec();
    bytes.extend_from_slice(body);
    bytes.push(0xF7);
    bytes
}
