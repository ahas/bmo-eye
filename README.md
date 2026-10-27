# bmo-eye

The platform's vision unit: one request and answer shape over three modes, each with its own model and its own
priority chain of backends.

| Mode | CLI | What it reads |
| --- | --- | --- |
| watch | `bmo eye watch` | video: a file, a URL, a YouTube link; scenes, on-screen text, speech |
| see | `bmo eye see` | images: photos, screenshots, charts, several at once |
| read | `bmo eye read` | documents: scans, PDFs, pages of text and tables (OCR) |

Each mode asks its chain in order and answers from the first backend that works: the local model on node n2
(an arbiter tier-1 occupant), then OpenRouter, then Claude or Codex where the road takes the input natively.
Every agent on the platform looks at an image or a video through bmo-eye first unless told otherwise.

`docs/` holds the canonical statements: the API, the modes and their chains, the n2 installs and their measured
fit, and the plan. Archive documents and reports for the owner are written in Korean; `docs/` stays in English.

## Layout

| Folder | Holds |
|---|---|
| `docs/` | The API, the modes and their chains, the n2 install, and the stage 2 plan |
| `scripts/` | The measurement harness the stage 1 numbers were taken with |
| `eye.card.toml` | The service's declaration to the card master |
