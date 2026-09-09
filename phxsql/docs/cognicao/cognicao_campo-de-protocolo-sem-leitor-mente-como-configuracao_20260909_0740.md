# Campo de protocolo sem leitor mente como configuração sem leitor

## 1. O que aconteceu

Achado A2 da revisão do motor (`p02_sql_on_conflict_set.py`): `INSERT … ON
CONFLICT (id) DO UPDATE SET nome = 'B'` gravava `nome='dois', cidade='Itajai'`
(o `VALUES`) em vez de `nome='B', cidade='Blumenau'` (a linha lida com o SET).
O tradutor (`dml.rs:527`) punha o SET no campo `"atualizar"` do pedido, a nota
da tradução **dizia** isso, o teste do tradutor conferia isso — e
`op_inserir` nunca lia `campo("atualizar")`. Uma rodada inteira sem erro
nenhum, porque nenhum foi emitido.

## 2. O que eu concluí primeiro, e estava errado

Que a mescla se fazia por um vaivém JSON — `linha_para_json(velha)`, sobrepor
o SET, `json_para_linha` — porque era o caminho que o `UPDATE` por chave já
percorria pelo protocolo. Funcionaria, e pagaria a conversão de **toda**
coluna da tabela para trocar duas. `json_para_valor` já converte uma coluna
pelo tipo do esquema; a mescla por valor (`upsert::mesclar`) é menor e não
passa por texto.

E que o conserto era só no `op_inserir`. O `empilhar` da transação decide o
upsert por conta própria (chama `t.buscar` e monta a `Escrita`), e a sincronia
do DbLink é o terceiro chamador de `upsert::aplicar`: a assinatura nova
obrigou a olhar os três, que é o que a lei do irmão pede — e o primeiro
desenho, com o merge dentro do `op_inserir`, teria deixado a transação
gravando o `VALUES`.

## 3. O que a medição disse

`p02`: 0 de 3 antes, 3 de 3 depois — `ON CONFLICT`, `ON DUPLICATE KEY` e o
campo `atualizar` direto no protocolo. A prova de unidade mede as **três**
colunas (a do SET mudou, a que só o `VALUES` trazia não mudou, a que ninguém
citou continua) e a linha nova (entra o `VALUES`, não o SET), nos dois
caminhos: `op_inserir` e transação.

## 4. A regra

**Campo que o pedido pode trazer e o servidor não lê é recusado, não
ignorado** — é o alcance de «configuração que não é lida mente» sobre o
protocolo. `atualizar` fora de `se_existir: "atualizar"` recusa nomeando.

## 5. Como está guardado hoje

`upsert::atualizar_do_pedido` (confere) e `upsert::mesclar` (mescla), chamados
pelo `op_inserir` e pelo `empilhar`; guarda `set-do-on-conflict-ignorado`;
`docs/SQL.md` §7. O buraco que ficou, nomeado: o gatilho BEFORE do upsert vê
a linha do `VALUES`, não a mesclada — o AFTER lê a gravada de volta e está
certo.
