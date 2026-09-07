# A guarda protegia a memória e o `;` do procedimento quase virou injeção

- **Quando:** 2026-09-07, 17:45
- **Onde:** `crates/phxsql-server/src/servidor.rs` (laço da conexão, portão
  2b-bis, contador de injeção), `crates/phxsql-sql/src/sintaxe.rs`
  (`comando_empilhado`), `bancada/seguranca/porta.py` (caso 4b),
  `bancada/seguranca/injecao.py` (casos 5b e 5c)
- **Custo:** dois erros meus, os dois apanhados antes de qualquer commit — um
  pela leitura do fonte da bancada vizinha, outro pela própria medição

## O que aconteceu

Três pedidos de segurança na mesma rodada (214, 215, 216), e os três com o
mesmo cheiro: uma guarda que **existe e funciona** ao lado de uma guarda que
**não chegou a nascer**. O teto de 128 MiB do fio protegia a memória (RSS
6.040 → 8.164 kB, processo de pé) e não deixava rastro nenhum; a lista
`replicas_autorizadas` tranca quem está fora dela e nasce vazia; o analisador
SQL recusa toda injeção clássica e ninguém contava as tentativas.

## O que eu concluí primeiro, e estava errado

**Duas vezes**, e as duas valem mais que o conserto.

**Primeira.** Escrevi o classificador de injeção como uma varredura do fluxo de
símbolos: «achou um `;` com símbolo depois, é comando empilhado». Isso já
respeitava a lei de analisar em vez de recortar — o `'; DROP TABLE'` dentro de
aspas é um símbolo `Texto` e não seria acusado, e eu tinha teste para isso.
Parecia certo, e passou nos meus sete casos.

Só que, ao extrair a lista de SQL legítimo do fonte de
`bancada/sql-exemplos/exercitar.py` para medir o falso positivo, apareceu o
que eu não tinha imaginado: `CREATE PROCEDURE somar_ate(...) BEGIN DECLARE i
INT DEFAULT 0; SET i = i + 1; ... END`. O **corpo** de um procedimento e o de
um gatilho são cheios de ponto-e-vírgula legítimo. Um `CREATE PROCEDURE` que
falhasse por qualquer outro motivo — nome repetido, permissão, erro no corpo —
seria classificado como injeção, e o DBA que escreve procedimento a mão cairia
na blacklist.

O conserto não foi acrescentar uma exceção: foi trocar o crivo pelo **caminho
que o motor já percorre**. `comando_empilhado` roda o analisador desta
gramática e só acusa quando um comando **completo** foi lido e sobrou símbolo
depois do `;` — que é, letra por letra, a recusa «sobrou X depois do fim do
comando» que o pedido 215 mandava contar. `CREATE PROCEDURE` não é desta
gramática, o analisador não o lê, e ele sai `false` sem exceção nenhuma
escrita.

**Segunda.** Ia responder ao cliente a linha acima do teto escrevendo a resposta
e fechando a conexão em cima. Funciona no papel e não funciona no fio: fechar
um soquete com dado por ler no buffer de recepção manda RST, e o RST descarta a
resposta que o cliente ainda não leu. Eu teria «respondido» no código e
continuado invisível no fio — o defeito que o conserto existe para matar,
cometido dentro do conserto. Hoje a linha é **drenada até a quebra antes** de a
resposta sair.

## O que a medição disse

Antes de decidir o (c) do pedido 214 — «`aplicar` passa a respeitar
`somente_leitura`» —, a pergunta era se a réplica legítima usa a op. Source e
réplica de pé (papel `replica`, `somente_leitura` ligado), 200 linhas:

| lado | acessos | op `aplicar` |
|---|---:|---:|
| source | 216 (`inserir` 200, `posicao` 4, `replicar` 1, …) | **0** |
| réplica | 3 (`login` 1, `varrer` 2) | **0** |

200 de 200 linhas alcançadas. O laço da réplica puxa e aplica **por dentro**,
com `Table::aplicar_evento`, e nunca pelo protocolo — então o crivo do portão
2b-bis pôde ser o **papel**, e não a existência de origens nem a lista de
réplicas.

O falso positivo do 215, com o interruptor ligado e tolerância **1** (um único
falso positivo bloqueia na hora): **65 comandos** legítimos lidos do fonte de
`bancada/sql-exemplos/exercitar.py`, **32 recusados** pelo motor, **zero
bloqueios**.

E a medição da própria bancada estava errada, o que é a terceira lição de
graça: o caso 4b lia o `antes` do `acessos.log` **depois** de mandar a linha
gigante. A linha nova já estava dentro do `antes`, e a conta dava zero **com
rastro e sem**. Uma medição que dá o mesmo número nos dois mundos não mede
nada — e ela publicou «zero linhas novas» tanto no defeito quanto no conserto.

## A regra

**Quando a guarda nova classifica texto do usuário, a lista do que NÃO pode ser
acusado sai do código de outra frente, nunca da minha cabeça.** Eu não teria
imaginado o corpo do `CREATE PROCEDURE`; a lista de SQL legítimo do
`exercitar.py` imaginou por mim. E o corolário de método: **classificar pelo
caminho que o motor já percorre é sempre melhor que classificar pela forma do
texto** — não porque é mais elegante, mas porque a exceção que eu esqueceria de
escrever o motor já trata.

## Como está guardado hoje

`docs/SEGURANCA.md` §3 (as duas leves que entram pedidas, com os dois números e
o crivo do falso positivo) e `docs/REPLICACAO.md` §6 e §7 (o portão 2b-bis com a
medição do `aplicar`, e o aviso da lista vazia). Em teste:
`comando_empilhado_nao_acusa_o_legitimo` trava os oito casos de falso positivo,
e `sql_legitimo_recusado_nao_conta_como_injecao` trava o mesmo pelo `despachar`.
Na bancada: casos 5b e 5c de `injecao.py`, 4b e 4b-ii de `porta.py`.

O que fica **não medido**: se há outra família de recusa de SQL que valha contar
como injeção além do comando empilhado. A escolha de contar só essa é do pedido
215, e nada foi medido a favor ou contra ampliá-la.
