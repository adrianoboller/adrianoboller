# O `grep` acha o spawn, e não o teto — e o aceitador de uma thread só serializa as recusas

Data e hora da descoberta: 16/09/2026, 05:50 UTC (a primeira); 06:0x UTC (a
segunda). Frente T da rodada de threads (pedido 248).

## 1. O que aconteceu

O quadro da rodada (`docs/pmo/RODADA-2026-09-16-threads-e-disco.md`) foi
montado com `grep`/`sed` e dizia que o fecho da janela de durabilidade tinha
**«K fios, sem teto»** (`servidor.rs:14216`, `std::thread::scope`). O contrato
da frente mandava «medir antes de limitar» esse fecho.

Lendo o sítio antes de mexer: o `scope` está dentro de um
`for pedaco in lista.chunks(FIOS_DO_FECHO)` **28 linhas acima**, e a constante
`FIOS_DO_FECHO = 16` mora na linha 607, com um comentário de vinte linhas
explicando por que existe e mandando medir antes de mudá-la. A §12.6 do
`docs/CONCORRENCIA.md` já a registrava com o número medido (2,52× em K=16).
O teto existia, estava documentado, e o quadro não o viu.

A segunda: a primeira versão do teto da porta web era «espera `fila_web_ms`,
depois 503». Desenhando a prova real percebi que o aceitador é **uma** thread:
numa saturação longa, cada pedido esperaria os 2 s inteiros e o aceitador
entregaria **um 503 a cada 2 s** — a fila do `listen` estouraria por trás dele
e o cliente nem 503 receberia, só um `connect` sem resposta.

## 2. O que eu concluí primeiro, e estava errado

- Que o quadro estava certo sobre o fecho, porque tinha arquivo e linha ao
  lado. Arquivo e linha dão a **sensação** de medição; o que eles mediram foi
  a linha do spawn. O teto de uma thread quase nunca está na linha em que ela
  nasce — está num `chunks`, num `Semaforo`, num `pode_avisar` que roda antes,
  numa lista fixa do config.
- Que «`adquirir_ate(fila)` no aceitador» era a fila curta inteira. Era a
  metade: o comportamento sob saturação **prolongada** só apareceu ao imaginar
  o 65º, o 66º e o 500º cliente na sequência, e não o primeiro.

## 3. O que a medição disse

- O fecho, medido com `--example o-comboio-em-paralelo --tetos 4,8,16,0 16
  2000 30`, três corridas limpas: teto 4 custa **+34% a +68%**; teto 8 custa
  **+19%, +25% e +0,1%**; e «16» contra «sem» — o mesmo código — divergiu até
  **27,6%**, que é o ruído. Nenhum teto menor passa nos 5%: fica 16, que já
  era. Corridas em `bancada/concorrencia/corridas/fecho-com-teto-20260916-0608-*.txt`.
- A fila declarada cheia, na `enxurrada-web.py` (500 conexões seguradas, teto
  64, fila 2.000 ms): o primeiro 503 chega em ~3 s e os outros **435 em p50
  de 21 ms**; 436 de 436 recusas com `Retry-After`, zero `reset`. Sem o
  atalho, o 436º esperaria 436 × 2 s.
- O mapa das threads (`mapa-das-threads.py`) achou **19 sítios** em
  `crates/*/src` fora dos testes; com o catálogo escrito à mão, 0 sem teto.

## 4. A regra

**Quem inventaria threads por `grep` lista onde elas nascem; o teto alguém tem
de ler — e o catálogo diz onde ele mora.** E: quando um portão com espera
mora numa thread só, imagine o centésimo cliente antes do primeiro.

## 5. Como está guardado hoje

- `bancada/concorrencia/mapa-das-threads.py`: o catálogo à mão, com o teto e a
  linha onde ele mora, e a catraca `spawn-sem-teto = 0` +
  `catalogo-envelhecido = 0` como item 0c da `prova-bateria.py`. O
  `--autoteste` repõe sete defeitos do próprio medidor.
- `Servidor::vaga_http` e `http_cheia_ate_ms` no `servidor.rs`, com o motivo
  no comentário; a §17.4 do `docs/CONCORRENCIA.md` conta a versão incompleta
  e o número que a corrigiu.
- O que **não** está guardado: a medição do fecho vale para o `ext4` desta
  máquina, como a §12.6 já avisava — noutro sistema de arquivos o `--tetos`
  se roda de novo antes de acreditar no 16.
