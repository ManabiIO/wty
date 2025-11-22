pub mod tags_constants;

use indexmap::IndexMap;
use serde::Serialize;
use serde::ser::{SerializeSeq, Serializer};
use tags_constants::{POSES, TAG_BANK, TAG_ORDER};

use crate::models::Tag;

// TODO: a bunch of sorting and handling of tags should go here

/// Blacklisted tags when expanding forms @ tidy
pub const BLACKLISTED_TAGS: [&str; 14] = [
    "inflection-template",
    "table-tags",
    "canonical",
    "class",
    "error-unknown-tag",
    "error-unrecognized-form",
    "includes-article",
    "obsolete",
    "archaic",
    "used-in-the-form",
    "romanization",
    "dated",
    "auxiliary",
    // multiword-construction was in REDUNDANT_TAGS in the original.
    // Yet it only seems to give noise for the fr-en edition (@ prendre):
    // * Form: 'present indicative of avoir + past participle' ???
    // * Tags: ["indicative", "multiword-construction", "perfect", "present"]
    //
    // It also removes valid german forms that are nonetheless most useless:
    // * werde gepflogen haben (for pflegen)
    // (note that gepflogen is already added)
    // This was considered ok. To revisit if it is more intrusive in other languages.
    "multiword-construction",
];
/// Tags that are blacklisted if they happen at every expanded form @ tidy
pub const IDENTITY_TAGS: [&str; 3] = ["nominative", "singular", "infinitive"];
/// Tags that we just remove from forms
pub const REDUNDANT_TAGS: [&str; 1] = ["combined-form"];

// Internal legacy types that are just for documentation since we ended up loading
// tag_bank_term.json as a raw list of tuples in tags_constants
//
// #[derive(Deserialize, Default)]
// struct WhitelistedTags(Vec<WhitelistedTag>);
//
// // Internal type
// #[derive(Deserialize, Default)]
// struct WhitelistedTag {
//     short_tag: String,
//     category: String,
//     sort_order: i32,
//     aliases: Vec<String>, // only this changes
//     popularity_score: i32,
// }

// The actual type that we pass to yomitan (cf. tagbank.ts (yomichan-dict-builder))
#[derive(Debug)]
pub struct TagInformation {
    pub short_tag: String,
    pub category: String,
    sort_order: i32,
    pub long_tag: String, // only this changes
    popularity_score: i32,
}

impl Serialize for TagInformation {
    // serialize as array
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(5))?;
        seq.serialize_element(&self.short_tag)?;
        seq.serialize_element(&self.category)?;
        seq.serialize_element(&self.sort_order)?;
        seq.serialize_element(&self.long_tag)?;
        seq.serialize_element(&self.popularity_score)?;
        seq.end()
    }
}

// ignore target_iso !== en since tags should always be in English anyway
/// Sort tags by their position in the tag bank.
pub fn sort_tags(tags: &mut [Tag]) {
    tags.sort_by(|a, b| {
        let index_a = TAG_ORDER.iter().position(|&x| x == a);
        let index_b = TAG_ORDER.iter().position(|&x| x == b);

        match (index_a, index_b) {
            (Some(i), Some(j)) => i.cmp(&j), // both found → compare positions
            // This seems better but it's different from the original
            // (None, None) => a.cmp(b),        // neither found → alphabetical fallback
            (None, None) => std::cmp::Ordering::Equal, // neither found → do nothing
            (Some(_), None) => std::cmp::Ordering::Less, // found beats not-found
            (None, Some(_)) => std::cmp::Ordering::Greater,
        }
    });
}

