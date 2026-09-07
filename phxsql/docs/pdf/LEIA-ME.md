# O PDF das 26 perguntas — e o contrato de cada resposta

Pedido do dono, 07/09/2026: um PDF com 26 itens (A–Z), cada um com **exemplo
exercitado contra o motor vivo**, mais três perguntas abertas (o `.fts` no
organograma, a sequência, o prazo da trava) e a atualização do dossiê (item Z).

## O que sai daqui

- `docs/pdf/respostas/<LETRA>.md` — **uma resposta por item**, no formato
  abaixo. É o que o gerador lê.
- `docs/pdf/gerar.py` — monta `docs/pdf/phxsql-26-perguntas.html` a partir das
  respostas e imprime `docs/pdf/phxsql-26-perguntas.pdf` pelo Chromium
  (`--headless --print-to-pdf`). **Não se edita o HTML nem o PDF.**
- `bancada/<assunto>/` — o script que exercitou, para não morrer com a sessão.
- `docs/dossie/perguntas-no-dossie.py` — a seção 36 do dossiê, a **resposta curta**
  de cada item lida destes mesmos arquivos. Uma fonte, duas saídas: o PDF
  inteiro e o resumo no dossiê não podem divergir porque não há segunda cópia.

## O contrato de cada `respostas/<LETRA>.md`

```markdown
# <LETRA>) <o texto do pedido do dono, LITERAL, como ele escreveu>

## Resposta curta
Duas a cinco linhas. O que existe, o que não existe, e o número que decide.

## Exemplo exercitado
O comando (ou o pedido JSON, ou o SQL) e a SAÍDA REAL do motor, com a data e a
hora da corrida e o commit. Saída colada, não redigida. Se houver várias, em
blocos separados com o que cada uma prova.

## O que NÃO existe, e é dispensa registrada
O que o pedido supõe e o motor não tem — dito com todas as letras, e com o
motivo quando há decisão por trás. Vazio é «nada a declarar», não omissão.

## Como se refaz
O comando que reproduz o exemplo, do zero: `python3 bancada/...`.
```

## As cinco leis que valem aqui

1. **Número citado é número que não se mede.** Todo número da resposta sai de
   uma corrida DESTA rodada, datada. Doc antigo é ponto de partida, nunca
   fonte da resposta.
2. **O instrumento antes do veredito.** A sonda que dá zero prova primeiro que
   acha o que existe (controle positivo), senão o zero não vale.
3. **Dispensa registrada é decisão; silenciosa é esquecimento.** O que o pedido
   supõe e não existe entra na seção própria, com o motivo.
4. **Script que resolveu algo fica no repositório**, em `bancada/`, com
   `LEIA-ME.md` de três parágrafos: por que existe, o que mede, como roda.
5. **Rótulo se traduz, dado nunca** — e saída de motor é dado: cola-se como
   veio.

## O que cada frente NÃO faz

- Não compila: o binário está em `target/release/` (`phxsqld`, `phxsql`,
  `phxsqlcmd`). Se precisar MESMO compilar, `flock /tmp/phx-cargo.lock`.
- Não commita. Não edita `docs/dossie/`, `docs/PENDENCIAS.md`, `CLAUDE.md`.
  A integração é do orquestrador.
- Não usa porta fora da faixa que recebeu, e derruba todo servidor que subiu.
- Não deixa diretório em `/tmp`: cria em `/tmp/phx-<frente>-<pid>` e apaga.
