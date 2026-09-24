# Marca de vida sem `Drop`: a lição do 248 chegou ao registro da telemetria, e não às marcas do próprio servidor

**Estado:** PENDENTE

*24/09/2026, 16:10 — pedido 452, e o irmão no relógio de jobs.*

## 1. O que aconteceu

O supervisor do pulso marca o id em `pulsando` e sobe a thread; a thread
desmarcava na saída NORMAL (`estado.desmarcar_pulso(&no.id); return;`). Um
pânico pulava a linha, o id ficava marcado, e o supervisor — que só sobe
thread para quem ele mesmo marca — nunca mais pulsava o par. O comentário do
supervisor dizia «A própria thread desmarca ao morrer»: frase que se declara
resolvida, e por isso ninguém olhou de novo.

O irmão, medido na mesma tarde: `relogio_de_jobs` subia para `true` no
`subir_jobs` e nunca descia. Relógio morto continuava «no ar», o vigia nunca
via job parado, e a tela dizia «agendado».

## 2. O que eu concluí primeiro, e estava errado

- **«Essa lição já está paga: o `FichaViva` morre no `Drop` desde o 248.»**
  Está paga no REGISTRO da telemetria (a ficha da thread), e só nele. As marcas
  que o próprio servidor mantém sobre as threads (`pulsando`,
  `relogio_de_jobs`, `jobs_rodando`) são outra estrutura, com outra linha de
  desmarcação — e nenhuma tinha `Drop`. A lição estava escrita no comentário do
  `FichaViva` citando «o irmão que chama as mesmas funções na mesma ordem», e
  os irmãos daqui não chamam as mesmas funções.
- **«Basta um `Drop` que desmarque.»** Sem recuo, o supervisor sobe outra
  thread a cada 0,5 s, e um pânico determinístico vira laço de pânico.

## 3. O que a medição disse

- Com a desmarcação só na saída normal: **0 conexões em 5 s** depois de um
  pânico no pulso (o ouvinte do teste conta cada tentativa).
- Com o `Drop` e sem recuo: **9 pânicos em 4,5 s**. Com o recuo 1-2-4 s: **3**.
- `relogio_de_jobs` sem `Drop`: **verdadeiro 3 s** depois da morte da thread.

## 4. A regra

Toda marca «esta thread está viva» nasce junto de uma guarda cujo `Drop` a
tira — criada ANTES do `spawn` e movida para dentro da thread, para sair também
quando a thread nem nasce —; e quem sobe de novo o que morreu em pânico sobe
com recuo que dobra, não a cada volta.

## 5. Como está guardado hoje

- `crates/phxsql-server/src/cluster.rs`, `GuardaDoPulso` e o recuo em
  `marcar_pulso`; `servidor.rs`, `RelogioNoAr`.
- Provas: `servidor::testes_do_pulso_que_morre` (os dois sentidos) e
  `servidor::testes_do_relogio_e_do_backup::relogio_de_jobs_que_morre_nao_continua_dizendo_que_esta_no_ar`.
- Guardas: `pulso-que-morre-fica-marcado`, `pulso-em-panico-sem-recuo`,
  `relogio-de-jobs-morto-diz-que-esta-no-ar`.
- O terceiro irmão, achado na mesma varredura: o `amostrador` da telemetria
  (`marcar_amostrador` só subia a marca, e o retrato dizia `amostrador: true`
  de uma thread morta — medido: verdadeiro 3 s depois). Guarda
  `amostrador-morto-diz-que-esta-no-ar`, prova
  `servidor::testes_do_relogio_e_do_backup::amostrador_que_morre_nao_continua_dizendo_que_esta_no_ar`.
- **O que continua de fora:** nem o relógio de jobs nem o amostrador têm
  supervisor que os suba de novo; a marca agora diz a verdade (morto), e é o
  vigia de jobs quem avisa o job parado. Subi-los de novo seria decisão de
  desenho, não conserto.
