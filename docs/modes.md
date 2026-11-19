# The three modes and their chains

bmo-eye reads an image, a document or a video and answers a question about it. One mode names what is read,
and each mode has its own chain of backends. The request and answer shape is the same for all of them
(`docs/api.md`).

| Mode | CLI | Reads | The engine it is answered by |
|---|---|---|---|
| `see` | `bmo see` | images: photos, screenshots, charts, several at once | a Lab-arena instance on n3 to n6 |
| `read` | `bmo read` | documents: scans, PDFs, pages of text and tables | a Lab-arena instance on n3 to n6 |
| `watch` | `bmo watch` | video: a file, a video or video-page URL, YouTube included; scenes, on-screen text, speech | a Lab-arena instance for the frames, n2's CPU for the speech |

The engines are `docs/engines.md`. No mode has a model of its own: every mode reaches the Qwen3.8-Flash-Next
set that bmo-lab-arena already runs, and the vision unit itself holds no weights and no card.

## The rule every mode follows

- **The chain is asked in order, and the first backend that answers answers the look.** A rung that cannot
  be reached, or that fails, is recorded and the next is asked. The look's `chain` is the whole attempt
  (`docs/api.md`, 「The chain report」).
- **The local rung is the same four instances in every mode, n3 first.** n3 carries the general pack and is
  the best of the four at Korean; n4, n5 and n6 carry the Coder pack and are asked in that order when n3
  does not answer.
- **A picture is 300 tokens and seconds of the node's CPU.** That is what bounds a look: a few images in a
  `see` or `read`, a handful of frames in a `watch`.
- **An answer cut before its end is no answer.** A backend that stops at its token cap, or answers no words,
  or under `format: json` answers no JSON document, has failed its rung, and the next is asked.
- **The caller's own model is the last rung,** and only when the look's `handOff` says it takes the input
  natively. That rung does not answer: it hands the source to the caller, which looks itself. A look that
  does not ask for it ends `failed` with every rung's reason where nothing else answered.
- **A look is answered in the language it was asked in.** A document read by `read` is transcribed as it
  stands, in its own language.
- **A mode refuses what it does not read** with 422 `unsupported-source`, naming the mode and the source.

## see: images

Images are read as they are, up to sixteen in one look, in one request. A chart, a screenshot and a photo
are the same input; what differs is the question.

**What it takes.** PNG, JPEG and WebP, up to 20 MiB each; an animated image is read at its first frame. The
image is sent as it stands: the node's reader scales it to the picture it encodes, 300 tokens at most.

**The chain**

| Order | Backend | Answers |
|---|---|---|
| 1 | `lab-arena` on n3 | the answer, from `qwen3.8-flash-next-iq3_s` |
| 2 | `lab-arena` on n4, then n5, then n6 | the answer, from `qwen3.8-flash-next-coder-iq1_m` |
| 3 | `openrouter` | the answer, from the model the settings name |
| 4 | `caller` | hands the images over |

**What it answers with.** A sentence or a few, in the language of the question; code, numbers and names as
they stand in the picture. What the four nodes give is measured in `docs/engines.md`: a chart's largest bar
and its value, a screenshot's order number and amount, a sign's road name and its two numbers, a receipt's
shop, business number and total — each answered correctly.

## read: documents

A document is read for its text: a scan, a PDF page, an image of a table. The answer is the text itself, in
the document's own order, and a table as markdown where `tables` is true.

**What it takes.** PDF, PNG, JPEG and TIFF, up to 16 sources and 20 MiB each. A PDF is read page by page;
`pages` narrows it. A PDF with a text layer is still read as an image: what the caller wants is what the
page shows.

**The chain**

| Order | Backend | Answers |
|---|---|---|
| 1 | `lab-arena` on n3 | the transcription |
| 2 | `lab-arena` on n4, then n5, then n6 | the transcription, the Coder pack's |
| 3 | `openrouter` | the transcription |
| 4 | `caller` | hands the pages over |

**What it answers with.** The page's text, top to bottom, with its headings and list marks. A table comes
back as a markdown table. `pages` carries one entry per page read. The transcription's accuracy is the
node's: a clean Korean scan was read with one wrong word in thirteen lines, and a table of five rows
exactly, on n3.

