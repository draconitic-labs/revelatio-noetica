//! Typed, serializable parameter sets. Modules declare their controls once;
//! the shell auto-builds grouped UI, and scene files round-trip the values.

use std::collections::HashMap;

#[derive(Clone, Debug)]
pub enum ParamKind {
    Slider {
        min: f64,
        max: f64,
        unit: &'static str,
    },
    IntSlider {
        min: i64,
        max: i64,
    },
    Toggle,
    Choice(Vec<&'static str>),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PValue {
    F(f64),
    I(i64),
    B(bool),
    C(usize),
}

#[derive(Clone, Debug)]
pub struct Param {
    pub key: &'static str,
    pub label: &'static str,
    pub group: &'static str,
    pub kind: ParamKind,
    pub value: PValue,
}

#[derive(Clone, Debug, Default)]
pub struct Params {
    pub items: Vec<Param>,
    index: HashMap<&'static str, usize>,
}

impl Params {
    pub fn new() -> Self {
        Self::default()
    }

    fn push(&mut self, p: Param) -> &mut Self {
        self.index.insert(p.key, self.items.len());
        self.items.push(p);
        self
    }

    #[allow(clippy::too_many_arguments)]
    pub fn slider(
        &mut self,
        key: &'static str,
        label: &'static str,
        group: &'static str,
        min: f64,
        max: f64,
        value: f64,
        unit: &'static str,
    ) -> &mut Self {
        self.push(Param {
            key,
            label,
            group,
            kind: ParamKind::Slider { min, max, unit },
            value: PValue::F(value),
        })
    }

    pub fn int(
        &mut self,
        key: &'static str,
        label: &'static str,
        group: &'static str,
        min: i64,
        max: i64,
        value: i64,
    ) -> &mut Self {
        self.push(Param {
            key,
            label,
            group,
            kind: ParamKind::IntSlider { min, max },
            value: PValue::I(value),
        })
    }

    pub fn toggle(
        &mut self,
        key: &'static str,
        label: &'static str,
        group: &'static str,
        value: bool,
    ) -> &mut Self {
        self.push(Param {
            key,
            label,
            group,
            kind: ParamKind::Toggle,
            value: PValue::B(value),
        })
    }

    pub fn choice(
        &mut self,
        key: &'static str,
        label: &'static str,
        group: &'static str,
        options: Vec<&'static str>,
        value: usize,
    ) -> &mut Self {
        self.push(Param {
            key,
            label,
            group,
            kind: ParamKind::Choice(options),
            value: PValue::C(value),
        })
    }

    pub fn f(&self, key: &str) -> f64 {
        match self.get(key) {
            Some(PValue::F(v)) => v,
            Some(PValue::I(v)) => v as f64,
            _ => 0.0,
        }
    }
    pub fn i(&self, key: &str) -> i64 {
        match self.get(key) {
            Some(PValue::I(v)) => v,
            Some(PValue::F(v)) => v as i64,
            _ => 0,
        }
    }
    pub fn b(&self, key: &str) -> bool {
        matches!(self.get(key), Some(PValue::B(true)))
    }
    pub fn c(&self, key: &str) -> usize {
        match self.get(key) {
            Some(PValue::C(v)) => v,
            _ => 0,
        }
    }

    fn get(&self, key: &str) -> Option<PValue> {
        self.index.get(key).map(|&i| self.items[i].value)
    }

    pub fn set_f(&mut self, key: &str, v: f64) {
        self.set(key, PValue::F(v));
    }

    pub fn to_json(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();
        for p in &self.items {
            let v = match p.value {
                PValue::F(v) => serde_json::json!(v),
                PValue::I(v) => serde_json::json!(v),
                PValue::B(v) => serde_json::json!(v),
                PValue::C(v) => serde_json::json!(v),
            };
            map.insert(p.key.to_string(), v);
        }
        serde_json::Value::Object(map)
    }

    /// Overwrite values from JSON. Scene files and links are untrusted, so a
    /// missing key or a value of the wrong type leaves the parameter as it is,
    /// and numbers are clamped into the declared range.
    pub fn load_json(&mut self, v: &serde_json::Value) {
        let Some(map) = v.as_object() else { return };
        for p in &mut self.items {
            let Some(jv) = map.get(p.key) else { continue };
            let value = match &p.kind {
                ParamKind::Slider { .. } => jv.as_f64().map(PValue::F),
                ParamKind::IntSlider { .. } => jv.as_i64().map(PValue::I),
                ParamKind::Toggle => jv.as_bool().map(PValue::B),
                ParamKind::Choice(_) => jv.as_u64().map(|c| PValue::C(c as usize)),
            };
            if let Some(value) = value.and_then(|v| p.fit(v)) {
                p.value = value;
            }
        }
    }

    pub fn param(&self, key: &str) -> Option<&Param> {
        self.index.get(key).map(|&i| &self.items[i])
    }

    /// Set a value if the key exists and the type matches, clamped to range.
    pub fn set(&mut self, key: &str, value: PValue) -> bool {
        let Some(&i) = self.index.get(key) else {
            return false;
        };
        match self.items[i].fit(value) {
            Some(v) => {
                self.items[i].value = v;
                true
            }
            None => false,
        }
    }

