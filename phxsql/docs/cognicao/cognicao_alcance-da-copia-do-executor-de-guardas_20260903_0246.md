# Cognição: o alcance do `COPIAR` de `bancada/guardas/` não cobria `docs/`

## 1. O que aconteceu

Ao tentar rodar o catálogo completo de `bancada/guardas/` (60 entradas, para
regravar a tabela de `docs/TESTES.md` §8 depois de acrescentar três guardas
novas), a **árvore limpa** — a checagem que roda ANTES de qualquer defeito
ser reposto, para garantir que o veredito não vem de um teste já vermelho —
reprovava:

```
phxsql-core --lib   VERMELHA   6.7 s   252 testes
  panicked at src/error.rs:476:33
  nao li /root/.cache/phx-guardas/docs/ROTEIRO-1.0.md: No such file or directory (os error 2)
```

O teste `error::testes_codigo::nenhuma_sprint_citada_e_inventada` lê
`docs/ROTEIRO-1.0.md` em **tempo de execução**
(`std::fs::read_to_string`, e não `include_str!`) para conferir que nenhuma
sprint citada num erro é inventada. O `COPIAR` do executor
(`bancada/guardas/provar-guardas.py`) sempre foi `["Cargo.toml", "Cargo.lock",
"crates", "exemplos"]` — sem `docs/`.

## 2. O que eu concluí primeiro, e estava errado

A primeira leitura do sintoma — uma falha na árvore limpa, sem eu ter tocado
em `phxsql-core` — apontava direto para o padrão já documentado no
`LEIA-ME.md` daquela pasta: **"duas árvores de trabalho na mesma máquina
disputam a mesma cópia, e o estrago engana"** — três vereditos de mentira já
aconteceram exatamente assim numa rodada anterior. Havia, de fato, outra
frente mexendo em `crates/phxsql-store/src/table.rs` na mesma máquina nesta
sessão. Cheguei a considerar forçar uma cópia nova (`--limpar`) achando que
era contaminação entre rodadas concorrentes.

Estava errado: `diff` entre o `error.rs` da cópia e o da árvore de verdade
deu **vazio** — o arquivo Rust estava idêntico. A causa não era um arquivo
mutado por outra rodada; era um arquivo **ausente**, porque nunca esteve na
lista do que se copia. A pista que devia ter me levado lá direto — o `LEIA-ME`
já registra que `crates/` sozinho não compila por causa de um
`include_str!` de `exemplos/`, que é exatamente esta classe de problema, só
que resolvida — mas eu li o sintoma (falha na árvore limpa) e associei ao
padrão mais recente e mais dramático (contaminação entre rodadas), não ao
mais simples (lista incompleta).

## 3. O que a medição disse

`ls ~/.cache/phx-guardas/docs/ROTEIRO-1.0.md` — não existe. Não é uma versão
velha do arquivo (o que confirmaria contaminação); é a pasta `docs/`
inteira ausente da cópia, porque nunca esteve em `COPIAR`. Depois de
acrescentar `"docs"` (6,3 MB, `du -sh docs/`) à lista, a árvore limpa passou
a ficar verde nas mesmas 252 provas — sem tocar em nada além da lista.

## 4. A regra

**Sintoma "árvore limpa reprova" tem duas causas bem diferentes, e a
distinção é rápida de medir**: `diff` entre o arquivo da cópia e o da árvore
de verdade. Se o arquivo é IDÊNTICO ou AUSENTE, a causa é o `COPIAR` — uma
lista que não alcança o que o código passou a precisar. Se o arquivo
DIFERE, a causa é contaminação entre rodadas (o padrão já documentado). E o
corolário já conhecido desta casa se repete aqui, num lugar novo: **quando
um gerador (aqui, a cópia isolada) depende de uma lista, a lista tem de sair
do código** — e até isso ter uma resposta melhor, o `COPIAR` continua sendo
uma lista digitada que qualquer novo `std::fs::read_to_string` fora de
`crates/`/`exemplos/` pode furar de novo, em silêncio, sem quebrar a
compilação (só a árvore limpa, que é fácil de atribuir à causa errada).

## 5. Como está guardado hoje

`bancada/guardas/provar-guardas.py`, linha do `COPIAR`: comentário
explicando por que `docs/` entrou, com o teste e o arquivo que a motivaram.
**Não está guardado por um teste automático** — não há guarda que impeça um
próximo `std::fs::read_to_string` de fora de `crates/`, `exemplos/` ou
`docs/` de reabrir o mesmo buraco num quarto diretório; o comentário é a
memória, e a árvore limpa continua sendo quem primeiro sente. Registrar essa
lacuna aqui é o "onde o buraco ficou" que a seção 5 deste formato pede.
