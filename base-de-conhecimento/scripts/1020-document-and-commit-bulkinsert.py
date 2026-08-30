# Document and commit BULKINSERT
# 29/08 03:01

import pathlib
p = pathlib.Path("docs/DESEMPENHO.md")
s = p.read_text()
alvo = '''| Inserção pela rede, linha a linha vs. lote | 2.659/s | 43.302/s | **16,3×** |'''
novo = '''| Inserção pela rede, linha a linha vs. lote | 2.659/s | 43.302/s | **16,3×** |
| Carga em lote, sem reserva vs. com `BULKINSERT` | 43.500/s | 66.500/s | **1,53×** |'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)
p.write_text(s)

p = pathlib.Path("/home/user/adrianoboller/CLAUDE.md")
s = p.read_text()
alvo = '''**Configuração que não é lida mente.**'''
novo = '''**Teste unitário não prova queda de conexão — soquete prova.** Os dez testes do
`BULKINSERT` passavam, e a prova pelo soquete mostrou que a queda da conexão
**não soltava a reserva**. A causa não estava no servidor: era o teste, porque
`socket.makefile()` do Python segura o descritor e fechar só o soquete deixa o
fd aberto — o servidor nunca via o fim da conexão. Duas lições numa: o que
depende do sistema operacional se prova contra o sistema operacional, e um teste
que passa por engano é pior que um teste que falta.

**Configuração que não é lida mente.**'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)
p.write_text(s)
