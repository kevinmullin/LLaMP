//! Snap and dock. 10 skin pixels to snap, 12 to undock. Four windows share one group.

use crate::eq_window::Frame;

pub(crate) const SNAP: i32 = 10;
pub(crate) const UNDOCK2: i32 = 12 * 12;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pane {
    Main = 0,
    Eq = 1,
    Playlist = 2,
    Browser = 3,
}

impl Pane {
    fn index(self) -> usize {
        self as usize
    }

    fn all() -> [Pane; 4] {
        [Pane::Main, Pane::Eq, Pane::Playlist, Pane::Browser]
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupMove {
    frames: [Frame; 4],
    docked: [bool; 4],
}

impl GroupMove {
    pub fn frame(&self, pane: Pane) -> Frame {
        self.frames[pane.index()]
    }

    pub fn docked(&self, pane: Pane) -> bool {
        self.docked[pane.index()]
    }
}

pub struct DockGroup {
    frames: [Frame; 4],
    docked: [bool; 4],
    accum_x: i32,
    accum_y: i32,
}

impl DockGroup {
    pub fn new() -> Self {
        Self {
            frames: [
                Frame::new(0, 0, 275, 116),
                Frame::new(0, 140, 275, 116),
                Frame::new(275, 0, 275, 116),
                Frame::new(275, 140, 275, 116),
            ],
            docked: [false; 4],
            accum_x: 0,
            accum_y: 0,
        }
    }

    pub fn set_frame(&mut self, pane: Pane, x: i32, y: i32, w: i32, h: i32) {
        self.frames[pane.index()] = Frame::new(x, y, w, h);
    }

    pub fn begin_drag(&mut self) {
        self.accum_x = 0;
        self.accum_y = 0;
    }

    pub fn drag(&mut self, pane: Pane, dx: i32, dy: i32) -> GroupMove {
        self.accum_x = self.accum_x.saturating_add(dx);
        self.accum_y = self.accum_y.saturating_add(dy);
        let travel = self.accum_x.saturating_mul(self.accum_x) + self.accum_y.saturating_mul(self.accum_y);
        if self.docked[pane.index()] && travel > UNDOCK2 {
            self.docked[pane.index()] = false;
            shift(&mut self.frames[pane.index()], dx, dy);
        } else if self.docked[pane.index()] {
            for flag in &self.docked {
                let _ = flag;
            }
            for (index, docked) in self.docked.iter().enumerate() {
                if *docked {
                    shift(&mut self.frames[index], dx, dy);
                }
            }
        } else {
            shift(&mut self.frames[pane.index()], dx, dy);
            for other in Pane::all() {
                if other == pane {
                    continue;
                }
                let other_frame = self.frames[other.index()];
                snap(&mut self.frames[pane.index()], other_frame);
            }
        }
        self.snapshot()
    }

    pub fn end_drag(&mut self) -> GroupMove {
        self.docked = components(&self.frames);
        self.accum_x = 0;
        self.accum_y = 0;
        self.snapshot()
    }

    fn snapshot(&self) -> GroupMove {
        GroupMove {
            frames: self.frames,
            docked: self.docked,
        }
    }
}

impl Default for DockGroup {
    fn default() -> Self {
        Self::new()
    }
}

pub(crate) fn snap(mover: &mut Frame, other: Frame) {
    // A sliver on the other axis is not an edge. The pair tests overlap the full side.
    if overlap_len(mover.x, mover.w, other.x, other.w) > SNAP {
        if (mover.y - (other.y + other.h)).abs() <= SNAP {
            mover.y = other.y + other.h;
        } else if ((mover.y + mover.h) - other.y).abs() <= SNAP {
            mover.y = other.y - mover.h;
        }
    }
    if overlap_len(mover.y, mover.h, other.y, other.h) > SNAP {
        if (mover.x - (other.x + other.w)).abs() <= SNAP {
            mover.x = other.x + other.w;
        } else if ((mover.x + mover.w) - other.x).abs() <= SNAP {
            mover.x = other.x - mover.w;
        }
    }
    if vertically_adjacent(*mover, other) && (mover.x - other.x).abs() <= SNAP {
        mover.x = other.x;
    }
    if horizontally_adjacent(*mover, other) && (mover.y - other.y).abs() <= SNAP {
        mover.y = other.y;
    }
}

pub(crate) fn touching(a: Frame, b: Frame) -> bool {
    (vertically_adjacent(a, b) && overlaps(a.x, a.w, b.x, b.w))
        || (horizontally_adjacent(a, b) && overlaps(a.y, a.h, b.y, b.h))
}

fn components(frames: &[Frame; 4]) -> [bool; 4] {
    let mut seen = [false; 4];
    let mut docked = [false; 4];
    for start in 0..4 {
        if seen[start] {
            continue;
        }
        let mut stack = vec![start];
        let mut members = Vec::new();
        seen[start] = true;
        while let Some(index) = stack.pop() {
            members.push(index);
            for other in 0..4 {
                if seen[other] || !touching(frames[index], frames[other]) {
                    continue;
                }
                seen[other] = true;
                stack.push(other);
            }
        }
        if members.len() > 1 {
            for index in members {
                docked[index] = true;
            }
        }
    }
    docked
}

fn shift(frame: &mut Frame, dx: i32, dy: i32) {
    frame.x += dx;
    frame.y += dy;
}

fn vertically_adjacent(a: Frame, b: Frame) -> bool {
    a.y + a.h == b.y || b.y + b.h == a.y
}

fn horizontally_adjacent(a: Frame, b: Frame) -> bool {
    a.x + a.w == b.x || b.x + b.w == a.x
}

fn overlaps(origin: i32, len: i32, other: i32, other_len: i32) -> bool {
    overlap_len(origin, len, other, other_len) > 0
}

fn overlap_len(origin: i32, len: i32, other: i32, other_len: i32) -> i32 {
    let start = origin.max(other);
    let end = (origin + len).min(other + other_len);
    (end - start).max(0)
}