// Sort forms tags
pub fn sort_tags_by_similar(tags: &mut [Tag]) {
    tags.sort_by(|a, b| {
        let a_words: Vec<&str> = a.split(' ').collect();
        let b_words: Vec<&str> = b.split(' ').collect();

        // Compare the second word (index #1) if present
        let a_second = a_words.get(1).unwrap_or(&"");
        let b_second = b_words.get(1).unwrap_or(&"");

        let main_comparison = a_second.cmp(b_second);
        if main_comparison != std::cmp::Ordering::Equal {
            return main_comparison;
        }

        // Compare word-by-word
        let len = a_words.len().min(b_words.len());
        for i in 0..len {
            if a_words[i] != b_words[i] {
                return a_words[i].cmp(b_words[i]);
            }
        }

        // Fallback: shorter is "less"
        a_words.len().cmp(&b_words.len())
    });
}

/// Remove tag1 if there is a tag2 such that tag1 <= tag2
///
/// Takes space-separated tags.
pub fn remove_redundant_tags(tags: &mut Vec<Tag>) {
    let mut keep = vec![true; tags.len()];

    for i in 0..tags.len() {
        for j in 0..tags.len() {
            // tag_i <= tag_j
            if i != j && tags_are_subset(&tags[i], &tags[j]) {
                keep[i] = false;
                break;
            }
        }
    }

    let mut idx = 0;
    tags.retain(|_| {
        let k = keep[idx];
        idx += 1;
        k
    });
}

fn tags_are_subset(a: &str, b: &str) -> bool {
    let a_words: Vec<&str> = a.split(' ').collect();
    let b_words: Vec<&str> = b.split(' ').collect();
    a_words.iter().all(|p| b_words.contains(p))
}

/// Return a Vec<TagInformation> from `tag_bank_terms` that fits the yomitan tag schema.
pub fn get_tag_bank_as_tag_info() -> Vec<TagInformation> {
    TAG_BANK
        .iter()
        .map(|entry| TagInformation {
            short_tag: entry.0.into(),
            category: entry.1.into(),
            sort_order: entry.2,
            long_tag: entry.3[0].into(), // normalized
            popularity_score: entry.4,
        })
        .collect()
}

// the bank should be shared across all languages anyway
//
/// Look for the tag in `TAG_BANK` (`tag_bank_terms.json`) and return the `TagInformation` if any.
///
/// Note that `long_tag` is returned normalized.
pub fn find_tag_in_bank(tag: &str) -> Option<TagInformation> {
    TAG_BANK.iter().find_map(|entry| {
        if entry.3.contains(&tag) {
            Some(TagInformation {
                short_tag: entry.0.into(),
                category: entry.1.into(),
                sort_order: entry.2,
                long_tag: entry.3[0].into(), // normalized
                popularity_score: entry.4,
            })
        } else {
            None
        }
    })
}

// the pos tags should be shared across all languages anyway
//
/// Look for the pos in POSES (`parts_of_speech.json`) and return the short form if any.
pub fn find_pos(pos: &str) -> Option<&'static str> {
    POSES.iter().find_map(|entry| {
        if entry.contains(&pos) {
            Some(entry[0])
        } else {
            None
        }
    })
}

const PERSON_TAGS: [&str; 3] = ["first-person", "second-person", "third-person"];

fn person_sort(tags: &mut [String]) {
    tags.sort_by_key(|x| PERSON_TAGS.iter().position(|p| p == x).unwrap_or(999));
}

