# Prova diferencial contra o `HEAD` expira no proprio commit

**Estado:** FRUTÍFERO
**Evidência:** `42e1bf3`; `docs/dossie/prova-do-depois-da-versao.py`

Papel A (integração), 24/09/2026, pedido 484.

## 1. O que aconteceu

A prova do quarto estado (`docs/dossie/prova-do-depois-da-versao.py`) comparava o
gerador novo com o de «antes» lido por `git show HEAD:...`, e montava o mundo «sem
`⏸`» lendo o `PENDENCIAS.md` real. As duas premissas eram verdade só no dia da
frente. Quando apliquei a triagem do escopo (51 pedidos viram `⏸`), a prova caiu
com rc=1: o leitor de «antes» parou no pedido 209, que agora era `⏸` no arquivo real.

## 2. O que eu concluí primeiro, e estava errado

Que bastava normalizar o arquivo, trocando cada `⏸` por `☐` na cópia da prova. Isso
consertava o que eu via, mas a outra metade ia quebrar em silêncio. Depois do commit,
o `HEAD` passa a ser o gerador novo: o «suporte reposto ausente» deixa de estar
ausente, e a prova acusaria o conserto de ser o defeito. A prova vermelha que eu vi
era de um motivo, e a próxima viria de outro.

## 3. O que a medição disse

- Antes do conserto, com a triagem aplicada: rc=1 (o leitor de antes parou no 209).
- Com o «antes» num commit fixo (`257f854`, o último sem `⏸`) e a cópia normalizada:
  rc=0, com todos os `ok`, incluindo as 6 páginas byte a byte iguais sem `⏸`.
- Depois do commit `42e1bf3`, a mesma prova com `ANTES = "HEAD"`: **rc=1** (quebra
  em `ex_velho.ler_pedidos()`, porque o «extrair.py de antes» já é o novo). Com o
  commit fixo: **rc=0**.

## 4. A regra

**Prova diferencial fixa o «antes» num commit com nome, nunca no `HEAD`, e monta o
mundo sem a mudança por conta própria, nunca lendo o arquivo vivo.**

## 5. Como está guardado hoje

Só nesta prova (`ANTES = "257f854"` e `sem_depois()`). Não há conferidor que ache
outra prova do repositório com `git show HEAD:` em papel de «antes». Esse buraco não
virou pedido: é endurecimento, e a versão está congelada.

## 6. O alcance, pago de novo 70 minutos depois

A mesma armadilha, com o sinal trocado: a prova do `portoes.sh` (pedido 421)
montava a árvore do HEAD e passou na árvore exata porque o HEAD ainda NÃO a
continha. No commit `6fd7d5f` ela entrou no HEAD, o `todas.py` a achou como
catraca, e ela se rodou dentro da própria árvore temporária. O `comunicacao.sh`
acusou em minutos. **Árvore montada do HEAD mente nas duas direções:** antes do
commit falta a peça nova, e depois ela mesma está lá. As peças sob prova vão
da cópia viva.

