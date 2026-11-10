# bmo-eye API

This document and `docs/openapi.json` are the contract of the vision unit's server: one request and one
answer shape for all three modes and for every backend of their chains. It states version `0.1.0-rev.1`,
the version the server carries at its first release; the server is stage 2 of `docs/plan.md` and nothing
serves these routes yet.

The server runs on the operations machine and holds no model and no card (`docs/engines.md`). It calls the
Lab-arena engines on n3 to n6 and the hosted backends itself, and reads a video's speech on n2's CPU. A caller
never reaches an engine directly.

## Wire format

The common wire format and the route rules of bmo hold here (bmo-chat's `docs/conventions.md`, 「The
common wire format」 and 「API design」).

- Field names are camelCase; instants are integers of epoch milliseconds ending in `At`; spans are
  milliseconds ending in `Ms`; a size is a number in the unit its name gives.
- A page is `{"items": [...], "more": boolean}`; the next page is asked with the last item's `id` as
  `after`.
- Every failure is a non-2xx status with a body of `code` and `error`. A look that could not be answered
  is not a failure of the route: it is a member whose `state` is `failed` and whose `error` says why.
- Every route takes `Authorization: Bearer <token>`.

## Health

| Route | Answers |
|---|---|
| `GET /health` | 200 `ready`, `reason` (null when ready), `version` |

`ready` is false while the server starts, while its store is unreadable, and — stated in `reason` — while
no backend of any chain could be reached at the last check. A caller that gets `ready: false` may still
create a look; the look waits.

## What a caller asks

A **look** is one request to look at something: one mode, one question, one set of sources, one answer. A
look is a member of the service and keeps its record after it ends.

| Route | Takes | Answers |
|---|---|---|
| `POST /looks` | query `waitMs?`; body is a look taken in | 201, the `Look` |
| `GET /looks` | query `after?`, `limit?`, `mode?`, `state?` | 200, a `LookPage`, newest first |
| `GET /looks/{lookId}` | | 200, the `Look` |
| `DELETE /looks/{lookId}` | | 204; a running look is stopped and its backend's answer is abandoned |

- **`waitMs`** is how long the route holds the answer open: zero where absent, at most `600000`. The look
  is created either way and runs to its end whatever the hold; a caller that wants the end reads the look
  again until it has ended.
- **`limit`** is 50 where absent and 200 at most.
- **`DELETE`** on a look that already ended answers 204 and changes nothing. The record stays; the look's
  state does not move.

**A look taken in (`POST /looks`):**

| Field | Rule |
|---|---|
| `mode` | Required; `see`, `watch` or `read` |
| `question` | Required; one text, 8000 characters at most, in the caller's language |
| `sources` | Required; one to sixteen sources, each of the shapes below |
| `options` | Optional; the mode's own options, refused where a mode holds none of that name |
| `labels` | Optional; a map of strings, the caller's own metadata |

**A source** is one of three shapes, distinguished by `kind`:

| `kind` | Fields | For |
|---|---|---|
| `upload` | `uploadId` | bytes the caller sent to `POST /uploads` |
| `url` | `url` | an `https` address the server fetches, or downloads with `yt-dlp` where it holds a video page |
| `path` | `path` | an absolute path on the server's node, under the source root the settings name |

`see` and `read` take `upload`, `url` and `path`. `watch` takes `url` and `path`, and an `upload` only
where its bytes are a video file; a YouTube link and any other video page address are `url` sources,
downloaded on the node with `yt-dlp` (`docs/modes.md`, 「watch: video」).

**A look:**

