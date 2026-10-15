//! The C ABI of WeftStudio (T69.1): the `ws_*` editor calls over `weft-studio`, linked together
//! with slint-bindings' `sb_*` surface calls, so a shell loads one Rust library and holds two
//! opaque handles, `WsEditor*` for the document and `SbHost*` for the surface. Two Rust libraries
//! in one app would each carry a copy of Slint, and the screen would not find the platform the
//! host installed.
//!
//! Everything a shell passes in is untrusted: a null pointer, a path that is not UTF-8 or a call
//! from another thread is a failed call with a message for `ws_last_error`, and no panic unwinds
//! into Swift or C#. Only [`abi`] holds unsafe code.

use std::cell::RefCell;
use std::ffi::{CString, c_char, c_void};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;
use std::thread::ThreadId;

use serde::Serialize;
use serde_json::{Value as Json, json};
use slint_bindings_ffi::SbHost;
use slint_interpreter::Value;
use weft_slint::GenerateError;
use weft_studio::{Editor, EditorError};

#[allow(unsafe_code)]
pub mod abi;

/// The version of the `ws_*` API and of the JSON it sends; a shell refuses any other.
pub const WS_API_VERSION: u32 = 0;

/// Decision 8 of T69.1: one style on both platforms, the one the screenshot baselines use.
const STYLE: &str = "fluent";

thread_local! {
    static LAST_ERROR: RefCell<Option<CString>> = const { RefCell::new(None) };
}

fn set_error(message: String) {
    // A message with a NUL in it keeps the part before it rather than vanishing.
    let message = match CString::new(message) {
        Ok(text) => text,
        Err(e) => {
            let at = e.nul_position();
            let mut bytes = e.into_vec();
            bytes.truncate(at);
            CString::new(bytes).unwrap_or_default()
        }
    };
    LAST_ERROR.with_borrow_mut(|slot| *slot = Some(message));
}

fn last_error() -> *const c_char {
    LAST_ERROR.with_borrow(|slot| slot.as_ref().map_or(std::ptr::null(), |text| text.as_ptr()))
}

/// Clears the last error, runs `body`, and turns an error or a panic into `fail` and a message.
/// `slint-bindings-ffi` keeps its own guard private, so this is the editor's copy.
fn guard<T>(fail: T, body: impl FnOnce() -> Result<T, String>) -> T {
    LAST_ERROR.with_borrow_mut(|slot| *slot = None);
    match catch_unwind(AssertUnwindSafe(body)) {
        Ok(Ok(value)) => value,
        Ok(Err(message)) => {
            set_error(message);
            fail
        }
        Err(_) => {
            set_error("panic inside weft-studio-ffi".to_owned());
            fail
        }
    }
}

/// The shell's event callback: `user_data`, then one event as UTF-8 JSON, valid during the call.
pub type WsEventFn = Option<extern "C" fn(user_data: *mut c_void, event_json: *const c_char)>;

/// Where events go. Copied into each surface's `perform` handler, so a surface never points into
/// the editor; the shell keeps `user_data` alive while either lives.
#[derive(Clone, Copy)]
struct Sink {
    on_event: WsEventFn,
    user_data: *mut c_void,
}

impl Sink {
    fn emit(self, event: &Json) {
        let Some(on_event) = self.on_event else {
            return;
        };
        if let Ok(text) = CString::new(event.to_string()) {
            on_event(self.user_data, text.as_ptr());
        }
    }
}

fn changed() -> Json {
    json!({ "v": WS_API_VERSION, "type": "changed" })
}

/// One editor window's document. Every call must come from the thread that created it, the UI
/// thread, as slint-bindings requires of its hosts.
pub struct WsEditor {
    editor: Option<Editor>,
    sink: Sink,
    thread: ThreadId,
}

#[derive(Serialize)]
struct Versioned<T> {
    v: u32,
    #[serde(flatten)]
    inner: T,
}

impl WsEditor {
    fn new(on_event: WsEventFn, user_data: *mut c_void) -> WsEditor {
        WsEditor {
            editor: None,
            sink: Sink {
                on_event,
                user_data,
            },
            thread: std::thread::current().id(),
        }
    }

    fn check_thread(&self) -> Result<(), String> {
        if std::thread::current().id() == self.thread {
            Ok(())
        } else {
            Err("the editor was called from a thread other than the one that created it".into())
        }
    }

    fn document(&self) -> Result<&Editor, String> {
        self.editor
            .as_ref()
            .ok_or_else(|| "no document is open".to_owned())
    }

    /// A failed open keeps the document that was open before.
    fn open(&mut self, path: &Path) -> Result<(), String> {
        self.editor = Some(Editor::open(path).map_err(describe)?);
        Ok(())
    }

    fn save(&mut self, path: Option<&Path>) -> Result<(), String> {
        let editor = self
            .editor
            .as_mut()
            .ok_or_else(|| "no document is open".to_owned())?;
        editor.save(path).map_err(describe)
    }

    fn state_json(&self) -> Result<String, String> {
        serde_json::to_string(&Versioned {
            v: WS_API_VERSION,
            inner: self.document()?.state(),
        })
        .map_err(|e| e.to_string())
    }

    /// The screen compiled and hosted at `width`×`height` physical pixels, its data filled from
    /// `render.data`, and its `perform(action, id)` raised as an `action` event.
    fn surface(&self, width: u32, height: u32, scale: f32) -> Result<SbHost, String> {
        let input = self.document()?.render_input().map_err(describe)?;
        let definition = weft_slint::runtime::compile(&input.source, STYLE)
            .map_err(|errors| errors.join("\n"))?;
        let host = SbHost::new(&definition, width, height, scale).map_err(|e| e.to_string())?;
        if let Some(data) = &input.data {
            // The sample data is a preview aid: a value of the wrong type leaves that property at
            // its default and the screen still shows, so the problems are not an error.
            let _problems = weft_slint::runtime::apply_data(host.component(), &input.source, data);
        }
        let sink = self.sink;
        host.component()
            .set_callback("perform", move |args| {
                if let [Value::String(action), Value::String(id)] = args {
                    sink.emit(&json!({
                        "v": WS_API_VERSION,
                        "type": "action",
                        "action": action.as_str(),
                        "id": id.as_str(),
                    }));
                }
                Value::Void
            })
            .map_err(|e| format!("the generated screen has no perform callback: {e}"))?;
        Ok(host)
    }
}

/// The error and its causes on one line, then the diagnostics behind a syntax error or a refused
/// screen one per line, so the shell's alert says what to fix.
fn describe(error: EditorError) -> String {
    let mut text = error.to_string();
    let mut source = std::error::Error::source(&error);
    while let Some(cause) = source {
        text.push_str(&format!(": {cause}"));
        source = cause.source();
    }
    let diagnostics = match &error {
        EditorError::Syntax { diagnostics, .. }
        | EditorError::Generate(GenerateError::Invalid(diagnostics)) => diagnostics.as_slice(),
        _ => &[],
    };
    for d in diagnostics {
        match (d.line, d.column) {
            (Some(line), Some(column)) => {
                text.push_str(&format!("\n{line}:{column}: {}", d.message))
            }
            _ => text.push_str(&format!("\n{}: {}", d.path, d.message)),
        }
    }
    text
}
