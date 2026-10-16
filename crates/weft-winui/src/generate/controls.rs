//! The value, choice and content controls (T105.3): every kind whose WinUI control holds its value
//! as something other than text goes through the `weft:Choice.Value` helper of `Weft.cs`, and the
//! number, date and colour literals are written as `weft_core::controls` reads them, so a literal
//! screen shows what every other target shows.

use weft_core::controls::{DateType, color_value, date_text, slider_span, stepper_bounds};
use weft_core::{Child, Node, Value, js_number, safe_absolute_url, safe_url};

use super::{Gen, path_of};
use crate::data::{Leaf, Read};
use crate::xaml::{Element, literal};

/// The kinds of this module, as `generate::KINDS`: kind, WinUI element, props besides the common.
pub(super) const KINDS: &[(&str, &str, &str)] = &[
    ("select", "ComboBox", "value disabled"),
    ("combobox", "ComboBox", "value placeholder disabled"),
    ("option", "ComboBoxItem", "value text"),
    ("radio-group", "RadioButtons", "value"),
    ("radio", "RadioButton", "value disabled text"),
    ("segmented-control", "SelectorBar", "value"),
    ("segment", "SelectorBarItem", "value disabled text"),
    ("slider", "Slider", "value min max step disabled"),
    ("stepper", "NumberBox", "value min max step disabled"),
    (
        "date-picker",
        "CalendarDatePicker",
        "type value min max disabled",
    ),
    ("color-picker", "DropDownButton", "value disabled"),
    ("link", "HyperlinkButton", "href text"),
    ("image", "Image", "src"),
    ("model", "Image", "src usdz fallback"),
    ("alert", "InfoBar", "tone text"),
    ("menu", "DropDownButton", ""),
    ("menu-item", "MenuFlyoutItem", "disabled text"),
];

/// The element of a `date-picker`, which depends on its `type`.
pub(super) fn element(node: &Node, control: &'static str) -> &'static str {
    match (node.kind.as_str(), date_type(node)) {
        ("date-picker", Some(DateType::Time)) => "TimePicker",
        ("date-picker", Some(DateType::DateTime)) => "StackPanel",
        _ => control,
    }
}

/// The attribute that shows a caption `label`, where it is not `AutomationProperties.Name`.
pub(super) fn caption(node: &Node) -> Option<&'static str> {
    match node.kind.as_str() {
        "date-picker" if date_type(node) == Some(DateType::DateTime) => None,
        "select" | "combobox" | "radio-group" | "slider" | "stepper" | "date-picker" => {
            Some("Header")
        }
        _ => None,
    }
}

/// A literal `type`; `None` when bound, which `Gen::word` refuses.
fn date_type(node: &Node) -> Option<DateType> {
    match node.props.get("type") {
        None => Some(DateType::Date),
        Some(Value::String(t)) => Some(DateType::of(t)),
        Some(_) => None,
    }
}