// merge similar tags if the only difference is the persons
// input: ['first-person singular present', 'third-person singular present']
// output: ['first/third-person singular present']
pub fn merge_person_tags(tags: &[Tag]) -> Vec<Tag> {
    let contains_person = tags
        .iter()
        .any(|tag| PERSON_TAGS.iter().any(|p| tag.contains(p)));

    if tags.is_empty() || !contains_person {
        return tags.into();
    }

    let mut result = Vec::new();
    let mut merge_obj: IndexMap<Tag, Vec<Tag>> = IndexMap::new();

    for tag in tags {
        let all_tags: Vec<_> = tag.split(' ').collect();
        let person_tags: Vec<_> = all_tags
            .iter()
            .copied()
            .filter(|t| PERSON_TAGS.contains(t))
            .collect();

        if person_tags.len() == 1 {
            let person = person_tags[0].to_string();
            let other_tags: Vec<_> = all_tags
                .iter()
                .copied()
                .filter(|t| !PERSON_TAGS.contains(t))
                .map(str::to_string)
                .collect();

            let tag_key = other_tags.join("_");
            merge_obj.entry(tag_key).or_default().push(person);
        } else {
            result.push(tag.clone());
        }
    }

    for (tag_key, mut person_matches) in merge_obj {
        let mut tags: Vec<_> = if tag_key.is_empty() {
            Vec::new()
        } else {
            tag_key.split('_').map(str::to_string).collect()
        };

        person_sort(&mut person_matches);
        let merged_tag = format!("{}-person", person_matches.join("/").replace("-person", ""));
        tags.push(merged_tag);
        sort_tags(&mut tags);
        result.push(tags.join(" "));
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn to_string_vec(str_vec: &[&str]) -> Vec<String> {
        str_vec.iter().map(|s| (*s).to_string()).collect()
    }

    // This imitates the original. Can be removed if sort_tags logic changes.
    #[test]
    fn sort_tags_base() {
        let tag_not_found = "__sentinel";
        assert!(!TAG_ORDER.contains(&tag_not_found));
        let mut received = to_string_vec(&[tag_not_found, "Gheg"]);
        let expected = to_string_vec(&[tag_not_found, "Gheg"]);
        sort_tags(&mut received);
        assert_eq!(received, expected);
    }

    fn make_test_sort_tags_by_similar(received: &[&str], expected: &[&str]) {
        let mut vreceived: Vec<String> = to_string_vec(received);
        let vexpected: Vec<String> = to_string_vec(expected);
        sort_tags_by_similar(&mut vreceived);
        assert_eq!(vreceived, vexpected);
    }

    #[test]
    fn sort_tags_by_similar1() {
        make_test_sort_tags_by_similar(&["singular", "accusative"], &["accusative", "singular"]);
    }

    #[test]
    fn sort_tags_by_similar2() {
        make_test_sort_tags_by_similar(
            &["accusative", "singular", "neuter", "nominative", "vocative"],
            &["accusative", "neuter", "nominative", "singular", "vocative"],
        );
    }

    #[test]
    fn sort_tags_by_similar3() {
        make_test_sort_tags_by_similar(
            &["dual nominative", "accusative dual", "dual vocative"],
            &["accusative dual", "dual nominative", "dual vocative"],
        );
    }

    fn make_test_merge_person_tags(received: &[&str], expected: &[&str]) {
        let vreceived: Vec<String> = to_string_vec(received);
        let received = merge_person_tags(&vreceived);
        let vexpected: Vec<String> = to_string_vec(expected);
        assert_eq!(received, vexpected);
    }

    #[test]
    fn merge_person_tags1() {
        make_test_merge_person_tags(
            &[
                "first-person singular present",
                "third-person singular present",
            ],
            &["first/third-person singular present"],
        );
    }

    // Improvement over the original that would return:
    // "first/second-person singular past",
    // "third-person singular past",
    #[test]
    fn merge_person_tags2() {
        make_test_merge_person_tags(
            &[
                "first-person singular past",
                "second-person singular past",
                "third-person singular past",
            ],
            &["first/second/third-person singular past"],
        );
    }

    #[test]
    fn remove_redundant_tags1() {
        let mut received = to_string_vec(&["foo", "bar", "foo bar", "foo bar zee"]);
        let expected = to_string_vec(&["foo bar zee"]);
        remove_redundant_tags(&mut received);
        assert_eq!(received, expected);
    }

    #[test]
    fn remove_redundant_tags2() {
        let mut received = to_string_vec(&[
            "first-person singular indicative preterite",
            "first-person singular preterite",
        ]);
        let expected = to_string_vec(&["first-person singular indicative preterite"]);
        remove_redundant_tags(&mut received);
        assert_eq!(received, expected);
    }
}
