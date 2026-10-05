use std::collections::{HashMap, HashSet};

use crate::text::slug;

/// The ids an import has handed out, so that generated ids never collide with each other or with
/// the ids the source carries.
#[derive(Clone, Debug, Default)]
pub struct IdState {
    pub used: HashSet<String>,
    /// Ids the source carries that no element has taken yet: generated ids avoid them, and the
    /// element that carries one may still take it.
    pub reserved: HashSet<String>,
    /// Where the last numbered search for a stem stopped, so many equal names stay linear.
    pub counters: HashMap<String, usize>,
}

impl IdState {
    /// An id that reads as `base` plus the slug of `name`, numbered when taken or when the name
    /// gives no slug.
    pub fn fresh(&mut self, base: &str, name: &str) -> String {
        let s = slug(name);
        if !s.is_empty() {
            let id = format!("{base}-{s}");
            if !self.taken(&id) {
                self.used.insert(id.clone());
                return id;
            }
        }
        let stem = if s.is_empty() {
            base.to_owned()
        } else {
            format!("{base}-{s}")
        };
        let mut n = self
            .counters
            .get(&stem)
            .copied()
            .unwrap_or(if s.is_empty() { 1 } else { 2 });
        while self.taken(&format!("{stem}-{n}")) {
            n += 1;
        }
        self.counters.insert(stem.clone(), n + 1);
        let id = format!("{stem}-{n}");
        self.used.insert(id.clone());
        id
    }

    fn taken(&self, id: &str) -> bool {
        self.used.contains(id) || self.reserved.contains(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_named_then_numbered() {
        let mut ids = IdState::default();
        assert_eq!(ids.fresh("button", "Save"), "button-save");
        assert_eq!(ids.fresh("button", "Save"), "button-save-2");
        assert_eq!(ids.fresh("button", "Save"), "button-save-3");
        assert_eq!(ids.fresh("stack", ""), "stack-1");
        ids.used.insert("stack-2".into());
        assert_eq!(ids.fresh("stack", "!!"), "stack-3");
        ids.reserved.insert("link-home".into());
        assert_eq!(ids.fresh("link", "Home"), "link-home-2");
    }
}
