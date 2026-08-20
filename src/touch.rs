use std::io;
use std::time::Duration;

use crate::multiplexer::{Point, TouchMultiplexer};

/// Abstract touch action.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TouchAction {
    Down(Point),
    Move(Point),
    Up,
}

/// Context available for a profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TouchContext {
    pub display_width: i32,
    pub display_height: i32,
}

/// Touch behavior profile.
pub trait TouchProfile {
    /// Processes a sequence of actions.
    ///
    /// The profile can:
    ///
    /// - modify coordinates;
    /// - insert movements;
    /// - alter timing;
    /// - generate multiple actions;
    /// - discard actions.
    fn process(
        &mut self,
        action: TouchAction,
        context: TouchContext,
    ) -> Vec<(TouchAction, Duration)>;

    /// Called when the profile stops processing an
    /// active sequence.
    fn reset(&mut self) {}
}

/// High-level controller for profiles.
pub struct TouchController<P> {
    multiplexer: TouchMultiplexer,
    profile: P,
}

impl<P> TouchController<P>
where
    P: TouchProfile,
{
    pub fn new(multiplexer: TouchMultiplexer, profile: P) -> Self {
        Self {
            multiplexer,
            profile,
        }
    }

    pub fn multiplexer(&self) -> &TouchMultiplexer {
        &self.multiplexer
    }

    pub fn multiplexer_mut(&mut self) -> &mut TouchMultiplexer {
        &mut self.multiplexer
    }

    pub fn profile(&self) -> &P {
        &self.profile
    }

    pub fn profile_mut(&mut self) -> &mut P {
        &mut self.profile
    }

    pub fn poll(&mut self) -> io::Result<u64> {
        self.multiplexer.poll()
    }

    pub fn execute(&mut self, action: TouchAction) -> io::Result<()> {
        let display = self.multiplexer.display_size();

        let context = TouchContext {
            display_width: display.width,

            display_height: display.height,
        };

        let actions = self.profile.process(action, context);

        for (action, delay) in actions {
            self.execute_action(action)?;

            if !delay.is_zero() {
                std::thread::sleep(delay);
            }
        }

        Ok(())
    }

    pub fn touch_down(&mut self, point: crate::Point) -> io::Result<()> {
        self.execute(TouchAction::Down(point))
    }

    pub fn touch_move(&mut self, point: crate::Point) -> io::Result<()> {
        self.execute(TouchAction::Move(point))
    }

    pub fn touch_up(&mut self) -> io::Result<()> {
        self.execute(TouchAction::Up)
    }

    pub fn tap(&mut self, point: crate::Point) -> io::Result<()> {
        self.touch_down(point)?;

        self.touch_up()
    }

    fn execute_action(&mut self, action: TouchAction) -> io::Result<()> {
        match action {
            TouchAction::Down(point) => self.multiplexer.touch_down(point),

            TouchAction::Move(point) => self.multiplexer.touch_move(point),

            TouchAction::Up => self.multiplexer.touch_up(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn touch_action_down_eq() {
        let a = TouchAction::Down(Point::new(100, 200));
        let b = TouchAction::Down(Point::new(100, 200));
        assert_eq!(a, b);
    }

    #[test]
    fn touch_action_down_neq_different_point() {
        let a = TouchAction::Down(Point::new(100, 200));
        let b = TouchAction::Down(Point::new(300, 400));
        assert_ne!(a, b);
    }

    #[test]
    fn touch_action_move_eq() {
        let a = TouchAction::Move(Point::new(10, 20));
        let b = TouchAction::Move(Point::new(10, 20));
        assert_eq!(a, b);
    }

    #[test]
    fn touch_action_up_eq() {
        assert_eq!(TouchAction::Up, TouchAction::Up);
    }

    #[test]
    fn touch_action_different_variants_neq() {
        let down = TouchAction::Down(Point::new(0, 0));
        let mov = TouchAction::Move(Point::new(0, 0));
        let up = TouchAction::Up;
        assert_ne!(down, mov);
        assert_ne!(down, up);
        assert_ne!(mov, up);
    }

    #[test]
    fn touch_action_clone() {
        let a = TouchAction::Down(Point::new(50, 60));
        let b = a;
        assert_eq!(a, b);
    }

    #[test]
    fn touch_action_copy() {
        let a = TouchAction::Move(Point::new(1, 2));
        let b = a;
        assert_eq!(a, b);
    }

    #[test]
    fn touch_action_debug() {
        let action = TouchAction::Down(Point::new(1, 2));
        let debug = format!("{action:?}");
        assert!(debug.contains("Down"));
        assert!(debug.contains("1"));
        assert!(debug.contains("2"));
    }

    #[test]
    fn touch_context_new() {
        let ctx = TouchContext {
            display_width: 1080,
            display_height: 2400,
        };
        assert_eq!(ctx.display_width, 1080);
        assert_eq!(ctx.display_height, 2400);
    }

    #[test]
    fn touch_context_eq() {
        let a = TouchContext { display_width: 720, display_height: 1600 };
        let b = TouchContext { display_width: 720, display_height: 1600 };
        assert_eq!(a, b);
    }

    #[test]
    fn touch_context_neq() {
        let a = TouchContext { display_width: 720, display_height: 1600 };
        let b = TouchContext { display_width: 1080, display_height: 2400 };
        assert_ne!(a, b);
    }

    #[test]
    fn touch_context_clone() {
        let a = TouchContext { display_width: 100, display_height: 200 };
        let b = a;
        assert_eq!(a, b);
    }

    #[test]
    fn touch_context_copy() {
        let a = TouchContext { display_width: 50, display_height: 60 };
        let b = a;
        assert_eq!(a, b);
    }

    #[test]
    fn touch_context_debug() {
        let ctx = TouchContext { display_width: 720, display_height: 1600 };
        let debug = format!("{ctx:?}");
        assert!(debug.contains("720"));
        assert!(debug.contains("1600"));
    }
}
