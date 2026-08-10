//! A standalone window presenting a Launchpad emulator as a virtual MIDI device.

mod icon;

use std::error::Error;
use std::time::Duration;

use eframe::egui::{CentralPanel, Frame, Key, Panel, Slider, Ui, ViewportBuilder};
use launchpad_emulator::devices::{LaunchpadMiniMk3, LaunchpadX};
use launchpad_emulator::{DeviceSpec, Emulator, Interaction, Pad};
use launchpad_emulator_ui::{Console, Labels, LaunchpadUi, Layout};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::util::SubscriberInitExt as _;

/// Height of the bars above and below the surface with the console hidden.
const CHROME: f32 = 72.0;

/// Width the surface opens at.
const BOARD_SIZE: f32 = 460.0;

/// Smallest the surface is allowed to become.
const MIN_BOARD: f32 = 280.0;

/// Narrowest the window may be, set by the controls rather than the surface.
const MIN_WIDTH: f32 = 520.0;

/// What the command line asked for.
struct Args {
    device: String,
    port: Option<String>,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = parse_args(std::env::args().skip(1))?;
    match args.device.as_str() {
        "x" => run::<LaunchpadX>(args.port.as_deref()),
        "mini-mk3" => run::<LaunchpadMiniMk3>(args.port.as_deref()),
        other => Err(format!("unknown device {other:?}, expected \"x\" or \"mini-mk3\"").into()),
    }
}

/// Reads `[device] [--port NAME]`.
fn parse_args(args: impl Iterator<Item = String>) -> Result<Args, Box<dyn Error>> {
    let mut parsed = Args {
        device: "x".into(),
        port: None,
    };
    let mut args = args.peekable();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--port" => {
                parsed.port = Some(args.next().ok_or("--port needs a name")?);
            }
            "-h" | "--help" => {
                println!("usage: launchpad-emulator [x|mini-mk3] [--port NAME]");
                std::process::exit(0);
            }
            other => parsed.device = other.into(),
        }
    }
    Ok(parsed)
}

/// Opens the window for one device.
fn run<S: DeviceSpec + 'static>(port: Option<&str>) -> Result<(), Box<dyn Error>> {
    // The console collects traces from the whole program, and stderr keeps them after it closes
    let console = Console::new();
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(console.layer())
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .init();

    let mut emulator = match port {
        Some(name) => Emulator::<S>::new(name)?,
        None => Emulator::<S>::with_default_name()?,
    };
    let hardware = emulator.attach_hardware().is_ok();
    tracing::info!(device = S::NAME, hardware, "ready");

    let options = eframe::NativeOptions {
        // Without this the window reopens at whatever size it was last dragged to
        persist_window: false,
        viewport: ViewportBuilder::default()
            .with_icon(icon::build())
            .with_inner_size([BOARD_SIZE, BOARD_SIZE + CHROME])
            .with_min_inner_size([MIN_WIDTH, MIN_BOARD + CHROME]),
        ..Default::default()
    };
    eframe::run_native(
        S::NAME,
        options,
        Box::new(move |cc| {
            // Labels should appear the moment the pointer arrives, with no fade
            cc.egui_ctx.all_styles_mut(|style| {
                style.animation_time = 0.0;
                style.interaction.tooltip_delay = 0.0;
                style.interaction.tooltip_grace_time = 0.0;
            });
            // The device's own names, with what each pad sends laid over the top
            let names = Labels::defaults::<S>();
            let numbers: Vec<_> = Pad::all(S::WIDTH, S::HEIGHT)
                .filter_map(|pad| {
                    let name = names.get(pad)?;
                    Some((pad, format!("{name}  ({})", S::pad_to_midi(pad)?)))
                })
                .collect();
            let widget = LaunchpadUi::new().with_labels(names.clone().with_all(numbers));
            Ok(Box::new(App {
                layout: Layout::for_device::<S>(),
                widget,
                emulator,
                hardware,
                console: console.clone(),
                decoded: 0,
            }))
        }),
    )?;
    Ok(())
}

