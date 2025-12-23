use std::fs;

use anyhow::{Ok, Result};
use indexmap::IndexMap;

use crate::{Map, path::PathManager};

type Key = String; // A tag
type Word = String; // A word
// Vec of words in which the tag was encountered
type CounterValue = Vec<Word>;
type Counter = Map<Key, CounterValue>;

// For debugging purposes
#[derive(Debug, Default)]
pub struct Diagnostics {
    /// Tags found in bank
    accepted_tags: Counter,
    /// Tags not found in bank
    rejected_tags: Counter,
}

impl Diagnostics {
    fn increment(map: &mut Counter, key: Key, word: Word) {
        map.entry(key).or_default().push(word);
    }

    pub fn increment_accepted_tag(&mut self, tag: Key, word: Word) {
        Self::increment(&mut self.accepted_tags, tag, word);
    }

    pub fn increment_rejected_tag(&mut self, tag: Key, word: Word) {
        Self::increment(&mut self.rejected_tags, tag, word);
    }

    fn is_empty(&self) -> bool {
        self.accepted_tags.is_empty() && self.rejected_tags.is_empty()
    }

    pub fn write(&self, pm: &PathManager) -> Result<()> {
        if self.is_empty() {
            return Ok(());
        }

        let dir_diagnostics = pm.dir_diagnostics();
        fs::create_dir_all(&dir_diagnostics)?;

        let accepted_sorted = convert_and_sort_indexmap(&self.accepted_tags);
        let rejected_sorted = convert_and_sort_indexmap(&self.rejected_tags);
        let json: Map<&'static str, _> =
            Map::from_iter([("rejected", rejected_sorted), ("accepted", accepted_sorted)]);
        let writer = fs::File::create(dir_diagnostics.join("tags.json"))?;
        serde_json::to_writer_pretty(writer, &json)?;

        Ok(())
    }
}

// hacky: takes advantage of insertion order
fn convert_and_sort_indexmap(map: &Counter) -> IndexMap<String, (usize, Word)> {
    // Display first word
    let mut entries: Vec<_> = map
        .iter()
        .filter_map(|(key, words)| {
            words
                .first()
                .cloned()
                .map(|first_word| (key.clone(), (words.len(), first_word)))
        })
        .collect();

    entries.sort_by(|a, b| b.1.0.cmp(&a.1.0));
    let mut sorted = IndexMap::with_capacity(entries.len());
    for (key, value) in entries {
        sorted.insert(key, value);
    }

    sorted
}
