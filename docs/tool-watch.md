# The `eye/watch` entry

Look at a video and answer a question about it, with what is said in it: a file on this machine, an https
address, or a YouTube link.

```
bmo use eye/watch --arg path=https://www.youtube.com/watch?v=… --arg question="무엇을 보도하는가?"
```

- **Frames and speech are read apart.** A dozen frames go to a Lab-arena node as pictures; the audio is read
  by faster-whisper on n2's CPU and put into the prompt with the frames.
- **The frame budget is the node's reader:** a picture is about 300 tokens and seconds of CPU, so
  `maxFrames` (12 by default) decides what a minute of video is answered from. `fps` is the ceiling, not a
  promise: frames are taken evenly across the span when the count would take more.
- **A minute of video takes about a minute,** most of it the speech model and the frames' reading.
