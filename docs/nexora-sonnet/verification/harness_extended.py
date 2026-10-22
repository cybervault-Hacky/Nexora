#!/usr/bin/env python3
"""Wider standalone harness: real source for inference::{error,registry,settings,loading,budget}
and usage::{mod,store}. serde derives are stripped; serde_json-dependent tests are dropped.
Compiled with rustc --test. NOT cargo test; commands.rs/client.rs/process.rs are not included."""
import re, sys, pathlib, subprocess, shutil
SRC = pathlib.Path("/home/user/Nexora/apps/desktop/src-tauri/src")
OUT = pathlib.Path("/tmp/harness/out2"); shutil.rmtree(OUT, ignore_errors=True)
(OUT/"inference").mkdir(parents=True); (OUT/"usage").mkdir(parents=True)
def strip_serde(text):
    def fix(m):
        kept=[x.strip() for x in m.group(1).split(",") if x.strip() not in ("Serialize","serde::Serialize","Deserialize")]
        return "#[derive("+", ".join(kept)+")]" if kept else ""
    text=re.sub(r"#\[derive\(([^)]*)\)\]", fix, text)
    text=re.sub(r"^\s*#\[serde\(.*?\)\]\s*\n","",text,flags=re.M)
    return text.replace("use serde::Serialize;\n","").replace("use serde::ser::Serializer;\n","")
def drop_json_tests(text):
    lines=text.split("\n"); res=[]; j=0
    while j<len(lines):
        if lines[j].strip()=="#[test]" and j+1<len(lines) and lines[j+1].lstrip().startswith("fn "):
            k=j+1; depth=0; started=False
            while k<len(lines):
                depth+=lines[k].count("{")-lines[k].count("}")
                if "{" in lines[k]: started=True
                if started and depth==0: break
                k+=1
            if "serde_json" not in "\n".join(lines[j:k+1]): res.extend(lines[j:k+1])
            j=k+1; continue
        res.append(lines[j]); j+=1
    return "\n".join(res)
def load(rel, drop=True):
    t=strip_serde((SRC/rel).read_text())
    return drop_json_tests(t) if drop else t
for f in ["error.rs","registry.rs","loading.rs","budget.rs"]:
    t=load("inference/"+f)
    if f=="error.rs": t=re.sub(r"impl From<(reqwest|serde_json)::Error> for InferenceError \{.*?\n\}\n","",t,flags=re.S)
    (OUT/"inference"/f).write_text(t)
(OUT/"inference"/"settings.rs").write_text("pub const DEFAULT_CONTEXT_SIZE: u32 = 32_768;\npub const MIN_CONTEXT_SIZE: u32 = 2_048;\npub const MAX_CONTEXT_SIZE: u32 = 131_072;\n")
(OUT/"inference"/"mod.rs").write_text("pub mod error;\npub mod registry;\npub mod settings;\npub mod loading;\npub mod budget;\n")
for f in ["mod.rs","store.rs"]:
    pass
for f in ["mod.rs","store.rs"]:
    t=load("usage/"+f)
    t=re.sub(r"^pub mod commands;\n","",t,flags=re.M)
    (OUT/"usage"/f).write_text(t)
(OUT/"serde_stub.rs").write_text("pub trait Serialize {}\n")
(OUT/"main.rs").write_text("pub mod inference;\npub mod usage;\nfn main() {}\n")
r1=subprocess.run(["rustc","--crate-type","rlib","--crate-name","serde","--edition","2021","-o",str(OUT/"libserde.rlib"),str(OUT/"serde_stub.rs")],capture_output=True,text=True)
if r1.returncode: print(r1.stderr); sys.exit(1)
r=subprocess.run(["rustc","--test","--edition","2021","-o",str(OUT/"harness"),"--extern",f"serde={OUT}/libserde.rlib",str(OUT/"main.rs"),"-A","dead_code","-A","unused"],capture_output=True,text=True)
if r.returncode: print(r.stderr[-8000:]); sys.exit(r.returncode)
run=subprocess.run([str(OUT/"harness")],capture_output=True,text=True)
lines=[l for l in run.stdout.split("\n") if l.startswith("test ") and not l.endswith("... ok")]
print("\n".join(lines)[-3000:]); print([l for l in run.stdout.split("\n") if "test result" in l]); print("HARNESS_EXIT",run.returncode)
