## How to update a dictionary

Work in progress...

1. Click on check for updates
2. Click on the ! mark next to any dictionary that has an update
3. Click on update

Add a picture here, don't think yomitan covers it.

This should fetch the dictionary for a future release.

Explain this quote from [here](https://yomitan.wiki/dictionaries/):

> Be aware that non-English dictionaries generally contain fewer entries than their English counterparts. Even if your primary language is not English, you may consider also importing the English version for better coverage.

## How updating works internally

A comprehensive guide about making yomitan dictionaries can be found [here](https://github.com/yomidevs/yomitan/blob/master/docs/making-yomitan-dictionaries.md).

Updating is done via the dictionary index ([schema](https://github.com/yomidevs/yomitan/blob/master/ext/data/schemas/dictionary-index-schema.json)), and more precisely, via these four attributes:

1. `revision`: the sematic or calendar version of the dictionary. We use calendar for all dictionaries.
2. `isUpdatable`: set to true, makes the dictionary updatable.
3. `indexUrl`: points to an unzipped copy of the new dictionary index.
4. `downloadUrl`: points to a zipped version of the new dictionary.

When clicking `Check for Updates`, the [yomitan code](https://github.com/yomidevs/yomitan/blob/c0abb9e98a15aeb6b6f8f6e2d91fe5e54240b54a/ext/js/dictionary/dictionary-data-util.js#L350) compares the `revision` of the current, imported dictionary, with the one in the unzipped index at `indexUrl`. If the revision found in the latter is more recent, it downloads from `downloadUrl` the new dictionary and replaces the old version.

Example:

```json
{
  "revision": "2026.02.22",
  "isUpdatable": true,
  "indexUrl": "https://some/website/wty-el-el-index",
  "downloadUrl": "https://some/website/wty-el-el.zip",
  ...
}
```
