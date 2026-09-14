//! Browser window. Always CoreText. No invented `gen` step. No shade height.

use crate::playlist_window::RowFont;

pub const MIN_W: i32 = 275;
pub const MIN_H: i32 = 116;

pub fn size() -> (i32, i32) {
    (MIN_W, MIN_H)
}

pub fn browser_row_font(_text: &str) -> RowFont {
    RowFont::CoreText
}

/// Granted library rows. Never `text.bmp`.
pub struct BrowserList {
    rows: Vec<String>,
}

impl BrowserList {
    pub fn new() -> Self {
        Self { rows: Vec::new() }
    }

    pub fn load_granted(
        &mut self,
        source: &dyn llamp_plugin_api::MediaSource,
    ) -> Result<(), String> {
        self.rows = source
            .browse()?
            .into_iter()
            .map(|item| item.label)
            .collect();
        Ok(())
    }

    pub fn rows(&self) -> &[String] {
        &self.rows
    }
}

impl Default for BrowserList {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_stays_at_the_minimum_until_a_gen_step_is_locked() {
        assert_eq!(size(), (275, 116));
    }
}
