"""Write a test registry and update tests from there.

The registry must contain the original json lines with attributes that we may not
use in the making of a dictionary, in case we use them in the future.

The registry is only intended to be used via git diffs, to check for updates.
"""

import argparse
import json
import re
from difflib import SequenceMatcher
from pathlib import Path
from typing import Any, Literal, TypedDict, cast, get_args

import requests

TESTS_DIR_PATH = Path("tests")
TESTS_PATH = TESTS_DIR_PATH / "kaikki"
REGISTRY_PATH = TESTS_DIR_PATH / "registry.json"

L = Literal[
    "cs",
    "de",
    "el",
    "en",
    "es",
    "fa",
    "fr",
    "grc",
    "ja",
    "ko",
    "la",
    "ru",
    "sq",
    "th",
    "zh",
]
"""A language code."""

ALLOWED_LANGS = frozenset(get_args(L))


class RegValue(TypedDict):
    url: str
    download_url: str
    json: Any


Reg = dict[L, dict[L, list[RegValue]]]
"""A registry that we will dump as JSON at REGISTRY_PATH."""


def read_jsonl(text: str) -> list[Any]:
    return [json.loads(line) for line in text.strip().splitlines()]


def flatten_json(obj: Any, prefix: str = "") -> dict[str, str]:
    items = {}
    if isinstance(obj, dict):
        for k, v in obj.items():
            items.update(flatten_json(v, f"{prefix}.{k}" if prefix else k))
    elif isinstance(obj, list):
        for i, v in enumerate(obj):
            items.update(flatten_json(v, f"{prefix}[{i}]"))
    else:
        items[prefix] = str(obj)
    return items


def json_to_str(a: Any) -> str:
    return "\n".join(f"{k}: {v}" for k, v in sorted(flatten_json(a).items()))


def json_similarity(a: Any, b: Any) -> float:
    return SequenceMatcher(None, json_to_str(a), json_to_str(b)).ratio()


def get_test_path(source: L, target: L) -> Path:
    return TESTS_PATH / f"{source}-{target}-extract.jsonl"


def add_to_registry(registry: Reg, source: L, target: L, value: RegValue) -> None:
    if source not in registry:
        registry[source] = {target: []}
    registry[source][target].append(value)


def update_registry_for_pair(source: L, target: L) -> Reg:
    print(f"Updating {source}-{target} (registry)", flush=True)
    tests_path = get_test_path(source, target)
    tests = read_jsonl(tests_path.read_text())

    registry: Reg = {}

    for test in tests:
        word = test["word"]

        search_query = "/".join([word[0], word[:2], word])
        # We can replace the "All languages combined" with the source but it requires
        # knowing how to convert from an iso (en) to a long name (English)
        if target == "en":
            url = f"https://kaikki.org/dictionary/All%20languages%20combined/meaning/{search_query}.jsonl"
        else:
            url = f"https://kaikki.org/{target}wiktionary/All%20languages%20combined/meaning/{search_query}.jsonl"

        resp = requests.get(url)
        if not resp.ok:
            print(
                f"[WARN] (err. {resp.status_code}) Failed to fetch {word} @ {url}\n"
                "Ignore this if the given word is a custom testcase not in kaikki"
            )
            # If it is a custom testcase, while not ideal, we still store it in the registry
            # for simplicity. This way we just need to read the registry for updating tests.
            custom_test: RegValue = {
                "url": "none",
                "download_url": "none",
                "json": test,
            }
            add_to_registry(registry, source, target, custom_test)
            continue

        text = resp.content.decode("utf-8")
        jsonl = read_jsonl(text)

        scores = [
            (i, json_similarity(test, cand), cand) for i, cand in enumerate(jsonl)
        ]
        scores.sort(reverse=True, key=lambda x: x[1])
        best_index, _, best_match = scores[0]

        registry_value: RegValue = {
            "url": url.replace(".jsonl", ".html"),
            "download_url": url,
            "json": best_match,
        }
        add_to_registry(registry, source, target, registry_value)

    return registry


def validate_lang(lang: str) -> L:
    if lang not in ALLOWED_LANGS:
        raise ValueError(f"String {lang} was not found in L")
    return cast(L, lang)


FNAME_RE = re.compile(r"^([a-zA-Z]+)-([a-zA-Z]+)-extract\.jsonl$")


def get_lang_pairs_to_update() -> dict[L, list[L]]:
    lang_pairs: dict[L, list[L]] = {}

    for file in TESTS_PATH.iterdir():
        m = FNAME_RE.match(file.name)
        if not m:
            raise ValueError(f"Unexpected filename in {TESTS_PATH}: {file.name}")

        source = validate_lang(m.group(1))
        target = validate_lang(m.group(2))

        if source not in lang_pairs:
            lang_pairs[source] = []
        lang_pairs[source].append(target)

    lang_pairs = {src: sorted(lang_pairs[src]) for src in sorted(lang_pairs)}

    return lang_pairs


def update_registry() -> None:
    registry: Reg = {}
    for source, targets in get_lang_pairs_to_update().items():
        if source not in registry:
            registry[source] = {}

        for target in targets:
            registry_for_pair = update_registry_for_pair(source, target)
            registry[source][target] = registry_for_pair[source][target]

    REGISTRY_PATH.write_text(json.dumps(registry, indent=2, ensure_ascii=False))


def update_tests() -> None:
    if not REGISTRY_PATH.exists():
        print(f"{REGISTRY_PATH} not found. Run with flag '--update-registry' first.")
        return

    registry: Reg = json.loads(REGISTRY_PATH.read_text())

    for source, target in registry.items():
        for target, registry_values in target.items():
            new_tests: list[str] = [value["json"] for value in registry_values]
            tests_path = get_test_path(source, target)
            print(f"Updating {source}-{target} (test)", flush=True)
            with tests_path.open("w", encoding="utf-8") as f:
                for new_test in new_tests:
                    json.dump(new_test, f, ensure_ascii=False)
                    f.write("\n")


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Update and manage Kaikki tests.",
    )
    parser.add_argument(
        "--update-registry",
        action="store_true",
        help="Update the registry before updating tests.",
    )
    args = parser.parse_args()

    if args.update_registry:
        print(f"Updating registry at {REGISTRY_PATH}")
        update_registry()

    print(f"Updating tests at {TESTS_PATH}")
    update_tests()


if __name__ == "__main__":
    main()
