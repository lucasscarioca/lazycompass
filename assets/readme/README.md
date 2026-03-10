# README Assets

This folder holds release-facing media used by the top-level README.

## Current README images

- `lazycompass-tui-documents.png` main hero image
- `lazycompass-tui-queryresult.png` inline query result
- `lazycompass-tui-savedqueries.png` saved query flow
- `lazycompass-tui-document.png` document detail
- `lazycompass-tui-indexes.png` indexes view
- `lazycompass-cli.png` CLI query + table output

Other captures kept in this folder:

- `lazycompass-tui-collections.png`
- `lazycompass-tui-export.png`
- `lazycompass-tui-help.png`
- `tui-demo.gif` optional future asset

## Capture goals

### `lazycompass-tui-documents.png`

Show the default browsing flow with enough data to make the UI feel real:

- connections, databases, collections visible
- document list populated
- footer/help visible if possible
- use the `ember` theme unless another theme photographs better

Suggested caption in README:

`Browse connections, collections, and documents without leaving the terminal.`

### `lazycompass-tui-queryresult.png`

Show a query or aggregation result state:

- non-trivial result rows
- export or copy action visible if possible
- table density high enough to look useful

Suggested caption in README:

`Run queries, inspect results, and export as JSON, CSV, or table output.`

### `lazycompass-tui-savedqueries.png`

Show the saved workflow:

- saved query or aggregation picker open, or
- a drafted query/aggregation ready to rerun or save

Suggested caption in README:

`Keep repo-local queries and aggregations in git-friendly JSON files.`

### `tui-demo.gif`

Keep it short:

- 8 to 20 seconds
- open app
- move through collections
- run saved query
- show results

## Capture notes

- Use a seeded local dataset from [dev/qa/README.md](../../dev/qa/README.md)
- Prefer a wide terminal with clean margins
- Hide machine-specific prompts and shell noise
- Avoid sensitive hostnames, db names, or user names
- Keep copy sharp: realistic collection names, realistic fields, no lorem ipsum
- If the terminal font/line spacing looks cramped, tune the terminal first instead of scaling the image later

## README wiring

The root README currently uses:

- `assets/readme/lazycompass-tui-documents.png`
- `assets/readme/lazycompass-tui-queryresult.png`
- `assets/readme/lazycompass-tui-savedqueries.png`
- `assets/readme/lazycompass-tui-document.png`
- `assets/readme/lazycompass-tui-indexes.png`
- `assets/readme/lazycompass-cli.png`
