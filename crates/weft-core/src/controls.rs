//! How the number, date and colour controls read their props (SPEC §5.1 notes): what a slider
//! shows, how a stepper steps, which text is a date. `@weft/render-react` (`expand.ts`) and the
//! generated components restate this in JavaScript; the targets written in Rust call it, so a
//! literal document reads the same everywhere. The functions never fail: anything that does not
//! fit is the control's fallback.

use crate::json::js_number;

/// A prop as a finite number: a number, or text written as one. Text such as `0x10` or
/// `Infinity` is data that does not fit the prop, so it reads as absent.
pub fn numeric(number: Option<f64>, text: Option<&str>) -> Option<f64> {
    match (number, text) {
        (Some(n), _) => n.is_finite().then_some(n),
        (None, Some(t)) => float(t.trim()).then(|| t.trim().parse().ok()).flatten(),
        (None, None) => None,
    }
}

/// `^-?(\d+(\.\d+)?|\.\d+)([eE][-+]?\d+)?$`, the text a number input accepts.
fn float(s: &str) -> bool {
    let s = s.strip_prefix('-').unwrap_or(s);
    let (mantissa, exponent) = match s.find(['e', 'E']) {
        Some(at) => (&s[..at], Some(&s[at + 1..])),
        None => (s, None),
    };
    let digits = |t: &str| !t.is_empty() && t.bytes().all(|b| b.is_ascii_digit());
    let mantissa_ok = match mantissa.split_once('.') {
        Some((whole, fraction)) => (whole.is_empty() || digits(whole)) && digits(fraction),
        None => digits(mantissa),
    };
    let exponent_ok = exponent.is_none_or(|e| digits(e.strip_prefix(['-', '+']).unwrap_or(e)));
    mantissa_ok && exponent_ok
}

/// The digits after the decimal point of a number as JavaScript prints it.
fn decimals(x: f64) -> i32 {
    let text = js_number(x);
    let (mantissa, exponent) = match text.split_once('e') {
        Some((m, e)) => (m, e.parse::<i32>().unwrap_or(0)),
        None => (text.as_str(), 0),
    };
    let own = mantissa.split_once('.').map_or(0, |(_, f)| f.len() as i32);
    (own - exponent).max(0)
}

