//! Export: still images, frame sequences for video, and scene files.

use std::path::Path;

use anyhow::{Context, Result};

use super::gpu::ViewTarget;
use super::module::Engine;

pub fn save_png(path: &Path, w: u32, h: u32, rgba: &[u8]) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let img = image::RgbaImage::from_raw(w, h, rgba.to_vec()).context("bad pixel buffer size")?;
    img.save(path)?;
    Ok(())
}

impl Engine {
    /// Render one representation of the active module offscreen at (w, h).
    pub fn render_still(&mut self, mode: usize, w: u32, h: u32) -> Result<Vec<u8>> {
        let target = ViewTarget::new(&self.gpu, &self.rend.view_bgl, w, h);
        self.render_view(mode, &target);
        target.read_pixels(&self.gpu)
    }

    pub fn export_png(&mut self, mode: usize, w: u32, h: u32, path: &Path) -> Result<()> {
        let px = self.render_still(mode, w, h)?;
        save_png(path, w, h, &px)
    }

    /// Render `n` frames spanning the timeline into `dir` as PNGs, and write
    /// an ffmpeg one-liner alongside for assembling a video.
    pub fn export_frames(
        &mut self,
        mode: usize,
        w: u32,
        h: u32,
        n: usize,
        fps: u32,
        dir: &Path,
    ) -> Result<()> {
        std::fs::create_dir_all(dir)?;
        let span = self.timeline.span;
        let gpu = self.gpu.clone();
        let mut prev_t = 0.0;
        for i in 0..n {
            let t = span * i as f64 / (n - 1).max(1) as f64;
            self.timeline.t = t;
            self.modules[self.active].update(&gpu, t, t - prev_t);
            prev_t = t;
            let px = self.render_still(mode, w, h)?;
            save_png(&dir.join(format!("frame_{i:05}.png")), w, h, &px)?;
        }
        let cmd = format!(
            "ffmpeg -framerate {fps} -i frame_%05d.png -c:v libx264 -pix_fmt yuv420p -crf 16 out.mp4\n"
        );
        std::fs::write(dir.join("make_video.txt"), cmd)?;
        Ok(())
    }

    /// Interactive scene file: module, time, parameters, open representations.
    pub fn scene_json(&mut self) -> serde_json::Value {
        let active = self.active;
        let id = self.modules[active].info().id;
        serde_json::json!({
            "app": "revelatio-noetica",
            "version": 1,
            "module": id,
            "t": self.timeline.t,
            "span": self.timeline.span,
            "speed": self.timeline.speed,
            "modes": self.active_modes,
            "params": self.modules[active].params_ref().to_json(),
        })
    }

    pub fn save_scene(&mut self, path: &Path) -> Result<()> {
        let v = self.scene_json();
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, serde_json::to_string_pretty(&v)?)?;
        Ok(())
    }

    pub fn load_scene(&mut self, path: &Path) -> Result<()> {
        let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path)?)?;
        let id = v["module"].as_str().unwrap_or_default();
        if let Some(idx) = self.modules.iter().position(|m| m.info().id == id) {
            self.set_active(idx);
            if let Some(p) = v.get("params") {
                self.modules[idx].params().load_json(p);
            }
            if let Some(span) = v["span"].as_f64() {
                self.timeline.span = span;
            }
            if let Some(speed) = v["speed"].as_f64() {
                self.timeline.speed = speed;
            }
            if let Some(t) = v["t"].as_f64() {
                self.timeline.t = t;
            }
            if let Some(modes) = v["modes"].as_array() {
                let n = self.modules[idx].modes().len();
                let m: Vec<usize> = modes
                    .iter()
                    .filter_map(|x| x.as_u64().map(|u| u as usize))
                    .filter(|&u| u < n)
                    .collect();
                if !m.is_empty() {
                    self.active_modes = m;
                }
            }
        } else {
            anyhow::bail!("unknown module id in scene file: {id}");
        }
        Ok(())
    }
}
