# Working in bmo-eye

This repository holds the vision unit: its server, its CLI noun `eye` with the sub-verbs `watch`, `see` and
`read`, the tool the tool host places for the agents, and the documents that state how the three modes and
their backend chains are run.

Archive documents and reports for the owner are written in Korean; the documents under `docs/` stay in English.

## Rules

- The document comes before the code: `docs/` is canonical, and a change that would make a document wrong
  repairs the document first. No change history, no code quotations, no file:line coordinates in prose.
- The server is written in Rust and holds no model, no weight and no card. Pictures are read by the Lab-arena
  engines on n3 to n6 (`docs/engines.md`); nothing of this unit is installed as a model on a node.
- The Lab-arena nodes are reached at `127.0.0.1:18101`–`18104` from main-host, with their keys under
  `~/.local/share/bmo-lab-arena/keys/`. Their API is OpenAI's chat completions; a picture is an `image_url`
  part and thinking is always off.
- n2 is reached through the jake hub's control API (`POST http://127.0.0.1:8791/exec?agent=n2`, bearer token
  in `~/.local/share/jake/hub.env`, SSE output) for a video's speech; there is no direct ssh. The node
  account is `labuser`.
- The vision unit takes no card from bmo-arbiter and no other service is stopped by hand.
- Every measurement carries the node, the engine version, the model file and the commit it was taken at.
