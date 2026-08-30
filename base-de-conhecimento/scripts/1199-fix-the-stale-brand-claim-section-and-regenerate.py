# Fix the stale brand-claim section and regenerate
# 29/08 19:52

import io
p="phxsql/docs/PENDENCIAS.md"
s=io.open(p,encoding="utf-8").read()
velho = """## 7. Duas afirmações da folha de marca que continuam falsas

Registrado no `CLAUDE.md` e repetido aqui porque é fácil esquecer: a folha diz
*ACID compliant* e *built-in replication*. **Nenhuma das duas é verdade hoje** —
não há transação, e a replicação não transporta evento. Não repetir em documento
técnico enquanto não forem."""
novo = """## 7. Uma afirmação da folha de marca que continua falsa — e uma que virou verdade

Registrado no `CLAUDE.md` e repetido aqui porque é fácil esquecer: a folha diz
*ACID compliant* e *built-in replication*.

- ***ACID compliant* continua falso.** Não há transação: sem `commit` e sem
  `rollback` de várias operações, e a própria tela de Transações diz isso em
  letra grande. Existe o desfazer de UMA inserção quando o índice recusa, e é
  outra coisa. **Não repetir em documento técnico enquanto não for.**
- ***built-in replication* deixou de ser falso.** O pedido 19 registra a
  medição: `.log` v2 com a imagem da linha, ops `posicao`/`replicar`/`aplicar`,
  o laço dentro do `phxsqld`, quatro servidores medidos e retrato SHA-256 das
  quatro tabelas idêntico. Os quatro modos estão em `REPLICACAO.md` §9.

Esta seção mesma é a lição: ela afirmava «a replicação não transporta evento»
enquanto o item 19, no MESMO documento, trazia a medição do contrário. **A lista
do que falta também é palpite até alguém medir** — inclusive a lista de
afirmações falsas."""
assert s.count(velho)==1
io.open(p,"w",encoding="utf-8").write(s.replace(velho,novo))
print("ok")
