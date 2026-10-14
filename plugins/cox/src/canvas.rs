//! The Design tab (plan.md T106.8; cox `docs/design/plugin-canvas.md`): the last markup the tools
//! saw for each screen, drawn as static pages whose elements carry their diagnostic paths, and a
//! click on one turned into a short attachment the person may send with their next message.
//!
//! cox asks for the frames again after each of this plugin's tools finishes, so a tool call only
//! has to remember what it saw; nothing here calls `cox_redraw`.

use cox_plugin_sdk::{CanvasAttach, CanvasDecl, CanvasOut, Frame, MAX_CANVAS_FRAMES, Value};
use serde_json::json;
use weft_binding::tools::{Outcome, ToolHost};
use weft_core::{ParseOptions, element_paths, parse, path_segment};

/// A phone's width, the design's default; the pane scrolls and zooms.
pub const FRAME_WIDTH_PT: u32 = 390;
/// The attribute the page writes and a click reports (`HtmlOptions::paths`).
pub const SELECT_ATTR: &str = "data-weft-path";
/// The longest markup line an attachment quotes, in characters, so it stays near 1 KiB.
const LINE_CHARS: usize = 400;

/// The pane cox shows when the person grants `desktop.canvas`.
pub fn decl() -> CanvasDecl {
    CanvasDecl {
        title: "Weft".into(),
        symbol: "paintpalette".into(),
        select_attr: SELECT_ATTR.into(),
    }
}

struct Screen {
    /// The frame's id: `s<n>`, since a screen id may hold characters a frame id may not.
    frame: String,
    id: String,
    markup: String,
    data: Option<Value>,
    project: Option<Value>,
    /// The last page that drew. A later markup that does not draw leaves it, so the frame keeps
    /// the last good page while the agent fixes the markup.
    page: Option<String>,
    stale: bool,
    /// When it last changed, to drop the screen left alone longest once the canvas is full.
    changed: u64,
}

#[derive(Default)]
pub struct Canvas {
    screens: Vec<Screen>,
    frames: u64,
    changes: u64,
}

impl Canvas {
    /// Remembers the screen a successful call worked on: the markup `weft_validate`,
    /// `weft_format` and `weft_render` were given, or the markup `weft_patch` returned. A call
    /// without `data` or `project` keeps the ones an earlier call gave for that screen.
    pub fn saw(&mut self, name: &str, input: &Value, outcome: &Outcome) {
        let blocks = match outcome {
            Outcome::Content { is_error: true, .. } => return,
            Outcome::Content { blocks, .. } => blocks.as_slice(),
            Outcome::Render(_) => &[],
        };
        let markup = match name {
            "weft_validate" | "weft_format" | "weft_render" => input["markup"].as_str(),
            "weft_patch" => blocks.first().map(String::as_str),
            _ => None,
        };
        let Some(markup) = markup else { return };
        let Some(document) = parse(markup, &ParseOptions::default()).document else {
            return;
        };
        let (kind, Some(id)) = (document.root.kind, document.root.id) else {
            return;
        };
        if kind != "screen" {
            return;
        }
        let given = |key: &str| input.get(key).filter(|v| !v.is_null()).cloned();
        let (data, project) = (given("data"), given("project"));
        self.changes += 1;
        if let Some(screen) = self.screens.iter_mut().find(|s| s.id == id) {
            let data = data.or_else(|| screen.data.clone());
            let project = project.or_else(|| screen.project.clone());
            if screen.markup == markup && screen.data == data && screen.project == project {
                return;
            }
            (screen.markup, screen.data, screen.project) = (markup.to_owned(), data, project);
            (screen.stale, screen.changed) = (true, self.changes);
            return;
        }
        if self.screens.len() == MAX_CANVAS_FRAMES {
            let oldest = (0..self.screens.len()).min_by_key(|&i| self.screens[i].changed);
            if let Some(oldest) = oldest {
                self.screens.remove(oldest);
            }
        }
        self.frames += 1;
        self.screens.push(Screen {
            frame: format!("s{}", self.frames),
            id,
            markup: markup.to_owned(),
            data,
            project,
            page: None,
            stale: true,
            changed: self.changes,
        });
    }

    /// One frame per screen that has drawn, in the order the screens first appeared. A changed
    /// screen is drawn as `weft_render` with `html` draws it, in the scope it was checked in.
    pub fn frames(&mut self, host: &ToolHost) -> CanvasOut {
        for screen in self.screens.iter_mut().filter(|s| s.stale) {
            screen.stale = false;
            let mut args = json!({ "markup": screen.markup, "html": true });
            if let Some(data) = &screen.data {
                args["data"] = data.clone();
            }
            if let Some(project) = &screen.project {
                args["project"] = project.clone();
            }
            if let Outcome::Render(request) = host.call("weft_render", &args.to_string())
                && let Ok(page) = crate::draw(&request, true)
            {
                screen.page = Some(page);
            }
        }
        let frames = self.screens.iter().filter_map(|s| {
            Some(Frame {
                id: s.frame.clone(),
                title: s.id.clone(),
                width_pt: FRAME_WIDTH_PT,
                html: s.page.clone()?,
            })
        });
        CanvasOut {
            frames: frames.collect(),
        }
    }

