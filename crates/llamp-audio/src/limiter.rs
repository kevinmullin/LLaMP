//! Feed-forward limiter. Lookahead buffer is allocated in [`Limiter::new`], not in the callback.

/// −1.0 dBFS.
pub struct Limiter {
    delay: Vec<f32>,
    cursor: usize,
    filled: usize,
    gain: f32,
    release: f32,
}

impl Limiter {
    pub const LOOKAHEAD: usize = 64;
    pub const CEILING: f32 = 0.8912509381337456;

    pub fn new(sample_rate: u32) -> Self {
        let release = 1.0 - (-1.0 / (0.050 * sample_rate as f32)).exp();
        Self {
            delay: vec![0.0; Self::LOOKAHEAD * 2],
            cursor: 0,
            filled: 0,
            gain: 1.0,
            release,
        }
    }

    pub fn process(&mut self, frames: &mut [f32]) {
        let n = frames.len() / 2;
        for i in 0..n {
            let sample = [frames[i * 2], frames[i * 2 + 1]];
            let idx = self.cursor * 2;
            let delayed = [self.delay[idx], self.delay[idx + 1]];
            self.delay[idx] = sample[0];
            self.delay[idx + 1] = sample[1];
            self.cursor = (self.cursor + 1) % Self::LOOKAHEAD;
            if self.filled < Self::LOOKAHEAD {
                self.filled += 1;
            }
            let mut peak = 0.0f32;
            for s in &self.delay[..self.filled * 2] {
                peak = peak.max(s.abs());
            }
            let needed = if peak > Self::CEILING {
                Self::CEILING / peak
            } else {
                1.0
            };
            if needed < self.gain {
                self.gain = needed;
            } else {
                self.gain += self.release * (needed - self.gain);
            }
            let out = if self.filled < Self::LOOKAHEAD {
                [0.0, 0.0]
            } else {
                [delayed[0] * self.gain, delayed[1] * self.gain]
            };
            frames[i * 2] = out[0];
            frames[i * 2 + 1] = out[1];
        }
    }
}
