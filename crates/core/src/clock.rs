//! The beat the host's clock defines, which flashing and pulsing follow.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// MIDI clock ticks per beat.
pub const TICKS_PER_BEAT: u16 = 24;

/// Tempo assumed until the host sends clock ticks.
pub const DEFAULT_BPM: f32 = 120.0;

/// How long a tick-driven clock keeps running after the ticks stop.
const TIMEOUT: Duration = Duration::from_millis(750);

/// Beats of ticks kept to measure tempo across.
const HISTORY_BEATS: usize = 4;

/// Ticks kept to measure tempo across.
const HISTORY: usize = HISTORY_BEATS * TICKS_PER_BEAT as usize + 1;

/// Tracks the beat that flashing and pulsing are synchronised to.
///
/// Feed it [`Self::tick`] for every clock tick the host sends. Without ticks it free-runs at
/// [`DEFAULT_BPM`], matching what the hardware does.
#[derive(Debug, Clone)]
pub struct Clock {
    epoch: Instant,
    beat_seconds: f32,
    last_tick: Option<Instant>,
    tick_interval: Option<f32>,
    /// Ticks counted since the host started sending them.
    ticks: u64,
    /// Recent tick instants, oldest first.
    history: VecDeque<Instant>,
}

impl Clock {
    /// Creates a clock free-running at [`DEFAULT_BPM`] from `now`.
    #[must_use]
    pub fn new(now: Instant) -> Self {
        Self {
            epoch: now,
            beat_seconds: 60.0 / DEFAULT_BPM,
            last_tick: None,
            tick_interval: None,
            ticks: 0,
            history: VecDeque::with_capacity(HISTORY),
        }
    }

    /// Records a clock tick from the host.
    ///
    /// Tempo is measured across the whole window rather than between consecutive ticks, so the
    /// jitter in any one interval does not move the reported tempo.
    pub fn tick(&mut self, at: Instant) {
        self.history.push_back(at);
        while self.history.len() > HISTORY {
            self.history.pop_front();
        }
        if let (Some(&first), Some(&last)) = (self.history.front(), self.history.back()) {
            let intervals = self.history.len().saturating_sub(1);
            let span = last.saturating_duration_since(first).as_secs_f32();
            if intervals > 0 && span > 0.0 {
                #[allow(clippy::cast_precision_loss)]
                let interval = span / intervals as f32;
                self.tick_interval = Some(interval);
                self.beat_seconds = interval * f32::from(TICKS_PER_BEAT);
            }
        }
        self.last_tick = Some(at);
        self.ticks += 1;
    }

    /// Whether the host is currently driving the clock.
    #[must_use]
    pub fn is_driven(&self, at: Instant) -> bool {
        self.last_tick
            .is_some_and(|last| at.saturating_duration_since(last) < TIMEOUT)
    }

    /// Tempo in beats per minute.
    #[must_use]
    pub fn bpm(&self) -> f32 {
        if self.beat_seconds > 0.0 {
            60.0 / self.beat_seconds
        } else {
            DEFAULT_BPM
        }
    }

    /// Beats elapsed at `at`, counting up without wrapping.
    ///
    /// Effects with a period longer than one beat need the whole position, not just where in the
    /// current beat we are.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn beats(&self, at: Instant) -> f32 {
        match (self.is_driven(at), self.last_tick, self.tick_interval) {
            // Interpolate between ticks so the beat advances smoothly
            (true, Some(last), Some(interval)) if interval > 0.0 => {
                let since = at.saturating_duration_since(last).as_secs_f32() / interval;
                (self.ticks as f32 + since.min(1.0)) / f32::from(TICKS_PER_BEAT)
            }
            (true, _, _) => self.ticks as f32 / f32::from(TICKS_PER_BEAT),
            _ => at.saturating_duration_since(self.epoch).as_secs_f32() / self.beat_seconds,
        }
    }

    /// How far through the current beat `at` falls, in `0.0..1.0`.
    #[must_use]
    pub fn phase(&self, at: Instant) -> f32 {
        self.beats(at).rem_euclid(1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Interval between ticks at a given tempo.
    fn tick_gap(bpm: f32) -> Duration {
        Duration::from_secs_f32(60.0 / bpm / f32::from(TICKS_PER_BEAT))
    }

    #[test]
    fn a_fresh_clock_free_runs_at_the_default_tempo() {
        let start = Instant::now();
        let clock = Clock::new(start);
        assert!((clock.bpm() - DEFAULT_BPM).abs() < 0.01);
        assert!(!clock.is_driven(start));
        assert!(clock.phase(start).abs() < 0.001);
    }

    #[test]
    fn a_free_running_beat_lasts_half_a_second_at_120_bpm() {
        let start = Instant::now();
        let clock = Clock::new(start);
        assert!((clock.phase(start + Duration::from_millis(250)) - 0.5).abs() < 0.01);
        assert!(clock.phase(start + Duration::from_millis(500)) < 0.01);
    }

    #[test]
    fn ticks_take_over_the_tempo() {
        let start = Instant::now();
        let mut clock = Clock::new(start);
        let gap = tick_gap(60.0);
        let mut at = start;
        for _ in 0..(TICKS_PER_BEAT * 8) {
            clock.tick(at);
            at += gap;
        }
        assert!(clock.is_driven(at));
        assert!((clock.bpm() - 60.0).abs() < 1.0, "bpm was {}", clock.bpm());
    }

    #[test]
    fn phase_advances_across_a_tick_driven_beat() {
        let start = Instant::now();
        let mut clock = Clock::new(start);
        let gap = tick_gap(120.0);
        let mut at = start;
        // A full beat of ticks returns the phase to the start of the next beat
        for _ in 0..TICKS_PER_BEAT {
            clock.tick(at);
            at += gap;
        }
        assert!(clock.phase(at) < 0.05, "phase was {}", clock.phase(at));

        for _ in 0..(TICKS_PER_BEAT / 2) {
            clock.tick(at);
            at += gap;
        }
        let mid = clock.phase(at);
        assert!((0.45..0.6).contains(&mid), "phase was {mid}");
    }

    /// Jitter on individual intervals must not move the reported tempo
    #[test]
    fn tempo_is_steady_under_jittery_ticks() {
        let start = Instant::now();
        let mut clock = Clock::new(start);
        let gap = tick_gap(120.0).as_secs_f64();
        let mut at = start;
        let mut worst: f32 = 0.0;
        // A deterministic wobble of about a quarter of the gap, alternating either way
        for i in 0..(TICKS_PER_BEAT * 16) {
            let wobble = if i % 2 == 0 { 0.25 } else { -0.25 };
            at += Duration::from_secs_f64(gap * (1.0 + wobble));
            clock.tick(at);
            if i as usize >= HISTORY {
                worst = worst.max((clock.bpm() - 120.0).abs());
            }
        }
        assert!(worst < 2.0, "tempo drifted by {worst} bpm under jitter");
    }

    #[test]
    fn the_clock_free_runs_again_once_ticks_stop() {
        let start = Instant::now();
        let mut clock = Clock::new(start);
        clock.tick(start);
        assert!(clock.is_driven(start));
        assert!(!clock.is_driven(start + TIMEOUT + Duration::from_millis(1)));
    }

    #[test]
    fn phase_always_stays_within_one_beat() {
        let start = Instant::now();
        let clock = Clock::new(start);
        for ms in [0, 1, 250, 499, 500, 1000, 12_345] {
            let phase = clock.phase(start + Duration::from_millis(ms));
            assert!((0.0..1.0).contains(&phase), "phase {phase} at {ms}ms");
        }
    }
}
