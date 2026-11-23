# The engines the vision unit looks through

bmo-eye holds no model of its own. It looks through the Qwen3.8-Flash-Next set that the Lab-arena deployment
already runs on n3, n4, n5 and n6, and it reads a video's speech on n2's CPU. This document states that set, how a
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
`~/ahalab-arena/model/mmproj-Qwen3.8-Flash-Next-BF16.gguf`, 907 MB (866 MiB). The reader is
`bin/lab-arena-vision` in the Lab-arena deployment's folder, the engine runs with `--vision`, and the config's `vision`
section names the reader, the projector, the model's first shard and the reader's limits: `gpu` false,
300 tokens per picture, eight threads.

## How a picture reaches the engine

A caller sends a picture as OpenAI's `image_url` part — a `data:` URL, an `http(s)` URL, or a local path on
the node — or as an Anthropic image block. bmo-eye sends a `data:` URL, base64 of the file as it stands.
The gateway passes request bodies through unchanged, so nothing between the caller and the engine rewrites
a picture.

Two settings are not optional:

- **Thinking is off:** `chat_template_kwargs.enable_thinking` is `false`. A node that leaves it on spends
  its answer on the thought and returns no answer at all. The field is the Lab-arena engine's own: OpenRouter
  ignores it, and is asked with its own `reasoning` setting turned off instead (`docs/api.md`, 「The chain
  report」).
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
in 63 s, because a Lab-arena instance shares its card with the borrowing engine and yields it. A look waits for
its node's answer however long that takes; a caller holds the route open with `waitMs` and reads the look
again until it ends.

**What the vision unit costs the text path:** nothing of the card, and about 1.1 GiB of the node's RAM for
the reader once it has read a picture. A text request sent while four pages were being read answered at the
same tokens per second and waited 4 to 8 s longer in the queue; the numbers are in `docs/modes.md`,
「What a picture costs the text path」.

## The workbench on n2

Three things a look needs that the Lab-arena set does not give:

| What | Where | Card | Why there |
|---|---|---|---|
| `ffmpeg` | main-host | none | cutting a video into frames and its audio track |
| `faster-whisper` `large-v3` int8 | n2, `~/eye/venv-whisper`, CPU, and 2.9 GiB of weights in the node's shared model cache | none | reading a video's speech, installed in stage 1 and reused |
| `yt-dlp` | main-host, `~/.local/share/bmo/data/eye/venv` | none | fetching a YouTube link or a video page's file |

Speech is read on the node's CPU: 81 s for a 66 s clip and 537 s for an 11 min 32 s one, measured in stage
1. It takes no card and no VRAM, so a video's speech never competes with the Lab-arena nodes. It does take the
node's cores and RAM beside the two services that live there, so a long video is read at the speed the node
has spare.

n2 is reached from the eye service through the jake hub's control API, the only road to it. The service
posts the audio in base64, runs the installed whisper on the node with `--service` nothing and no card
turn, and reads the transcript back.

## What n2 runs

n2 is the workbench and a card machine of its own, an RTX 4070 of 12,282 MiB. The figures below are the
node's and its driver's own, read on the node on 2026-10-28 at driver 595.58.03, the card at
10,791 MiB in use and 1,491 MiB free while they were read. A size is what `du` on the node reports.

### The services that take the card

| Unit | What it serves | On disk | On the card now | Peak while serving |
|---|---|---|---|---|
| `hy-mt2-engine` | a translation engine: `llama.cpp` over `HY-MT2-7B-Q8_0`, a 32,768 token context, four slots, an 8-bit cache, every layer offloaded | 7.5 GiB, the model file 7.4 of it; the `llama.cpp` build it runs is counted under `~/kiki-llm` below | 10,140 MiB | 10,140 MiB, flat: the context is taken at start and a line does not move it |
| `bmo-voice` | a voice server over `VoxCPM2` with one LoRA checkpoint at a time, `bmo-005-e-1000` now | 6.8 GiB, its environment 6.3 and the checkpoints with their references 0.45 | 218 MiB, its process's context | 7.1 GB, the voice server's own record of a line spoken on an RTX 4070, and 6,908 MiB on an RTX 3060 Ti; not measurable here, see below |

The voice server keeps the model in page-locked host memory, 5.39 GiB of the node's RAM, and moves it onto
the card only while a line is spoken; idle it holds its context alone. The base weights it loads are no
longer on the node's disk: the directory its checkpoint names is gone from the shared model cache, so that
5.39 GiB in RAM stands as the only copy and a restart of the unit would find nothing to load.

