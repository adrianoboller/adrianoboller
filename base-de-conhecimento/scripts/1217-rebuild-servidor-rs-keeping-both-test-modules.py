# Rebuild servidor.rs keeping both test modules
# 29/08 22:15

import io
p="phxsql/crates/phxsql-server/src/servidor.rs"
s=io.open(p,encoding="utf-8").read().split("\n")
ours=io.open("/tmp/ours.rs",encoding="utf-8").read().split("\n")
theirs=io.open("/tmp/theirs.rs",encoding="utf-8").read().split("\n")
# tudo antes do primeiro marcador ja tem as mudancas das duas frentes
i=next(n for n,l in enumerate(s) if l.startswith("<<<<<<< HEAD"))
cabeca=s[:i]
modA=ours[17110:]     # #[cfg(test)] mod testes_restaurar_backup ate o fim
modB=theirs[16229:]   # #[cfg(test)] mod testes_janela_e_cadeia ate o fim
assert modA[0].startswith("#[cfg(test)]") and "testes_restaurar_backup" in modA[1], modA[:2]
assert modB[0].startswith("#[cfg(test)]") and "testes_janela_e_cadeia" in modB[1], modB[:2]
novo = cabeca + modA + [""] + modB
txt="\n".join(novo)
assert "<<<<<<<" not in txt and ">>>>>>>" not in txt
io.open(p,"w",encoding="utf-8").write(txt)
print("servidor.rs: os dois modulos de teste preservados,", len(novo), "linhas")
