use glutin::VirtualKeyCode;
use std::collections::HashMap;

/// A global input state cache that you can feed input events into
/// and poll from anywhere to avoid complicated event piping.
pub struct InputCache {
    keyboard: HashMap<VirtualKeyCode, (KeyState, u32)>,
}

impl InputCache {
    /// Create a new InputState.
    pub fn new() -> Self {
        InputCache {
            keyboard: HashMap::new(),
        }
    }

    /// Updates the ages of tracked keys. Call this every update loop.
    pub fn update_ages(&mut self) {
        for (_key, (_state, age)) in &mut self.keyboard {
            *age += 1;
        }
    }

    /// Add keys for tracking. Only keys added with this method will have their state stored.
    pub fn track_keys(&mut self, keys: &[VirtualKeyCode]) {
        self.keyboard.reserve(keys.len());
        for key in keys {
            self.keyboard.insert(*key, (KeyState::Released, 0));
        }
    }

    /// Get the state of a keyboard key along with its age, or None if it isn't tracked.
    pub fn get_key_state(&self, key: VirtualKeyCode) -> Option<&(KeyState, u32)> {
        self.keyboard.get(&key)
    }

    /// True if the requested key is currently pressed
    /// (for fewer frames than age_limit if provided), false otherwise.
    /// # Panics
    /// Panics if the requested key is not tracked.
    pub fn is_key_pressed(&self, key: VirtualKeyCode, age_limit: Option<u32>) -> bool {
        let (state, age) = self
            .keyboard
            .get(&key)
            .expect(format!("Untracked key: {:?}", key).as_str());
        if let KeyState::Pressed = state {
            if let Some(al) = age_limit {
                *age <= al
            } else {
                true
            }
        } else {
            false
        }
    }

    /// Track the effect of a keyboard event.
    pub fn handle_keyboard(&mut self, evt: glutin::KeyboardInput) {
        if let Some(code) = evt.virtual_keycode {
            if let Some((state, age)) = self.keyboard.get_mut(&code) {
                match evt.state {
                    glutin::ElementState::Pressed => {
                        if let KeyState::Released = state {
                            *state = KeyState::Pressed;
                            *age = 0;
                        }
                    }
                    glutin::ElementState::Released => {
                        if let KeyState::Pressed = state {
                            *state = KeyState::Released;
                            *age = 0;
                        }
                    }
                }
            }
        }
    }
}

/// The state of an individual key.
pub enum KeyState {
    Released,
    Pressed,
}