The 1,491 MiB free is where a third service would have to fit, and it is below the 7.2 GiB a voice model
load asks for before it starts. The translation engine holds the node's shared card lock for as long as it
runs, so the arbiter answers a voice line with "the card's memory stayed taken" and the voice server, which
will not stop another service to make room, answers the caller with a 503. A line spoken while the
translation engine runs is therefore not measurable here: the peak in the table is the voice server's own
record and not a figure taken on this node.

### The services that take no card

| Unit | What it is | On disk | On the card |
|---|---|---|---|
| `bmo-card` | the node's card arbiter, which opens the device and takes no memory of it | 1.1 MiB, and a 12 GiB record file under its data that nothing has written since 2026-10-06 | 0 MiB |
| `hy-mt2-tunnel` | the reverse tunnel that stands the engine's port 8050 on the operations machine's loopback 18050 | nothing | none |
| `jake-agent` | the hub's agent on the node, the road this unit reaches n2 by | 3.5 MiB | none |

### The bundles on the disk that nothing runs

| Path | What it is | On disk |
|---|---|---|
| `~/eye` | stage 1 of this unit, kept: 64 GiB of model files no unit on the node loads, an OCR environment, the whisper environment and its samples | 71 GiB |
| `~/yue2` | song generation: its models, the LoRAs trained on them, the songs it rendered and four environments | 72 GiB |
| `~/kiki-llm` | three quantisations of one `RVN` model, a remote cache and the `llama.cpp` build the translation engine runs | 47 GiB |
| `~/workspace` | training projects of the node account, a crack detector among them kept twice, one trainer archived beside itself, and a plate reader | 54 GiB |
| `~/research` | one training run's outputs | 50 GiB |
| `~/.cache/huggingface` | the shared model cache: `large-v3` for this unit's speech, 2.9 GiB, and `large-v3-turbo` for the ear service, 1.6 GiB | 7.1 GiB |
| `~/comfy` | a `ComfyUI` checkout and its environment, with no checkpoint under it | 9.2 GiB |
| `~/bmo-laya` | the training project of the card's `laya` occupant | 5.9 GiB |
| `~/bmo-voice-train` | the `VoxCPM` training checkout and its experiment outputs | 2.7 GiB |
| `~/bmo-ear` | the ear service's environment, its unit gone and its weights in the cache above | 0.4 GiB |

Stage 1 is the largest claim this unit makes on the node and nothing there reads it, so it is the first
room to give back where the node wants some.

### The rest of the machine

Everything else on n2 is not an AI service: nginx, the garage store with its console container, mariadb,
postgresql 14, rabbitmq, docker and containerd, the two `dl-` viewers, `matterport-thief` on port 8888 with
33 GiB of scraped tours, and n6's home mounted at `~/repos/david`.

| What | Size |
|---|---|
| `/` | 916 GiB, 571 in use, 299 free |
| the node account's home | 460 GiB, of which 23 GiB of download caches and 12 GiB the arbiter's record file |
| docker | about 13 GiB of images and 13 GiB of build cache |
| `/mnt/data1` | a second disk of 2.8 TB, 1.8 TB of it reported in use, whose directory reads empty for the node account and is therefore counted nowhere above |

## Where the vision unit stands

`bmo-eye` runs on main-host, the operations machine, and holds no card and no model:

| What | Where |
|---|---|
| the server | main-host, `127.0.0.1:7435`, `bmo-eye.service` |
| the release | `~/.local/share/bmo-eye/deploy`, the one path every caller uses |
| the settings | `~/.local/share/bmo/data/eye/settings.toml`, linked into the release |
| its store | `~/.local/share/bmo/data/eye/`, SQLite and the uploads' bytes |
| the Lab-arena nodes | `127.0.0.1:18101`–`18104`, the loopback tunnels the Lab-arena deployment keeps |
| the nodes' keys | each node's `key_file` in the settings, `~/.local/share/bmo/data/eye/keys/<node>.key` where none is named |
| n2's CPU | through the jake hub's control API at `127.0.0.1:8791` |
| OpenRouter | `https://openrouter.ai/api/v1`, the settings' `hosted.openrouter.address`; the key in `~/.local/share/bmo/data/env/openrouter.key` |
| Gemini | configured by no key on this machine |

The service needs no GPU, no VRAM and no card turn. A look is answered by whichever node answers first,
and the eye service never stops, starts or yields anything on a node.

**The unit** is `bmo-eye.service`, a user unit of main-host, marked `X-BMO-Deploy=bot` so every deployment
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

The card master holds no placement for it, and the node's occupants are `trainer ear gguf-llm laya-train song
voice`. The repository has no declaration file, and the card rules do not name `eye`.

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
