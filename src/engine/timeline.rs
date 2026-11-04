//! Global time: the generating parameter of every fluent quantity.

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum LoopMode {
    Loop,
    PingPong,
    Once,
}

impl LoopMode {
    pub const ALL: [LoopMode; 3] = [LoopMode::Loop, LoopMode::PingPong, LoopMode::Once];

    pub fn label(self) -> &'static str {
        match self {
            LoopMode::Loop => "Loop",
            LoopMode::PingPong => "Bounce",
            LoopMode::Once => "Once",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            LoopMode::Loop => "loop",
            LoopMode::PingPong => "pingpong",
            LoopMode::Once => "once",
        }
    }

    pub fn from_key(key: &str) -> Option<LoopMode> {
        Self::ALL.into_iter().find(|m| m.key() == key)
    }
}

pub struct Timeline {
    pub t: f64,
    pub playing: bool,
    pub speed: f64,
    pub span: f64,
    pub loop_mode: LoopMode,
    /// The viewer sent time backward. Simulations start over once, on the next
    /// tick. A loop wrapping around on its own does not set this, so whatever
    /// the viewer has painted, struck, or seeded survives it.
    pub rewound: bool,
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
            rewound: false,
            direction: 1.0,
        }
    }

    pub fn reset(&mut self) {
        self.t = 0.0;
        self.direction = 1.0;
        self.playing = true;
        self.rewound = true;
    }

    /// Back to t = 0, keeping play state.
    pub fn restart(&mut self) {
        self.t = 0.0;
        self.direction = 1.0;
        self.rewound = true;
    }

    /// Move to `t` at the viewer's request. Small backward nudges keep the
    /// simulation running; a real jump back starts it over.
    pub fn seek(&mut self, t: f64) {
        let t = t.clamp(0.0, self.span);
        if t < self.t - 0.5 {
            self.rewound = true;
        }
        self.t = t;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loop_wraps_at_the_span() {
        let mut t = Timeline::new();
        t.span = 1.0;
        t.t = 0.95;
        t.advance(0.1);
        assert_eq!(t.t, 0.0);
    }

    #[test]
    fn bounce_turns_around_at_both_ends() {
        let mut t = Timeline::new();
        t.span = 1.0;
        t.loop_mode = LoopMode::PingPong;
        t.t = 0.95;
        t.advance(0.1);
        assert_eq!(t.t, 1.0);
        t.advance(0.05);
        assert!(t.t < 1.0);
    }

    #[test]
    fn once_stops_at_the_end() {
        let mut t = Timeline::new();
        t.span = 1.0;
        t.loop_mode = LoopMode::Once;
        t.t = 0.95;
        t.advance(0.1);
        assert_eq!(t.t, 1.0);
        assert!(!t.playing);
    }

    #[test]
    fn only_the_viewer_rewinds() {
        let mut t = Timeline::new();
        t.span = 1.0;
        t.t = 0.95;
        t.advance(0.1);
        assert_eq!(t.t, 0.0, "the loop wrapped");
        assert!(!t.rewound, "a natural wrap is not a rewind");
        t.t = 0.8;
        t.seek(0.7);
        assert!(!t.rewound, "a small nudge is not a rewind");
        t.seek(0.1);
        assert!(t.rewound, "a real jump back is");
    }

    #[test]
    fn loop_mode_keys_round_trip() {
        for m in LoopMode::ALL {
            assert_eq!(LoopMode::from_key(m.key()), Some(m));
        }
    }
}
