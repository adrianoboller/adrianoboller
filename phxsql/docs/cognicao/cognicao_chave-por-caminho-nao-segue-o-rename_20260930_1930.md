# Chave por caminho não segue o `rename`: mudar o nome nas sujas não bastava

**Estado:** FRUTÍFERO
**Evidência:** `crates/phxsql-server/src/servidor.rs::tabela_excluida_ou_renomeada_na_janela_nao_segura_as_marcas` falha com `mudar_pendentes_de_nome` desligado (`familias_devendo_em` = 12) e passa com ele ligado; guarda `renomear-deixa-o-registro-no-nome-velho` em `bancada/guardas/catalogo.py`, provada pelo `provar-guardas.py --so`. Pedido 536, 30/09/2026.

## O que aconteceu

O pedido 536 tratava de uma tabela escrita na janela e depois excluída ou
renomeada. Ela continuava nas sujas pelo nome velho e segurava todas as marcas
de COMMIT.

## O que eu concluí primeiro, e estava errado

1. **«Sincroniza na própria operação e tira a chave.»** Fechava o defeito, mas
   pôs `fsync` sob a trava global em mais duas seções. A catraca
   `alcancam-fsync-2` subiu de 23 para 25, e catraca não sobe.
2. **«Então só muda a chave das sujas de nome, e o fecho sincroniza depois.»**
   As sujas ficaram vazias e a marca saiu, então parecia pronto. O instrumento
   do teste (`familias_devendo_em`) acusou 12 famílias devendo. O registro de
   escritas pendentes do `Volumes` também tem o **caminho** como chave. O
   `rename` muda o caminho sem mudar o inode, e o registro ficou no nome velho.

## O que a medição disse

| desenho | catraca de `fsync` | sujas depois do fecho | famílias devendo |
|---|---|---|---|
| sem conserto | 23 | `b/a`, `b/b` | — |
| `fsync` na operação | **25** (reprovado) | vazias | 0 |
| só a chave das sujas | 23 | vazias | **12** |
| chave das sujas + registro segue o arquivo | 23 | vazias | 0 |

## A regra

Todo estado em memória cuja chave é um caminho tem de **seguir o `rename`**, ou
sair junto com o arquivo quando ele é apagado. Nesta casa já são três:

- o atestado do `.ndx` (522, `levar_atestado`);
- o registro de pendentes (536, `mudar_pendentes_de_nome`);
- a chave das sujas (536, `renomear_nas_sujas`).

Quando um estado novo for chaveado por caminho, procure o `renomear_tabela`
e o `excluir_tabela`, e confira se ele também está lá.

E a catraca ajudou a achar a raiz: o conserto barato demais era barato porque
deixava de ver o segundo registro.
