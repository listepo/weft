//! The tables of the WinUI reader: which control is which kind, which attribute is which prop, and
//! which loss an attribute that maps to nothing is. They mirror the mapping of
//! `docs/winui-design.md` and of the generator.

use roxmltree::Node as Xml;
use weft_import::LossKind;

use crate::generate::containers::CARD;

pub const PRESENTATION: &str = "http://schemas.microsoft.com/winfx/2006/xaml/presentation";
pub const XAML: &str = "http://schemas.microsoft.com/winfx/2006/xaml";

/// How an attribute value becomes the text of a prop, before the catalog types it.
pub enum Conv {
    Same,
    /// A XAML `Boolean` read with the opposite sense (`IsEnabled` → `disabled`).
    Not,
    Words(&'static [(&'static str, &'static str)]),
}

const LEVELS: &[(&str, &str)] = &[
    ("Level1", "1"),
    ("Level2", "2"),
    ("Level3", "3"),
    ("Level4", "4"),
    ("Level5", "5"),
    ("Level6", "6"),
];
/// The type ramp the generator gives each heading level; 4–6 share one style.
const RAMP: &[(&str, &str)] = &[
    ("{StaticResource TitleLargeTextBlockStyle}", "1"),
    ("{StaticResource TitleTextBlockStyle}", "2"),
    ("{StaticResource SubtitleTextBlockStyle}", "3"),
    ("{StaticResource BodyStrongTextBlockStyle}", "4"),
];

/// `(kinds, attribute, prop, conversion)`: `*` is every kind, an empty prop the element's text.
pub const ATTRS: &[(&str, &str, &str, Conv)] = &[
    ("*", "AutomationProperties.Name", "label", Conv::Same),
    ("*", "IsEnabled", "disabled", Conv::Not),
    (
        "*",
        "Visibility",
        "hidden",
        Conv::Words(&[("Collapsed", "true"), ("Visible", "false")]),
    ),
    ("grid", "Columns", "columns", Conv::Same),
    (
        "stack",
        "Orientation",
        "direction",
        Conv::Words(&[("Horizontal", "row"), ("Vertical", "column")]),
    ),
    (
        "text",
        "Foreground",
        "tone",
        Conv::Words(&[
            ("{ThemeResource TextFillColorSecondaryBrush}", "muted"),
            ("{ThemeResource SystemFillColorSuccessBrush}", "success"),
            ("{ThemeResource SystemFillColorCautionBrush}", "warning"),
            ("{ThemeResource SystemFillColorCriticalBrush}", "danger"),
        ]),
    ),
    ("text heading", "Text", "", Conv::Same),
    (
        "heading",
        "AutomationProperties.HeadingLevel",
        "level",
        Conv::Words(LEVELS),
    ),
    ("heading", "Style", "level", Conv::Words(RAMP)),
    (
        "button link radio segment option item column cell",
        "Content",
        "",
        Conv::Same,
    ),
    (
        "button",
        "Style",
        "variant",
        Conv::Words(&[("{StaticResource AccentButtonStyle}", "primary")]),
    ),
    ("link", "NavigateUri", "href", Conv::Same),
    ("field", "Text", "value", Conv::Same),
    ("field", "Password", "value", Conv::Same),
    ("field", "PlaceholderText", "placeholder", Conv::Same),
    (
        "field",
        "InputScope",
        "type",
        Conv::Words(&[
            ("EmailSmtpAddress", "email"),
            ("Number", "number"),
            ("Search", "search"),
        ]),
    ),
    (
        "field",
        "AcceptsReturn",
        "type",
        Conv::Words(&[("True", "multiline")]),
    ),
    (
        "field",
        "AutomationProperties.IsRequiredForForm",
        "required",
        Conv::Same,
    ),
    ("checkbox menu", "Content", "label", Conv::Same),
    ("checkbox", "IsChecked", "checked", Conv::Same),
    ("switch", "IsOn", "checked", Conv::Same),
    (
        "field switch radio-group select combobox slider stepper date-picker tab",
        "Header",
        "label",
        Conv::Same,
    ),
    ("combobox", "PlaceholderText", "placeholder", Conv::Same),
    ("slider stepper", "Minimum", "min", Conv::Same),
    ("date-picker", "MinDate", "min", Conv::Same),
    ("date-picker", "MaxDate", "max", Conv::Same),
    ("slider stepper", "Maximum", "max", Conv::Same),
    ("slider stepper", "Value", "value", Conv::Same),
    ("slider", "StepFrequency", "step", Conv::Same),
    ("stepper", "SmallChange", "step", Conv::Same),
    ("radio segment option", "Tag", "value", Conv::Same),
    ("option", "ToolTipService.ToolTip", "", Conv::Same),
    ("image", "Source", "src", Conv::Same),
    ("model", "Source", "fallback", Conv::Same),
    (
        "alert",
        "Severity",
        "tone",
        Conv::Words(&[
            ("Informational", "info"),
            ("Success", "success"),
            ("Warning", "warning"),
            ("Error", "danger"),
        ]),
    ),
    ("alert menu-item", "Message", "", Conv::Same),
    ("segment menu-item", "Text", "", Conv::Same),
];

/// Attributes the mapping writes beside the ones it reads, or that only decide the kind.
const EXPECTED: &[(&str, &str)] = &[
    ("screen form section", "AutomationProperties.LandmarkType"),
    // Only a vertical panel is read as the screen.
    ("screen", "Orientation"),
    ("section", "AutomationProperties.LocalizedLandmarkType"),
    ("text heading", "TextWrapping"),
    ("field", "TextWrapping"),
    ("select combobox", "IsEditable"),
    ("select", "SelectedValuePath"),
    ("stepper", "SpinButtonPlacementMode"),
    ("date-picker", "Orientation"),
    ("list", "SelectionMode"),
    ("tabs", "IsAddTabButtonVisible"),
    ("tab", "IsClosable"),
    // A tab's `Tag` repeats its id, for `Choice.Value`.
    ("tab", "Tag"),
    // A table's panels line up column headers and cells by track.
    ("column cell", "Grid.Column"),
    ("alert", "IsOpen"),
    ("alert", "IsClosable"),
];

/// The kinds panels and buttons carry no semantics for, by `LocalizedControlType`: the table's,
/// and the item a list's template draws.
const TABLE: &[(&str, &str, &str)] = &[
    ("StackPanel", "table", "table"),
    ("Grid", "row", "row"),
    ("ContentControl", "cell", "cell"),
    ("Button", "column header", "column"),
    ("ContentControl", "list item", "item"),
];

const LAYOUT: &[&str] = &[
    "Margin",
    "Padding",
    "Width",
    "Height",
    "MinWidth",
    "MinHeight",
    "MaxWidth",
    "MaxHeight",
    "HorizontalAlignment",
    "VerticalAlignment",
    "HorizontalContentAlignment",
    "VerticalContentAlignment",
    "Spacing",
    "RowSpacing",
    "ColumnSpacing",
    "CornerRadius",
    "Grid.Row",
    "Grid.Column",
    "Grid.RowSpan",
    "Grid.ColumnSpan",
];
/// What a literal colour, brush or font size says, which Weft states only through tokens.
const LOOK: &[&str] = &[
    "Foreground",
    "Background",
    "BorderBrush",
    "BorderThickness",
    "FontSize",
    "FontWeight",
    "FontFamily",
    "FontStyle",
    "Opacity",
];
/// Events a page wires to code-behind handlers, which the XAML alone cannot follow.
const EVENTS: &[&str] = &[
    "Click",
    "Tapped",
    "DoubleTapped",
    "Checked",
    "Unchecked",
    "Toggled",
    "TextChanged",
    "PasswordChanged",
    "SelectionChanged",
    "ValueChanged",
    "ItemClick",
    "KeyDown",
    "Loaded",
    "PointerPressed",
    "DateChanged",
    "TimeChanged",
    "ColorChanged",
    "Closed",
    "Command",
];

/// `(kinds, attribute, prop)`: attributes that hold a `Weft.*` token resource.
const TOKENS: &[(&str, &str, &str)] = &[
    ("stack grid", "Spacing", "gap"),
    ("grid", "MinColumnWidth", "min-column-width"),
    ("stack grid", "RowSpacing", "gap"),
    ("stack grid", "ColumnSpacing", "gap"),
    ("stack grid", "Padding", "padding"),
    ("stack grid", "MaxWidth", "max-width"),
    ("stack grid", "Background", "material"),
];
/// A native event or `Command` bound to the view model, and the Weft events it may be, in order.
const BOUND_EVENTS: &[(&str, &[&str])] = &[
    ("Command", &["press"]),
    ("Click", &["press", "change"]),
    ("ItemClick", &["press"]),
    ("Toggled", &["change"]),
    ("TextChanged", &["change"]),
    ("PasswordChanged", &["change"]),
    ("SelectionChanged", &["change"]),
    ("ValueChanged", &["change"]),
    ("DateChanged", &["change"]),
    ("TimeChanged", &["change"]),
    ("ColorChanged", &["change"]),
    ("Closed", &["close"]),
];

pub fn token_of(kind: &str, name: &str) -> Option<&'static str> {
    TOKENS
        .iter()
        .find(|(kinds, attr, _)| *attr == name && listed(kinds, kind))
        .map(|t| t.2)
}

