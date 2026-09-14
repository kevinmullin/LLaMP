//! Playlist window. Resize is 25×29 from 275×116. Rows are a window, not a full redraw.

pub const MIN_W: i32 = 275;
pub const MIN_H: i32 = 116;
pub const STEP_X: i32 = 25;
pub const STEP_Y: i32 = 29;
/// Locked `text.bmp` cell width. Not a new pitch.
pub const CELL_W: i32 = 5;
/// Locked `text.bmp` cell height. Not a new pitch.
pub const ROW_H: i32 = 7;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowFont {
    Bitmap,
    CoreText,
    Mixed,
}

/// Per scalar. A present `text.bmp` glyph stays bitmap. A missing one is CoreText.
pub fn glyph_font(ch: char, has_glyph: impl Fn(char) -> bool) -> RowFont {
    if has_glyph(ch) {
        RowFont::Bitmap
    } else {
        RowFont::CoreText
    }
}

/// Row summary. Mixed means the seam is inside the row.
pub fn row_font(text: &str, has_glyph: impl Fn(char) -> bool) -> RowFont {
    let mut bitmap = false;
    let mut core = false;
    for ch in text.chars() {
        match glyph_font(ch, &has_glyph) {
            RowFont::Bitmap => bitmap = true,
            RowFont::CoreText => core = true,
            RowFont::Mixed => {}
        }
    }
    match (bitmap, core) {
        (_, false) => RowFont::Bitmap,
        (false, true) => RowFont::CoreText,
        (true, true) => RowFont::Mixed,
    }
}

pub struct PlaylistWindow {
    w: i32,
    h: i32,
    entries: Vec<String>,
}

impl PlaylistWindow {
    pub fn new() -> Self {
        Self {
            w: MIN_W,
            h: MIN_H,
            entries: Vec::new(),
        }
    }

    pub fn size(&self) -> (i32, i32) {
        (self.w, self.h)
    }

    pub fn propose_size(&mut self, w: i32, h: i32) -> Result<(), String> {
        if w < MIN_W || (w - MIN_W) % STEP_X != 0 {
            return Err(format!("width {w} is not a 25 px step from {MIN_W}"));
        }
        if h < MIN_H || (h - MIN_H) % STEP_Y != 0 {
            return Err(format!("height {h} is not a 29 px step from {MIN_H}"));
        }
        self.w = w;
        self.h = h;
        Ok(())
    }

    pub fn visible_range(&self, len: usize, scroll: usize) -> std::ops::Range<usize> {
        let start = scroll.min(len);
        let end = start.saturating_add(self.client_rows()).min(len);
        start..end
    }

    pub fn hit_row(&self, y: i32, scroll: usize, len: usize) -> Option<usize> {
        if y < 0 || y >= self.h - ROW_H {
            return None;
        }
        let index = scroll + (y / ROW_H) as usize;
        (index < len).then_some(index)
    }

    /// Visible window length. Does not walk `len`.
    pub fn rows_touched(&self, len: usize, scroll: usize) -> usize {
        self.visible_range(len, scroll).len()
    }

    pub fn enqueue(&mut self, path: &str) {
        self.entries.push(path.to_string());
    }

    /// Search hits stay the granted path. The window does not copy the file.
    pub fn enqueue_hit(&mut self, hit: &llamp_library::SearchHit) {
        self.enqueue(hit.path.to_string_lossy().as_ref());
    }

    pub fn reorder(&mut self, from: usize, to: usize) {
        if from >= self.entries.len() || to >= self.entries.len() || from == to {
            return;
        }
        let item = self.entries.remove(from);
        self.entries.insert(to, item);
    }

    pub fn remove(&mut self, index: usize) -> Result<(), String> {
        if index >= self.entries.len() {
            return Err("row".into());
        }
        self.entries.remove(index);
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn entry(&self, index: usize) -> &str {
        &self.entries[index]
    }

    /// Bottom strip. Not a `pledit.bmp` slice. Width is split evenly. Height is the locked cell.
    pub fn menu_button(&self, index: usize) -> Option<(&'static str, i32, i32, i32, i32)> {
        const LABELS: [&str; 5] = ["Add", "Rem", "Sel", "Misc", "List"];
        let label = *LABELS.get(index)?;
        let width = self.w / 5;
        Some((label, index as i32 * width, self.h - ROW_H, width, ROW_H))
    }

    fn client_rows(&self) -> usize {
        let client = (self.h - ROW_H).max(ROW_H);
        (client / ROW_H) as usize
    }
}

impl Default for PlaylistWindow {
    fn default() -> Self {
        Self::new()
    }
}
