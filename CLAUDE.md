# Working in bmo-eye

This repository holds the vision unit: its server, its CLI noun `eye` with the sub-verbs `watch`, `see` and
`read`, the tool the tool host places for the agents, and the documents that state how the three modes and
their backend chains are run.

Archive documents and reports for the owner are written in Korean; the documents under `docs/` stay in English.

## Rules

- The document comes before the code: `docs/` is canonical, and a change that would make a document wrong
  repairs the document first. No change history, no code quotations, no file:line coordinates in prose.
- `docs/openapi.json` is generated from the types the routes take and answer, and the suite holds the kept
  document to them: a change to a route or a body writes it again with
  `cargo run -p eye-server --example openapi > docs/openapi.json`.
- The server is written in Rust and holds no model, no weight and no card. Pictures are read by the local
  engine nodes the settings name (`docs/engines.md`); nothing of this unit is installed as a model on a node.
- The local engine nodes are reached at the addresses the settings' `[local]` section lists, none by default,
  each with the key file the settings name for it. Their API is OpenAI's chat completions; a picture is an `image_url`
  part and thinking is always off.
- The speech node is reached through the jake hub's control API (`POST <hub>/exec?agent=<speech.agent>`, bearer token
  in `~/.local/share/jake/hub.env`, SSE output) for a video's speech; there is no direct ssh. The speech paths
  are the node account's own, under its home.
- The vision unit takes no card from bmo-arbiter and no other service is stopped by hand.
- Every measurement carries the node, the engine version, the model file and the commit it was taken at.
