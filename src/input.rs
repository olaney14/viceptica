use std::collections::HashMap;

use winit::keyboard::Key;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum KeyState {
    Pressed,
    Released,
    JustPressed
}

/// Input manager, must be updated externally with `on_key_released` and `on_key_pressed`. `update` must be called every frame.
pub struct Input {
    pub keys: HashMap<Key, KeyState>,
    pub needs_update: bool
}

impl Input {
    pub fn new() -> Self {
        Input {
            keys: HashMap::new(),
            needs_update: false
        }
    }

    /// Call when a key is pressed in your window manager loop
    pub fn on_key_pressed(&mut self, key: Key) {
        self.keys.insert(key, KeyState::JustPressed);
        self.needs_update = true;
    }

    /// Call when a key is released in your window manager loop
    pub fn on_key_released(&mut self, key: Key) {
        self.keys.insert(key, KeyState::Released);
    }

    /// Call every frame after this struct is done being used, resets `JustPressed` keystates to `Pressed`
    pub fn update(&mut self) {
        if self.needs_update {
            for (_, state) in self.keys.iter_mut() {
                if *state == KeyState::JustPressed {
                    *state = KeyState::Pressed;
                }
            }
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
}