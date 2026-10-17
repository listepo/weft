//! The data a page reads, as a tree of paths. x:Bind is compiled against the view model, so every
//! path a property reads directly needs one C# type, and its name is the view-model member,
//! verbatim (design, open question 2) except for C# keywords (`member`). A read through a `Weft`
//! helper or a marker takes any value and only suggests a type.

use std::borrow::Cow;

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
    /// The item of an `<each>`, an object whatever its members.
    object: bool,
}

/// An `<each>` the generator is inside: its loop variable, the shape of its item, and whether
/// its list is below the enclosing loop's item, whose `WeftParent` then reaches that item.
struct Scope {
    name: String,
    item: usize,
    nested: bool,
}

/// Every data path of a screen; shape 0 is the view model.
pub struct Data {
    pub shapes: Vec<Shape>,
    scopes: Vec<Scope>,
}

impl Default for Data {
    fn default() -> Self {
        let root = Shape {
            path: "$".to_owned(),
            ..Shape::default()
        };
        Data {
            shapes: vec![root],
            scopes: vec![],
        }
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
        let (xbind, shape) = self.walk(bind, at, problems)?;
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

    /// The x:Bind path of the list an `<each>` repeats, and the shape of its item.
    pub fn list(
        &mut self,
        bind: &str,
        at: &str,
        problems: &mut Vec<Unsupported>,
    ) -> Option<(String, usize)> {
        let (xbind, shape) = self.walk(bind, at, problems)?;
        let item = match self.shapes[shape].item {
            Some(item) => item,
            None => self.add(shape, "0", true),
        };
        self.shapes[item].object = true;
        if self.shapes[item].at.is_empty() {
            self.shapes[item].at = at.to_owned();
        }
        Some((xbind, item))
    }

    /// Inside the `<each>` over `bind` whose item is `item`, until `leave`.
    pub fn enter(&mut self, name: &str, item: usize, bind: &str) {
        // `WeftParent` is the nearest enclosing list item of the data, which is the enclosing
        // loop's item only when this list is a member path below it.
        let nested = self.scopes.last().is_some_and(|s| {
            bind.strip_prefix('$')
                .and_then(|b| b.strip_prefix(s.name.as_str()))
                .and_then(|rest| rest.strip_prefix('.'))
                .is_some_and(|rest| !rest.split('.').any(is_index))
        });
        self.scopes.push(Scope {
            name: name.to_owned(),
            item,
            nested,
        });
    }

    pub fn leave(&mut self) {
        self.scopes.pop();
    }

    /// The view model as a binding reaches it: the page's, or a template item's `WeftRoot`.
    pub fn root(&self) -> &'static str {
        if self.scopes.is_empty() {
            "ViewModel"
        } else {
            "WeftRoot"
        }
    }

    /// The x:Bind path and the shape `bind` reaches, adding the shapes it names.
    fn walk(
        &mut self,
        bind: &str,
        at: &str,
        problems: &mut Vec<Unsupported>,
    ) -> Option<(String, usize)> {
        let (mut shape, mut xbind, path) = match self.start(bind) {
            Ok(start) => start,
            Err(message) => {
                problems.push(Unsupported::new(at, message));
                return None;
            }
        };
        let segments: Vec<&str> = path.split('.').collect();
        if let Some(message) = refused(&segments, shape == 0) {
            problems.push(Unsupported::new(bind, message));
            return None;
        }
        for segment in segments {
            let index = is_index(segment);
            let next = if index {
                xbind.push_str(&format!("[{segment}]"));
                self.shapes[shape].item
            } else {
                if !xbind.is_empty() {
                    xbind.push('.');
                }
                xbind.push_str(&member(segment));
                self.shapes[shape].fields.get(segment).copied()
            };
            shape = next.unwrap_or_else(|| self.add(shape, segment, index));
            if self.shapes[shape].at.is_empty() {
                self.shapes[shape].at = at.to_owned();
            }
        }
        Some((xbind, shape))
    }

