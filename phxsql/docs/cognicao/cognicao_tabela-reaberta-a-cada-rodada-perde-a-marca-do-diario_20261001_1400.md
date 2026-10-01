# Tabela reaberta a cada rodada perde a marca do diário — e o custo do 330 não era só do arranque

**Estado:** PENDENTE

## O que aconteceu

O pedido 330 dizia que a primeira rodada do bidirecional depois do arranque
reabsorve o diário local inteiro no mapa de toques (`absorver_diario_local`,
`servidor.rs`). A premissa foi medida antes do conserto com
`cargo run --release --example custo-da-absorcao-do-bidi -p phxsql-store -- N`,
na receita do servidor (lotes de 500, teto de 16 MiB, `valores_da_imagem`,
`HashMap` por chave).

E a medição achou um segundo custo, que o pedido não nomeava: **toda rodada
seguinte com um evento local novo** também caminhava o diário, porque
`abrir_para_bidi` reabre a tabela a cada rodada e a tabela reaberta nasce sem
a `MarcaDoDiario`. Ler o evento `vistos` era andar do começo do volume até ele,
cabeçalho por cabeçalho — e o diário, sem `diario.bytes_por_volume`, é um
volume só.

## O que eu concluí primeiro, e estava errado

Que o 330 era um custo de **arranque**: pago uma vez por processo e por tabela.
O comentário de dentro da função reforçava isso — «A `Table` fica aberta entre
os lotes, então a marca do diário faz cada lote continuar de onde o anterior
parou» —, e é verdade **dentro de uma rodada**. Entre rodadas a marca morria
junto com a tabela, e o comentário não dizia.

E, na prova pelo soquete, concluí primeiro que um teto na espera máxima de um
`inserir` concorrente provava o conserto. Flocou: com o conserto e zero eventos
absorvidos sob a exclusiva, a espera máxima foi 225 ms, 447 ms, 460 ms e
**2,01 s** em corridas seguidas — o pico era o `fsync` do próprio `inserir` num
disco dividido com outras frentes compilando (a absorção parou no mesmo
instante, o que a trava não explicaria).

E concluí que a ficha compartilhada abria qualquer tabela sã. Não abre a que foi
escrita desde o último fecho da janela: o cabeçalho do `.log` só vai a disco no
`sincronizar`, e `Table::abrir_para_ler` recusa a cauda além dele («o diário
.log ficou para trás numa queda e precisa de cura») — corrigir o cabeçalho é
escrever. O teste unitário passava sozinho e caía com quatro em paralelo, porque
o fecho da janela dependia do relógio. Sob um escritor da mesma tabela, era a
recusa de toda fatia: a absorção voltava inteira para a exclusiva, calada. A
saída foi contar a cauda **na memória** (`LogFile::curar_em_memoria`, aberta por
`Raiz::abrir_diario_para_ler`): sob a compartilhada nenhum escritor anexa, e cada
evento além do `fim` se confere pelo CRC dele.

## O que a medição disse

Release, 01/10/2026, diário de 1 M eventos, três corridas:

| rodada | custo |
|---|---|
| primeira depois do arranque (mapa vazio) | 2,26–2,63 µs/evento, 2,26–2,52 s |
| seguinte, 1 evento novo, **sem** a marca | 507–517 ms |
| seguinte, 1 evento novo, **com** a marca guardada | 0,1 ms |

E as duas com a trava **exclusiva** de dados na mão — servidor parado para
escrita e leitura. A 100 k a rodada seguinte custava 72,6 ms; a 300 k, 144,7 ms:
linear no tamanho do diário, a cada rodada em que alguém escreveu.

## A regra

Estado que precisa sobreviver entre rodadas não pode morar no objeto que a
rodada reabre: quem reabre a tabela a cada passada guarda a marca do diário
fora dela. E prova de trava em ambiente com disco dividido conta **quem andou
durante** o intervalo, não cronometra o pior caso.

## Como está guardado hoje

- `MapaDeToques::marca` (`bidirecional.rs`), gravada a cada lote em
  `absorver_diario_local`; teste
  `servidor::testes_da_absorcao_do_bidi::a_rodada_seguinte_comeca_da_marca_guardada`
  e guarda `bidi-rodada-seguinte-sem-a-marca-do-diario`.
- A primeira rodada sob a trava de leitura, em fatias de 10 ms
  (`pre_absorver_sob_leitura`); teste
  `servidor::testes_da_absorcao_do_bidi::a_primeira_rodada_absorve_o_grosso_fora_da_exclusiva`,
  guarda `bidi-absorve-o-diario-sob-a-exclusiva`, e a prova pelo soquete
  `tests/absorcao-do-bidi-no-arranque.rs`, com o escritor na MESMA tabela, que
  conta os `inserir` que terminam com o mapa pela metade (5–38 com o conserto;
  o meio nem existe com o defeito, que segurou o escritor 1,8–2,3 s e passou os
  300.022 eventos pela exclusiva).
- A cauda do `.log` contada na memória: teste
  `leitura::testes::o_diario_conta_a_cauda_na_memoria_sem_gravar` (o `.log` sai
  byte a byte igual) e guarda `diario-sob-a-compartilhada-recusa-a-cauda`.
- **O buraco que fica:** o mapa continua sem teto de RAM (88–114 B por chave,
  medido pelo J). O número agora sai no `replicacao_estado` (`toques_no_mapa`),
  e a saída de verdade — o último toque na própria linha — é a parte (b) da
  decisão do J, do papel C, fora desta onda.
