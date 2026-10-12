//! Every corpus screen, drawn by Slint's software renderer and compared with its reviewed PNG.
//!
//! The web and SwiftUI screenshots live in `@weft/visual`. Slint draws its own pixels, so the
//! comparison is here: the testing backend's software renderer, the fluent style (one look on
//! every OS, not the platform widgets) and the light color scheme (a dark desktop would change
//! every pixel). Fonts still come from the OS, so baselines are per platform, the same rule as
//! `@weft/visual`. `WEFT_UPDATE_SCREENSHOTS=1` rewrites them.

// A test crate: a failed unwrap or panic is a failed test, which is the point.
#![allow(clippy::unwrap_used, clippy::panic)]

mod common;

use std::path::{Path, PathBuf};

use serde_json::Value as Json;
use slint_interpreter::ComponentHandle;
use weft_core::parse_json;
use weft_slint::{GenerateOptions, generate, runtime};

use common::{document, ensure_backend, screens, setup};

/// The web screenshots are 480px wide (`packages/visual/test/web/stage.ts`); the same measure here.
const WIDTH: u32 = 480;
/// Tall enough that trimming, not clipping, decides the height. A screen that reaches the bottom
/// fails instead of silently losing its last rows.
const CANVAS: u32 = 4_000;
/// Background rows kept under the last row that is not the window color.
const PAD: u32 = 16;

struct Image {
    width: u32,
    height: u32,
    data: Vec<u8>,
}

fn platform() -> String {
    // `@weft/visual` names its folders from Node (`darwin-arm64`). Rust says `macos` and `aarch64`.
    let os = match std::env::consts::OS {
        "macos" => "darwin",
        other => other,
    };
    let arch = match std::env::consts::ARCH {
        "aarch64" => "arm64",
        other => other,
    };
    format!("{os}-{arch}")
}

fn baselines_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/screenshots/baselines")
        .join(platform())
}

fn diffs_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/screenshots/diffs")
        .join(platform())
}

fn trim(width: u32, height: u32, bytes: &[u8]) -> Image {
    let bg = &bytes[..4];
    let mut last = 0u32;
    for y in 0..height {
        let start = (y * width * 4) as usize;
        let row = &bytes[start..start + (width * 4) as usize];
        if row.chunks(4).any(|px| px != bg) {
            last = y;
        }
    }
    let kept = (last + 1 + PAD).min(height);
    assert!(
        kept < height,
        "the render filled the {width}x{height} canvas; raise CANVAS"
    );
    Image {
        width,
        height: kept,
        data: bytes[..(kept * width * 4) as usize].to_vec(),
    }
}

fn render(source: &str, data: &Json) -> Image {
    ensure_backend();
    let instance = runtime::compile(source, "fluent")
        .unwrap()
        .create()
        .unwrap();
    let problems = runtime::apply_data(&instance, source, data);
    assert!(problems.is_empty(), "{problems:#?}");
    instance
        .window()
        .set_size(slint_interpreter::PhysicalSize::new(WIDTH, CANVAS));
    let buffer = instance.window().take_snapshot().unwrap();
    let bytes = buffer.as_bytes();
    assert_eq!(bytes.len(), (buffer.width() * buffer.height() * 4) as usize);
    trim(buffer.width(), buffer.height(), bytes)
}

fn encode_png(image: &Image) -> Vec<u8> {
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, image.width, image.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(&image.data)
        .unwrap();
    out
}

fn decode_png(bytes: &[u8]) -> Image {
    let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    let mut reader = decoder.read_info().unwrap();
    let mut buf = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut buf).unwrap();
    assert_eq!(info.color_type, png::ColorType::Rgba);
    assert_eq!(info.bit_depth, png::BitDepth::Eight);
    buf.truncate(info.buffer_size());
    Image {
        width: info.width,
        height: info.height,
        data: buf,
    }
}

fn pixel(image: &Image, x: u32, y: u32) -> [u8; 4] {
    // A missing row is a size change, painted in a color no screen uses, as the web comparator does.
    if x >= image.width || y >= image.height {
        return [255, 0, 255, 255];
    }
    let i = ((y * image.width + x) * 4) as usize;
    image.data[i..i + 4].try_into().unwrap()
}

fn differing(left: &Image, right: &Image) -> usize {
    let width = left.width.max(right.width);
    let height = left.height.max(right.height);
    let mut count = 0;
    for y in 0..height {
        for x in 0..width {
            if pixel(left, x, y) != pixel(right, x, y) {
                count += 1;
            }
        }
    }
    count
}

