//! A scrolling record of what a host has sent.

use std::collections::VecDeque;

use chrono::{DateTime, Local, TimeZone};
use egui::{ScrollArea, Ui};
use launchpad_emulator::HostMessage;

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

/// A scrolling record of what a host has sent, hidden until asked for.
///
/// Feed it [`Console::record`] for host messages, or [`Console::push`] for anything else. It draws
/// nothing while hidden, so a front end can call [`Console::show`] unconditionally.
#[derive(Debug, Clone)]
pub struct Console {
    entries: VecDeque<Entry>,
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
            entries: VecDeque::new(),
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
        self.entries.push_back(Entry {
            at: Local::now(),
            text: text.into(),
        });
        while self.entries.len() > self.limit {
            self.entries.pop_front();
        }
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
        self.entries.clear();
    }

    /// How many lines are held.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether no lines are held.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The lines held, oldest first, each stamped with the time it arrived.
    pub fn lines(&self) -> impl Iterator<Item = String> + use<'_> {
        self.entries
            .iter()
            .map(|entry| format!("{} {}", stamp(&entry.at), entry.text))
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
        ScrollArea::vertical()
            .stick_to_bottom(true)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for line in self.lines() {
                    ui.monospace(line);
                }
            });
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
        let line = console.lines().next().unwrap_or_default();
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
        let lines: Vec<String> = console.lines().collect();
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
            console
                .lines()
                .next()
                .unwrap_or_default()
                .contains("Brightness"),
            "the message should be recorded"
        );
    }
}
