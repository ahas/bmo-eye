#!/usr/bin/env python3
"""One measurement run for bmo-eye: start llama-server on a model, ask it samples, record the fit.

Runs inside a card turn (bmo-card/turn.py --service eye), so the card is eye's while it runs.
Every number it writes is tagged with the node, the engine build, the model file and the time.
"""
from __future__ import annotations
import argparse, base64, json, os, re, signal, socket, subprocess, sys, threading, time, urllib.request

BIN = os.path.expanduser("~/eye/llama.cpp/build/bin")
OUT = os.path.expanduser("~/eye/results")


def free_port() -> int:
    s = socket.socket(); s.bind(("127.0.0.1", 0)); p = s.getsockname()[1]; s.close(); return p


def gpu_used() -> int:
    out = subprocess.run(["nvidia-smi", "--query-gpu=memory.used", "--format=csv,noheader,nounits"],
                         capture_output=True, text=True).stdout.strip().splitlines()[0]
    return int(out)


def rss_mib(pid: int) -> int:
    try:
        for line in open(f"/proc/{pid}/status"):
            if line.startswith("VmRSS:"):
                return int(line.split()[1]) // 1024
    except FileNotFoundError:
        pass
    return 0


def wait_ready(port: int, proc: subprocess.Popen, log: str, limit_s: float) -> float:
    t0 = time.time()
    while time.time() - t0 < limit_s:
        if proc.poll() is not None:
            raise SystemExit(f"server exited early:\n{open(log).read()[-3000:]}")
        try:
            with urllib.request.urlopen(f"http://127.0.0.1:{port}/health", timeout=1) as r:
                if json.load(r).get("status") == "ok":
                    return time.time() - t0
        except Exception:
            pass
        time.sleep(0.3)
    raise SystemExit("server did not become ready")


