# launchpad-emulator

A Novation Launchpad emulator that presents itself as a MIDI device.

A host application connects to the emulator instead of the hardware. Everything
it sends to light the surface is decoded into a `Surface` a front end can draw,
and interactions travel the other way, so clicking a pad reaches the host as a
real press would. The emulator answers the same queries the hardware answers, so
a host that identifies a device by asking it who it is finds a Launchpad.

Attaching a real Launchpad mirrors both directions at once: the host's lighting
reaches the hardware unchanged, and hardware presses are reported to the host.
The emulator sits in the signal path, so it sees LED traffic a passive MIDI
monitor cannot.

## Supported devices

Note: I only have a Launchpad X so it's unlikely I will dedicate much time to
implementing the other models, however I encourage you to implement your model
based on the existing code and I'll happily merge.

- [ ] Launchpad (MK1)
- [ ] Launchpad S
- [ ] Launchpad Mini
- [ ] Launchpad Control
- [ ] Launchpad Control XL
- [ ] Launchpad Pro
- [ ] Launchpad MK2
- [x] Launchpad X
- [x] Launchpad Mini MK3
- [ ] Launchpad Pro MK2

## Running

```
cargo run --bin launchpad-emulator                      # Launchpad X
cargo run --bin launchpad-emulator mini-mk3             # Launchpad Mini MK3
cargo run --bin launchpad-emulator -- --port "My Pad"   # a name of your own
```

The virtual ports are named after the hardware, so a host that discovers a
Launchpad by port name finds the emulator. With real hardware attached as well,
the hardware's own ports extend the emulator's name and are listed first, so a
host matching on a substring reaches the hardware instead. Match the port name
exactly, or select it by hand.

Clicking a pad reports a press, holding one ramps aftertouch pressure, and
resting the pointer on one shows its label.

## Using the library

### Emulator

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

`Emulator::in_process` publishes no MIDI ports. Drive it with `feed` and collect
what the user does with `reported`:

```rust
use launchpad_emulator::{devices::LaunchpadX, Emulator};

let mut emulator = Emulator::<LaunchpadX>::in_process();
emulator.feed(&[0x90, 11, 5]);
for interaction in emulator.reported() {
    println!("{interaction:?}");
}
```

Call `Emulator::advance` once a frame so a text scroll started by the host moves
on, and pass `Emulator::beats` to `Surface::color_at` so flashing and pulsing
follow the host's tempo.

### Labels

Each pad can carry a label, shown while the pointer rests on it.
`Labels::defaults` names the buttons with the words printed on the device and
numbers the grid from its own top left corner, so the first pad reads
`Grid 0,0`, which is `Pad::new(0, 1)` on the surface. `Labels::none` starts
empty, and layers go on top of either:

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

Labels are drawn as egui tooltips, and are tweaked the same way.

### Console

`Console` records what the host has sent, stamped with the time it arrived. It starts hidden and
draws nothing until shown, so a front end can call `show` unconditionally.

```rust
use launchpad_emulator_ui::Console;

let mut console = Console::new().with_visible(true).with_limit(500);
console.push("connected");
```

`record` takes a `HostMessage` and skips the lighting and clock messages that arrive continuously.
`push` takes any line.

`Console::layer` is a `tracing` layer, so installing it puts every event in the program on screen,
including events from crates the front end brings itself:

```rust
use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::util::SubscriberInitExt as _;

let console = launchpad_emulator_ui::Console::new();
tracing_subscriber::registry().with(console.layer()).init();
```

## Platform support

macOS and Linux. Windows has no virtual MIDI ports, so the crate does not build
there.

Linux needs the ALSA development headers:

```
sudo apt-get install libasound2-dev
```

## Development

- `crates/launchpad-emulator` — the library, with no front end dependencies
- `crates/launchpad-emulator-ui` — an egui widget, embeddable in any egui app
- `crates/launchpad-emulator-app` — a standalone window wrapping the widget

Device differences live behind the `DeviceSpec` trait. `Surface` is not generic
over the device.

Three examples drive the emulator without a Launchpad to hand. `drive` acts as a
host, lighting a pattern, scrolling text and sending a beat clock; `loopback`
checks that interactions reach a host and that queries are answered; `smoke`
drives the surface and prints it as coloured blocks.

```
cargo run --example drive
cargo run --example loopback
cargo run --example smoke
```

## License

Licensed under the [MIT License](./LICENSE-MIT).

Any contribution intentionally submitted for inclusion in this repository shall
be licensed as above, without any additional terms or conditions.
