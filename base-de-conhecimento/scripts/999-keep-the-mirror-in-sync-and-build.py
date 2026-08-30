# Keep the mirror in sync and build
# 29/08 02:06

import pathlib
p = pathlib.Path("crates/phxsql-server/src/servidor.rs")
s = p.read_text()

# ligar: o espelho sobe DENTRO da trava, para nao divergir
alvo = '''        prof.ligar(filtro, &arquivo, teto, agora)?;'''
novo = '''        prof.ligar(filtro, &arquivo, teto, agora)?;
        // Dentro da trava, e depois de `ligar` ter dado certo: um espelho que
        // sobe antes de a observacao existir faria o caminho quente pagar por
        // um profiler que nao ligou.
        self.profiler_ligado.store(true, Ordering::Relaxed);'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)

alvo = '''        prof.desligar(crate::agora_ms());'''
novo = '''        prof.desligar(crate::agora_ms());
        self.profiler_ligado.store(false, Ordering::Relaxed);'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)
p.write_text(s)
print("ok")
