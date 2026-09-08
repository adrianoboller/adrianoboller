# Cognição: o compilador acha o irmão quando a regra vira TIPO

**Descoberta:** 08/09/2026, ~16:15 UTC, desenhando o índice parcial da frente
F-NÚCLEO (as dezoito do comparativo), antes de escrever o código do
`table.rs`.

## 1. O que aconteceu

O índice parcial (`onde`) muda UMA pergunta do motor: «esta linha entra neste
índice?». Hoje a resposta é sempre sim, e ela está implícita em todo lugar
que calcula chave: `todas_as_chaves` devolvia `Vec<Vec<u8>>`, uma chave por
índice, e cada caminho de escrita fazia `for (i, chave) in chaves` sem
perguntar nada.

A casa já pagou três vezes em 03/09 pelo padrão «conserto entra no caminho
que o motivou, e o irmão fica» (pedidos 172, 173 e 176). A pergunta desta vez
era como NÃO pagar a quarta.

## 2. O que eu concluí primeiro, e estava errado

Que bastava uma função nova, `chave_se_pertence`, chamada dos **três**
caminhos de escrita que eu tinha na cabeça — `inserir`, `atualizar` e
`excluir_de_vez` —, e um `grep` para conferir que não sobrava nenhum. Três
era o número da memória, e a memória é exatamente o que a lei dos irmãos diz
que falha.

## 3. O que a medição disse

O `grep` por `todas_as_chaves` achou **5** chamadores, não 3: além dos três,
a **marca de exclusão suave** (que reescreve a chave quando a coluna de
sistema está num índice) e o `recascatear`. E `codificar_chave` direto tinha
mais **5**: o `buscar` (duas vezes — a chave pedida e a conferência da linha
pendente da transação), o `intervalo` (duas) e o `reindexar`. Dez lugares
onde a pergunta «pertence?» estava implícita, e eu teria acertado três.

Então a regra não entrou como função a mais: entrou como **mudança do
tipo** que todos consomem. `todas_as_chaves` passou a devolver
`Vec<Option<Vec<u8>>>`, e o `cargo build` listou sozinho cada consumidor
que ainda tratava a chave como certa — o `match` de tipos incompatíveis no
`trocar_chaves` foi o único erro humano, e apareceu na hora. Nenhum dos dez
ficou para trás, e nenhum foi achado por leitura.

O mesmo desenho separou o que NÃO devia mudar: a chave que um chamador
**pede** para buscar (`chave_de_busca`) não reavalia a expressão do índice,
porque quem procura por `lower(nome)` já manda o valor baixo — e reaplicar
estaria certo para `lower` por acaso e errado para qualquer expressão que não
seja idempotente. Aí o tipo não ajuda, e foi decisão escrita.

## 4. A regra

**Quando uma regra nova precisa alcançar caminhos irmãos, mude o TIPO que
todos consomem, e deixe o compilador listar os irmãos; conte os chamadores
por `grep` só para conferir que o tipo alcançou todos.**

## 5. Como está guardado hoje

- `crates/phxsql-store/src/table.rs`: `todas_as_chaves` devolve
  `Vec<Option<Vec<u8>>>`, com o comentário dizendo que é `Option` de
  propósito; `trocar_chaves` é o lugar único do «sai/entra/muda» para
  `atualizar` e para a marca.
- `crates/phxsql-store/tests/regras-de-esquema.rs`: o teste do índice
  parcial exercita as quatro portas (inserir, atualizar nos dois sentidos,
  excluir de vez, `reindexar`) e caiu com o `onde` ignorado — prova real.
- **O buraco:** a regra vale para o que o tipo alcança. Um caminho novo que
  calcule chave sem passar por `todas_as_chaves`/`chave_se_pertence` — o
  `.fts`, por exemplo, tem indexação própria — não é alcançado por ela, e a
  guarda contra isso continua sendo a lei dos irmãos, não o compilador.
