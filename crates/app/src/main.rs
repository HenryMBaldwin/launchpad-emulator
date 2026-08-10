//! A standalone window presenting a Launchpad emulator as a virtual MIDI device.

mod aspect;

use std::error::Error;
use std::time::Duration;

use eframe::egui::{
    CentralPanel, Context, Frame, Panel, ScrollArea, Slider, Ui, Vec2, ViewportBuilder,
    ViewportCommand,
};
use launchpad_emulator::devices::{LaunchpadMiniMk3, LaunchpadX};
use launchpad_emulator::{DeviceSpec, Emulator, HostMessage, Interaction};
use launchpad_emulator_ui::{LaunchpadUi, Layout};

/// Messages kept in the activity log.
const LOG_LIMIT: usize = 200;

/// Height the activity log opens at.
const LOG_HEIGHT: f32 = 120.0;

/// Width the surface opens at.
const BOARD_SIZE: f32 = 460.0;

/// Smallest the surface is allowed to become.
const MIN_BOARD: f32 = 280.0;

/// Smallest the activity log is allowed to become.
const MIN_LOG: f32 = 64.0;

fn main() -> Result<(), Box<dyn Error>> {
    let device = std::env::args().nth(1).unwrap_or_else(|| "x".into());
    match device.as_str() {
        "x" => run::<LaunchpadX>(),
        "mini-mk3" => run::<LaunchpadMiniMk3>(),
        other => Err(format!("unknown device {other:?}, expected \"x\" or \"mini-mk3\"").into()),
    }
}

/// Opens the window for one device.
fn run<S: DeviceSpec + 'static>() -> Result<(), Box<dyn Error>> {
    let mut emulator = Emulator::<S>::with_default_name()?;
    let hardware = emulator.attach_hardware().is_ok();
    let aspect = f32::from(S::WIDTH) / f32::from(S::HEIGHT);

    let options = eframe::NativeOptions {
        viewport: ViewportBuilder::default()
            .with_inner_size([BOARD_SIZE, BOARD_SIZE / aspect + LOG_HEIGHT + 60.0])
            .with_min_inner_size([MIN_BOARD, MIN_BOARD / aspect + LOG_HEIGHT]),
        ..Default::default()
    };
    eframe::run_native(
        S::NAME,
        options,
        Box::new(move |_cc| {
            Ok(Box::new(App {
                layout: Layout::for_device::<S>(),
                widget: LaunchpadUi::new(),
                emulator,
                hardware,
                aspect,
                matched: None,
                native_lock: false,
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
    aspect: f32,
    /// Window size the proportions were last matched to.
    matched: Option<Vec2>,
    /// Whether the window server is holding the ratio for us.
    native_lock: bool,
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

    /// Keeps the window shaped so the surface stays square.
    ///
    /// Windows have no aspect constraint of their own, so this corrects the dimension the pointer
    /// did not drag: widen the window and the height follows, shorten it and the width follows.
    /// `chrome` is the height the bars take. Maximised and fullscreen windows are left alone and
    /// the surface centres itself in whatever space there is.
    fn hold_aspect(&mut self, ctx: &Context, frame: &eframe::Frame, chrome: f32) {
        // Set once and never revised: re-applying it fights the user's own resizes
        if !self.native_lock {
            let width = self.matched.map_or(BOARD_SIZE, |size| size.x);
            self.native_lock = aspect::enforce(frame, width, width / self.aspect + chrome);
        }
        if self.native_lock {
            self.matched = ctx.input(|i| i.viewport().inner_rect.map(|r| r.size()));
            return;
        }

        let (size, free) = ctx.input(|i| {
            let v = i.viewport();
            (
                v.inner_rect.map(|r| r.size()),
                !v.maximized.unwrap_or(false) && !v.fullscreen.unwrap_or(false),
            )
        });
        let Some(size) = size.filter(|_| free) else {
            self.matched = None;
            return;
        };

        let moved = self.matched.unwrap_or(size) - size;
        let wanted = if moved.x.abs() >= moved.y.abs() {
            Vec2::new(size.x, size.x / self.aspect + chrome)
        } else {
            Vec2::new((size.y - chrome).max(MIN_BOARD) * self.aspect, size.y)
        };
        let wanted = wanted.max(Vec2::new(MIN_BOARD, MIN_BOARD / self.aspect + chrome));

        if (wanted - size).abs().max_elem() > 1.0 {
            ctx.send_viewport_cmd(ViewportCommand::InnerSize(wanted));
        }
        self.matched = Some(wanted);
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
        ui.horizontal(|ui| {
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
    fn ui(&mut self, ui: &mut Ui, frame: &mut eframe::Frame) {
        self.pump();

        let phase = self.emulator.phase().unwrap_or(0.0);
        let bpm = self.emulator.bpm().unwrap_or(0.0);
        let Ok(surface) = self.emulator.surface() else {
            return;
        };

        let window = ui
            .ctx()
            .input(|i| i.viewport().inner_rect.map(|r| r.height()));
        let top = Panel::top("status").show(ui, |ui| self.status_bar(ui, bpm, &surface));

        // The surface takes the full width as a square, and the log absorbs whatever is left, so
        // no strip of background is ever exposed beside or below it
        let board = ui.available_width();
        let log = window.map_or(LOG_HEIGHT, |h| {
            (h - top.response.rect.height() - board).max(MIN_LOG)
        });
        let bottom = Panel::bottom("log")
            .exact_size(log)
            .show(ui, |ui| self.log_panel(ui));
        let chrome = top.response.rect.height() + bottom.response.rect.height();
        self.hold_aspect(ui.ctx(), frame, chrome);

        let board = CentralPanel::default()
            .frame(Frame::NONE)
            .show(ui, |ui| self.widget.show(ui, &self.layout, &surface, phase))
            .inner;
        self.send(board.inner);

        // Flashing and pulsing animate between host messages
        ui.ctx().request_repaint_after(Duration::from_millis(16));
    }
}
