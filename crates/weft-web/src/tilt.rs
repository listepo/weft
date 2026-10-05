//! The 3D tilt of SPEC §2.2 as a CSS `transform`, written by the generators and read back by the
//! importers.

use weft_core::{Map, TILT_PROPS, Value, js_number};

/// The tilt of an element's props as CSS.
pub(crate) fn css_of(props: &Map<Value>) -> Option<String> {
    css(|name| match props.get(name) {
        Some(Value::Number(n)) => Some(*n),
        _ => None,
    })
}

/// CSS function and unit for each of `TILT_PROPS`.
const FUNCTIONS: [(&str, &str); 4] = [
    ("perspective", "px"),
    ("rotateX", "deg"),
    ("rotateY", "deg"),
    ("rotateZ", "deg"),
];

/// `perspective(p) rotateX(x) rotateY(y) rotateZ(z)` for the tilt attributes an element has;
/// `None` for an element with none. The attributes are literals, so this is known at generation
/// time. `get` reads a number prop by name.
pub(crate) fn css(get: impl Fn(&str) -> Option<f64>) -> Option<String> {
    let parts: Vec<String> = TILT_PROPS
        .iter()
        .zip(FUNCTIONS)
        .filter_map(|(name, (function, unit))| {
            Some(format!("{function}({}{unit})", js_number(get(name)?)))
        })
        .collect();
    (!parts.is_empty()).then(|| parts.join(" "))
}

/// The attributes a `transform` value stands for, when it is exactly a list of the functions
/// `css` writes, each at most once (in any order); `None` for anything else, which is not ours to
/// read.
pub(crate) fn parse(transform: &str) -> Option<Vec<(&'static str, f64)>> {
    let mut found: Vec<(&'static str, f64)> = Vec::new();
    let mut rest = transform.trim();
    if rest.is_empty() {
        return None;
    }
    while !rest.is_empty() {
        let open = rest.find('(')?;
        let close = rest.find(')')?;
        let name = rest.get(..open)?.trim();
        let inner = rest.get(open + 1..close)?.trim();
        let (index, (_, unit)) = FUNCTIONS
            .iter()
            .enumerate()
            .find(|(_, (function, _))| *function == name)?;
        let prop = TILT_PROPS[index];
        let number: f64 = inner.strip_suffix(unit)?.trim().parse().ok()?;
        if !number.is_finite() || found.iter().any(|(p, _)| *p == prop) {
            return None;
        }
        found.push((prop, number));
        rest = rest.get(close + 1..)?.trim_start();
    }
    Some(found)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn props(pairs: &[(&str, f64)]) -> Map<Value> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), Value::Number(*v)))
            .collect()
    }

    #[test]
    fn writes_the_functions_in_css_order() {
        let p = props(&[
            ("rotate-z", 5.0),
            ("perspective", 800.0),
            ("rotate-y", -30.5),
        ]);
        assert_eq!(
            css_of(&p).as_deref(),
            Some("perspective(800px) rotateY(-30.5deg) rotateZ(5deg)")
        );
        assert_eq!(css_of(&Map::new()), None);
    }

    #[test]
    fn reads_back_what_it_writes_and_nothing_else() {
        let p = props(&[
            ("perspective", 800.0),
            ("rotate-x", 20.0),
            ("rotate-y", -30.5),
        ]);
        let read = parse(&css_of(&p).unwrap()).unwrap();
        assert_eq!(
            read,
            [
                ("perspective", 800.0),
                ("rotate-x", 20.0),
                ("rotate-y", -30.5)
            ]
        );
        for other in [
            "",
            "rotate(5deg)",
            "rotateX(5deg) rotateX(6deg)",
            "rotateX(5px)",
            "rotateX(5deg) translateX(3px)",
            "rotateX(5deg",
            "rotateX(NaNdeg)",
        ] {
            assert_eq!(parse(other), None, "{other}");
        }
    }
}
