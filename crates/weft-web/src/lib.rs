//! Weft and the web, both ways. Importers read HTML and JSX source into a document with a loss
//! table; generators write HTML/CSS, React and SolidJS from a document.

pub mod dom;
mod fill;
mod from_jsx;
pub mod html;
mod import;
mod js;
pub mod jsx;
pub mod provenance;
mod tilt;
pub mod tree;

pub use dom::{IMPLICIT_ROLES, INPUT_ROLES, MAX_HTML_LENGTH, from_dom, instance_id};
pub use from_jsx::{MAX_JSX_LENGTH, import_jsx};
pub use html::{BASE_CSS, HtmlOptions, Invalid, to_html, to_html_with_data, tokens_css};
pub use import::{ImportOptions, import_html};
pub use jsx::{Framework, JsxError, JsxOptions, to_jsx};
pub use tree::{Dom, HNode, parse_html};
