//! A standalone window presenting a Launchpad emulator as a virtual MIDI device.

mod icon;

use std::error::Error;
use std::time::Duration;

use eframe::egui::{CentralPanel, Frame, Panel, ScrollArea, Slider, Ui, ViewportBuilder};
use launchpad_emulator::devices::{LaunchpadMiniMk3, LaunchpadX};
use launchpad_emulator::{DeviceSpec, Emulator, HostMessage, Interaction, Pad};
use launchpad_emulator_ui::{Labels, LaunchpadUi, Layout};

/// Messages kept in the activity log.
const LOG_LIMIT: usize = 200;

/// Height the activity log opens at.
const LOG_HEIGHT: f32 = 96.0;

/// Width the surface opens at.
const BOARD_SIZE: f32 = 460.0;

/// Smallest the surface is allowed to become.
const MIN_BOARD: f32 = 280.0;

/// Narrowest the window may be, set by the controls rather than the surface.
const MIN_WIDTH: f32 = 440.0;

/// Smallest the activity log is allowed to become.
const MIN_LOG: f32 = 64.0;

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
    let mut emulator = match port {
        Some(name) => Emulator::<S>::new(name)?,
        None => Emulator::<S>::with_default_name()?,
    };
    let hardware = emulator.attach_hardware().is_ok();

    let options = eframe::NativeOptions {
        // Without this the window reopens at whatever size it was last dragged to
        persist_window: false,
        viewport: ViewportBuilder::default()
            .with_icon(icon::build())
            .with_inner_size([BOARD_SIZE, BOARD_SIZE + LOG_HEIGHT + 34.0])
            .with_min_inner_size([MIN_WIDTH, MIN_BOARD + MIN_LOG]),
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
                log: Vec::new(),
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
    log: Vec<String>,
    decoded: usize,
}

impl<S: DeviceSpec> App<S> {
    /// Records a line in the activity log, dropping the oldest once it is full.
    fn note(&mut self, line: String) {
        self.log.push(line);
        if self.log.len() > LOG_LIMIT {
            self.log.drain(..self.log.len() - LOG_LIMIT);
        }
    }

    /// Drains the emulator and forwards anything attached hardware reported.
    fn pump(&mut self) {
        for message in self.emulator.poll() {
            self.decoded += 1;
            match message {
                // Lighting and clock arrive constantly and would drown the log
                HostMessage::Lighting { .. } | HostMessage::Clock => {}
                other => self.note(format!("{other:?}")),
            }
        }
        match self.emulator.pump_hardware() {
            Ok(interactions) => {
                for interaction in interactions {
                    self.note(format!("hardware {interaction:?}"));
                }
            }
            Err(e) => self.note(format!("hardware error: {e}")),
        }
    }

    /// Reports interactions produced by the widget.
    fn send(&mut self, interactions: Vec<Interaction>) {
        for interaction in interactions {
            if let Err(e) = self.emulator.send(interaction) {
                self.note(format!("send failed: {e}"));
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

    /// Draws the controls and the activity log.
    fn log_panel(&mut self, ui: &mut Ui) {
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
            ui.label(format!("{} decoded", self.decoded));
        });
        ScrollArea::vertical().stick_to_bottom(true).show(ui, |ui| {
            for line in &self.log {
                ui.monospace(line);
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
        Panel::bottom("log")
            .resizable(true)
            .default_size(LOG_HEIGHT)
            .show(ui, |ui| self.log_panel(ui));

        let board = CentralPanel::default()
            .frame(Frame::NONE)
            .show(ui, |ui| self.widget.show(ui, &self.layout, &surface, beats))
            .inner;
        self.send(board.inner);

        // Flashing and pulsing animate between host messages
        ui.ctx().request_repaint_after(Duration::from_millis(16));
    }
}
