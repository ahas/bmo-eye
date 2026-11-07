# Reference material for a drawing request

**Status: design only.** This document settles how a drawing request finds, checks and injects reference
material. It writes no code, and nothing in bmo-eye serves it today. It is filed here because the request
reached the platform through this repository; bmo-eye's part in the design is the vision check alone. The
unit that draws is new, and is called `bmo-paint`: its noun is `paint`, never `draw`, because `bmo draw` is
the receiver's fair draw and keeps that name.

## The request, and the words

- **A drawing request** asks for one picture from a prompt. It may name a **subject**: a character, a work,
  a style, an object, a place or a real person.
- **A subject record** is what the platform knows of that subject: a short knowledge entry, a long dossier,
  and a set of checked reference images.
- **A reference set** is the images of one subject that passed the checks, with where each came from and
  what licence it carries.
- **A road** is how a picture is made: plain generation from the prompt, reference injection through an
  IP-Adapter, or face-identity injection through IP-Adapter FaceID or InstantID.

The owner's example is a drawing "like 치이카와". That is a character subject: it has a written appearance
and, if a record is built, a reference set of pictures of the character. The design below applies to any
subject, not only to that one.

## Where a subject's record lives

Three stores hold three different things. None of them holds a copy of what another serves.

| Store | Holds | Authoritative for |
|---|---|---|
| bmo-brain | one subject and one knowledge memory | the key, the aliases, the status, the pointers |
| bmo-artifact | one `record` document | the human-readable dossier, sources, licence lines, consent |
| bmo-paint | the reference images' exact bytes and the manifest | the bytes, their digests, the vision verdicts |

The brain holds no image and no file: its members are text. The archive keeps a dossier and a preview; it
re-encodes a picture to AVIF at most 2048 pixels on the long edge, so it is not the place for the bytes a
diffusion model is fed. bmo-paint keeps those bytes, because they are its input.

### The brain entry

- **The subject** is `type` `paint-subject`, with a key that is stable for the life of the record:
  `<slug>-<hash8>`, the slug made from the name's Latin form where one exists, and `hash8` the first eight
  hexadecimal digits of the SHA-256 of the NFC name as the requester wrote it. The original name, every
  alias and the kind stand in the subject's `description` and in the memory's labels. The brain finds a
  subject by `type` and `key`, never by name.
- **The memory** is `type` `knowledge`, key `paint-subject/<key>`, written with `actorName` `paint` and
  `importance` 7. Its text is a gist of at most 2,000 characters: what the subject is, its canonical
  appearance in a few lines, and the constraints that matter to a drawing. The labels carry the machine
  facts:

  | Label | Holds |
  |---|---|
  | `paint.schema` | `1` |
  | `paint.subject-key` | the subject's key |
  | `paint.kind` | `character`, `work`, `style`, `object`, `place`, `person` or `other` |
  | `paint.aliases` | the aliases as a JSON array |
  | `paint.record` | the artifact document's id |
  | `paint.reference-set` | the bmo-paint reference set's id |
  | `paint.status` | `complete`, `partial` or `description-only` |
  | `paint.licences` | the distinct licence ids, comma-separated |
  | `paint.checked-at` | when the set was last checked, epoch milliseconds |
  | `paint.consent` | the consent record's artifact id, for a real person only |

- A correction writes a new version through `POST /brains/{brainId}/memories/{memoryId}/versions`; the old
  version is superseded and stays. Nothing is deleted. The subject's `description` is updated with
  `PATCH /brains/{brainId}/subjects/{subjectId}` when the name or aliases change.
- Which brain is a setting: `[brain] address`, `token_file`, `brain_id`. bmo-paint does not choose an
  owner's brain by name.

### The archive record

- One artifact document of kind `record`, filed with `actorName` `paint` and `scope` `paint`. Its title is
  "paint subject: <name>". Its labels repeat `paint.subject-key`, `paint.schema`, `paint.kind`, `paint.status`
  and `paint.licences`, so a lookup by label finds it without reading bodies.
- The body has these sections in order: what the subject is; appearance and the features that must survive;
  aliases; the reference table; the sources the description rests on; the consent, where the subject is a
  real person; and the research document's link.
- The **reference table** has one row per image: SHA-256, role, dimensions, source page, direct address,
  creator, publisher, retrieved at, licence id and attribution, and the vision verdict with its reason.