def ask(port: int, model: str, prompt: str, images: list[str], max_tokens: int) -> dict:
    content = [{"type": "text", "text": prompt}]
    for path in images:
        data = base64.b64encode(open(path, "rb").read()).decode()
        ext = "png" if path.lower().endswith(".png") else "jpeg"
        content.append({"type": "image_url", "image_url": {"url": f"data:image/{ext};base64,{data}"}})
    body = json.dumps({"model": model, "messages": [{"role": "user", "content": content}],
                       "max_tokens": max_tokens, "temperature": 0, "stream": False,
                       "chat_template_kwargs": {"enable_thinking": False},
                       "timings_per_token": True}).encode()
    req = urllib.request.Request(f"http://127.0.0.1:{port}/v1/chat/completions", data=body,
                                 headers={"content-type": "application/json"})
    t0 = time.time(); timings = {}; text = ""
    with urllib.request.urlopen(req, timeout=7200) as r:
        ev = json.load(r)
    timings = ev.get("timings") or {}
    msg = (ev.get("choices") or [{}])[0].get("message", {}) or {}
    text = msg.get("content") or ""
    if not text and msg.get("reasoning_content"):
        text = "[reasoning] " + msg["reasoning_content"]
    total = time.time() - t0
    return {"answer": text, "wall_s": round(total, 2),
            "firstToken_s": round((timings.get("prompt_ms") or 0) / 1000, 3) or None,
            "timings": timings}


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--name", required=True)
    ap.add_argument("--model", required=True)
    ap.add_argument("--mmproj")
    ap.add_argument("--ctx", type=int, default=4096)
    ap.add_argument("--ngl", type=int, default=-1)
    ap.add_argument("--extra", default="")
    ap.add_argument("--samples", required=True, help="JSON list of {id, question, images, answer}")
    ap.add_argument("--max-tokens", type=int, default=512)
    args = ap.parse_args()

    os.makedirs(OUT, exist_ok=True)
    port = free_port()
    log = os.path.join(OUT, f"{args.name}.server.log")
    cmd = ["stdbuf", "-oL", "-eL", f"{BIN}/llama-server", "-m", args.model, "--host", "127.0.0.1", "--port", str(port),
           "-c", str(args.ctx), "--no-webui", "--jinja", "--metrics", "--fit", "on",
           "--fit-target", "512", "--log-verbosity", "1"]
    if args.ngl >= 0:
        cmd += ["-ngl", str(args.ngl)]
    if args.mmproj:
        cmd += ["--mmproj", args.mmproj]
    cmd += args.extra.split()
    # The master stops the occupant that held the card only on its next read, so the turn can arrive
    # while another process still fills the card. Wait for it to be empty before loading anything.
    t_wait = time.time()
    while time.time() - t_wait < 180 and gpu_used() > 2000:
        time.sleep(2)
    t_start = time.time()
    gpu_before = gpu_used()
    slog = open(log, "wb")
    proc = subprocess.Popen(cmd, stdout=slog, stderr=slog)
    try:
        load_s = wait_ready(port, proc, log, 1800)
        gpu_after = gpu_used()
        time.sleep(1.5)
        gpu_settled = gpu_used()
        rss = rss_mib(proc.pid)
        server_log = open(log, errors="replace").read()
        offload = re.findall(r"(load_tensors: offloaded \d+/\d+ layers to GPU|offloading \d+ repeating layers)", server_log)
        bufs = re.findall(r"(CUDA0 model buffer size = [\d.]+ MiB|CPU model buffer size = [\d.]+ MiB|CUDA0 buffer size = [\d.]+ MiB|CPU buffer size = [\d.]+ MiB)", server_log)
        samples = json.load(open(args.samples))
        results = []
        for s in samples:
            t = time.time()
            out = ask(port, args.model, s["question"], s.get("images", []), args.max_tokens)
            tm = out.get("timings") or {}
            results.append(dict(id=s["id"], question=s["question"], reference=s.get("answer"),
                                answer=out["answer"], wall_s=out["wall_s"],
                                first_token_s=out["firstToken_s"],
                                prompt_tokens=tm.get("prompt_n"), prompt_ms=tm.get("prompt_ms"),
                                predicted_tokens=tm.get("predicted_n"), predicted_ms=tm.get("predicted_ms"),
                                predicted_per_s=tm.get("predicted_per_second"),
                                prompt_per_s=tm.get("prompt_per_second")))
            results[-1]["gpu_mib"] = gpu_used(); results[-1]["rss_mib"] = rss_mib(proc.pid)
            print(f"{s['id']}: {out['wall_s']}s, {tm.get('predicted_per_second')} tok/s, gpu {results[-1]['gpu_mib']}", flush=True)
        record = dict(run=args.name, node=os.uname().nodename, at=time.strftime("%Y-%m-%dT%H:%M:%S%z"),
                      model=args.model, mmproj=args.mmproj, engine="llama.cpp",
                      engine_version=[l for l in (subprocess.run([f"{BIN}/llama-cli", "--version"], capture_output=True, text=True).stdout + subprocess.run([f"{BIN}/llama-cli", "--version"], capture_output=True, text=True).stderr).splitlines() if l.strip()][:3],
                      engine_commit=open(os.path.expanduser("~/eye/logs/llama-commit.txt")).read().strip(),
                      command=" ".join(cmd), ctx=args.ctx, ngl=args.ngl, extra=args.extra,
                      gpu_before_mib=gpu_before, gpu_loaded_mib=gpu_settled, gpu_delta_mib=gpu_settled - gpu_before,
                      rss_mib=rss, load_s=round(load_s, 2), offload=offload, buffers=bufs,
                      samples=results)
        path = os.path.join(OUT, f"{args.name}.json")
        json.dump(record, open(path, "w"), ensure_ascii=False, indent=1)
        print("wrote", path)
    finally:
        proc.send_signal(signal.SIGTERM)
        try:
            proc.wait(timeout=30)
        except subprocess.TimeoutExpired:
            proc.kill()
        time.sleep(2)
        print("gpU after stop:", gpu_used())


if __name__ == "__main__":
    main()
