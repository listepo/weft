//! The data a page reads, as a tree of paths. x:Bind is compiled against the view model, so every
//! path a property reads directly needs one C# type, and its name is the view-model member,
//! verbatim (design, open question 2). A read through a `Weft` helper or a marker takes any value
//! and only suggests a type.

use indexmap::IndexMap;

use crate::Unsupported;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Leaf {
    Text,
    Flag,
    Number,
}

impl Leaf {
    fn name(self) -> &'static str {
        match self {
            Leaf::Text => "text",
            Leaf::Flag => "a flag",
            Leaf::Number => "a number",
        }
    }
}

/// How a prop reads a path: as a property of that type, or loosely, through a helper or a marker.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Read {
    Is(Leaf),
    Like(Leaf),
}

/// What a path holds once every read is known.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ty {
    Leaf(Leaf),
    Object,
    /// A list; the index of the shape of one item.
    Array(usize),
}

/// Members of every generated class, and of `object`, which a property must not hide.
const MEMBERS: &[&str] = &[
    "PropertyChanged",
    "Equals",
    "GetHashCode",
    "GetType",
    "ToString",
    "MemberwiseClone",
    "ReferenceEquals",
    "Finalize",
];
/// Members only the view model has.
const ROOT_MEMBERS: &[&str] = &["Perform", "Performed", "OnPerformed"];

#[derive(Default)]
pub struct Shape {
    pub fields: IndexMap<String, usize>,
    pub item: Option<usize>,
    /// The Weft path, such as `$.todos.0.title`, for messages.
    pub path: String,
    /// The direct type and the element that fixed it.
    is: Option<(Leaf, String)>,
    like: Option<Leaf>,
    /// The element that first read the path or a path below it.
    at: String,
}

/// Every data path of a screen; shape 0 is the view model.
pub struct Data {
    pub shapes: Vec<Shape>,
}

impl Default for Data {
    fn default() -> Self {
        let root = Shape {
            path: "$".to_owned(),
            ..Shape::default()
        };
        Data { shapes: vec![root] }
    }
}

impl Data {
    /// The x:Bind path of `bind`, such as `ViewModel.items[0].name`, read at element `at`.
    pub fn read(
        &mut self,
        bind: &str,
        read: Read,
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
        if let Some(message) = refused(&segments) {
            problems.push(Unsupported::new(bind, message));
            return None;
        }
        let mut shape = 0;
        let mut xbind = "ViewModel".to_owned();
        for segment in segments {
            let index = segment.starts_with(|c: char| c.is_ascii_digit());
            let next = if index {
                xbind.push_str(&format!("[{segment}]"));
                self.shapes[shape].item
            } else {
                xbind.push('.');
                xbind.push_str(segment);
                self.shapes[shape].fields.get(segment).copied()
            };
            shape = next.unwrap_or_else(|| self.add(shape, segment, index));
            if self.shapes[shape].at.is_empty() {
                self.shapes[shape].at = at.to_owned();
            }
        }
        let s = &mut self.shapes[shape];
        match read {
            Read::Is(leaf) => match &s.is {
                Some((other, first)) if *other != leaf => problems.push(Unsupported::new(
                    at,
                    format!(
                        "`{bind}` is read as {} here and as {} at {first}",
                        leaf.name(),
                        other.name()
                    ),
                )),
                Some(_) => {}
                None => s.is = Some((leaf, at.to_owned())),
            },
            // Text wins: any value can be shown as text, and the helpers test it as it is.
            Read::Like(leaf) if s.like != Some(Leaf::Text) => s.like = Some(leaf),
            Read::Like(_) => {}
        }
        Some(xbind)
    }

    fn add(&mut self, parent: usize, segment: &str, index: bool) -> usize {
        let path = format!("{}.{segment}", self.shapes[parent].path);
        self.shapes.push(Shape {
            path,
            ..Shape::default()
        });
        let next = self.shapes.len() - 1;
        if index {
            self.shapes[parent].item = Some(next);
        } else {
            self.shapes[parent].fields.insert(segment.to_owned(), next);
        }
        next
    }