- One accepted image is attached as the cover, so the document has a picture to show. The attached file is
  a preview, not the reference byte: the archive's own image handling is enough for a cover.
- A share link is opened with `POST /shares` for the owner; it is the link the room shows. One live link per
  document, as bmo-artifact states.

### The bmo-paint store

- Reference bytes stand under `~/.local/share/bmo/data/paint/references/<subjectKey>/<sha256>.<ext>`, one
  file per digest. The manifest beside them is the working index and carries the same fields as the
  reference table plus the verdict and the eye look's id.
- A content-addressed file is never renamed or overwritten. A byte that fails a check is discarded after
  the check; only its digest, address and reason stay in the manifest.
- The archive keeps the dossier and one preview; bmo-paint keeps the bytes. That is not a duplicate,
  because the archive serves a re-encoded preview and never the bytes the model is given. If the owner later
  wants the archive to be the only keeper of exact reference bytes, bmo-artifact needs a preserved-original
  attachment, a byte digest and a licence field; that is a later change, not part of this design.

### Looking a subject up

1. Normalise the requested name: NFKC, trim, collapse internal whitespace, and build the key and the alias
   list. The requester's own spelling is kept as an alias.
2. Ask the brain for the subject by `type` and `key`. If it stands, read its live `paint-subject/<key>`
   memory. If the memory names an artifact record and a reference set, read both.
3. If the brain has no exact key, search the brain by the name and the aliases (`q` on subjects and
   memories). A near match is a candidate, not an answer.
4. Search artifact by `q` and by the `paint.subject-key` label. A record found here but missing from the
   brain is repaired: the brain entry is written from the archive record.
5. If more than one subject answers to the name, stop with `subject-ambiguous` and name the candidates. The
   chat agent asks the owner one question. The pipeline never picks the first match for a real person or
   for adult content.
6. If nothing stands, the record is `missing` and the research run below begins.

A record is fresh for 30 days. It is refreshed earlier when it has fewer than two accepted references, when
`paint.status` is not `complete`, when the owner asks for a refresh, or when a new image of the subject is
supplied. A complete, fresh record is not researched again.

The lookup's answer is one of: `complete` (a fresh set with at least one accepted reference), `partial` (a
record exists but the set is thin or stale), `description-only` (the description stands, no licensed image
does), `missing`, `ambiguous` or `refused`.

### Writing a new record

The order is fixed so a failure leaves a truthful state:

1. The exact bytes are committed to the bmo-paint store and the manifest is written.
2. The artifact record is filed, or patched by id where one already stands for the key. Its cover is
   attached. The document's id and share link are read back.
3. The brain subject is find-or-made, its description updated, and the knowledge memory written or
   versioned with the artifact id, the reference-set id and the status.
4. Only after all three stand is the record marked `complete`.

A step that fails leaves `partial`; the next run retries and writes no duplicate. The key, the record id and
the digest set make the write idempotent. A record built from the owner's own picture, with no research, takes
the same path with the licence `owner-supplied`.

## When no record stands: the research run

The missing record becomes a research run on bmo-research: `POST /research-runs` with a new key,
`references`. The run is asynchronous and is waited on with no clock; a drawing that asked for `references`
`required` waits for it, and one that asked for `auto` waits too when the subject is the point of the
request.

```json
{
  "key": "references",
  "query": "치이카와 (Chiikawa) — a reference dossier for drawing",
  "asker": "paint",
  "options": {
    "subjectKey": "chiikawa-1a2b3c4d",
    "language": "ko",
    "candidateLimit": 12,
    "requireLicence": true,
    "imageEngines": ["openverse", "wikicommons.images", "unsplash"]
  }
}
```

The run must bring back a description and a candidate list, not prose alone. Its document is filed in
bmo-artifact with labels `paint.subject-key`, `paint.schema` `references-v1` and the usual research labels;
the last fenced block of its body is this JSON, which bmo-paint parses and validates:

