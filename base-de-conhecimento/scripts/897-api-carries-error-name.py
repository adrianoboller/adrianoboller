# api() carries error name
# 28/08 23:54

import pathlib
p = pathlib.Path("crates/phxsql-server/ui/index.html")
s = p.read_text()

# 1) api(): o erro carrega o nome simbolico
alvo = '''  const j = await r.json();
  if (j.sessao) est.sessao = j.sessao;
  if (!j.ok) throw new Error(j.erro || "falha");
  return j.resultado;
}'''
novo = '''  const j = await r.json();
  if (j.sessao) est.sessao = j.sessao;
  if (!j.ok) {
    // O texto é para quem lê; o nome e o código são para quem programa. Sem
    // eles pendurados aqui, distinguir «conflito de escrita» de qualquer
    // outra recusa obrigaria a comparar a REDAÇÃO da mensagem — e o dia em
    // que alguém a melhorasse, a tela quebrava calada.
    const e = new Error(j.erro || "falha");
    e.nome = j.nome || "";
    e.codigo = j.codigo || 0;
    throw e;
  }
  return j.resultado;
}'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)
p.write_text(s)
print("api ok")
