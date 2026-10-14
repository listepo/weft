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

/// The stack a recursive reader gets on native targets: enough for a debug build of a parser
/// at the nesting its own estimate allows.
#[cfg(not(target_arch = "wasm32"))]
const READER_STACK: usize = 64 << 20;

/// Runs `read` on a thread named `name` with a large stack, so a recursive parser meets its
/// nesting bound before it meets the end of the stack. `None` when the reader panicked. Where no
/// thread can be had, and on `wasm32`, which has none to give, it runs on the caller's stack: the
/// reader's own nesting bound has to hold there too.
pub fn on_reader_stack<R: Send>(name: &str, read: impl Fn() -> R + Sync) -> Option<R> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let ran = std::thread::scope(|s| {
            std::thread::Builder::new()
                .name(name.into())
                .stack_size(READER_STACK)
                .spawn_scoped(s, &read)
                .map(|h| h.join().ok())
        });
        if let Ok(result) = ran {
            return result;
        }
    }
    let _ = name;
    Some(read())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn depth(n: u32) -> u32 {
        // Deep enough to overflow a 2 MiB test thread in a debug build.
        let pad = std::hint::black_box([0u8; 4096]);
        if n == 0 {
            u32::from(pad[0])
        } else {
            depth(n - 1) + 1
        }
    }

    #[test]
    fn a_reader_gets_its_stack_and_a_panic_is_none() {
        assert_eq!(on_reader_stack("deep", || depth(4000)), Some(4000));
        assert_eq!(
            on_reader_stack("broken", || -> u32 { panic!("reader") }),
            None
        );
    }
}
