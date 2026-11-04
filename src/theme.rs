//! Visual identity: black glass, luminous fields, engraved notation.
//! Each branch of mathematics carries its own light.

use egui::{Color32, CornerRadius, FontFamily, FontId, Margin, Shadow, Stroke, Visuals};

// Premium pewter chrome: dark brushed silver, ink text, deep gold.
pub const BG_VOID: Color32 = Color32::from_rgb(112, 115, 122);
pub const BG_PANEL: Color32 = Color32::from_rgb(126, 129, 136);
pub const BG_GLASS: Color32 = Color32::from_rgb(138, 141, 148);
pub const BG_INSET: Color32 = Color32::from_rgb(153, 156, 163);
pub const HAIRLINE: Color32 = Color32::from_rgba_premultiplied(10, 11, 15, 80);
pub const HAIRLINE_LIT: Color32 = Color32::from_rgba_premultiplied(28, 33, 52, 160);
pub const TEXT_DIM: Color32 = Color32::from_rgb(51, 55, 66);
pub const TEXT_BODY: Color32 = Color32::from_rgb(24, 27, 34);
pub const TEXT_BRIGHT: Color32 = Color32::from_rgb(8, 9, 12);
pub const GOLD: Color32 = Color32::from_rgb(136, 99, 30);
pub const GOLD_DIM: Color32 = Color32::from_rgb(104, 80, 34);
/// Lettering gold for text sitting directly on the pewter.
pub const GOLD_BRIGHT: Color32 = Color32::from_rgb(228, 188, 112);

// The GPU canvases stay black glass; their annotations use these.
pub const OVERLAY_BRIGHT: Color32 = Color32::from_rgb(236, 240, 250);
pub const OVERLAY_DIM: Color32 = Color32::from_rgb(148, 156, 176);
pub const OVERLAY_GOLD: Color32 = Color32::from_rgb(230, 190, 110);
pub const CANVAS_FRAME: Color32 = Color32::from_rgba_premultiplied(26, 29, 40, 220);

/// Darken a branch accent for legibility on the pewter chrome.
pub fn ink(c: Color32) -> Color32 {
    let f = |v: u8| (v as f32 * 0.40) as u8;
    Color32::from_rgb(f(c.r()), f(c.g()), f(c.b()))
}

