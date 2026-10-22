import json, time, urllib.request, sys
sys.argv=[sys.argv[0],"18080"]
exec(open("stream_check.py").read().split('print("== 1.')[0])  # reuse helpers only
# 4. disconnect after a few deltas
print("== 4. client disconnect mid-stream ==")
resp = urllib.request.urlopen(post(KEY, 4000), timeout=120)
seen = 0
for raw in resp:
    if raw.startswith(b"data:") and b"content" in raw: seen += 1
    if seen >= 5: break
t_cut = time.time(); resp.close()
print("closed after", seen, "content events")
# 5. next request must not wait for the abandoned generation (parallel=1 slot)
t0 = time.time()
r2 = urllib.request.urlopen(post(KEY, 16), timeout=120)
n = sum(1 for raw in r2 if b"finish_reason" in raw and b'"finish_reason":null' not in raw)
print(f"next request answered in {time.time()-t0:.2f}s, finish chunk seen={n>0}")
