# The three modes and their chains

bmo-eye reads an image, a video or a document and answers a question about it. One mode names what is
read, and each mode has its own model on the node and its own chain of backends. The request and answer
shape is the same for all of them (`docs/api.md`).

| Mode | CLI | Reads | The node's model |
|---|---|---|---|
| `watch` | `bmo eye watch` | video: a file, a URL, a YouTube link; scenes, on-screen text, speech | Qwen3.6-27B, llama.cpp |
| `see` | `bmo eye see` | images: photos, screenshots, charts, several at once | Qwen3.8-27B, llama.cpp |
| `read` | `bmo eye read` | documents: scans, PDFs, pages of text and tables | DeepSeek-OCR-2 3B, its own runtime |

The node's model is the first rung of each chain. What was measured for each is `docs/n2.md`; this
document says how a chain is walked, what each rung takes, and when a mode refuses an input.

## The rule every mode follows

- **The chain is asked in order, and the first backend that answers answers the look.** A rung that
  cannot be reached, or that fails, is recorded and the next is asked. The look's `chain` is the whole
  attempt (`docs/api.md`, 「The chain report」).
- **The caller's own model is the last rung,** and only when it takes the input natively. That rung does
  not answer: it hands the source to the caller, which looks itself.
- **One mode, one model on the card.** The three node models do not stand on the card together; a look
  that asks a mode whose model is not resident pays the move (`docs/n2.md`, 「Sharing one card」).
- **A look is answered in the language it was asked in.** The question decides the answer's language, and
  a document read by `read` is transcribed as it stands, in its own language.
- **A mode refuses what it does not read** with 422 `unsupported-source`, naming the mode and the source.

## watch: video

A video is read as frames and as speech. The frames go to the node's video model as images, one request
with as many images as the options allow; the speech goes to a speech model and comes back as a
transcript. The answer is written from both.

**What it takes**

| Input | How it is read |
|---|---|
| a YouTube link | `yt-dlp` downloads the video at 1080p at most, then it is read as a file |
| an `https` URL to a video file | downloaded the same way |
| an uploaded or local file | read where it stands |

**Frames.** `fps` frames are taken from each second, from `startS` to `endS`, up to `maxFrames`; where
`fps` would take more than `maxFrames`, frames are taken evenly across the span instead, and `frames.fps`
reports the rate that was used. A frame is a JPEG of the video's own size, at most 1280 pixels on its
long edge. `frames.count` and `frames.tokensPerFrame` are what the backend was actually given, the
second being the image tokens the backend's own accounting reports for one frame.

**Speech.** The audio track is extracted and read by faster-whisper `large-v3` on the node, in the
video's language, with timestamps; `speech.text` is the transcript and `speech.model` names the model. A
backend that takes audio itself (Gemini) answers its own transcript and the node's speech model is not
run.

**The chain**

| Order | Backend | Takes | Answers |
|---|---|---|---|
| 1 | `n2` | frames as images, the transcript in the prompt | the answer, from Qwen3.6-27B |
| 2 | `gemini` | a YouTube URL or an uploaded video, natively, with audio | the answer, and its own transcript |
| 3 | `caller` | the source, when the caller's model takes video natively | hands the source over |

- **The `gemini` rung is asked only for a link or an upload,** never for a path on the node: the hosted
  API takes no local file. With no Gemini key configured the rung is `skipped`.
- **The `caller` rung is skipped for any video over 30 seconds** or any source the caller's transport
  cannot carry; Claude takes no video at all.
- **A 10-minute video is read at a lower frame rate than a 1-minute one** for the same `maxFrames`; what
  that costs in time and what it misses is measured in `docs/n2.md`.

## see: images

Images are read as they are, up to sixteen in one look, in one request. A chart, a screenshot and a
photo are the same input; what differs is the question.

**What it takes.** PNG, JPEG and WebP, up to 20 MiB each; an animated image is read at its first frame.
An image is scaled so that its long edge is at most 2048 pixels for `detail: high` and 768 for `low`.

**The chain**

| Order | Backend | Takes | Answers |
|---|---|---|---|
| 1 | `n2` | the images as images | the answer, from Qwen3.8-27B |
| 2 | `openrouter` | the images as images | the answer, from the model the chain names |
| 3 | `caller` | the images, when the caller's model takes images | hands the sources over |

- **Every `caller` in use today takes an image** (Claude, Codex), so this rung is reached more often than
  in the other modes: it is what an agent falls back to when the node is busy and no key is configured.
- **The node's `see` model is the slowest rung and the best reader of a chart or a table image**; for a
  document page the `read` mode is faster and more exact, and the `see` mode is for what OCR does not
  answer.

## read: documents

A document is read for its text: a scan, a PDF page, an image of a table. The answer is the text itself,
in the document's own layout where the backend can keep it, and a table as markdown where `tables` is
true.

**What it takes.** PDF, PNG, JPEG and TIFF, up to 16 sources and 20 MiB each. A PDF is read page by page;
`pages` narrows it. A PDF with a text layer is still read as an image: what the caller wants is what the
page shows.

**The chain**

| Order | Backend | Takes | Answers |
|---|---|---|---|
| 1 | `n2` (read) | page images | the transcription, from DeepSeek-OCR-2 |
| 2 | `n2` (see) | page images | the transcription, from Qwen3.8-27B, as a fallback for the page it refuses |
| 3 | `openrouter` | page images | the transcription |
| 4 | `caller` | the page images | hands the sources over |

- **The second rung is the `see` model, not a second OCR engine.** It is asked only where the read model
  failed on a page, and its answer is marked in `pages` as the other model's.
- **The read model is one page at a time;** a look over several pages asks it once per page, and `pages`
  carries what each answered.
- **A document that is not a scan or an image** — a text file, a word processor's file — is refused 422
  `unsupported-source`; a caller converts it or hands it to an agent that reads text.

## Where the models stand

| Model | Runtime | Card | Modes |
|---|---|---|---|
| Qwen3.6-27B Q4_K_M + mmproj | llama.cpp | n2, one 27B at a time | watch |
| Qwen3.8-27B Q4_K_M + mmproj | llama.cpp | n2, one 27B at a time | see, and read's second rung |
| DeepSeek-OCR-2 3B BF16 | its own runtime (transformers, CUDA) | n2, alone on the card | read |
| faster-whisper large-v3 | CTranslate2, CPU or card | n2 | watch's speech |

The card is taken from bmo-arbiter as the tier-1 occupant `eye`, and given back when no look is being
answered past the hold time (`docs/n2.md`, 「The card」).

## Which mode a caller uses

- **A picture of a screen, a scene or a person: `see`.** The question decides what is wanted; a chart is a
  `see` sample, and so is a receipt read for one number.
- **A page of text or a table: `read`.** It is exact where `see` paraphrases, and it is faster.
- **Anything that moves: `watch`.** A single frame of a video read as an image is a `see` look on an
  extracted frame, which is the honest way to ask about one moment of it.

An agent looks at an image or a video through bmo-eye first unless its instruction says otherwise; what it
does with the answer is its own. The rule that places the tool and tells the agents so is stage 2 of
`docs/plan.md`.