/// The window state.
struct App<S: DeviceSpec> {
    emulator: Emulator<S>,
    layout: Layout,
    widget: LaunchpadUi,
    hardware: bool,
    console: Console,
    decoded: usize,
}

impl<S: DeviceSpec> App<S> {
    /// Drains the emulator and forwards anything attached hardware reported.
    fn pump(&mut self) {
        for message in self.emulator.poll() {
            self.decoded += 1;
            self.console.record(&message);
        }
        match self.emulator.pump_hardware() {
            Ok(interactions) => {
                for interaction in interactions {
                    tracing::info!(?interaction, "hardware");
                }
            }
            Err(error) => tracing::warn!(%error, "could not read the hardware"),
        }
    }

    /// Reports interactions produced by the widget.
    fn send(&mut self, interactions: Vec<Interaction>) {
        for interaction in interactions {
            if let Err(error) = self.emulator.send(interaction) {
                tracing::warn!(%error, "could not report the interaction");
            }
        }
    }

    /// Draws the strip describing what the host has asked for.
    fn status_bar(&self, ui: &mut Ui, bpm: f32, surface: &launchpad_emulator::Surface) {
        ui.horizontal(|ui| {
            ui.label(S::NAME);
            ui.separator();
            ui.label(if surface.is_programmer_mode() {
                "programmer"
            } else {
                "live"
            });
            ui.separator();
            ui.label(format!("brightness {}", surface.brightness()));
            ui.separator();
            ui.label(format!("{bpm:.0} bpm"));
            ui.separator();
            ui.label(if self.hardware {
                "hardware mirrored"
            } else {
                "no hardware"
            });
            if surface.is_asleep() {
                ui.separator();
                ui.label("asleep");
            }
        });
    }

    /// Draws the controls, which stay visible whether the console is showing or not.
    fn controls(&mut self, ui: &mut Ui) {
        ui.horizontal_wrapped(|ui| {
            let mut velocity = self.widget.velocity();
            if ui
                .add(Slider::new(&mut velocity, 1..=127).text("velocity"))
                .changed()
            {
                self.widget.set_velocity(velocity);
            }
            let mut aftertouch = self.widget.aftertouch_on_hold();
            if ui.checkbox(&mut aftertouch, "aftertouch on hold").changed() {
                self.widget.set_aftertouch_on_hold(aftertouch);
            }
            ui.separator();
            if ui
                .selectable_label(self.console.is_visible(), "console")
                .on_hover_text("Show what the host has sent (`)")
                .clicked()
            {
                self.console.toggle();
            }
            ui.label(format!("{} decoded", self.decoded));
            if self.console.is_visible() && ui.button("clear").clicked() {
                self.console.clear();
            }
        });
    }
}

impl<S: DeviceSpec> eframe::App for App<S> {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        self.pump();
        let _ = self.emulator.advance();

        let beats = self.emulator.beats().unwrap_or(0.0);
        let bpm = self.emulator.bpm().unwrap_or(0.0);
        let Ok(surface) = self.emulator.surface() else {
            return;
        };

        Panel::top("status").show(ui, |ui| self.status_bar(ui, bpm, &surface));
        if ui.ctx().input(|i| i.key_pressed(Key::Backtick)) {
            self.console.toggle();
        }
        Panel::bottom("console")
            .resizable(self.console.is_visible())
            .show(ui, |ui| {
                self.controls(ui);
                self.console.show(ui);
            });

        let board = CentralPanel::default()
            .frame(Frame::NONE)
            .show(ui, |ui| self.widget.show(ui, &self.layout, &surface, beats))
            .inner;
        self.send(board.inner);

        // Flashing and pulsing animate between host messages
        ui.ctx().request_repaint_after(Duration::from_millis(16));
    }
}
