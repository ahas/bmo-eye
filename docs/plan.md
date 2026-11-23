# The plan, and what of it stands

The vision unit holds no model of its own: the pictures are read by local engine nodes its settings name and
by hosted backends. This document states what stands now and what is still to build.

## What stands

| Part | Where it stands |
|---|---|
| the API | `docs/api.md` and the `docs/openapi.json` its own types generate, served by `bmo-eye` on the operations machine at `127.0.0.1:7435` |
| the modes and their chains | `docs/modes.md`, one chain per mode over the same local engine nodes |
| the engines it looks through | `docs/engines.md`: the local engine nodes, the picture reader, and the speech node's CPU for speech |
| the server | `crates/server`, the binary `bmo-eye`, with the store, the chain and the four backends |
| the client | `crates/client`, the types and the HTTP client the command and the tests use |
| the command | `bmo see`, `bmo read`, `bmo watch`, and `bmo eye looks`, `bmo eye look`, `bmo eye modes` |
| the unit | `bmo-eye.service` on the operations machine, enabled, its health `bmo-eye health` |
| the tools | `eye/see`, `eye/read` and `eye/watch`, placed with the tool host by the deployment |
| the chat line | the chat agent's image note reads the picture through `bmo see` (agency `prompts/agents/chat/manual.md`) |
| the tests | `tests/api.rs`, a module of the one test target `tests/main.rs`: a stand-in local engine node and the routes walked against it |

The server holds no weights, no card and no VRAM: a look is answered by an engine node or a hosted backend, and the vision
unit's own cost is a request on the network and an ffmpeg process for a video.

## What is still to build

1. **A suite that answers a look per mode.** `testbed.toml` runs the workspace's tests; a test that answers
   one look per mode against the samples of `docs/modes.md` still wants writing, and it can only run where
   local engine nodes are reachable.
2. **A hard document, and the nodes' picture quality against each other,** as 「What is still to
   measure」 below says.

## The tool

`tool.toml` at the repository's root declares three entries, one per mode, and each is answered by the same
binary's `tool` word: the call comes on standard input as the tool host's protocol states it, and the
answer is the look as `eye/see`, `eye/read` and `eye/watch` carry it.

| Entry | Takes | Answers |
|---|---|---|
| `eye/see` | `path`, `question` | the look: the answer's text, the backend that answered and what it cost |
| `eye/read` | `path`, `question`, `pages` | the look: the transcription page by page |
| `eye/watch` | `path`, `question`, `fps`, `maxFrames`, `speech` | the look: the answer, its frames and its speech |

A run whose session record says its model reads the mode's material asks `eye/see` and `eye/read` for the
hand-off, and a look no backend answers comes back to it as its material, the question and every rung's
reason, so the agent looks itself; a call from a shell or a schedule, one from a run on a text-only model,
and every `eye/watch` call, answers such a look as an error (`docs/api.md`, 「Handing the input to the
caller」).

The deployment places the binary, the manifest and the schemas with the tool host once the service answers
(`bmo pocket put .tool --placed --service bmo-eye`), so the entries stand from the flip onward and an agent
reaches them by name where its line shows them and with `bmo use eye/see` where it does not. Each entry is
unbounded to the tool host: a call waits for its look to end, however long the node takes to answer.

## The chat line

A picture that arrives in a room reaches the chat agent with the note that names the file, and the agent's
manual tells it what to do with that note. That paragraph is where the picture is sent through bmo-eye: it
names `bmo see <path> -q <question>` as the way to read the picture, keeps the program's own Read tool
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
- **The local backend** stands once per node the settings' `[local]` section lists, in its order, each
  with the node's address, key and model name; none is listed by default. It sends the pictures as `data:` URLs with thinking off.
- **The video path** is ffmpeg on the service's machine for frames and audio, and the speech node's CPU whisper, reached
  through the jake hub's control API, for the speech. Neither touches a card.
- **The hosted backends** are called by the service itself: OpenRouter with the key the data root holds, and
  Gemini where a key is configured.

**Acceptance:** every route of `docs/api.md` answers its documented codes, and the three modes answer a look
end to end on real inputs with the backend, the node and the time recorded. Both are met on the
general pack's node: a screenshot in 2.7 s, a photo of a receipt in 2.9 s, a document scan in 5.2 s and a
one-minute video in 62 s, each answered by the `local` rung, and `cargo test` green over the routes.

## The command

The vision unit is the platform's eye group, and its three verbs are first-class commands: the binary declares
`eye`, `see`, `read` and `watch` when asked `nouns`, so the platform hands `bmo see`, `bmo read` and `bmo watch`
straight to it. The same verbs stand under the unit's own noun as well, and `bmo eye` carries the looks and the
modes beside them.

| Command | Does |
|---|---|
| `bmo see <source>… -q <question>` | creates a `see` look and prints its answer |
| `bmo read <source>… [-q <question>]` | creates a `read` look and prints the transcription |
| `bmo watch <source> -q <question>` | creates a `watch` look and prints its answer |
| `bmo eye see` / `bmo eye read` / `bmo eye watch` | the same three verbs under the unit's own noun |
| `bmo eye looks` | lists the looks, newest first |
| `bmo eye look <lookId>` | prints one look |
| `bmo eye modes` | prints the chains and what stands behind each rung |

`--json` prints the member or the page exactly as the API answers it; `--format`, `--fps`, `--max-frames`,
`--pages` and `--language` carry the mode's options. A verb waits for its look to end and prints it then;
`--wait 0` prints the look as it stands at once instead. A look that ended `failed` exits non-zero with the
reason.

## What this build must not cost

- **The engine nodes' text path.** A picture takes nothing from the card and nothing from a text row's tokens per
  second; what it takes is the node's queue while the reader encodes. The measurement is in
  `docs/modes.md`.
- **A card.** Nothing of the vision unit takes a turn (`docs/engines.md`, 「The card」).
- **A new model.** No weight is downloaded: the pictures are read by the engine nodes that already stand, and
  the speech by the whisper model prepared on the speech node.

## What is still to measure

- **The nodes' picture quality against each other,** on the samples of `docs/modes.md`: the general pack
  against the Coder pack.
- **A hard document:** a folded page, handwriting, a phone photo — what the 3-bit general pack does when
  the page is not clean.
- **The nodes under the borrowing engine,** where a picture's answer took 63 s once against 5 s on an idle
  node.
