use weft_core::{Code, Diagnostic};

/// Below the 256 levels a document may nest (SPEC §2), leaving room for wrappers an importer adds.
pub const MAX_DEPTH: usize = 200;
pub const MAX_NODES: usize = 20_000;

/// W602: the input was cut at a limit; `what` completes "The input …".
pub fn limit_reached(diagnostics: &mut Vec<Diagnostic>, path: &str, what: &str) {
    diagnostics.push(Diagnostic::new(
        Code::W602,
        path,
        format!("The input {what}; the rest was not imported."),
        format!("at most {MAX_NODES} elements, nested at most {MAX_DEPTH} levels deep"),
    ));
}