fn write_diff(name: &str, expected: &Image, actual: &Image) {
    let width = expected.width.max(actual.width);
    let height = expected.height.max(actual.height);
    let mut data = vec![0; (width * height * 4) as usize];
    for y in 0..height {
        for x in 0..width {
            let i = ((y * width + x) * 4) as usize;
            let (e, a) = (pixel(expected, x, y), pixel(actual, x, y));
            if e == a {
                data[i..i + 4].copy_from_slice(&[e[0] / 4, e[1] / 4, e[2] / 4, 255]);
            } else {
                data[i..i + 4].copy_from_slice(&[255, 0, 0, 255]);
            }
        }
    }
    let dir = diffs_dir();
    std::fs::create_dir_all(&dir).unwrap();
    let png = encode_png(&Image {
        width,
        height,
        data,
    });
    std::fs::write(dir.join(format!("{name}.diff.png")), &png).unwrap();
    std::fs::write(dir.join(format!("{name}.actual.png")), encode_png(actual)).unwrap();
    std::fs::write(
        dir.join(format!("{name}.expected.png")),
        encode_png(expected),
    )
    .unwrap();
}

fn corpus() -> Vec<(String, PathBuf)> {
    screens()
        .into_iter()
        .filter(|(_, path)| path.components().any(|c| c.as_os_str() == "corpus"))
        .collect()
}

fn generated(path: &Path) -> (String, Json) {
    let (catalog, tokens) = setup();
    let markup = std::fs::read_to_string(path).unwrap();
    let doc = document(&markup, &catalog, &tokens);
    let source = generate(
        &doc,
        &GenerateOptions {
            catalog: &catalog,
            tokens: &tokens,
            name: None,
        },
    )
    .unwrap();
    let data =
        parse_json(&std::fs::read_to_string(path.with_file_name("data.json")).unwrap()).unwrap();
    (source, data)
}

fn screen(name: &str) -> (String, Json) {
    let path = corpus().into_iter().find(|(n, _)| n == name).unwrap().1;
    generated(&path)
}

#[test]
fn a_render_is_deterministic() {
    let (source, data) = screen("login");
    let first = render(&source, &data);
    let second = render(&source, &data);
    assert_eq!(differing(&first, &second), 0);
}

#[test]
fn a_one_pixel_spacing_change_is_caught() {
    let (source, data) = screen("login");
    let changed = source.replacen("spacing: 16px", "spacing: 17px", 1);
    assert_ne!(changed, source);
    let count = differing(&render(&source, &data), &render(&changed, &data));
    assert!(
        count > 0,
        "one pixel of spacing did not change the screenshot"
    );
}

#[test]
fn a_same_length_word_change_is_caught() {
    let (source, data) = screen("login");
    // The same words also sit in the source comment, which is not drawn. The literal is.
    let changed = source.replacen(
        "text: \"Forgot password?\"",
        "text: \"Forgot passcode?\"",
        1,
    );
    assert_ne!(changed, source);
    let count = differing(&render(&source, &data), &render(&changed, &data));
    assert!(count > 0, "a changed word did not change the screenshot");
}

#[test]
fn every_corpus_screen_matches_its_baseline() {
    let dir = baselines_dir();
    let updating = std::env::var_os("WEFT_UPDATE_SCREENSHOTS").is_some_and(|v| v == "1");
    if !updating && !dir.is_dir() {
        eprintln!(
            "no Slint baselines for {}; skipping the PNG comparison (the mutation checks still run)",
            platform()
        );
        return;
    }
    if updating {
        std::fs::create_dir_all(&dir).unwrap();
    }
    let mut mismatches = Vec::new();
    for (name, path) in corpus() {
        let (source, data) = generated(&path);
        let actual = render(&source, &data);
        let file = dir.join(format!("{name}.png"));
        if updating {
            std::fs::write(&file, encode_png(&actual)).unwrap();
            continue;
        }
        let expected = decode_png(&std::fs::read(&file).unwrap_or_else(|_| {
            panic!(
                "missing baseline {} (set WEFT_UPDATE_SCREENSHOTS=1 to write it)",
                file.display()
            )
        }));
        let count = differing(&expected, &actual);
        if count > 0 {
            write_diff(&name, &expected, &actual);
            mismatches.push(format!(
                "{name}: {count} pixels, see tests/screenshots/diffs/"
            ));
        }
    }
    assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
}
