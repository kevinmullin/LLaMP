//! Visualizer draw budget. Runs on the render thread. Not the audio callback.

use std::cell::Cell;

pub const DRAW_BUDGET_MS: u128 = 8;
pub const KILL_MISSES: usize = 3;
pub const KILL_WINDOW_MS: u128 = 5000;

pub struct FakeClock {
    pub now_ms: Cell<u128>,
    pub draw_cost_ms: u128,
}

impl FakeClock {
    pub fn new(draw_cost_ms: u128) -> Self {
        Self {
            now_ms: Cell::new(0),
            draw_cost_ms,
        }
    }
}

pub struct VisBudget {
    misses: Vec<u128>,
    pub killed: bool,
}

impl VisBudget {
    pub fn new() -> Self {
        Self {
            misses: Vec::new(),
            killed: false,
        }
    }

    /// Times `draw` with `clock`. Allocates only on this thread.
    pub fn run(&mut self, clock: &FakeClock, draw: impl FnOnce()) {
        if self.killed {
            return;
        }
        let start = clock.now_ms.get();
        draw();
        clock.now_ms.set(start.saturating_add(clock.draw_cost_ms));
        if clock.draw_cost_ms > DRAW_BUDGET_MS {
            self.misses.push(start);
        }
        let now = clock.now_ms.get();
        self.misses.retain(|t| now.saturating_sub(*t) <= KILL_WINDOW_MS);
        if self.misses.len() >= KILL_MISSES {
            self.killed = true;
        }
    }
}

impl Default for VisBudget {
    fn default() -> Self {
        Self::new()
    }
}
