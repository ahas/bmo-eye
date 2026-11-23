# bmo-eye API

This document and `docs/openapi.json` are the contract of the vision unit's server: one request and one
answer shape for all three modes and for every backend of their chains. The OpenAPI document is written
from the types the routes take and answer, and a test holds the copy kept beside this document to what
the code writes; a change to a route or a body writes it again with
`cargo run -p eye-server --example openapi > docs/openapi.json`.

The version is the repository's one, `0.1.0-rev.1`, kept in the Cargo manifests, in the health answer and
in the OpenAPI document; it moves only at a release the owner declares, as bmo-chat's
`docs/conventions.md` 「Versions」 states. The server is built and deployed: it answers these routes on
the operations machine at `127.0.0.1:7435`, and what of the plan is still to build stands in `docs/plan.md`,
「What is still to build」.

The server runs on the operations machine and holds no model and no card (`docs/engines.md`). It calls the
local engine nodes its settings name and the hosted backends itself, and reads a video's speech on the
speech node's CPU. A caller
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

`GET /health` answers 200 whatever it finds: its answer is a fresh check of what stands now, reading its
store and asking the state of every mode's chain — the same check `GET /modes` spells out.

- **`ready` is false, and `reason` names which,** while its store cannot be read and while no backend of any
  mode's chain could be reached. A server that has not opened its store and bound its listener answers no
  route at all, which is what a caller waits on through the unit's own start.
- **A caller that gets `ready: false` may still create a look** where no backend could be reached: the look
  is created and waits. Where its store cannot be read no look can be created, and `POST /looks` answers 503
  `not-ready` with `Retry-After: 5`, the seconds to wait before asking again.

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

An `upload` source's id is also taken as `upload_id`, the name some callers send; the server writes it as
`uploadId` wherever it answers a source.

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

- **A `json` answer is a JSON document and nothing else.** The backend's answer is read as JSON, either
  whole or as the first fenced code block of it that reads, and the document alone is kept as
  `answer.text`; an object and an array are both documents. An answer that holds no document that reads
  is a `failed` rung, and the next backend is asked.
- **An answer is whole or it is no answer.** A backend that ends its answer for any reason but its own end,
  the token cap above all, has failed its rung, whatever format was asked.

**States and the moves between them:**

- A look is `queued` when it is taken in and `running` once a backend of its chain begins.
- It ends `done` when a backend answers or hands the input to a caller that asked for it, `failed` when the
  chain ran out, and `stopped` when a caller deleted it while it stood. `endedAt`, `error` and `backend`
  follow.
- **A look the chain ran out on names every rung's reason in its `error`,** each backend with its node and
  whether it failed or was skipped, so a caller that reads only the error reads why.
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
| 500 | `failed` | A store call behind the route failed; `error` is the store's sentence |
| 503 | `not-ready` | Its store cannot be read, so no look can be created; `Retry-After` says when to ask again |

## Uploads

A caller that holds the bytes sends them here first and names the upload in its look. The server keeps an
upload for as long as a look names it, and removes it a day after its last look ended.

| Route | Takes | Answers |
|---|---|---|
| `POST /uploads` | the bytes as the body, a media type the mode reads, `Content-Length` | 201, the `Upload` |
| `GET /uploads/{uploadId}/content` | the `Range` header | 200 or 206, the bytes |
| `DELETE /uploads/{uploadId}` | | 204 |

**The bytes go out in the media type the upload was sent as,** and the route says it takes byte ranges with
`Accept-Ranges: bytes`. A `Range` header names one span of them, `bytes=` its first and last byte counted in,
and the answer is 206 with that span and a `Content-Range` header naming it in the same shape. A span that
does not read, or that starts at or past the upload's last byte, is ignored and the whole bytes answered with
200.

**An upload:** `id`, `mediaType`, `bytes`, `sha256`, `createdAt`. A body over `20971520` bytes, or over
`2147483648` bytes where its media type is a video, is 413 `too-large`, whatever size the body arrives at;
a media type outside the set in 「What each mode reads」 is 422 `unsupported-source`.

## Modes

| Mode | Reads | Sources | The mode's options |
|---|---|---|---|
| `see` | images: a photo, a screenshot, a chart, several at once | 1–16 | `format`, `detail` |
| `watch` | video: a file, a video or video-page URL, YouTube included; scenes, on-screen text, speech | 1 | `format`, `fps`, `maxFrames`, `startS`, `endS`, `speech` |
| `read` | documents: a scan, a PDF, a table image | 1–16 | `format`, `pages`, `language`, `tables` |

| Option | Takes | Where unset |
|---|---|---|
| `format` | `text`, `markdown`, `json` | `text` |
| `handOff` | `true` or `false`, whether the caller's own model takes the sources where no backend of the server answers (「Handing the input to the caller」); every mode takes it | `false` |
| `detail` | `low`, `high` — how much of an image the backend is shown | `high` |
| `fps` | 0.05–4, frames taken from each second of the video | `1` |
| `maxFrames` | 1–1024, the ceiling on frames taken from one video | 12 |
| `startS`, `endS` | a span of the video, in seconds | the whole video |
| `speech` | `true` or `false`, whether the speech is read | `true` |
| `pages` | a list of page numbers, or null for all | all |
| `language` | a BCP 47 language tag the document is expected in | `ko` |
| `tables` | `true` or `false`, whether a table is answered as markdown | `true` |

`GET /modes` answers the three modes as they stand: for each, its `mode`, the `chain` it asks in order,
and for each backend of that chain its `name`, `node` where it has one, `model`, `state` (`ready`, `busy`,
`down`, `unconfigured`) and `reason`. The `local` backend stands once per node the settings name, so the chain
shown is the order the look will ask them in. A tool host reads it to say whether vision is available at all; an agent reads it to know
which transport a look will take.

