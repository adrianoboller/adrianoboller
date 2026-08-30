# Resolve the dossiê conflicts
# 30/08 03:13

import io
M=">>>>>>> worktree-agent-ad8980709910012d6"
for p,modo in [("phxsql/CHANGELOG.md","aditivo"),("phxsql/docs/PENDENCIAS.md","numerar")]:
    s=io.open(p,encoding="utf-8").read()
    n=0
    while "<<<<<<< HEAD" in s:
        a=s.index("<<<<<<< HEAD"); b=s.index("\n=======\n",a); c=s.index(M,b); cfim=s.index("\n",c)+1
        meu=s[a+len("<<<<<<< HEAD\n"):b+1]; deles=s[b+len("\n=======\n"):c]
        if modo=="aditivo":
            s=s[:a]+meu+deles+s[cfim:]
        else:
            # o pedido deles ganha numero novo depois do ultimo do HEAD
            linhas=[l for l in deles.split("\n") if l.startswith("| ☑️ |") or l.startswith("| ◐ |")]
            marca="<!-- pedidos:contagem:inicio -->"
            if linhas and marca in meu:
                import re
                usados={int(m.group(1)) for m in re.finditer(r"^\| [☑◐] .* \| (\d+) \|", s, re.M)}
                prox=max(usados)+1 if usados else 1
                novas=[]
                for l in linhas:
                    l2=re.sub(r"^(\| [☑◐]\S* \| )\d+( \|)", lambda m: f"{m.group(1)}{prox}{m.group(2)}", l, count=1)
                    novas.append(l2); prox+=1
                meu=meu.replace(marca, "\n".join(novas)+"\n\n"+marca, 1)
            s=s[:a]+meu+s[cfim:]
        n+=1
    io.open(p,"w",encoding="utf-8").write(s)
    print(p, n, "bloco(s)")
