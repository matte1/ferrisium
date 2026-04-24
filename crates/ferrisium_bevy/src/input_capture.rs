//! Shared input-capture state for UI integrations.
//!
//! Ferrisium should not know about every possible UI framework. Instead, UI
//! integrations mark pointer input as captured for the current frame before
//! Ferrisium's input systems run.

use bevy::prelude::*;

/// Per-frame input capture state respected by Ferrisium pointer consumers.
///
/// Systems that render UI over a Ferrisium view can set [`Self::pointer`] to
/// `true` before Ferrisium input runs. Ferrisium resets this resource every
/// update frame, so multiple UI systems can safely OR into it without leaving
/// stale capture state behind.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct FerrisiumInputCapture {
    /// Whether Ferrisium should ignore pointer-driven controls this frame.
    pub pointer: bool,
}

impl FerrisiumInputCapture {
    /// Marks pointer input as captured for the current frame.
    pub const fn capture_pointer(&mut self) {
        self.pointer = true;
    }

    /// Clears all capture flags.
    pub const fn reset(&mut self) {
        self.pointer = false;
    }

    /// Returns whether Ferrisium pointer input is captured this frame.
    #[must_use]
    pub const fn pointer_captured(self) -> bool {
        self.pointer
    }
}

pub(crate) fn reset_ferrisium_input_capture(mut capture: ResMut<'_, FerrisiumInputCapture>) {
    capture.reset();
}

#[cfg(test)]
mod tests {
    use super::FerrisiumInputCapture;

    #[test]
    fn input_capture_defaults_to_uncaptured() {
        assert!(!FerrisiumInputCapture::default().pointer_captured());
    }

    #[test]
    fn input_capture_can_be_reset_each_frame() {
        let mut capture = FerrisiumInputCapture::default();

        capture.capture_pointer();
        assert!(capture.pointer_captured());
        capture.reset();

        assert!(!capture.pointer_captured());
    }
}
