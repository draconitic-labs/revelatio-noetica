//! Observer frames: orbit camera for 3D representations, pan/zoom plane for 2D.

use glam::{Mat4, Vec3};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CamKind {
    /// 2D plane view; `half_h` is the visible half-height in world units.
    Plane { half_h: f32 },
    /// 3D orbit view.
    Orbit { dist: f32, pitch: f32, yaw: f32 },
}

#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub kind: CamKind,
    pub target: Vec3,
    pub aspect: f32,
}

/// Views are composed for at least this width-to-height ratio. A narrower pane
/// shows the same composition with extra space above and below instead of
/// cropping the sides. At this ratio and wider, nothing changes.
const DESIGN_ASPECT: f32 = 1.6;

impl Camera {
    fn fit(&self) -> f32 {
        (DESIGN_ASPECT / self.aspect.max(0.01)).max(1.0)
    }

    /// Serializable state, for scene files and links. Aspect belongs to the
    /// pane showing the camera, so it is not part of this.
    pub fn to_json(self) -> serde_json::Value {
        let target = self.target.to_array();
        match self.kind {
            CamKind::Plane { half_h } => serde_json::json!({ "target": target, "half_h": half_h }),
            CamKind::Orbit { dist, pitch, yaw } => {
                serde_json::json!({ "target": target, "dist": dist, "pitch": pitch, "yaw": yaw })
            }
        }
    }

    /// Restore from `to_json`. Input is untrusted: non-finite or missing
    /// values are ignored, and the rest is clamped to what zoom would allow.
    pub fn apply_json(&mut self, v: &serde_json::Value) {
        let num = |key: &str| {
            v.get(key)
                .and_then(serde_json::Value::as_f64)
                .map(|x| x as f32)
                .filter(|x| x.is_finite())
        };
        if let Some(t) = v.get("target").and_then(serde_json::Value::as_array) {
            let t: Vec<f32> = t
                .iter()
                .filter_map(serde_json::Value::as_f64)
                .map(|x| x as f32)
                .collect();
            if t.len() == 3 && t.iter().all(|x| x.is_finite() && x.abs() < 1e7) {
                self.target = Vec3::new(t[0], t[1], t[2]);
            }
        }
        match &mut self.kind {
            CamKind::Plane { half_h } => {
                if let Some(h) = num("half_h") {
                    *half_h = h.clamp(1e-4, 1e6);
                }
            }
            CamKind::Orbit { dist, pitch, yaw } => {
                if let Some(d) = num("dist") {
                    *dist = d.clamp(0.1, 300.0);
                }
                if let Some(p) = num("pitch") {
                    *pitch = p.clamp(-1.5, 1.5);
                }
                if let Some(y) = num("yaw") {
                    *yaw = y;
                }
            }
        }
    }

    pub fn new(kind: CamKind) -> Self {
        Self {
            kind,
            target: Vec3::ZERO,
            aspect: 16.0 / 9.0,
        }
    }

    pub fn eye(&self) -> Vec3 {
        match self.kind {
            CamKind::Plane { .. } => self.target + Vec3::new(0.0, 0.0, 10.0),
            CamKind::Orbit { dist, pitch, yaw } => {
                let (sy, cy) = yaw.sin_cos();
                let (sp, cp) = pitch.sin_cos();
                self.target + dist * Vec3::new(cp * cy, sp, cp * sy)
            }
        }
    }

    pub fn view_proj(&self) -> Mat4 {
        match self.kind {
            CamKind::Plane { half_h } => {
                let half_h = half_h * self.fit();
                let hw = half_h * self.aspect;

                Mat4::orthographic_rh(
                    self.target.x - hw,
                    self.target.x + hw,
                    self.target.y - half_h,
                    self.target.y + half_h,
                    -100.0,
                    100.0,
                )
            }
            CamKind::Orbit { .. } => {
                let view = Mat4::look_at_rh(self.eye(), self.target, Vec3::Y);
                let fov = 2.0 * ((22.5f32).to_radians().tan() * self.fit()).atan();
                let proj = Mat4::perspective_rh(fov, self.aspect, 0.05, 400.0);
                proj * view
            }
        }
    }

    /// NDC -> world position on the z = 0 plane (plane cameras only).
    pub fn unproject_plane(&self, ndc: glam::Vec2) -> glam::Vec2 {
        match self.kind {
            CamKind::Plane { half_h } => {
                let half_h = half_h * self.fit();
                glam::vec2(
                    self.target.x + ndc.x * half_h * self.aspect,
                    self.target.y + ndc.y * half_h,
                )
            }
            CamKind::Orbit { .. } => glam::vec2(0.0, 0.0),
        }
    }

