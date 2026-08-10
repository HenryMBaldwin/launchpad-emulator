# launchpad-emulator

A Novation Launchpad emulator that presents itself as a MIDI device.

The emulator creates a pair of virtual MIDI ports. A host application connects
to those instead of the hardware, and everything it sends to light the surface
is decoded into a `Surface` a front end can draw. Interactions travel the other
way, so clicking a pad in a front end reaches the host as a real press would.

Attaching a real Launchpad mirrors both directions at once: the host's lighting
reaches the hardware unchanged, and hardware presses are reported to the host.
Because the emulator sits in the signal path rather than beside it, it sees LED
traffic that a passive MIDI monitor cannot.

## Layout

- `crates/core` — the `launchpad-emulator` library, with no front end dependencies
- `crates/ui` — an egui widget, embeddable in any egui app
- `crates/app` — a standalone window wrapping the widget

Device differences live behind the `DeviceSpec` trait, so the emulator, the
surface and any front end are written once. `Surface` is deliberately not
generic over the device, which keeps front ends free of type parameters.

Currently implemented: Launchpad X and Launchpad Mini MK3.

## Usage

```rust
use launchpad_emulator::{devices::LaunchpadX, Emulator, Interaction, Pad};

let mut emulator = Emulator::<LaunchpadX>::with_default_name()?;
emulator.attach_hardware()?;

for message in emulator.poll() {
    println!("{message:?}");
}

emulator.send(Interaction::Press {
    pad: Pad::new(0, 8),
    velocity: 100,
})?;
```

## Running

```
cargo run -p launchpad-emulator-app            # Launchpad X
cargo run -p launchpad-emulator-app mini-mk3   # Launchpad Mini MK3
```

The window creates the virtual ports and draws whatever a host sends. Clicking a pad reports a
press, and holding one ramps aftertouch pressure.

Two examples help when working on it. `drive` acts as a host, lighting a pattern and sending a
beat clock, and `loopback` checks that interactions reach a host unchanged:

```
cargo run --example drive
cargo run --example loopback
```

## Platform support

Virtual MIDI ports are unavailable on Windows, so this crate does not build
there. macOS (CoreMIDI) and Linux (ALSA, JACK) are supported.

Building on Linux needs the ALSA development headers:

```
sudo apt-get install libasound2-dev
```

## Colour palette

The 128 palette colours were sampled from the chart in the Launchpad X
Programmer's Reference Manual and rescaled so that entry 0 is the unlit black
the hardware shows. The chart renders every channel with a floor of `0x61`;
rescaling that floor to zero maps entry 0 to exact black and the primaries to
pure red, green and blue.

## License

Licensed under the [MIT License](./LICENSE-MIT).

Any contribution intentionally submitted for inclusion in this repository shall
be licensed as above, without any additional terms or conditions.
