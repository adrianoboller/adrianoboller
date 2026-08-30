# Mark #123 done and recount
# 29/08 00:04

import pathlib
p = pathlib.Path("docs/PENDENCIAS.md")
s = p.read_text()
alvo = '''| ☐ | 123 | **Janela de conflito de escrita** | a melhor ideia do PDF do HFSQL(R): o segundo a salvar vê «valor anterior / o outro escreveu / você escreve» e escolhe. Hoje a segunda gravação vence em silêncio — e a peça já está no formato: o `.reg` guarda uma **versão por registro**. É o item mais barato com o maior ganho de correção |'''
novo = '''| ☑️ | 123 | **Janela de conflito de escrita** | feito **sem mudar formato**: a versão por registro do `.reg` estava lá desde a v1. `ler` devolve a versão com `"com_versao"`, `atualizar`/`excluir`/`restaurar` conferem a versão que o cliente mandar, e a recusa é o erro **3004 `CONFLITO`**. A janela mostra as três colunas do PDF e vai além dele: **já vem marcado quem mexeu em cada coluna**, então dois que editaram campos diferentes saem com os dois trabalhos. A conferência é **pedida, não imposta** — cliente antigo continua gravando |'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)
s = s.replace("**107 feitos · 7 parciais · 13 planejados**, de 127 pedidos.",
              "**108 feitos · 7 parciais · 12 planejados**, de 127 pedidos.", 1)
p.write_text(s)
print("ok")