pub fn events_of(name: &str) -> Option<&'static [&'static str]> {
    BOUND_EVENTS.iter().find(|e| e.0 == name).map(|e| e.1)
}

fn listed(kinds: &str, kind: &str) -> bool {
    kinds == "*" || kinds.split(' ').any(|k| k == kind)
}

/// The entry that reads attribute `name` on an element of `kind`.
pub fn attr_of(
    kind: &str,
    name: &str,
) -> Option<&'static (&'static str, &'static str, &'static str, Conv)> {
    ATTRS
        .iter()
        .find(|(kinds, attr, ..)| *attr == name && listed(kinds, kind))
}

pub fn expected(kind: &str, name: &str, value: &str) -> bool {
    let card = kind == "section" && CARD.contains(&(name, value));
    let table = name == "AutomationProperties.LocalizedControlType"
        && TABLE.iter().any(|t| (t.1, t.2) == (value, kind));
    card || table
        || EXPECTED
            .iter()
            .any(|(kinds, attr)| *attr == name && listed(kinds, kind))
}

/// The loss for an attribute nothing reads: a binding, a token, an event, layout or look.
pub fn loss_of(name: &str, value: &str) -> (LossKind, &'static str) {
    if EVENTS.contains(&name) {
        return (LossKind::Actions, "an event handler");
    }
    if value.starts_with('{') && !value.starts_with("{}") {
        if value.starts_with("{x:Bind") || value.starts_with("{Binding") {
            return (LossKind::Bindings, "a binding");
        }
        if value.contains(" Weft.") {
            return (LossKind::Tokens, "a token reference");
        }
        return (LossKind::Props, "a markup extension");
    }
    if LAYOUT.contains(&name) {
        (LossKind::Layout, "a layout value")
    } else if LOOK.contains(&name) {
        (LossKind::Tokens, "a literal look")
    } else {
        (LossKind::Props, "an attribute with no Weft prop")
    }
}

