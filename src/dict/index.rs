use crate::lang::Lang;

const BASE_URL: &str = "https://huggingface.co/datasets/daxida/test-dataset/resolve/main";

/// The url to download this dictionary.
///
/// See: docs/javascripts/download.js (keep in sync)
fn download_url(dict_name_expanded: &str, source: Lang, target: Lang) -> String {
    format!("{BASE_URL}/dict/{target}/{source}/{dict_name_expanded}.zip?download=true")
}

/// The url of the cloned index of this dictionary.
///
/// See: scripts/release.py (keep in sync)
fn index_url(dict_name_expanded: &str) -> String {
    format!("{BASE_URL}/index/{dict_name_expanded}-index?download=true")
}

// Original index attributes:
// https://github.com/yomidevs/kaikki-to-yomitan/blob/7b5bd7f922c9003b09f253f361b8a2e4ff26e13a/4-make-yomitan.js#L19
// https://github.com/yomidevs/kaikki-to-yomitan/blob/7b5bd7f922c9003b09f253f361b8a2e4ff26e13a/4-make-yomitan.js#L809
//
// How updating works:
// * check for updates
// https://github.com/yomidevs/yomitan/blob/d82684d9b746da60adb0e28dec5f4a4914da68c1/ext/js/pages/settings/dictionary-controller.js#L174
// 1. Fetch the new index from indexUrl
// 2. Compare revisions to see if the one from the new index comes *after* our current index
// 3. If so, store the downloadUrl of the new index
// 4. Show a button notifying that an update is available
// 5. If the user decides to update, download downloadUrl
//
/// Dictionary index.
///
/// indexUrl points to a separate copy of the index in the download repository.
/// downloadUrl points to the download link in the download repository.
///
/// <https://github.com/yomidevs/yomitan/blob/master/ext/data/schemas/dictionary-index-schema.json>
pub fn get_index(dict_name_expanded: &str, source: Lang, target: Lang) -> String {
    let current_date = chrono::Utc::now().format("%Y.%m.%d"); // needs to be dot separated
    let index_url = index_url(dict_name_expanded);
    let download_url = download_url(dict_name_expanded, source, target);
    format!(
        r#"{{
  "title": "{dict_name_expanded}",
  "format": 3,
  "revision": "{current_date}",
  "sequenced": true,
  "author": "kty contributors",
  "url": "https://github.com/daxida/kty",
  "description": "Dictionaries for various language pairs generated from Wiktionary data, via Kaikki and kty.",
  "attribution": "https://kaikki.org/",
  "sourceLanguage": "{source}",
  "targetLanguage": "{target}",
  "isUpdatable": true,
  "indexUrl": "{index_url}",
  "downloadUrl": "{download_url}"
}}"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls() {
        let dict_name = "kty-afb-en-ipa";
        let durl = download_url(dict_name, Lang::En, Lang::Afb);
        assert_eq!(
            durl,
            "https://huggingface.co/datasets/daxida/test-dataset/resolve/main/dict/afb/en/kty-afb-en-ipa.zip?download=true"
        );
        let iurl = index_url(dict_name);
        assert_eq!(
            iurl,
            "https://huggingface.co/datasets/daxida/test-dataset/resolve/main/index/kty-afb-en-ipa-index?download=true"
        );
    }
}