    /// Where a slider sits within its range, in 0..=1.
    pub fn fraction(&self, key: &str) -> Option<f64> {
        let p = self.param(key)?;
        match (&p.kind, p.value) {
            (ParamKind::Slider { min, max, .. }, PValue::F(v)) if max > min => {
                Some((v - min) / (max - min))
            }
            (ParamKind::IntSlider { min, max }, PValue::I(v)) if max > min => {
                Some((v - min) as f64 / (max - min) as f64)
            }
            _ => None,
        }
    }

    /// Move a slider to a position within its range, in 0..=1.
    pub fn set_fraction(&mut self, key: &str, f: f64) {
        let Some(&i) = self.index.get(key) else {
            return;
        };
        let f = f.clamp(0.0, 1.0);
        let p = &mut self.items[i];
        match p.kind {
            ParamKind::Slider { min, max, .. } => p.value = PValue::F(min + f * (max - min)),
            ParamKind::IntSlider { min, max } => {
                p.value = PValue::I(min + (f * (max - min) as f64).round() as i64)
            }
            _ => {}
        }
    }

    /// Draw new values for the given keys. Sliders and choices move; toggles
    /// stay, because they switch overlays rather than explore the system.
    pub fn randomize(&mut self, keys: &[&str], mut uniform: impl FnMut() -> f64) {
        for key in keys {
            let Some(&i) = self.index.get(key) else {
                continue;
            };
            let p = &mut self.items[i];
            match &p.kind {
                ParamKind::Slider { min, max, .. } => {
                    p.value = PValue::F(min + uniform() * (max - min));
                }
                ParamKind::IntSlider { min, max } => {
                    let span = (max - min + 1) as f64;
                    p.value = PValue::I(min + (uniform() * span) as i64).min_int(*max);
                }
                ParamKind::Choice(options) if !options.is_empty() => {
                    let pick = (uniform() * options.len() as f64) as usize;
                    p.value = PValue::C(pick.min(options.len() - 1));
                }
                _ => {}
            }
        }
    }
}

impl PValue {
    fn min_int(self, max: i64) -> Self {
        match self {
            PValue::I(v) => PValue::I(v.min(max)),
            other => other,
        }
    }
}

impl Param {
    /// `value` adjusted to fit this parameter, or None if the type is wrong.
    fn fit(&self, value: PValue) -> Option<PValue> {
        match (&self.kind, value) {
            (ParamKind::Slider { min, max, .. }, PValue::F(v)) if v.is_finite() => {
                Some(PValue::F(v.clamp(*min, *max)))
            }
            (ParamKind::IntSlider { min, max }, PValue::I(v)) => {
                Some(PValue::I(v.clamp(*min, *max)))
            }
            (ParamKind::Toggle, PValue::B(v)) => Some(PValue::B(v)),
            (ParamKind::Choice(options), PValue::C(v)) if !options.is_empty() => {
                Some(PValue::C(v.min(options.len() - 1)))
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Params {
        let mut p = Params::new();
        p.slider("a", "A", "G", -1.0, 3.0, 1.0, "");
        p.int("n", "N", "G", 2, 9, 5);
        p.toggle("on", "On", "G", true);
        p.choice("c", "C", "G", vec!["x", "y", "z"], 1);
        p
    }

    #[test]
    fn json_round_trips() {
        let mut p = sample();
        p.set("a", PValue::F(2.5));
        p.set("n", PValue::I(7));
        p.set("on", PValue::B(false));
        p.set("c", PValue::C(2));
        let mut q = sample();
        q.load_json(&p.to_json());
        assert_eq!(q.f("a"), 2.5);
        assert_eq!(q.i("n"), 7);
        assert!(!q.b("on"));
        assert_eq!(q.c("c"), 2);
    }

    #[test]
    fn untrusted_json_is_clamped_or_ignored() {
        let mut p = sample();
        p.load_json(&serde_json::json!({
            "a": 1e9, "n": "seven", "on": 3, "c": 99, "unknown": 1
        }));
        assert_eq!(p.f("a"), 3.0, "clamped to max");
        assert_eq!(p.i("n"), 5, "wrong type keeps the old value");
        assert!(p.b("on"), "wrong type keeps the old value");
        assert_eq!(p.c("c"), 2, "clamped to the last option");
    }

    #[test]
    fn set_rejects_the_wrong_type() {
        let mut p = sample();
        assert!(!p.set("a", PValue::I(1)));
        assert!(!p.set("missing", PValue::F(1.0)));
        assert!(!p.set("a", PValue::F(f64::NAN)));
        assert_eq!(p.f("a"), 1.0);
    }

    #[test]
    fn fraction_round_trips_and_stays_in_range() {
        let mut p = sample();
        assert_eq!(p.fraction("a"), Some(0.5));
        p.set_fraction("a", 0.25);
        assert_eq!(p.f("a"), 0.0);
        p.set_fraction("a", 7.0);
        assert_eq!(p.f("a"), 3.0);
        p.set_fraction("n", 1.0);
        assert_eq!(p.i("n"), 9);
        assert_eq!(p.fraction("on"), None);
    }

    #[test]
    fn randomize_stays_in_range_and_leaves_toggles() {
        let mut p = sample();
        let keys = ["a", "n", "on", "c"];
        for u in [0.0, 0.5, 0.999_999_9] {
            p.randomize(&keys, || u);
            assert!((-1.0..=3.0).contains(&p.f("a")));
            assert!((2..=9).contains(&p.i("n")));
            assert!(p.c("c") <= 2);
            assert!(p.b("on"));
        }
    }
}
