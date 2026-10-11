//! The expression tree of a SwiftUI view → Weft elements. Each SwiftUI form `generate` prints is
//! recognised exactly; other views and modifiers map to the nearest kind or become losses.

mod collections;
mod controls;
mod entry;
mod helpers;
mod layouts;
mod loops;
mod overlays;
mod types;
mod values;
mod views;
mod widgets;

pub use entry::read;
pub use helpers::element_path;

#[cfg(test)]
mod tests {
    use super::helpers::{is_action, loop_name};

    #[test]
    fn names_come_back() {
        assert_eq!(loop_name("self_"), "self");
        assert_eq!(loop_name("model_"), "model");
        assert_eq!(loop_name("todo"), "todo");
        assert_eq!(loop_name("$0"), "item");
        assert!(is_action("auth.submit"));
        assert!(!is_action("Auth"));
    }
}
