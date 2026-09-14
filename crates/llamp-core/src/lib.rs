//! Session and transport. The UI polls a seqlock. The audio callback only
//! `fetch_add`s a frame counter. Neither side takes a lock the other waits on.

mod browser_window;
mod dock;
mod eq_window;
mod playlist_window;
mod session;

pub use browser_window::{browser_row_font, BrowserList};

pub fn browser_size() -> (i32, i32) {
    browser_window::size()
}
pub use dock::{DockGroup, GroupMove, Pane};
pub use eq_window::{DockMove, EqWindow, Frame, Which};
pub use playlist_window::{glyph_font, list_font, row_font, PlaylistWindow, RowFont, CELL_W, ROW_H};
pub use session::{PlaybackSnapshot, Session, ARROW_SEEK_SECONDS, TITLE_CAP};
