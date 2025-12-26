use crate::{
    Map, Set,
    cli::Options,
    dict::{Diagnostics, Dictionary, LabelledYomitanEntry, get_ipas, get_reading},
    lang::{EditionLang, Lang},
    models::{
        kaikki::WordEntry,
        yomitan::{
            DetailedDefinition, NTag, Node, PhoneticTranscription, TermBank, TermBankMeta,
            TermPhoneticTranscription, YomitanEntry, wrap,
        },
    },
    tags::find_short_pos,
};

#[derive(Debug, Clone, Copy)]
pub struct DGlossary;

#[derive(Debug, Clone, Copy)]
pub struct DGlossaryExtended;

#[derive(Debug, Clone, Copy)]
pub struct DIpa;

#[derive(Debug, Clone, Copy)]
pub struct DIpaMerged;

impl Dictionary for DGlossary {
    type I = Vec<YomitanEntry>;

    fn process(
        &self,
        edition: EditionLang,
        _source: Lang,
        target: Lang,
        entry: &WordEntry,
        irs: &mut Self::I,
    ) {
        process_glossary(edition, target, entry, irs);
    }

    fn to_yomitan(
        &self,
        _edition: EditionLang,
        _source: Lang,
        _target: Lang,
        _options: &Options,
        _diagnostics: &mut Diagnostics,
        irs: Self::I,
    ) -> Vec<LabelledYomitanEntry> {
        vec![("term", irs)]
    }
}

impl Dictionary for DGlossaryExtended {
    type I = Vec<IGlossaryExtended>;

    fn process(
        &self,
        edition: EditionLang,
        source: Lang,
        target: Lang,
        entry: &WordEntry,
        irs: &mut Self::I,
    ) {
        process_glossary_extended(edition, source, target, entry, irs);
    }

    fn postprocess(&self, irs: &mut Self::I) {
        let mut map = Map::default();

        for (lemma, pos, edition, translations) in irs.drain(..) {
            let entry = map
                .entry(lemma.clone())
                .or_insert_with(|| (pos.clone(), edition, Set::default()));

            for tr in translations {
                entry.2.insert(tr);
            }
        }

        irs.extend(map.into_iter().map(|(lemma, (pos, edition, set))| {
            (lemma, pos, edition, set.into_iter().collect::<Vec<_>>())
        }));
    }

    fn to_yomitan(
        &self,
        _edition: EditionLang,
        _source: Lang,
        _target: Lang,
        _options: &Options,
        _diagnostics: &mut Diagnostics,
        irs: Self::I,
    ) -> Vec<LabelledYomitanEntry> {
        vec![("term", to_yomitan_glossary_extended(irs))]
    }
}

impl Dictionary for DIpa {
    type I = Vec<IIpa>;

    fn process(
        &self,
        edition: EditionLang,
        source: Lang,
        _target: Lang,
        entry: &WordEntry,
        irs: &mut Self::I,
    ) {
        process_ipa(edition, source, entry, irs);
    }

    fn to_yomitan(
        &self,
        _edition: EditionLang,
        _source: Lang,
        _target: Lang,
        _options: &Options,
        _diagnostics: &mut Diagnostics,
        irs: Self::I,
    ) -> Vec<LabelledYomitanEntry> {
        vec![("term", to_yomitan_ipa(irs))]
    }
}

impl Dictionary for DIpaMerged {
    type I = Vec<IIpa>;

    fn process(
        &self,
        edition: EditionLang,
        source: Lang,
        _target: Lang,
        entry: &WordEntry,
        irs: &mut Self::I,
    ) {
        process_ipa(edition, source, entry, irs);
    }

    fn postprocess(&self, irs: &mut Self::I) {
        // TODO: use dedup
        // Keep only unique entries
        let mut seen = Set::default();
        seen.extend(irs.drain(..));
        *irs = seen.into_iter().collect();
        // Sorting is not needed ~ just for visibility
        irs.sort_by(|a, b| a.0.cmp(&b.0));
    }

    fn to_yomitan(
        &self,
        _edition: EditionLang,
        _source: Lang,
        _target: Lang,
        _options: &Options,
        _diagnostics: &mut Diagnostics,
        tidy: Self::I,
    ) -> Vec<LabelledYomitanEntry> {
        vec![("term", to_yomitan_ipa(tidy))]
    }
}

