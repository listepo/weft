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

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::task::{Context, Poll, Waker};

use serde_json::Value as Json;
use slint_interpreter::{ComponentHandle, ComponentInstance, SharedString, Struct, Value};
use weft_core::parse_json;
use weft_slint::{GenerateOptions, generate};

use common::{document, exported_component, screens, setup};

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

enum SlintTy {
    String,
    Bool,
    Int,
    Float,
    StringList,
    StructList(Vec<(String, Leaf)>),
}

enum Leaf {
    String,
    Bool,
    Int,
    Float,
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

/// One backend per thread. `threading: false` keeps the platform thread-local, so parallel tests
/// do not share it; a second call on the same thread finds it already set.
fn ensure_backend() {
    i_slint_core::with_global_context(
        || {
            let backend = i_slint_backend_testing::TestingBackend::new(
                i_slint_backend_testing::TestingBackendOptions {
                    mock_time: true,
                    threading: false,
                    renderer_name: Some("software".into()),
                },
            );
            Ok(Box::new(backend) as Box<dyn i_slint_core::platform::Platform>)
        },
        |ctx| ctx.set_color_scheme(i_slint_core::items::ColorScheme::Light),
    )
    .unwrap();
}

fn compile(source: &str) -> slint_interpreter::ComponentDefinition {
    let mut compiler = slint_interpreter::Compiler::default();
    compiler.set_style("fluent".into());
    let mut build =
        std::pin::pin!(compiler.build_from_source(source.into(), "screen.slint".into()));
    let mut cx = Context::from_waker(Waker::noop());
    let result = loop {
        if let Poll::Ready(result) = build.as_mut().poll(&mut cx) {
            break result;
        }
    };
    let errors: Vec<_> = result
        .diagnostics()
        .filter(|d| d.level() == slint_interpreter::DiagnosticLevel::Error)
        .map(|d| d.message().to_owned())
        .collect();
    assert!(errors.is_empty(), "{errors:#?}");
    result.component(exported_component(source)).unwrap()
}

fn properties(source: &str) -> Vec<(String, String)> {
    source
        .lines()
        .filter_map(|line| {
            let rest = line.trim().strip_prefix("in-out property <")?;
            let (ty, name) = rest.split_once("> ")?;
            Some((name.trim_end_matches(';').to_owned(), ty.to_owned()))
        })
        .collect()
}

fn leaf(ty: &str) -> Leaf {
    match ty {
        "string" => Leaf::String,
        "bool" => Leaf::Bool,
        "int" => Leaf::Int,
        "float" => Leaf::Float,
        other => panic!("unhandled field type {other}"),
    }
}

fn parse_ty(ty: &str) -> SlintTy {
    match ty {
        "string" => SlintTy::String,
        "bool" => SlintTy::Bool,
        "int" => SlintTy::Int,
        "float" => SlintTy::Float,
        "[string]" => SlintTy::StringList,
        other if other.starts_with("[{") && other.ends_with("}]") => {
            let inner = &other[2..other.len() - 2];
            let fields = inner
                .split(',')
                .map(|part| {
                    let (name, ty) = part.trim().split_once(':').unwrap();
                    (name.trim().to_owned(), leaf(ty.trim()))
                })
                .collect();
            SlintTy::StructList(fields)
        }
        other => panic!("unhandled Slint type {other}"),
    }
}

/// `$.stats.velocity` is the property `stats-velocity`, so a nested object flattens on `-`.
/// An array stays whole: it is the model, not another path.
fn flatten(value: &Json, prefix: &str, out: &mut BTreeMap<String, Json>) {
    if let Json::Object(map) = value
        && !map.is_empty()
    {
        for (key, child) in map {
            let name = if prefix.is_empty() {
                key.clone()
            } else {
                format!("{prefix}-{key}")
            };
            flatten(child, &name, out);
        }
        return;
    }
    if !prefix.is_empty() {
        out.insert(prefix.to_owned(), value.clone());
    }
}

fn lookup<'a>(flat: &'a BTreeMap<String, Json>, name: &str) -> Option<&'a Json> {
    flat.get(name).or_else(|| {
        // A path Slint's `Window` already owns is stored as `<name>-data`.
        name.strip_suffix("-data").and_then(|bare| flat.get(bare))
    })
}

