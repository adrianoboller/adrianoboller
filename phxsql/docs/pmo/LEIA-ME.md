# `docs/pmo/` — o board de controle e o painel PMO

A pasta do controle de projeto: o que está aberto, de que pilar é, que escalão
merece, e a página que mostra isso ao dono. Nada aqui é entregável de produto;
é apoio de processo (cláusula pétrea dos `.md` de apoio, `CLAUDE.md`).

| Arquivo | O que é | Edita-se? |
|---|---|---|
| `BACKLOG.md` | o board: quatro tabelas de item (Pilar 1, 2, 3, Governança) com dono, escalão e estado | **sim** — é a avaliação datada de escalão e dono; o bloco entre `<!-- ROLLUP:inicio -->` e `<!-- ROLLUP:fim -->` **não**, é gerado |
| `rollup.py` | conta aberto / entregue-fechado / parado por pilar e os «forte» abertos, e regrava o bloco do rollup no `BACKLOG.md` | — |
| `pagina-do-status-do-projeto.py` | gera o painel PMO (três vistas: painel, fluxo, equipe) | — |
| `status-do-projeto.html` | a página gerada, publicada em https://claude.ai/artifact/VEC7fc38SF5t2fRqrwEy8p | **não** — mexeu numa fonte, rode o gerador |
| `RODADA-*.md` | o quadro de uma rodada: medições, contratos das frentes, encontro das frentes | sim |

## Como rodar

```bash
python3 docs/pmo/rollup.py                         # regrava o rollup do board
python3 docs/pmo/pagina-do-status-do-projeto.py    # regrava a página
python3 docs/dossie/portao-dos-geradores.py        # confere que nenhum derivado esta velho
```

Os dois geradores estão no **portão dos geradores** em modo `sem-carimbo`
(o carimbo «gerado em … UTC» é relógio de parede; qualquer outro número que
mude ao re-rodar reprova). Publique a página **passando a URL acima**, para
cair na mesma página em vez de criar outra.

## De onde sai cada número da página

- **Pedidos** (total, feitos, parciais, planejados): `docs/PENDENCIAS.md`, pelo
  mesmo `ler()`/`ESTADOS` de `docs/dossie/pagina-dos-pedidos.py` — uma receita
  só para o mesmo número, para as duas páginas nunca divergirem.
- **Board**: `BACKLOG.md`, pelo `ler()` do `rollup.py`. O estado é a PRIMEIRA
  palavra da última célula (aberto / entregue / fechado / parado); palavra fora
  do léxico PARA nomeando a linha.
- **Motor**: `CAPABILITIES.json`, cada cartão com a data `medido_em` ao lado.
- **Últimas frentes**: `git log -n 8`, só leitura.
- **Gates externos**: pedidos ☐ ou ◐ cujo texto casa o léxico explícito
  `LEXICO_GATE` do gerador («decisão do dono», «parado por decisão», «bloqueio
  externo»…). A página mostra a frase que casou — quem lê julga o casamento.
- **Equipe**: `.claude/agents/*.md` (frontmatter `name`/`description`), o
  `README.md` da pasta (papel → agente) e a tabela de escalão do
  `docs/MODELOS.md`. O escalão aparece pelo **nível** (forte / meio / leve),
  nunca por nome de modelo; papel sem escalão registrado diz isso.

Se uma fonte faltar, o gerador **para com o motivo** — nunca grava zero calado.

## O que a captura achou que o código não mostrava

Quatro defeitos só apareceram exercitando no navegador (a lei da casa): a
legenda «FALHA com o defeito reposto…» atravessando a caixa do fluxograma; o
«sim» do losango encostando na caixa PARADO; a caixa do J esticando a grade
inteira do organograma (os subagentes ganharam fila própria); e o nome da
branch partindo no meio da palavra em Exo 2 (virou mono, com quebra só no
hífen). O capturador é o `docs/dossie/olhar.mjs`, pelo caminho absoluto.
