#!/usr/bin/env python3
"""Read a video's speech with faster-whisper on the CPU; write the transcript with timestamps."""
import json, sys, time
from faster_whisper import WhisperModel

audio, out = sys.argv[1], sys.argv[2]
lang = sys.argv[3] if len(sys.argv) > 3 else "ko"
t0 = time.time()
model = WhisperModel("large-v3", device="cpu", compute_type="int8", cpu_threads=8)
load_s = time.time() - t0
t1 = time.time()
segments, info = model.transcribe(audio, language=lang, beam_size=5, vad_filter=True)
rows = [{"start": round(s.start, 2), "end": round(s.end, 2), "text": s.text.strip()} for s in segments]
took = time.time() - t1
json.dump({"audio": audio, "model": "faster-whisper large-v3 int8 cpu", "language": info.language,
           "language_probability": round(info.language_probability, 3), "load_s": round(load_s, 1),
           "transcribe_s": round(took, 1), "segments": rows,
           "text": " ".join(r["text"] for r in rows)}, open(out, "w"), ensure_ascii=False, indent=1)
print(f"{out}: {len(rows)} segments in {took:.1f}s, language {info.language}")
