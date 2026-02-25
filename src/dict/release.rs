//! Unused - experimental
//!
//! Command to limit memory usage (linux):
//! systemd-run --user --scope -p MemoryMax=24G -p MemoryHigh=24G cargo run -r -- release -v

#![allow(unused)]

use core::panic;
use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    time::Instant,
};

use anyhow::Result;
use rayon::ThreadPoolBuilder;
use rayon::prelude::*;
use rkyv::Archived;
use rusqlite::{Connection, params};

use crate::lang::{Edition, EditionSpec, Lang, LangSpec};
use crate::models::kaikki::WordEntry;
use crate::path::{PathKind, PathManager};
use crate::utils::skip_because_file_exists;
use crate::{Map, cli::GlossaryLangs};
use crate::{cli::IpaArgs, dict::writer::write_yomitan};
use crate::{
    cli::IpaMergedArgs,
    dict::{DGlossaryExtended, DIpa, DIpaMerged},
};
use crate::{
    cli::IpaMergedLangs,
    dict::{DMain, Dictionary, Intermediate, IterLang, Langs, LangsKey},
};
use crate::{
    cli::{DictName, GlossaryArgs, MainArgs, MainLangs, Options},
    dict::DGlossary,
};

// runs main source all
fn release_main(edition: Edition) {
    // Limit only this workload (as opposed to the full logic. IPA and glossaries are completely
    // fine and will never OOM).
    let pool = ThreadPoolBuilder::new()
        // 2 seems fine with a MemoryMax of 20GB (works on my machine TM)
        // 8 is fine for testing with only English/German/French editions
        .num_threads(2)
        .build()
        .expect("Failed to build local thread pool");

    pool.install(|| {
        Lang::all().par_iter().for_each(|source| {
            let start = Instant::now();

            let langs = match (edition, source) {
                (Edition::Simple, Lang::Simple) => MainLangs {
                    source: LangSpec::One(edition.into()),
                    target: EditionSpec::One(edition),
                },
                (Edition::Simple, _) | (_, Lang::Simple) => return,
                // (Edition::En, Lang::Fi) => {
                //     tracing::warn!("Skipping finnish-english...");
                //     return;
                // }
                _ => MainLangs {
                    source: LangSpec::One(*source),
                    target: EditionSpec::One(edition),
                },
            };

            let args = MainArgs {
                langs,
                dict_name: DictName::default(),
                options: Options {
                    quiet: true,
                    root_dir: "data".into(),
                    ..Default::default()
                },
            };

            match make_dict(DMain, args) {
                Ok(()) => pp("main", *source, edition.into(), start),
                Err(err) => tracing::error!("[main-{source}-{edition}] ERROR: {err:?}"),
            }
        });
    });
}

fn release_ipa(edition: Edition) {
    Lang::all().par_iter().for_each(|source| {
        let start = Instant::now();

        let langs = match (edition, source) {
            (Edition::Simple, Lang::Simple) => MainLangs {
                source: LangSpec::One(edition.into()),
                target: EditionSpec::One(edition),
            },
            (Edition::Simple, _) | (_, Lang::Simple) => return,
            _ => MainLangs {
                source: LangSpec::One(*source),
                target: EditionSpec::One(edition),
            },
        };

        let args = IpaArgs {
            langs,
            dict_name: DictName::default(),
            options: Options {
                quiet: true,
                root_dir: "data".into(),
                ..Default::default()
            },
        };

        match make_dict(DIpa, args) {
            Ok(()) => pp("ipa", *source, edition.into(), start),
            Err(err) => tracing::error!("[ipa-{source}-{edition}] ERROR: {err:?}"),
        }
    });
}

fn release_ipa_merged(edition: Edition) {
    let start = Instant::now();

    let langs = match edition {
        Edition::Simple => return,
        _ => IpaMergedLangs {
            target: LangSpec::One(edition.into()),
        },
    };

    let args = IpaMergedArgs {
        langs,
        dict_name: DictName::default(),
        options: Options {
            quiet: true,
            root_dir: "data".into(),
            ..Default::default()
        },
    };

    match make_dict(DIpaMerged, args) {
        // Lang::Sq is a filler, doesn't exist
        Ok(()) => pp("gloss", Lang::Sq, edition.into(), start),
        Err(err) => tracing::error!("[ipa-merged--{edition}] ERROR: {err:?}"),
    }
}

