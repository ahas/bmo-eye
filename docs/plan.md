# The plan, and what of it stands

The vision unit was planned in two stages: a stage that settled what it is, and a stage that built it. The
first stage settled the modes, the API and the chains; the second was steered away from a model of its own
when the owner's direction of 2026-10-27 09:34 KST said the Qwen3.8 set already standing on n3 to n6 would
read the pictures. This document states what stands now and what is still to build.

## What stands

| Part | Where it stands |
|---|---|
| the API | `docs/api.md`, served by `bmo-eye` on main-host at `127.0.0.1:7435` |
| the modes and their chains | `docs/modes.md`, one chain per mode over the same four Lab-arena instances |
| the engines it looks through | `docs/engines.md`: the Qwen3.8-Flash-Next set on n3 to n6, the picture reader, and n2's CPU for speech |
| the server | `crates/server`, the binary `bmo-eye`, with the store, the chain and the four backends |
| the client | `crates/client`, the types and the HTTP client the command and the tests use |
| the command | `bmo eye see`, `bmo eye read`, `bmo eye watch`, `bmo eye looks`, `bmo eye look`, `bmo eye modes` |
| the unit | `bmo-eye.service` on main-host, deployed from this repository, its health `bmo-eye health` |
| the tools | `eye/see`, `eye/read` and `eye/watch`, placed with the tool host by the deployment |
| the tests | `tests/api.rs`: a stand-in Lab-arena node and the routes walked against it |

The server holds no weights, no card and no VRAM: a look is answered by a Lab-arena instance, and the vision
unit's own cost is a request on the network and an ffmpeg process for a video.

## What is still to build

1. **A suite that answers a look per mode.** `testbed.toml` runs the workspace's tests; a test that answers
   one look per mode against the samples of `docs/modes.md` still wants writing, and it can only run where
   the Lab-arena nodes are reachable.
2. **A hard document, and the four nodes' picture quality against each other,** as 「What is still to
   measure」 below says.

## The tool

`tool.toml` at the repository's root declares three entries, one per mode, and each is answered by the same
binary's `tool` word: the call comes on standard input as the tool host's protocol states it, and the
answer is the look as `eye/see`, `eye/read` and `eye/watch` carry it.

| Entry | Takes | Answers |
|---|---|---|
| `eye/see` | `path`, `question`, `wait` | the look: the answer's text, the backend that answered and what it cost |
| `eye/read` | `path`, `question`, `pages` | the look: the transcription page by page |
| `eye/watch` | `path`, `question`, `fps`, `maxFrames`, `speech` | the look: the answer, its frames and its speech |

The deployment places the binary, the manifest and the schemas with the tool host once the service answers
(`bmo pocket put .tool --placed --service bmo-eye`), so the entries stand from the flip onward and an agent
reaches them by name where its line shows them and with `bmo use eye/see` where it does not.

## The chat line

A picture that arrives in a room reaches the chat agent with the note that names the file, and the agent's
manual tells it what to do with that note. That paragraph is where the picture is sent through bmo-eye: it
names `bmo eye see <path> -q <question>` as the way to read the picture, keeps the program's own Read tool
as the fallback for the day the command cannot be reached, and leaves the decision to answer to the same
line as before. The change stands in the agency's `prompts/agents/chat/manual.md` and is live when the
agency is deployed.

## The server

One binary, `bmo-eye`, a Rust workspace at this repository's root.

- **Shape.** `crates/server` holds the routes, the store and the chain; `crates/client` the types and the
  HTTP client; the binary's first word is the noun `eye`, which is how `bmo` hands it a command.
- **The store** is SQLite in `~/.local/share/bmo/data/eye/`: looks, uploads and the event table. An upload's
  bytes stand in the same folder, named by its id and its sha256.
- **The chain** is code, not configuration: each mode's chain is the list `docs/modes.md` states, and each
  backend answers one trait — whether it takes a look's sources, how it reads them, and what it answers. A
  backend that is not configured answers `unconfigured` and is skipped.
- **The Lab-arena backend** stands once per node in the order n3, n4, n5, n6, each with the node's address,
  key and model name from the settings file. It sends the pictures as `data:` URLs with thinking off.
- **The video path** is ffmpeg on the service's machine for frames and audio, and n2's CPU whisper, reached
  through the jake hub's control API, for the speech. Neither touches a card.
- **The hosted backends** are called by the service itself: OpenRouter with the key the data root holds, and
  Gemini where a key is configured.

**Acceptance:** every route of `docs/api.md` answers its documented codes, and the three modes answer a look
end to end on real inputs with the backend, the node and the time recorded. Both were met on n3 on
2026-10-27: a screenshot in 2.7 s, a photo of a receipt in 2.9 s, a document scan in 5.2 s and a one-minute
video in 62 s, each answered by `lab-arena` on n3, and `cargo test` green over the routes.

## The command

`bmo eye` is one noun in the platform's CLI, handed to `bmo-eye` the way every other noun is: the binary
declares the noun `eye` when asked `nouns`, and the platform finds it on the path.

| Command | Does |
|---|---|
| `bmo eye see <source>… -q <question>` | creates a `see` look and prints its answer |
| `bmo eye read <source>… [-q <question>]` | creates a `read` look and prints the transcription |
| `bmo eye watch <source> -q <question>` | creates a `watch` look and prints its answer |
| `bmo eye looks` | lists the looks, newest first |
| `bmo eye look <lookId>` | prints one look |
| `bmo eye modes` | prints the chains and what stands behind each rung |

`--json` prints the member or the page exactly as the API answers it; `--format`, `--wait`, `--fps`,
`--max-frames`, `--pages` and `--language` carry the mode's options; a look that ended `failed` exits
non-zero with the reason.

## What this build must not cost

- **The Lab-arena text path.** A picture takes nothing from the card and nothing from a text row's tokens per
  second; what it takes is the node's queue while the reader encodes. The measurement is in
  `docs/modes.md`.
- **A card on n2.** Nothing of the vision unit takes a turn; the leftover `eye` occupant and placement of
  stage 1 are to be removed by the operator (`docs/engines.md`, 「The card」).
- **A new model.** No weight is downloaded: the pictures are read by the set that already stands, and the
  speech by the model stage 1 installed on n2.

## What is still to measure

- **The four nodes' picture quality against each other,** on the samples of `docs/modes.md`: n3's general
  pack against the Coder pack behind it.
- **A hard document:** a folded page, handwriting, a phone photo — what the 3-bit general pack does when
  the page is not clean.
- **The nodes under the borrowing engine,** where a picture's answer took 63 s once against 5 s on an idle
  node.