```json
{
  "subject": {"name": "치이카와", "aliases": ["Chiikawa", "ちいかわ"], "kind": "character"},
  "description": "a short, factual account of the appearance",
  "features": ["the features a drawing must keep"],
  "constraints": ["what must not be drawn, and why"],
  "candidates": [
    {
      "url": "https://…/picture.png",
      "pageUrl": "https://…/page",
      "title": "…",
      "creator": "…",
      "publisher": "…",
      "retrievedAt": 0,
      "licence": {"id": "CC-BY-4.0", "url": "https://…", "attribution": "…"},
      "mediaType": "image/png",
      "width": 1200,
      "height": 1200,
      "note": "what this picture contributes"
    }
  ],
  "sources": [{"url": "https://…", "what": "what it supports"}],
  "unresolved": ["what could not be found, and why"]
}
```

The runner must not invent a candidate, a licence or a dimension. A candidate it cannot support goes into
`unresolved`. bmo-paint re-reads every address and every licence line before it trusts a candidate: the
research result is a lead, not a permission.

### Licence

The reference set is not a scrapbook. A picture is injected only where its licence permits this use, or
where the owner supplied it. The rest may inform the written description but never the injection.

| Licence | Injected | Why |
|---|---|---|
| public domain, `CC0-1.0`, `PDM-1.0` | yes | no condition |
| `CC-BY-*` | yes | attribution is recorded and shown |
| `CC-BY-SA-*` | yes | attribution and share-alike are recorded |
| Unsplash licence | yes, with the photographer's attribution | the licence permits it |
| the owner's own picture, `owner-supplied` | yes | his own assertion, recorded |
| written permission, filed in artifact | yes | the permission record is cited |
| `CC-BY-NC-*` | description only | the use may not be non-commercial |
| `CC-BY-ND-*` | description only | generation is a derivative |
| unknown, all rights reserved, no licence line | description only | no permission is known |

A character's official art is usually all rights reserved. Where no licensed picture exists, the record is
`description-only`: the pipeline can draw an interpretation from the written appearance, and the result says
so. It never claims a likeness the references did not support.

## The reference images: fetch, check, keep

### Fetch

- bmo-paint asks bmo-explorer for the bytes through its `fetch_raster` operation: one address, JPEG, PNG,
  GIF, WebP, AVIF or TIFF, 5 MiB at most, answered as bytes and a media type. The explorer checks the
  address and the redirects; bmo-paint writes no fetcher of its own.
- The provenance kept is the candidate's `pageUrl`, the direct `url`, the creator, the publisher, the
  licence and the moment of the read. A direct address without a page is kept as `pageUrl` null and is not
  injected unless the licence line is explicit.
- Pictures are deduplicated by SHA-256 and by a 64-bit perceptual hash: the same picture at two sizes is one
  reference.
- A candidate larger than 4096 pixels on the long edge is not rejected; it is downscaled to 2048 for the
  check and the original is kept. A candidate smaller than 512 pixels on the short edge is rejected as too
  small to carry the subject.

### Check

The check is one bmo-eye look. bmo-paint sends the bytes to `POST /uploads`, one upload per picture, then
one `POST /looks`:

- `mode` `see`, `options.format` `json`, `options.detail` `high`;
- the sources are the uploads, at most eight in one look;
- the question is fixed, in English, so the answer's shape does not drift:

> You are checking reference images for an image-generation reference set. For each image, in the order it
> was sent, answer one JSON object with: `index`; `depictsSubject` (`yes`, `no`, `uncertain`);
> `subjectConfidence` (0 to 1); `kind` (`photograph`, `illustration`, `screenshot`, `other`);
> `singleSubject` (true or false); `realPerson` (`yes`, `no`, `uncertain`); `apparentAge` (`child`, `teen`,
> `adult`, `unknown`); `hasWatermarkOrText` (true or false); `sexualContent` (`none`, `suggestive`,
> `explicit`); `usableAsReference` (true or false); `features` (the visual features this picture shows);
> `reason` (one sentence). The subject is: {name}. What is known of it: {description}. Do not answer with
> anything outside the JSON array.

- A picture is **accepted** when `depictsSubject` is `yes` with confidence at least 0.7, `usableAsReference`
  is true, it has no watermark or interface text across the subject, it is a single subject, and the
  licence is in the injected column above. For a fictional subject, `realPerson` may be `no` or
  `uncertain`; for a real person, it must be `yes` and the consent record must stand.
- A picture is **rejected** for any other answer, and the reason is kept. The rejected bytes are discarded.
- Four accepted references are enough for a character; the pipeline asks for at most eight and stops when
  it has four. A set with one accepted reference is `partial`.