**A hard page is the mode's limit.** 300 tokens is the whole picture's budget, so a dense page, a folded
page, handwriting or a phone photo of a page is read worse than the clean scans measured here. The mode
answers anyway, and the caller reads what came back; it never answers with an empty transcript in place of
one, and a page it could not read is answered as a failed rung with the reason.

## watch: video

A video is read as frames and as speech. The frames go to a Lab-arena instance as pictures; the speech goes to
n2's CPU and comes back as a transcript; the answer is written from both.

**What it takes**

| Input | How it is read |
|---|---|
| a YouTube link | `yt-dlp` on the service's machine downloads it at 720p at most, then it is read as a file |
| an `https` URL to a video file | fetched by the service itself, then read as a file |
| an `https` URL to a video page | downloaded to a file by the same `yt-dlp`, then read as a file |
| an uploaded or local file | read where it stands |

- **An address is fetched directly when it serves a media type the look can read,** the content type or
  the URL's extension saying video, audio, image or PDF.
- **A page address is pulled with `yt-dlp` instead:** the page of a host known to hold a video
  (instagram.com, tiktok.com, x.com, twitter.com, facebook.com, vimeo.com, threads.net), and any other
  address whose bytes come back as something a look cannot read, the HTML page of a login wall being
  the common one. The extractor decides what the address holds; an address it cannot resolve fails the
  look with its reason.

**Frames.** `fps` frames are taken from each second, from `startS` to `endS`, up to `maxFrames`; where
`fps` would take more than `maxFrames`, frames are taken evenly across the span instead, and `frames.fps`
reports the rate that was used. A frame is a JPEG at its own size, at most 1280 pixels on its long edge.

**The frame budget is the reader's.** A picture costs about 300 tokens and several seconds of a node's CPU
to encode, so a minute of video is answered from a dozen frames, not sixty: the default `maxFrames` is 12.
`frames.count`, `frames.fps` and `frames.tokensPerFrame` say what was actually sent.

**Speech.** The audio track is extracted and read on n2's CPU by faster-whisper `large-v3` int8, in the
video's language, with timestamps; `speech.text` is the transcript and `speech.model` names the model. A
video with no audio, or with `speech: false`, answers `speech` null. The transcript goes into the prompt the
frames are asked with, so the answer rests on what was said as well as on what was shown.

**The chain**

| Order | Backend | Answers |
|---|---|---|
| 1 | `lab-arena` on n3 | the answer, from the frames and the transcript |
| 2 | `lab-arena` on n4, then n5, then n6 | the same |
| 3 | `gemini` | the video natively, where a key is configured and the source is a link or an upload |
| 4 | `caller` | hands the source over, for a video a caller's own model takes |

- **The `gemini` rung is skipped with no key,** and skipped for a path on the service's own machine, which a
  hosted API cannot reach.
- **The `caller` rung is skipped for any video over 30 seconds** or any source the caller's transport cannot
  carry.

## What a picture costs the text path

A picture is read on the node's CPU, before the engine prefills, and takes nothing from the card. Measured
on n3 on 2026-10-27, a text-only request answered four times on an idle node and four times while a `read`
look of four pages was being answered by the same node:

| The node | Wall | Generated |
|---|---|---|
| idle | 0.9–2.3 s | 46–59 tokens/s |
| while four pages were being read | 6.3–10.5 s | 40–51 tokens/s |

The card's rows do not move: the tokens per second stay in the node's own spread. What moves is the queue —
a picture is a request like any other, and a text request behind it waits for the reader. So a caller that
mixes text and pictures on one node pays the pictures in waiting, not in the text's speed. The reading
itself was 2.7 s for a screenshot, 2.9 s for a photo and 5.2 s for a scan, each including the model's
answer.

## Which mode a caller uses

- **A picture of a screen, a scene or a person: `see`.** The question decides what is wanted.
- **A page of text or a table: `read`.** It answers the text itself, which a `see` answer only paraphrases.
- **Anything that moves: `watch`.** One moment of a video read as an image is a `see` look on an extracted
  frame, which is the honest way to ask about that moment.

An agent looks at an image or a video through bmo-eye first unless its instruction says otherwise; what it
does with the answer is its own. The rule that places the tool and tells the agents so is the part of
`docs/plan.md` that still stands open.
