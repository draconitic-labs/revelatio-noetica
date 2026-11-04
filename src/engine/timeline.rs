//! Global time: the generating parameter of every fluent quantity.

#[derive(Clone, Copy, PartialEq)]
pub enum LoopMode {
    Loop,
    PingPong,
    Once,
}

pub struct Timeline {
    pub t: f64,
    pub playing: bool,
    pub speed: f64,
    pub span: f64,
    pub loop_mode: LoopMode,
    direction: f64,
}

impl Timeline {
    pub fn new() -> Self {
        Self {
            t: 0.0,
            playing: true,
            speed: 1.0,
            span: 12.0,
            loop_mode: LoopMode::Loop,
            direction: 1.0,
        }
    }

    pub fn reset(&mut self) {
        self.t = 0.0;
        self.direction = 1.0;
        self.playing = true;
    }

    /// Advance by real seconds; returns the simulated dt applied.
    pub fn advance(&mut self, real_dt: f64) -> f64 {
        if !self.playing {
            return 0.0;
        }
        let dt = real_dt.min(0.1) * self.speed * self.direction;
        self.t += dt;
        match self.loop_mode {
            LoopMode::Loop => {
                if self.t > self.span {
                    self.t = 0.0;
                } else if self.t < 0.0 {
                    self.t = self.span;
                }
            }
            LoopMode::PingPong => {
                if self.t > self.span {
                    self.t = self.span;
                    self.direction = -1.0;
                } else if self.t < 0.0 {
                    self.t = 0.0;
                    self.direction = 1.0;
                }
            }
            LoopMode::Once => {
                if self.t > self.span {
                    self.t = self.span;
                    self.playing = false;
                }
            }
        }
        dt
    }
}
