# Cognição: o gatilho do upsert é o do ramo que ele virou — e o «vê a linha errada» era «não roda»

**Descoberta:** 16/09/2026, 11:18 — frente U (papel B com F), pedido 245, o gap
que a revisão G4-MOTOR deixou nomeado.

## 1. O que aconteceu

O `PENDENCIAS.md` dizia, no fim do pedido 245: *«no upsert com `atualizar`, o
gatilho BEFORE vê a linha do VALUES, não a mesclada»*. Montei o caso menor —
`clientes(id, nome, cidade)`, linha `{1, Ana, Blumenau}`, um `BEFORE UPDATE`
que lê a **cidade** (coluna que o `VALUES` do upsert não traz) e deixa o que
viu no `nome` — e rodei o upsert `{"linha":{"id":1,"nome":"x"},
"se_existir":"atualizar","atualizar":{"nome":"B"}}`.

Cinco testes em `crates/phxsql-server/src/servidor.rs` (`testes_gatilhos`,
nomes `*upsert*`), todos vermelhos no código de 16/09 antes do conserto:

| faceta | caminho | o que saiu |
|---|---|---|
| `BEFORE UPDATE` com `SET` | `op_inserir` | `nome = "B"` — o gatilho **não rodou** |
| `BEFORE UPDATE` sem `SET` (linha inteira) | `op_inserir` | `nome = "x"`, `cidade = "JOINVILLE"` — o `BEFORE INSERT` rodou, o `UPDATE` não |
| `AFTER` do ramo que atualizou | `op_inserir` | auditoria `["entrou Ana", "entrou Bia"]` — `AFTER INSERT` numa atualização |
| `AFTER` do ramo ignorado | `op_inserir` | auditoria `["entrou Ana", "entrou Ana"]` — `AFTER INSERT` com a linha que **já estava lá** como `NEW`, por um pedido que não gravou byte nenhum |
| `BEFORE UPDATE` na transação | `empilhar` | `nome = "B"` — não rodou na instrução |

## 2. O que eu concluí primeiro, e estava errado

Li a frase da pendência ao pé da letra: *o BEFORE vê a linha do VALUES*. Então
o defeito seria de **alimentação** — existiria um `BEFORE` rodando no ramo de
atualização e alguém lhe passava a linha crua em vez da mesclada; o conserto
seria trocar o argumento. Fui procurar «onde o BEFORE recebe a linha errada».

Não existia esse lugar. O que rodava no ramo de atualização era o `BEFORE
INSERT`, sobre a linha do `VALUES` — e **isso está certo**: PostgreSQL,
MariaDB e MySQL disparam o `BEFORE INSERT` sobre a linha proposta antes de
descobrir a chave repetida. Consertar «alimentando a mesclada» ao `BEFORE
INSERT` teria quebrado o comportamento certo para produzir o errado com outra
cara. O defeito era de **ausência**: o par de `UPDATE` nunca era consultado, e
o `BEFORE UPDATE` — o único que deve ver a mesclada — não tinha onde rodar.

E a segunda conclusão errada, mais curta: assumi que o `AFTER` estava certo
porque o `commit` da transação escolhe o evento pela ação empilhada
(`Acao::Atualizar → Evento::Atualizar`). O `op_inserir` não escolhe: lia o par
de `INSERT` no começo e o disparava nos três ramos. Só listar os ramos
(inseriu, atualizou, ignorou) e perguntar «que AFTER roda aqui?» mostrou o
terceiro, o do ignorado — que nenhuma frase da pendência mencionava.

## 3. O que a medição disse

A tabela da seção 1 é a medição, cinco corridas vermelhas. Com o conserto:
`70 passed; 0 failed` em `testes_gatilhos:: testes_upsert:: testes_transacoes::`.
A guarda `upsert-gatilho-do-ramo` repõe os três pontos e derruba os cinco.

E uma medição fora do escopo, que ficou nomeada em vez de consertada: um
`AFTER UPDATE` que grava auditoria, disparado **no `COMMIT`**, grava pela
sessão que ainda está em `Confirmando` — o `inserir` derivado cai no
`empilhar` de uma transação descartada logo depois. Saída medida:
`COMMIT => {"transaction_state":"COMMITTED","gravadas":1}` e
`AUDITORIA => []`, sem `gatilhos_avisos`. Não é o mesmo defeito (é o despacho
dos AFTER no commit, para qualquer evento); vai ao integrador para virar
pendência.

## 4. A regra

**Quando uma operação vira outra no meio do caminho, os observadores são os
da operação que ela VIROU — e «vê a linha errada» pode ser «não roda»: antes
de trocar o argumento, prove que a chamada existe.**

## 5. Como está guardado hoje

- O gancho `upsert::AntesDeAtualizar` em `crates/phxsql-server/src/upsert.rs`:
  roda depois da mescla e antes da gravação, único instante em que a linha
  final existe sem estar no disco. O `op_inserir` o liga quando há gatilho de
  `UPDATE` na tabela; o DbLink passa `None` com o motivo escrito (a sincronia
  não dispara gatilho nenhum — copia o que o outro lado já julgou).
- `op_inserir` escolhe o `AFTER` pelo ramo (`atualizada` → `UPDATE` com
  `OLD`; `ignorada` → nenhum; senão `INSERT`); `empilhar` roda o `BEFORE
  UPDATE` na instrução, entre a mescla e `julgar_regras_de_escrita`, a mesma
  ordem do ramo `Acao::Atualizar`.
- Cinco testes em `testes_gatilhos`; guarda `upsert-gatilho-do-ramo` no fim
  de `bancada/guardas/catalogo.py`; tabela por ramo em `docs/TRIGGERS.md` §1
  e o aviso em `docs/SQL.md` §7.
- **Onde o buraco ficou:** o AFTER disparado no commit que grava pela sessão
  em `Confirmando` perde a escrita em silêncio (seção 3). Medido, não
  consertado, não é desta frente.