/// Procedural grey marble with charcoal veining and gold accent veins,
/// generated once at startup. Sampled in screen space so the slab runs
/// continuously beneath every panel.
pub fn brushed_texture(ctx: &egui::Context) -> egui::TextureHandle {
    const N: usize = 640;
    fn hash2(ix: i32, iy: i32, seed: u32) -> f32 {
        let mut h = (ix as u32).wrapping_mul(374761393)
            ^ (iy as u32).wrapping_mul(668265263)
            ^ seed.wrapping_mul(2246822519);
        h = (h ^ (h >> 13)).wrapping_mul(1274126177);
        ((h ^ (h >> 16)) & 0xffffff) as f32 / 16777215.0
    }
    fn vnoise(x: f32, y: f32, seed: u32) -> f32 {
        let (ix, iy) = (x.floor() as i32, y.floor() as i32);
        let (fx, fy) = (x - x.floor(), y - y.floor());
        let (sx, sy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
        let a = hash2(ix, iy, seed);
        let b = hash2(ix + 1, iy, seed);
        let c = hash2(ix, iy + 1, seed);
        let d = hash2(ix + 1, iy + 1, seed);
        a + (b - a) * sx + (c - a) * sy + (a - b - c + d) * sx * sy
    }
    fn fbm(x: f32, y: f32, seed: u32, oct: u32) -> f32 {
        let (mut v, mut amp, mut fx, mut fy) = (0.0, 0.5, x, y);
        for _ in 0..oct {
            v += amp * vnoise(fx, fy, seed);
            amp *= 0.5;
            fx *= 2.03;
            fy *= 2.07;
        }
        v
    }
    let mut px = Vec::with_capacity(N * N);
    for j in 0..N {
        for i in 0..N {
            let u = i as f32 / N as f32 * 6.0;
            let v = j as f32 / N as f32 * 6.0;
            // Domain warp gives the veins their meander.
            let wx = fbm(u + 13.7, v, 7, 4) * 2.4;
            let wy = fbm(u, v + 41.3, 11, 4) * 2.4;
            let (pu, pv) = (u + wx, v + wy);
            // Base stone: pale grey clouded by slow fBm, pulled darker.
            let cloud = fbm(pu * 0.7, pv * 0.7, 3, 4);
            let mut r = 172.0 - cloud * 34.0;
            let mut g = 173.0 - cloud * 34.0;
            let mut b = 178.0 - cloud * 32.0;
            // Charcoal veins: contour bands of warped noise.
            let d1 = (fbm(pu * 1.35, pv * 1.35, 23, 5) - 0.5).abs();
            let vein1 = (1.0 - (d1 / 0.045).min(1.0)).powi(2)
                * fbm(pu * 0.5 + 7.0, pv * 0.5, 31, 3).clamp(0.0, 1.0);
            r -= vein1 * 96.0;
            g -= vein1 * 94.0;
            b -= vein1 * 86.0;
            // Secondary fine cracks.
            let d1b = (fbm(pu * 3.1 + 3.3, pv * 3.1, 43, 4) - 0.5).abs();
            let vein1b = (1.0 - (d1b / 0.020).min(1.0)).max(0.0) * 0.5;
            r -= vein1b * 52.0;
            g -= vein1b * 50.0;
            b -= vein1b * 46.0;
            // Gold veins: a sparser, independent branching system.
            let d2 = (fbm(pu * 0.95 + 91.0, pv * 0.95 - 17.0, 57, 5) - 0.5).abs();
            let gold_mask = fbm(pu * 0.4 + 33.0, pv * 0.4, 61, 3);
            let vein2 =
                (1.0 - (d2 / 0.035).min(1.0)).powi(2) * (gold_mask * 1.4 - 0.25).clamp(0.0, 1.0);
            let core = (1.0 - (d2 / 0.012).min(1.0)).max(0.0) * vein2;
            r = r + vein2 * (188.0 - r) + core * 62.0;
            g = g + vein2 * (146.0 - g) + core * 52.0;
            b = b + vein2 * (74.0 - b) + core * 30.0;
            // Faint glitter along the gold.
            if vein2 > 0.25 && hash2(i as i32, j as i32, 77) > 0.994 {
                r += 55.0;
                g += 48.0;
                b += 26.0;
            }
            px.push(Color32::from_rgb(
                r.clamp(0.0, 255.0) as u8,
                g.clamp(0.0, 255.0) as u8,
                b.clamp(0.0, 255.0) as u8,
            ));
        }
    }
    let img = egui::ColorImage {
        size: [N, N],
        source_size: egui::vec2(N as f32, N as f32),
        pixels: px,
    };
    ctx.load_texture(
        "marble",
        img,
        egui::TextureOptions {
            magnification: egui::TextureFilter::Linear,
            minification: egui::TextureFilter::Linear,
            wrap_mode: egui::TextureWrapMode::ClampToEdge,
            mipmap_mode: None,
        },
    )
}

/// Paint the marble slab region for this panel: uv taken from the panel's
/// position on screen, so veins run continuously across the whole window.
pub fn paint_brushed(painter: &egui::Painter, rect: egui::Rect, tex: &egui::TextureHandle) {
    let screen = painter.ctx().screen_rect();
    let uv = egui::Rect::from_min_max(
        egui::pos2(
            (rect.left() - screen.left()) / screen.width().max(1.0),
            (rect.top() - screen.top()) / screen.height().max(1.0),
        ),
        egui::pos2(
            (rect.right() - screen.left()) / screen.width().max(1.0),
            (rect.bottom() - screen.top()) / screen.height().max(1.0),
        ),
    );
    painter.image(tex.id(), rect, uv, Color32::WHITE);
    // Top sheen, bottom shade: a vertical gradient mesh.
    let mut mesh = egui::Mesh::default();
    let top = Color32::from_rgba_unmultiplied(255, 255, 255, 14);
    let bot = Color32::from_rgba_unmultiplied(0, 0, 0, 26);
    let idx = mesh.vertices.len() as u32;
    for (p, c) in [
        (rect.left_top(), top),
        (rect.right_top(), top),
        (rect.right_bottom(), bot),
        (rect.left_bottom(), bot),
    ] {
        mesh.vertices.push(egui::epaint::Vertex {
            pos: p,
            uv: egui::epaint::WHITE_UV,
            color: c,
        });
    }
    mesh.indices
        .extend_from_slice(&[idx, idx + 1, idx + 2, idx, idx + 2, idx + 3]);
    painter.add(egui::Shape::mesh(mesh));
}

/// A branch of mathematics or natural philosophy, with its own luminous identity.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum Branch {
    Fluxions,
    Analysis,
    ComplexAnalysis,
    VectorCalculus,
    DifferentialForms,
    Geometry,
    Topology,
    LinearAlgebra,
    DiffEq,
    Dynamics,
    Waves,
    Electromagnetism,
    Quantum,
    StatMech,
    Relativity,
    Gravitation,
    NumberTheory,
}

