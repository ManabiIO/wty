//! Wiktextract / kaikki data models.
//!
//! Non-en JSON schemas:
//! <https://tatuylonen.github.io/wiktextract>
//!
//! There is no EN JSON schema but there are some approximations:
//! <https://kaikki.org/dictionary/errors/mapping/index.html>
//! <https://github.com/tatuylonen/wiktextract/blob/master/src/wiktextract/extractor/en/type_utils.py>
//!
//! Example (el):
//! <https://github.com/tatuylonen/wiktextract/blob/master/src/wiktextract/extractor/el/models.py>

use serde::{Deserialize, Serialize};

// In case we ever decide to narrow them
pub type Tag = String;
pub type Pos = String;

#[derive(Debug, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct WordEntry {
    pub word: String,
    pub pos: Pos,

    pub lang_code: String,

    pub head_templates: Vec<HeadTemplate>,

    pub etymology_text: String,

    pub sounds: Vec<Sound>,

    pub senses: Vec<Sense>,
    pub tags: Vec<Tag>,

    pub forms: Vec<Form>,
    pub form_of: Vec<AltForm>,
    pub alt_of: Vec<AltForm>,

    pub translations: Vec<Translation>, // used in glossary
}

// To be avoided as much as possible: sort of internal field.
#[derive(Debug, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct HeadTemplate {
    pub expansion: String,
}

#[derive(Debug, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct Sound {
    pub ipa: String,
    pub tags: Vec<Tag>,
    pub note: String,
    // pub other: String, // [ja]
}

#[derive(Debug, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct Sense {
    // Glosses are usually a one string vector, but when there's more, it follows:
    // ["Gloss supercategory", "Specific gloss.", "More specific...", etc.]
    // cf. https://en.wiktionary.org/wiki/pflegen
    pub glosses: Vec<String>,
    pub examples: Vec<Example>,
    pub form_of: Vec<AltForm>,
    pub alt_of: Vec<AltForm>,
    pub tags: Vec<Tag>,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
#[serde(default)]
pub struct Example {
    pub text: String,
    pub translation: String,
}

#[derive(Debug, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct AltForm {
    pub word: String,
}

#[derive(Debug, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct Form {
    pub form: String,
    pub tags: Vec<Tag>,
    pub ruby: Vec<(String, String)>, // [ja] (kanji, hiragana)
}

#[derive(Debug, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct Translation {
    pub lang_code: String,
    pub word: String,
    pub sense: String,
}
