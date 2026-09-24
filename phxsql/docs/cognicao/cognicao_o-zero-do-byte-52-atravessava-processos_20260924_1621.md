# O 0 do byte 52 atravessava processos, e três lugares dependiam disso sem dizer

**Estado:** PENDENTE

## O que aconteceu

Pedido 522: o `fechar` do `.ndx` gravava o byte 52 em 0 sem `fsync`, e o papel
C provou contra o SO que o núcleo guarda esse cabeçalho limpo e perde as
páginas (ext4 sobre loop com provisionamento fino, 3/3). O conserto seguiu a
direção dele: só o `sincronizar` grava o 0; o `fechar` deixa o 1 e **atesta**,
num registro do processo, que o 1 é coerente no núcleo (`ndx.rs`,
`ATESTADOS`, amarrado ao CRC do cabeçalho).

Assim que o 0 deixou de sair do `fechar`, três coisas que moravam **fora do
processo que escreveu** quebraram — nenhuma delas citava o byte 52:

- as duas sondas de `fsync` por `strace` (`fsync-por-fecho`,
  `o-comboio-em-paralelo --contar`) semeavam num processo e fechavam a janela
  noutro: o `.ndx` abria marcado, o `sincronizar` dele não descia, e a catraca
  `TETO_FSYNC_POR_FECHO_V2` mediu **6** em vez de 8;
- o backup copia o `.ndx` como o núcleo o tem, e a restauração entregava a
  tabela recusando escrita: `restaurar_com_outro_nome_cria_o_banco_integro` e
  três do PITR caíram (4 de 1.456 da `--lib` do servidor);
- o `phxsqld` só para por sinal, então **toda** parada sob carga passaria a
  deixar tabelas recusando até alguém mandar `reindexar`.

## O que eu concluí primeiro, e estava errado

Que a mudança era local ao `ndx.rs`: «muda QUANDO o 0 se grava, sem leiaute
novo» parecia dizer que só o próprio arquivo sentiria. Pensei no processo que
reabre (o atestado) e no arranque; não pensei em quem **lê o arquivo em outro
processo esperando o 0** — as sondas de medição e a cópia de segurança.

E sobre a catraca: o primeiro impulso diante do «gasta 6 e a catraca está em
8» seria baixar o teto, como a própria mensagem manda. Seria baixar uma
catraca por um número que não mede o fecho da janela — mede o `.ndx` deixando
de ir ao disco.

## O que a medição disse

- catraca do fecho: 6 com a semeadura noutro processo, **8** com a escrita
  pendente e o fecho no mesmo processo tracado (marco `getppid` no traço) — o
  número de antes, pelo motivo certo;
- suíte do servidor antes do passe na restauração: 1.452 ok, 4 falhas, todas
  «ficou para trás numa queda» em tabela restaurada; depois, verde;
- preço no `phxsqld` de release (`preco-do-522.py`): 8 de 8 tabelas marcadas
  depois de `SIGKILL` sob carga, reconstruídas no arranque em 265–506 ms; com
  200 tabelas pequenas, 10/91/160 marcadas e 38–2.357 ms. Controle ocioso: 0.

## A regra

Quando um byte de formato muda de **quem pode escrevê-lo**, procure quem o lê
**noutro processo** — medidor, cópia, arranque — antes de procurar quem o lê
no mesmo.

## Como está guardado hoje

- `bancada/guardas/catalogo.py`: `restauracao-nao-reconstroi-o-marcado`,
  `arranque-nao-reconstroi-o-marcado`, `fechar-baixa-o-byte-52-sem-fsync`
  (as três provadas vermelhas à mão com o defeito reposto, 24/09/2026);
- as sondas se descrevem: o comentário do `sonda` nos dois exemplos diz por que
  a escrita pendente mora no processo tracado;
- **buraco:** outras ferramentas que escrevem num processo e leem noutro sem
  `sincronizar` (bancadas em Python, scripts de exemplo) não foram varridas
  uma a uma; a `carga.rs` sincroniza no fim de cada fase e passou.
