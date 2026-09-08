# Cognição: o `UPDATE` pelo SQL não é tradução direta — o protocolo grava a linha inteira

**Descoberta:** 08/09/2026, ~14:28 UTC, no reconhecimento da frente G (INSERT,
UPDATE e DELETE por chave na camada SQL), antes de escrever uma linha de código.

## 1. O que aconteceu

A tabela da §1 do `docs/SQL.md` diz `UPDATE … WHERE rowid = ?` → `atualizar`, e
`DELETE` → `excluir`. Lendo o `op_atualizar` e o `json_para_linha`
(`crates/phxsql-server/src/valores.rs`), apareceu a frase que muda o desenho:
*«Colunas ausentes no objeto entram como NULL.»* O `atualizar` recebe a linha
**inteira**. Um `UPDATE t SET nome = 'x' WHERE id = 5` traduzido direto mandaria
só `nome` — e zeraria `cidade`, `telefone` e tudo o mais, **sem erro nenhum**.

## 2. O que eu concluí primeiro, e estava errado

Que o passo 2 do roteiro era «tradução de coisa medida», como o próprio
`SQL.md` §4 promete: `INSERT`→`inserir`, `UPDATE`→`atualizar`,
`DELETE`→`excluir`, um verbo para uma operação, igual ao `SELECT`→`buscar`. O
plano de tradução sairia «certo» — `{"op":"atualizar","rowid":…,"valores":{"nome":"x"}}`
— e um teste de tradução passaria. O defeito só apareceria em produção, na
primeira ficha com mais de uma coluna, e apareceria como **dado sumido**, não
como erro.

Errei por casar pela **forma do verbo** (UPDATE ≈ atualizar) sem olhar o
**contrato do pedido** (o que o `atualizar` faz com o que não vem). É o mesmo
erro da cognição do contador do `.log`: forma igual não é garantia igual.

## 3. O que a leitura e a prova disseram

- `json_para_linha`: coluna ausente → `Value::Null` (e `softdeleted` → `false`,
  o que sozinho **ressuscitaria** uma linha excluída — o `op_atualizar` já se
  defende disso lendo a linha atual quando a marca não vem).
- O `buscar` devolve `rowid` e a linha, mas **não** a `versao`; o `ler` com
  `com_versao` devolve as duas.
- `inserir`, `atualizar` e `excluir` empilham numa transação aberta
  (`OPS_EMPILHAVEIS`, `servidor.rs`); `inserir_lote` **não** — um `VALUES` com
  várias linhas traduzido para ele gravaria por fora de um `BEGIN…ROLLBACK`.

Daí o desenho: `UPDATE`/`DELETE` por chave são **três passos** (`buscar` →
`ler` → `atualizar`/`excluir` com a linha **mesclada** e a `versao` lida), e
uma linha por `INSERT`. A prova real: `update_por_chave_muda_so_a_coluna_do_set`
falha na coluna `cidade` com a mescla tirada, e
`os_pedidos_dos_passos_levam_a_linha_mesclada_e_a_versao` falha com a linha da
`versao` tirada.

E um segundo erro menor, da mesma família: o primeiro teste do `DELETE`
conferia que o `buscar` **não** achava mais a linha excluída. Falhou — o índice
continua a achá-la, e tem de continuar, porque o `restaurar` precisa dela.
«Some da lista» é o `varrer`; eu tinha medido o observável errado.

## 4. A regra

**Antes de traduzir um verbo para uma operação do protocolo, leia o que a
operação faz com o que NÃO vem no pedido.** Tradução direta é a que preserva
o contrato de quem recebe, não a que casa o nome do verbo — e um plano que
«sai certo» num teste de tradução ainda pode apagar dado no motor.

## 5. Como está guardado hoje

Em `crates/phxsql-sql/src/dml.rs` (o módulo diz no cabeçalho por que não é
tradução direta), em `executar_dml` e nas funções puras `pedido_de_ler`,
`pedido_de_atualizar` e `pedido_de_excluir` do `servidor.rs`, e no `docs/SQL.md`
§6. O que ficou **sem medir**: `UPDATE`/`DELETE` de uma linha nascida na mesma
transação aberta — a linha empilhada ainda não está no índice que o passo 1
desce; o desfecho esperado é `afetadas: 0`, e está escrito como não medido
até ter teste.
