# Add the action colour tokens and classes
# 28/08 22:49

import pathlib
p = pathlib.Path("crates/phxsql-server/ui/index.html")
s = p.read_text()

# 1. tokens, nos dois temas
antigo = """  --ok:#6cc98c; --aviso:#ffc43d;"""
novo = """  --ok:#6cc98c; --aviso:#ffc43d;
  /* As cores da ACAO. Convencao pedida, e ela vale mais que a paleta da marca
     aqui porque o usuario aprende a cor uma vez e ela vale em toda tela:
     verde inclui, amarelo altera, vermelho exclui de vez, rosa marca (o
     excluir que volta), azul consulta. */
  --acao-incluir:#6cc98c; --acao-alterar:#ffc43d; --acao-excluir:#ff5f5f;
  --acao-marcar:#ff8fc7;  --acao-consultar:#5fa6e8;"""
assert antigo in s
s = s.replace(antigo, novo)

antigo = """  --ok:#2f7a3e; --aviso:#8a6a1f;"""
novo = """  --ok:#2f7a3e; --aviso:#8a6a1f;
  /* No papel as cinco escurecem: verde e rosa claros dao menos de 4,5:1 sobre
     branco, que e o minimo para texto. E a mesma adaptacao que o vermelhao da
     marca ja fazia. */
  --acao-incluir:#2f7a3e; --acao-alterar:#8a6a1f; --acao-excluir:#b71414;
  --acao-marcar:#b5257f;  --acao-consultar:#1f5c93;"""
assert antigo in s
s = s.replace(antigo, novo)

# 2. as cinco classes, no mesmo estilo do `perigo` -- contorno, e nao fundo
antigo = """.botao.perigo{background:transparent;border:1px solid var(--log);color:var(--log);
              font-weight:500}
.botao.perigo:hover{background:var(--log);color:#fff;border-color:var(--log)}"""
novo = """.botao.perigo{background:transparent;border:1px solid var(--log);color:var(--log);
              font-weight:500}
.botao.perigo:hover{background:var(--log);color:#fff;border-color:var(--log)}

/* AS CORES DA ACAO -- contorno, e nao fundo cheio.
   A licao esta duas linhas acima: fundo laranja com texto escuro em cima
   ficava ilegivel. O contorno diz a cor sem apostar o contraste do texto
   nela; o preenchimento so acontece no hover, quando ha intencao. */
.botao.incluir,.botao.alterar,.botao.excluir,.botao.marcar,.botao.consultar{
  background:transparent;border:1px solid currentColor;font-weight:600;width:auto;
  padding:9px 16px;
}
.botao.incluir{color:var(--acao-incluir)}
.botao.alterar{color:var(--acao-alterar)}
.botao.excluir{color:var(--acao-excluir)}
.botao.marcar{color:var(--acao-marcar)}
.botao.consultar{color:var(--acao-consultar)}
.botao.incluir:hover{background:var(--acao-incluir);color:var(--fundo)}
.botao.alterar:hover{background:var(--acao-alterar);color:var(--fundo)}
.botao.excluir:hover{background:var(--acao-excluir);color:#fff}
.botao.marcar:hover{background:var(--acao-marcar);color:var(--fundo)}
.botao.consultar:hover{background:var(--acao-consultar);color:var(--fundo)}
.botao.mini.incluir,.botao.mini.alterar,.botao.mini.excluir,
.botao.mini.marcar,.botao.mini.consultar{padding:4px 10px;font-size:11px;font-weight:500}

/* A legenda das cinco cores, para a convencao ser aprendida uma vez. */
.cores-acao{display:flex;gap:16px;flex-wrap:wrap;align-items:center;
  font-size:11px;color:var(--texto-3);margin:10px 0 0}
.cores-acao b{display:inline-flex;align-items:center;gap:5px;font-weight:500}
.cores-acao i{width:10px;height:10px;border-radius:3px;display:inline-block}"""
assert antigo in s
s = s.replace(antigo, novo)
p.write_text(s)
print("ok")
