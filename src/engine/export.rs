//! Export: still images, frame sequences for video, and scene files.

use std::path::Path;

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

use super::gpu::ViewTarget;
use super::module::{Engine, ViewRef, MAX_PANES};
use super::timeline::LoopMode;

pub fn save_png(path: &Path, w: u32, h: u32, rgba: &[u8]) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)
            .with_context(|| format!("creating folder {}", dir.display()))?;
    }
    let img = image::RgbaImage::from_raw(w, h, rgba.to_vec()).context("bad pixel buffer size")?;
    img.save(path)
        .with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

impl Engine {
    /// Render one view offscreen at (w, h).
    pub fn render_still(&mut self, view: ViewRef, w: u32, h: u32) -> Result<Vec<u8>> {
        let target = ViewTarget::new(&self.gpu, &self.rend.view_bgl, w, h);
        self.render_view(view, &target);
        target.read_pixels(&self.gpu)
    }

    pub fn export_png(&mut self, view: ViewRef, w: u32, h: u32, path: &Path) -> Result<()> {
        let px = self.render_still(view, w, h)?;
        save_png(path, w, h, &px)
    }

    /// Render frame `i` of `n` spanning the timeline: the single step shared
    /// by the CLI, which loops over it, and the GUI, which does one per frame
    /// so the window stays responsive.
    pub fn export_frame(
        &mut self,
        view: ViewRef,
        (w, h): (u32, u32),
        (i, n): (usize, usize),
        prev_t: f64,
        dir: &Path,
    ) -> Result<f64> {
        if i == 0 {
            self.modules[view.module].rewind();
        }
        let t = self.timeline.span * i as f64 / (n - 1).max(1) as f64;
        self.timeline.t = t;
        let gpu = self.gpu.clone();
        self.modules[view.module].update(&gpu, t, t - prev_t);
        let px = self.render_still(view, w, h)?;
        save_png(&dir.join(format!("frame_{i:05}.png")), w, h, &px)?;
        Ok(t)
    }

    /// Write the ffmpeg one-liner that assembles a frame sequence.
    pub fn write_video_note(dir: &Path, fps: u32) -> Result<()> {
        std::fs::create_dir_all(dir)
            .with_context(|| format!("creating folder {}", dir.display()))?;
        let cmd = format!(
            "ffmpeg -framerate {fps} -i frame_%05d.png -c:v libx264 -pix_fmt yuv420p -crf 16 out.mp4\n"
        );
        std::fs::write(dir.join("make_video.txt"), cmd)
            .with_context(|| format!("writing into {}", dir.display()))?;
        Ok(())
    }

    /// Render `n` frames spanning the timeline into `dir` as PNGs, and write
    /// an ffmpeg one-liner alongside for assembling a video.
    pub fn export_frames(
        &mut self,
        view: ViewRef,
        size: (u32, u32),
        n: usize,
        fps: u32,
        dir: &Path,
    ) -> Result<()> {
        Self::write_video_note(dir, fps)?;
        let mut prev_t = 0.0;
        for i in 0..n {
            prev_t = self.export_frame(view, size, (i, n), prev_t, dir)?;
        }
        Ok(())
    }

    /// The whole scene: what is open, the time, every open chamber's
    /// parameters, and every open camera. Version 1 readers still find
    /// `module`, `modes`, and `params` for the focused chamber.
    pub fn scene_json(&mut self) -> Value {
        let focused = self.focused();
        let id = self.modules[focused.module].info().id;
        let modes: Vec<usize> = self
            .panes
            .iter()
            .filter(|p| p.module == focused.module)
            .map(|p| p.mode)
            .collect();
        let panes = self.panes.clone();
        let pane_json: Vec<Value> = panes
            .iter()
            .map(|p| {
                let view = self.modules[p.module]
                    .modes()
                    .get(p.mode)
                    .map(|m| m.id)
                    .unwrap_or("");
                let camera = self.camera(p.module, p.mode).to_json();
                json!({
                    "module": self.modules[p.module].info().id,
                    "view": view,
                    "camera": camera,
                })
            })
            .collect();
        let mut params = serde_json::Map::new();
        for m in self.open_modules() {
            params.insert(
                self.modules[m].info().id.to_string(),
                self.modules[m].params_ref().to_json(),
            );
        }
        json!({
            "app": "revelatio-noetica",
            "version": 2,
            "module": id,
            "t": self.timeline.t,
            "span": self.timeline.span,
            "speed": self.timeline.speed,
            "loop": self.timeline.loop_mode.key(),
            "playing": self.timeline.playing,
            "modes": modes,
            "params": self.modules[focused.module].params_ref().to_json(),
            "panes": pane_json,
            "focus": self.focus,
            "all_params": params,
        })
    }

