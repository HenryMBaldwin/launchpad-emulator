//! What kind of control occupies a position on a surface.

/// The kind of control at a position, which front ends draw differently.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PadRole {
    /// One of the square pads of the central grid.
    Grid,
    /// One of the round buttons along the edges.
    Control,
    /// The logo, which lights but never reports a press.
    Logo,
    /// Nothing, for positions the device does not have.
    Absent,
}

impl PadRole {
    /// Whether a control of this kind reports presses.
    #[must_use]
    pub const fn is_button(self) -> bool {
        matches!(self, Self::Grid | Self::Control)
    }

    /// Whether a control of this kind lights up.
    #[must_use]
    pub const fn is_lit(self) -> bool {
        !matches!(self, Self::Absent)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_grid_and_control_report_presses() {
        assert!(PadRole::Grid.is_button());
        assert!(PadRole::Control.is_button());
        assert!(!PadRole::Logo.is_button());
        assert!(!PadRole::Absent.is_button());
    }

    #[test]
    fn everything_present_lights_up() {
        assert!(PadRole::Logo.is_lit());
        assert!(PadRole::Grid.is_lit());
        assert!(!PadRole::Absent.is_lit());
    }
}
