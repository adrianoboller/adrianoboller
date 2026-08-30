# Mark #124 done
# 29/08 00:33

import pathlib
p = pathlib.Path("docs/PENDENCIAS.md")
s = p.read_text()
alvo = '''| ☐ | 124 | **Direito no nível da tabela** | hoje a permissão para na base: quem lê a base lê todas as tabelas. O portão já existe e é um ponto só |'''
novo = '''| ☑️ | 124 | **Direito no nível da tabela** | `"tabelas"` dentro do objeto da base, e a regra da tabela **substitui** a da base ali — o que permite tirar `folha` de quem lê o banco inteiro **e** dar `clientes` a quem não lê o banco nenhum (interseção só resolveria o primeiro). O portão continua sendo um só; `juntar` e `unir` ganharam conferência própria porque não têm o campo `"tabela"` que ele lê. A árvore e o catálogo passaram a listar só o que dá para abrir. 9 testes |'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)
s = s.replace("**108 feitos · 8 parciais · 11 planejados**, de 127 pedidos.",
              "**109 feitos · 8 parciais · 10 planejados**, de 127 pedidos.", 1)
p.write_text(s)