    /// What a click on `path` in `frame` tells the agent: the screen, the path, and the element's
    /// markup line when the path names an element of the markup as written. cox checked the
    /// value's length and characters; it is only compared here and quoted as text.
    pub fn select(&self, frame: &str, path: &str) -> Option<CanvasAttach> {
        let screen = self.screens.iter().find(|s| s.frame == frame)?;
        let mut text = format!(
            "Selected on the Weft canvas: screen `{}`, element `{path}`",
            screen.id
        );
        let document = parse(&screen.markup, &ParseOptions::default()).document;
        let node = document.as_ref().and_then(|d| {
            element_paths(d)
                .into_iter()
                .find_map(|(node, p)| (p == path).then_some(node))
        });
        match node {
            Some(node) => {
                let label = path_segment(&node.kind, node.id.as_deref(), None);
                text.push_str(&format!(" ({label})."));
                let line = node.source.0.as_ref().map(|s| s.pos.line as usize);
                let quoted =
                    line.and_then(|n| Some((n, screen.markup.lines().nth(n.checked_sub(1)?)?)));
                if let Some((n, quoted)) = quoted {
                    let quoted: String = quoted.trim().chars().take(LINE_CHARS).collect();
                    text.push_str(&format!("\nMarkup line {n}: {quoted}"));
                }
            }
            // Inside an expanded fragment use or a repeated item the path is the drawn page's.
            None => text.push_str(", which the page drew from a fragment use or sample data."),
        }
        let last = path.rsplit('/').next().unwrap_or(path);
        Some(CanvasAttach {
            name: format!("Weft: {last}"),
            text,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOME: &str = "<screen weft=\"0.1\" id=\"home\" label=\"Home\">\n  <stack id=\"body\">\n    <button id=\"buy\" on-press=\"shop.buy\" text=\"Buy\"/>\n  </stack>\n</screen>";

    fn host() -> ToolHost {
        ToolHost::core(Default::default()).unwrap()
    }

    fn seen(canvas: &mut Canvas, name: &str, input: Value) {
        let outcome = host().call(name, &input.to_string());
        canvas.saw(name, &input, &outcome);
    }

    #[test]
    fn a_screen_the_tools_saw_is_a_frame_with_each_elements_path() {
        let mut canvas = Canvas::default();
        seen(&mut canvas, "weft_validate", json!({ "markup": HOME }));
        let out = canvas.frames(&host());
        assert_eq!(out.frames.len(), 1);
        let frame = &out.frames[0];
        assert_eq!((frame.id.as_str(), frame.title.as_str()), ("s1", "home"));
        assert_eq!(frame.width_pt, FRAME_WIDTH_PT);
        let marked = "data-weft-path=\"/screen#home/stack#body/button#buy\"";
        assert!(frame.html.contains(marked), "{}", frame.html);
    }

    #[test]
    fn a_patch_replaces_the_screen_and_a_failed_call_changes_nothing() {
        let mut canvas = Canvas::default();
        seen(&mut canvas, "weft_format", json!({ "markup": HOME }));
        let patches = json!([{ "op": "set", "id": "buy", "prop": "text", "value": "Pay" }]);
        seen(
            &mut canvas,
            "weft_patch",
            json!({ "markup": HOME, "patches": patches }),
        );
        let page = canvas.frames(&host()).frames[0].html.clone();
        assert!(page.contains("Pay"), "{page}");
        // Refused: the frame keeps the last page that drew.
        let broken = HOME.replace("</screen>", "");
        seen(&mut canvas, "weft_render", json!({ "markup": broken }));
        // Valid when lenient, so seen, but not drawable in strict mode: still the last page.
        let unknown = HOME.replace("</stack>", "<fancy id=\"f\"/></stack>");
        seen(&mut canvas, "weft_validate", json!({ "markup": unknown }));
        let out = canvas.frames(&host());
        assert_eq!(out.frames.len(), 1);
        assert_eq!(out.frames[0].html, page);
    }

    #[test]
    fn the_canvas_keeps_the_screens_changed_last() {
        let mut canvas = Canvas::default();
        for i in 0..=MAX_CANVAS_FRAMES {
            let markup = HOME.replace("\"home\"", &format!("\"s{i}\""));
            seen(&mut canvas, "weft_validate", json!({ "markup": markup }));
        }
        let titles: Vec<String> = canvas
            .frames(&host())
            .frames
            .into_iter()
            .map(|f| f.title)
            .collect();
        assert_eq!(titles.len(), MAX_CANVAS_FRAMES);
        assert_eq!(titles[0], "s1");
        // Markup that is not a screen with an id is no frame.
        seen(
            &mut canvas,
            "weft_validate",
            json!({ "markup": "<screen weft=\"0.1\"/>" }),
        );
        assert_eq!(canvas.frames(&host()).frames.len(), MAX_CANVAS_FRAMES);
    }

    #[test]
    fn a_click_is_the_screen_the_path_and_the_markup_line() {
        let mut canvas = Canvas::default();
        seen(&mut canvas, "weft_validate", json!({ "markup": HOME }));
        let path = "/screen#home/stack#body/button#buy";
        let attach = canvas.select("s1", path).unwrap();
        assert_eq!(attach.name, "Weft: button#buy");
        assert_eq!(
            attach.text,
            format!(
                "Selected on the Weft canvas: screen `home`, element `{path}` (button#buy).\nMarkup line 3: <button id=\"buy\" on-press=\"shop.buy\" text=\"Buy\"/>"
            )
        );
        assert!(attach.text.len() < 1024);
        let drawn = canvas
            .select("s1", "/screen#home/stack#body/text[0]")
            .unwrap();
        assert!(
            drawn.text.ends_with("from a fragment use or sample data."),
            "{}",
            drawn.text
        );
        assert_eq!(canvas.select("s2", path), None);
    }
}
