# bmo-eye

The platform's vision unit: one request and answer shape over three modes, each with its own chain of
backends. It holds no model of its own — every mode looks through the Qwen3.8-Flash-Next set that
bmo-lab-arena already runs on n3 to n6.

| Mode | CLI | What it reads |
| --- | --- | --- |
| watch | `bmo watch` | video: a file, a URL, a YouTube link; scenes, on-screen text, speech |
| see | `bmo see` | images: photos, screenshots, charts, several at once |
| read | `bmo read` | documents: scans, PDFs, pages of text and tables (OCR) |

Each mode asks its chain in order and answers from the first backend that works: the Lab-arena instance on n3,
then the three Coder nodes, then OpenRouter, then the caller's own model where it takes the input natively.
A video's speech is read on n2's CPU and the frames on a Lab-arena node; the vision unit takes no card.
Every agent on the platform looks at an image or a video through bmo-eye first unless told otherwise.

`docs/` holds the canonical statements: the API, the modes and their chains, the engines it looks through,
and the plan. Archive documents and reports for the owner are written in Korean; `docs/` stays in English.

## Layout

| Folder | Holds |
|---|---|
| `crates/` | The Rust workspace: `client` (the types and the HTTP client) and `server` (the routes, the store, the chain and the `bmo-eye` binary) |
| `docs/` | The API, the modes and their chains, the engines, and the plan |
| `scripts/` | The measurement harness stage 1's numbers were taken with |
| `src/` | The repository's own package, which holds no code |
| `tests/` | The tests that walk the routes against a stand-in Lab-arena node |
| `deploy/` | The user unit the deployment restarts and watches |

At the root stand the Cargo workspace, the tool host's manifest `tool.toml` with the schemas its entries name
(beside it, as the host requires), the testbed's `testbed.toml` and `CLAUDE.md`.
