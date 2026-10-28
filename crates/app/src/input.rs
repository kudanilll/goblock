//! Desktop input: keyboard and mouse mapped onto [`InputState`].

use std::collections::HashSet;

use glam::Vec2;
use goblock_input::{InputSource, InputState};
use winit::event::{DeviceEvent, ElementState, MouseButton, WindowEvent};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::Window;

/// Tracks raw device state and converts it to an [`InputState`] per frame.
pub struct DesktopInput {
    keys: HashSet<KeyCode>,
    mouse_captured: bool,
    look_delta: Vec2,
    first_mouse: bool,
}

impl DesktopInput {
    pub fn new() -> Self {
        Self {
            keys: HashSet::new(),
            mouse_captured: false,
            look_delta: Vec2::ZERO,
            first_mouse: true,
        }
    }

    pub fn set_cursor_grab(&self, window: &Window) {
        let _ = window
            .set_cursor_grab(winit::window::CursorGrabMode::Confined)
            .or_else(|_| window.set_cursor_grab(winit::window::CursorGrabMode::Locked));
        window.set_cursor_visible(false);
    }

    pub fn handle_window_event(&mut self, event: &WindowEvent) {
        match event {
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(code) = event.physical_key {
                    match event.state {
                        ElementState::Pressed => {
                            self.keys.insert(code);
                        }
                        ElementState::Released => {
                            self.keys.remove(&code);
                        }
                    }
                }
            }
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => {
                self.mouse_captured = *state == ElementState::Pressed;
                self.first_mouse = true;
            }
            WindowEvent::Focused(false) => {
                self.keys.clear();
            }
            _ => {}
        }
    }

    /// Raw mouse motion while the window is focused.
    pub fn handle_device_event(&mut self, event: &DeviceEvent) {
        if let DeviceEvent::MouseMotion { delta } = event {
            if self.first_mouse {
                self.first_mouse = false;
                return;
            }
            self.look_delta.x += delta.0 as f32;
            self.look_delta.y += delta.1 as f32;
        }
    }

    fn is_down(&self, code: KeyCode) -> bool {
        self.keys.contains(&code)
    }
}

impl Default for DesktopInput {
    fn default() -> Self {
        Self::new()
    }
}

impl InputSource for DesktopInput {
    fn poll(&mut self) -> InputState {
        let mut forward = 0.0;
        let mut strafe = 0.0;
        if self.is_down(KeyCode::KeyW) {
            forward += 1.0;
        }
        if self.is_down(KeyCode::KeyS) {
            forward -= 1.0;
        }
        if self.is_down(KeyCode::KeyD) {
            strafe += 1.0;
        }
        if self.is_down(KeyCode::KeyA) {
            strafe -= 1.0;
        }

        let mut vertical = 0.0;
        if self.is_down(KeyCode::Space) {
            vertical += 1.0;
        }
        if self.is_down(KeyCode::ControlLeft) || self.is_down(KeyCode::KeyC) {
            vertical -= 1.0;
        }

        let sprint = self.is_down(KeyCode::ShiftLeft) || self.is_down(KeyCode::ShiftRight);

        let look_delta = std::mem::take(&mut self.look_delta);

        InputState {
            move_axes: Vec2::new(strafe, forward),
            vertical,
            // Mouse Y grows downward on screen; camera pitch grows upward.
            look_delta: Vec2::new(look_delta.x, -look_delta.y),
            sprint,
            quit: self.is_down(KeyCode::Escape),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(input: &mut DesktopInput, code: KeyCode) {
        input.keys.insert(code);
    }

    fn release(input: &mut DesktopInput, code: KeyCode) {
        input.keys.remove(&code);
    }

    #[test]
    fn idle_produces_neutral_state() {
        let mut input = DesktopInput::new();
        let state = input.poll();
        assert_eq!(state.move_axes, Vec2::ZERO);
        assert_eq!(state.vertical, 0.0);
        assert!(!state.sprint);
        assert!(!state.quit);
    }

    #[test]
    fn wasd_maps_to_axes() {
        let mut input = DesktopInput::new();
        press(&mut input, KeyCode::KeyW);
        press(&mut input, KeyCode::KeyD);
        let state = input.poll();
        assert_eq!(state.move_axes, Vec2::new(1.0, 1.0));
    }

    #[test]
    fn opposite_keys_cancel() {
        let mut input = DesktopInput::new();
        press(&mut input, KeyCode::KeyW);
        press(&mut input, KeyCode::KeyS);
        assert_eq!(input.poll().move_axes.y, 0.0);
    }

    #[test]
    fn space_and_control_are_vertical() {
        let mut input = DesktopInput::new();
        press(&mut input, KeyCode::Space);
        assert_eq!(input.poll().vertical, 1.0);
        release(&mut input, KeyCode::Space);
        press(&mut input, KeyCode::ControlLeft);
        assert_eq!(input.poll().vertical, -1.0);
    }

    #[test]
    fn shift_is_sprint() {
        let mut input = DesktopInput::new();
        press(&mut input, KeyCode::ShiftLeft);
        assert!(input.poll().sprint);
    }

    #[test]
    fn escape_requests_quit() {
        let mut input = DesktopInput::new();
        press(&mut input, KeyCode::Escape);
        assert!(input.poll().quit);
    }

    #[test]
    fn look_delta_is_consumed_and_inverted() {
        let mut input = DesktopInput::new();
        input.first_mouse = false;
        input.handle_device_event(&DeviceEvent::MouseMotion { delta: (3.0, 5.0) });
        let state = input.poll();
        assert_eq!(state.look_delta, Vec2::new(3.0, -5.0));
        assert_eq!(input.poll().look_delta, Vec2::ZERO);
    }
}
