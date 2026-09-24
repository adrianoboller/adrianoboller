# Estado do processo guardado pelo caminho perde o arquivo que anda, e o contrato de quem sai do processo

**Estado:** PENDENTE

## O que aconteceu

Pedido 522, revisão do papel C (`docs/propostas/parecer-dba-522-2026-09-24.md`),
**BLOQUEIA** por dois defeitos que a frente trouxe:

- **B1** — o atestado do processo (`ndx.rs`, `ATESTADOS`) era chaveado pelo
  caminho. `renomear_tabela`, `duplicar_tabela` e `copiar_tabela_para` põem o
  `.ndx` num caminho novo sem `fsync`, e a tabela escrita desde o último fecho
  da janela chegava ao destino recusando tudo, «arquivo corrompido», sem queda
  nenhuma: 9 recusas em 9, contra 0 em 3 antes do 522;
- **B2** — o `phx_tabela_fechar` do embutido era só o `Drop`. O processo
  seguinte recebia o 1 sem atestado e recusava toda operação (3/3), e a ABI nem
  tinha `reindexar`.

## O que eu concluí primeiro, e estava errado

Escrevi no `FORMATO.md` que «renomear ou copiar outro `.ndx` para o mesmo lugar
**muda o CRC**, e o atestado deixa de valer sozinho». Renomear **não** muda o
CRC: é o mesmo inode. O que muda é o **caminho** — e o caminho era a chave. Eu
tinha pensado no arquivo que CHEGA a um caminho atestado (e o CRC resolve) e não
no arquivo atestado que SAI do caminho dele.

E sobre o embutido: tratei o `phx_sincronizar` como a porta da durabilidade e
não vi que o **contrato** do fechar — «o próximo processo abre» — era o que o
`EMBUTIDO.md` ensinava. Documentar no MANUAL que «quem embute chama
sincronizar» é guarda nova imposta a cliente antigo, o que a pétrea proíbe.

## O que a medição disse

- B1, com o atestado indo junto por um lugar só (`ndx::levar_atestado`):
  renomear, duplicar e colar de tabela recém-escrita reabrem sem recusa no
  mesmo processo, e o processo novo continua mandando reconstruir o destino.
  Com o motor reposto sem levar (e, noutra rodada, só o chamado do renomear
  tirado), o teste cai.
- B2, com o `phx_tabela_fechar` sincronizando **só** quando a marca se sustenta
  só neste processo: o processo seguinte — de verdade, o binário de teste
  reexecutado — abre, acha pela chave e grava. Com o fechar de antes, cai.
  Tabela só lida ou já sincronizada fecha sem `fsync`.

## A regra

Estado do processo chaveado por caminho tem de **acompanhar** toda operação que
move ou copia o arquivo; e quem muda o que o fechar deixa no disco confere o
contrato de **quem sai do processo** — a ABI, a linha de comando —, e não só o
de quem fica.

## Como está guardado hoje

- `bancada/guardas/catalogo.py`: `atestado-fica-no-caminho-velho`,
  `renomear-esquece-o-atestado`, `fechar-do-embutido-nao-sincroniza`, as três
  provadas vermelhas à mão em 24/09/2026;
- testes: `renomear_duplicar_e_colar_tabela_recem_escrita_abrem_sem_recusa`
  (store) e `testes::fechar_sem_sincronizar_e_o_proximo_processo_abre` (FFI,
  dois processos);
- **buraco:** a cópia continua sem `fsync` — um processo novo manda reconstruir
  o destino, como mandaria a origem. E outros estados do processo chaveados por
  caminho (a família do `Volumes`, a recusa do `sincronia`) não foram revistos
  contra renomear/colar nesta rodada.
