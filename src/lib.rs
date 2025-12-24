pub mod cli;
pub mod diagnostic;
pub mod dict;
pub mod download;
pub mod lang;
pub mod models;
pub mod path;
pub mod tags;
pub mod utils;

use anyhow::{Context, Ok, Result};
use fxhash::FxBuildHasher;
use indexmap::{IndexMap, IndexSet};
use serde::Serialize;
#[allow(unused)]
use tracing::{Level, debug, error, info, span, trace, warn};
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

use std::fs::{self, File};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};

use crate::cli::ArgsOptions;
use crate::diagnostic::Diagnostics;
use crate::dict::get_index;
#[cfg(feature = "html")]
use crate::download::download_jsonl;
use crate::lang::{EditionLang, Lang};
use crate::models::kaikki::WordEntry;
use crate::models::yomitan::YomitanEntry;
use crate::path::PathManager;
use crate::tags::get_tag_bank_as_tag_info;
use crate::utils::{
    CHECK_C, pretty_print_at_path, pretty_println_at_path, skip_because_file_exists,
};

pub type Map<K, V> = IndexMap<K, V, FxBuildHasher>; // Preserve insertion order
pub type Set<K> = IndexSet<K, FxBuildHasher>;

const CONSOLE_PRINT_INTERVAL: i32 = 10000;
const BANK_SIZE: usize = 25_000;

const STYLES_CSS: &[u8] = include_bytes!("../assets/styles.css");
const STYLES_CSS_EXPERIMENTAL: &[u8] = include_bytes!("../assets/styles_experimental.css");

