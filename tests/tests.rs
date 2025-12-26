use kty::cli::{GlossaryArgs, GlossaryLangs, MainArgs, MainLangs, Options};
use kty::dict::{DGlossary, DIpa, DMain};
use kty::lang::{EditionLang, Lang};
use kty::make_dict;
use kty::path::{DictionaryType, PathManager};

use anyhow::{Ok, Result};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::format::FmtSpan;

use std::fs;
use std::path::{Path, PathBuf};

/// Clean empty folders under folder "root" recursively.
fn cleanup(root: &Path) -> bool {
    let entries = fs::read_dir(root).unwrap();

    let mut is_empty = true;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let child_empty = cleanup(&path);
            if child_empty {
                fs::remove_dir(&path).unwrap();
            } else {
                is_empty = false;
            }
        } else if path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"))
        {
            panic!("zip found in tests");
        } else {
            is_empty = false;
        }
    }

    is_empty
}

fn fixture_options(fixture_dir: &Path) -> Options {
    Options {
        save_temps: true,
        pretty: true,
        experimental: false,
        root_dir: fixture_dir.to_path_buf(),
        ..Default::default()
    }
}

fn fixture_main_args(
    edition: EditionLang,
    source: Lang,
    target: EditionLang,
    fixture_dir: &Path,
) -> MainArgs {
    MainArgs {
        langs: MainLangs {
            edition,
            source,
            target,
        },
        options: fixture_options(fixture_dir),
        ..Default::default()
    }
}

fn fixture_glossary_args(
    edition: EditionLang,
    source: EditionLang,
    target: Lang,
    fixture_dir: &Path,
) -> GlossaryArgs {
    GlossaryArgs {
        langs: GlossaryLangs {
            edition,
            source,
            target,
        },
        options: fixture_options(fixture_dir),
        ..Default::default()
    }
}

fn setup_tracing_test() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn"));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_span_events(FmtSpan::CLOSE)
        .with_target(true)
        .with_level(true)
        .init();
}

/// Test via snapshots and git diffs like the original
#[test]
fn snapshot() {
    setup_tracing_test();

    let fixture_dir = PathBuf::from("tests");
    // have to hardcode this since we have not initialized args
    let fixture_input_dir = fixture_dir.join("kaikki");

    // Nuke the output dir to prevent pollution
    // It has the disadvantage of massive diffs if we failfast.
    //
    // let fixture_output_dir = fixture_dir.join("dict");
    // Don't crash if there is no output dir. It may happen if we nuke it manually
    // let _ = fs::remove_dir_all(fixture_output_dir);

    let mut cases = Vec::new();
    let mut langs_in_testsuite = Vec::new();

    // iterdir and search for source-target-extract.jsonl files
    for entry in fs::read_dir(&fixture_input_dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();

        if let Some(fname) = path.file_name().and_then(|f| f.to_str())
            && let Some(base) = fname.strip_suffix("-extract.jsonl")
            && let Some((source, target)) = base.split_once('-')
        {
            let src = source.parse::<Lang>().unwrap();
            let tar = target.parse::<Lang>().unwrap();
            cases.push((src, tar));

            if !langs_in_testsuite.contains(&src) {
                langs_in_testsuite.push(src);
            }
            if !langs_in_testsuite.contains(&tar) {
                langs_in_testsuite.push(tar);
            }
        }
    }

    tracing::debug!("Found {} cases: {cases:?}", cases.len());

    // failfast
    // main
    for (source, target) in &cases {
        let Result::Ok(target) = EditionLang::try_from(*target) else {
            continue; // skip if target is not edition
        };
        let args = fixture_main_args(target, *source, target, &fixture_dir);
        let pm = PathManager::new(DictionaryType::Main, &args);

        if let Err(e) = shapshot_main(&args.options, &pm) {
            panic!("({source}): {e}");
        }
    }

    // glossary
    for (source, target) in &cases {
        if source != target {
            continue;
        }

        let Result::Ok(source) = EditionLang::try_from(*source) else {
            continue; // skip if source is not edition
        };

        for possible_target in &langs_in_testsuite {
            let args = fixture_glossary_args(source, source, *possible_target, &fixture_dir);
            let pm = PathManager::new(DictionaryType::Glossary, &args);
            make_dict(DGlossary, &args.options, &pm).unwrap();
        }
    }

    // ipa
    for (source, target) in &cases {
        let Result::Ok(target) = EditionLang::try_from(*target) else {
            continue; // skip if target is not edition
        };
        let args = fixture_main_args(target, *source, target, &fixture_dir);
        let pm = PathManager::new(DictionaryType::Ipa, &args);
        make_dict(DIpa, &args.options, &pm).unwrap();
    }

    cleanup(&fixture_dir.join("dict"));
}

/// Delete generated artifacts from previous tests runs, if any
fn delete_previous_output(pm: &PathManager) -> Result<()> {
    let pathdir_dict_temp = pm.dir_temp_dict();
    if pathdir_dict_temp.exists() {
        tracing::debug!("Deleting previous output: {pathdir_dict_temp:?}");
        fs::remove_dir_all(pathdir_dict_temp)?;
    }
    Ok(())
}

/// Read the expected result in the snapshot first, then git diff
fn shapshot_main(options: &Options, pm: &PathManager) -> Result<()> {
    delete_previous_output(pm)?;
    make_dict(DMain, options, pm)?;
    check_git_diff(pm)?;
    Ok(())
}

/// Run git --diff for charges in the generated json
fn check_git_diff(pm: &PathManager) -> Result<()> {
    let output = std::process::Command::new("git")
        .args([
            "diff",
            "--color=always",
            "--unified=0", // show 0 context lines
            "--",
            // we don't care about changes in tidy files
            &pm.dir_temp_dict().to_string_lossy(),
        ])
        .output()?;
    if !output.stdout.is_empty() {
        eprintln!("{}", String::from_utf8_lossy(&output.stdout));
        anyhow::bail!("changes!")
    }

    Ok(())
}
