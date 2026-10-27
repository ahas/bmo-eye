#!/usr/bin/env python3
"""Read document images with DeepSeek-OCR-2 on the card; write one JSON of what each page answered."""
import json, os, sys, time
os.environ.setdefault("CUDA_VISIBLE_DEVICES", "0")
import torch
from transformers import AutoModel, AutoTokenizer

base = os.path.expanduser("~/eye/models/deepseek-ocr-2")
samples = json.load(open(sys.argv[1]))
out = sys.argv[2]
attn = sys.argv[3] if len(sys.argv) > 3 else "sdpa"

# The master stops the occupant that held the card only on its next read, so wait for the card to be empty.
import time as _t
_t0 = _t.time()
while _t.time() - _t0 < 180:
    used = int(os.popen("nvidia-smi --query-gpu=memory.used --format=csv,noheader,nounits").read().split()[0])
    if used < 2000:
        break
    _t.sleep(2)
t0 = time.time()
tok = AutoTokenizer.from_pretrained(base, trust_remote_code=True)
# Loaded in bfloat16 from the start: the default fp32 load is 13.5 GB and does not fit the 12 GB card.
model = AutoModel.from_pretrained(base, _attn_implementation=attn, trust_remote_code=True,
                                  use_safetensors=True, torch_dtype=torch.bfloat16)
model = model.eval().cuda()
print("loaded, vram MiB:", int(os.popen("nvidia-smi --query-gpu=memory.used --format=csv,noheader,nounits").read().split()[0]), flush=True)
load_s = time.time() - t0
vram = int(os.popen("nvidia-smi --query-gpu=memory.used --format=csv,noheader,nounits").read().split()[0])
rows = []
for s in samples:
    t = time.time()
    try:
        page_dir = f"/tmp/ocr-out/{s['id']}"
        model.infer(tok, prompt="<image>\n<|grounding|>Convert the document to markdown. ",
                    image_file=s["images"][0], output_path=page_dir, base_size=1024,
                    image_size=768, crop_mode=True, save_results=True)
        text = open(f"{page_dir}/result.mmd", encoding="utf-8").read()
        err = None
    except Exception as e:
        text, err = "", f"{type(e).__name__}: {e}"
    import re as _re
    clean = _re.sub(r"<\|[^|]*\|>", "", text).strip()
    rows.append(dict(id=s["id"], question=s["question"], reference=s.get("answer"), answer=clean, raw=text,
                     wall_s=round(time.time() - t, 2), error=err))
    print(f"{s['id']}: {rows[-1]['wall_s']}s {'ERR ' + err if err else ''}", flush=True)
json.dump(dict(run="read-deepseek-ocr-2", node=os.uname().nodename, at=time.strftime("%Y-%m-%dT%H:%M:%S%z"),
               model=base, attn=attn, torch=torch.__version__, cuda=torch.version.cuda,
               gpu="RTX 4070 12282MiB", load_s=round(load_s, 1), gpu_loaded_mib=vram,
               prompt="<image>\\n<|grounding|>Convert the document to markdown. ",
               base_size=1024, image_size=768, crop_mode=True, samples=rows),
          open(out, "w"), ensure_ascii=False, indent=1)
print("wrote", out)
