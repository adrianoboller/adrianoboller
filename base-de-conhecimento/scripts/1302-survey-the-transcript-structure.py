# Survey the transcript structure
# 30/08 16:25

import json, collections
T="/root/.claude/projects/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d.jsonl"
tipos=collections.Counter(); ferramentas=collections.Counter()
usuario=0; assistente=0; erros=0
with open(T, encoding="utf-8", errors="replace") as f:
    for ln in f:
        ln=ln.strip()
        if not ln: continue
        try: d=json.loads(ln)
        except Exception: erros+=1; continue
        t=d.get("type",""); tipos[t]+=1
        m=d.get("message") or {}
        c=m.get("content")
        if t=="user" and isinstance(c,str): usuario+=1
        if t=="assistant": assistente+=1
        if isinstance(c,list):
            for b in c:
                if isinstance(b,dict) and b.get("type")=="tool_use":
                    ferramentas[b.get("name","?")]+=1
print("tipos de linha:", dict(tipos))
print("linhas ilegiveis:", erros)
print("mensagens de texto do usuario:", usuario)
print()
print("ferramentas usadas:")
for k,v in ferramentas.most_common(20): print(f"  {k:28} {v}")