    pub fn ty(&self, at: usize) -> Ty {
        let s = &self.shapes[at];
        match (s.item, s.is.as_ref().map(|i| i.0), s.like) {
            (Some(item), _, _) => Ty::Array(item),
            _ if !s.fields.is_empty() || at == 0 => Ty::Object,
            (_, Some(leaf), _) | (_, None, Some(leaf)) => Ty::Leaf(leaf),
            (_, None, None) => Ty::Leaf(Leaf::Text),
        }
    }

    /// After every read: each path holds one thing, and no root name is a generated command.
    pub fn check(&self, commands: &[String], problems: &mut Vec<Unsupported>) {
        for s in &self.shapes {
            let below = s.fields.values().chain(&s.item).next();
            let what = if s.item.is_some() {
                "a list"
            } else {
                "an object"
            };
            if let (Some((leaf, at)), Some(&b)) = (&s.is, below) {
                problems.push(Unsupported::new(
                    at.clone(),
                    format!(
                        "`{}` is read as {} here and as {what} at {}",
                        s.path,
                        leaf.name(),
                        self.shapes[b].at
                    ),
                ));
            } else if let (Some(item), false) = (s.item, s.fields.is_empty()) {
                problems.push(Unsupported::new(
                    self.shapes[item].at.clone(),
                    format!("`{}` is read both as a list and as an object", s.path),
                ));
            } else if s.item.is_some_and(|item| self.shapes[item].item.is_some()) {
                problems.push(Unsupported::new(
                    s.at.clone(),
                    format!("`{}` is a list of lists, which is not generated", s.path),
                ));
            }
        }
        for (name, &at) in &self.shapes[0].fields {
            if commands.iter().any(|c| c == name) {
                problems.push(Unsupported::new(
                    self.shapes[at].at.clone(),
                    format!("`$.{name}` is the name of a generated command"),
                ));
            }
        }
    }
}

/// Why a path cannot be a member chain of the generated classes, if it cannot.
fn refused(segments: &[&str]) -> Option<String> {
    if segments[0].starts_with(|c: char| c.is_ascii_digit()) {
        return Some("the view model is an object, not a list".to_owned());
    }
    let member = |(i, s): (usize, &&str)| {
        s.starts_with("Weft") || MEMBERS.contains(s) || (i == 0 && ROOT_MEMBERS.contains(s))
    };
    if let Some((_, s)) = segments.iter().enumerate().find(|p| member(*p)) {
        return Some(format!("`{s}` is the name of a generated member"));
    }
    // The XAML compiler copies a path segment into C# as it is, so `@class` in the view model
    // is unreachable: x:Bind to `class` or `event` gives a page that does not compile (T105.7).
    let s = segments
        .iter()
        .find(|s| KEYWORDS.split_whitespace().any(|k| k == **s))?;
    Some(format!(
        "`{s}` is a C# keyword, which an x:Bind path cannot name"
    ))
}

/// C#'s reserved keywords.
const KEYWORDS: &str = "abstract as base bool break byte case catch char checked class const continue decimal \
     default delegate do double else enum event explicit extern false finally fixed float for \
     foreach goto if implicit in int interface internal is lock long namespace new null object \
     operator out override params private protected public readonly ref return sbyte sealed \
     short sizeof stackalloc static string struct switch this throw true try typeof uint ulong \
     unchecked unsafe ushort using virtual void volatile while";

#[cfg(test)]
mod tests {
    use super::*;

    fn read(data: &mut Data, bind: &str, read: Read, at: &str) -> Vec<Unsupported> {
        let mut problems = vec![];
        data.read(bind, read, at, &mut problems);
        problems
    }