## The chain report

A mode's chain is asked in order and the first backend that answers answers the look. The look's `chain`
is every backend that stood before the answer, in the order they were asked:

| Field | Holds |
|---|---|
| `backend` | `local`, `openrouter`, `gemini` or `caller` |
| `node` | The node the rung stood on, or null: the node's name from the settings for a `local` rung |
| `model` | The model that backend would use, or null |
| `state` | `answered`, `failed`, `skipped` or `handed-off` |
| `reason` | Why it failed or was skipped, in English, or null |
| `ms` | How long that backend was tried |

- **`skipped`** is a backend the look's sources or options cannot reach — an image in a chain step that
  takes video, a hosted backend with no key configured, a caller that did not ask for the hand-off or
  takes no such input. Its `reason` names the rule.
- **`failed`** is a backend that was tried and did not answer: the node refused or dropped the
  connection or answered an error, the model refused the input, the hosted call failed, the answer had no
  words, was cut before its end, or under `json` held no JSON document. A backend that is still reading is
  waited for, however long it takes. `reason` is the failure as a sentence, and the next backend is asked.
- **Every backend is asked with its thinking off,** each in its own request field: a local engine through
  its chat template, OpenRouter through its reasoning setting. A model that thinks spends the answer's
  token cap on the thinking and is cut before it answers.
- **`answered`** ends the chain; `backend` names that entry.
- **`handed-off`** ends the chain with no answer of the server's: the last rung of a chain whose caller
  asked for the hand-off and takes the input natively. The look ends `done` with `answer` null and
  `backend.name` `caller`, and the agent reads the source itself (「Handing the input to the caller」).

## Handing the input to the caller

The last rung of every chain is the caller's own model, and it is used only when the look's `handOff` says
that model takes the input natively: an image by Claude, an image or a video by Codex where its transport
carries it. The server cannot tell a caller's model from the request, so the caller states it.

- **A look that does not ask for the hand-off is never handed off.** A service or a tool that has no model
  of its own leaves `handOff` false; its chain's `caller` entry is `skipped`, and the look ends `failed` with
  every rung's reason where nothing else answered.
- **The look says so:** `backend.name` is `caller`, `answer` is null, and the chain's last entry is
  `handed-off` with the reason naming what the caller takes.
- **The eye tool asks for the hand-off where the caller's record says its model reads the mode's
  material.** A call of the tool carries the tool host's session record, whose issuer writes a `reads`
  label naming the media the run's model reads natively — words of `image`, `pdf` and `video`, judged by
  bmo-agency from the road the run goes out on (`docs/provider-order.md`, 「The media a model reads」). The
  tool sets `handOff` true for a `see` call whose record reads `image` and a `read` call whose record reads
  `image` or `pdf`, since the handed answer carries a picture where the source is one and names the document
  for the caller's own harness where it is one. A record with no `reads` label — a person's `bmo use`, a
  scheduled call, a run routed to a model that reads no picture — and every `watch` look leave it false:
  the first hands its answer to a reader that cannot open the material, and no harness here reads a video in
  a tool's answer. A service that asks the API itself states its own options and
  leaves it false.
- **The eye tool answers a handed-off look with the material,** not with prose. Its text says that no
  backend answered, gives the question, names each source the way the agent opens it — a path on this
  machine with its media type, or an address — and ends with every rung's reason. A picture at a path whose
  base64 fits in 5 MB, the most a model's API takes for one picture, also stands in the answer as an image
  piece, which a run holding no file reader of its own sees there; a document and an address stand as their
  path and URL alone. The structured part is the look itself. The agent then looks itself.
- **A caller that takes nothing over 30 seconds of video** is not handed a video: the chain's `caller`
  entry is `skipped` and the look ends `failed` where nothing else answered.
- **`caller` is never the first rung.** A look asks the local model first, then a hosted one, then the
  caller, as `docs/modes.md` states for each mode.

## Events

The service serves one stream, `GET /events`, and no other route streams. Events name a look and its
change: `look.created`, `look.running`, `look.done`, `look.failed`, `look.stopped`.

- **A reader resumes** with the id of the last event it read as the query `after`, and the stream answers
  at most 200 events to one pull before it waits for the next. The route takes no narrowing: a reader that
  wants one kind reads the stream and drops the rest.
- **The service keeps every event it has written** in its store, so a resume from any id it holds stands
  and nothing is answered `gone`.

**The API streams no answer tokens.** A look's answer is whole when its state is `done`; a caller that
wants the text before then has none to read, and the tool host shows the caller which backend is being
asked from the events. A backend that streams its own tokens is read to the end by the server before the
answer is written.

## Speech

`watch` reads the video's speech apart from its frames, so the answer rests on what was said as well as
on what was shown. `speech.text` is the transcript and `speech.language` its language; `speech.model`
names what read it. A video with no audio, or with `speech: false`, answers `speech` null. The transcript
is the backend's own when the backend hears audio (Gemini), and the speech node's CPU model otherwise
(faster-whisper `large-v3` int8, `docs/engines.md`).

## Limits

| What | Ceiling |
|---|---|
| An upload | 20 MiB, or 2 GiB where its media type is a video |
| Sources in one look | 16 for `see` and `read`, 1 for `watch` |
| A link downloaded with `yt-dlp` | 2 hours, 1080p at most |
| `waitMs` | 600,000 |
| Frames in one `watch` look | 12 where none is asked, 1024 at most |
| One look's pictures | 16, and each is about 300 tokens of the node's context |
