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

- `crates/launchpad-emulator` — the library, with no front end dependencies
- `crates/launchpad-emulator-ui` — an egui widget, embeddable in any egui app
- `crates/launchpad-emulator-app` — a standalone window wrapping the widget

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
cargo run --bin launchpad-emulator                      # Launchpad X
cargo run --bin launchpad-emulator mini-mk3             # Launchpad Mini MK3
cargo run --bin launchpad-emulator --port "My Pad"      # a name of your own
```

The virtual ports are named after the hardware, so a host that discovers a Launchpad by port name
finds the emulator. With real hardware attached as well the match is ambiguous and the host may pick
either, so select the port explicitly in that case.

## Embedding

`Emulator::in_process` publishes no MIDI ports at all. Drive it with `feed` and collect what the
user does with `reported`, for an application that both draws the surface and plays it:

```rust
use launchpad_emulator::{devices::LaunchpadX, Emulator};

let mut emulator = Emulator::<LaunchpadX>::in_process();
emulator.feed(&[0x90, 11, 5]);
for interaction in emulator.reported() {
    println!("{interaction:?}");
}
```

The window creates the virtual ports and draws whatever a host sends. Clicking a pad reports a
press, and holding one ramps aftertouch pressure.

Two examples help when working on it. `drive` acts as a host, lighting a pattern and sending a
beat clock, and `loopback` checks that interactions reach a host unchanged:

```
cargo run --example drive
cargo run --example loopback
```

## Text scrolling

A scroll started by the host is drawn across the grid and the right-hand column, leaving the top row
and the logo alone, and the lighting underneath returns once the scroll stops or finishes. Call
`Emulator::advance` once a frame to move it on.

The font covers the letters, digits and punctuation in `font::GLYPHS`. Lowercase is drawn with the
uppercase glyph and anything else is skipped, so it is narrower than what the hardware can display.

## Labels

Each pad can carry a label, shown while the pointer rests on it. `Labels::defaults` names the
buttons with the words printed on the device and numbers the grid from its own top left corner, so
the first pad reads `Grid 0,0` even though it sits at `Pad::new(0, 1)` on the surface. `Labels::none`
starts empty, and layers go on top of either:

```rust
use launchpad_emulator::{devices::LaunchpadX, Pad};
use launchpad_emulator_ui::{Labels, LaunchpadUi};

let labels = Labels::defaults::<LaunchpadX>()
    .with(Pad::new(0, 8), "kick")
    .with_all([(Pad::new(1, 8), "snare"), (Pad::new(2, 8), "hat")])
    .overlay(Labels::none().with(Pad::new(0, 0), "shift"))
    .without(Pad::new(8, 0));

let widget = LaunchpadUi::new().with_labels(labels);
```

The label is drawn as an egui tooltip at the pointer, so how quickly it appears is the host's to
decide. The standalone app makes it immediate:

```rust
# let ctx = egui::Context::default();
ctx.all_styles_mut(|style| {
    style.animation_time = 0.0;
    style.interaction.tooltip_delay = 0.0;
    style.interaction.tooltip_grace_time = 0.0;
});
```

## Identifying as a Launchpad

The emulator answers the queries the hardware answers, so a host that identifies a device by asking
it who it is gets the same reply a real Launchpad gives: a universal device inquiry, the layout, the
brightness, the sleep state, the velocity curve and the aftertouch mode, plus the echo the hardware
sends when the mode changes.

## Window icon

The icon is drawn in code from the pad colours, so the app ships no image files. To use a different
one, replace `icon::build` in `crates/launchpad-emulator-app`.

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
