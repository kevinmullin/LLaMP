//! Watch granted folders only. A new file is inserted. A delete marks the row missing.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};

use crate::LibraryInner;

pub struct Watch {
    _watcher: RecommendedWatcher,
    stop: Sender<()>,
    thread: Option<JoinHandle<()>>,
}

impl Watch {
    pub fn start(inner: Arc<Mutex<LibraryInner>>, folders: Vec<PathBuf>) -> Result<Self, String> {
        let (tx, rx) = mpsc::channel();
        let (stop_tx, stop_rx) = mpsc::channel();
        let mut watcher = RecommendedWatcher::new(
            move |res| {
                let _ = tx.send(res);
            },
            notify::Config::default().with_poll_interval(Duration::from_millis(200)),
        )
        .map_err(|err| err.to_string())?;
        for folder in &folders {
            watcher
                .watch(folder, RecursiveMode::Recursive)
                .map_err(|err| err.to_string())?;
        }
        let thread = thread::spawn(move || run(inner, rx, stop_rx));
        Ok(Self {
            _watcher: watcher,
            stop: stop_tx,
            thread: Some(thread),
        })
    }

    pub fn add(&mut self, folder: &Path) -> Result<(), String> {
        self._watcher
            .watch(folder, RecursiveMode::Recursive)
            .map_err(|err| err.to_string())
    }
}

impl Drop for Watch {
    fn drop(&mut self) {
        let _ = self.stop.send(());
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn run(inner: Arc<Mutex<LibraryInner>>, rx: Receiver<Result<notify::Event, notify::Error>>, stop: Receiver<()>) {
    loop {
        if stop.try_recv().is_ok() {
            return;
        }
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(Ok(event)) => apply(&inner, &event),
            Ok(Err(_)) => {}
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return,
        }
    }
}

fn apply(inner: &Mutex<LibraryInner>, event: &notify::Event) {
    let Ok(mut guard) = inner.lock() else {
        return;
    };
    for path in &event.paths {
        let Some(spelled) = guard.spell_like_grant(path) else {
            continue;
        };
        match event.kind {
            EventKind::Remove(_) => {
                let _ = guard.mark_missing(&spelled);
            }
            EventKind::Create(_) | EventKind::Modify(_) | EventKind::Any => {
                if spelled.is_dir() || path.is_dir() {
                    let _ = guard.note_new_files(&spelled);
                } else {
                    let _ = guard.insert_if_absent(&spelled);
                }
            }
            _ => {}
        }
    }
}
