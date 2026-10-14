//! Text and numbers as the TypeScript packages read them. Whitespace is ECMAScript's (`\s` and
//! `trim`), not Rust's: the two disagree on U+0085 and U+FEFF, and names an importer reads or a
//! renderer exposes must not depend on the host. `Number(string)` is ECMAScript's for the same
//! reason.

/// ECMAScript WhiteSpace and LineTerminator: what `\s` matches and `trim` removes.
pub fn is_js_space(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\u{B}' | '\u{C}' | '\r' | ' ' | '\u{A0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200A}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202F}'
                | '\u{205F}'
                | '\u{3000}'
                | '\u{FEFF}'
    )
}

pub fn js_trim(s: &str) -> &str {
    s.trim_matches(is_js_space)
}

/// Runs of whitespace become one space, and the ends are trimmed.
pub fn squash(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut space = false;
    for c in js_trim(s).chars() {
        if is_js_space(c) {
            space = true;
            continue;
        }
        if space {
            out.push(' ');
            space = false;
        }
        out.push(c);
    }
    out
}

/// ECMAScript `Number(string)`: whitespace-trimmed decimal, `0x`/`0o`/`0b` integers or
/// `Infinity`; anything else is NaN, and the empty string is 0.
pub fn js_number_from(s: &str) -> f64 {
    let t = js_trim(s);
    if t.is_empty() {
        return 0.0;
    }
    for (prefix, radix) in [
        ("0x", 16),
        ("0X", 16),
        ("0o", 8),
        ("0O", 8),
        ("0b", 2),
        ("0B", 2),
    ] {
        if let Some(digits) = t.strip_prefix(prefix) {
            if digits.is_empty() || !digits.chars().all(|c| c.is_digit(radix)) {
                return f64::NAN;
            }
            return digits
                .chars()
                .filter_map(|c| c.to_digit(radix))
                .fold(0.0, |n, d| n * f64::from(radix) + f64::from(d));
        }
    }
    let (sign, body) = match t.as_bytes()[0] {
        b'-' => (-1.0, &t[1..]),
        b'+' => (1.0, &t[1..]),
        _ => (1.0, t),
    };
    if body == "Infinity" {
        return sign * f64::INFINITY;
    }
    if !is_decimal_literal(body) {
        return f64::NAN;
    }
    body.parse::<f64>().map_or(f64::NAN, |n| sign * n)
}

/// `digits [. digits?] | . digits`, then an optional exponent: what `Number` accepts after a sign.
fn is_decimal_literal(s: &str) -> bool {
    let (mantissa, exponent) = match s.find(['e', 'E']) {
        Some(i) => (&s[..i], Some(&s[i + 1..])),
        None => (s, None),
    };
    let digits = |d: &str| d.chars().all(|c| c.is_ascii_digit());
    let mantissa_ok = match mantissa.split_once('.') {
        Some((int, frac)) => digits(int) && digits(frac) && !(int.is_empty() && frac.is_empty()),
        None => !mantissa.is_empty() && digits(mantissa),
    };
    let exponent_ok = exponent.is_none_or(|e| {
        let e = e.strip_prefix(['+', '-']).unwrap_or(e);
        !e.is_empty() && digits(e)
    });
    mantissa_ok && exponent_ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn squash_uses_ecmascript_whitespace() {
        assert_eq!(squash("  a \t\n b\u{A0}c "), "a b c");
        assert_eq!(squash("\u{FEFF}x\u{FEFF}"), "x");
        // U+0085 is whitespace to Rust but not to ECMAScript.
        assert_eq!(squash("a\u{85}b"), "a\u{85}b");
    }

    #[test]
    fn numbers_read_like_ecmascript() {
        assert_eq!(js_number_from(" 2 "), 2.0);
        assert_eq!(js_number_from("0x10"), 16.0);
        assert_eq!(js_number_from("1e1"), 10.0);
        assert_eq!(js_number_from(".5"), 0.5);
        assert_eq!(js_number_from("-Infinity"), f64::NEG_INFINITY);
        assert_eq!(js_number_from(""), 0.0);
        for bad in [
            "inf", "NaN", "1,5", "0x", "-0x1", "e5", ".", "1e", "infinity",
        ] {
            assert!(js_number_from(bad).is_nan(), "{bad}");
        }
    }
}