pub struct BranchStyle {
    pub name: &'static str,
    /// Primary luminous accent.
    pub accent: Color32,
    /// Secondary accent for counterpoint elements.
    pub accent2: Color32,
    /// Epigraph shown in the concept browser.
    pub motto: &'static str,
}

impl Branch {
    pub fn style(self) -> BranchStyle {
        use Branch::*;
        match self {
            Fluxions => BranchStyle {
                name: "Fluxions & Fluents",
                accent: Color32::from_rgb(230, 185, 100),
                accent2: Color32::from_rgb(140, 200, 255),
                motto: "Quantities generated by motion",
            },
            Analysis => BranchStyle {
                name: "Analysis",
                accent: Color32::from_rgb(120, 200, 255),
                accent2: Color32::from_rgb(255, 170, 120),
                motto: "The taming of the infinite",
            },
            ComplexAnalysis => BranchStyle {
                name: "Complex Analysis",
                accent: Color32::from_rgb(255, 110, 200),
                accent2: Color32::from_rgb(110, 255, 210),
                motto: "One plane, every direction of number",
            },
            VectorCalculus => BranchStyle {
                name: "Vector Calculus",
                accent: Color32::from_rgb(90, 220, 160),
                accent2: Color32::from_rgb(120, 160, 255),
                motto: "Fields, flux, and circulation",
            },
            DifferentialForms => BranchStyle {
                name: "Differential Forms",
                accent: Color32::from_rgb(110, 220, 190),
                accent2: Color32::from_rgb(220, 150, 90),
                motto: "The boundary of a boundary is zero",
            },
            Geometry => BranchStyle {
                name: "Differential Geometry",
                accent: Color32::from_rgb(150, 190, 255),
                accent2: Color32::from_rgb(230, 230, 250),
                motto: "Curvature is what remains",
            },
            Topology => BranchStyle {
                name: "Topology & Knots",
                accent: Color32::from_rgb(200, 130, 255),
                accent2: Color32::from_rgb(255, 170, 130),
                motto: "Properties that survive deformation",
            },
            LinearAlgebra => BranchStyle {
                name: "Linear Algebra",
                accent: Color32::from_rgb(255, 120, 120),
                accent2: Color32::from_rgb(130, 200, 255),
                motto: "Space, stretched and turned",
            },
            DiffEq => BranchStyle {
                name: "Differential Equations",
                accent: Color32::from_rgb(85, 210, 185),
                accent2: Color32::from_rgb(255, 170, 100),
                motto: "Change, written as law",
            },
            Dynamics => BranchStyle {
                name: "Dynamics & Chaos",
                accent: Color32::from_rgb(255, 145, 90),
                accent2: Color32::from_rgb(110, 220, 200),
                motto: "Sensitive dependence, eternal return",
            },
            Waves => BranchStyle {
                name: "Waves & Fields",
                accent: Color32::from_rgb(90, 200, 235),
                accent2: Color32::from_rgb(240, 200, 120),
                motto: "Disturbance, propagated",
            },
            Electromagnetism => BranchStyle {
                name: "Electromagnetism",
                accent: Color32::from_rgb(90, 205, 255),
                accent2: Color32::from_rgb(255, 120, 190),
                motto: "Charge, field, and light",
            },
            StatMech => BranchStyle {
                name: "Statistical Mechanics",
                accent: Color32::from_rgb(255, 175, 70),
                accent2: Color32::from_rgb(140, 165, 255),
                motto: "Order out of chance, one collision at a time",
            },
            NumberTheory => BranchStyle {
                name: "Number Theory",
                accent: Color32::from_rgb(170, 240, 130),
                accent2: Color32::from_rgb(230, 195, 110),
                motto: "The integers, seen from far away",
            },
            Quantum => BranchStyle {
                name: "Quantum Mechanics",
                accent: Color32::from_rgb(160, 130, 255),
                accent2: Color32::from_rgb(255, 200, 110),
                motto: "Amplitude, phase, and chance",
            },
            Relativity => BranchStyle {
                name: "Special Relativity",
                accent: Color32::from_rgb(255, 200, 90),
                accent2: Color32::from_rgb(120, 180, 235),
                motto: "The electrodynamics of moving bodies",
            },
            Gravitation => BranchStyle {
                name: "General Relativity",
                accent: Color32::from_rgb(255, 150, 80),
                accent2: Color32::from_rgb(150, 180, 230),
                motto: "Geometry tells matter how to move",
            },
        }
    }

