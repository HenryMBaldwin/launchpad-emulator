//! A scrolling record of what a host has sent and what the program has reported.

use std::collections::VecDeque;
use std::fmt::Write as _;
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Local, TimeZone};
use egui::{ScrollArea, Ui};
use launchpad_emulator::HostMessage;
use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::Layer;
use tracing_subscriber::layer::Context;

/// Lines kept before the oldest are dropped.
pub const DEFAULT_LIMIT: usize = 200;

/// Height the console asks for.
pub const DEFAULT_HEIGHT: f32 = 170.0;

/// One line, stamped with the time it arrived.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Entry {
    at: DateTime<Local>,
    text: String,
}

/// The lines, shared with the [`Layer`] that writes into them.
type Lines = Arc<Mutex<VecDeque<Entry>>>;

/// A scrolling record of what a host has sent, hidden until asked for.
///
/// Feed it [`Console::record`] for host messages and [`Console::push`] for anything else, or install
/// [`Console::layer`] and let every `tracing` event in the program arrive on its own. It draws
/// nothing while hidden, so a front end can call [`Console::show`] unconditionally.
#[derive(Debug, Clone)]
pub struct Console {
    lines: Lines,
    limit: usize,
    height: f32,
    visible: bool,
}

impl Default for Console {
    fn default() -> Self {
        Self::new()
    }
}

impl Console {
    /// Creates a hidden console.
    #[must_use]
    pub fn new() -> Self {
        Self {
            lines: Lines::default(),
            limit: DEFAULT_LIMIT,
            height: DEFAULT_HEIGHT,
            visible: false,
        }
    }

    /// Sets whether the console starts out showing.
    #[must_use]
    pub const fn with_visible(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }

    /// Sets how many lines are kept.
    #[must_use]
    pub const fn with_limit(mut self, limit: usize) -> Self {
        self.limit = limit;
        self
    }

    /// Sets the height the console asks for when it draws.
    #[must_use]
    pub const fn with_height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }

    /// A `tracing` layer that records every event into this console.
    ///
    /// Install it on a subscriber and anything the program traces arrives here, including events
    /// from crates the front end brings itself.
    #[must_use]
    pub fn layer<S: Subscriber>(&self) -> ConsoleLayer<S> {
        ConsoleLayer {
            lines: Arc::clone(&self.lines),
            limit: self.limit,
            subscriber: std::marker::PhantomData,
        }
    }

    /// Whether the console is showing.
    #[must_use]
    pub const fn is_visible(&self) -> bool {
        self.visible
    }

    /// Shows or hides the console.
    pub const fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    /// Hides the console if it is showing, and shows it if it is not.
    pub const fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    /// Records a line.
    pub fn push(&mut self, text: impl Into<String>) {
        record_line(&self.lines, self.limit, text.into());
    }

    /// Records a message from the host.
    ///
    /// Lighting and clock messages arrive continuously and are skipped.
    pub fn record(&mut self, message: &HostMessage) {
        if matches!(message, HostMessage::Lighting { .. } | HostMessage::Clock) {
            return;
        }
        self.push(format!("{message:?}"));
    }

    /// Drops every line.
    pub fn clear(&mut self) {
        if let Ok(mut lines) = self.lines.lock() {
            lines.clear();
        }
    }

    /// How many lines are held.
    #[must_use]
    pub fn len(&self) -> usize {
        self.lines.lock().map_or(0, |lines| lines.len())
    }

    /// Whether no lines are held.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The lines held, oldest first, each stamped with the time it arrived.
    #[must_use]
    pub fn lines(&self) -> Vec<String> {
        self.lines.lock().map_or_else(
            |_| Vec::new(),
            |lines| {
                lines
                    .iter()
                    .map(|entry| format!("{} {}", stamp(&entry.at), entry.text))
                    .collect()
            },
        )
    }

    /// Draws the console, and nothing at all while it is hidden.
    ///
    /// Asks for its own height, since a panel drawn inside a [`Ui`] takes the height of its
    /// contents.
    pub fn show(&mut self, ui: &mut Ui) {
        if !self.visible {
            return;
        }
        ui.separator();
        ui.set_min_height(self.height);
        let lines = self.lines();
        ScrollArea::vertical()
            .stick_to_bottom(true)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for line in lines {
                    ui.monospace(line);
                }
            });
    }
}

/// A `tracing` layer writing events into a [`Console`].
#[derive(Debug)]
pub struct ConsoleLayer<S> {
    lines: Lines,
    limit: usize,
    subscriber: std::marker::PhantomData<fn(S)>,
}

impl<S: Subscriber> Layer<S> for ConsoleLayer<S> {
    fn on_event(&self, event: &Event<'_>, _context: Context<'_, S>) {
        let mut fields = Fields::default();
        event.record(&mut fields);
        let level = short_level(*event.metadata().level());
        record_line(&self.lines, self.limit, format!("{level} {}", fields.text));
    }
}