fn release_glossary(edition: Edition) {
    Lang::all().par_iter().for_each(|target| {
        let start = Instant::now();

        let langs = match (edition, target) {
            (Edition::Simple, _) | (_, Lang::Simple) => return,
            _ if Lang::from(edition) == *target => return,
            _ => GlossaryLangs {
                source: EditionSpec::One(edition),
                target: LangSpec::One(*target),
            },
        };

        let args = GlossaryArgs {
            langs,
            dict_name: DictName::default(),
            options: Options {
                quiet: true,
                root_dir: "data".into(),
                ..Default::default()
            },
        };

        match make_dict(DGlossary, args) {
            // Order may be wrong
            Ok(()) => pp("gloss", *target, edition.into(), start),
            Err(err) => tracing::error!("[gloss-{target}-{edition}] ERROR: {err:?}"),
        }
    });
}

// Pretty print utility
#[allow(unused)]
const fn pp(dict_name: &str, first_lang: Lang, second_lang: Lang, time: Instant) {
    return;
    eprintln!(
        "[{dict_name}-{first_lang}-{second_lang}] done in {:.2?}",
        time.elapsed()
    );
}

pub fn release() -> Result<()> {
    let start = Instant::now();

    // let editions = Edition::all(); // WARN: OOMS 24GB pool
    // let editions: Vec<_> = Edition::all()
    //     .into_iter()
    //     .filter(|ed| *ed != Edition::En)
    //     .collect();
    // let editions = [Edition::En, Edition::De, Edition::Fr];

    let mut editions = Edition::all();
    // English is the bottleneck, and while I'm not entirely sure this works, getting to work asap
    // with English dictionaries should make things faster. This puts English first.
    editions.sort_by_key(|ed| i32::from(*ed != Edition::En));
    println!("Making release with {} editions", editions.len());
    println!("- {}", editions.iter().map(|ed| ed.to_string()).collect::<Vec<_>>().join(", "));

    // First, do all the downloading like in the python script.
    // We got rid of the filtered ones, so creating a pm for the main dictionary works fine
    // (actually any dictionary should work fine). It really doesn't matter as long as we download
    // every edition at some place.
    //
    // NOTE: For some reason this takes time even when db are init, why?
    // 
    editions.par_iter().for_each(|edition| {
        let lang: Lang = (*edition).into();
        let args = MainArgs {
            langs: MainLangs {
                source: lang.into(),
                target: (*edition).into(),
            },
            dict_name: DictName::default(),
            options: Options {
                quiet: false,
                root_dir: "data".into(),
                ..Default::default()
            },
        };
        let pm: &PathManager = &args.try_into().unwrap();
        let path_jsonl = find_or_download_jsonl(*edition, None, pm).unwrap();
        println!("Finished download for {edition} ({:.2?})", start.elapsed());
        let _ = WiktextractDb::create(*edition, path_jsonl).unwrap();
        println!("Finished database for {edition} ({:.2?})", start.elapsed());
    });
    let elapsed = start.elapsed();
    println!("Finished download & db creation in {elapsed:.2?}");

    let start = Instant::now();

    editions.par_iter().for_each(|edition| {
        release_main(*edition);
        release_ipa(*edition);
        release_ipa_merged(*edition);
        release_glossary(*edition);
    });

    let elapsed = start.elapsed();
    println!("Finished dictionaries in {elapsed:.2?}");

    Ok(())
}

pub struct WiktextractDb {
    pub conn: Connection,
}

impl WiktextractDb {
    // hardcoded at the moment, should require a PathManager
    fn db_path_for(edition: Edition) -> String {
        format!("data/db/wiktextract_{edition}.db")
    }

    pub fn open(edition: Edition) -> Result<Self> {
        let db_path = Self::db_path_for(edition);
        let conn = Connection::open(&db_path)?;
        Ok(Self { conn })
    }

