# A conferência da réplica fiel não mora na função que o bidirecional também chama

**Estado:** INFRUTÍFERO

**Causa:** a conferência da linhagem entrou em `garantir_tabela_da_replica` pelo nome da função, e o `abrir_para_bidi` também a chama — 6 testes de soquete caíram (5 do bidirecional, onde a linhagem diverge legitimamente).

**Prevenção:** antes de pôr uma conferência numa função, listar quem a chama; a da réplica fiel mora em `recusa_da_linhagem`, chamada só do `abrir_para_replicar`, e sai pelo `romper_continuidade`.

## O que aconteceu

Pedido 601 (01/10/2026), a linhagem da tabela no `PSCH` v11. A réplica fiel tem
de recusar a tabela daqui que é de outra história que a do source. Pus a
conferência em `garantir_tabela_da_replica` (`phxsql-server/src/servidor.rs`),
o lugar onde a tabela local é aberta contra o esquema que o `posicao` mandou.

A suíte derrubou **seis** testes de soquete: cinco do bidirecional
(`a_posicao_do_bidirecional_vai_ao_disco_depois_do_dado`,
`sem_colisao_o_laco_replica_como_sempre_e_nada_e_contado` e três irmãos) e o
`tabela_apagada_e_recriada_no_source_e_acusada_e_nao_aplicada`.

## O que eu concluí primeiro, e estava errado

Que `garantir_tabela_da_replica` era «o caminho da réplica fiel», pelo nome.
Não é: o `abrir_para_bidi` chama a mesma função, e no bidirecional os caixas de
um central **divergem na linhagem legitimamente** (cada um criou a sua) — o
parecer do DBA diz isso com todas as letras. A conferência ali parava o
bidirecional inteiro.

E o segundo erro, menor: a recusa saía como `Err` e caía no `ultimo_erro`, e
não no canal das recusas por tabela (`romper_continuidade`), que é por onde a
tela e o teste da tabela recriada a procuram.

## O que a medição disse

6 testes vermelhos com a conferência no `garantir`; 0 com ela em
`recusa_da_linhagem`, chamada só de `abrir_para_replicar` e reportada por
`romper_continuidade` — o mesmo canal da «tabela apagada e recriada», que é um
dos dois casos que a linhagem pega.

## A regra

Antes de pôr uma conferência numa função, liste **quem chama** a função: o
nome diz para que ela nasceu, e a lista de chamadores diz onde a conferência
vai valer.

## Como está guardado hoje

- `recusa_da_linhagem` mora fora do `garantir`, com o motivo escrito no
  comentário (o bidi também chama o `garantir`).
- Guarda `replica-fiel-sem-conferir-a-linhagem` (`bancada/guardas/catalogo.py`).
- **Buraco que fica:** nenhuma guarda acusa se alguém mover a conferência de
  volta para o `garantir` — quem acusa são os testes do bidirecional, por
  efeito colateral.
