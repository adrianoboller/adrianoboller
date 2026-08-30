# Renumber the crypto section references
# 29/08 21:03

import io,re,glob
# a cifra virou §11: toda referencia a §10.x que fala de CIFRA passa a §11.x
alvos = ["phxsql/crates/phxsql-store/src/reg.rs",
         "phxsql/crates/phxsql-store/src/table.rs",
         "phxsql/crates/phxsql-store/src/cofre.rs",
         "phxsql/crates/phxsql-store/examples/custo-da-cifra.rs",
         "phxsql/crates/phxsql-store/tests/cifra-modo-frogcript.rs",
         "phxsql/crates/phxsql-core/src/frogcript.rs",
         "phxsql/crates/phxsql-server/ui/index.html",
         "phxsql/docs/PENDENCIAS.md",
         "phxsql/docs/FORMATO.md"]
mudou=[]
for p in alvos:
    s=io.open(p,encoding="utf-8").read(); o=s
    s=s.replace("SEGURANCA.md` §10.4","SEGURANCA.md` §11.4").replace("SEGURANCA.md §10.4","SEGURANCA.md §11.4")
    s=s.replace("SEGURANCA.md` §10.3","SEGURANCA.md` §11.3").replace("SEGURANCA.md</code> §10.3","SEGURANCA.md</code> §11.3")
    s=s.replace("SEGURANCA.md` §10.2","SEGURANCA.md` §11.2")
    s=s.replace("SEGURANCA.md` §10.","SEGURANCA.md` §11.")
    s=s.replace("SEGURANCA.md §10.","SEGURANCA.md §11.")
    # `§10.` solto (sem numero) nos dois arquivos do store fala da cifra
    if p.endswith(("reg.rs","table.rs")):
        s=s.replace("SEGURANCA.md` §10.","SEGURANCA.md` §11.").replace("SEGURANCA.md §10.","SEGURANCA.md §11.")
    if s!=o: mudou.append(p); io.open(p,"w",encoding="utf-8").write(s)
print("\n".join(mudou))