/// The kind of a control, and the structure loss of a panel read as the nearest kind.
pub fn kind_of(el: Xml<'_, '_>) -> Option<(&'static str, Option<&'static str>)> {
    let helper = el
        .tag_name()
        .namespace()
        .is_some_and(|n| n.starts_with("using:"));
    // The generator's reflowing grid is the one control of its own helpers.
    if helper && el.tag_name().name() == "ColumnsPanel" {
        return Some(("grid", None));
    }
    if el.tag_name().namespace() != Some(PRESENTATION) {
        return None;
    }
    let at = |name: &str| el.attribute(name);
    let tag = el.tag_name().name();
    if let Some(&(_, _, kind)) = TABLE
        .iter()
        .find(|t| t.0 == tag && at("AutomationProperties.LocalizedControlType") == Some(t.1))
    {
        return Some((kind, None));
    }
    let kind = match tag {
        // Only the generator's date and time row holds a choice on a panel.
        "StackPanel" if el.attributes().any(|a| a.name() == "Choice.Value") => "date-picker",
        "StackPanel" => match at("AutomationProperties.LandmarkType") {
            Some("Form") => "form",
            Some("Custom")
                if at("AutomationProperties.LocalizedLandmarkType") == Some("region") =>
            {
                "section"
            }
            _ => "stack",
        },
        "Canvas" | "RelativePanel" | "VariableSizedWrapGrid" => {
            return Some((
                "stack",
                Some("a panel Weft has no layout for is read as a stack"),
            ));
        }
        "Grid" => "grid",
        "TextBlock" => {
            let ramp = at("Style").is_some_and(|s| RAMP[..3].iter().any(|(r, _)| *r == s));
            if ramp || at("AutomationProperties.HeadingLevel").is_some() {
                "heading"
            } else {
                "text"
            }
        }
        "Button" => "button",
        "HyperlinkButton" => "link",
        "TextBox" | "PasswordBox" => "field",
        "CheckBox" => "checkbox",
        "ToggleSwitch" => "switch",
        "RadioButtons" => "radio-group",
        "RadioButton" => "radio",
        "SelectorBar" => "segmented-control",
        "SelectorBarItem" => "segment",
        "ComboBox" if at("IsEditable") == Some("True") => "combobox",
        "ComboBox" => "select",
        "ComboBoxItem" => "option",
        "Slider" => "slider",
        "NumberBox" => "stepper",
        "CalendarDatePicker" | "DatePicker" | "TimePicker" => "date-picker",
        "ColorPicker" => "color-picker",
        "ListView" => "list",
        "ListViewItem" => "item",
        "TabView" => "tabs",
        "TabViewItem" => "tab",
        "ContentDialog" => "dialog",
        "InfoBar" => "alert",
        // The generator's colour picker is a swatch button that opens a ColorPicker.
        "DropDownButton"
            if el
                .descendants()
                .any(|d| d.has_tag_name((PRESENTATION, "ColorPicker"))) =>
        {
            "color-picker"
        }
        "DropDownButton" => "menu",
        "MenuFlyoutItem" => "menu-item",
        // A model shows its fallback picture and keeps its scene in markers; an image keeps a
        // `src` it cannot load in the marker alone.
        "Image"
            if el.attributes().any(|a| {
                a.name() == "Props.Usdz" || (a.name() == "Props.Src" && at("Source").is_some())
            }) =>
        {
            "model"
        }
        "Image" => "image",
        _ => return None,
    };
    Some((kind, None))
}

