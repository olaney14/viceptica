use std::collections::HashMap;

use winit::{event::MouseButton, keyboard::Key};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum KeyState {
    Pressed,
    Released,
    JustPressed
}

/// Input manager, must be updated externally with `on_key_released` and `on_key_pressed`. `update` must be called every frame.
pub struct Input {
    pub keys: HashMap<Key, KeyState>,
    pub mouse_buttons: HashMap<MouseButton, KeyState>,
    pub needs_update: bool,
    pub mouse_pos: (f64, f64),
    pub scroll: f32
}

impl Input {
    pub fn new() -> Self {
        Input {
            keys: HashMap::new(),
            mouse_buttons: HashMap::new(),
            needs_update: false,
            mouse_pos: (0.0, 0.0),
            scroll: 0.0
        }
    }

    /// Call when a key is pressed in your window manager loop
    pub fn on_key_pressed(&mut self, key: Key) {
        self.keys.insert(key, KeyState::JustPressed);
        self.needs_update = true;
    }

    pub fn on_mouse_moved(&mut self, x: f64, y: f64) {
        self.mouse_pos = (x, y);
    }

    /// Call when a key is released in your window manager loop
    pub fn on_key_released(&mut self, key: Key) {
        self.keys.insert(key, KeyState::Released);
    }

    pub fn on_mouse_button_pressed(&mut self, button: MouseButton) {
        self.mouse_buttons.insert(button, KeyState::JustPressed);
        self.needs_update = true;
    }

    pub fn on_mouse_button_released(&mut self, button: MouseButton) {
        self.mouse_buttons.insert(button, KeyState::Released);
    }

    pub fn set_scroll(&mut self, scroll: f32) {
        self.scroll = scroll;
        self.needs_update = true;
    }

    /// Call every frame after this struct is done being used, resets `JustPressed` keystates to `Pressed`
    pub fn update(&mut self) {
        if self.needs_update {
            for (_, state) in self.keys.iter_mut() {
                if *state == KeyState::JustPressed {
                    *state = KeyState::Pressed;
                }
            }
            for (_, state) in self.mouse_buttons.iter_mut() {
                if *state == KeyState::JustPressed {
                    *state = KeyState::Pressed;
                }
            }
            self.scroll = 0.0;
            self.needs_update = false;
        }
    }

    /// Return true if `key` is `Pressed` or `JustPressed`
    pub fn get_key_pressed(&self, key: Key) -> bool {
        if let Some(state) = self.keys.get(&key) {
            return *state == KeyState::JustPressed || *state == KeyState::Pressed;
        }

        false
    }

    /// Return true only if `key` is `JustPressed`
    pub fn get_key_just_pressed(&self, key: Key) -> bool {
        if let Some(state) = self.keys.get(&key) {
            return *state == KeyState::JustPressed;
        }

        false
    }

    /// Return true only if `key` is `Released`
    pub fn get_key_released(&self, key: Key) -> bool {
        if let Some(state) = self.keys.get(&key) {
            return *state == KeyState::Released;
        }

        true
    }

    pub fn get_mouse_button_pressed(&self, button: MouseButton) -> bool {
        *self.mouse_buttons.get(&button).unwrap_or(&KeyState::Released) != KeyState::Released
    }

    pub fn get_mouse_button_just_pressed(&self, button: MouseButton) -> bool {
        *self.mouse_buttons.get(&button).unwrap_or(&KeyState::Released) == KeyState::JustPressed
    }

    pub fn get_mouse_button_released(&self, button: MouseButton) -> bool {
        *self.mouse_buttons.get(&button).unwrap_or(&KeyState::Released) == KeyState::Released
    }
}