//! Session and transport. The UI polls a seqlock. The audio callback only
//! `fetch_add`s a frame counter. Neither side takes a lock the other waits on.

mod session;

pub use session::{PlaybackSnapshot, Session, ARROW_SEEK_SECONDS, TITLE_CAP};
