Everything here only concerns the main dictionary.

There are (at least) three cases in which one may want to modify tags:

1 - **Tag order**: rules the order in which tags are displayed.  
2 - **Tag filtering**: rules abbreviations and which tags are displayed.  
3 - **Extraction logic**: rules where to extract tags from wiktionary data.

Tag postprocessing is done after building the whole intermediate representation, to only sort once with every extracted tag. The relevant function is `src/dict/main.rs::postprocess_forms`.

### Tag order

Tag order is recorded in `assets/tag_order.json`. While this file has categories (formatility, cases etc.), those are later strip and serve only as visual help. The sorting is done with the flattened list.

!!! warning "Run the build script after any modification to update the rust code: either `just build` or `python3 scripts/build.py`"

### Tag filtering

Tag filtering is recorded in `assets/tag_bank_term.json`. The items of this JSON list are a custom version of:

```typescript
type TagInformation = [
  tagName: string,
  category: string,
  sortingOrder: number,
  notes: string,
  popularityScore: number,
];
```

where `notes` is replaced with either a string, or a list of strings representing aliases, the first one being shown when hovering the tag.

Here is an example of a simple [commit](https://github.com/daxida/wty/commit/00c69daa89344d971978d905897aa19e7c1ae619) to add the "Buddhism" tag, that modifies the JSON, then runs the build script.

!!! warning "Run the build script after any modification to update the rust code: either `just build` or `python3 scripts/build.py`"

### Extraction logic

This requires some knowledge of kaikki internals and how they extract tags. TODO
