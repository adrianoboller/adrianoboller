# Update CLAUDE.md and stage
# 28/08 21:21

import pathlib
p = pathlib.Path("CLAUDE.md")
s = p.read_text()
antigo = """- **URL:** https://claude.ai/code/artifact/5c14044e-0dc5-4832-b015-224ab1e40033
- **Fonte:** `phxsql/docs/dossie/dossie-phxsql.html` (versionado, para que
  qualquer sessão consiga atualizá-lo)

Publique sempre **passando essa URL**, para cair na mesma página em vez de
criar outra. Instruções e as armadilhas de estilo em
`phxsql/docs/dossie/LEIA-ME.md`."""
novo = """- **URL:** https://claude.ai/code/artifact/5c14044e-0dc5-4832-b015-224ab1e40033
- **Fonte:** `phxsql/docs/dossie/dossie-phxsql-0.15.html` (versionado, para que
  qualquer sessão consiga atualizá-lo)

Publique sempre **passando essa URL**, para cair na mesma página em vez de
criar outra. Instruções e as armadilhas de estilo em
`phxsql/docs/dossie/LEIA-ME.md`.

O nome do arquivo mudou na 0.15.0 (era `dossie-phxsql.html`), a pedido: o
dossiê foi refeito conferindo cada seção contra o código. Os dois scripts de
números aceitam o caminho do HTML como argumento, então trocar o nome de novo
não exige editá-los."""
assert antigo in s
s = s.replace(antigo, novo)
p.write_text(s)
print("ok")
