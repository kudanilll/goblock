//! Camera and input abstraction.
//!
//! Desktop keyboard and mouse input implements [`InputSource`], and a touch
//! implementation can do the same. The [`Camera`] and game code only ever
//! see an [`InputState`].

pub mod camera;

pub use camera::Camera;

use glam::Vec2;

/// A device-independent snapshot of what the user wants to do.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct InputState {
    /// Movement on the horizontal plane, `x` = right, `y` = forward.
    pub move_axes: Vec2,
    /// Fly up (+) or down (-).
    pub vertical: f32,
    /// Mouse look delta in screen pixels for this frame.
    pub look_delta: Vec2,
    pub sprint: bool,
    pub quit: bool,
}

/// Anything that can produce an [`InputState`]: keyboard and mouse today,
/// touch later.
pub trait InputSource {
    /// Snapshot the current input. Called once per frame.
    fn poll(&mut self) -> InputState;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixed(InputState);

    impl InputSource for Fixed {
        fn poll(&mut self) -> InputState {
            self.0.clone()
        }
    }

    #[test]
    fn default_state_is_neutral() {
        let s = InputState::default();
        assert_eq!(s.move_axes, Vec2::ZERO);
        assert_eq!(s.vertical, 0.0);
        assert_eq!(s.look_delta, Vec2::ZERO);
        assert!(!s.sprint);
        assert!(!s.quit);
    }

    #[test]
    fn trait_object_can_be_driven() {
        let mut source: Box<dyn InputSource> = Box::new(Fixed(InputState {
            move_axes: Vec2::new(1.0, 0.0),
            sprint: true,
            ..Default::default()
        }));
        let state = source.poll();
        assert_eq!(state.move_axes, Vec2::new(1.0, 0.0));
        assert!(state.sprint);
    }
}
