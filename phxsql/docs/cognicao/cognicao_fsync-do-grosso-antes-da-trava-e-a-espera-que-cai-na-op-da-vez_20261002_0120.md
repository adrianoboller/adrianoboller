# O `fsync` do grosso da cópia vai ANTES da trava, e a espera do backup cai na operação que estiver na vez

**Estado:** PENDENTE

## 1. O que aconteceu

Pedido 513, passo 2, bancada `bancada/backup/retrato-com-escritor.py`
(02/10/2026, 561 MiB em 20 tabelas, escritor sem parar, 3 voltas). Dois
números não fechavam com o desenho:

- a espera máxima de um `inserir` no **passo 1** deu 51 ms numa volta e
  7.466 ms na seguinte, com a mesma cópia de ~7 s;
- nas duas passadas, a espera máxima de uma escrita ficou **~100 ms acima da
  própria fase 2** (555/646/716 ms contra 477/545/581 ms), e o aceite do
  contrato («≤ fase 2 + 10 %») **reprovava**.

## 2. O que eu concluí primeiro, e estava errado

Primeiro: que medir o `inserir`, como o contrato nomeia, media a espera do
escritor. Não mede: o escritor alterna `inserir`/`atualizar`/`excluir`, e a
fase 2 cai na operação que estiver na vez — o máximo do `inserir` é sorteio.

Segundo: que a diferença de ~100 ms fosse a tomada do portão e da trava
(`tirar_retrato` esperando o escritor em voo, `travar_dados_para_ler`). Isso
custa microssegundos. A hipótese que escrevi antes de medir: o `concluir` de
depois da trava sincroniza **meio gigabyte** de cópias da fase 1, e o escritor
que acabou de sair do portão volta para um disco ocupado — disputa de E/S,
não de trava.

## 3. O que a medição disse

- Medindo toda escrita: passo 1 **7.202 ms** [6.052–7.919]; duas passadas
  **646 ms** [555–716] (cenário A). O `inserir` sozinho continua ao lado, por
  ser o que o contrato nomeou.
- Com o `fsync` das cópias da fase 1 movido para **antes** da trava
  (`backup::sincronizar_fase_1`, fora de qualquer ficha; o `concluir` só
  sincroniza o que a fase 2 reescreveu, pelas `sujas` das `Copias`): escrita
  máxima **665 ms** [585–699] contra fase 2 631 ms [532–670] — a sobra caiu de
  ~100 ms para ~30 ms, e o aceite passou (665 ≤ 694; 665 ≤ 1/10 de 7.202). A
  hipótese valeu nos dois sentidos: reposta, a sobra volta.
- No cenário B (escritor na maior tabela) a sobra fica em ~200 ms: ali a fase
  2 reescreve ~300 MB e é esse `fsync` que o escritor encontra depois. Não
  medido separado.
- `alcancam-fsync-3` continuou **24**: o `fsync` novo é fora da trava, e o
  mapa não o viu — como devia.

## 4. A regra

**Toda E/S que não precisa da trava sai de perto dela — antes, se o dado já
existe.** O escritor que esperou a seção crítica não pode encontrar, logo
depois dela, o disco ocupado com o que podia ter sido feito enquanto ele ainda
andava. E a espera de uma seção crítica se mede em **toda** operação que a
atravessa, não na que o contrato nomeou.

## 5. Como está guardado hoje

- `sincronizar_fase_1` no `backup.rs`, chamado no `fazer_backup` do servidor
  dentro da fase 1 (sem trava); teste
  `backup::tests::so_o_que_a_fase_2_reescreveu_fica_sujo_depois_do_fsync_da_fase_1`.
- A bancada mede `escrita_max_ms` e `inserir_max_ms` lado a lado, com a nota
  do porquê no `Escritor`.
- O buraco: a sobra de ~200 ms do cenário B (o `fsync` do que a fase 2
  reescreveu) não tem como sair da trava — o dado só existe depois dela. É o
  preço da tabela grande escrita durante o backup, e o 2b (rastro físico,
  H-C) não dispara: a fase 2 é 25,4 % da cópia, abaixo dos 50 % do contrato.
