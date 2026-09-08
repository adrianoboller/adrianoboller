# O terceiro irmão, e o oráculo escondido numa comparação de igualdade

Descoberto em 08/09/2026, escrevendo o direito por coluna (frente C19).

## 1. O que aconteceu

O contrato mandava chamar o direito por coluna de **dois** lugares —
`despachar` e `executar_derivado`. A pétrea define irmão como *quem chama as
mesmas funções na mesma ordem*, e não quem tem nome parecido. Aplicando a
definição em vez do número, a varredura de `crates/phxsql-server/src/servidor.rs`
achou **três** chamadores de `portoes_do_pedido` seguidos de `executar`:

| linha (antes) | quem | o que passa por ele |
|---|---|---|
| 7502 | `despachar` | a rede, e com ela o MCP, o REST e a tela (todos entram pelo `ExecutorLocal`) |
| 5370 | `executar_derivado` | a op `sql` e cada passo dela |
| 5380 | `executar_job` | o agendador, sob o usuário do job |

O terceiro não aparece em nenhum teste de portão: um job de `exportar` na
tabela restrita não chega nem por soquete nem por SQL.

E, escrevendo a regra de escrita, apareceu um segundo achado que não estava no
contrato: a exceção que evita quebrar quem lê a coluna e não a altera é um
**oráculo** quando o usuário não pode ler.

## 2. O que eu concluí primeiro, e estava errado

**Primeiro erro.** Concluí que dois lugares bastavam porque «o job também passa
pelo `executar`, então basta pôr a guarda lá dentro». Errado por duas razões, e
a segunda é a que decide: o `executar` é chamado **também** pelo `op_sql` para
os comandos de diretiva e de transação, e pelos próprios testes — pôr a peneira
ali a faria rodar em lugares que não passaram pelo portão, e a reposição do
valor gravado chamaria `executar("ler")` de dentro do `executar`, recursão que
só a trava reentrante acusaria. O lugar certo é o irmão, não o tronco.

**Segundo erro.** Escrevi a regra de escrita como o contrato a descreve — *valor
não nulo na coluna negada recusa* — e a achei completa. Ela quebra o par
`{"ler": true, "alterar": false}`: o cliente lê a linha inteira e a devolve
inteira, então **toda** gravação pela tela passaria a ser recusada. Consertei
aceitando o valor igual ao gravado, e só então vi o buraco que eu tinha acabado
de abrir: para quem **não** lê a coluna, «aceito quando bate» responde
`ACESSO_NEGADO` para o palpite errado e grava para o certo — vinte tentativas e
o salário aparece sem nunca ter sido devolvido. A comparação passou a acontecer
**só** quando o direito de `ler` daquela coluna existe.

O padrão dos dois erros é o mesmo: eu resolvi o caso que estava na minha frente
e não perguntei quem mais chega por outro caminho.

## 3. O que a medição disse

A prova real foi por sabotagem, uma de cada vez, na bateria
`testes_direito_por_coluna` (13 testes):

| sabotagem | o que caiu |
|---|---|
| tirar a chamada do `executar_derivado` | **1** — só `o_sql_herda_o_direito_por_coluna`, e a mensagem mostrou `"salario":5000` dentro do `SELECT` |
| tirar a chamada do `despachar` | **8** — e `o_sql_herda…` **passou** |
| não repor o valor gravado (`repor_coluna`) | **2** — «o salário foi zerado por quem nem podia vê-lo» e a versão pelo SQL |
| a peneira devolver a resposta crua | **4**, dois deles no teste de unidade da própria peneira |
| a recusa das operações que não peneiram virar no-op | **2** — `exportar` e `juntar` |

As duas primeiras linhas são o par que importa: os dois irmãos são
**independentes**, e nenhum dos dois cobre o outro.

E a lista das operações também foi medida em vez de suposta. A varredura do
catálogo por expressão regular achou 129 operações; o teste
`a_lista_e_o_catalogo_sao_a_mesma_lista` acusou **130**, e as quatro que faltavam
eram apelidos: `sequences`, e os três de `SelectMemory` (`selectmemory`,
`selecionar_memoria`) — que a minha regex perdeu porque ela só casava minúsculas.
O `SelectMemory` devolve **as mesmas linhas do `varrer`**: sem ele classificado,
bastava um `memoria_carregar` para a coluna negada sair inteira.

Suíte do servidor: **919 antes, 937 depois**, zero falhas, `clippy` com zero
avisos.

## 4. A regra

**Conte os irmãos pela definição, não pelo número que o pedido trouxe — e,
quando uma exceção aceitar «o valor que bate», pergunte se quem manda o palpite
poderia ler a resposta.**

## 5. Como está guardado hoje

- Os três chamadores estão nomeados no comentário de
  `Servidor::aplicar_direito_por_coluna`, com o que passa por cada um.
- O oráculo está travado no código (`direito.ler &&` antes da comparação) e no
  teste `quem_le_a_coluna_e_nao_a_altera_continua_gravando`, que prova os dois
  lados — o igual passa, o diferente recusa.
- A lista das 130 operações não se digita: sai de `CLASSES`, em
  `crates/phxsql-server/src/direito_coluna.rs`, e o gerador
  `docs/geradores/direito-por-coluna.py` escreve a §15 do `docs/SEGURANCA.md`
  — inclusive o **título**, que carrega três números.
- **Onde o buraco ficou:** não há guarda automática contra um quarto irmão. Se
  alguém escrever um caminho novo que chame `portoes_do_pedido` e depois
  `executar`, nada acusa — a mesma forma de risco que a pétrea já registra para
  a conferência própria do `juntar`. Um conferidor que contasse os pares
  `portoes_do_pedido` → `executar` no fonte é a próxima hipótese, e ela precisa
  ser medida antes de virar catraca: com três ocorrências, um casador de texto
  tem mais chance de dar falso positivo do que de achar o quarto.
