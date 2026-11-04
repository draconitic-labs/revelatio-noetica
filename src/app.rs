//! The atlas shell: concept browser, GPU canvas grid, symbolic inspector,
//! representation deck, observer controls, timeline, and export tools.

use std::collections::HashMap;
use std::time::Instant;

use egui::{Align2, Color32, CornerRadius, FontId, Rect, RichText, Stroke, Vec2};

use crate::engine::gpu::{GpuContext, ViewTarget};
use crate::engine::module::{Engine, InsRole};
use crate::engine::params::ParamKind;
use crate::engine::timeline::LoopMode;
use crate::modules;
use crate::theme::{self, Branch};

pub fn run() -> anyhow::Result<()> {
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1720.0, 1000.0])
            .with_min_inner_size([1100.0, 700.0])
            .with_title("Revelatio Noetica"),
        ..Default::default()
    };
    eframe::run_native(
        "Revelatio Noetica",
        options,
        Box::new(|cc| {
            theme::install_fonts(&cc.egui_ctx);
            theme::install_style(&cc.egui_ctx);
            let rs = cc
                .wgpu_render_state
                .clone()
                .expect("wgpu render state required");
            let gpu = GpuContext::from_eframe(&rs);
            let engine = Engine::new(gpu, modules::all());
            Ok(Box::new(AtlasApp {
                engine,
                render_state: rs,
                slots: HashMap::new(),
                last_frame: Instant::now(),
                status: None,
                scene_path: String::new(),
                fps: 0.0,
                search: String::new(),
                math_tex: HashMap::new(),
                brushed: None,
            }))
        }),
    )
    .map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(())
}

struct ViewSlot {
    target: ViewTarget,
    tex: egui::TextureId,
}

pub struct AtlasApp {
    engine: Engine,
    render_state: eframe::egui_wgpu::RenderState,
    slots: HashMap<usize, ViewSlot>,
    last_frame: Instant,
    status: Option<(String, Instant)>,
    scene_path: String,
    fps: f32,
    search: String,
    math_tex: HashMap<&'static str, Option<egui::TextureHandle>>,
    brushed: Option<egui::TextureHandle>,
}

impl AtlasApp {
    fn brush(&mut self, ui: &egui::Ui) {
        let ctx = ui.ctx().clone();
        let tex = self
            .brushed
            .get_or_insert_with(|| theme::brushed_texture(&ctx));
        theme::paint_brushed(ui.painter(), ui.max_rect().expand(12.0), tex);
    }
}

fn load_math_texture(ctx: &egui::Context, key: &'static str) -> Option<egui::TextureHandle> {
    let bytes = crate::mathimg::get(key)?;
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    let ci = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], img.as_raw());
    Some(ctx.load_texture(format!("math:{key}"), ci, egui::TextureOptions::LINEAR))
}

impl AtlasApp {
    fn set_status(&mut self, s: impl Into<String>) {
        self.status = Some((s.into(), Instant::now()));
    }

    fn ensure_slot(&mut self, mode: usize, w: u32, h: u32) {
        let need_new = match self.slots.get(&mode) {
            Some(s) => s.target.width != w || s.target.height != h,
            None => true,
        };
        if need_new {
            let target = ViewTarget::new(&self.engine.gpu, &self.engine.rend.view_bgl, w, h);
            let tex = {
                let mut renderer = self.render_state.renderer.write();
                if let Some(old) = self.slots.remove(&mode) {
                    renderer.free_texture(&old.tex);
                }
                renderer.register_native_texture(
                    &self.engine.gpu.device,
                    &target.resolve_view,
                    wgpu::FilterMode::Linear,
                )
            };
            self.slots.insert(mode, ViewSlot { target, tex });
        }
    }

    fn clear_slots(&mut self) {
        let mut renderer = self.render_state.renderer.write();
        for (_, s) in self.slots.drain() {
            renderer.free_texture(&s.tex);
        }
    }

    // ---------- panels ----------