type LabelledYomitanEntry = (&'static str, Vec<YomitanEntry>);

enum BankSink<'a> {
    Disk,
    Zip(&'a mut ZipWriter<File>, SimpleFileOptions),
}

/// Write lemma / form / whatever banks to either disk or zip.
///
/// If `save_temps` is true, we assume that the user is debugging and does not need the zip.
fn write_yomitan(
    source: Lang,
    target: Lang,
    options: &ArgsOptions,
    pm: &PathManager,
    labelled_entries: &[LabelledYomitanEntry],
) -> Result<()> {
    let mut bank_index = 0;

    if options.save_temps {
        let out_dir = pm.dir_temp_dict();
        fs::create_dir_all(&out_dir)?;
        for (entry_ty, entries) in labelled_entries {
            write_banks(
                options.pretty,
                options.quiet,
                entries,
                &mut bank_index,
                entry_ty,
                &out_dir,
                BankSink::Disk,
            )?;
        }

        if !options.quiet {
            pretty_println_at_path(&format!("{CHECK_C} Wrote temp data"), &out_dir);
        }
        return Ok(());
    }

    let writer_path = pm.path_dict();
    let writer_file = File::create(&writer_path)?;
    let mut zip = ZipWriter::new(writer_file);
    let zip_options =
        SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    // Zip index.json
    let index_string = get_index(&pm.dict_name_expanded(), source, target);
    zip.start_file("index.json", zip_options)?;
    zip.write_all(index_string.as_bytes())?;

    // Copy paste styles.css
    zip.start_file("styles.css", zip_options)?;
    if options.experimental {
        zip.write_all(STYLES_CSS_EXPERIMENTAL)?;
    } else {
        zip.write_all(STYLES_CSS)?;
    }

    // Copy paste tag_bank.json
    let tag_bank = get_tag_bank_as_tag_info();
    let tag_bank_bytes = serde_json::to_vec_pretty(&tag_bank)?;
    zip.start_file("tag_bank_1.json", zip_options)?; // it needs to end in _1
    zip.write_all(&tag_bank_bytes)?;

    for (entry_ty, entries) in labelled_entries {
        write_banks(
            options.pretty,
            options.quiet,
            entries,
            &mut bank_index,
            entry_ty,
            &writer_path,
            BankSink::Zip(&mut zip, zip_options),
        )?;
    }

    zip.finish()?;

    if !options.quiet {
        pretty_println_at_path(&format!("{CHECK_C} Wrote yomitan dict"), &writer_path);
    }

    Ok(())
}

/// Writes `yomitan_entries` in batches to `out_sink` (either disk or a zip).
#[tracing::instrument(skip_all)]
fn write_banks(
    pretty: bool,
    quiet: bool,
    yomitan_entries: &[YomitanEntry],
    bank_index: &mut usize,
    entry_ty: &str,
    out_dir: &Path,
    mut out_sink: BankSink,
) -> Result<()> {
    // NOTE: this assumes that once a type is passed, all the remaining entries are of same type
    let bank_name_prefix = match yomitan_entries.first() {
        Some(first) => first.file_prefix(),
        None => return Ok(()),
    };

    let total_bank_num = yomitan_entries.len().div_ceil(BANK_SIZE);

    for (bank_num, bank) in yomitan_entries.chunks(BANK_SIZE).enumerate() {
        *bank_index += 1;

        let json_bytes = if pretty {
            serde_json::to_vec_pretty(&bank)?
        } else {
            serde_json::to_vec(&bank)?
        };

        let bank_name = format!("{bank_name_prefix}_{bank_index}.json");
        let file_path = out_dir.join(&bank_name);

        match out_sink {
            BankSink::Disk => {
                let mut file = File::create(&file_path)?;
                file.write_all(&json_bytes)?;
            }
            BankSink::Zip(ref mut zip, zip_options) => {
                zip.start_file(&bank_name, zip_options)?;
                zip.write_all(&json_bytes)?;
            }
        }

        if !quiet {
            if bank_num > 0 {
                print!("\r\x1b[K");
            }
            pretty_print_at_path(
                &format!(
                    "Wrote yomitan {entry_ty} bank {}/{total_bank_num} ({} entries)",
                    bank_num + 1,
                    bank.len()
                ),
                &file_path,
            );
            std::io::stdout().flush()?;
        }
    }

    if !quiet {
        println!();
    }

    Ok(())
}

/// Trait for Intermediate representation. Used for postprocessing (merge, etc.) and debugging via snapshots.
///
/// The simplest form is a Vec<YomitanEntry> if we don't want to do anything fancy, cf. `DGlossary`
pub trait Intermediate: Default {
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// How to write `Self::I` to disk. This is only called if `options.save_temps` is set and
    /// `Dictionary::write_ir` returns true.
    #[allow(unused_variables)]
    fn write(&self, pm: &PathManager, options: &ArgsOptions) -> Result<()> {
        Ok(())
    }
}

impl<T> Intermediate for Vec<T>
where
    T: Serialize,
{
    fn len(&self) -> usize {
        Self::len(self)
    }

    fn write(&self, pm: &PathManager, options: &ArgsOptions) -> Result<()> {
        let writer_path = pm.dir_tidy().join("tidy.jsonl");
        let writer_file = File::create(&writer_path)?;
        let writer = BufWriter::new(&writer_file);
        if options.pretty {
            serde_json::to_writer_pretty(writer, self)?;
        } else {
            serde_json::to_writer(writer, self)?;
        }
        if !options.quiet {
            pretty_print_at_path("Wrote tidy", &writer_path);
        }
        Ok(())
    }
}

/// Trait to abstract the process of making a dictionary.
pub trait Dictionary {
    type I: Intermediate;

    // NOTE:Maybe in the future we can get rid of this. It requires cleaning up the legacy mutable
    // behaviour of the main dictionary.
    //
    /// How to preprocess a `WordEntry`. Everything that mutates `word_entry` should go here.
    #[allow(unused_variables)]
    fn preprocess(
        &self,
        edition: EditionLang,
        source: Lang,
        target: Lang,
        word_entry: &mut WordEntry,
        options: &ArgsOptions,
        irs: &mut Self::I,
    ) {
    }

    /// How to transform a `WordEntry` into intermediate representation.
    ///
    /// Most dictionaries only make *at most one* `Self::I` from a `WordEntry`.
    fn process(
        &self,
        edition: EditionLang,
        source: Lang,
        target: Lang,
        word_entry: &WordEntry,
        irs: &mut Self::I,
    );

    /// Console message for found entries. It is customized for the main dictionary.
    fn found_ir_message(&self, irs: &Self::I) {
        println!("Found {} entries", irs.len());
    }

    /// Whether to write or not `Self::I` to disk.
    ///
    /// Compare to `save_temp`, that rules if `Self::I` AND the `term_banks` are written to disk.
    ///
    /// This is mainly a debug function, in order to allow not writing the ir `Self::I` to disk for
    /// minor dictionaries in the testsuite. It is only set to true in the main dictionary.
    fn write_ir(&self) -> bool {
        false
    }

    /// How to postprocess the intermediate representation.
    ///
    /// This can be implemented, for instance, to merge entries from different editions, or to
    /// postprocess forms, tags etc.
    #[allow(unused_variables)]
    fn postprocess(&self, irs: &mut Self::I) {}

    // Returns a Vec<LabelledYomitanEntry> instead of Vec<YomitanEntry> because that is the
    // argument type of write_yomitan.
    fn to_yomitan(
        &self,
        edition: EditionLang,
        source: Lang,
        target: Lang,
        options: &ArgsOptions,
        diagnostics: &mut Diagnostics,
        irs: Self::I,
    ) -> Vec<LabelledYomitanEntry>;

    /// How to write diagnostics, if any.
    #[allow(unused_variables)]
    fn write_diagnostics(&self, pm: &PathManager, diagnostics: &Diagnostics) -> Result<()> {
        Ok(())
    }
}

fn find_or_download_jsonl(
    edition: EditionLang,
    lang: Lang,
    paths: &[PathBuf],
    options: &ArgsOptions,
) -> Result<PathBuf> {
    let first_path_found = paths.iter().find(|pbuf| pbuf.exists());

    if let (false, Some(pbuf)) = (options.redownload, first_path_found) {
        if !options.quiet {
            skip_because_file_exists("download", pbuf);
        }
        Ok(pbuf.clone())
    } else {
        let path_jsonl_raw_of_download = paths.last().unwrap();
        #[cfg(feature = "html")]
        download_jsonl(edition, lang, path_jsonl_raw_of_download, options.quiet)?;
        Ok(path_jsonl_raw_of_download.clone())
    }
}

fn rejected(entry: &WordEntry, options: &ArgsOptions) -> bool {
    options
        .reject
        .iter()
        .any(|(k, v)| k.field_value(entry) == v)
        || !options
            .filter
            .iter()
            .all(|(k, v)| k.field_value(entry) == v)
}

pub fn make_dict<D: Dictionary>(dict: D, options: &ArgsOptions, pm: &PathManager) -> Result<()> {
    let (edition_pm, source_pm, target_pm) = pm.langs();

    pm.setup_dirs()?;

    let capacity = 256 * (1 << 10); // default is 8 * (1 << 10) := 8KB
    let mut line = Vec::with_capacity(1 << 10);
    let mut entries = D::I::default();

    for (edition, paths) in pm.paths_jsonl_raw() {
        let path_jsonl_raw = find_or_download_jsonl(edition, source_pm, &paths, options)?;
        tracing::debug!("path_jsonl_raw: {}", path_jsonl_raw.display());

        let reader_file = File::open(&path_jsonl_raw)?;
        let mut reader = BufReader::with_capacity(capacity, reader_file);

        let mut line_count = 0;
        let mut accepted_count = 0;

        loop {
            line.clear();
            if reader.read_until(b'\n', &mut line)? == 0 {
                break; // EOF
            }

            line_count += 1;

            let mut word_entry: WordEntry =
                serde_json::from_slice(&line).with_context(|| "Error decoding JSON @ make_dict")?;

            if !options.quiet && line_count % CONSOLE_PRINT_INTERVAL == 0 {
                print!("Processed {line_count} lines...\r");
                std::io::stdout().flush()?;
            }

            if rejected(&word_entry, options) {
                continue;
            }

            accepted_count += 1;
            if accepted_count == options.first {
                break;
            }

            dict.preprocess(
                edition,
                source_pm,
                target_pm,
                &mut word_entry,
                options,
                &mut entries,
            );

            dict.process(edition, source_pm, target_pm, &word_entry, &mut entries);
        }

        if !options.quiet {
            println!("Processed {line_count} lines. Accepted {accepted_count} lines.");
        }
    }

    if !options.quiet {
        dict.found_ir_message(&entries);
    }

    if entries.is_empty() {
        return Ok(());
    }

    dict.postprocess(&mut entries);

    if options.save_temps && dict.write_ir() {
        entries.write(pm, options)?;
    }

    if !options.skip_yomitan {
        let mut diagnostics = Diagnostics::default();

        // HACK: This unwrap_or is only for GlossaryExtended and works as a filler
        // because the edition is not used in the implementation of to_yomitan for that dict.
        // It is basically here to not crash the code. Happy face.
        let ed = edition_pm.try_into().unwrap_or(EditionLang::En);
        let labelled_entries =
            dict.to_yomitan(ed, source_pm, target_pm, options, &mut diagnostics, entries);

        dict.write_diagnostics(pm, &diagnostics)?;

        write_yomitan(source_pm, target_pm, options, pm, &labelled_entries)?;
    }

    Ok(())
}