    /// Where `bind` starts: the view model (`WeftRoot` inside a template) or a loop's item, as a
    /// shape, the x:Bind path to it, and the path after it.
    fn start<'b>(&self, bind: &'b str) -> Result<(usize, String, &'b str), String> {
        if let Some(path) = bind.strip_prefix("$.") {
            return Ok((0, self.root().to_owned(), path));
        }
        let (name, path) = bind[1..].split_once('.').unwrap_or((&bind[1..], ""));
        let Some(i) = self.scopes.iter().rposition(|s| s.name == name) else {
            return Err(format!(
                "`{bind}` reads a loop variable no `<each>` around it declares"
            ));
        };
        if self.scopes[i + 1..].iter().any(|s| !s.nested) {
            return Err(format!(
                "`{bind}` reads an enclosing loop's item from a loop over another list, which \
                 cannot reach it"
            ));
        }
        if path.is_empty() {
            return Err(format!(
                "`{bind}` reads the loop's item itself, which is not generated"
            ));
        }
        let up = vec!["WeftParent"; self.scopes.len() - 1 - i].join(".");
        Ok((self.scopes[i].item, up, path))
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
            _ if !s.fields.is_empty() || at == 0 || s.object => Ty::Object,
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

fn is_index(segment: &str) -> bool {
    segment.starts_with(|c: char| c.is_ascii_digit())
}

/// Why a path cannot be a member chain of the generated classes, if it cannot; `root` when it
/// starts at the view model rather than at a loop's item.
fn refused(segments: &[&str], root: bool) -> Option<String> {
    if is_index(segments[0]) {
        let what = if root {
            "the view model"
        } else {
            "a loop's item"
        };
        return Some(format!("{what} is an object, not a list"));
    }
    let generated = |(i, s): (usize, &&str)| {
        s.starts_with("Weft") || MEMBERS.contains(s) || (i == 0 && root && ROOT_MEMBERS.contains(s))
    };
    let (_, s) = segments.iter().enumerate().find(|p| generated(*p))?;
    Some(format!("`{s}` is the name of a generated member"))
}

/// The C# member of the Weft name `name`. A C# reserved keyword followed by any number of `_`
/// takes one more `_` (`public` → `public_`, `public_` → `public__`), so every name keeps a member
/// of its own and `weft_name` gives it back. `@public` would be the C# way, but the XAML compiler
/// does not parse `@` in a binding path and copies a path segment into C# as it is (T105.17).
pub fn member(name: &str) -> Cow<'_, str> {
    if is_keyword(name.trim_end_matches('_')) {
        Cow::Owned(format!("{name}_"))
    } else {
        Cow::Borrowed(name)
    }
}

/// The Weft name of the C# member `member`, undoing `member`: one `_` comes off a keyword followed
/// by underscores. A bare keyword, which XAML that builds cannot hold, is read as itself.
#[cfg(any(feature = "import", test))]
pub fn weft_name(member: &str) -> &str {
    match member.strip_suffix('_') {
        Some(name) if is_keyword(name.trim_end_matches('_')) => name,
        _ => member,
    }
}

fn is_keyword(name: &str) -> bool {
    KEYWORDS.split_whitespace().any(|k| k == name)
}

/// C#'s reserved keywords, the first table of
/// <https://learn.microsoft.com/en-us/dotnet/csharp/language-reference/keywords/> (checked
/// 2026-10-17). Contextual keywords such as `value` are identifiers wherever a member is.
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
    fn a_keyword_takes_a_suffix_and_contextual_ones_do_not() {
        for (name, member_name) in [
            ("public", "public_"),
            ("public_", "public__"),
            ("public__", "public___"),
            ("class", "class_"),
            ("value", "value"),
            ("var", "var"),
            ("value_", "value_"),
            ("publicity", "publicity"),
            ("_", "_"),
        ] {
            assert_eq!(member(name), member_name, "{name}");
            assert_eq!(weft_name(member_name), name, "{member_name}");
        }
        // Hand-written XAML that does not build reads as it is.
        assert_eq!(weft_name("public"), "public");
    }

    #[test]
    fn every_keyword_and_its_suffixed_forms_map_back() {
        let keywords: Vec<_> = KEYWORDS.split_whitespace().collect();
        assert_eq!(keywords.len(), 77);
        for k in keywords {
            for name in [k.to_owned(), format!("{k}_"), format!("{k}__")] {
                let m = member(&name);
                assert_eq!(m, format!("{name}_"));
                assert_eq!(weft_name(&m), name);
            }
        }
    }

    #[test]
    fn keyword_paths_bind_their_suffixed_members() {
        let mut data = Data::default();
        for (bind, xbind) in [
            ("$.account.public", "ViewModel.account.public_"),
            ("$.account.public_", "ViewModel.account.public__"),
            ("$.class.0.event", "ViewModel.class_[0].event_"),
            ("$.value", "ViewModel.value"),
        ] {
            let mut problems = vec![];
            let path = data.read(bind, Read::Is(Leaf::Text), "text#t", &mut problems);
            assert!(problems.is_empty(), "{problems:?}");
            assert_eq!(path.as_deref(), Some(xbind));
        }
        // The data keeps the Weft names.
        assert_eq!(data.shapes[1].path, "$.account");
        assert!(data.shapes[1].fields.contains_key("public"));
        assert!(data.shapes[1].fields.contains_key("public_"));
    }
}
