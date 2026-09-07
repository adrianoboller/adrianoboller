# 0.1) Índice de texto — o .fts contra a varredura. Onde fica os .fts por que o dossiê não tem no gráfico: Organograma dos arquivos?

*Medido em 2026-09-07 16:39 UTC, commit `a56a165`.*

## Resposta curta

Você apontou certo: o `.fts` **não estava na Figura 1** do dossiê («Organograma
dos arquivos» — *cada tabela é: SEMPRE os sete, SÓ ÀS VEZES três*), e medido
antes de mexer ele **também não estava** na Figura 8 (o caminho de uma
inserção, que desenhava 8 extensões) **nem na tabela-mestra de arquivos do
`docs/FORMATO.md`** (9 linhas, sem `.fts`). Três lugares, o mesmo buraco.

A causa é uma só: o inventário «que arquivos uma tabela tem» mora em **três
lugares escritos à mão** e nenhum sai do código — a lista certa é a de
`arquivos_da_tabela` no `catalogo.rs`. O `.fts` entrou no motor em
07/09/2026 (pedido 200) e ninguém voltou às figuras: *figura desenhada à mão
envelhece como número digitado à mão.*

Onde ele **fica**: ao lado dos outros arquivos da tabela, `<tabela>.fts`, um
por tabela, criado no `criar_tabela` que declara `indices_texto` — e a ordem
de gravação é `.reg → .ndx → .fts → .log` (a Figura 10, o caminho do pedido,
já dizia isso no `aria-label`). Por dentro ele é um `.ndx` — mesma assinatura
`PHXNDX\0\0`, mesma versão, mesmo CRC de página —, com uma chave por termo
dobrado. É **derivado** (se reconstrói do `.reg`) e por isso fica **fora do
desfazer** de uma inserção que falha no meio (`docs/FTS.md` §2.1).

## Exemplo exercitado

O que a medição achou e o que ficou depois do conserto, nesta rodada:

```text
                              antes                              depois
Figura 1  (organograma)       SÓ ÀS VEZES: .lgpd .bkp .pag       .lgpd .bkp .pag .fts   (4 condicionais)
Figura 8  (caminho da inserção) bin bkp lgpd log memo ndx pag reg + fts, pendurado no .ndx, tracejado,
                                                                 «só com índice de texto · fora do desfazer»
FORMATO.md, tabela de arquivos  9 linhas, sem .fts               10 linhas, + §17 «.fts — o índice de texto»
ocorrências de «.fts» no dossiê 4 (nenhuma em figura)            nas duas figuras, com legenda e aria-label
```

As duas figuras foram **provadas no navegador** (captura de cada uma, com o
texto `.fts` lido de dentro do SVG), porque figura só se prova exercitando.

O `.fts` **contra a varredura** — o número que responde «vale a pena?» — está
na bancada `bancada/fts/` e na página de testes («Índice de texto — o .fts
contra a varredura», medida em 07/09/2026 01:09), e no `docs/FTS.md` §4.1.2,
o índice contra a varredura **no milhão de linhas**.

## O que NÃO existe, e é dispensa registrada

- **As figuras não saem de gerador.** Gerar a Figura 1 e a Figura 8 da lista
  `arquivos_da_tabela` do código é o conserto de raiz e fica **nomeado** em vez
  de feito: cada caixa carrega um texto de decisão («se o espelho está
  ligado», «fora do desfazer, de propósito») que um gerador não inventa. O que
  cabe é uma **guarda**: um teste que leia a lista do código e confira que
  toda extensão dela aparece nas duas figuras e na tabela do `FORMATO.md` —
  vira pedido.
- O `.tx` (marca de commit em curso) ficou **de fora da Figura 1 de
  propósito**: é do database, não da tabela (`FORMATO.md` §16).

## Como se refaz

```bash
python3 docs/dossie/numerar-figuras.py      # renumera; a figura em si é o HTML
python3 bancada/fts/medir.py 1000000 20     # o .fts contra a varredura, no milhão
```