| Field | Holds |
|---|---|
| `id` | Its id, a signed 64-bit integer |
| `mode` | `see`, `watch` or `read` |
| `state` | `queued`, `running`, `done`, `failed` or `stopped` |
| `question` | The question as asked |
| `sources` | The sources as read, each with `kind`, its key, `mediaType`, `bytes`, and `width`/`height` or `durationMs` where they are known |
| `options` | The options the look ran with, defaults filled in |
| `backend` | The backend that answered: `name`, `node`, `model`, or null |
| `chain` | Every backend tried, in order (「The chain report」) |
| `answer` | The answer, `{ "text", "format" }`, or null |
| `speech` | For `watch`: the speech read from the audio, or null (「Speech」) |
| `frames` | For `watch`: `{ "count", "fps", "tokensPerFrame" }`, what was taken from the video |
| `pages` | For `read`: one entry per page read, `{ "page", "text" }` |
| `usage` | `{ "promptTokens", "outputTokens", "firstTokenMs", "totalMs" }` of the answering backend, each null where it does not apply |
| `error` | Why the look failed or was stopped, or null |
| `labels` | The caller's map |
| `createdAt`, `startedAt`, `endedAt` | When it was taken in, when a backend began, when it ended; the last two null until they happen |

`answer.text` is the answer in the language the question was asked in. `answer.format` is `text`,
`markdown` or `json`, as the mode's options asked; `json` is a JSON document in the answer's text.

**States and the moves between them:**

- A look is `queued` when it is taken in and `running` once a backend of its chain begins.
- It ends `done` when a backend answers, `failed` when the chain ran out, and `stopped` when a caller
  deleted it while it stood. `endedAt`, `error` and `backend` follow.
- No move goes back. `DELETE` on an ended look is not a move.

**Errors of the routes:**

| Status | `code` | When |
|---|---|---|
| 400 | `bad-request` | The body or query is not the shape above, a mode is unknown, or an option is not the one the mode takes |
| 401 | `unauthorized` | The token is missing or wrong |
| 404 | `not-found` | No look or upload of that id |
| 404 | `no-route` | No route answers this method and path |
| 413 | `too-large` | An upload over the ceiling, or more sources than the route takes |
| 422 | `unsupported-source` | The mode takes no such source: an image in `watch`, a document that is not a PDF or an image in `read`, a `path` outside the source root |
| 503 | `not-ready` | The server is starting and cannot take a look yet; `Retry-After` says when to ask again |

## Uploads

A caller that holds the bytes sends them here first and names the upload in its look. The server keeps an
upload for as long as a look names it, and removes it a day after its last look ended.

| Route | Takes | Answers |
|---|---|---|
| `POST /uploads` | the bytes as the body, a media type the mode reads, `Content-Length` | 201, the `Upload` |
| `GET /uploads/{uploadId}/content` | the `Range` header | 200 or 206, the bytes |
| `DELETE /uploads/{uploadId}` | | 204 |

**An upload:** `id`, `mediaType`, `bytes`, `sha256`, `createdAt`. A body over `20971520` bytes is 413
`too-large`; a media type outside the set in 「What each mode reads」 is 422 `unsupported-source`.

## Modes

| Mode | Reads | Sources | The mode's options |
|---|---|---|---|
| `see` | images: a photo, a screenshot, a chart, several at once | 1–16 | `format`, `detail` |
| `watch` | video: a file, a video or video-page URL, YouTube included; scenes, on-screen text, speech | 1 | `format`, `fps`, `maxFrames`, `startS`, `endS`, `speech` |
| `read` | documents: a scan, a PDF, a table image | 1–16 | `format`, `pages`, `language`, `tables` |

| Option | Takes | Where unset |
|---|---|---|
| `format` | `text`, `markdown`, `json` | `text` |
| `detail` | `low`, `high` — how much of an image the backend is shown | `high` |
| `fps` | 0.05–4, frames taken from each second of the video | `1` |
| `maxFrames` | 1–1024, the ceiling on frames taken from one video | 128 |
| `startS`, `endS` | a span of the video, in seconds | the whole video |
| `speech` | `true` or `false`, whether the speech is read | `true` |
| `pages` | a list of page numbers, or null for all | all |
| `language` | a BCP 47 language tag the document is expected in | `ko` |
| `tables` | `true` or `false`, whether a table is answered as markdown | `true` |