    pub fn all() -> &'static [Branch] {
        use Branch::*;
        &[
            Fluxions,
            ComplexAnalysis,
            Analysis,
            VectorCalculus,
            DifferentialForms,
            Geometry,
            Topology,
            LinearAlgebra,
            DiffEq,
            Dynamics,
            Waves,
            Electromagnetism,
            Quantum,
            StatMech,
            Relativity,
            Gravitation,
            NumberTheory,
        ]
    }
}

/// Font family handles.
pub fn serif() -> FontFamily {
    FontFamily::Name("serif".into())
}
pub fn math() -> FontFamily {
    FontFamily::Name("math".into())
}

pub fn heading_font(size: f32) -> FontId {
    FontId::new(size, serif())
}
pub fn math_font(size: f32) -> FontId {
    FontId::new(size, math())
}

pub fn install_fonts(ctx: &egui::Context) {
    use egui::{FontData, FontDefinitions};
    let mut fonts = FontDefinitions::default();

    fonts.font_data.insert(
        "cormorant".into(),
        std::sync::Arc::new(FontData::from_static(include_bytes!(
            "../assets/fonts/CormorantGaramond.ttf"
        ))),
    );
    fonts.font_data.insert(
        "inter".into(),
        std::sync::Arc::new(FontData::from_static(include_bytes!(
            "../assets/fonts/Inter.ttf"
        ))),
    );
    fonts.font_data.insert(
        "spectral".into(),
        std::sync::Arc::new(FontData::from_static(include_bytes!(
            "../assets/fonts/Spectral.ttf"
        ))),
    );
    fonts.font_data.insert(
        "spectralmed".into(),
        std::sync::Arc::new(FontData::from_static(include_bytes!(
            "../assets/fonts/SpectralMedium.ttf"
        ))),
    );
    fonts.font_data.insert(
        "notomath".into(),
        std::sync::Arc::new(FontData::from_static(include_bytes!(
            "../assets/fonts/NotoSansMath.ttf"
        ))),
    );

    // Body text: Spectral (a bookish screen serif), math + Inter fallbacks.
    let prop = fonts.families.entry(FontFamily::Proportional).or_default();
    prop.insert(0, "spectralmed".into());
    prop.insert(1, "notomath".into());
    prop.insert(2, "inter".into());

    // Monospace also needs the math glyphs (dotted fluxions, integrals).
    let mono = fonts.families.entry(FontFamily::Monospace).or_default();
    mono.push("notomath".into());
    mono.push("inter".into());

    // Display serif for headings and epigraphs.
    fonts.families.insert(
        FontFamily::Name("serif".into()),
        vec!["cormorant".into(), "notomath".into(), "inter".into()],
    );
    // Engraved notation: math symbols first.
    fonts.families.insert(
        FontFamily::Name("math".into()),
        vec!["notomath".into(), "inter".into()],
    );

    ctx.set_fonts(fonts);
}

