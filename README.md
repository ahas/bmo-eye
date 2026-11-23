**English** | [한국어](README.ko.md) | [日本語](README.ja.md) | [简体中文](README.zh-CN.md)

# bmo-eye

The platform's vision unit: one request and answer shape over three modes, each with its own chain of
backends. It holds no model of its own — every mode looks through the Qwen3.8-Flash-Next set that the
platform's local engine already runs on four nodes.

| Mode | CLI | What it reads |
| --- | --- | --- |
| watch | `bmo watch` | video: a file, a video or video-page URL, YouTube included; scenes, on-screen text, speech |
| see | `bmo see` | images: photos, screenshots, charts, several at once |
| read | `bmo read` | documents: scans, PDFs, pages of text and tables (OCR) |

Each mode asks its chain in order and answers from the first backend that works: the local engine on the
general-pack node, then the three Coder nodes, then OpenRouter, then the caller's own model where the caller
asks for it and it takes the input natively.
A video's speech is read on a workbench node's CPU and the frames on a local engine node; the vision unit takes
no card.
Every agent on the platform looks at an image or a video through bmo-eye first unless told otherwise.

`docs/` holds the canonical statements: the API, the modes and their chains, the engines it looks through,
the plan, and the design for a drawing request's reference pipeline. Archive documents and reports for the
owner are written in Korean; `docs/` stays in English.

## Layout

| Folder | Holds |
|---|---|
| `crates/` | The Rust workspace: `client` (the types and the HTTP client) and `server` (the routes, the store, the chain, the `bmo-eye` binary and the module that writes the OpenAPI document, printed by its `openapi` example) |
| `docs/` | The index of the documents, the API and the OpenAPI document it generates, the modes and their chains, the engines, the plan, the drawing-reference design, and the three tool entries `eye/see`, `eye/read` and `eye/watch` |
| `scripts/` | The measurement harness stage 1's numbers were taken with |
| `src/` | The repository's own package, which holds no code, only the tests that span the crates |
| `tests/` | The tests that walk the routes against a stand-in engine node, read the media a look is made of and hold the kept OpenAPI document to what the code writes |
| `deploy/` | The user unit the deployment restarts and watches |

At the root stand the Cargo workspace, the tool host's manifest `tool.toml` with the schemas its entries name
(beside it, as the host requires), the testbed's `testbed.toml` and `CLAUDE.md`.