    #[test]
    fn a_path_has_one_type_and_helpers_only_suggest_one() {
        let mut data = Data::default();
        assert!(read(&mut data, "$.ok", Read::Is(Leaf::Flag), "checkbox#a").is_empty());
        assert!(read(&mut data, "$.ok", Read::Like(Leaf::Text), "button#b").is_empty());
        assert_eq!(data.ty(1), Ty::Leaf(Leaf::Flag));
        let problems = read(&mut data, "$.ok", Read::Is(Leaf::Text), "text#c");
        assert_eq!(problems[0].path, "text#c");
        assert!(problems[0].message.contains("checkbox#a"), "{problems:?}");
        read(&mut data, "$.note", Read::Like(Leaf::Flag), "text#d");
        assert_eq!(data.ty(2), Ty::Leaf(Leaf::Flag));
        read(&mut data, "$.note", Read::Like(Leaf::Text), "text#e");
        read(&mut data, "$.note", Read::Like(Leaf::Flag), "text#f");
        assert_eq!(data.ty(2), Ty::Leaf(Leaf::Text));
    }

    #[test]
    fn indexes_are_indexers_and_make_lists() {
        let mut data = Data::default();
        let path = data.read(
            "$.todos.0.title",
            Read::Is(Leaf::Text),
            "text#a",
            &mut vec![],
        );
        assert_eq!(path.as_deref(), Some("ViewModel.todos[0].title"));
        assert_eq!(data.ty(1), Ty::Array(2));
        assert_eq!(data.ty(2), Ty::Object);
        assert_eq!(data.shapes[3].path, "$.todos.0.title");
    }

    #[test]
    fn a_path_holds_one_thing_and_no_command() {
        let mut data = Data::default();
        read(&mut data, "$.user", Read::Is(Leaf::Text), "text#a");
        read(&mut data, "$.user.name", Read::Like(Leaf::Text), "text#b");
        read(&mut data, "$.both.0", Read::Is(Leaf::Text), "text#c");
        read(&mut data, "$.both.name", Read::Is(Leaf::Text), "text#d");
        read(&mut data, "$.grid.0.1", Read::Is(Leaf::Text), "text#e");
        read(
            &mut data,
            "$.save_doneCommand",
            Read::Like(Leaf::Text),
            "text#f",
        );
        let mut problems = vec![];
        data.check(&["save_doneCommand".to_owned()], &mut problems);
        let paths: Vec<_> = problems.iter().map(|p| p.path.as_str()).collect();
        assert_eq!(
            paths,
            ["text#a", "text#c", "text#e", "text#f"],
            "{problems:?}"
        );
        assert!(problems[0].message.contains("text#b"));
    }

    #[test]
    fn generated_members_and_loop_variables_are_refused() {
        for bind in [
            "$.Perform",
            "$.a.PropertyChanged",
            "$.a.WeftRoot",
            "$.a.ToString",
            "$.Equals",
            "$.0.name",
            "$item.name",
        ] {
            let problems = read(&mut Data::default(), bind, Read::Like(Leaf::Text), "text#t");
            assert_eq!(problems.len(), 1, "{bind}");
        }
        assert!(
            read(
                &mut Data::default(),
                "$.a.Perform",
                Read::Like(Leaf::Text),
                "t"
            )
            .is_empty()
        );
    }

    #[test]
    fn keywords_are_refused_and_contextual_ones_are_not() {
        for bind in ["$.class", "$.user.event", "$.todos.0.new"] {
            let problems = read(&mut Data::default(), bind, Read::Is(Leaf::Text), "text#t");
            assert_eq!(problems.len(), 1, "{bind}");
            assert_eq!(problems[0].path, bind);
            assert!(problems[0].message.contains("C# keyword"), "{problems:?}");
        }
        for bind in ["$.value", "$.var", "$.title"] {
            assert!(read(&mut Data::default(), bind, Read::Is(Leaf::Text), "t").is_empty());
        }
    }
}
