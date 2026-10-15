//! The `ws_*` entry points. Each checks its pointers and strings, runs inside the guard, and sends
//! events only after it no longer borrows the editor, because a shell's callback may call back in
//! (`ws_state_json` on `changed`).

use std::ffi::{CStr, CString, c_char, c_void};
use std::path::Path;
use std::ptr;

use slint_bindings_ffi::SbHost;

use crate::{WS_API_VERSION, WsEditor, WsEventFn, changed, guard, last_error};

/// # Safety
/// `text` is null or a NUL-terminated string valid for the call.
unsafe fn path_arg<'a>(text: *const c_char) -> Result<&'a Path, String> {
    if text.is_null() {
        return Err("a null path was passed".into());
    }
    // SAFETY: forwarded caller contract.
    let text = unsafe { CStr::from_ptr(text) };
    text.to_str()
        .map(Path::new)
        .map_err(|_| "the path is not valid UTF-8".into())
}

/// # Safety
/// `editor` is null or a live editor from `ws_editor_new`.
unsafe fn editor_ref<'a>(editor: *const WsEditor) -> Result<&'a WsEditor, String> {
    // SAFETY: forwarded caller contract.
    let editor = unsafe { editor.as_ref() }.ok_or("a null editor was passed")?;
    editor.check_thread()?;
    Ok(editor)
}

/// # Safety
/// `editor` is null or a live editor from `ws_editor_new`, not borrowed elsewhere.
unsafe fn editor_mut<'a>(editor: *mut WsEditor) -> Result<&'a mut WsEditor, String> {
    // SAFETY: forwarded caller contract.
    let editor = unsafe { editor.as_mut() }.ok_or("a null editor was passed")?;
    editor.check_thread()?;
    Ok(editor)
}

/// The API version, 0; a shell refuses any other.
#[unsafe(no_mangle)]
pub extern "C" fn ws_api_version() -> u32 {
    WS_API_VERSION
}

/// A new editor with no document. `on_event` (may be null) receives `user_data` and each event
/// as JSON. Free with `ws_editor_free`.
#[unsafe(no_mangle)]
pub extern "C" fn ws_editor_new(on_event: WsEventFn, user_data: *mut c_void) -> *mut WsEditor {
    guard(ptr::null_mut(), || {
        Ok(Box::into_raw(Box::new(WsEditor::new(on_event, user_data))))
    })
}

/// Frees an editor; null is ignored. Surfaces made from it stay valid.
///
/// # Safety
/// `editor` is null or a live editor from `ws_editor_new`, not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ws_editor_free(editor: *mut WsEditor) {
    guard((), || {
        if !editor.is_null() {
            // SAFETY: the caller hands over the editor `ws_editor_new` boxed.
            drop(unsafe { Box::from_raw(editor) });
        }
        Ok(())
    });
}

/// Opens the `.weft` markup at the UTF-8 `path` with the project above it. False, with a message
/// and the syntax diagnostics in `ws_last_error`, when it cannot; the open document stays.
///
/// # Safety
/// `editor` is null or live; `path` is null or a NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ws_open(editor: *mut WsEditor, path: *const c_char) -> bool {
    guard(false, || {
        // SAFETY: forwarded caller contract.
        let (editor, path) = unsafe { (editor_mut(editor)?, path_arg(path)?) };
        editor.open(path)?;
        let sink = editor.sink;
        sink.emit(&changed());
        Ok(true)
    })
}

/// Saves the canonical markup atomically to the UTF-8 `path`, or to the current path when `path`
/// is null; another path becomes the document's path.
///
/// # Safety
/// `editor` is null or live; `path` is null or a NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ws_save(editor: *mut WsEditor, path: *const c_char) -> bool {
    guard(false, || {
        // SAFETY: forwarded caller contract.
        let editor = unsafe { editor_mut(editor)? };
        let path = if path.is_null() {
            None
        } else {
            // SAFETY: forwarded caller contract.
            Some(unsafe { path_arg(path)? })
        };
        editor.save(path)?;
        let sink = editor.sink;
        sink.emit(&changed());
        Ok(true)
    })
}

/// The open document's state as JSON (`v`, `path`, `title`, `screen`, `dirty`, `diagnostics`,
/// `project`, `project_diagnostics`). Null without a document. Free with `ws_string_free`.
///
/// # Safety
/// `editor` is null or live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ws_state_json(editor: *const WsEditor) -> *mut c_char {
    guard(ptr::null_mut(), || {
        // SAFETY: forwarded caller contract.
        let json = unsafe { editor_ref(editor)? }.state_json()?;
        CString::new(json)
            .map(CString::into_raw)
            .map_err(|e| e.to_string())
    })
}

/// A new surface showing the open screen at `width`×`height` physical pixels and `scale`, drawn
/// on the CPU. The shell owns it: draw and feed it with the `sb_host_*` calls, free it with
/// `sb_host_free`. Null, with the generator's or the compiler's message, when the screen is
/// refused.
///
/// # Safety
/// `editor` is null or live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ws_surface_new(
    editor: *mut WsEditor,
    width: u32,
    height: u32,
    scale: f32,
) -> *mut SbHost {
    guard(ptr::null_mut(), || {
        // SAFETY: forwarded caller contract.
        let editor = unsafe { editor_ref(editor)? };
        Ok(editor.surface(width, height, scale)?.into_raw())
    })
}

/// Frees a string from `ws_state_json`; null is ignored.
///
/// # Safety
/// `text` is null or a string from a `ws_*` call, not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ws_string_free(text: *mut c_char) {
    guard((), || {
        if !text.is_null() {
            // SAFETY: the caller hands back a string `CString::into_raw` made.
            drop(unsafe { CString::from_raw(text) });
        }
        Ok(())
    });
}

/// The message of this thread's last failed `ws_*` call, or null. Valid until the next call.
#[unsafe(no_mangle)]
pub extern "C" fn ws_last_error() -> *const c_char {
    last_error()
}