The same look, with a final-image question, checks the rendered picture before it is shown: it must depict
the subject or the requested scene, must contain no apparent minor in adult content, must not be a
screenshot or carry a watermark, and must not be sexual content when `adult` is false. A rejected output is
retried once with a new seed; a second rejection fails the draw with `output-rejected`. The vision model is
not a safety classifier; it is the platform's eye, and the design does not claim more than that.

### Keep

- Accepted bytes go to the bmo-paint store under their digest. The manifest keeps the verdict, the eye look
  id, the address, the licence and the dimensions.
- The archive gets one cover preview and the dossier; the room gets a signed link to the result, never the
  reference set.
- A subject's set is deleted only by the owner through bmo-paint; the artifact record and the brain memory
  are then marked `description-only`, not deleted.

## The injection on n2

### What stands

The drawing happens in the ComfyUI installation on n2, which the card master already places as the service
`comfyui`. bmo-paint holds no card and no model; it asks the master for a card of `comfyui` and uses the
installation it finds.

| What | As it stood on 2026-11-08 |
|---|---|
| the node | n2, an RTX 4070 of 12,282 MiB |
| the card declaration | service `comfyui`, 11,000 MiB, priority 20, tier 2, `yield_by` process, one node |
| the installation | `~/ComfyUI`, 56 GiB, ComfyUI 0.44.0 at commit `1332fab`, Python 3.10.12, torch 2.14.1+cu130 |
| the launcher | `--lowvram --reserve-vram 0.5`, under `BMO_CARD_SERVICE=comfyui` and the node's card lock |
| the SDXL checkpoint | `TalmendoXLBeta_v1.1.safetensors`, 6,938,040,682 B, an SDXL architecture |
| the VAE | `sdxl_vae.safetensors` |
| other diffusion models | Chroma and Z-Image models, with their text encoders; they are not on this road, which needs SDXL for the IP-Adapter |
| what is absent | no IP-Adapter, no CLIP Vision, no InstantID, no InsightFace, no ControlNet, and no IP-Adapter or face-identity custom node |

The installation has already drawn at 1024×1024 with the Talmendo checkpoint and the SDXL VAE. The older
`~/comfy` checkout is a different, smaller installation: it carries IP-Adapter and ControlNet custom nodes
but no weights and does not serve; the running installation is `~/ComfyUI`.

The card's state moves, and the design takes that as normal. At one read on 2026-11-08 ComfyUI was active
and held 7,162 MiB, with 4,417 MiB free; at another read the unit was stopped, because the master stops it
when another occupant takes the card. The older documented state, with the translation engine holding the
card, is only one of the states a draw can meet. A draw waits for the card; it never stops another service
and never asks the owner to stop one.

### The roads

- **Road A, IP-Adapter Plus, is the default for a character, a style, an object or a place.** It reads the
  reference pictures as image prompts through a CLIP Vision encoder and steers the SDXL checkpoint towards
  their appearance. It is the right road for a cartoon character: IP-Adapter FaceID and InstantID are face
  identity models, and a character such as 치이카와 has no human face for them to find.
- **Road B, IP-Adapter FaceID or InstantID, is only for a real adult whose consent record stands.** It is a
  likeness tool; it is never used on a subject without that record, and never as a fallback when Road A's
  references are rejected.
- **Road C, plain generation, is the prompt alone.** It is used when the request asks for no reference, when
  the record is description-only, or when every candidate was rejected and the request allows a fallback.

The Lab-arena engine on n3 to n6 also has an image route the maker documents: `POST /v1/images/generations`,
Qwen-Image-2.1-Turbo through stable-diffusion.cpp, taking a prompt, a size, a count, a seed and a step count.
It is off on every node at this read (`image_generation: false`), and it takes no reference picture, so it
cannot carry the injection this design is about. It is a plain-only alternative for Road C if the owner turns
it on; this design keeps Road C on ComfyUI so one service and one workflow make every draw.

### Settings

Road A, on an SDXL checkpoint:

