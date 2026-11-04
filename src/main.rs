mod app;
mod atlas;
mod engine;
mod mathimg;
mod modules;
mod session;
mod theme;
mod widgets;

use std::path::PathBuf;

use engine::gpu::GpuContext;
use engine::module::{Engine, ViewRef};

fn main() {
    env_logger::init();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match opening_scene(&args) {
        Some(Ok(scene)) => app::run(Some(scene)),
        Some(Err(e)) => Err(e),
        None if args.is_empty() => app::run(None),
        None => run_cli(&args),
    };
    if let Err(e) = result {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}

/// `revelatio <link>` and `revelatio <file.scene.json>` open the window at that
/// scene, so links work from a shell or a desktop launcher.
fn opening_scene(args: &[String]) -> Option<anyhow::Result<serde_json::Value>> {
    let first = args.first()?;
    if first.starts_with("--") {
        return None;
    }
    if session::is_link(first) {
        return Some(session::decode_link(first));
    }
    Some(
        std::fs::read_to_string(first)
            .map_err(anyhow::Error::from)
            .and_then(|text| serde_json::from_str(&text).map_err(anyhow::Error::from))
            .map_err(|e| e.context(format!("could not open scene {first}"))),
    )
}

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1).cloned())
}

fn parse_size(s: &str) -> (u32, u32) {
    let mut it = s.split(['x', 'X']);
    let w = it.next().and_then(|v| v.parse().ok()).unwrap_or(1280);
    let h = it.next().and_then(|v| v.parse().ok()).unwrap_or(720);
    (w, h)
}

fn headless_engine() -> anyhow::Result<Engine> {
    let gpu = GpuContext::headless()?;
    Ok(Engine::new(gpu, modules::all()))
}

fn warmup(e: &mut Engine, t: f64) {
    let steps = 90;
    for i in 1..=steps {
        let tt = t * i as f64 / steps as f64;
        e.timeline.t = tt;
        let gpu = e.gpu.clone();
        for m in e.open_modules() {
            e.modules[m].update(&gpu, tt, t / steps as f64);
        }
    }
}

fn run_cli(args: &[String]) -> anyhow::Result<()> {
    match args[0].as_str() {
        "--list" => {
            let e = headless_engine()?;
            for i in 0..e.modules.len() {
                let info = e.modules[i].info();
                println!("{}  \u{2014}  {} ({})", info.id, info.name, info.tagline);
                for (j, m) in e.modules[i].modes().iter().enumerate() {
                    println!("    [{j}] {} ({}): {}", m.id, m.name, m.desc);
                }
            }
            Ok(())
        }
        "--export" => {
            let which = args.get(1).map(String::as_str).unwrap_or("all");
            let (w, h) = parse_size(&flag(args, "--size").unwrap_or_default());
            let t = flag(args, "--t").and_then(|v| v.parse().ok()).unwrap_or(2.5);
            let out = PathBuf::from(flag(args, "--out").unwrap_or_else(|| "exports".into()));
            let mode_sel: Option<usize> = flag(args, "--mode").and_then(|v| v.parse().ok());
            let mut e = headless_engine()?;
            let ids: Vec<(usize, String)> = (0..e.modules.len())
                .map(|i| (i, e.modules[i].info().id.to_string()))
                .filter(|(_, id)| which == "all" || id == which)
                .collect();
            anyhow::ensure!(!ids.is_empty(), "no module matches '{which}' (try --list)");
            for (idx, id) in ids {
                e.set_active(idx);
                warmup(&mut e, t);
                let n_modes = e.modules[idx].modes().len();
                for m in 0..n_modes {
                    if let Some(sel) = mode_sel {
                        if m != sel {
                            continue;
                        }
                    }
                    let mode_id = e.modules[idx].modes()[m].id;
                    let path = out.join(format!("{}_{}.png", id.replace('.', "_"), mode_id));
                    e.export_png(ViewRef { module: idx, mode: m }, w, h, &path)?;
                    println!("wrote {}", path.display());
                }
            }
            Ok(())
        }
        "--frames" => {
            let which = args.get(1).cloned().unwrap_or_default();
            let (w, h) = parse_size(&flag(args, "--size").unwrap_or_default());
            let n: usize = flag(args, "--n").and_then(|v| v.parse().ok()).unwrap_or(150);
            let fps: u32 = flag(args, "--fps").and_then(|v| v.parse().ok()).unwrap_or(30);
            let mode: usize = flag(args, "--mode").and_then(|v| v.parse().ok()).unwrap_or(0);
            let out = PathBuf::from(
                flag(args, "--out").unwrap_or_else(|| format!("exports/frames_{which}")),
            );
            let mut e = headless_engine()?;
            let idx = (0..e.modules.len())
                .find(|&i| e.modules[i].info().id == which)
                .ok_or_else(|| anyhow::anyhow!("unknown module '{which}' (try --list)"))?;
            e.set_active(idx);
            e.export_frames(ViewRef { module: idx, mode }, (w, h), n, fps, &out)?;
            println!("wrote {n} frames to {}", out.display());
            Ok(())
        }
        "--scene" => {
            let path = PathBuf::from(args.get(1).cloned().unwrap_or_default());
            let mut e = headless_engine()?;
            e.load_scene(&path)?;
            let (w, h) = parse_size(&flag(args, "--size").unwrap_or_default());
            let out = PathBuf::from(flag(args, "--out").unwrap_or_else(|| "exports/scene.png".into()));
            let t = e.timeline.t;
            warmup(&mut e, t.max(0.01));
            e.timeline.t = t;
            e.export_png(e.focused(), w, h, &out)?;
            println!("wrote {}", out.display());
            Ok(())
        }
        other => anyhow::bail!(
            "unknown command '{other}'. Usage:\n  revelatio                      launch the atlas\n  revelatio <file.scene.json>    launch at a saved scene\n  revelatio '<revelatio:scene?...>'  launch at a copied scene link\n  revelatio --list               list modules & representations\n  revelatio --export <id|all> [--mode N] [--size WxH] [--t sec] [--out dir]\n  revelatio --frames <id> [--mode N] [--n 150] [--fps 30] [--size WxH] [--out dir]\n  revelatio --scene <file.json> [--size WxH] [--out png]"
        ),
    }
}
