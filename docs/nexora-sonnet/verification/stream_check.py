"""Exercises the /v1/chat/completions contract that Nexora's client.rs sends and reads.
Request body and headers mirror client.rs (model 'local', stream true, system + user turns,
max_tokens, temperature, Bearer key). Targets the SYNTHETIC test model only."""
import json, sys, time, urllib.request, urllib.error
PORT = int(sys.argv[1]) if len(sys.argv) > 1 else 18080
KEY = open("/home/user/.nexora-verify/runtime-session.key").read().strip()
URL = f"http://127.0.0.1:{PORT}/v1/chat/completions"
SYSTEM = "You are Nexora, a local coding assistant running on the user's own machine."

def body(max_tokens=64):
    return json.dumps({"model": "local", "stream": True,
        "messages": [{"role": "system", "content": SYSTEM}, {"role": "user", "content": "Say hello."}],
        "max_tokens": max_tokens, "temperature": 0.2}).encode()

def post(key, max_tokens=64):
    h = {"Content-Type": "application/json"}
    if key is not None: h["Authorization"] = f"Bearer {key}"
    return urllib.request.Request(URL, data=body(max_tokens), headers=h, method="POST")

print("== 1. request without key ==")
try:
    urllib.request.urlopen(post(None), timeout=20); print("UNEXPECTED: accepted")
except urllib.error.HTTPError as e:
    print("HTTP", e.code, e.read()[:200])

print("== 2. request with wrong key ==")
try:
    urllib.request.urlopen(post("00"*32), timeout=20); print("UNEXPECTED: accepted")
except urllib.error.HTTPError as e:
    print("HTTP", e.code)

print("== 3. correct key, stream to completion ==")
t0 = time.time()
resp = urllib.request.urlopen(post(KEY, 64), timeout=120)
print("HTTP", resp.status, resp.headers.get("Content-Type"))
lines, finish, done, deltas = [], None, False, 0
for raw in resp:
    line = raw.decode("utf-8", "replace").rstrip("\r\n")
    if not line.startswith("data:"): continue
    payload = line[5:].strip()
    if payload == "[DONE]": done = True; continue
    try: chunk = json.loads(payload)
    except Exception: continue
    for ch in chunk.get("choices", []):
        if ch.get("delta", {}).get("content"): deltas += 1
        if ch.get("finish_reason"): finish = ch["finish_reason"]
    if "usage" in chunk: lines.append(chunk["usage"])
print(f"content deltas={deltas} finish_reason={finish!r} [DONE]={done} usage={lines[-1] if lines else None} elapsed={time.time()-t0:.2f}s")
print("client.rs stream_completed would be:", done or bool(finish))