| Setting | Character or object | Style |
|---|---|---|
| checkpoint | `TalmendoXLBeta_v1.1.safetensors` | the same, or the checkpoint the style's own record names |
| IP-Adapter | `ip-adapter-plus_sdxl_vit-h.safetensors` | the same |
| CLIP Vision | `CLIP-ViT-H-14-laion2B-s32B-b79K.safetensors` | the same |
| weight | 0.65–0.80 | 0.45–0.60 |
| weight type | `linear` or `ease in` | `style transfer` |
| start / end | 0.0 / 0.80 | 0.0 / 0.70 |
| combine | `average` over 2–4 references | `average` over 1–3 |
| embeds scaling | `V only`; `K+V` where identity is weak | `V only` |
| steps | 28 | 24 |
| CFG | 6.5 | 6.0 |
| sampler / scheduler | `dpmpp_2m` / `karras` | the same |
| size | 1024×1024, or 832×1216 for a portrait | the requested size |
| batch | 1; a second take is a second draw | the same |
| seed | the request's, else random; recorded | the same |

Road B, only with consent:

| Setting | Value |
|---|---|
| checkpoint | as Road A |
| IP-Adapter | the maker's SDXL FaceID adapter, or InstantID's `ip-adapter.bin` |
| control | InstantID's ControlNet, weight 0.5–0.7 |
| identity weight | 0.8–1.0 |
| adapter weight | 0.6–0.8, start 0.0, end 1.0 |
| face analysis | InsightFace `antelopev2`; one clear face image |
| steps / CFG | 30 / 5.0–6.0 |
| size | 1024×1024 |

The workflow is a ComfyUI graph of `CheckpointLoaderSimple`, `CLIPVisionLoader`, `IPAdapterModelLoader`,
`ImageBatch` over the accepted references, `IPAdapterAdvanced`, two `CLIPTextEncode` nodes, `EmptyLatentImage`,
`KSampler`, `VAEDecode` and `SaveImage`. Road B swaps the adapter nodes for `InstantIDModelLoader`,
`InstantIDFaceAnalysis` and `ApplyInstantID`. The exact node versions are pinned with the custom nodes; a
workflow's JSON digest is recorded with the draw.

The prompt is built from the record, not from the research prose: the subject's description in at most
1,200 characters, the requested scene, the style, and a short quality suffix. The negative prompt names the
things the record says must not appear and the usual defects. The subject's name is included, but the
description carries the drawing; a model that does not know the name still gets an accurate prompt.

### What the ComfyUI installation must gain

**Phase 1, Road A:**

1. The custom node `ComfyUI_IPAdapter_plus`, pinned to a commit, with its requirements installed in the
   installation's own virtual environment.
2. `models/clip_vision/CLIP-ViT-H-14-laion2B-s32B-b79K.safetensors` (about 2.5 GiB).
3. `models/ipadapter/ip-adapter-plus_sdxl_vit-h.safetensors` (about 0.8 GiB).
4. A model manifest, tracked with the installation, that records each file's name, size, SHA-256 and the
   licence the model card states. A health check answers which of them are present; a draw that needs a
   missing file fails with `model-missing` and names it.
5. No InsightFace and no ONNX runtime: Road A does not use them.

**Phase 2, Road B:**

1. The custom node `ComfyUI_InstantID`, `insightface` and `onnxruntime-gpu` in the same environment.
2. `models/instantid/ip-adapter.bin`, InstantID's ControlNet, and the `antelopev2` InsightFace models.
3. The same manifest and health check. The InsightFace models carry a non-commercial research licence; the
   phase 2 deployment records it and the owner accepts it before the road is enabled. Road A and the
   character path do not need any of this.

The installation's card declaration, unit and launcher stand today in the card master's own directory and
are not tracked by a repository. They move, unchanged in behaviour, into the new bmo-paint repository's
`comfy/` folder, so the installation, the model manifest and the drawing service are one versioned thing.
The declaration keeps `service = "comfyui"`, 11,000 MiB, tier 2 and `yield_by = "process"`; the master's
placement of `comfyui` on n2 stays.

### The render

1. bmo-paint asks the arbiter's master for one card of `comfyui`, with `workKey` `paint:draw-<id>` and a
   `ref` naming the draw. It holds the demand for the whole render and gives it back after the output is
   read. A stopped ComfyUI is brought up by the master; a card taken by a higher-priority occupant is waited
   out, with no clock.
2. When the card is granted, bmo-paint checks `GET /system_stats` on the granted node through the jake hub's
   control API. It uploads the accepted references to `~/ComfyUI/input/` with the hub's `upload` route.
3. It writes the workflow JSON with the settings above and posts it to ComfyUI's `/prompt` through the hub's
   `exec` route. It keeps the returned prompt id.