`GET /modes` answers the three modes as they stand: for each, its `mode`, the `chain` it asks in order,
and for each backend of that chain its `name`, `node` where it has one, `model`, `state` (`ready`, `busy`,
`down`, `unconfigured`) and `reason`. The `lab-arena` backend stands once per node it can reach, so the chain
shown is the order the look will ask them in. A tool host reads it to say whether vision is available at all; an agent reads it to know
which transport a look will take.

## The chain report

A mode's chain is asked in order and the first backend that answers answers the look. The look's `chain`
is every backend that stood before the answer, in the order they were asked:

| Field | Holds |
|---|---|
| `backend` | `lab-arena`, `openrouter`, `gemini` or `caller` |
| `node` | The node the rung stood on, or null: `n3`, `n4`, `n5`, `n6` for the Lab-arena set |
| `model` | The model that backend would use, or null |
| `state` | `answered`, `failed`, `skipped` or `handed-off` |
| `reason` | Why it failed or was skipped, in English, or null |
| `ms` | How long that backend was tried |

- **`skipped`** is a backend the look's sources or options cannot reach — an image in a chain step that
  takes video, a hosted backend with no key configured, a `handed-off` caller that takes no such input.
  Its `reason` names the rule.
- **`failed`** is a backend that was tried and did not answer: the node refused or dropped the
  connection or answered an error, the model refused the input, the hosted call failed. A backend that is
  still reading is waited for, however long it takes. `reason` is the failure as a sentence, and the
  next backend is asked.
- **`answered`** ends the chain; `backend` names that entry.
- **`handed-off`** ends the chain with no answer of the server's: the last rung of a chain whose caller
  takes the input natively. The look ends `done` with `answer` null and `backend.name` `caller`, and the
  tool host or the agent reads the source itself (「Handing the input to the caller」).

## Handing the input to the caller

The last rung of every chain is the caller's own model, and it is used when that model takes the input
natively: an image by Claude, an image or a video by Codex where its transport carries it.

- **The look says so:** `backend.name` is `caller`, `answer` is null, and the chain's last entry is
  `handed-off` with the reason naming what the caller takes.
- **The tool host answers with the material,** not with prose: the sources as the caller can read them —
  an upload's bytes, a URL, a path — the question, and the mode. The agent then looks itself.
- **A caller that takes nothing over 30 seconds of video** is not handed a video: the chain's `caller`
  entry is `skipped` and the look ends `failed` where nothing else answered.
- **`caller` is never the first rung.** A look asks the local model first, then a hosted one, then the
  caller, as `docs/modes.md` states for each mode.

## Events

The service serves one stream, `GET /events`, and no other route streams. Events name a look and its
change: `look.created`, `look.running`, `look.done`, `look.failed`, `look.stopped`. A reader narrows with
`kinds` and resumes with `Last-Event-ID`. The service keeps the last 4,096 events; a resume from before
them is 410 `gone`.

**The API streams no answer tokens.** A look's answer is whole when its state is `done`; a caller that
wants the text before then has none to read, and the tool host shows the caller which backend is being
asked from the events. A backend that streams its own tokens is read to the end by the server before the
answer is written.

## Speech

`watch` reads the video's speech apart from its frames, so the answer rests on what was said as well as
on what was shown. `speech.text` is the transcript and `speech.language` its language; `speech.model`
names what read it. A video with no audio, or with `speech: false`, answers `speech` null. The transcript
is the backend's own when the backend hears audio (Gemini), and n2's CPU speech model otherwise
(faster-whisper `large-v3` int8, `docs/engines.md`).

## Limits

| What | Ceiling |
|---|---|
| An upload | 20 MiB, or 2 GiB where the mode is `watch` |
| Sources in one look | 16 for `see` and `read`, 1 for `watch` |
| A link downloaded with `yt-dlp` | 2 hours, 1080p at most |
| `waitMs` | 600,000 |
| Frames in one `watch` look | 12 where none is asked, 1024 at most |
| One look's pictures | 16, and each is about 300 tokens of the node's context |
