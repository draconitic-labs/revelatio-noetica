//! LaTeX-rendered formula images, baked into the binary.
//! Regenerate with `tools/render_math.sh` (or `tools/render_math.ps1` on Windows) after editing assets/math/manifest.txt.

include!("mathimg_gen.rs");

pub fn get(key: &str) -> Option<&'static [u8]> {
    MATH_PNGS.iter().find(|(k, _)| *k == key).map(|(_, b)| *b)
}
