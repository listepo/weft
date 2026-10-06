//! Data paths: the bindings of Weft (`$.a.b`, `$todo.title`) and the JSON Pointers other formats
//! read their data model with. Both formats Weft meets here scope a repetition the same way: an
//! absolute pointer starts with `/`, and one without it is a field of the innermost item.

use weft_core::is_binding;

/// The pointer for a binding; `scope` holds the loop variables around it, outermost first.
/// The reason is returned when the format has no way to write the path.
pub fn to_pointer(bind: &str, scope: &[String]) -> Result<String, String> {
    let rest = bind.strip_prefix('$').unwrap_or_default();
    if let Some(path) = rest.strip_prefix('.') {
        return Ok(format!("/{}", path.replace('.', "/")));
    }
    let (var, field) = rest.split_once('.').unwrap_or((rest, ""));
    match scope.iter().rposition(|v| v == var) {
        _ if field.is_empty() => Err(format!(
            "{bind} is a whole repetition item, which has no path"
        )),
        Some(i) if i + 1 == scope.len() => Ok(field.replace('.', "/")),
        Some(_) => Err(format!(
            "{bind} reads an outer repetition; only the innermost item is in scope"
        )),
        None => Err(format!("{bind} is outside any repetition that defines it")),
    }
}

/// The binding for a pointer, or `None` when its segments are not Weft path names.
pub fn from_pointer(pointer: &str, scope: &[String]) -> Option<String> {
    let (absolute, rest) = pointer
        .strip_prefix('/')
        .map_or((false, pointer), |r| (true, r));
    let path = rest
        .split('/')
        .map(|s| s.replace("~1", "/").replace("~0", "~"))
        .collect::<Vec<_>>()
        .join(".");
    let bind = match (absolute, scope.last()) {
        (true, _) => format!("$.{path}"),
        (false, Some(var)) if rest.is_empty() => format!("${var}"),
        (false, Some(var)) => format!("${var}.{path}"),
        (false, None) => return None,
    };
    is_binding(&bind).then_some(bind)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pointers_round_trip() {
        let scope = ["todo".to_owned()];
        assert_eq!(to_pointer("$.a.b", &[]).as_deref(), Ok("/a/b"));
        assert_eq!(to_pointer("$todo.title", &scope).as_deref(), Ok("title"));
        assert!(to_pointer("$todo", &scope).is_err());
        assert!(to_pointer("$todo.title", &[]).is_err());
        assert_eq!(from_pointer("/a/b", &[]).as_deref(), Some("$.a.b"));
        assert_eq!(
            from_pointer("title", &scope).as_deref(),
            Some("$todo.title")
        );
        assert_eq!(from_pointer("title", &[]), None);
        assert_eq!(from_pointer("/a b", &[]), None);
        assert_eq!(from_pointer("/", &[]), None);
    }
}
