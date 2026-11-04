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

impl Camera {
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
                let proj = Mat4::perspective_rh(45f32.to_radians(), self.aspect, 0.05, 400.0);
                proj * view
            }
        }
    }

    /// NDC -> world position on the z = 0 plane (plane cameras only).
    pub fn unproject_plane(&self, ndc: glam::Vec2) -> glam::Vec2 {
        match self.kind {
            CamKind::Plane { half_h } => glam::vec2(
                self.target.x + ndc.x * half_h * self.aspect,
                self.target.y + ndc.y * half_h,
            ),
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
        match &mut self.kind {
            CamKind::Plane { half_h } => {
                let per_px = 2.0 * *half_h / size.1;
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
