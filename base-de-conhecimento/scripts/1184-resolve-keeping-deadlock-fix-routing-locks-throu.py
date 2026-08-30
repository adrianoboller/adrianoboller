# Resolve keeping deadlock fix, routing locks through the single point
# 29/08 18:40

import re, pathlib
p = pathlib.Path("crates/phxsql-server/src/servidor.rs"); t = p.read_text()
ms = list(re.finditer(r"<<<<<<< [^\n]*\n(.*?)=======\n(.*?)>>>>>>> [^\n]*\n", t, re.S))
novos = {}

# 1 e 2: campos e construtor -- aditivos.
novos[0] = ms[0].group(1) + ms[0].group(2)
novos[1] = ms[1].group(1).rstrip().replace("        }))", "") + "            telemetria: Arc::new(crate::telemetria::Telemetria::default()),\n"
# 3: metodos -- aditivos (papel_atual do HEAD + travar_dados do ramo).
novos[2] = ms[2].group(1).rstrip() + "\n\n" + ms[2].group(2).rstrip() + "\n"
# 4, 5, 6: fica a logica do HEAD (gatilhos), mas a trava passa pelo ponto
# unico que a telemetria criou -- e o que instrumenta a espera.
for i in (3, 4, 5):
    novos[i] = ms[i].group(1).replace(
        "let _trava = self.dados.lock().map_err(|_| trava_envenenada())?;",
        "let _trava = self.travar_dados()?;")
# 7: fica o HEAD INTEIRO. O ramo reintroduziria a SEGUNDA tomada da mesma
# trava dentro de uma funcao que ja a segura -- o deadlock que a frente da
# configuracao acabou de consertar.
novos[6] = ms[6].group(1)

for i in sorted(novos, reverse=True):
    t = t[:ms[i].start()] + novos[i] + t[ms[i].end():]
p.write_text(t)
print("resolvidos; marcas restantes:", t.count("<<<<<<<"))