    /// World -> normalized device coords; returns None if behind the camera.
    pub fn project(&self, p: Vec3) -> Option<glam::Vec2> {
        let clip = self.view_proj() * p.extend(1.0);
        if clip.w <= 0.0001 {
            return None;
        }
        Some(glam::vec2(clip.x / clip.w, clip.y / clip.w))
    }

    /// Apply a mouse drag in pixels over a viewport of `size` pixels.
    pub fn drag(&mut self, dx: f32, dy: f32, size: (f32, f32)) {
        let fit = self.fit();
        match &mut self.kind {
            CamKind::Plane { half_h } => {
                let per_px = 2.0 * *half_h * fit / size.1;
                self.target.x -= dx * per_px;
                self.target.y += dy * per_px;
            }
            CamKind::Orbit { pitch, yaw, .. } => {
                *yaw += dx * 0.008;
                *pitch = (*pitch + dy * 0.008).clamp(-1.5, 1.5);
            }
        }
    }

    /// Pan for orbit cameras (secondary drag): move target in the view plane.
    pub fn pan(&mut self, dx: f32, dy: f32, size: (f32, f32)) {
        match self.kind {
            CamKind::Plane { .. } => self.drag(dx, dy, size),
            CamKind::Orbit { dist, pitch, yaw } => {
                let (sy, cy) = yaw.sin_cos();
                let fwd = -Vec3::new(pitch.cos() * cy, pitch.sin(), pitch.cos() * sy);
                let right = fwd.cross(Vec3::Y).normalize_or_zero();
                let up = right.cross(fwd).normalize_or_zero();
                let per_px = 1.6 * dist / size.1;
                self.target += (-right * dx + up * dy) * per_px;
            }
        }
    }

    pub fn zoom(&mut self, scroll: f32) {
        let f = (-scroll * 0.0015).exp();
        match &mut self.kind {
            CamKind::Plane { half_h } => *half_h = (*half_h * f).clamp(1e-4, 1e6),
            CamKind::Orbit { dist, .. } => *dist = (*dist * f).clamp(0.1, 300.0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plane_project_and_unproject_agree_at_any_aspect() {
        for aspect in [0.6, 1.0, 1.6, 2.4] {
            let mut c = Camera::new(CamKind::Plane { half_h: 3.0 });
            c.target = Vec3::new(4.0, 1.0, 0.0);
            c.aspect = aspect;
            let p = Vec3::new(5.5, -0.75, 0.0);
            let ndc = c.project(p).unwrap();
            let back = c.unproject_plane(ndc);
            assert!((back.x - p.x).abs() < 1e-4 && (back.y - p.y).abs() < 1e-4);
        }
    }

    #[test]
    fn a_narrow_pane_keeps_the_composed_width_in_view() {
        let mut wide = Camera::new(CamKind::Plane { half_h: 3.0 });
        wide.aspect = DESIGN_ASPECT;
        let edge = Vec3::new(3.0 * DESIGN_ASPECT, 0.0, 0.0);
        assert!((wide.project(edge).unwrap().x - 1.0).abs() < 1e-5);
        let mut narrow = wide;
        narrow.aspect = 0.8;
        assert!(narrow.project(edge).unwrap().x <= 1.0 + 1e-5);
    }

    #[test]
    fn hostile_camera_json_cannot_break_the_view() {
        let mut c = Camera::new(CamKind::Plane { half_h: 3.0 });
        c.apply_json(&serde_json::json!({ "target": [1e30, 0, 0], "half_h": -5.0 }));
        assert_eq!(c.target, Vec3::ZERO);
        assert_eq!(c.kind, CamKind::Plane { half_h: 1e-4 });
        let mut o = Camera::new(CamKind::Orbit {
            dist: 5.0,
            pitch: 0.1,
            yaw: 0.2,
        });
        o.apply_json(&serde_json::json!({ "pitch": 90.0, "dist": "far" }));
        assert_eq!(
            o.kind,
            CamKind::Orbit {
                dist: 5.0,
                pitch: 1.5,
                yaw: 0.2
            }
        );
    }

    #[test]
    fn camera_json_round_trips() {
        let mut o = Camera::new(CamKind::Orbit {
            dist: 5.0,
            pitch: 0.1,
            yaw: 0.2,
        });
        o.target = Vec3::new(1.0, 2.0, 3.0);
        let mut restored = Camera::new(CamKind::Orbit {
            dist: 1.0,
            pitch: 0.0,
            yaw: 0.0,
        });
        restored.apply_json(&o.to_json());
        assert_eq!(restored.kind, o.kind);
        assert_eq!(restored.target, o.target);
    }
}