4. It reads `/history/<promptId>` until the entry stands, downloads the output through the hub and computes
   its SHA-256 and dimensions. The temp input files are removed.
5. It gives the demand back. A failure at any step is recorded with the step's name; the draw does not
   silently fall back to a different node or a different checkpoint.

## Choosing a road, and what is refused

A draw request carries `subject` (optional), `references` (`auto`, `required` or `off`), `adult` (default
false), `consentId` (only for a real person), and the output options. `auto` uses a reference set when one
stands and falls back to plain generation otherwise; `required` fails when no accepted reference stands;
`off` draws plain and never looks a subject up.

| The record | `auto` | `required` | `off` |
|---|---|---|---|
| `complete` | Road A or B with the set | the same | Road C |
| `partial` | Road A with the accepted references, and the report says the set is thin | fails `no-licensed-reference` | Road C |
| `description-only` | Road C, marked an interpretation | fails `no-licensed-reference` | Road C |
| `missing` | research, then the row above | research, then the row above | Road C |
| `ambiguous` | asks the owner; no draw yet | asks the owner; no draw yet | Road C |
| `refused` | fails with the refusal | fails with the refusal | fails with the refusal |

A reference that was rejected is never injected, however few remain. Where every candidate was rejected
and the request allowed a fallback, the draw is plain and the report names each rejection; where the request
required references, it fails. The pipeline never quietly swaps a subject for a lookalike.

The limit on adult content is a gate, not a style choice:

- **Adults only.** `adult` false is the default. `adult` true is refused when the subject is, or appears to
  be, a minor: a real person under 18, a fictional character whose canonical appearance is childlike, or an
  image the vision check reads as `child` or `teen`. A childlike character is refused for adult content even
  though the character is fictional.
- **No real person's likeness without consent.** A real person's subject needs an artifact `record` with
  labels `paint.consent` `person`, `paint.person-key` matching the subject, `paint.scope` naming
  likeness generation, and `paint.expires-at` in the future, filed on the owner's word. The draw names it
  as `consentId`. Without it the draw fails `likeness-consent-missing`; Road B is not offered, and Road C
  is not used to approximate the person from a description.
- **The output is checked too.** A picture that reads as a minor in adult content, or as a real person's
  likeness without consent, is not shown; the draw fails `output-rejected`.

Refusal codes: `subject-refused-minor`, `likeness-consent-missing`, `no-licensed-reference`,
`references-rejected`, `output-rejected`, `subject-ambiguous`, `model-missing`. A refusal names what would
unblock it: a consent record, a licensed picture, or a different subject.

## The draw record, and what the owner sees

A draw is a member of bmo-paint with this shape:

| Field | Holds |
|---|---|
| `id`, `state` | its id; `queued`, `looking-up`, `researching`, `fetching`, `checking`, `ready`, `waiting-for-card`, `rendering`, `done`, `failed` or `stopped` |
| `prompt`, `subject` | the request, and the subject key, name and kind, or null |
| `record` | the subject record's `state`, artifact id and link, or null |
| `references` | `requested`, `found`, `accepted`, `rejected` (each digest, address and reason), and the set's id, count, licences and checked-at |
| `road` | `plain`, `ip-adapter-plus`, `ip-adapter-faceid` or `instantid` |
| `injection` | the adapter files, checkpoint, weight, start, end, combine, steps, CFG, sampler, seed and size |
| `card` | the node, the service `comfyui`, the demand id, and what it waited on |
| `comfy` | the prompt id and the workflow's JSON digest |
| `output` | path, SHA-256, bytes, media type, width and height |
| `licences` | the distinct licence lines the result rests on |
| `error`, `labels`, `createdAt`, `endedAt` | why it failed or was stopped, the caller's map, and the times |

The owner is shown, in the room: the picture, and one short Korean caption naming the subject, whether
references were used, how many, their licences, and a link to the subject record. For example:

- "치이카와로 그렸어. 레퍼런스 3장을 썼고 라이선스는 CC BY 4.0이야. [레코드](…)"
- "레퍼런스를 찾지 못해서 설명만으로 그린 해석이야. [레코드](…)"
- "찾은 레퍼런스가 검증에서 다 떨어져서 설명만으로 그렸어. 이유는 레코드에 있어."
- "성인 콘텐츠로는 그릴 수 없어. 이 대상은 아이로 보여."
- "실제 인물의 얼굴은 동의 기록이 있어야 그릴 수 있어. 지금은 없어서 그리지 않았어."

