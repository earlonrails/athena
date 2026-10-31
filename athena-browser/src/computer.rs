use std::sync::{Arc, Mutex};
use enigo::{Enigo, KeyboardControllable, MouseControllable, MouseButton};
use xcap::Monitor;
use base64::{Engine as _, engine::general_purpose};
use tracing::debug;

pub struct ComputerUse {
    enigo: Arc<Mutex<Enigo>>,
}

impl Default for ComputerUse {
    fn default() -> Self {
        Self::new()
    }
}

impl ComputerUse {
    pub fn new() -> Self {
        Self {
            enigo: Arc::new(Mutex::new(Enigo::new())),
        }
    }

    /// Moves the mouse to specific coordinates
    pub fn mouse_move(&self, x: i32, y: i32) -> Result<(), String> {
        let mut enigo = self.enigo.lock().map_err(|_| "Mutex poisoned")?;
        enigo.mouse_move_to(x, y);
        debug!("Moved mouse to {}, {}", x, y);
        Ok(())
    }

    /// Clicks the mouse
    pub fn mouse_click(&self, right_click: bool) -> Result<(), String> {
        let mut enigo = self.enigo.lock().map_err(|_| "Mutex poisoned")?;
        if right_click {
            enigo.mouse_click(MouseButton::Right);
        } else {
            enigo.mouse_click(MouseButton::Left);
        }
        debug!("Mouse clicked (right_click: {})", right_click);
        Ok(())
    }

    /// Types text
    pub fn type_text(&self, text: &str) -> Result<(), String> {
        let mut enigo = self.enigo.lock().map_err(|_| "Mutex poisoned")?;
        enigo.key_sequence(text);
        debug!("Typed text: {}", text);
        Ok(())
    }

    /// Captures the primary screen and returns it as a base64 encoded PNG
    pub fn capture_screen(&self) -> Result<String, String> {
        let monitors = Monitor::all().map_err(|e| e.to_string())?;
        
        let primary_monitor = monitors.into_iter()
            .next()
            .ok_or("No monitors found")?;
            
        let image = primary_monitor.capture_image().map_err(|e| e.to_string())?;
        
        // Convert to PNG in memory
        let mut buffer = std::io::Cursor::new(Vec::new());
        image.write_to(&mut buffer, image::ImageFormat::Png)
            .map_err(|e| e.to_string())?;
            
        let encoded = general_purpose::STANDARD.encode(buffer.into_inner());
        debug!("Captured screen image, length: {}", encoded.len());
        
        Ok(encoded)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_computer_use_init() {
        let cu = ComputerUse::default();
        assert!(cu.enigo.lock().is_ok());
    }
}

// Rust guideline compliant 2026-02-21
