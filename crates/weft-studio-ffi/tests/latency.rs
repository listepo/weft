//! Every corpus screen opens and draws its first frame through the C ABI, and the time from
//! `ws_open` to that frame is printed (T69.1c measures it in release:
//! `cargo test --release -p weft-studio-ffi --test latency -- --nocapture`). Its own test binary,
//! because Slint keeps the first thread that draws as its UI thread for the whole process.

// A test crate: a failed unwrap or panic is a failed test, which is the point.
#![allow(clippy::unwrap_used, clippy::panic)]
// It drives the C ABI, as a shell does.
#![allow(unsafe_code)]

use std::ffi::{CStr, CString};
use std::path::Path;
use std::ptr;
use std::time::{Duration, Instant};

use slint_bindings_ffi::{sb_host_frame_len, sb_host_free, sb_host_render};
use weft_studio_ffi::abi::{ws_editor_free, ws_editor_new, ws_last_error, ws_open, ws_surface_new};

fn last_error() -> String {
    // SAFETY: a non-null `ws_last_error` is NUL-terminated until the next call.
    unsafe { CStr::from_ptr(ws_last_error()) }
        .to_string_lossy()
        .into_owned()
}

#[test]
fn every_corpus_screen_opens_and_draws_its_first_frame() {
    let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus");
    let mut screens: Vec<_> = std::fs::read_dir(corpus)
        .unwrap()
        .map(|entry| entry.unwrap().path().join("screen.weft"))
        .filter(|path| path.is_file())
        .collect();
    screens.sort();
    assert!(!screens.is_empty());
    let (width, height) = (480u32, 800u32);
    let mut pixels = vec![0u8; (width * height * 4) as usize];
    let mut slowest = (Duration::ZERO, String::new());
    for screen in screens {
        let name = screen
            .parent()
            .unwrap()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let path = CString::new(screen.to_str().unwrap()).unwrap();
        let editor = ws_editor_new(None, ptr::null_mut());
        let start = Instant::now();
        // SAFETY: a live editor, a NUL-terminated path, a host and a buffer of its frame length;
        // each handle is freed once.
        unsafe {
            assert!(ws_open(editor, path.as_ptr()), "{name}: {}", last_error());
            let host = ws_surface_new(editor, width, height, 1.0);
            assert!(!host.is_null(), "{name}: {}", last_error());
            assert_eq!(sb_host_frame_len(host), pixels.len());
            let frame = sb_host_render(host, pixels.as_mut_ptr(), pixels.len());
            let elapsed = start.elapsed();
            assert_eq!(frame.failed, 0, "{name}");
            assert!(pixels.chunks(4).any(|p| p[3] != 0), "{name}: transparent");
            println!("{name}: {} ms", elapsed.as_millis());
            if elapsed > slowest.0 {
                slowest = (elapsed, name);
            }
            sb_host_free(host);
            ws_editor_free(editor);
        }
    }
    println!("slowest: {} in {} ms", slowest.1, slowest.0.as_millis());
}