    fn top_bar(&mut self, ctx: &egui::Context) {
        let accent = self.engine.modules[self.engine.active]
            .info()
            .branch
            .style()
            .accent;
        let t = self.engine.timeline.t;
        egui::TopBottomPanel::top("brand")
            .exact_height(52.0)
            .frame(
                egui::Frame::new()
                    .fill(theme::BG_VOID)
                    .inner_margin(egui::Margin::symmetric(16, 8)),
            )
            .show(ctx, |ui| {
                self.brush(ui);
                // The flowing motif behind the brand, in the active branch light.
                let bar = ui.max_rect().expand2(egui::vec2(16.0, 8.0));
                theme::paint_ribbons(ui.painter(), bar, accent, t);
                ui.painter().line_segment(
                    [bar.left_bottom(), bar.right_bottom()],
                    Stroke::new(1.0, theme::GOLD_DIM.gamma_multiply(0.55)),
                );
                ui.horizontal_centered(|ui| {
                    ui.label(
                        RichText::new("REVELATIO  NOETICA")
                            .font(theme::heading_font(23.0))
                            .color(theme::GOLD_BRIGHT),
                    );
                    ui.add_space(14.0);
                    ui.label(
                        RichText::new("A Living Atlas of Mathematics and Natural Philosophy")
                            .font(theme::heading_font(14.5))
                            .color(theme::TEXT_DIM),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!("{:>4.0} fps", self.fps))
                                .font(FontId::monospace(10.0))
                                .color(theme::TEXT_DIM.gamma_multiply(0.8)),
                        );
                        ui.add_space(8.0);
                        if let Some((msg, when)) = &self.status {
                            if when.elapsed().as_secs_f32() < 6.0 {
                                ui.label(RichText::new(msg).color(theme::GOLD).size(11.5));
                            }
                        }
                    });
                });
            });
    }

    fn concept_browser(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("browser")
            .default_width(252.0)
            .frame(
                egui::Frame::new()
                    .fill(theme::BG_PANEL)
                    .inner_margin(egui::Margin::same(10)),
            )
            .show(ctx, |ui| {
                self.brush(ui);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("CONCEPT ATLAS")
                            .font(FontId::proportional(10.5))
                            .color(theme::GOLD_DIM),
                    );
                    let n_mod = self.engine.modules.len();
                    let n_rep: usize = (0..n_mod)
                        .map(|i| self.engine.modules[i].modes().len())
                        .sum();
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!("{n_mod} chambers \u{00B7} {n_rep} views"))
                                .size(9.0)
                                .color(theme::TEXT_DIM.gamma_multiply(0.8)),
                        );
                    });
                });
                ui.add_space(4.0);
                ui.add(
                    egui::TextEdit::singleline(&mut self.search)
                        .hint_text("search the atlas\u{2026}")
                        .desired_width(f32::INFINITY),
                );
                ui.add_space(2.0);
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let needle = self.search.to_lowercase();
                    // (module index, mode index or usize::MAX for module default)
                    let mut switch_to: Option<(usize, usize)> = None;
                    for &branch in Branch::all() {
                        // Each branch lists every representation as an entry.
                        let mut entries: Vec<(usize, usize, String, &'static str)> = Vec::new();
                        let mut n_in_branch = 0usize;
                        for i in 0..self.engine.modules.len() {
                            let info = self.engine.modules[i].info();
                            if info.branch != branch {
                                continue;
                            }
                            for (mi, m) in self.engine.modules[i].modes().iter().enumerate() {
                                n_in_branch += 1;
                                let hit = needle.is_empty()
                                    || info.name.to_lowercase().contains(&needle)
                                    || m.name.to_lowercase().contains(&needle)
                                    || branch.style().name.to_lowercase().contains(&needle);
                                if hit {
                                    entries.push((i, mi, m.name.to_string(), m.desc));
                                }
                            }
                        }
                        if entries.is_empty() {
                            continue;
                        }
                        let st = branch.style();
                        let acc = theme::ink(st.accent);
                        ui.add_space(9.0);
                        ui.horizontal(|ui| {
                            let (r, _) =
                                ui.allocate_exact_size(Vec2::splat(7.0), egui::Sense::hover());
                            ui.painter().circle_filled(r.center(), 2.6, acc);
                            ui.painter().circle_stroke(
                                r.center(),
                                4.6,
                                Stroke::new(0.8, acc.gamma_multiply(0.4)),
                            );
                            ui.label(
                                RichText::new(st.name)
                                    .font(theme::heading_font(15.5))
                                    .color(theme::TEXT_BRIGHT),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.label(
                                        RichText::new(format!("{n_in_branch}"))
                                            .size(9.0)
                                            .color(theme::TEXT_DIM),
                                    );
                                },
                            );
                        });
                        ui.label(RichText::new(st.motto).size(9.5).color(theme::TEXT_DIM));
                        ui.add_space(2.0);
                        let mut last_module = usize::MAX;
                        for (idx, mi, name, desc) in entries {
                            // Module sub-header when it changes.
                            if idx != last_module {
                                last_module = idx;
                                let info = self.engine.modules[idx].info();
                                ui.add_space(3.0);
                                ui.horizontal(|ui| {
                                    ui.add_space(8.0);
                                    ui.label(
                                        RichText::new(info.name.to_uppercase())
                                            .size(8.5)
                                            .color(theme::TEXT_DIM),
                                    );
                                    if info.id == "newton.fluxions" {
                                        ui.label(
                                            RichText::new("FLAGSHIP")
                                                .size(7.5)
                                                .color(theme::GOLD_DIM),
                                        );
                                    }
                                });
                            }
                            let selected =
                                idx == self.engine.active && self.engine.active_modes.contains(&mi);
                            let (rect, resp) = ui.allocate_exact_size(
                                Vec2::new(ui.available_width(), 20.0),
                                egui::Sense::click(),
                            );
                            let hover = resp.hovered();
                            let p = ui.painter_at(rect);
                            if selected || hover {
                                p.rect_filled(
                                    rect,
                                    CornerRadius::same(4),
                                    if selected {
                                        Color32::from_rgba_unmultiplied(255, 255, 255, 56)
                                    } else {
                                        Color32::from_rgba_unmultiplied(255, 255, 255, 26)
                                    },
                                );
                            }
                            if selected {
                                p.line_segment(
                                    [
                                        rect.left_top() + Vec2::new(1.5, 3.0),
                                        rect.left_bottom() + Vec2::new(1.5, -3.0),
                                    ],
                                    Stroke::new(2.0, acc),
                                );
                            }
                            p.text(
                                rect.left_center() + Vec2::new(16.0, 0.0),
                                Align2::LEFT_CENTER,
                                &name,
                                FontId::proportional(12.0),
                                if selected {
                                    acc
                                } else if hover {
                                    theme::TEXT_BRIGHT
                                } else {
                                    theme::TEXT_BODY
                                },
                            );
                            let resp = resp.on_hover_text(desc);
                            if resp.clicked() {
                                switch_to = Some((idx, mi));
                            }
                        }
                    }
                    if let Some((idx, mi)) = switch_to {
                        if idx != self.engine.active {
                            self.engine.set_active(idx);
                        }
                        self.engine.active_modes = vec![mi];
                        self.clear_slots();
                    }
                    ui.add_space(12.0);
                });
            });
    }

    fn inspector_panel(&mut self, ctx: &egui::Context) {
        egui::SidePanel::right("inspector")
            .default_width(310.0)
            .frame(
                egui::Frame::new()
                    .fill(theme::BG_PANEL)
                    .inner_margin(egui::Margin::same(12)),
            )
            .show(ctx, |ui| {
                self.brush(ui);
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let active = self.engine.active;
                    let info = self.engine.modules[active].info();
                    let accent = theme::ink(info.branch.style().accent);

                    ui.label(
                        RichText::new(info.name)
                            .font(theme::heading_font(24.0))
                            .color(accent),
                    );
                    ui.label(
                        RichText::new(info.tagline)
                            .size(11.5)
                            .color(theme::TEXT_DIM),
                    );
                    if let Some((quote, who)) = info.epigraph {
                        ui.add_space(6.0);
                        egui::Frame::new()
                            .fill(theme::BG_INSET)
                            .inner_margin(egui::Margin::same(8))
                            .corner_radius(CornerRadius::same(4))
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new(format!("\u{201C}{quote}\u{201D}"))
                                        .font(theme::heading_font(14.5))
                                        .color(theme::TEXT_BODY),
                                );
                                ui.label(
                                    RichText::new(format!("\u{2014} {who}"))
                                        .size(10.0)
                                        .color(theme::TEXT_DIM),
                                );
                            });
                    }
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(info.description)
                            .size(12.0)
                            .color(theme::TEXT_BODY),
                    );

                    ui.add_space(10.0);
                    section(ui, "SYMBOLIC INSPECTOR");
                    let lines = self.engine.modules[active].inspector(self.engine.timeline.t);
                    egui::Frame::new()
                        .fill(theme::BG_INSET)
                        .stroke(Stroke::new(1.0, theme::HAIRLINE))
                        .inner_margin(egui::Margin::same(10))
                        .corner_radius(CornerRadius::same(4))
                        .show(ui, |ui| {
                            for l in lines {
                                match l.role {
                                    InsRole::Title => {
                                        ui.label(
                                            RichText::new(l.text)
                                                .font(theme::heading_font(15.5))
                                                .color(accent),
                                        );
                                        ui.add_space(2.0);
                                    }
                                    InsRole::Formula => {
                                        let mut drawn = false;
                                        if let Some(key) = l.tex {
                                            let ctx2 = ui.ctx().clone();
                                            let tex = self
                                                .math_tex
                                                .entry(key)
                                                .or_insert_with(|| load_math_texture(&ctx2, key));
                                            if let Some(tex) = tex {
                                                let mut size = tex.size_vec2() * 0.34;
                                                let avail = ui.available_width() - 4.0;
                                                if size.x > avail {
                                                    size *= avail / size.x;
                                                }
                                                ui.add_space(3.0);
                                                ui.add(egui::Image::new((tex.id(), size)));
                                                ui.add_space(3.0);
                                                drawn = true;
                                            }
                                        }
                                        if !drawn {
                                            ui.label(
                                                RichText::new(l.text)
                                                    .font(theme::math_font(14.5))
                                                    .color(theme::TEXT_BRIGHT),
                                            );
                                            ui.add_space(2.0);
                                        }
                                    }
                                    InsRole::Def => {
                                        ui.label(
                                            RichText::new(l.text)
                                                .font(theme::math_font(12.0))
                                                .color(theme::TEXT_BODY),
                                        );
                                    }
                                    InsRole::Value => {
                                        ui.label(
                                            RichText::new(l.text)
                                                .font(FontId::monospace(11.5))
                                                .color(theme::ink(info.branch.style().accent2)),
                                        );
                                    }
                                    InsRole::Note => {
                                        ui.add_space(2.0);
                                        ui.label(
                                            RichText::new(l.text).size(10.5).color(theme::TEXT_DIM),
                                        );
                                    }
                                }
                            }
                        });

                    ui.add_space(10.0);
                    section(ui, "PARAMETERS");
                    self.params_ui(ui);

                    ui.add_space(10.0);
                    section(ui, "OBSERVER FRAME");
                    ui.horizontal(|ui| {
                        if ui.button("Reset cameras").clicked() {
                            for m in self.engine.active_modes.clone() {
                                self.engine.reset_camera(active, m);
                            }
                        }
                        ui.label(
                            RichText::new(
                                "drag: orbit/pan \u{00B7} wheel: zoom \u{00B7} right-drag: pan",
                            )
                            .size(9.5)
                            .color(theme::TEXT_DIM),
                        );
                    });

                    ui.add_space(10.0);
                    section(ui, "EXPORT");
                    self.export_ui(ui);
                    ui.add_space(20.0);
                });
            });
    }

    fn params_ui(&mut self, ui: &mut egui::Ui) {
        let active = self.engine.active;
        let params = self.engine.modules[active].params();
        for group in params.groups() {
            ui.add_space(4.0);
            ui.label(
                RichText::new(group.to_uppercase())
                    .size(9.5)
                    .color(theme::TEXT_DIM),
            );
            let items: Vec<usize> = (0..params.items.len())
                .filter(|&i| params.items[i].group == group)
                .collect();
            for i in items {
                let p = &mut params.items[i];
                match (&p.kind, &mut p.value) {
                    (ParamKind::Slider { min, max, unit }, crate::engine::params::PValue::F(v)) => {
                        ui.horizontal(|ui| {
                            let mut val = *v;
                            let resp = ui.add(
                                egui::Slider::new(&mut val, *min..=*max)
                                    .text("")
                                    .show_value(true)
                                    .suffix(*unit),
                            );
                            if resp.changed() {
                                *v = val;
                            }
                            ui.label(RichText::new(p.label).size(11.5).color(theme::TEXT_BODY));
                        });
                    }
                    (ParamKind::IntSlider { min, max }, crate::engine::params::PValue::I(v)) => {
                        ui.horizontal(|ui| {
                            ui.add(egui::Slider::new(v, *min..=*max).text(""));
                            ui.label(RichText::new(p.label).size(11.5).color(theme::TEXT_BODY));
                        });
                    }
                    (ParamKind::Toggle, crate::engine::params::PValue::B(v)) => {
                        ui.checkbox(v, RichText::new(p.label).size(11.5));
                    }
                    (ParamKind::Choice(options), crate::engine::params::PValue::C(v)) => {
                        ui.horizontal(|ui| {
                            egui::ComboBox::from_id_salt(p.key)
                                .selected_text(options.get(*v).copied().unwrap_or("?"))
                                .show_ui(ui, |ui| {
                                    for (oi, o) in options.iter().enumerate() {
                                        ui.selectable_value(v, oi, *o);
                                    }
                                });
                            ui.label(RichText::new(p.label).size(11.5).color(theme::TEXT_BODY));
                        });
                    }
                    _ => {}
                }
            }
        }
    }

    fn export_ui(&mut self, ui: &mut egui::Ui) {
        let active = self.engine.active;
        let id = self.engine.modules[active].info().id.replace('.', "_");
        let stamp = || {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0)
        };
        ui.horizontal_wrapped(|ui| {
            if ui.button("Still \u{00B7} 1920\u{00D7}1080").clicked() {
                let mode = self.engine.active_modes.first().copied().unwrap_or(0);
                let path = std::path::PathBuf::from(format!("exports/{id}_{}.png", stamp()));
                match self.engine.export_png(mode, 1920, 1080, &path) {
                    Ok(()) => self.set_status(format!("wrote {}", path.display())),
                    Err(e) => self.set_status(format!("export failed: {e}")),
                }
            }
            if ui.button("Still \u{00B7} 4K").clicked() {
                let mode = self.engine.active_modes.first().copied().unwrap_or(0);
                let path = std::path::PathBuf::from(format!("exports/{id}_{}_4k.png", stamp()));
                match self.engine.export_png(mode, 3840, 2160, &path) {
                    Ok(()) => self.set_status(format!("wrote {}", path.display())),
                    Err(e) => self.set_status(format!("export failed: {e}")),
                }
            }
            if ui.button("Video frames \u{00B7} 240").clicked() {
                let mode = self.engine.active_modes.first().copied().unwrap_or(0);
                let dir = std::path::PathBuf::from(format!("exports/frames_{id}_{}", stamp()));
                match self.engine.export_frames(mode, 1920, 1080, 240, 30, &dir) {
                    Ok(()) => self.set_status(format!(
                        "wrote 240 frames + ffmpeg cmd to {}",
                        dir.display()
                    )),
                    Err(e) => self.set_status(format!("frames failed: {e}")),
                }
            }
            if ui.button("Save scene").clicked() {
                let path = std::path::PathBuf::from(format!("exports/{id}_{}.scene.json", stamp()));
                match self.engine.save_scene(&path) {
                    Ok(()) => self.set_status(format!("wrote {}", path.display())),
                    Err(e) => self.set_status(format!("scene save failed: {e}")),
                }
            }
        });
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.scene_path)
                    .hint_text("path to .scene.json")
                    .desired_width(190.0),
            );
            if ui.button("Load").clicked() {
                let p = std::path::PathBuf::from(self.scene_path.clone());
                match self.engine.load_scene(&p) {
                    Ok(()) => {
                        self.clear_slots();
                        self.set_status("scene loaded");
                    }
                    Err(e) => self.set_status(format!("load failed: {e}")),
                }
            }
        });
    }

    fn timeline_panel(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("timeline")
            .exact_height(74.0)
            .frame(
                egui::Frame::new()
                    .fill(theme::BG_PANEL)
                    .inner_margin(egui::Margin::symmetric(14, 6)),
            )
            .show(ctx, |ui| {
                self.brush(ui);
                // Representation deck.
                let active = self.engine.active;
                let modes = self.engine.modules[active].modes();
                let accent = theme::ink(self.engine.modules[active].info().branch.style().accent);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("REPRESENTATIONS")
                            .size(9.5)
                            .color(theme::TEXT_DIM),
                    );
                    let mut changed = false;
                    let mut act = self.engine.active_modes.clone();
                    for (i, m) in modes.iter().enumerate() {
                        let on = act.contains(&i);
                        let text = RichText::new(m.name).size(11.5).color(if on {
                            theme::TEXT_BRIGHT
                        } else {
                            theme::TEXT_DIM
                        });
                        let resp = ui.add(
                            egui::Button::new(text)
                                .fill(if on {
                                    accent.gamma_multiply(0.20)
                                } else {
                                    Color32::TRANSPARENT
                                })
                                .stroke(Stroke::new(1.0, if on { accent } else { theme::HAIRLINE }))
                                .corner_radius(CornerRadius::same(11)),
                        );
                        if resp.on_hover_text(m.desc).clicked() {
                            if on {
                                if act.len() > 1 {
                                    act.retain(|&x| x != i);
                                    changed = true;
                                }
                            } else if act.len() < 4 {
                                act.push(i);
                                changed = true;
                            }
                        }
                    }
                    if changed {
                        self.engine.active_modes = act;
                        self.clear_slots();
                    }
                });

                // Transport.
                let tl = &mut self.engine.timeline;
                ui.horizontal(|ui| {
                    let (label, fill, fg) = if tl.playing {
                        ("Pause", Color32::TRANSPARENT, theme::TEXT_BODY)
                    } else {
                        ("Play", theme::GOLD, theme::BG_INSET)
                    };
                    if ui
                        .add(
                            egui::Button::new(RichText::new(label).size(12.0).color(fg))
                                .fill(fill)
                                .stroke(Stroke::new(
                                    1.0,
                                    if tl.playing {
                                        theme::HAIRLINE_LIT
                                    } else {
                                        theme::GOLD
                                    },
                                ))
                                .corner_radius(CornerRadius::same(12))
                                .min_size(Vec2::new(64.0, 22.0)),
                        )
                        .clicked()
                    {
                        tl.playing = !tl.playing;
                    }
                    if ui.button("\u{21BA} 0").on_hover_text("rewind").clicked() {
                        tl.t = 0.0;
                    }
                    ui.label(
                        RichText::new("t")
                            .font(theme::math_font(13.0))
                            .color(theme::GOLD),
                    );
                    let span = tl.span;
                    ui.style_mut().spacing.slider_width = (ui.available_width() - 420.0).max(160.0);
                    ui.add(egui::Slider::new(&mut tl.t, 0.0..=span).show_value(false));
                    ui.label(
                        RichText::new(format!("{:6.2}s", tl.t))
                            .font(FontId::monospace(11.5))
                            .color(theme::TEXT_BODY),
                    );
                    ui.add_space(8.0);
                    ui.label(RichText::new("speed").size(10.0).color(theme::TEXT_DIM));
                    ui.add(
                        egui::Slider::new(&mut tl.speed, 0.05..=4.0)
                            .show_value(true)
                            .fixed_decimals(2),
                    );
                    ui.label(RichText::new("span").size(10.0).color(theme::TEXT_DIM));
                    ui.add(
                        egui::DragValue::new(&mut tl.span)
                            .range(1.0..=120.0)
                            .speed(0.5)
                            .suffix("s"),
                    );
                    let mode_label = match tl.loop_mode {
                        LoopMode::Loop => "loop",
                        LoopMode::PingPong => "pingpong",
                        LoopMode::Once => "once",
                    };
                    egui::ComboBox::from_id_salt("loopmode")
                        .selected_text(mode_label)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut tl.loop_mode, LoopMode::Loop, "loop");
                            ui.selectable_value(&mut tl.loop_mode, LoopMode::PingPong, "pingpong");
                            ui.selectable_value(&mut tl.loop_mode, LoopMode::Once, "once");
                        });
                });
            });
    }

    fn canvas(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(theme::BG_VOID)
                    .inner_margin(egui::Margin::same(6)),
            )
            .show(ctx, |ui| {
                self.brush(ui);
                let full = ui.available_rect_before_wrap();
                let modes = self.engine.active_modes.clone();
                let n = modes.len().max(1);
                let cols = if n == 1 { 1 } else { 2 };
                let rows = n.div_ceil(cols);
                let gap = 6.0;
                let cell_w = (full.width() - gap * (cols - 1) as f32) / cols as f32;
                let cell_h = (full.height() - gap * (rows - 1) as f32) / rows as f32;
                let ppp = ctx.pixels_per_point();

                for (vi, &mode) in modes.iter().enumerate() {
                    let (r, c) = (vi / cols, vi % cols);
                    let mut rect = Rect::from_min_size(
                        egui::pos2(
                            full.left() + c as f32 * (cell_w + gap),
                            full.top() + r as f32 * (cell_h + gap),
                        ),
                        Vec2::new(cell_w, cell_h),
                    );
                    // Last odd pane spans the full row.
                    if vi == n - 1 && n % 2 == 1 && n > 1 {
                        rect.set_right(full.right());
                    }
                    self.canvas_pane(ui, rect, mode, ppp);
                }
            });
    }

    fn canvas_pane(&mut self, ui: &mut egui::Ui, rect: Rect, mode: usize, ppp: f32) {
        let active = self.engine.active;
        let w = (rect.width() * ppp).round().max(8.0) as u32;
        let h = (rect.height() * ppp).round().max(8.0) as u32;
        self.ensure_slot(mode, w, h);

        // Interaction: modules may claim the primary button on plane views;
        // the camera then moves via shift-drag / right-drag.
        let resp = ui.interact(
            rect,
            egui::Id::new(("canvas", mode)),
            egui::Sense::click_and_drag(),
        );
        let shift = ui.input(|i| i.modifiers.shift);
        let module_claims = {
            let plane = matches!(
                self.engine.camera(active, mode).kind,
                crate::engine::camera::CamKind::Plane { .. }
            );
            plane && self.engine.modules[active].wants_pointer(mode)
        };
        if module_claims && !shift {
            let cam = *self.engine.camera(active, mode);
            let to_world = |pos: egui::Pos2| {
                let ndc = glam::vec2(
                    (pos.x - rect.left()) / rect.width() * 2.0 - 1.0,
                    1.0 - (pos.y - rect.top()) / rect.height() * 2.0,
                );
                cam.unproject_plane(ndc)
            };
            use crate::engine::module::PointerEvent as PE;
            if let Some(pos) = resp.interact_pointer_pos() {
                let w = to_world(pos);
                if resp.drag_started_by(egui::PointerButton::Primary) || resp.clicked() {
                    self.engine.modules[active].pointer(mode, PE::Down(w));
                }
                if resp.dragged_by(egui::PointerButton::Primary) {
                    self.engine.modules[active].pointer(mode, PE::Drag(w));
                }
                if resp.drag_stopped_by(egui::PointerButton::Primary) {
                    self.engine.modules[active].pointer(mode, PE::Up(w));
                }
            }
        }
        {
            let cam = self.engine.camera(active, mode);
            cam.aspect = rect.width() / rect.height().max(1.0);
            if resp.dragged_by(egui::PointerButton::Primary) && (!module_claims || shift) {
                let d = resp.drag_delta();
                if shift {
                    cam.pan(d.x, d.y, (rect.width(), rect.height()));
                } else {
                    cam.drag(d.x, d.y, (rect.width(), rect.height()));
                }
            }
            if resp.dragged_by(egui::PointerButton::Secondary) {
                let d = resp.drag_delta();
                cam.pan(d.x, d.y, (rect.width(), rect.height()));
            }
            if resp.hovered() {
                let scroll = ui.input(|i| i.raw_scroll_delta.y);
                if scroll.abs() > 0.0 {
                    cam.zoom(scroll);
                }
            }
            if resp.double_clicked() {
                self.engine.reset_camera(active, mode);
            }
        }

        // Render into the slot (disjoint fields: engine vs slots).
        if let Some(slot) = self.slots.get(&mode) {
            self.engine.render_view(mode, &slot.target);
        }
        let tex = self.slots[&mode].tex;

        let painter = ui.painter_at(rect);
        painter.image(
            tex,
            rect,
            Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            Color32::WHITE,
        );

        // Engraved module overlay.
        let cam = *self.engine.camera(active, mode);
        let t = self.engine.timeline.t;
        self.engine.modules[active].overlay(&painter, rect, &cam, mode, t);

        // Instrument chrome: frame, corner ticks, mode chip, honesty label.
        let info = self.engine.modules[active].info();
        let accent = info.branch.style().accent;
        painter.rect_stroke(
            rect,
            CornerRadius::same(3),
            Stroke::new(1.0, theme::HAIRLINE),
            egui::StrokeKind::Inside,
        );
        let tick = 9.0;
        let tc = theme::OVERLAY_DIM.gamma_multiply(0.7);
        for (corner, dx, dy) in [
            (rect.left_top(), 1.0, 1.0),
            (rect.right_top(), -1.0, 1.0),
            (rect.left_bottom(), 1.0, -1.0),
            (rect.right_bottom(), -1.0, -1.0),
        ] {
            let c = corner + Vec2::new(dx * 2.0, dy * 2.0);
            painter.line_segment([c, c + Vec2::new(dx * tick, 0.0)], Stroke::new(1.2, tc));
            painter.line_segment([c, c + Vec2::new(0.0, dy * tick)], Stroke::new(1.2, tc));
        }
        let mode_name = self.engine.modules[active]
            .modes()
            .get(mode)
            .map(|m| m.name)
            .unwrap_or("view");
        let title_rect = painter.text(
            rect.left_top() + Vec2::new(12.0, 9.0),
            Align2::LEFT_TOP,
            mode_name,
            theme::heading_font(14.0),
            accent,
        );
        painter.line_segment(
            [
                title_rect.left_bottom() + Vec2::new(0.0, 2.0),
                title_rect.right_bottom() + Vec2::new(0.0, 2.0),
            ],
            Stroke::new(1.0, accent.gamma_multiply(0.45)),
        );
        if let Some(label) = self.engine.modules[active].approx(mode) {
            painter.text(
                rect.left_bottom() + Vec2::new(10.0, -8.0),
                Align2::LEFT_BOTTOM,
                label,
                FontId::proportional(9.5),
                theme::OVERLAY_DIM.gamma_multiply(0.9),
            );
        }
        if let Some(hint) = self.engine.modules[active].pointer_hint(mode) {
            painter.text(
                rect.right_top() + Vec2::new(-10.0, 9.0),
                Align2::RIGHT_TOP,
                format!("\u{2726} {hint}"),
                FontId::proportional(10.0),
                theme::OVERLAY_GOLD.gamma_multiply(0.9),
            );
        }
    }
}

fn section(ui: &mut egui::Ui, name: &str) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(name).size(10.0).color(theme::GOLD_DIM));
        let where_to = ui.available_rect_before_wrap();
        let y = where_to.center().y;
        ui.painter().line_segment(
            [
                egui::pos2(where_to.left() + 4.0, y),
                egui::pos2(where_to.right() - 2.0, y),
            ],
            Stroke::new(1.0, theme::HAIRLINE),
        );
    });
    ui.add_space(2.0);
}

impl eframe::App for AtlasApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame).as_secs_f64().min(0.25);
        self.last_frame = now;
        self.fps = 0.9 * self.fps + 0.1 * (1.0 / dt.max(1e-4)) as f32;

        self.engine.tick(dt);

        self.top_bar(ctx);
        self.concept_browser(ctx);
        self.inspector_panel(ctx);
        self.timeline_panel(ctx);
        self.canvas(ctx);

        ctx.request_repaint();
    }
}
