//! Small numeric helpers shared by modules: complex arithmetic, colors, ODE steps.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct C {
    pub re: f64,
    pub im: f64,
}

pub const fn c(re: f64, im: f64) -> C {
    C { re, im }
}

impl C {
    pub const ZERO: C = c(0.0, 0.0);
    pub const ONE: C = c(1.0, 0.0);

    pub fn abs(self) -> f64 {
        self.re.hypot(self.im)
    }
    pub fn arg(self) -> f64 {
        self.im.atan2(self.re)
    }
    pub fn conj(self) -> C {
        c(self.re, -self.im)
    }
    pub fn exp(self) -> C {
        let r = self.re.exp();
        c(r * self.im.cos(), r * self.im.sin())
    }
    pub fn ln(self) -> C {
        c(self.abs().max(1e-300).ln(), self.arg())
    }
    pub fn powi(self, n: i32) -> C {
        let mut r = C::ONE;
        let mut b = if n < 0 { C::ONE / self } else { self };
        let mut e = n.unsigned_abs();
        while e > 0 {
            if e & 1 == 1 {
                r = r * b;
            }
            b = b * b;
            e >>= 1;
        }
        r
    }
    pub fn sin(self) -> C {
        c(
            self.re.sin() * self.im.cosh(),
            self.re.cos() * self.im.sinh(),
        )
    }
    pub fn from_polar(r: f64, th: f64) -> C {
        c(r * th.cos(), r * th.sin())
    }
}

impl std::ops::Add for C {
    type Output = C;
    fn add(self, o: C) -> C {
        c(self.re + o.re, self.im + o.im)
    }
}
impl std::ops::Sub for C {
    type Output = C;
    fn sub(self, o: C) -> C {
        c(self.re - o.re, self.im - o.im)
    }
}
impl std::ops::Mul for C {
    type Output = C;
    fn mul(self, o: C) -> C {
        c(
            self.re * o.re - self.im * o.im,
            self.re * o.im + self.im * o.re,
        )
    }
}
impl std::ops::Mul<f64> for C {
    type Output = C;
    fn mul(self, s: f64) -> C {
        c(self.re * s, self.im * s)
    }
}
impl std::ops::Div for C {
    type Output = C;
    fn div(self, o: C) -> C {
        let d = (o.re * o.re + o.im * o.im).max(1e-300);
        c(
            (self.re * o.re + self.im * o.im) / d,
            (self.im * o.re - self.re * o.im) / d,
        )
    }
}

/// Perceptual phase wheel matching the WGSL `phase_color` (returns linear RGB).
pub fn phase_color(theta: f32) -> [f32; 4] {
    let (l, a, b) = (0.74f32, 0.135 * theta.cos(), 0.135 * theta.sin());
    let l_ = l + 0.396_337_78 * a + 0.215_803_76 * b;
    let m_ = l - 0.105_561_346 * a - 0.063_854_17 * b;
    let s_ = l - 0.089_484_18 * a - 1.291_485_5 * b;
    let (l3, m3, s3) = (l_ * l_ * l_, m_ * m_ * m_, s_ * s_ * s_);
    let r = 4.076_741_7 * l3 - 3.307_711_6 * m3 + 0.230_969_94 * s3;
    let g = -1.268_438 * l3 + 2.609_757_4 * m3 - 0.341_319_38 * s3;
    let bl = -0.0041960863 * l3 - 0.703_418_6 * m3 + 1.707_614_7 * s3;
    [
        r.clamp(0.0, 1.0),
        g.clamp(0.0, 1.0),
        bl.clamp(0.0, 1.0),
        1.0,
    ]
}

pub fn with_alpha(mut col: [f32; 4], a: f32) -> [f32; 4] {
    col[3] = a;
    col
}

pub fn scale_rgb(col: [f32; 4], s: f32) -> [f32; 4] {
    [col[0] * s, col[1] * s, col[2] * s, col[3]]
}

/// One RK4 step of dy/dt = f(t, y) for small fixed-size state vectors.
pub fn rk4<const N: usize>(
    f: &dyn Fn(f64, &[f64; N]) -> [f64; N],
    t: f64,
    y: &[f64; N],
    h: f64,
) -> [f64; N] {
    let add = |a: &[f64; N], b: &[f64; N], s: f64| {
        let mut r = *a;
        for i in 0..N {
            r[i] += b[i] * s;
        }
        r
    };
    let k1 = f(t, y);
    let k2 = f(t + h * 0.5, &add(y, &k1, h * 0.5));
    let k3 = f(t + h * 0.5, &add(y, &k2, h * 0.5));
    let k4 = f(t + h, &add(y, &k3, h));
    let mut r = *y;
    for i in 0..N {
        r[i] += h / 6.0 * (k1[i] + 2.0 * k2[i] + 2.0 * k3[i] + k4[i]);
    }
    r
}
