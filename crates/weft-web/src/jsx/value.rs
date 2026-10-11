//! Values known at compile time (`lit`) or computed by `js` at run time.

use crate::js::{V, js_round};

// ---- Compile-time values ----

/// A value known at compile time (`lit`) or computed by `js` at run time; `bool` marks an
/// expression that already yields a boolean.
#[derive(Clone, Debug)]
pub(super) struct E {
    pub(super) js: String,
    pub(super) lit: Option<V>,
    pub(super) bool: bool,
}

pub(super) fn lit_e(v: V) -> E {
    E {
        js: v.literal(),
        lit: Some(v),
        bool: false,
    }
}

pub(super) fn dyn_e(js: impl Into<String>, bool: bool) -> E {
    E {
        js: js.into(),
        lit: None,
        bool,
    }
}

impl E {
    pub(super) fn is(&self, v: &V) -> bool {
        self.lit.as_ref() == Some(v)
    }
}

pub(super) const TRUE: V = V::Bool(true);
pub(super) const FALSE: V = V::Bool(false);

pub(super) fn str_v(s: impl Into<String>) -> V {
    V::Str(s.into())
}

/// A number prop brought into its declared range (SPEC §5.1), as the renderer's `prop` does.
pub(crate) fn clamp_number(v: &V, integer: bool, min: Option<f64>, max: Option<f64>) -> V {
    let V::Num(x) = v else {
        return v.clone();
    };
    if !x.is_finite() {
        return V::Undef;
    }
    let mut out = if integer { js_round(*x) } else { *x };
    if let Some(min) = min {
        out = out.max(min);
    }
    if let Some(max) = max {
        out = out.min(max);
    }
    V::Num(out)
}

pub(super) fn opt_literal(n: Option<f64>) -> String {
    n.map_or_else(|| "undefined".to_owned(), |n| V::Num(n).literal())
}