/// Elements that only hold others: read through, with the loss their own meaning is.
pub fn wrapper(el: Xml<'_, '_>) -> Option<Option<(LossKind, &'static str)>> {
    if el.tag_name().namespace() != Some(PRESENTATION) {
        return None;
    }
    Some(match el.tag_name().name() {
        "DataTemplate" | "MenuFlyout" => None,
        // A content control takes one element; the generator's bare panel holds the several of
        // an alert, dialog, item (a list's own or its template's), tab or cell, while a `stack`
        // of the source carries its AutomationId.
        "StackPanel"
            if el.attributes().next().is_none()
                && el.parent_element().is_some_and(|p| {
                    let holder = ["InfoBar", "ContentDialog", "ListViewItem", "TabViewItem"];
                    holder.contains(&p.tag_name().name())
                        || kind_of(p).is_some_and(|(kind, _)| matches!(kind, "cell" | "item"))
                }) =>
        {
            None
        }
        // The header row of a table, which holds its columns.
        "Grid"
            if el.attributes().next().is_none()
                && el
                    .parent_element()
                    .and_then(kind_of)
                    .is_some_and(|(kind, _)| kind == "table") =>
        {
            None
        }
        "ScrollViewer" | "Border" | "Viewbox" => Some((
            LossKind::Layout,
            "a scrolling or framing wrapper; its content is read in its place",
        )),
        "ItemsRepeater" => Some((
            LossKind::Repetition,
            "an ItemsRepeater is read once, from its template",
        )),
        _ => return None,
    })
}

/// `MinColumnWidth` → `min-column-width`: the `weft:Props` marker of a prop.
pub fn marker_prop(name: &str) -> String {
    let mut out = String::new();
    for (i, c) in name.chars().enumerate() {
        if c.is_ascii_uppercase() && i > 0 {
            out.push('-');
        }
        out.push(c.to_ascii_lowercase());
    }
    out
}
