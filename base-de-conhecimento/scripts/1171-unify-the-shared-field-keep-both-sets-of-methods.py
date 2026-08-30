# Unify the shared field, keep both sets of methods
# 29/08 18:08

import re, pathlib
p = pathlib.Path("crates/phxsql-server/src/servidor.rs"); t = p.read_text()
ms = list(re.finditer(r"<<<<<<< [^\n]*\n(.*?)=======\n(.*?)>>>>>>> [^\n]*\n", t, re.S))

# 1) Campos: fica o HEAD inteiro (ja tem somente_leitura_vivo) e entram apenas
#    os DOIS campos novos do ramo. Um `somente_leitura_vivo` so, com os dois
#    caminhos que o escrevem.
novo1 = ms[0].group(1).rstrip() + """
    /// Os outros dois ajustes que a tela de configuracao muda A QUENTE.
    ///
    /// O `somente_leitura_vivo` acima serve aos DOIS caminhos que o mudam: a
    /// promocao de um spare e a gravacao pela tela. Sao a mesma pergunta --
    /// "este servidor aceita escrita AGORA?" -- e dois campos para ela
    /// virariam duas respostas no dia em que um caminho esquecesse o outro.
    max_linhas_vivo: AtomicU64,
    espelho_vivo: AtomicBool,
"""
# 2) Construtor: idem -- os dois campos novos, sem repetir o compartilhado.
novo2 = ms[1].group(1).rstrip() + "\n"
extra2 = """            max_linhas_vivo: AtomicU64::new(max_linhas),
            espelho_vivo: AtomicBool::new(espelho),
"""
# 3) Metodos: aditivo puro.
novo3 = ms[2].group(1).rstrip() + "\n\n" + ms[2].group(2).rstrip() + "\n"

for m, novo in ((ms[0], novo1), (ms[2], novo3)):
    t = t.replace(m.group(0), novo, 1)
# o 2 precisa colocar os campos ANTES do fecho do struct literal
t = t.replace(ms[1].group(0), novo2, 1)
t = t.replace("""            rotinas: Mutex::new(rotinas),
            ha_gatilhos,
        });""", """            rotinas: Mutex::new(rotinas),
            ha_gatilhos,
""" + extra2 + "        });", 1)
p.write_text(t)
print("resolvidos 1-3; restam:", t.count("<<<<<<<"))