pub fn install_style(ctx: &egui::Context) {
    let mut visuals = Visuals::light();
    visuals.override_text_color = Some(TEXT_BODY);
    visuals.panel_fill = BG_PANEL;
    visuals.window_fill = BG_GLASS;
    visuals.extreme_bg_color = BG_INSET;
    visuals.faint_bg_color = Color32::from_rgba_unmultiplied(255, 255, 255, 60);
    visuals.window_stroke = Stroke::new(1.0, HAIRLINE);
    visuals.window_shadow = Shadow {
        offset: [0, 6],
        blur: 22,
        spread: 0,
        color: Color32::from_black_alpha(60),
    };
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, HAIRLINE);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT_BODY);
    visuals.widgets.inactive.bg_fill = Color32::from_rgb(144, 147, 154);
    visuals.widgets.inactive.weak_bg_fill = Color32::from_rgb(136, 139, 146);
    visuals.widgets.inactive.bg_stroke =
        Stroke::new(1.0, Color32::from_rgba_unmultiplied(20, 23, 32, 70));
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT_BODY);
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(154, 157, 164);
    visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(148, 151, 158);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, HAIRLINE_LIT);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.2, TEXT_BRIGHT);
    visuals.widgets.active.bg_fill = Color32::from_rgb(162, 165, 172);
    visuals.widgets.active.fg_stroke = Stroke::new(1.2, TEXT_BRIGHT);
    visuals.widgets.open.bg_fill = Color32::from_rgb(150, 153, 160);
    visuals.selection.bg_fill = Color32::from_rgba_unmultiplied(136, 99, 30, 80);
    visuals.selection.stroke = Stroke::new(1.0, GOLD);
    visuals.hyperlink_color = GOLD;
    visuals.slider_trailing_fill = true;

    let mut style = (*ctx.style()).clone();
    style.visuals = visuals;
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.window_margin = Margin::same(12);
    style.spacing.button_padding = egui::vec2(10.0, 4.0);
    style.spacing.slider_width = 140.0;
    let corner = CornerRadius::same(4);
    style.visuals.widgets.noninteractive.corner_radius = corner;
    style.visuals.widgets.inactive.corner_radius = corner;
    style.visuals.widgets.hovered.corner_radius = corner;
    style.visuals.widgets.active.corner_radius = corner;
    style.visuals.widgets.open.corner_radius = corner;

    use egui::TextStyle::*;
    style.text_styles.insert(Heading, heading_font(24.0));
    style.text_styles.insert(Body, FontId::proportional(13.0));
    style.text_styles.insert(Monospace, FontId::monospace(12.0));
    style.text_styles.insert(Button, FontId::proportional(13.0));
    style.text_styles.insert(Small, FontId::proportional(10.5));
    ctx.set_style(style);
}

/// Flowing ribbon of fine luminous curves — the house motif. Painted behind
/// panel chrome; cheap (a few hundred segments) and animated by `t`.
pub fn paint_ribbons(painter: &egui::Painter, rect: egui::Rect, tint: Color32, t: f64) {
    let n_lines = 11;
    let n_seg = 46;
    let w = rect.width();
    let h = rect.height();
    // Engraved into the silver: darkened strokes, denser toward the center.
    let tint = ink(tint);
    for i in 0..n_lines {
        let fi = i as f32 / (n_lines - 1) as f32;
        let alpha = (60.0 - (fi - 0.5).abs() * 70.0).max(18.0) as u8;
        let color = Color32::from_rgba_unmultiplied(tint.r(), tint.g(), tint.b(), alpha);
        let mut pts = Vec::with_capacity(n_seg + 1);
        for s in 0..=n_seg {
            let x = s as f32 / n_seg as f32;
            let ph = t as f32 * 0.13;
            // Two incommensurate waves; lines fan apart where the envelope swells.
            let env = ((x * 2.4 + 0.4).sin() * 0.5 + 0.5) * 0.8 + 0.2;
            let y = 0.5
                + (x * 6.9 + ph + fi * 0.55).sin() * 0.16 * env
                + (x * 3.1 - ph * 0.7 + fi * 1.35).sin() * 0.22 * env
                + (fi - 0.5) * 0.30 * env;
            pts.push(egui::pos2(
                rect.left() + x * w,
                rect.top() + y.clamp(0.04, 0.96) * h,
            ));
        }
        painter.add(egui::Shape::line(pts, Stroke::new(1.0, color)));
    }
}

/// Color32 -> linear-space [f32;4] for GPU clear colors / uniforms.
pub fn to_linear(c: Color32) -> [f32; 4] {
    let f = |u: u8| {
        let s = u as f32 / 255.0;
        if s <= 0.04045 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    };
    [f(c.r()), f(c.g()), f(c.b()), c.a() as f32 / 255.0]
}

pub fn to_vec4(c: Color32) -> [f32; 4] {
    [
        c.r() as f32 / 255.0,
        c.g() as f32 / 255.0,
        c.b() as f32 / 255.0,
        c.a() as f32 / 255.0,
    ]
}
