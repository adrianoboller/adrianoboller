# Crase dentro de template literal derruba o arquivo inteiro

- **Quando:** 2026-09-02, 17:32 (e de novo às 19:32, no mesmo dia)
- **Onde:** `crates/phxsql-server/ui/telemetria.js:620`, e depois num gerador
  de PDF fora da árvore
- **Custo:** 31 casos da bateria reprovados nos dois temas, com quatro
  mensagens diferentes e **nenhuma delas nomeando arquivo ou linha**

## O que aconteceu

Escrevi um comentário explicando por que o gestor de threads nasce fechado, e
dentro dele pus a palavra `toggle` entre crases. O comentário mora **dentro de
um template literal** — a crase fechou a string, o arquivo parou de compilar,
`PhxTelemetria` virou `undefined`, e a bateria reprovou dizendo
`Unexpected identifier`, `PhxTelemetria is not defined` em `campoDeCor`, em
`telaTelemetria` e em `verConfigServidor`.

Pior: **capturei e entreguei telas antes de rodar a bateria.** A prova vem
antes da entrega, e nesse dia não veio.

## O que eu concluí primeiro, e estava errado

Que a caçada à falha intermitente da telemetria estava medindo o produto. Ela
estava medindo uma página quebrada por mim, e cada rodada custava cinco
minutos.

## O que a medição disse

`node --check crates/phxsql-server/ui/telemetria.js` responde em milissegundos
com **arquivo, linha e o token exato**. A bateria inteira leva ~5 min para
dizer menos.

## A regra

**Comentário dentro de template literal não leva crase.** E, mais geral: erro
de sintaxe não é assunto de teste de ponta a ponta — é portão, e vem antes de
subir servidor e abrir navegador.

## Como está guardado hoje

`conferirSintaxeDaInterface()` em `testes-web/bateria.mjs`, rodando **antes** de
`conferirBinario`. Custa ~200 ms. Cobre todo `.js` de `ui/` e cada `<script>`
embutido nos `.html`, com linhas em branco na frente para o número bater com o
arquivo de verdade. Prova real nos dois sentidos: acusa `telemetria.js:620` com
a crase reposta e `index.html:2057` com um `return );` plantado.

**O buraco que ficou:** o portão cobre **só `ui/`**. Às 19:32 do mesmo dia
cometi o mesmo erro num script de geração de PDF no scratchpad, e nada o pegou
— o `node` falhou, mas os PDFs anteriores continuavam em disco e o conferidor
olhou os velhos. Ver `cognicao_saida-velha-mente_20260902_1932.md`.
