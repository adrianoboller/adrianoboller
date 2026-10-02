# A porta SQL de uma operação que exige `administrar` é porta dos fundos se copiar o ramo das diretivas

**Estado:** PENDENTE

## 1. O que aconteceu

Pedido 268: `criptografar`/`descriptografar` ganharam uma fachada SQL
(`ALTER TABLE t ENCRYPT|DECRYPT`), que mora na gramática das diretivas
(`crates/phxsql-sql/src/diretiva.rs`) porque é ela a dona do
`ALTER TABLE <nome> <verbo>`. O ramo do `op_sql` que atende as diretivas chama
`self.executar(&c.op, …)` — e a op `sql` só exige `Atividade::Ler`
(`usuarios.rs`). Copiar o ramo como estava deixaria quem só lê cifrar uma
tabela inteira pelo SQL, enquanto o pedido `op` equivalente seria recusado.

## 2. O que eu concluí primeiro, e estava errado

Que bastava reaproveitar o ramo das diretivas: elas já são ops administrativas
(`diretiva_gravar`) chamadas pelo SQL. O que eu não tinha visto é que **elas
conferem `administrar` POR DENTRO** (`exigir_administrar_config`), e a minha
operação confere pelo portão geral do `despachar` — que o ramo do SQL nunca
atravessa. O comentário vizinho («as diretivas conferem por dentro») estava
escrito; eu li o ramo e não o comentário.

## 3. O que a medição disse

Com `executar` no lugar de `executar_derivado`, a guarda
`migracao-da-cifra-pelo-sql-sem-portao` reprova: `1/1 cairam`
(`quem_nao_administra_nao_migra_nem_pelo_sql`, em
`crates/phxsql-server/tests/migracao-da-cifra-pelo-soquete.rs`), com o usuário
`so_le` cifrando a tabela pelo SQL e a versão do `.reg` indo de 4 para 5. O teste
do `op` sozinho passa com o defeito reposto.

## 4. A regra

Operação que exige mais que `ler` e ganha fachada SQL entra pelo
`executar_derivado` (os mesmos portões do `despachar`), nunca pelo `executar`
do ramo das diretivas — e a prova é um usuário que só lê tentando pelo SQL.

## 5. Como está guardado hoje

Guarda `migracao-da-cifra-pelo-sql-sem-portao` (provada, `--so`) e o teste
acima. **Buraco:** o ramo das diretivas continua chamando `executar`; a próxima
operação administrativa que morar ali herda o mesmo risco, e nenhum
conferidor procura «fachada SQL sem portão» — é a lei de 03/09 («o portão é
UM; quando o portão passar a olhar um campo novo, procure quem não tem esse
campo») vista pelo SQL.