    fn find_module(&self, id: &str) -> Option<usize> {
        self.modules.iter().position(|m| m.info().id == id)
    }

    /// Restore a scene. Input may come from a file or a pasted link, so every
    /// field is optional or validated, and an unknown module is an error.
    pub fn apply_scene(&mut self, v: &Value) -> Result<()> {
        let id = v["module"].as_str().unwrap_or_default();
        let Some(idx) = self.find_module(id) else {
            bail!("this scene names an unknown chamber: {id:?}");
        };
        self.set_active(idx);
        if let Some(p) = v.get("params") {
            self.modules[idx].params().load_json(p);
        }
        if let Some(all) = v["all_params"].as_object() {
            for (key, params) in all {
                if let Some(m) = self.find_module(key) {
                    self.modules[m].params().load_json(params);
                }
            }
        }

        let mut panes = Vec::new();
        let mut cameras = Vec::new();
        if let Some(list) = v["panes"].as_array() {
            for p in list.iter().take(MAX_PANES) {
                let Some(m) = p["module"].as_str().and_then(|s| self.find_module(s)) else {
                    continue;
                };
                let view = p["view"].as_str().unwrap_or_default();
                let Some(mode) = self.modules[m].modes().iter().position(|r| r.id == view) else {
                    continue;
                };
                let vr = ViewRef { module: m, mode };
                if !panes.contains(&vr) {
                    panes.push(vr);
                    cameras.push(p["camera"].clone());
                }
            }
        }
        if panes.is_empty() {
            // Version 1: indices into the focused chamber's views.
            let n = self.modules[idx].modes().len();
            if let Some(modes) = v["modes"].as_array() {
                for mode in modes.iter().filter_map(|x| x.as_u64()) {
                    let vr = ViewRef {
                        module: idx,
                        mode: mode as usize,
                    };
                    if vr.mode < n && panes.len() < MAX_PANES && !panes.contains(&vr) {
                        panes.push(vr);
                        cameras.push(Value::Null);
                    }
                }
            }
        }
        if !panes.is_empty() {
            for p in &panes {
                self.ensure_init(p.module);
            }
            self.focus = v["focus"]
                .as_u64()
                .map_or(0, |f| (f as usize).min(panes.len() - 1));
            self.panes = panes;
            for (p, cam) in self.panes.clone().into_iter().zip(&cameras) {
                if cam.is_object() {
                    self.camera(p.module, p.mode).apply_json(cam);
                }
            }
        }

        if let Some(span) = v["span"].as_f64() {
            self.timeline.span = span.clamp(1.0, 120.0);
        }
        if let Some(speed) = v["speed"].as_f64() {
            self.timeline.speed = speed.clamp(0.05, 4.0);
        }
        if let Some(mode) = v["loop"].as_str().and_then(LoopMode::from_key) {
            self.timeline.loop_mode = mode;
        }
        if let Some(playing) = v["playing"].as_bool() {
            self.timeline.playing = playing;
        }
        if let Some(t) = v["t"].as_f64() {
            self.timeline.t = t.clamp(0.0, self.timeline.span);
        }
        Ok(())
    }

    pub fn save_scene(&mut self, path: &Path) -> Result<()> {
        let v = self.scene_json();
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)
                .with_context(|| format!("creating folder {}", dir.display()))?;
        }
        std::fs::write(path, serde_json::to_string_pretty(&v)?)
            .with_context(|| format!("writing {}", path.display()))?;
        Ok(())
    }

    pub fn load_scene(&mut self, path: &Path) -> Result<()> {
        let text =
            std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let v: Value = serde_json::from_str(&text)
            .with_context(|| format!("{} is not a scene file", path.display()))?;
        self.apply_scene(&v)
    }
}