The rejected references, the exact settings and the sources are in the record behind the link, not in the
room. The owner can open the subject record, replace a reference with his own picture, or ask for a refresh.
A console screen for the records is later work; the room message and the archive link carry the first
version.

## The pipeline end to end

1. A drawing request reaches bmo-paint with a prompt and, where the chat agent read one, a subject.
2. bmo-paint looks the subject up in the brain, then the archive; a fresh complete record goes to step 6.
3. A missing or stale record becomes a `references` research run; its dossier is filed in the archive.
4. bmo-paint reads the candidate list, fetches each picture through bmo-explorer, and deduplicates.
5. bmo-eye checks the candidates in one look; the accepted bytes are kept, the rejected are dropped with
   their reasons.
6. bmo-paint writes the artifact record, the brain memory and its own manifest, and marks the record.
7. It asks the arbiter's master for a card of `comfyui`, waits for the card, and uploads the references.
8. It posts the workflow, reads the output, downloads it, and has bmo-eye check the output.
9. It records the draw, gives the card back, and serves the result at a signed address.
10. The chat agent sends the picture and the caption to the room; the link opens the record.

## What must be built, repository by repository

| Repository | What is built |
|---|---|
| **bmo-paint (new)** | the drawing service: its API, store, CLI `bmo paint`, tool `paint/draw`, reference lookup, research call, fetch, eye check, record writes, card demand, ComfyUI bench, result serving, and the `comfy/` folder that takes the ComfyUI declaration, unit, launcher and model manifest under version control. This is the bulk of the work. |
| **bmo-research** | the `references` run key: its prompt, its candidate schema, its licence-aware sources, and the dossier it files in artifact. Without it the pipeline can still use an owner-supplied picture, but cannot research a missing subject. |
| **bmo-explorer** | a structured image search over the engines it already asks (Openverse, Wikimedia Commons, Unsplash), answering direct address, page address, creator, publisher, dimensions, media type and licence as fields, not as text in a snippet. `fetch_raster` already gives the bytes; the licence fields are the gap. |
| **bmo-agency** | the chat agent's manual and tool listing: a drawing request goes to `paint/draw`; the subject is read from the request, not guessed; the consent question and the adult gate are asked before the draw; the result is sent with its caption. No service code. |
| **bmo-arbiter** | no code. The `comfyui` placement on n2 stays; the declaration moves into bmo-paint's `comfy/` and the master is pointed at it. bmo-paint asks for a card of `comfyui`; no new service declaration is needed. |
| **bmo-eye** | no code. The design uses `POST /uploads` and `POST /looks` in `see` with `format` `json`; the check's question and acceptance rules belong to bmo-paint. |
| **bmo-brain** | no code. The subject, memory, version and label routes above all stand. |
| **bmo-artifact** | no code for the first version. The `record` kind, labels, attachments, cover and shares are enough. Exact reference bytes stay in bmo-paint; a preserved-original attachment is a later change if the owner wants the archive to be their only keeper. |
| **bmo-node**, **bmo-receiver**, **bmo-testbed** | no service code. The node service's target route and the hub's `exec`, `upload` and `download` are the road to n2; the receiver's room picture send is the road to the room; the new repository brings its own `testbed.toml`. |
| **bmo-web** | later: a console screen for the subject records, the rejected references and the consent records. The first version uses the archive and the room. |

## What the first build must measure

No figure in this document is a promise. The first build measures, and each figure carries the node, the
engine version, the model file and the commit:

1. The peak card memory and the wall time of Road A at 1024×1024 on n2, at ComfyUI 0.44.0, the pinned
   IP-Adapter node, the chosen checkpoint and the draw's commit.
2. The same for Road B, with the InsightFace and ControlNet files named, if phase 2 is opened.
3. How often the card master stops ComfyUI for another occupant, and how long a draw waits, over a week of
   real requests.
4. The eye check's time for one to eight candidate pictures, on the Lab-arena node that answers.
5. How often the vision check accepts a candidate the research run offered, and how often it rejects one
   that looks right, so the acceptance threshold is a measured choice rather than a guess.
6. The research run's time to a filed dossier, by key and by subject kind.
