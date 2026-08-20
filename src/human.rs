use std::time::Duration;

use crate::{TouchAction, TouchContext, TouchProfile};

/// Human behavior profile.
///
/// In this initial version, it preserves the original action.
/// Behaviors regarding timing, micro-movement, and jitter
/// will be added incrementally.
#[derive(Clone, Debug)]
pub struct HumanProfile {
    /// Initial delay before a DOWN.
    pub down_delay: Duration,

    /// Delay before an UP.
    pub up_delay: Duration,

    /// Delay between movements.
    pub move_delay: Duration,
}

impl Default for HumanProfile {
    fn default() -> Self {
        Self {
            down_delay: Duration::ZERO,

            up_delay: Duration::ZERO,

            move_delay: Duration::ZERO,
        }
    }
}

impl HumanProfile {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_down_delay(mut self, delay: Duration) -> Self {
        self.down_delay = delay;

        self
    }

    pub fn with_up_delay(mut self, delay: Duration) -> Self {
        self.up_delay = delay;

        self
    }

    pub fn with_move_delay(mut self, delay: Duration) -> Self {
        self.move_delay = delay;

        self
    }
}

impl TouchProfile for HumanProfile {
    fn process(
        &mut self,
        action: TouchAction,
        _context: TouchContext,
    ) -> Vec<(TouchAction, Duration)> {
        let delay = match action {
            TouchAction::Down(_) => self.down_delay,

            TouchAction::Move(_) => self.move_delay,

            TouchAction::Up => self.up_delay,
        };

        vec![(action, delay)]
    }

    fn reset(&mut self) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Point;

    fn ctx() -> TouchContext {
        TouchContext { display_width: 720, display_height: 1600 }
    }

    #[test]
    fn default_all_zero() {
        let p = HumanProfile::default();
        assert_eq!(p.down_delay, Duration::ZERO);
        assert_eq!(p.up_delay, Duration::ZERO);
        assert_eq!(p.move_delay, Duration::ZERO);
    }

    #[test]
    fn new_equals_default() {
        let a = HumanProfile::new();
        let b = HumanProfile::default();
        assert_eq!(a.down_delay, b.down_delay);
        assert_eq!(a.up_delay, b.up_delay);
        assert_eq!(a.move_delay, b.move_delay);
    }

    #[test]
    fn builder_down_delay() {
        let p = HumanProfile::new().with_down_delay(Duration::from_millis(50));
        assert_eq!(p.down_delay, Duration::from_millis(50));
        assert_eq!(p.up_delay, Duration::ZERO);
        assert_eq!(p.move_delay, Duration::ZERO);
    }

    #[test]
    fn builder_up_delay() {
        let p = HumanProfile::new().with_up_delay(Duration::from_millis(30));
        assert_eq!(p.down_delay, Duration::ZERO);
        assert_eq!(p.up_delay, Duration::from_millis(30));
        assert_eq!(p.move_delay, Duration::ZERO);
    }

    #[test]
    fn builder_move_delay() {
        let p = HumanProfile::new().with_move_delay(Duration::from_millis(10));
        assert_eq!(p.down_delay, Duration::ZERO);
        assert_eq!(p.up_delay, Duration::ZERO);
        assert_eq!(p.move_delay, Duration::from_millis(10));
    }

    #[test]
    fn builder_chaining() {
        let p = HumanProfile::new()
            .with_down_delay(Duration::from_millis(100))
            .with_up_delay(Duration::from_millis(200))
            .with_move_delay(Duration::from_millis(50));
        assert_eq!(p.down_delay, Duration::from_millis(100));
        assert_eq!(p.up_delay, Duration::from_millis(200));
        assert_eq!(p.move_delay, Duration::from_millis(50));
    }

    #[test]
    fn process_down_uses_down_delay() {
        let mut p = HumanProfile::new().with_down_delay(Duration::from_millis(75));
        let result = p.process(TouchAction::Down(Point::new(10, 20)), ctx());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, TouchAction::Down(Point::new(10, 20)));
        assert_eq!(result[0].1, Duration::from_millis(75));
    }

    #[test]
    fn process_move_uses_move_delay() {
        let mut p = HumanProfile::new().with_move_delay(Duration::from_millis(25));
        let result = p.process(TouchAction::Move(Point::new(30, 40)), ctx());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, TouchAction::Move(Point::new(30, 40)));
        assert_eq!(result[0].1, Duration::from_millis(25));
    }

    #[test]
    fn process_up_uses_up_delay() {
        let mut p = HumanProfile::new().with_up_delay(Duration::from_millis(40));
        let result = p.process(TouchAction::Up, ctx());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, TouchAction::Up);
        assert_eq!(result[0].1, Duration::from_millis(40));
    }

    #[test]
    fn process_preserves_action() {
        let mut p = HumanProfile::new();
        let action = TouchAction::Down(Point::new(99, 88));
        let result = p.process(action, ctx());
        assert_eq!(result[0].0, action);
    }

    #[test]
    fn process_returns_single_action() {
        let mut p = HumanProfile::new();
        let result = p.process(TouchAction::Up, ctx());
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn reset_is_noop() {
        let mut p = HumanProfile::new().with_down_delay(Duration::from_secs(1));
        p.reset();
        assert_eq!(p.down_delay, Duration::from_secs(1));
    }

    #[test]
    fn clone() {
        let p = HumanProfile::new()
            .with_down_delay(Duration::from_millis(10))
            .with_up_delay(Duration::from_millis(20))
            .with_move_delay(Duration::from_millis(30));
        let c = p.clone();
        assert_eq!(p.down_delay, c.down_delay);
        assert_eq!(p.up_delay, c.up_delay);
        assert_eq!(p.move_delay, c.move_delay);
    }

    #[test]
    fn debug() {
        let p = HumanProfile::new();
        let d = format!("{p:?}");
        assert!(d.contains("HumanProfile"));
        assert!(d.contains("down_delay"));
        assert!(d.contains("up_delay"));
        assert!(d.contains("move_delay"));
    }

    #[test]
    fn zero_delay_still_returns_action() {
        let mut p = HumanProfile::new();
        let result = p.process(TouchAction::Down(Point::new(0, 0)), ctx());
        assert_eq!(result[0].1, Duration::ZERO);
    }

    #[test]
    fn large_delay() {
        let mut p = HumanProfile::new().with_down_delay(Duration::from_secs(10));
        let result = p.process(TouchAction::Down(Point::new(0, 0)), ctx());
        assert_eq!(result[0].1, Duration::from_secs(10));
    }
}
