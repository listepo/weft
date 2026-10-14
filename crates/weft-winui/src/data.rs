//! The data a page reads. x:Bind is compiled against the view model, so every path a property
//! reads directly needs one C# type, and its name is the view-model member, verbatim (design,
//! open question 2). A read through a `Weft` helper function takes any value and fixes no type.

use indexmap::IndexMap;

use crate::Unsupported;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Leaf {
    Text,
    Flag,
}

impl Leaf {
    fn name(self) -> &'static str {
        match self {
            Leaf::Text => "text",
            Leaf::Flag => "a flag",
        }
    }
}

/// Members every generated view model has: a data name equal to one would collide with it.
const MEMBERS: &[&str] = &["PropertyChanged", "Perform", "Performed", "OnPerformed"];

#[derive(Default)]
pub struct Data {
    /// Each directly read path, with its type and the element that first read it.
    typed: IndexMap<String, (Leaf, String)>,
    /// Every path read, with the element that first read it.
    read: IndexMap<String, String>,
}

impl Data {
    /// The x:Bind path of `bind`, read at element `at` as `leaf` (`None`: through a helper).
    pub fn read(
        &mut self,
        bind: &str,
        leaf: Option<Leaf>,
        at: &str,
        problems: &mut Vec<Unsupported>,
    ) -> Option<String> {
        let Some(path) = bind.strip_prefix("$.") else {
            problems.push(Unsupported::new(
                at,
                format!("`{bind}` reads a loop variable, and `<each>` is not generated yet"),
            ));
            return None;
        };
        let segments: Vec<&str> = path.split('.').collect();
        let problem = if segments
            .iter()
            .any(|s| s.starts_with(|c: char| c.is_ascii_digit()))
        {
            Some("an array index is not generated yet".to_owned())
        } else if let Some(s) = segments
            .iter()
            .find(|s| s.starts_with("Weft") || **s == "PropertyChanged")
        {
            Some(format!("`{s}` is the name of a generated member"))
        } else {
            MEMBERS
                .contains(&segments[0])
                .then(|| format!("`{}` is the name of a generated member", segments[0]))
        };
        if let Some(message) = problem {
            problems.push(Unsupported::new(bind, message));
            return None;
        }
        self.read
            .entry(path.to_owned())
            .or_insert_with(|| at.to_owned());
        if let Some(leaf) = leaf {
            match self.typed.get(path) {
                Some((other, first)) if *other != leaf => problems.push(Unsupported::new(
                    at,
                    format!(
                        "`{bind}` is read as {} here and as {} at {first}",
                        leaf.name(),
                        other.name()
                    ),
                )),
                Some(_) => {}
                None => {
                    self.typed.insert(path.to_owned(), (leaf, at.to_owned()));
                }
            }
        }
        Some(format!("ViewModel.{path}"))
    }

    /// After every read: a typed path must not also hold members, and a root name must not be a
    /// command the view model generates.
    pub fn check(&self, commands: &[String], problems: &mut Vec<Unsupported>) {
        for (path, (leaf, at)) in &self.typed {
            let prefix = format!("{path}.");
            if let Some((_, other)) = self.read.iter().find(|(p, _)| p.starts_with(&prefix)) {
                problems.push(Unsupported::new(
                    at.clone(),
                    format!(
                        "`$.{path}` is read as {} here and as an object at {other}",
                        leaf.name()
                    ),
                ));
            }
        }
        for (path, at) in &self.read {
            let root = path.split('.').next().unwrap_or_default();
            if commands.iter().any(|c| c == root) {
                problems.push(Unsupported::new(
                    at.clone(),
                    format!("`$.{root}` is the name of a generated command"),
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(data: &mut Data, bind: &str, leaf: Option<Leaf>, at: &str) -> Vec<Unsupported> {
        let mut problems = vec![];
        data.read(bind, leaf, at, &mut problems);
        problems
    }

    #[test]
    fn a_path_has_one_type_and_helpers_fix_none() {
        let mut data = Data::default();
        assert!(read(&mut data, "$.ok", Some(Leaf::Flag), "checkbox#a").is_empty());
        assert!(read(&mut data, "$.ok", None, "button#b").is_empty());
        let problems = read(&mut data, "$.ok", Some(Leaf::Text), "text#c");
        assert_eq!(problems[0].path, "text#c");
        assert!(problems[0].message.contains("checkbox#a"), "{problems:?}");
    }

    #[test]
    fn a_value_is_not_also_an_object_or_a_command() {
        let mut data = Data::default();
        read(&mut data, "$.user", Some(Leaf::Text), "text#a");
        read(&mut data, "$.user.name", None, "text#b");
        read(&mut data, "$.save_doneCommand", None, "text#c");
        let mut problems = vec![];
        data.check(&["save_doneCommand".to_owned()], &mut problems);
        assert_eq!(problems.len(), 2, "{problems:?}");
        assert!(problems[0].message.contains("text#b"));
        assert_eq!(problems[1].path, "text#c");
    }

    #[test]
    fn generated_members_indexes_and_loop_variables_are_refused() {
        for bind in [
            "$.Perform",
            "$.a.PropertyChanged",
            "$.a.WeftRoot",
            "$.items.0.name",
            "$item.name",
        ] {
            let problems = read(&mut Data::default(), bind, None, "text#t");
            assert_eq!(problems.len(), 1, "{bind}");
        }
        assert!(read(&mut Data::default(), "$.a.Perform", None, "text#t").is_empty());
    }
}
