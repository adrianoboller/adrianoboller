# A janela do `.ndx` aberta pelo passo inteiro da cascata quebra a neta

**Estado:** PENDENTE

## O que aconteceu

Pedido 490: um pânico entre duas filhas da cascata do `ao_alterar`, fora de
transação, deixava as filhas seguintes na chave velha **sem recusa** — o `Drop`
do desenrolar achava `escritas_em_voo = 0` (a janela do `.ndx` da filha abre e
fecha a cada linha) e baixava o byte 52. Um `SIGKILL` no mesmo ponto deixa o
byte em 1 e a tabela recusa. Medido antes do conserto:
`panico_entre_duas_filhas_deixa_a_filha_recusando_como_um_sigkill` caía com
«a tabela nao recusa», e pelo soquete o `buscar` na filha respondia `ok` com a
órfã dentro.

## O que eu concluí primeiro, e estava errado

Que a receita do parecer do papel C bastava: «a janela do `.ndx` da filha fica
aberta pelo passo inteiro da cascata, não por linha» — `comecar_escrita` antes
do laço, `terminar_escrita` depois. Ela fecha o 490 (o teste passou), e eu teria
parado ali.

## O que a medição disse

Com a janela do passo inteiro, `a_cascata_alcanca_a_neta` (16 de 17 verdes no
`cascata-ao-alterar`) passou a **recusar toda cascata de três níveis, com a avó
já gravada**: a escrita em voo segura o `sincronizar` da filha, o byte 52 fica
em 1 no disco, e a neta confere a chave dela num **segundo descritor** — que
bate na guarda de visibilidade («a guarda de visibilidade de pedidos.ndx
recusou responder»). O conserto do pânico virava um defeito no caminho que
funciona.

O que ficou: uma marca **só do `Drop`** (`NdxFile::cascata_em_voo`), ligada em
toda filha desde que a mãe vai ao disco até o passo dela terminar. Ela não
segura `sincronizar` nem `fechar`; o `Drop` que a encontra ligada levanta o
byte 52 e não baixa. 17 de 17 verdes, e a janela anterior (mãe gravada, texto e
diário por fazer) coberta pelo mesmo mecanismo
(`panico_depois_da_mae_e_antes_da_primeira_filha_tambem_recusa`).

## A regra

Antes de estender uma janela de escrita em voo, pergunte quem LÊ aquele arquivo
por um segundo descritor enquanto ela está aberta — a janela que segura o byte
52 no disco também tranca a conferência da neta.

## Como está guardado hoje

Guardas `cascata-em-voo-ignorada-no-drop` e `cascata-em-voo-so-no-aplicar` no
catálogo, com `a_cascata_alcanca_a_neta` no `seguem` das duas — é ele que
acusaria a volta da receita do passo inteiro. O que continua: fora de
transação a cascata interrompida **não se completa** (não há marca); a
garantia é só a da queda — a tabela recusa até o `reindexar`, e as órfãs ficam
à vista de quem busca pela chave velha e do `--example conferir-integridade`.