impl Gen<'_> {
    pub(super) fn control(&mut self, e: &mut Element, node: &Node) {
        match node.kind.as_str() {
            "select" | "combobox" => {
                let editable = node.kind == "combobox";
                if editable {
                    e.attr("IsEditable", "True");
                    self.set(e, node, "placeholder", "PlaceholderText", Leaf::Text);
                }
                self.choice(e, node);
                self.disabled(e, node);
                let outer = std::mem::replace(&mut self.editable, editable);
                self.items(e, node);
                self.editable = outer;
            }
            "option" => {
                // A combobox writes the chosen item's text into its box, so its item shows the
                // value and keeps the option's own text as a tooltip.
                let shown = if self.editable {
                    self.set(e, node, "value", "Content", Leaf::Text);
                    "ToolTipService.ToolTip"
                } else {
                    "Content"
                };
                self.content(e, node, shown);
                self.set(e, node, "value", "Tag", Leaf::Text);
            }
            "radio-group" | "segmented-control" => {
                self.choice(e, node);
                self.children(e, &node.children);
            }
            "radio" | "segment" => {
                let text = if node.kind == "radio" {
                    "Content"
                } else {
                    "Text"
                };
                self.content(e, node, text);
                self.set(e, node, "value", "Tag", Leaf::Text);
                self.disabled(e, node);
            }
            "slider" | "stepper" => self.number(e, node),
            "date-picker" => self.date(e, node),
            "color-picker" => self.color(e, node),
            "link" => {
                self.content(e, node, "Content");
                self.href(e, node);
            }
            "image" => self.source(e, node, "src", true),
            "model" => {
                self.source(e, node, "fallback", false);
                self.marker(e, node, "src", "Src");
                self.marker(e, node, "usdz", "Usdz");
            }
            "alert" => self.alert(e, node),
            "menu" => {
                self.set(e, node, "label", "Content", Leaf::Text);
                let mut flyout = Element::new("MenuFlyout");
                self.items(&mut flyout, node);
                e.children.push(property("DropDownButton.Flyout", flyout));
            }
            "menu-item" => {
                self.content(e, node, "Text");
                self.disabled(e, node);
            }
            _ => {}
        }
    }

    /// `weft:Choice.Value`, two-way: the helper turns the text into the control's selection.
    fn choice(&mut self, e: &mut Element, node: &Node) {
        if let Some(v) = self.value(node, "value", Leaf::Text, true) {
            e.attr("weft:Choice.Value", v);
        }
    }

    fn number(&mut self, e: &mut Element, node: &Node) {
        let slider = node.kind == "slider";
        if !slider {
            e.attr("SpinButtonPlacementMode", "Inline");
        }
        let lit = |p: &str| match node.props.get(p) {
            Some(Value::Number(n)) => Some(*n),
            _ => None,
        };
        let (lo, hi, step) = if slider {
            let s = slider_span(lit("min"), lit("max"), lit("step"));
            // A bound `min` is not known here, so a literal `max` is raised only to a literal one.
            let bound = matches!(node.props.get("min"), Some(Value::Bind { .. }));
            (
                Some(s.min),
                if bound { lit("max") } else { Some(s.max) },
                s.step,
            )
        } else {
            let b = stepper_bounds(lit("min"), lit("max"), lit("step"));
            (b.min, b.max, b.step)
        };
        let step_attr = if slider {
            "StepFrequency"
        } else {
            "SmallChange"
        };
        for (prop, xaml, fixed) in [
            ("min", "Minimum", lo),
            ("max", "Maximum", hi),
            ("step", step_attr, Some(step)),
        ] {
            match (node.props.get(prop), fixed) {
                (Some(Value::Number(_)), Some(n)) => e.attr(xaml, js_number(n)),
                (Some(_), _) => self.set(e, node, prop, xaml, Leaf::Number),
                _ => {}
            }
        }
        if let Some(v) = self.value(node, "value", Leaf::Number, true) {
            e.attr("Value", v);
        }
        self.disabled(e, node);
    }

    fn date(&mut self, e: &mut Element, node: &Node) {
        let Some(kind) = date_type(node) else {
            self.word(node, "type");
            return;
        };
        self.choice(e, node);
        if kind != DateType::DateTime {
            self.disabled(e, node);
            return self.bounds(e, node, kind);
        }
        e.attr("Orientation", "Horizontal");
        let mut day = Element::new("CalendarDatePicker");
        self.set(&mut day, node, "label", "Header", Leaf::Text);
        self.disabled(&mut day, node);
        self.bounds(&mut day, node, kind);
        let mut time = Element::new("TimePicker");
        self.disabled(&mut time, node);
        e.children.extend([day, time]);
    }

    /// `min` and `max`: a date's bounds through `Weft.Earliest` and `Weft.Latest`; a time picker
    /// has none, so its bounds, and a literal not of the format of `type`, are markers.
    fn bounds(&mut self, e: &mut Element, node: &Node, kind: DateType) {
        for (prop, xaml, function, marker) in [
            ("min", "MinDate", "Earliest", "Min"),
            ("max", "MaxDate", "Latest", "Max"),
        ] {
            let value = match (node.props.get(prop), kind) {
                (None, _) => continue,
                (_, DateType::Time) => None,
                // `date_text` keeps only digits, `-`, `T` and `:`, so no quote can end the literal.
                (Some(Value::String(s)), _) if !date_text(kind, s).is_empty() => {
                    Some(format!("{{x:Bind weft:Weft.{function}('{s}')}}"))
                }
                (Some(Value::Bind { bind, not: false }), _) => {
                    let at = path_of(node);
                    let path =
                        self.data
                            .read(bind, Read::Like(Leaf::Text), &at, &mut self.problems);
                    path.map(|p| format!("{{x:Bind weft:Weft.{function}({p}), Mode=OneWay}}"))
                }
                _ => None,
            };
            match value {
                Some(v) => e.attr(xaml, v),
                None => self.marker(e, node, prop, marker),
            }
        }
    }

    fn color(&mut self, e: &mut Element, node: &Node) {
        self.choice(e, node);
        self.disabled(e, node);
        let mut swatch = Element::new("Border");
        swatch.attr("Width", "16");
        swatch.attr("Height", "16");
        swatch.attr("CornerRadius", "4");
        let fill = match node.props.get("value") {
            Some(Value::String(s)) => Some(color_value(s)),
            Some(Value::Bind { bind, not: false }) => {
                let at = path_of(node);
                let path = self
                    .data
                    .read(bind, Read::Like(Leaf::Text), &at, &mut self.problems);
                path.map(|p| format!("{{x:Bind weft:Weft.Swatch({p}), Mode=OneWay}}"))
            }
            _ => None,
        };
        swatch.attr("Background", fill.unwrap_or_else(|| color_value("")));
        let mut row = Element::new("StackPanel");
        row.attr("Orientation", "Horizontal");
        row.attr("Spacing", "8");
        row.children.push(swatch);
        if node.props.contains_key("label") {
            let mut caption = Element::new("TextBlock");
            self.set(&mut caption, node, "label", "Text", Leaf::Text);
            row.children.push(caption);
        }
        let mut picker = Element::new("ColorPicker");
        picker.attr("IsAlphaEnabled", "False");
        let mut flyout = Element::new("Flyout");
        flyout.children.push(picker);
        e.children.push(row);
        e.children.push(property("DropDownButton.Flyout", flyout));
    }

    /// SPEC §9 Trust: only `http`, `https` and `mailto` open; any other literal is kept inert.
    fn href(&mut self, e: &mut Element, node: &Node) {
        match node.props.get("href") {
            Some(Value::String(s)) => match safe_absolute_url(s) {
                Some(url) => e.attr("NavigateUri", url),
                None => self.marker(e, node, "href", "Href"),
            },
            Some(Value::Bind { bind, not: false }) => {
                if let Some(p) = self.helper(node, bind) {
                    e.attr(
                        "NavigateUri",
                        format!("{{x:Bind weft:Weft.Link({p}), Mode=OneWay}}"),
                    );
                }
            }
            Some(_) => self.refuse(node, "`href` has no WinUI form here"),
            None => {}
        }
    }

    /// An image address: a literal one only when SPEC §9 Trust allows it, a bound one through
    /// `Weft.Image`, which applies the same rule when the app runs.
    fn source(&mut self, e: &mut Element, node: &Node, prop: &str, bindable: bool) {
        match node.props.get(prop) {
            Some(Value::String(s)) => match safe_url(s) {
                Some(url) => e.attr("Source", literal(&url)),
                None => self.marker(e, node, prop, "Src"),
            },
            Some(Value::Bind { bind, not: false }) if bindable => {
                if let Some(p) = self.helper(node, bind) {
                    e.attr(
                        "Source",
                        format!("{{x:Bind weft:Weft.Image({p}), Mode=OneWay}}"),
                    );
                }
            }
            Some(_) => self.refuse(node, format!("`{prop}` has no WinUI form here")),
            None => {}
        }
    }

    /// A path a `Weft` function reads as text.
    fn helper(&mut self, node: &Node, bind: &str) -> Option<String> {
        let at = path_of(node);
        self.data
            .read(bind, Read::Like(Leaf::Text), &at, &mut self.problems)
    }

    fn alert(&mut self, e: &mut Element, node: &Node) {
        e.attr("IsOpen", "True");
        e.attr("IsClosable", "False");
        let severity = match self.word(node, "tone") {
            Some("success") => Some("Success"),
            Some("warning") => Some("Warning"),
            Some("danger") => Some("Error"),
            Some(_) => Some("Informational"),
            None => None,
        };
        if let Some(s) = severity {
            e.attr("Severity", s);
        }
        if !node.children.iter().any(|c| matches!(c, Child::Node(_))) {
            return self.content(e, node, "Message");
        }
        let body = self.panel(&node.children);
        e.children.push(body);
    }
}

/// A property element, such as `<DropDownButton.Flyout>`, holding `value`.
fn property(name: &'static str, value: Element) -> Element {
    let mut property = Element::new(name);
    property.children.push(value);
    property
}
