//! Checks that an interaction reported by the emulator arrives at a host application.
//!
//! Creates an emulator, connects to its ports the way a host would, sends a press and a release,
//! and reports what came back.

use std::error::Error;
use std::sync::mpsc::channel;
use std::thread::sleep;
use std::time::Duration;

use launchpad_emulator::devices::LaunchpadX;
use launchpad_emulator::{DeviceSpec, Emulator, Interaction, Pad};
use midir::MidiInput;

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
            let _ = sender.send(LaunchpadX::decode_interaction(bytes));
        },
        (),
    )?;

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

    let got: Vec<_> = received.try_iter().flatten().collect();
    println!("sent     {sent:?}");
    println!("received {got:?}");
    if got == sent {
        println!("PASS: every interaction reached the host unchanged");
        Ok(())
    } else {
        Err("interactions did not survive the round trip".into())
    }
}
