# The plan for stage 2

Stage 1 settled what bmo-eye is: three modes, one request and answer shape, one model per mode on n2, and a
chain of backends behind each mode. Its documents are `docs/api.md`, `docs/modes.md` and `docs/n2.md`; its
measurements are in `docs/n2.md` and in the archive document this stage leaves.

Stage 2 is a separate item after this plan is reviewed. Nothing here is built or deployed by the stage 1
job.

## What stage 2 builds

1. **The Rust server on n2** that serves `docs/api.md`: looks, uploads, modes, events and health, with the
   three mode chains behind them.
2. **The `bmo eye` noun** in the platform's CLI, with the sub-verbs `watch`, `see` and `read`.
3. **The tool the tool host places,** so an agent calls a look through the host and not through a shell.
4. **The agency rule** that an agent looks at an image or a video through bmo-eye first.
5. **The deployment**: the arbiter declaration and its rules entry, the n2 install, and the testbed
   declaration that puts it all on the node.

## The server

One binary, `bmo-eye`, a Rust workspace at this repository's root, serving the routes of `docs/api.md` on
n2's loopback behind the platform's token.

- **Shape.** `server/` holds the routes, the store and the chain; `client/` the Rust client; `plugin/` the
  MCP server and the `bmo-eye` skill; `runtime/` the three model drivers; `crates/eye-core` the types the
  routes, the client and the plugin share.
- **The store** is SQLite in the service's data root, `~/.local/share/bmo-eye/`: looks, uploads and the
  event table. An upload's bytes stand in the same folder, named by the upload's id and its sha256.
- **The chain** is code, not configuration: each mode's chain is a fixed list of backends in the order
  `docs/modes.md` states, and each backend answers one trait — whether it takes a look's sources, how it
  reads them, and what it answers. A backend that is not configured answers `unconfigured` and is skipped;
  the chain never grows by adding a row to a table.
- **The model drivers** are processes the server starts, stops and talks to over HTTP on the node's
  loopback: `llama-server` for the two 27B models, the OCR runtime for DeepSeek-OCR-2, and
  `faster-whisper` for speech. The server never forks a model's own code and never loads weights into its
  own address space.
- **The card** is taken through the arbiter's turn client on the node (`~/bmo-card/turn.py` as it stands, or
  the Rust client) as `eye`, for as long as a model of the look's mode stands on the card. A model that has
  answered stays resident for `keepMs` (default 600000) with its turn rested, and gives way when any other
  occupant asks. The declaration is `docs/n2.md`, 「The declaration」.
- **The processes are stopped** by the master when a lower-tier occupant or a claimant asks for the card:
  the declaration yields by its process, and the adapter's `stop` takes the models off the card and
  `start` brings the resident one back.

**Acceptance:** every route of `docs/api.md` answers its documented codes, `docs/openapi.json` is generated
from the server's own types and a test fails when it differs, and the three chains answer a look end to end
on n2 with the arbiter holding the card.

## The CLI

`bmo eye` is one noun in the platform's CLI, handed to the `bmo-eye` binary the way every other noun is:
the noun registry in the CLI's own repository gains one entry, and the server's repository ships the binary.

| Command | Does |
|---|---|
| `bmo eye see <source>… [-q question]` | creates a `see` look and prints its answer |
| `bmo eye read <source>… [-q question]` | creates a `read` look and prints its answer |
| `bmo eye watch <source> [-q question]` | creates a `watch` look and prints its answer |
| `bmo eye looks` | lists the looks, newest first |
| `bmo eye look <lookId>` | prints one look |
| `bmo eye modes` | prints the three chains and what stands behind each |

- A source is a path, an `https` URL or a YouTube link; a path is uploaded first, and a YouTube link is
  handed to the server as a `url`.
