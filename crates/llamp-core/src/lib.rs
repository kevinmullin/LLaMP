//! Session and transport. The UI polls a seqlock. The audio callback only
//! `fetch_add`s a frame counter. Neither side takes a lock the other waits on.

mod eq_window;
mod session;

pub use eq_window::{DockMove, EqWindow, Frame, Which};
pub use session::{PlaybackSnapshot, Session, ARROW_SEEK_SECONDS, TITLE_CAP};
