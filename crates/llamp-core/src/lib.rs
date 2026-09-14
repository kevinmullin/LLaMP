//! Session and transport. The UI polls a seqlock. The audio callback only
//! `fetch_add`s a frame counter. Neither side takes a lock the other waits on.

mod browser_window;
mod dock;
mod eq_window;
mod lyrics_window;
mod playlist_window;
mod session;
mod vis_window;

pub use browser_window::{browser_row_font, BrowserList};

pub fn browser_size() -> (i32, i32) {
    browser_window::size()
}
pub use dock::{DockGroup, GroupMove, Pane};
pub use lyrics_window::{
    client_rect as lyrics_client_rect, cursor_for, line_rect, line_style, mark_rect, propose_size as lyrics_propose_size,
    scroll_for_active, step_offset, text_origin, LineStyle, TextRect, LINE_H, MARK_H, MARK_W, MIN_H as LYRICS_MIN_H,
    MIN_W as LYRICS_MIN_W, PRIVACY_NOTE, SIDECAR_NOT_WRITTEN, TEXT_PAD,
};
pub use eq_window::{DockMove, EqWindow, Frame, Which};
pub use playlist_window::{
    glyph_font, list_font, row_font, PlaylistWindow, RowFont, CELL_W, ROW_H,
};
pub use session::{PlaybackSnapshot, Session, ARROW_SEEK_SECONDS, TITLE_CAP};
pub use vis_window::{client_rect, propose_size, ClientRect, CHROME_BOTTOM, CHROME_LEFT, CHROME_RIGHT, CHROME_TOP, MIN_H as VIS_MIN_H, MIN_W as VIS_MIN_W};
