# The `eye/see` entry

Look at one picture and answer a question about it. The picture is read by the Qwen3.8-Flash-Next set that
bmo-lab-arena runs on n3 to n6 (`docs/modes.md`), so nothing of the vision unit itself runs a model.

Call it with a path on this machine — the path a room's message note names — and the question you want
answered. The answer comes back in the language the question was asked in, with the backend and the node
that answered it in the structured part.

```
bmo use eye/see --arg path=/home/…/image.jpg --arg question="무엇이 보이는가?"
```

- **What it is good at:** a screenshot's fields and numbers, a photo of a sign or a receipt, a chart's
  largest bar. It describes; for a page of text use `eye/read`.
- **What it costs:** a picture is about 300 tokens of the node's context and a few seconds of its CPU, and
  the answer is one line to a paragraph in about three to six seconds.
- **When no backend can answer:** the look is handed to you. The answer says so, gives your question and
  the picture's path, carries the picture itself where it is small enough to send, and ends with why every
  backend failed; look at the picture yourself and answer. Called from a shell, where nobody looks, the call
  answers an error with every backend's reason instead (`docs/api.md`, 「Handing the input to the caller」).
