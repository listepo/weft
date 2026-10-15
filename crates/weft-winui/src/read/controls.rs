//! The tables of the WinUI reader: which control is which kind, which attribute is which prop, and
//! which loss an attribute that maps to nothing is. They mirror the mapping of
//! `docs/winui-design.md` and of the generator.

use roxmltree::Node as Xml;
use weft_import::LossKind;

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
        "button link radio segment option item",
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
    ("alert", "IsOpen"),
    ("alert", "IsClosable"),
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

pub fn expected(kind: &str, name: &str) -> bool {
    EXPECTED
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
    if el.tag_name().namespace() != Some(PRESENTATION) {
        return None;
    }
    let at = |name: &str| el.attribute(name);
    let kind = match el.tag_name().name() {
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
        // An InfoBar takes one content element; the generator's bare panel holds an alert's
        // several children, while a `stack` of the source always carries its AutomationId.
        "StackPanel"
            if el.attributes().next().is_none()
                && el
                    .parent_element()
                    .is_some_and(|p| p.tag_name().name() == "InfoBar") =>
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
