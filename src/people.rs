//! What the bot knows about specific people, keyed by Discord user ID so it
//! keeps recognising them when they change their nickname.

use std::{collections::HashMap, fs, io::ErrorKind, path::Path};

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use serenity::all::UserId;

#[derive(Debug, Clone)]
pub struct Person {
    /// The name the bot uses for this person, whatever their nickname is.
    pub name: String,
    /// Facts the bot knows about this person.
    pub notes: Vec<String>,
}

#[derive(Debug, Default)]
pub struct People {
    by_id: HashMap<UserId, Person>,
    /// IDs in the order they appear in the file, so the prompt built from
    /// them is identical every time (which lets llama-server reuse its work).
    order: Vec<UserId>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PeopleFile {
    #[serde(default)]
    person: Vec<Entry>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    id: u64,
    name: String,
    #[serde(default)]
    notes: Vec<String>,
}

impl People {
    /// Loads `path`. A missing file is only an error when `required` is set,
    /// so the default location can be left out.
    pub fn load(path: &Path, required: bool) -> Result<Self> {
        match fs::read_to_string(path) {
            Ok(text) => Self::parse(&text).with_context(|| format!("invalid {}", path.display())),
            Err(e) if e.kind() == ErrorKind::NotFound && !required => Ok(Self::default()),
            Err(e) => Err(e).with_context(|| format!("could not read {}", path.display())),
        }
    }

    fn parse(text: &str) -> Result<Self> {
        let file: PeopleFile = toml::from_str(text)?;
        let mut by_id = HashMap::new();
        let mut order = Vec::new();
        for Entry { id, name, notes } in file.person {
            if id == 0 {
                bail!("person {name:?} has an invalid id of 0");
            }
            let name = name.trim().to_owned();
            if name.is_empty() {
                bail!("person with id {id} has an empty name");
            }
            let person = Person { name, notes };
            order.push(UserId::new(id));
            if let Some(previous) = by_id.insert(UserId::new(id), person) {
                bail!(
                    "id {id} is listed more than once (again after {:?})",
                    previous.name
                );
            }
        }
        Ok(Self { by_id, order })
    }

    pub fn get(&self, id: UserId) -> Option<&Person> {
        self.by_id.get(&id)
    }

    /// Everyone's `name`, in file order.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.order
            .iter()
            .filter_map(|id| self.get(*id))
            .map(|person| person.name.as_str())
    }

    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    /// A paragraph for the system prompt describing everyone with notes, in
    /// file order, or `None` when no one has notes. Sent with every message so
    /// the bot knows people however they're referred to: by name, nickname,
    /// "he", or not at all.
    pub fn describe_all(&self) -> Option<String> {
        let lines: Vec<String> = self
            .order
            .iter()
            .filter_map(|id| self.get(*id))
            .filter(|person| !person.notes.is_empty())
            .map(|person| format!("- {}: {}", person.name, person.notes.join("; ")))
            .collect();
        if lines.is_empty() {
            return None;
        }
        Some(format!(
            "# People you know\n\
             These are people in this server. Use these facts when they come up, \
             whether or not the person is in the conversation:\n{}",
            lines.join("\n")
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
        [[person]]
        id = 123456789012345678
        name = "Alex"
        notes = ["always asks what to eat", "mains support"]

        [[person]]
        id = 42
        name = "Jordan"
    "#;

    #[test]
    fn parses_and_looks_up_by_id() {
        let people = People::parse(SAMPLE).unwrap();
        assert_eq!(people.len(), 2);
        assert_eq!(
            people.get(UserId::new(123456789012345678)).unwrap().name,
            "Alex"
        );
        assert!(people.get(UserId::new(42)).unwrap().notes.is_empty());
        assert!(people.get(UserId::new(7)).is_none());
    }

    #[test]
    fn describes_everyone_with_notes_in_file_order() {
        let text = People::parse(&format!(
            "{SAMPLE}\n[[person]]\nid = 7\nname = \"Kevin\"\nnotes = [\"loves light novels\"]"
        ))
        .unwrap()
        .describe_all()
        .unwrap();
        assert!(text.ends_with(
            "\n- Alex: always asks what to eat; mains support\n\
             - Kevin: loves light novels"
        ));
        assert!(!text.contains("Jordan"), "people without notes are left out");

        let no_notes = People::parse("[[person]]\nid = 42\nname = \"Jordan\"").unwrap();
        assert!(no_notes.describe_all().is_none());
    }

    #[test]
    fn rejects_duplicates_and_typos() {
        let duplicate = "[[person]]\nid = 1\nname = \"a\"\n[[person]]\nid = 1\nname = \"b\"";
        assert!(People::parse(duplicate).is_err());
        let typo = "[[person]]\nid = 1\nname = \"a\"\nnote = [\"x\"]";
        assert!(People::parse(typo).is_err());
    }

    #[test]
    fn example_file_parses() {
        let people = People::parse(include_str!("../people.example.toml")).unwrap();
        assert!(people.len() > 0);
    }

    #[test]
    fn missing_optional_file_is_empty() {
        let people = People::load(Path::new("does/not/exist.toml"), false).unwrap();
        assert_eq!(people.len(), 0);
        assert!(People::load(Path::new("does/not/exist.toml"), true).is_err());
    }
}
