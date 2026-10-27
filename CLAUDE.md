# Working in bmo-eye

This repository holds the vision unit: its server, its CLI noun `eye` with the sub-verbs `watch`, `see` and
`read`, the tool the tool host places for the agents, the n2 model installs and the documents that state how
the three modes and their backend chains are run.

Archive documents and reports for the owner are written in Korean; the documents under `docs/` stay in English.

## Rules

- The document comes before the code: `docs/` is canonical, and a change that would make a document wrong
  repairs the document first. No change history, no code quotations, no file:line coordinates in prose.
- The server is written in Rust. Model runtimes on n2 (llama.cpp, a model's own runtime) are installed and
  driven, not forked.
- n2 is reached through the jake hub's control API (`POST http://127.0.0.1:8791/exec?agent=n2`, bearer token
  in `~/.local/share/jake/hub.env`, SSE output); there is no direct ssh. The node account is `labuser`.
- The card on n2 is held through bmo-arbiter (tier 1 occupant `eye`); other services are never stopped by hand.
- Every measurement carries the node, the engine version, the model file and the commit it was taken at.