    pub fn create(edition: Edition, path_jsonl: PathBuf) -> Result<Self> {
        let db_path = Self::db_path_for(edition);

        if let Some(parent) = Path::new(&db_path).parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(&db_path)?;

        conn.execute_batch(
            r"
            CREATE TABLE IF NOT EXISTS wiktextract (
                id   INTEGER PRIMARY KEY,
                lang TEXT NOT NULL,
                entry BLOB NOT NULL
            );
            ",
        )?;

        let mut db = Self { conn };

        // NOTE: Not sure if we need to check that we init the db beforehand
        let count: i64 = db.conn.query_row(
            "SELECT COUNT(*) FROM wiktextract",
            [],
            |row| row.get(0),
        )?;

        if count == 0 {
            tracing::info!("DB empty for {edition}, importing JSONL...");
            db.import_jsonl(path_jsonl)?;
        } else {
            tracing::trace!("DB already initialized for {edition} ({count} rows)");
        }

        Ok(db)
    }

    #[tracing::instrument(skip_all, level = "debug")]
    pub fn import_jsonl<P: AsRef<Path>>(&mut self, jsonl_path: P) -> Result<()> {
        let start = Instant::now();
        let file = File::open(&jsonl_path)?;
        let reader = BufReader::new(file);

        let tx = self.conn.transaction()?;
        {
            let mut stmt = tx.prepare("INSERT INTO wiktextract (lang, entry) VALUES (?, ?)")?;

            for line in reader.lines() {
                let line = line?;
                let word_entry: WordEntry = serde_json::from_str(&line)?;
                let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&word_entry)?;

                stmt.execute(params![word_entry.lang_code, bytes.as_ref()])?;
            }
        }
        tx.commit()?;
        tracing::debug!(
            "Making db took {:.3} ms",
            start.elapsed().as_secs_f64() * 1000.0
        );

        Ok(())
    }

    pub fn blob_to_word_entry(blob: &[u8]) -> Result<WordEntry> {
        let archived: &Archived<WordEntry> =
            rkyv::access::<Archived<WordEntry>, rkyv::rancor::Error>(blob).unwrap();
        let word_entry: WordEntry =
            rkyv::deserialize::<WordEntry, rkyv::rancor::Error>(archived).unwrap();
        Ok(word_entry)
    }
}

pub fn make_dict<D: Dictionary + IterLang + EditionFrom>(dict: D, raw_args: D::A) -> Result<()> {
    let pm: &PathManager = &raw_args.try_into()?;
    let (_, source_pm, target_pm) = pm.langs();
    let opts = &pm.opts;
    pm.setup_dirs()?;

    tracing::trace!("{pm:#?}");

    // (source, target) -> D::I
    let mut irs_map: Map<LangsKey, D::I> = Map::default();

    for pair in iter_datasets(pm) {
        let (edition, _path_jsonl) = pair?;

        let db = WiktextractDb::open(edition)?;
        let source = match source_pm {
            LangSpec::All => panic!(),
            LangSpec::One(lang) => lang,
        };
        let target = match target_pm {
            LangSpec::All => panic!(),
            LangSpec::One(lang) => lang,
        };
        let langs = Langs {
            edition,
            source,
            target,
        };

        let other = match dict.edition_is() {
            EditionIs::Target => source,
            EditionIs::Source => target,
        };
        tracing::trace!("Opened db for {edition} edition, selecting lang {other}...");

        let mut stmt = db
            .conn
            .prepare("SELECT entry FROM wiktextract WHERE lang = ?")?;
        let mut rows = stmt.query([other.as_ref()])?;

        while let Some(row) = rows.next()? {
            let blob: &[u8] = row.get_ref(0)?.as_blob()?;
            let mut entry = WiktextractDb::blob_to_word_entry(blob)?;

            // TODO: iter_langs doesn't make any sense...
            // we should make a dict for (edition, source, target) at a time...
            let key = dict.langs_to_key(langs);
            let irs = irs_map.entry(key).or_default();
            dict.preprocess(langs, &mut entry, opts, irs);
            dict.process(langs, &entry, irs);
        }
    }

    if irs_map.len() > 1 {
        tracing::debug!("Matrix ({}): {:?}", irs_map.len(), irs_map.keys());
    }

    for (key, mut irs) in irs_map {
        // if !opts.quiet {
        dict.found_ir_message(&key, &irs);
        // }
        if irs.is_empty() {
            continue;
        }
        dict.postprocess(&mut irs);
        if opts.save_temps && dict.write_ir() {
            irs.write(pm)?;
        }
        if !opts.skip_yomitan {
            let mut pm2 = pm.clone();
            let source = key.source;
            let target = key.target;
            pm2.set_source(source.into());
            pm2.set_target(target.into());
            pm2.setup_dirs()?;
            tracing::trace!("calling to_yomitan with (source={source}, target={target})",);
            let labelled_entries = match key.edition {
                EditionSpec::All => {
                    let langs = Langs::new(Edition::Zh, key.source, key.target);
                    dict.to_yomitan(langs, irs)
                }
                EditionSpec::One(edition) => {
                    let langs = Langs::new(edition, key.source, key.target);
                    dict.to_yomitan(langs, irs)
                }
            };
            write_yomitan(source, target, opts, &pm2, labelled_entries)?;
        }
    }
    Ok(())
}