- `--json` prints the member or the page exactly as the API answers it; `--format markdown|json` sets the
  answer's format; `--wait <seconds>` sets `waitMs`; `--speech/--no-speech`, `--fps`, `--max-frames`,
  `--pages` and `--language` carry the mode's options.
- The command exits 0 on a look that ended `done` and non-zero, naming the reason, on `failed` or
  `stopped`.

**Acceptance:** each sub-verb answers against the n2 server, `--json` matches the API byte for byte, and a
path, a URL and a YouTube link are each exercised.

## The tool

The service registers its own tool through its deployment, `bmo-pocket put --service bmo-eye`, with a
`tool.toml` at this repository's root. One entry per mode, so a harness sees three tools with the three
questions its schema asks:

| Tool | Answers |
|---|---|
| `eye/see` | looks at images and answers a question about them |
| `eye/read` | reads a document page or a table and answers its text |
| `eye/watch` | looks at a video and answers a question about it, with its speech |

- Each entry's input is the look's body of its mode; its output is the `Look`, `answer.text` first.
- A tool call carries the harness's own files: the host uploads a path the harness names, so an agent
  screenshots or downloads into a file and calls the tool with it.
- The MCP server is `bmo-eye mcp`, and the skill beside it tells a harness which mode to ask for what.

**Acceptance:** `bmo use eye/see --arg path=… --arg question=…` answers from a fresh session, and a call
from the tool host's own MCP session answers the same look shape.

## The agency rule

The agents look at an image or a video through bmo-eye first unless their instruction says otherwise. That
is one rule in the agency's role prompts and one line in the tool session's brief; it is not a change to
any agent's own repository.

**Acceptance:** a fresh agent session with an image path in its instruction calls `eye/see` before it opens
the file itself, and a session told to look for itself does not.

## The node

The install on n2 is `docs/n2.md` as it stands: llama.cpp, the three model files, the speech model, the
data root and the units. What stage 2 adds:

- **The service's unit,** `bmo-eye.service` on n2, which starts the server and nothing else: the model
  processes are the server's children and are stopped with it.
- **The arbiter declaration, applied by the operator.** `eye.card.toml` stands in this repository;
  `docs/n2.md` states it and the operator adds `eye` to the card rules at tier 1, and the repository root
  to the master's declaration roots.
- **The service's own placement,** `POST /placements` with `eye` on `n2`, so the node's occupants carry it.

**Acceptance:** a look answered on n2 leaves the card empty when it ends, `gguf-llm` back on it, and the
master's events showing `occupant-stopped` and `occupant-started` for it as they do today.

## The testbed

The repository's `testbed.toml` declares what a testbed run brings up: the server from this tree, the
node's units, and the checks that read back one look per mode against the samples stage 1 measured. A test
that needs the card is marked as one and skipped where the card cannot be had.

**Acceptance:** `bmo testbed run` on this repository brings the server up on n2 and answers three looks,
one per mode, from the samples folder.

## The order of the work

1. The server's types and routes with the chain as a trait, `see` first: it needs one model and no video.
2. The CLI noun and its client calls; the archive's own samples become the checks.
3. `read`: the OCR runtime driver and its page loop.
4. `watch`: frame extraction, the speech driver and the long-video path.
5. The tool, the skill and the agency rule.
6. The deployment: declaration, unit, testbed, and the operator's rules entry.

Each step ends with `bmo check` green and the modes' samples answering, and the whole with the suite and
the n2 run.

## What stage 2 must re-measure

- **The swap between the three models** with the real server, not a hand-run model: the time from a `see`
  look arriving while `watch`'s model rests to its first token.
- **The keep policy's cost on the other occupants**: how long `gguf-llm` is off the card across a day of
  looks, from the master's own events.
- **The resident set under the new shapes**: whether a 27B and the OCR runtime can stand together at any
  point (stage 1 measured them alone), and whether the `see` model is fast enough to be `read`'s fallback.
- **Gemini and OpenRouter** from the node, with a key, for the input each takes.
