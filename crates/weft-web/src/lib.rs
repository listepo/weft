//! Weft and the web, both ways. Importers read HTML and JSX source into a document with a loss
//! table; generators write HTML/CSS, React and SolidJS from a document.

pub mod dom;
pub mod html;
mod import;
mod js;
pub mod jsx;
pub mod provenance;
pub mod tree;

pub use dom::{IMPLICIT_ROLES, INPUT_ROLES, MAX_HTML_LENGTH, from_dom, instance_id};
pub use html::{HtmlOptions, Invalid, to_html};
pub use import::{ImportOptions, import_html};
pub use jsx::{BadComponentName, Framework, JsxOptions, to_jsx};
pub use tree::{Dom, HNode, parse_html};