fn process_glossary(
    source: EditionLang,
    target: Lang,
    word_entry: &WordEntry,
    irs: &mut Vec<YomitanEntry>,
) {
    // rg: process translations processtranslations
    let target_str = target.to_string();

    let mut translations: Map<Option<String>, Vec<String>> = Map::default();
    for translation in word_entry.non_trivial_translations() {
        if translation.lang_code != target_str {
            continue;
        }

        let sense = if translation.sense.is_empty() {
            None
        } else {
            Some(translation.sense.clone())
        };

        translations
            .entry(sense)
            .or_default()
            .push(translation.word.clone());
    }

    if translations.is_empty() {
        return;
    }

    let mut definitions = Vec::new();
    for (sense, translations) in translations {
        match sense {
            None => {
                for translation in translations {
                    definitions.push(DetailedDefinition::Text(translation));
                }
            }
            Some(sense) => {
                let mut sc_translations_content = Node::new_array();
                sc_translations_content.push(wrap(NTag::Span, "", Node::Text(sense)));
                sc_translations_content.push(wrap(
                    NTag::Ul,
                    "",
                    Node::Array(
                        translations
                            .into_iter()
                            .map(|translation| wrap(NTag::Li, "", Node::Text(translation)))
                            .collect(),
                    ),
                ));
                let sc_translations =
                    DetailedDefinition::structured(wrap(NTag::Div, "", sc_translations_content));
                definitions.push(sc_translations);
            }
        }
    }

    let reading =
        get_reading(source, target, word_entry).unwrap_or_else(|| word_entry.word.clone());
    let found_pos = match find_short_pos(&word_entry.pos) {
        Some(short_pos) => short_pos.to_string(),
        None => word_entry.pos.clone(),
    };

    let ir = YomitanEntry::TermBank(TermBank(
        word_entry.word.clone(),
        reading,
        found_pos.clone(),
        found_pos,
        definitions,
    ));
    irs.push(ir);
}

type IGlossaryExtended = (String, String, EditionLang, Vec<String>);

fn process_glossary_extended(
    edition: EditionLang,
    source: Lang,
    target: Lang,
    word_entry: &WordEntry,
    irs: &mut Vec<IGlossaryExtended>,
) {
    let target_str = target.to_string();
    let source_str = source.to_string();

    let mut translations: Map<&str, (Vec<&str>, Vec<&str>)> = Map::default();
    for translation in word_entry.non_trivial_translations() {
        if translation.lang_code == target_str {
            translations
                .entry(&translation.sense)
                .or_default()
                .0
                .push(&translation.word);
        }

        if translation.lang_code == source_str {
            translations
                .entry(&translation.sense)
                .or_default()
                .1
                .push(&translation.word);
        }
    }

    // We only keep translations with matches in both languages (source and target)
    translations.retain(|_, (targets, sources)| !targets.is_empty() && !sources.is_empty());

    if translations.is_empty() {
        return;
    }

    let found_pos = match find_short_pos(&word_entry.pos) {
        Some(short_pos) => short_pos.to_string(),
        None => word_entry.pos.clone(),
    };

    // A "semi" cartesian product:
    // {
    //   "British overseas territory":
    //   (["Gjibraltar", "Gjibraltari"], ["Ἡράκλειαι στῆλαι", "Κάλπη"])
    // }
    //     source                            target (what we search)
    // >>> ["Gjibraltar", "Gjibraltari"]  <> "Ἡράκλειαι στῆλαι"
    // >>> ["Gjibraltar", "Gjibraltari"]  <> "Κάλπη"
    let mut translations_semi_product: Vec<IGlossaryExtended> = Vec::new();

    for (_sense, translations) in translations {
        for lemma in translations.1 {
            let definitions = translations.0.iter().map(|def| def.to_string()).collect();
            let entry = (lemma.to_string(), found_pos.clone(), edition, definitions);
            translations_semi_product.push(entry);
        }
    }

    irs.extend(translations_semi_product);
}

fn to_yomitan_glossary_extended(irs: Vec<IGlossaryExtended>) -> Vec<YomitanEntry> {
    irs.into_iter()
        .map(|(lemma, found_pos, _, translations)| {
            let definitions = translations
                .into_iter()
                .map(|translation| DetailedDefinition::Text(translation))
                .collect();

            YomitanEntry::TermBank(TermBank(
                lemma,
                String::new(),
                found_pos.clone(),
                found_pos,
                definitions,
            ))
        })
        .collect()
}

type IIpa = (String, PhoneticTranscription);

fn process_ipa(edition: EditionLang, source: Lang, word_entry: &WordEntry, irs: &mut Vec<IIpa>) {
    let ipas = get_ipas(word_entry);

    if ipas.is_empty() {
        return;
    }

    let phonetic_transcription = PhoneticTranscription {
        reading: get_reading(edition, source, word_entry)
            .unwrap_or_else(|| word_entry.word.clone()),
        transcriptions: ipas,
    };

    irs.push((word_entry.word.clone(), phonetic_transcription));
}

fn to_yomitan_ipa(irs: Vec<IIpa>) -> Vec<YomitanEntry> {
    irs.into_iter()
        .map(|(lemma, transcription)| {
            YomitanEntry::TermBankMeta(TermBankMeta::TermPhoneticTranscription(
                TermPhoneticTranscription(lemma, "ipa".to_string(), transcription),
            ))
        })
        .collect()
}
