# Status dos dez recursos — a fonte da página

Esta tabela é a **fonte** de `docs/dossie/status.html`, que **não se edita**:
`python3 docs/dossie/pagina-de-status.py` a gera daqui, e injeta ao lado os
números **medidos** — do `CAPABILITIES.json` e dos `resultados.json` das
bancadas, cada um com a data em que foi medido.

Ela carrega dois tipos de número, e a página os separa de propósito:

- **A nota (0–10) é avaliação**, não medida: saiu da leitura do código e dos
  testes de cada recurso em **08/09/2026**, com as fontes nomeadas na própria
  linha. Quem discordar de uma nota muda **esta** linha, com o motivo, e roda o
  gerador.
- **O painel do alto é medido**: testes, operações, linhas, a bancada de
  gestão, a replicação e o cluster. Nenhum desses se digita aqui.

O «tipo» diz o que a nota significa: `construído` (existe e está provado),
`parcial` (existe, com a metade que falta nomeada), `recusa medida` (não
existe **porque** foi medido que não compensa — não é buraco) e `promessa`
(a documentação prometeu mais do que o código entrega).

| # | recurso | nota | tipo | como está | o que falta |
|---|---|---|---|---|---|
| A | Paginação | 9 | construído | Endereço por conta (`rowid → volume`) em `paginacao.rs`, `.pag` gerado, partição A–Z de 37 baldes, cursor/keyset, `rownum` por bisseção, quatro modos de varredura. Testes em `phxsql-core`, em `phxsql-store/tests/paginacao.rs` e `alfanumerica.rs`, mais os de protocolo; os defeitos históricos estão corrigidos e documentados no `PENDENCIAS.md`. | Comando na CLI — só protocolo e tela (pedido 14). A varredura por índice ainda usa posição, não cursor: decisão documentada em `op_varrer`. |
| B | Replicação | 8 | construído | Diário `.log`, quatro modos, cluster com eleição e promoção automáticas. A bancada mede **1 master + 3 réplicas** com `iguais_no_fim` (retrato SHA-256 idêntico) e o cluster com **3 nós** — a minoria não se elege. Os números estão no painel acima, com a data. | Configuração só à mão no `config.json`, não pela tela. Bidirecional provado só no par. Pulso do cluster em claro. Nó a quente não entra. A busca do lote acontece **dentro da trava** — uma réplica cortada parou 30,7 s (`REPLICACAO.md` §13). Sem TLS real: a cifra é a Noise-like da casa. |
| C | Tabela na memória | 7 | construído | `TabelaMemoria` guarda a linha **decodificada** em RAM com mapa de igualdade por coluna, coerente com o disco na mesma trava e sob o teto `memoria_max_mb`. Doze testes unitários mais a integração ponta a ponta; o ganho sobre o disco está em `DESEMPENHO.md`, medido pelo `examples/memoria.rs`. | Carga e liberação manuais. Mapa só de igualdade — sem min–max para faixa e `BETWEEN`. O pivot decodifica cada linha duas vezes. O catálogo prometia um `mmap` que não existe — corrigido em 08/09/2026. |
| D | Colunar / vetorial tipo HANA | 2 | recusa medida | **Nada colunar existe**: o motor é 100% row-store. Column store como segundo motor foi recusado com número em três documentos (`MEMORIA.md` §4, `VETORES.md` §0, `GPU.md` §7): achatar linha→coluna custa uma passada inteira pela memória, **mais que a varredura que aceleraria**. | É recusa medida, não buraco — reabre só com gargalo analítico medido. A proposta de vetor **de IA** (`VECTOR` + HNSW, `VETORES.md`) é outra coisa, e está «proposta, não construída». |
| E | IA cognitiva / ações automáticas | 3 | promessa | Integração 100% no navegador: o `claude.js` fala com a API; a IA **propõe** — SQL, plano de modelagem, índice — e toda gravação exige um clique. Quatro testes Rust em `http.rs` (CSP liberado, sem chave no binário). | **Nenhuma ação autônoma do motor**, e isso é escolha registrada (`CLAUDE-IA.md` §10), não falta. As 79 provas de navegador da documentação **não estão no repositório** — pedido 231. Nunca testado com chave real. |
| F | Backup e restauração | 9 | construído | Backup de dados em zip com manifesto SHA-256 e, desde 08/09/2026, **carimbo do instante** (`quando_ms`); restauração com palco fora da raiz e *rename* atômico; **recuperação a um ponto no tempo** (`restaurar_backup` com `ate`): a cópia restaurada vira réplica do diário vivo até o instante pedido, evento a evento, pelo mesmo aplicador da réplica. Doze testes de PITR com oito sabotagens e a bancada `bancada/pitr/provar.py` (22 conferências pelo soquete, com controle). | Sem ZIP64 — acima de 4 GiB recusa alto. Sem prova de autoria: SHA é integridade, não assinatura. Um banco por vez. PITR exige imagem no diário e recusa `por_cima`; tabela nascida depois da cópia não é recriada (o formato não guarda a história do esquema). |
| G | Comandos SQL | 8 | parcial | `SELECT` com **expressão no `WHERE`** (aritmética, `AND`/`OR`, `IN`, `BETWEEN`, `LIKE`, `IS NULL`, funções), **`GROUP BY`** com `COUNT`/`SUM`/`AVG`/`MIN`/`MAX`/`COUNT(DISTINCT)` e `HAVING`, **`JOIN`/`LEFT JOIN`** em sequência, **`WITH`**, subconsulta no `FROM`, `IN (SELECT …)`, subconsulta **escalar**, **`ROW_NUMBER() OVER`**, **`CREATE`/`DROP VIEW`**, `INSERT … ON CONFLICT`/`ON DUPLICATE KEY`, parâmetros `?` resolvidos por token, e o DML por chave única. Tudo traduz para operações do protocolo (`varrer.expressao`, `agrupar`, `consultar`, visões) e passa pelo portão único — **cada tabela de dentro de uma junção ou subconsulta é conferida**. Recusas nomeadas, nunca «sintaxe inválida». Os testes da crate estão no painel. | **Planejador** (o primeiro índice compatível vence), subconsulta **correlacionada** e `EXISTS`, `RIGHT`/`FULL`/`CROSS JOIN`, `COUNT(coluna)`, `ORDER BY` por nome qualificado, `UNION`/`DISTINCT`, `UPDATE`/`DELETE` por faixa. Só `READ COMMITTED` (a Sombra espera decisão). Era **6** na tarde de 08/09; junções, agregação, subconsultas e visões subiram para 8. |
| H | Stored Procedures | 7 | construído | Guardadas em `base/<database>/procedimentos.json` — **uma por database**, não por tabela. `CREATE`/`DROP`/`CALL`; corpo com `IF`/`WHILE`/`DECLARE`/`SET`/`SIGNAL`/`INSERT`/`SELECT INTO`, decimal exato em `i128`; portão `administrar`. Dezesseis testes mais a prova por soquete (`bancada/rotinas/prova-rotinas.py`: `CALL somar(100)` → 5050). **Funciona.** | Sem `UPDATE`/`DELETE` no corpo; sem `CASE`/`LOOP`, cursor, `FUNCTION`, `CALL` aninhado ou transação. Job com corpo `CALL` funciona, mas **sem teste por soquete**. |
| I | Triggers | 8 | construído | Guardados em `base/<database>/gatilhos.json`. `BEFORE`/`AFTER` × `INSERT`/`UPDATE`/`DELETE`; o `BEFORE` roda **com a trava** (altera `NEW`, cancela por `SIGNAL`, não fala com o motor); o `AFTER` solto (audita, falha vira aviso). Teto de cadeia de 8. A bateria achou e corrigiu **dois defeitos graves** — estouro de pilha e um *deadlock* (`TRIGGERS.md` §9). | O mesmo subconjunto de corpo do item H. Sem `FOLLOWS`/`PRECEDES`. Diferenças em relação ao MySQL(R) documentadas. |
| J | Jobs | 8 | construído | Agendador de 30 s; o job carrega um pedido inteiro do protocolo; `jobs.json` mais `jobs.log` *append-only*. Roda **com o poder do usuário do job** — provado: `so_le` recusado a criar database, `root` cria. Aviso por e-mail (falhou, parado) *opt-in*. Dez testes por soquete mais `prova-avisos.py` com SMTP falso. | Sem cron completo — a agenda é o enum `Agenda`. Job com corpo `CALL` sem teste por soquete. |

## Leitura

O núcleo **de armazenamento** — paginação, replicação, backup, jobs, triggers —
está entre 8 e 9: construído, medido e provado por soquete, com defeitos de
produção já caçados. A **camada SQL** deixou de ser o ponto fino em 08/09:
junções, agregação, subconsultas e visões entraram, e o que falta ali é o
planejador e as formas correlacionadas. Sobram os dois pedidos que hoje quase
não existem: colunar tipo HANA e IA autônoma. E a distinção que importa: dessas
duas notas baixas, **D é recusa medida** (o número diz que o colunar custaria
mais do que renderia), enquanto **E é promessa maior que a entrega** — a UI
assistiva funciona, mas «ações automáticas» e as provas documentadas não
estão no código.
