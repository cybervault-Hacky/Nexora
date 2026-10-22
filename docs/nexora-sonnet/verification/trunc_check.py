import json, os, signal, subprocess, sys, time, urllib.request
KEY = open("/home/user/.nexora-verify/runtime-session.key").read().strip()
body = json.dumps({"model":"local","stream":True,"messages":[{"role":"user","content":"Write a long story."}],"max_tokens":4000,"temperature":0.2}).encode()
req = urllib.request.Request("http://127.0.0.1:18081/v1/chat/completions", data=body, headers={"Authorization": f"Bearer {KEY}","Content-Type":"application/json"}, method="POST")
resp = urllib.request.urlopen(req, timeout=120)
got, finish, done = 0, None, False
pid = int(subprocess.check_output(["pgrep","-f","llama-server .*--port 18081"]).split()[0])
for raw in resp:
    line = raw.decode().strip()
    if line.startswith("data:"):
        p=line[5:].strip()
        if p=="[DONE]": done=True
        else:
            try:
                c=json.loads(p)
                for ch in c.get("choices",[]):
                    if ch.get("delta",{}).get("content"): got+=1
                    if ch.get("finish_reason"): finish=ch["finish_reason"]
            except Exception: pass
    if got == 5:
        os.kill(pid, signal.SIGKILL)  # engine dies mid-reply
        print("engine killed after", got, "deltas")
print("stream ended: deltas=%d finish_reason=%r [DONE]=%s" % (got, finish, done))
print("client.rs stream_completed would be:", done or bool(finish))
