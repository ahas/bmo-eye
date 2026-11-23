# The engines the vision unit looks through

bmo-eye holds no model of its own. It looks through the local engine nodes its settings name, each an
OpenAI-compatible chat server that reads pictures, then through the hosted backends, and it reads a video's
speech on the CPU of a node reached through the jake hub. This document states what such a node must offer,
how a picture reaches it, what a picture costs, and where the vision unit itself stands.

## The local engine nodes

The settings' `[local]` section lists the nodes, in the order the chain asks them, each with a `name`, the
`address` it answers on, the `model` name it carries and the `key_file` holding its key. No node is named by
default: with the list empty, every look starts at the hosted backends.

A node answers OpenAI's `/v1/chat/completions` with a vision-language model and its projector loaded — a
llama.cpp-compatible engine over a GGUF build of the Qwen3.8-Flash-Next family and its `mmproj` file is
what this unit was measured against — and its `/health` answers `images: true` while its picture reader is
configured. A node that reads no pictures is shown as down in the chain report.

The engine's picture reader in the measured set runs on the node's CPU with 300 tokens per picture and
eight threads, the projector beside the weights.

## How a picture reaches the engine

A caller sends a picture as OpenAI's `image_url` part — a `data:` URL, an `http(s)` URL, or a local path on
the node — or as an Anthropic image block. bmo-eye sends a `data:` URL, base64 of the file as it stands.
The gateway passes request bodies through unchanged, so nothing between the caller and the engine rewrites
a picture.

Two settings are not optional:

- **Thinking is off:** `chat_template_kwargs.enable_thinking` is `false`. A node that leaves it on spends
  its answer on the thought and returns no answer at all. The field is the local engine's own: OpenRouter
  ignores it, and is asked with its own `reasoning` setting turned off instead (`docs/api.md`, 「The chain
  report」).
- **`max_tokens` is set:** the answer's length is the caller's to bound, and a document's transcription
  needs the room.

## What a picture costs

The reader runs on the node's CPU. It takes nothing from the card, and a picture becomes at most 300 tokens
of the model's context, so a picture is cheap in context and expensive in time: the reader encodes before
the engine prefills.

Measured on one node of the general pack, through the node's own `/v1/chat/completions`, one look at a time:

| Sent | Prompt tokens | Wall |
|---|---|---|
| text only | 7–30 | 0.9–2.3 s |
| one 1440×900 screenshot, answered | 314 | 2.7 s |
| one 1769×2369 photo of a receipt, answered | 310 | 2.9 s |
| one 1654×2339 document scan, transcribed | 308 | 5.2 s |
| one table image, transcribed | 332 | 6 s, and 63 s while the borrowing engine held the card |
| twelve frames of a minute of video, answered | 3,649 | 6.6 s of the model, 62 s of the look |

The spread is the node's, not the caller's: the same question against the same picture answered in 6 s and
in 63 s, because that node shares its card with a borrowing engine and yields it. A look waits for
its node's answer however long that takes; a caller holds the route open with `waitMs` and reads the look
again until it ends.

**What the vision unit costs the text path:** nothing of the card, and about 1.1 GiB of the node's RAM for
the reader once it has read a picture. A text request sent while four pages were being read answered at the
same tokens per second and waited 4 to 8 s longer in the queue; the numbers are in `docs/modes.md`,
「What a picture costs the text path」.

## The speech workbench

Three things a look needs that the engine nodes do not give:

| What | Where | Card | Why there |
|---|---|---|---|
| `ffmpeg` | the service's machine | none | cutting a video into frames and its audio track |
| `faster-whisper` `large-v3` int8 | the speech node, an environment prepared with `scripts/whisper.py`, CPU, and 2.9 GiB of weights in the node's model cache | none | reading a video's speech |
| `yt-dlp` | the service's machine, `~/.local/share/bmo/data/eye/venv` | none | fetching a YouTube link or a video page's file |

Speech is read on the node's CPU: 81 s for a 66 s clip and 537 s for an 11 min 32 s one, as measured. It
takes no card and no VRAM, so a video's speech never competes with the engine nodes. It does take the
node's cores and RAM beside whatever else lives there, so a long video is read at the speed the node has
spare.

The speech node is the hub agent the settings' `speech.agent` names; none is named by default, and speech
stands only once one is. It is reached through the jake hub's control API, the only road to it. The service
carries the audio with the hub's upload, runs the installed whisper on the node with no card turn, and
reads the transcript back.

## Where the vision unit stands

`bmo-eye` runs on the operations machine and holds no card and no model:

| What | Where |
|---|---|
| the server | `127.0.0.1:7435`, `bmo-eye.service` |
| the release | `~/.local/share/bmo-eye/deploy`, the one path every caller uses |
| the settings | `~/.local/share/bmo/data/eye/settings.toml`, linked into the release |
| its store | `~/.local/share/bmo/data/eye/`, SQLite and the uploads' bytes |
| the local engine nodes | the addresses the settings' `[local]` section lists, none by default |
| the nodes' keys | each node's `key_file` in the settings |
| the speech node | the hub agent `speech.agent` names, through the jake hub's control API |
| OpenRouter | `https://openrouter.ai/api/v1`, the settings' `hosted.openrouter.address`; the key in `~/.local/share/bmo/data/env/openrouter.key` |
| Gemini | configured by no key on this machine |

The service needs no GPU, no VRAM and no card turn. A look is answered by whichever node answers first,
and the eye service never stops, starts or yields anything on a node.

**The unit** is `bmo-eye.service`, a user unit of the operations machine, marked `X-BMO-Deploy=bot` so every deployment
restarts it and waits on its health. Its `ExecStart` runs `bmo-eye serve` from the release, and
`bmo-eye health` — the same binary — is what the deployment's health command runs. `bmo eye` finds the
binary through the link `~/.local/bin/bmo-eye`, which the deployment makes before the flip, so the noun is
served from any line on the machine.

**The deployment** is the repository's own (`testbed.toml`, `[deploy]`): the release at
`~/.local/share/bmo-eye/deploy`, the settings linked from the data root, the unit restarted, the health
waited on, and then the repository's tool placed with the tool host
(`bmo pocket put .tool --placed --service bmo-eye`, `docs/plan.md` 「The tool」).

## The card

**The vision unit is not a card occupant.** It takes no turn, loads no weights and holds nothing on any
card.

The card master holds no placement for it, the repository has no declaration file, and the card rules do
not name `eye`.

## What this set does not do

- **Precise OCR of a hard page.** The general pack read a clean Korean scan with one wrong word and a
  table exactly; a folded, handwritten or low-resolution page was not measured, and the 3-bit quantisation
  is where that would show first.
- **A long video at a high frame rate.** A picture is 300 tokens and seconds of the reader's time, so a
  minute of video is a handful of frames, not sixty. What a frame rate costs is in `docs/modes.md`.
- **Speech on the engine nodes.** They read pictures and text only; the speech is the speech node's CPU.