/// The fields of a `tracing` event, flattened into one line.
#[derive(Default)]
struct Fields {
    text: String,
}

impl Visit for Fields {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if !self.text.is_empty() {
            self.text.push(' ');
        }
        if field.name() == "message" {
            let _ = write!(self.text, "{value:?}");
        } else {
            let _ = write!(self.text, "{}={value:?}", field.name());
        }
    }
}

/// Adds a line, dropping the oldest once `limit` is reached.
fn record_line(lines: &Lines, limit: usize, text: String) {
    if let Ok(mut lines) = lines.lock() {
        lines.push_back(Entry {
            at: Local::now(),
            text,
        });
        while lines.len() > limit {
            lines.pop_front();
        }
    }
}

/// The single letter a level is shown as.
const fn short_level(level: Level) -> &'static str {
    match level {
        Level::ERROR => "E",
        Level::WARN => "W",
        Level::INFO => "I",
        Level::DEBUG => "D",
        Level::TRACE => "T",
    }
}

/// Renders an arrival time as `[hh:mm:ss.mmm]` in the system's own zone.
fn stamp<Tz>(at: &DateTime<Tz>) -> String
where
    Tz: TimeZone,
    Tz::Offset: std::fmt::Display,
{
    at.format("[%H:%M:%S%.3f]").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use launchpad_emulator::{Lighting, Pad, Rgb};
    use tracing_subscriber::layer::SubscriberExt as _;

    #[test]
    fn a_new_console_is_hidden_and_empty() {
        let console = Console::new();
        assert!(!console.is_visible());
        assert!(console.is_empty());
    }

    #[test]
    fn visibility_can_be_set_and_toggled() {
        let mut console = Console::new().with_visible(true);
        assert!(console.is_visible());
        console.toggle();
        assert!(!console.is_visible());
        console.set_visible(true);
        assert!(console.is_visible());
    }

    #[test]
    fn lines_carry_a_stamp_in_brackets() {
        let mut console = Console::new();
        console.push("hello");
        let lines = console.lines();
        let line = lines.first().map(String::as_str).unwrap_or_default();
        assert!(line.starts_with('['), "got {line:?}");
        assert!(line.ends_with(" hello"), "got {line:?}");
    }

    #[test]
    fn stamps_read_as_the_time_of_day() -> Result<(), chrono::ParseError> {
        let at = DateTime::parse_from_rfc3339("2026-08-10T14:23:05.123+01:00")?;
        assert_eq!(stamp(&at), "[14:23:05.123]");
        Ok(())
    }

    #[test]
    fn the_oldest_lines_are_dropped_once_full() {
        let mut console = Console::new().with_limit(2);
        for line in ["one", "two", "three"] {
            console.push(line);
        }
        assert_eq!(console.len(), 2);
        let lines = console.lines();
        assert!(lines[0].ends_with("two"), "got {lines:?}");
        assert!(lines[1].ends_with("three"), "got {lines:?}");
    }

    #[test]
    fn clearing_drops_everything() {
        let mut console = Console::new();
        console.push("hello");
        console.clear();
        assert!(console.is_empty());
    }

    #[test]
    fn the_messages_that_arrive_constantly_are_skipped() {
        let mut console = Console::new();
        console.record(&HostMessage::Clock);
        console.record(&HostMessage::Lighting {
            pad: Pad::new(0, 0),
            lighting: Lighting::Static(Rgb::BLACK),
        });
        assert!(console.is_empty(), "lighting and clock should be skipped");

        console.record(&HostMessage::Brightness(64));
        assert_eq!(console.len(), 1);
        assert!(
            console.lines()[0].contains("Brightness"),
            "the message should be recorded"
        );
    }

    #[test]
    fn traced_events_arrive_with_their_level_and_fields() {
        let console = Console::new();
        let subscriber = tracing_subscriber::registry().with(console.layer());
        tracing::subscriber::with_default(subscriber, || {
            tracing::warn!(port = "LPX", "could not send");
        });

        assert_eq!(console.len(), 1, "the event should have been recorded");
        let line = &console.lines()[0];
        assert!(line.contains(" W "), "the level should show: {line:?}");
        assert!(line.contains("could not send"), "got {line:?}");
        assert!(line.contains("port=\"LPX\""), "got {line:?}");
    }

    #[test]
    fn a_layer_shares_the_console_limit() {
        let console = Console::new().with_limit(1);
        let subscriber = tracing_subscriber::registry().with(console.layer());
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!("first");
            tracing::info!("second");
        });
        assert_eq!(console.len(), 1);
        assert!(console.lines()[0].contains("second"));
    }
}
