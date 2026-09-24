# A FK conferida antes do preenchimento: o irmão com defeito virou especificação

**Estado:** PENDENTE

## 1. O que aconteceu

O papel C mediu no N1 da segunda revisão do 448 (`docs/propostas/parecer-dba-448-2a-2026-09-24.md`) um pedido com `cod_cliente` pelo DEFAULT 7, sem cliente 7, gravado fora e dentro da transação. É o pedido 514.

A causa estava em `crates/phxsql-store/src/table.rs`: a `conferir_as_maes` rodava com a linha **crua**, antes de `completar`, `numerar` e `aplicar_regras`. Nulo satisfaz FK (`MATCH SIMPLE`), então o nulo passava e o DEFAULT punha o 7 depois. Quatro caminhos tinham a mesma ordem: o `inserir`, o `atualizar` e os dois braços da `pre_conferir` do 448.

O defeito já tinha sido **visto duas vezes**, e nas duas o conserto contornou em vez de mover a conferência:

- a condição 3 de `coluna_do_uuid_gerado` tem a frase «`conferir_fks_com` roda ANTES desta funcao e deixa o nulo passar». Por isso ela não gera v7 em coluna de FK. A conferência ficou onde estava;
- o achado A3 do 448 passou a unicidade e o plano para a linha **prevista**. O doc da `pre_conferir` escreveu então: «A chave estrangeira confere a linha CRUA, **como o `inserir` e o `atualizar` conferem**».

A segunda é a que ensina. O irmão foi usado como **especificação**, e a especificação era o defeito. Espelhar o irmão é a regra certa para não divergir. Aqui ela propagou a ordem errada para o caminho novo, e ainda deixou o motivo escrito como se fosse desenho.

## 2. O que eu concluí primeiro, e estava errado

- **Que mover a chamada não mudava mais nada.** Mudou: agora a conferência vem depois da `Sequence`, e a linha recusada pela FK passou a gastar um número. Medido: numa tabela com `Sequence`, depois de uma recusa pela FK o id seguinte era `1` antes do conserto e passou a `2`. O CHECK já gastava (`3` → `4`), e o PostgreSQL, o MySQL e o MariaDB também gastam na recusa. Não há como conferir antes, porque o DEFAULT e a calculada podem usar o número gerado.
- **Que a unicidade tinha o mesmo defeito em todos os caminhos.** Não tinha. O `inserir`, o `atualizar` e a `pre_conferir` já conferiam a linha final. Só a unicidade da **instrução** (`empilhar`, no servidor) olha a linha crua. Medido: com o 7 no disco numa coluna única com DEFAULT 7, a instrução empilha e o COMMIT recusa com «nada desta transacao foi gravado». É a mesma ordem, mas o dado fica salvo: o que muda é onde a recusa acontece.
- **Que a mudança não mexia no catálogo de guardas.** Mexia: a `replica-julga-fk` amarrava o corpo antigo da `conferir_as_maes`. A catraca `trecho-vivo.py` acusou `TETO_TRECHO_MORTO: 1` na primeira corrida.

## 3. O que a medição disse

Cada teste novo deu vermelho com o defeito reposto, caminho por caminho:

- **`inserir`:** `veio Ok(1)` (a filha órfã ganhou o rowid 1).
- **`inserir_lote`:** 3 gravadas de 3.
- **`atualizar` da calculada:** `veio Ok(())`.
- **`Sequence` que é FK:** `veio Ok(2)`.
- **`pre_conferir`, braço da inserção:** `veio Ok([])`.
- **`pre_conferir`, braço da alteração:** `veio Ok([])`, medido com só esse braço reposto.
- **Servidor, fora de transação:** `{"rowid":1,"registros":1}`.
- **Servidor, dentro de transação:** `"transaction_state":"COMMITTED","gravadas":1`, com (linhas, slots) de `(0, 0)` para `(1, 1)`.

Com o conserto, os oito ficam verdes e o controle positivo (DEFAULT com mãe grava o 7) passa nos dois sentidos.

## 4. A regra

Quando um conserto novo copiar a ordem de um irmão, confira primeiro se a ordem do irmão está certa. Comentário que diz «como o irmão faz» é a hipótese que se mede, não a justificativa.

## 5. Como está guardado hoje

- `crates/phxsql-store/tests/fk-na-linha-final.rs::o_default_sem_mae_e_recusado_no_inserir` e os irmãos do arquivo (seis testes, cinco que caem e o controle).
- `crates/phxsql-server/src/servidor.rs::o_default_sem_mae_e_recusado_fora_e_dentro_da_transacao` e `a_calculada_sem_mae_e_recusada_fora_e_dentro_da_transacao`, no módulo `testes_transacoes::pedido_514`.
- As guardas `fk-antes-do-default` e `fk-antes-do-default-pelo-servidor` do catálogo, as duas provadas pelas vagas do `cargo-da-frente.sh` (5 caem e 1 segue; 2 caem e 2 seguem).
- A ordem mora numa função só, a `Table::linha_final`: a `conferir_as_maes` só é chamada por ela.
- **O irmão que eu não vi, e o DBA mediu (P1):** a cascata do `ao_alterar` escreve na filha por um terceiro caminho, o elo. O `conferir_a_arvore` montava a filha crua. Com a chave calculada em cascata, a mãe ficava gravada em 8 e a filha em 9. Com o CHECK da filha, a mãe ficava em 200 e a filha em 9. Fechou na declaração, onde PG, MySQL e MariaDB recusam o mesmo par (`chave_sobre_calculada_recusa_cascata_e_anular_na_declaracao`), e na tabela que já existe, pela `linha_do_elo` (`a_cascata_sobre_calculada_recusa_antes_de_gravar_a_mae`, `o_check_da_filha_recusa_a_cascata_antes_de_gravar_a_mae`). A lição reforça a regra da §4: eu procurei irmãos entre quem chama a `conferir_as_maes`, e o elo não a chama. Irmão é quem **escreve a mesma linha**, não só quem chama a mesma função.
- **Os buracos que ficam:**
  - a unicidade da instrução no `empilhar` continua olhando a linha crua. O dado fica salvo porque o COMMIT recusa, mas a transação inteira termina em vez de só a instrução. É a mesma família do 459, que está ⏸;
  - o `se_existir` do upsert decide pela chave crua. Com a coluna da chave pelo DEFAULT, ele cai no `inserir`, que recusa por DUPLICADO. Não foi consertado nem medido a fundo.