fn missing(ty: &Leaf) -> Json {
    match ty {
        Leaf::String => Json::String(String::new()),
        Leaf::Bool => Json::Bool(false),
        Leaf::Int | Leaf::Float => Json::from(0),
    }
}

fn leaf_value(ty: &Leaf, json: &Json, at: &str) -> Value {
    match ty {
        // A number field is still a LineEdit, so its property is a string and the sample may be a number.
        Leaf::String => Value::String(SharedString::from(match json {
            Json::String(text) => text.clone(),
            Json::Number(number) => number.to_string(),
            Json::Bool(value) => value.to_string(),
            other => panic!("{at}: expected text, got {other}"),
        })),
        // A bool property is often a text path read for truthiness (`!= ""`), and the sample
        // still holds the text, as `selection: ""`.
        Leaf::Bool => Value::Bool(match json {
            Json::Bool(value) => *value,
            Json::String(text) => !text.is_empty(),
            Json::Number(number) => number.as_f64().unwrap_or(0.0) != 0.0,
            other => panic!("{at}: expected a bool, got {other}"),
        }),
        Leaf::Int | Leaf::Float => Value::Number(
            json.as_f64()
                .unwrap_or_else(|| panic!("{at}: expected a number, got {json}")),
        ),
    }
}

fn model(items: Vec<Value>) -> Value {
    let rc = i_slint_core::model::VecModel::from_slice(&items);
    Value::from(rc)
}

/// A `[string]` model is sometimes fed by objects whose one text field is the row, as `cities`
/// is `[{ name }]` while the combo box model is `[string]`.
fn string_item(item: &Json, at: &str) -> Value {
    match item {
        Json::Object(map) => {
            let texts: Vec<_> = map.iter().filter(|(_, value)| value.is_string()).collect();
            assert_eq!(
                texts.len(),
                1,
                "{at}: a [string] row needs one text field, got {item}"
            );
            leaf_value(&Leaf::String, texts[0].1, at)
        }
        other => leaf_value(&Leaf::String, other, at),
    }
}

fn json_to_value(ty: &SlintTy, json: &Json, at: &str) -> Value {
    match ty {
        SlintTy::String => leaf_value(&Leaf::String, json, at),
        SlintTy::Bool => leaf_value(&Leaf::Bool, json, at),
        SlintTy::Int => leaf_value(&Leaf::Int, json, at),
        SlintTy::Float => leaf_value(&Leaf::Float, json, at),
        SlintTy::StringList => {
            let items = json
                .as_array()
                .unwrap_or_else(|| panic!("{at}: expected an array, got {json}"))
                .iter()
                .map(|item| string_item(item, at))
                .collect();
            model(items)
        }
        SlintTy::StructList(fields) => {
            let items = json
                .as_array()
                .unwrap()
                .iter()
                .map(|row| {
                    let mut item = Struct::default();
                    for (name, ty) in fields {
                        let value = match row.get(name) {
                            Some(v) => leaf_value(ty, v, name),
                            None => leaf_value(ty, &missing(ty), name),
                        };
                        item.set_field(name.clone(), value);
                    }
                    Value::Struct(item)
                })
                .collect();
            model(items)
        }
    }
}

fn apply(source: &str, instance: &ComponentInstance, data: &Json) {
    let mut flat = BTreeMap::new();
    flatten(data, "", &mut flat);
    for (name, ty) in properties(source) {
        let Some(value) = lookup(&flat, &name) else {
            continue;
        };
        let parsed = parse_ty(&ty);
        instance
            .set_property(&name, json_to_value(&parsed, value, &name))
            .unwrap_or_else(|err| panic!("{name} ({ty}): {err} from {value}"));
    }
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
    let instance = compile(source).create().unwrap();
    apply(source, &instance, data);
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
