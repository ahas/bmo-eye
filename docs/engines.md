# The engines the vision unit looks through

bmo-eye holds no model of its own. It looks through the Qwen3.8-Flash-Next set that bmo-lab-arena already
runs on n3, n4, n5 and n6, and it reads a video's speech on n2's CPU. This document states that set, how a
picture reaches it, what a picture costs, and where the vision unit itself stands.

## The set

Four Lab-arena instances answer on their own node, each with the OpenAI-compatible API its own document
states (`lab-arena/docs/deploy.md`). The picture reader is the engine's, built beside it as `lab-arena-vision`,
and each instance's `/health` answers `images: true` while a reader is configured.

| Node | Address | Model name | Weights | The node's part |
|---|---|---|---|---|
| n3 | `192.0.2.18:8080` | `qwen3.8-flash-next-iq3_s` | `IQ3_S/Qwen3.8-Flash-Next-GSQ-RCO-IQ3_S-00001-of-00002.gguf` 54,817,524,224 B and `-00002` 28,800,138,432 B | general pack, the best of the four at Korean |
| n4 | `192.0.2.19:8080` | `qwen3.8-flash-next-coder-iq1_m` | `IQ1_M` two shards, 55 GB | Coder pack |
| n5 | `192.0.2.20:8080` | `qwen3.8-flash-next-coder-iq1_m` | the same | Coder pack |
| n6 | `192.0.2.21:8080` | `qwen3.8-flash-next-coder-iq1_m` | the same, with `data/packs` 25 GB | Coder pack |

The projector stands beside the weights on every node,
`/home/labuser/ahalab-arena/model/mmproj-Qwen3.8-Flash-Next-BF16.gguf`, 907 MB (866 MiB). The reader is
`/home/labuser/bmo-lab-arena/bin/lab-arena-vision`, the engine runs with `--vision`, and the config's `vision`
section names the reader, the projector, the model's first shard and the reader's limits: `gpu` false,
300 tokens per picture, eight threads.

## How a picture reaches the engine

A caller sends a picture as OpenAI's `image_url` part — a `data:` URL, an `http(s)` URL, or a local path on
the node — or as an Anthropic image block. bmo-eye sends a `data:` URL, base64 of the file as it stands.
The gateway passes request bodies through unchanged, so nothing between the caller and the engine rewrites
a picture.

Two settings are not optional:

- **Thinking is off:** `chat_template_kwargs.enable_thinking` is `false`. A node that leaves it on spends
  its answer on the thought and returns no answer at all.
- **`max_tokens` is set:** the answer's length is the caller's to bound, and a document's transcription
  needs the room.

## What a picture costs

The reader runs on the node's CPU. It takes nothing from the card, and a picture becomes at most 300 tokens
of the model's context, so a picture is cheap in context and expensive in time: the reader encodes before
the engine prefills.

Measured on n3 on 2026-10-27, through the node's own `/v1/chat/completions`, one look at a time:

| Sent | Prompt tokens | Wall |
|---|---|---|
| text only | 7–30 | 0.9–2.3 s |
| one 1440×900 screenshot, answered | 314 | 2.7 s |
| one 1769×2369 photo of a receipt, answered | 310 | 2.9 s |
| one 1654×2339 document scan, transcribed | 308 | 5.2 s |
| one table image, transcribed | 332 | 6 s, and 63 s while the borrowing engine held the card |
| twelve frames of a minute of video, answered | 3,649 | 6.6 s of the model, 62 s of the look |

The spread is the node's, not the caller's: the same question against the same picture answered in 6 s and
in 63 s, because a Lab-arena instance shares its card with the borrowing engine and yields it. A caller that
needs a bound sets `waitMs` and reads the look again.

**What the vision unit costs the text path:** nothing of the card, and about 1.1 GiB of the node's RAM for
the reader once it has read a picture. A text request sent while four pages were being read answered at the
same tokens per second and waited 4 to 8 s longer in the queue; the numbers are in `docs/modes.md`,
「What a picture costs the text path」.

## The workbench on n2

Three things a look needs that the Lab-arena set does not give:

| What | Where | Card | Why there |
|---|---|---|---|
| `ffmpeg` | main-host | none | cutting a video into frames and its audio track |
| `faster-whisper` `large-v3` int8 | n2, `~/eye/venv-whisper`, CPU | none | reading a video's speech, installed in stage 1 and reused |
| `yt-dlp` | main-host, `~/.local/share/bmo-eye/venv` | none | fetching a YouTube link |

Speech is read on the node's CPU: 81 s for a 66 s clip and 537 s for an 11 min 32 s one, measured in stage
1. It takes no card and no VRAM, so a video's speech never competes with the Lab-arena nodes.

n2 is reached from the eye service through the jake hub's control API, the only road to it. The service
posts the audio in base64, runs the installed whisper on the node with `--service` nothing and no card
turn, and reads the transcript back.

## Where the vision unit stands

`bmo-eye` runs on main-host, the operations machine, and holds no card and no model:

| What | Where |
|---|---|
| the server | main-host, `127.0.0.1:7435`, `bmo-eye.service` |
| its store | `~/.local/share/bmo-eye/`, SQLite and the uploads' bytes |
| the Lab-arena nodes | `127.0.0.1:18101`–`18104`, the loopback tunnels the Lab-arena deployment keeps |
| the nodes' keys | `~/.local/share/bmo-lab-arena/keys/n3.key` … `n6.key` |
| n2's CPU | through the jake hub's control API at `127.0.0.1:8791` |
| OpenRouter | `https://openrouter.ai/api/v1`, the key in `~/.local/share/bmo/data/env/openrouter.key` |
| Gemini | configured by no key on this machine |

The service needs no GPU, no VRAM and no card turn. A look is answered by whichever node answers first,
and the eye service never stops, starts or yields anything on a node.

## The card

**The vision unit is not a card occupant.** It takes no turn, loads no weights and holds nothing on any
card.

What was done on 2026-10-27: the stage 1 placement was removed through the card master's own verb
(`POST /placements` with `kind: remove`), and the node's occupants went from `trainer ear eye gguf-llm
laya-train song voice` to `trainer ear gguf-llm laya-train song voice`. The repository's declaration file,
`eye.card.toml`, is gone, and the card rules never named `eye` — stage 1's proposal to add it at tier 1 is
withdrawn with this document.

What is left for the operator: the `…/bmo/services/eye` entry this repository once added to the master's
`CARD_DECLARATIONS` can be dropped at its next restart. With no declaration file in the repository it
declares nothing, so the entry is only a stale line in a settings file.

## What this set does not do

- **Precise OCR of a hard page.** The general pack read a clean Korean scan with one wrong word and a
  table exactly; a folded, handwritten or low-resolution page was not measured, and the 3-bit quantisation
  is where that would show first.
- **A long video at a high frame rate.** A picture is 300 tokens and seconds of the reader's time, so a
  minute of video is a handful of frames, not sixty. What a frame rate costs is in `docs/modes.md`.
- **Speech on the nodes.** The Lab-arena set reads pictures and text only; the speech is n2's CPU.
- **The 27B models stage 1 installed on n2.** They are still on the node's disk
  (`~/eye/models`, 46 GB) and nothing uses them. They may be deleted; nothing in this repository names
  them any more.
