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
        if let Some(&i) = self.index.get(key) {
            self.items[i].value = PValue::F(v);
        }
    }
    pub fn set_c(&mut self, key: &str, v: usize) {
        if let Some(&i) = self.index.get(key) {
            self.items[i].value = PValue::C(v);
        }
    }

    /// Ordered list of distinct groups, preserving declaration order.
    pub fn groups(&self) -> Vec<&'static str> {
        let mut out = Vec::new();
        for p in &self.items {
            if !out.contains(&p.group) {
                out.push(p.group);
            }
        }
        out
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

    pub fn load_json(&mut self, v: &serde_json::Value) {
        let Some(map) = v.as_object() else { return };
        for p in &mut self.items {
            let Some(jv) = map.get(p.key) else { continue };
            p.value = match (&p.kind, jv) {
                (ParamKind::Slider { min, max, .. }, v) => {
                    PValue::F(v.as_f64().unwrap_or(0.0).clamp(*min, *max))
                }
                (ParamKind::IntSlider { min, max }, v) => {
                    PValue::I(v.as_i64().unwrap_or(0).clamp(*min, *max))
                }
                (ParamKind::Toggle, v) => PValue::B(v.as_bool().unwrap_or(false)),
                (ParamKind::Choice(opts), v) => {
                    PValue::C((v.as_u64().unwrap_or(0) as usize).min(opts.len().saturating_sub(1)))
                }
            };
        }
    }
}
