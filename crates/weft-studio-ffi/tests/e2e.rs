//! The editor through the C ABI a shell calls, headless: `ws_*` for the document, slint-bindings'
//! `sb_*` for the surface, both from this one library.
//!
//! Slint takes the first thread that draws as its UI thread for the whole process, so only one
//! test here makes a surface; `tests/latency.rs` (its own binary) draws the rest of the corpus.

// A test crate: a failed unwrap or panic is a failed test, which is the point.
#![allow(clippy::unwrap_used, clippy::panic)]
// It drives the C ABI, as a shell does.
#![allow(unsafe_code)]

use std::ffi::{CStr, CString, c_char, c_void};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::ptr;

use serde_json::{Value as Json, json};
use slint_bindings_ffi::{
    sb_host_frame_len, sb_host_free, sb_host_pointer_moved, sb_host_pointer_pressed,
    sb_host_pointer_released, sb_host_render,
};
use weft_studio_ffi::WsEditor;
use weft_studio_ffi::abi::{
    ws_api_version, ws_editor_free, ws_editor_new, ws_last_error, ws_open, ws_save, ws_state_json,
    ws_string_free, ws_surface_new,
};

const CORPUS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus");

/// A directory of its own per test, removed afterwards.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Scratch {
        let dir =
            std::env::temp_dir().join(format!("weft-studio-ffi-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Scratch(dir)
    }

    fn write(&self, name: &str, content: &[u8]) -> PathBuf {
        let path = self.0.join(name);
        std::fs::write(&path, content).unwrap();
        path
    }

    /// The login screen with its sample data as the project's `render.data`, so the submit button
    /// is enabled (`disabled="{!$.email}"`).
    fn login(&self) -> PathBuf {
        let corpus = Path::new(CORPUS).join("login");
        self.write(
            "data.json",
            &std::fs::read(corpus.join("data.json")).unwrap(),
        );
        self.write("weft.json", br#"{ "render": { "data": "data.json" } }"#);
        self.write(
            "screen.weft",
            &std::fs::read(corpus.join("screen.weft")).unwrap(),
        )
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn c(path: &Path) -> CString {
    CString::new(path.to_str().unwrap()).unwrap()
}

fn last_error() -> String {
    let text = ws_last_error();
    assert!(!text.is_null(), "a failed call left no message");
    // SAFETY: a non-null `ws_last_error` is NUL-terminated until the next call.
    unsafe { CStr::from_ptr(text) }.to_str().unwrap().to_owned()
}

extern "C" fn record(user_data: *mut c_void, event: *const c_char) {
    // SAFETY: `user_data` is the test's `Vec<Json>`, live while its editor and surface are.
    let events = unsafe { &mut *user_data.cast::<Vec<Json>>() };
    // SAFETY: the event is NUL-terminated for the length of the call.
    let text = unsafe { CStr::from_ptr(event) }.to_str().unwrap();
    events.push(serde_json::from_str(text).unwrap());
}

/// An editor whose events land in `events`.
fn editor(events: &mut Vec<Json>) -> *mut WsEditor {
    let editor = ws_editor_new(Some(record), ptr::from_mut(events).cast());
    assert!(!editor.is_null());
    editor
}

fn state(editor: *mut WsEditor) -> Json {
    // SAFETY: a live editor.
    let text = unsafe { ws_state_json(editor) };
    assert!(!text.is_null(), "{}", last_error());
    // SAFETY: a string from `ws_state_json`, freed once below.
    let json = serde_json::from_str(unsafe { CStr::from_ptr(text) }.to_str().unwrap()).unwrap();
    // SAFETY: as above.
    unsafe { ws_string_free(text) };
    json
}

/// The middle of the rows the primary (accent-filled) button covers: rows with a long run of
/// blue. Link text is blue too, but never that wide.
fn primary_button(pixels: &[u8], width: usize) -> (f32, f32) {
    let blue = |p: &[u8]| p[2] > 120 && p[2] as i32 > p[0] as i32 + 80 && p[3] > 200;
    let rows: Vec<(usize, Vec<usize>)> = pixels
        .chunks(width * 4)
        .enumerate()
        .map(|(y, row)| {
            let xs = row
                .chunks(4)
                .enumerate()
                .filter(|(_, p)| blue(p))
                .map(|(x, _)| x)
                .collect();
            (y, xs)
        })
        .filter(|(_, xs): &(usize, Vec<usize>)| xs.len() > 100)
        .collect();
    assert!(!rows.is_empty(), "no primary button in the frame");
    let (y, xs) = &rows[rows.len() / 2];
    (xs[xs.len() / 2] as f32, *y as f32)
}

#[test]
fn the_api_version_is_zero() {
    assert_eq!(ws_api_version(), 0);
}

#[test]
fn login_opens_renders_and_its_submit_button_raises_one_action() {
    let scratch = Scratch::new("login");
    let screen = scratch.login();
    let mut events = Vec::new();
    let editor = editor(&mut events);
    // SAFETY: a live editor and a NUL-terminated path.
    assert!(
        unsafe { ws_open(editor, c(&screen).as_ptr()) },
        "{}",
        last_error()
    );
    assert_eq!(events, [json!({ "v": 0, "type": "changed" })]);
    let state = state(editor);
    assert_eq!(state["v"], 0);
    assert_eq!(state["title"], "screen.weft");
    assert_eq!(state["screen"], "login");
    assert_eq!(state["dirty"], false);

    // SAFETY: a live editor.
    let host = unsafe { ws_surface_new(editor, 480, 800, 1.0) };
    assert!(!host.is_null(), "{}", last_error());
    // SAFETY: a live host and a buffer of the length it asks for.
    let len = unsafe { sb_host_frame_len(host) };
    assert_eq!(len, 480 * 800 * 4);
    let mut pixels = vec![0u8; len];
    // SAFETY: as above.
    let frame = unsafe { sb_host_render(host, pixels.as_mut_ptr(), len) };
    assert_eq!(frame.failed, 0);
    assert!(
        pixels.chunks(4).any(|p| p[3] != 0),
        "the frame is transparent"
    );

    let (x, y) = primary_button(&pixels, 480);
    events.clear();
    // SAFETY: a live host; scale 1, so pixels are logical points.
    unsafe {
        assert!(sb_host_pointer_moved(host, x, y));
        assert!(sb_host_pointer_pressed(host, x, y, 0));
        assert!(sb_host_pointer_released(host, x, y, 0));
    }
    assert_eq!(
        events,
        [json!({ "v": 0, "type": "action", "action": "auth.submit", "id": "form" })]
    );

    // The surface does not point into the editor, so either may go first.
    // SAFETY: each handle is live and freed once.
    unsafe {
        ws_editor_free(editor);
        sb_host_free(host);
    }
}

#[test]
fn a_save_round_trips_through_the_abi() {
    let scratch = Scratch::new("save");
    let original = std::fs::read(Path::new(CORPUS).join("login/screen.weft")).unwrap();
    let screen = scratch.write("screen.weft", &original);
    let copy = scratch.0.join("copy.weft");
    let mut events = Vec::new();
    let editor = editor(&mut events);
    // SAFETY: a live editor and NUL-terminated paths; null saves to the current path.
    unsafe {
        assert!(ws_open(editor, c(&screen).as_ptr()), "{}", last_error());
        assert!(ws_save(editor, c(&copy).as_ptr()), "{}", last_error());
        assert_eq!(std::fs::read(&copy).unwrap(), original);
        assert_eq!(state(editor)["path"], copy.to_str().unwrap());
        assert_eq!(state(editor)["title"], "copy.weft");
        std::fs::remove_file(&copy).unwrap();
        assert!(ws_save(editor, ptr::null()), "{}", last_error());
        assert_eq!(std::fs::read(&copy).unwrap(), original);
        assert!(ws_open(editor, c(&copy).as_ptr()), "{}", last_error());
        ws_editor_free(editor);
    }
    assert_eq!(events.len(), 4);
    assert!(
        events
            .iter()
            .all(|e| *e == json!({ "v": 0, "type": "changed" }))
    );
}

#[test]
fn bad_input_fails_with_a_message() {
    let scratch = Scratch::new("errors");
    let screen = scratch.login();
    let missing = scratch.0.join("missing.weft");
    let refused = scratch.write(
        "refused.weft",
        br#"<screen id="s" weft="0.1"><heading id="t" level="1" shade="dark">Hi</heading></screen>"#,
    );
    let syntax = scratch.write("syntax.weft", b"<screen id=\"s\" weft=\"0.1\"><heading");
    let not_utf8 = c"/tmp/\xff\xfe.weft";
    let mut events = Vec::new();
    let editor = editor(&mut events);
    // SAFETY: each pointer is null, a live editor or a NUL-terminated string.
    unsafe {
        assert!(!ws_open(ptr::null_mut(), c(&screen).as_ptr()));
        assert!(last_error().contains("null editor"), "{}", last_error());
        assert!(!ws_open(editor, ptr::null()));
        assert!(last_error().contains("null path"), "{}", last_error());
        assert!(!ws_open(editor, not_utf8.as_ptr()));
        assert!(last_error().contains("UTF-8"), "{}", last_error());
        assert!(!ws_open(editor, c(&missing).as_ptr()));
        assert!(last_error().contains("cannot read"), "{}", last_error());
        assert!(!ws_open(editor, c(&syntax).as_ptr()));
        assert!(last_error().contains("syntax errors\n"), "{}", last_error());

        // Nothing is open yet.
        assert!(ws_state_json(editor).is_null());
        assert!(last_error().contains("no document"), "{}", last_error());
        assert!(ws_surface_new(editor, 480, 800, 1.0).is_null());
        assert!(last_error().contains("no document"), "{}", last_error());
        assert!(!ws_save(editor, ptr::null()));
        assert!(last_error().contains("no document"), "{}", last_error());
        assert!(ws_state_json(ptr::null()).is_null());
        assert!(ws_surface_new(ptr::null_mut(), 480, 800, 1.0).is_null());
        assert!(!ws_save(ptr::null_mut(), ptr::null()));
        assert!(last_error().contains("null editor"), "{}", last_error());

        // A strictly invalid screen opens and its surface is refused with the diagnostics.
        assert!(ws_open(editor, c(&refused).as_ptr()), "{}", last_error());
        assert!(ws_surface_new(editor, 480, 800, 1.0).is_null());
        let message = last_error();
        assert!(message.starts_with("the document has "), "{message}");
        assert!(message.contains("shade"), "{message}");

        // A failed open keeps the open document.
        assert!(!ws_open(editor, c(&missing).as_ptr()));
        assert_eq!(state(editor)["title"], "refused.weft");
        assert!(
            ws_last_error().is_null(),
            "a successful call clears the message"
        );

        // A save into a directory that does not exist fails and changes nothing.
        let nowhere = scratch.0.join("no/such/dir/screen.weft");
        assert!(!ws_save(editor, c(&nowhere).as_ptr()));
        assert!(!last_error().is_empty());
        assert_eq!(state(editor)["title"], "refused.weft");

        ws_editor_free(editor);
        ws_editor_free(ptr::null_mut());
        ws_string_free(ptr::null_mut());
    }
    assert_eq!(events.len(), 1, "only the successful open is an event");
}

#[test]
fn a_call_from_another_thread_fails() {
    let scratch = Scratch::new("thread");
    let screen = c(&scratch.login());
    let editor = ws_editor_new(None, ptr::null_mut()) as usize;
    let message = std::thread::spawn(move || {
        // SAFETY: a live editor, used from the wrong thread on purpose.
        assert!(!unsafe { ws_open(editor as *mut WsEditor, screen.as_ptr()) });
        last_error()
    })
    .join()
    .unwrap();
    assert!(message.contains("thread"), "{message}");
    // SAFETY: freed once, on its own thread.
    unsafe { ws_editor_free(editor as *mut WsEditor) };
}

#[test]
fn weft_and_slint_bindings_share_one_slint_core() {
    let out = Command::new(env!("CARGO"))
        .args(["metadata", "--format-version", "1"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let metadata: Json = serde_json::from_slice(&out.stdout).unwrap();
    let cores: Vec<&Json> = metadata["packages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|p| p["name"] == "i-slint-core")
        .map(|p| &p["version"])
        .collect();
    assert_eq!(cores.len(), 1, "{cores:?}");
}

/// The static library a macOS shell links holds both halves of the ABI: `#[no_mangle]` functions
/// of an rlib can be left out of a library that never references the crate.
#[cfg(target_os = "macos")]
#[test]
fn the_macos_static_library_exports_ws_and_sb_calls() {
    // `CARGO_TARGET_TMPDIR` is `<target>/tmp`, so the library lands beside this test's build.
    let target = Path::new(env!("CARGO_TARGET_TMPDIR")).parent().unwrap();
    let build = Command::new(env!("CARGO"))
        .arg("rustc")
        .arg("--target-dir")
        .arg(target)
        .args([
            "-p",
            "weft-studio-ffi",
            "--lib",
            "--crate-type",
            "staticlib",
            "--message-format",
            "short",
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let library = target.join("debug/libweft_studio_ffi.a");
    // Rust's objects carry LLVM bitcode newer than Xcode's reader; the symbol tables are enough.
    let nm = Command::new("nm")
        .args(["--no-llvm-bc", "-g", "-j", "-U"])
        .arg(&library)
        .output()
        .unwrap();
    // A debug static library is about 280 MB, and nothing else reads this one.
    let _ = std::fs::remove_file(&library);
    assert!(
        nm.status.success(),
        "{}",
        String::from_utf8_lossy(&nm.stderr)
    );
    let symbols = String::from_utf8_lossy(&nm.stdout);
    for name in [
        "_ws_open",
        "_ws_surface_new",
        "_sb_host_render",
        "_sb_host_free",
    ] {
        assert!(
            symbols.lines().any(|line| line == name),
            "{name} is missing"
        );
    }
}