/// Sums and products of decimal steps carry binary error (0.1 × 3); rounding to the decimals the
/// inputs have gives back the number a person means.
pub fn tidy(x: f64, inputs: &[f64]) -> f64 {
    let digits = inputs
        .iter()
        .map(|&i| decimals(i))
        .max()
        .unwrap_or(0)
        .min(100);
    format!("{x:.*}", digits as usize).parse().unwrap_or(x)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Span {
    pub min: f64,
    pub max: f64,
    pub step: f64,
}

/// A slider: both bounds always exist, `max` below `min` is `min`, and a step that is not above 0
/// is 1.
pub fn slider_span(min: Option<f64>, max: Option<f64>, step: Option<f64>) -> Span {
    let min = min.unwrap_or(0.0);
    let max = max.unwrap_or(100.0).max(min);
    Span {
        min,
        max,
        step: step.filter(|s| *s > 0.0).unwrap_or(1.0),
    }
}

/// What a range input shows: the value inside the bounds on the grid of steps from `min`, a tie
/// going up and a point past the last step falling back to it.
pub fn slider_value(value: Option<f64>, span: Span) -> f64 {
    let Span { min, max, step } = span;
    let v = value.unwrap_or(min).max(min).min(max);
    let mut on = min + ((v - min) / step + 0.5).floor() * step;
    if on > max {
        on -= step;
    }
    tidy(on.max(min), &[min, step])
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub step: f64,
}

/// A stepper: a side without a bound is open.
pub fn stepper_bounds(min: Option<f64>, max: Option<f64>, step: Option<f64>) -> Bounds {
    Bounds {
        min,
        max: match (min, max) {
            (Some(lo), Some(hi)) => Some(hi.max(lo)),
            (_, hi) => hi,
        },
        step: step.filter(|s| *s > 0.0).unwrap_or(1.0),
    }
}

pub fn clamp_to(v: f64, bounds: Bounds) -> f64 {
    let v = bounds.min.map_or(v, |lo| v.max(lo));
    bounds.max.map_or(v, |hi| v.min(hi))
}

pub fn stepper_value(value: Option<f64>, bounds: Bounds) -> f64 {
    clamp_to(value.unwrap_or(0.0), bounds)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DateType {
    Date,
    Time,
    DateTime,
}

impl DateType {
    /// The `type` prop of a `date-picker`; anything but `time` and `datetime` is a date.
    pub fn of(text: &str) -> Self {
        match text {
            "time" => DateType::Time,
            "datetime" => DateType::DateTime,
            _ => DateType::Date,
        }
    }
}

fn valid_date(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return false;
    }
    let digits = |r: std::ops::Range<usize>| b[r].iter().all(u8::is_ascii_digit);
    if !(digits(0..4) && digits(5..7) && digits(8..10)) {
        return false;
    }
    let number = |r: std::ops::Range<usize>| s[r].parse::<u32>().unwrap_or(0);
    let (year, month, day) = (number(0..4), number(5..7), number(8..10));
    if year < 1 || !(1..=12).contains(&month) || day < 1 {
        return false;
    }
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let length = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    day <= length[month as usize - 1]
}

fn valid_time(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 5
        && b[2] == b':'
        && b[0].is_ascii_digit()
        && b[1].is_ascii_digit()
        && b[3].is_ascii_digit()
        && b[4].is_ascii_digit()
        && (b[0] < b'2' || (b[0] == b'2' && b[1] <= b'3'))
        && b[3] <= b'5'
}

/// The text of a date, time or both when it is written in the format of `kind`, else empty.
pub fn date_text(kind: DateType, value: &str) -> String {
    let ok = match kind {
        DateType::Date => valid_date(value),
        DateType::Time => valid_time(value),
        DateType::DateTime => value
            .split_once('T')
            .is_some_and(|(d, t)| valid_date(d) && valid_time(t)),
    };
    if ok { value.to_owned() } else { String::new() }
}

/// The colour a colour input shows: the value as lowercase `#rrggbb`, black when it is not one.
pub fn color_value(value: &str) -> String {
    let hex = value
        .strip_prefix('#')
        .filter(|h| h.len() == 6 && h.bytes().all(|b| b.is_ascii_hexdigit()));
    match hex {
        Some(h) => format!("#{}", h.to_ascii_lowercase()),
        None => "#000000".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_come_from_numbers_and_number_text() {
        assert_eq!(numeric(Some(3.0), None), Some(3.0));
        assert_eq!(numeric(Some(f64::INFINITY), None), None);
        assert_eq!(numeric(None, Some(" 2.5 ")), Some(2.5));
        assert_eq!(numeric(None, Some(".5e1")), Some(5.0));
        for bad in ["", "0x10", "Infinity", "1.", "1e", "--1", "1 2"] {
            assert_eq!(numeric(None, Some(bad)), None, "{bad}");
        }
    }

    #[test]
    fn a_slider_snaps_to_its_grid() {
        let span = slider_span(Some(0.0), Some(10.0), Some(2.0));
        assert_eq!(slider_value(Some(3.0), span), 4.0);
        assert_eq!(slider_value(Some(99.0), span), 10.0);
        assert_eq!(slider_value(None, span), 0.0);
        let odd = slider_span(Some(1.0), Some(9.0), Some(4.0));
        assert_eq!(
            slider_value(Some(9.0), odd),
            9.0 - 0.0 - (9.0 - 1.0 - 8.0) - 0.0
        );
        let backwards = slider_span(Some(8.0), Some(2.0), None);
        assert_eq!((backwards.min, backwards.max), (8.0, 8.0));
        assert_eq!(slider_span(None, None, Some(-1.0)).step, 1.0);
        let tenths = slider_span(Some(0.0), Some(1.0), Some(0.1));
        assert_eq!(slider_value(Some(0.3), tenths), 0.3);
    }

    #[test]
    fn a_stepper_keeps_to_its_bounds() {
        let b = stepper_bounds(Some(1.0), Some(4.0), None);
        assert_eq!(stepper_value(Some(9.0), b), 4.0);
        assert_eq!(stepper_value(None, b), 1.0);
        let open = stepper_bounds(None, None, Some(0.0));
        assert_eq!((open.min, open.max, open.step), (None, None, 1.0));
        assert_eq!(stepper_bounds(Some(5.0), Some(2.0), None).max, Some(5.0));
    }

    #[test]
    fn dates_are_checked_against_the_calendar() {
        assert_eq!(date_text(DateType::Date, "2026-10-05"), "2026-10-05");
        assert_eq!(date_text(DateType::Date, "2026-02-30"), "");
        assert_eq!(date_text(DateType::Date, "2024-02-29"), "2024-02-29");
        assert_eq!(date_text(DateType::Date, "0000-01-01"), "");
        assert_eq!(date_text(DateType::Time, "23:59"), "23:59");
        assert_eq!(date_text(DateType::Time, "24:00"), "");
        assert_eq!(
            date_text(DateType::DateTime, "2026-10-05T09:30"),
            "2026-10-05T09:30"
        );
        assert_eq!(date_text(DateType::DateTime, "2026-10-05"), "");
        assert_eq!(date_text(DateType::DateTime, "2026-10-05T09:30T1"), "");
    }

    #[test]
    fn colours_are_lowercase_hex_or_black() {
        assert_eq!(color_value("#3B82F6"), "#3b82f6");
        assert_eq!(color_value("blue"), "#000000");
        assert_eq!(color_value("#12345"), "#000000");
    }
}
