# Document, commit and push
# 29/08 02:13

import pathlib
p = pathlib.Path("/home/user/adrianoboller/CLAUDE.md")
s = p.read_text()
alvo = '''**Configuração que não é lida mente.**'''
novo = '''**Instrumentação desligada tem de custar zero — e o portão que decide isso vem
ANTES do trabalho.** O Profiler desligado cobrava 7% da carga pela rede: o ponto
de captura fazia dois `Json::analisar` do corpo inteiro, três `String` e um
mutex, e só então perguntava se estava ligado. Num lote de cinco mil linhas era
analisar meio megabyte de JSON duas vezes para jogar fora. O mutex era o pior
pedaço: além do custo, ele **serializa** — todo mundo na mesma fila para
descobrir que não havia o que registrar. Quando entrar um observador novo,
procure o que ele faz antes de olhar o próprio interruptor.

**Configuração que não é lida mente.**'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)
p.write_text(s)