pub fn find_or_download_jsonl(
    edition: Edition,
    lang: Option<Lang>,
    pm: &PathManager,
) -> Result<PathBuf> {
    let paths_candidates = pm.dataset_paths(edition, lang);
    let kinds_to_check = [PathKind::Unfiltered, PathKind::Filtered];
    let of_kind: Vec<_> = paths_candidates
        .inner
        .iter()
        .filter(|p| kinds_to_check.contains(&p.kind))
        .collect();

    if !pm.opts.redownload
        && let Some(existing) = of_kind.iter().find(|p| p.path.exists())
    {
        if !pm.opts.quiet {
            skip_because_file_exists("download", &existing.path);
        }
        return Ok(existing.path.clone());
    }

    let path = &of_kind
        .iter()
        .next_back()
        .unwrap_or_else(|| {
            panic!(
                "No path available, \
             for edition={edition:?} and lang={lang:?} | {paths_candidates:?}"
            )
        })
        .path;

    // TODO: remove this once it's done: it prevents downloading in the testsuite
    // anyhow::bail!(
    //     "Downloading is disabled but JSONL file was not found @ {}",
    //     path.display()
    // );

    #[cfg(feature = "html")]
    crate::download::download_jsonl(edition, path, false)?;

    Ok(path.clone())
}

fn iter_datasets(pm: &PathManager) -> impl Iterator<Item = Result<(Edition, PathBuf)>> + '_ {
    let (edition_pm, source_pm, _) = pm.langs();

    edition_pm.variants().into_iter().map(move |edition| {
        let lang_opt = match source_pm {
            LangSpec::All => None,
            LangSpec::One(lang) => Some(lang),
        };
        let path_jsonl = find_or_download_jsonl(edition, lang_opt, pm)?;
        // tracing::debug!("edition: {edition}, path: {}", path_jsonl.display());

        Ok((edition, path_jsonl))
    })
}

pub enum EditionIs {
    Target,
    Source,
}

// Replacement of IterLang/DatasetStrategy here
pub trait EditionFrom {
    fn edition_is(&self) -> EditionIs;
}

impl EditionFrom for DMain {
    fn edition_is(&self) -> EditionIs {
        EditionIs::Target
    }
}

impl EditionFrom for DIpa {
    fn edition_is(&self) -> EditionIs {
        EditionIs::Target
    }
}

impl EditionFrom for DGlossary {
    fn edition_is(&self) -> EditionIs {
        EditionIs::Source
    }
}

impl EditionFrom for DIpaMerged {
    fn edition_is(&self) -> EditionIs {
        // Does not really matter since for this dict source == target
        EditionIs::Source
    }
}

impl EditionFrom for DGlossaryExtended {
    fn edition_is(&self) -> EditionIs {
        todo!()
    }
}
