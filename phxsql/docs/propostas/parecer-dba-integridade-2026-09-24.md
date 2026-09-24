# Parecer do DBA (papel C): lote «integridade na transação» — 491, 492, 515, 516, 490 — 24/09/2026

A frente conferida é o worktree `agent-a4004338c4969bba0`, sem commit, sobre o `96fa9b0`. Chamo de **base** o `96fa9b0` (`git archive`) e de **depois** o worktree.

- **Sondas:** rodaram em release, fora da árvore, em `scratchpad/dba-integridade/` (`sonda/src/main.rs`). A mesma sonda compila contra a base e contra o depois.
- **Motores de referência:** o PG 16 e o MySQL 8.0 locais foram medidos em bancos descartáveis. O MariaDB não foi medido: a sondagem da instância local foi recusada pelo ambiente.
- **Testes da frente:** `cascata-ao-alterar` dá 18/18 e `integridade_na_transacao` dá 5/5.

**Veredito: LIBERA COM CONDIÇÃO.**

- Os quatro defeitos de dado fecham, medidos vermelhos na base e verdes depois.
- O 516 introduz uma regressão de vivacidade, também medida (C1).
- Há duas condições de texto (C2 e C3).
- Quatro defeitos que já existiam vão para a conta como ☐, e dois itens ficam ⏸.
- Nada sobe ao dono hoje. Sobe um empate só se o J medir o MariaDB (Q4).

## Q1 — Alguma garantia cai?

| Cenário | Base | Depois |
|---|---|---|
| 492: `[excluir suave M, atualizar M.nome]` na tx | M **ressuscita** | M continua excluída |
| 492: upsert fora de tx sobre M excluída | **ressuscita** | continua excluída |
| 515a: `[filha id 10→11, mãe 5→6]` | id 10 | id 11, cód. 6 |
| 515b: `[filha troca para 8, mãe 5→6]` | cód. **6** | cód. 8 |
| 515c: `[excluir suave a filha, mãe 5→6]` | **ressuscita** | excluída, cód. 6 (restaurável com a FK válida) |
| 515d: `[mãe 5→6, mãe 6→7]` | 7 | 7 |
| 491: folha de auto-referência **sem índice** em `chefe` | sai | **recusa** («crie o índice») |

- **`rowstamp` e `rownum` das filhas:** nenhum elo os mexe. É igual à base em todos os cenários, então «nunca empata, nunca recua» continua valendo.
- **Ordem de digitação:** nenhum slot é reaproveitado.
- **Regra primordial:** vale nos quatro pedidos. No 490, o pânico passa a deixar o que a queda deixa; o resíduo está no P4.
- **Leitura repetível:** vale. O elo que o COMMIT descobre espera o leitor (516). O elo planejado no `empilhar` fica protegido pelo conflito S×IX da tabela: medi nas duas ordens, e o leitor RR ou o escritor é recusado com `EM_TRANSACAO`. **Mas veja C1.**

## Q2 — 516: o laço vivo é real

Medido: T1 muda a mãe `a`, e T2 muda a mãe `b`. Uma terceira sessão insere `c→a` e `d→b`. T1 altera `d.x`, e T2 altera `c.x`.

- Os dois COMMIT respondem `EM_TRANSACAO repetir:true`, **cada um barrado pela trava do outro**.
- Foram **1.870 rodadas em 11 s**, com um prazo de transação de 3 s. Ninguém sai, porque o COMMIT não confere o prazo (P3).
- O ciclo só se quebra quando uma **terceira** sessão dá BEGIN e a varredura roda. Aí **os dois** morrem com `TRANSACAO_ABORTADA`, e ninguém ganha.
- Na base não há laço: T1 grava, e T2 é recusado pela FK antes da marca.
- A mensagem manda «mande COMMIT de novo», e é exatamente isso que alimenta o laço.

## Q3 — 490 × 522: não há conflito mecânico, mas há um lógico

- **Mecânico:** as duas mudanças convergem. O `Drop` do 490 liga `escrita_interrompida`. Com isso, o `pode_baixar_a_marca` do 522 dá falso, o `fechar` não atesta, e o `levantar_marca` do 522 tira o atestado antes. O resultado é o byte 52 em 1 sem atestado. O `sincronizar` continua sendo o único que grava 0, e é dele que a neta precisa entre duas linhas.
- **Mecânico, para o integrador:** o conflito textual cai no `NdxFile` (campos, os dois construtores), no `levantar_marca` e no `Drop`.
- **Lógico:** o arranque do 522 (`reconstruir_indices_marcados`) reconstrói sozinho todo `.ndx` marcado.
  - O sinal do 490, «a filha recusa até o `reindexar`», que o `MANUAL` usa para mandar o operador procurar a chave velha, **some no primeiro reinício**. Vira uma contagem, «índices reconstruídos».
  - Continua valendo só dentro do processo que entrou em pânico.
  - Passam a ser falsas depois do 522: a linha G do `CONTRATO-1.0` («também não é calado»), o `MANUAL` e o `FORMATO.md`.
  - O mesmo já vale para o `SIGKILL`. O conserto de verdade é o P4.

## Q4 — 491: a linha que aponta só para si sair

Não mata pai com filho no sentido que a pétrea protege. A única filha é a própria linha, e nenhuma órfã fica; a palavra do dono é «filhos **em outra tabela(s)**».

| Motor (peso) | `DELETE` do auto-laço | Medido? |
|---|---|---|
| PG (4) | aceita, com RESTRICT e com NO ACTION; o ciclo de dois também sai numa instrução | sim, PG 16 |
| SQLite (1) | aceita | sim, 3.45.1 |
| MySQL (2) | **recusa** (ERROR 1451); a doc diz «not possible to delete a row that refers to itself» | sim, 8.0.46 |
| MariaDB (3) | não medido. É o mesmo InnoDB, então provavelmente recusa | **não** |

- Aceitar soma 5. Recusar soma 2 medidos, ou 5 com o MariaDB.
- **Se o J medir o MariaDB recusando, é empate real 5×5, e sobe ao dono.** Até lá fica a escolha da frente, que não deixa órfã.

## Q5 — Os três «não medidos», medidos

| Item | Medido | Destino |
|---|---|---|
| Inserir numa instrução a linha que aponta para si | PhxSql recusa. PG, MySQL e SQLite aceitam | ⏸ P5: choque com a pétrea «pai primeiro» |
| OLD do BEFORE UPDATE no `empilhar` | `delta = NEW.qtd − OLD.qtd`, na tx 5→3→1: **−4** (na base também). PG 16 e MySQL 8.0 dão **−2** | ☐ P2 |
| Elo do `empilhar` trava a filha só por intenção | Leitura repetível: segura, pelo S×IX. **Mas houve update perdido:** T2 grava `x=1` na filha, solta ou em transação, e o COMMIT de T1 regrava `x=0`. Vale na base e depois | ☐ P1 |

## Condições

**C1 — 516, desempate (entra no mesmo commit).**

- **Regra:** quando o COMMIT é barrado por uma transação que também está com o COMMIT barrado por elo, **a mais nova** recebe `repetir:false`, com o recado «ciclo com a transação N: mande ROLLBACK». A mais velha segue com `repetir:true`.
- **Custo:** uma marca «COMMIT barrado desde» na ficha da transação.
- **Prova real:** no cenário da Q2 (`sonda ciclo`), uma transação termina COMMITTED e a outra recusada, em vez de nenhuma.

**C2 — 491, o preço vai escrito (texto).** A tabela com auto-referência conferida **sem índice** na coluna filha passa a recusar **todo** excluir, inclusive o da folha. Medido: na base saía. Isso vai para o `MANUAL`, o `INTEGRIDADE.md` e o `CHANGELOG`, como nota de atualização, com o conserto: criar o índice, ou `verificar:false`.

**C3 — 490 × 522 (texto, para quem integrar por segundo).** Reescrever a linha G do `CONTRATO-1.0` e o `MANUAL`: depois do 522, o reinício reconstrói a filha marcada em silêncio, e o operador só vê a contagem.

## Pedidos novos (o número é o próximo livre)

**P1 — ☐ ALTO — «O elo planejado no `empilhar` não trava a linha da filha e regrava a linha inteira que viu no `empilhar`: update perdido»**

- **Medido na base e depois:** T1 muda a mãe 5→6, com a filha `x=0`. T2 grava `x=1`, solta ou em transação, e confirma. O COMMIT de T1 deixa `x=0`.
- **Conserto:** travar a LINHA de cada filha do plano no `empilhar`, pelo `travar_para_empilhar`, como o 516 faz no COMMIT. E a passada aplicar só as colunas da chave sobre a linha atual.
- **Formato:** não muda.

**P2 — ☐ MÉDIO — «Dentro da transação, o OLD do BEFORE UPDATE é a linha do DISCO, e não a que a transação vê»**

- **Medido:** um gatilho de delta de estoque dá −4 onde o PG e o MySQL dão −2.
- **Conserto:** passar ao gatilho a `linha_na_transacao` do 492 como OLD. É a família do 492.

**P3 — ☐ MÉDIO — «O COMMIT ignora o prazo da transação»**

- **Medido:** COMMITTED 600 ms depois de um prazo de 200 ms, na base e depois.
- **Causa:** `OPS_DE_TRANSACAO` pula o portão do prazo, e a varredura só roda no `begin` e no `transacoes`.
- **Conserto:** o COMMIT confere o prazo antes da marca. Sem isso, o C1 fica sem teto.

**P4 — ☐ ALTO — «A cascata solta (fora de transação) não tem marca: queda ou pânico no meio deixa a filha órfã, e o `reindexar` a cala (com o 522, o arranque também)»**

- **O que já existe:** o 490 dá paridade com o `SIGKILL`, e a prova da frente mostra a filha 2 no `buscar(1)` depois do `reindexar`.
- **Conserto:** o `atualizar` solto com cascata não vazia grava a marca, como uma transação de uma instrução, e a recuperação completa. O 451 já prova isso dentro de transação.
- **Custo:** um `fsync` de marca por troca de chave com filhas, uma operação rara.
- **Formato:** não muda.

**P5 — ⏸ — «Inserir numa instrução a linha que aponta para si»**

- **Medido:** o PG, o MySQL e o SQLite aceitam, e o PhxSql recusa.
- **Choque com a pétrea «só existe filho se o pai existir primeiro»:** a linha é pai de si no mesmo instante. Quem quer o auto-laço hoje insere com nulo e depois altera.
- Vai à mesa como choque registrado, e só se alguém pedir a mudança.

## Ao dono

**Nada hoje.** Sobe só se o J medir o MariaDB recusando o auto-laço (Q4, empate 5×5).

## Re-checagem das condições — 24/09/2026, mesmo worktree, sem commit

**Veredito: LIBERA.** C1, C2 e C3 estão cumpridas e medidas. Sobraram três achados pequenos, e nenhum é de dado: um ⏸ e dois ajustes de texto que cabem no mesmo commit.

- **Sonda:** a de antes (`scratchpad/dba-integridade/sonda`), com três modos novos: `c1`, `c1misto` e `c2`. Os mesmos rodaram contra a base (`sonda-base`). As saídas estão em `ciclo-C1.txt`, `c1-bordas-depois.txt`, `c1-bordas-base.txt`, `c1-misto-depois.txt` e `c2-depois.txt`.
- **Testes da frente**, no target do worktree:
  - `dois_commits_que_se_barram_cedem_pela_mais_nova`, `o_ciclo_de_commits_barrados_cede_pela_mais_nova` e os quatro do 491: **verdes**, 6/6.
  - O filtro de prazo, com `o_prazo_estourado_reverte_e_solta_as_travas` dentro: **17/17**.

### C1 — cumprida

| Prova | Antes do C1 | Agora |
|---|---|---|
| `sonda ciclo 3000` (o cenário da Q2) | 1.870 rodadas em 11 s, e ninguém sai | Na rodada 1, a mais nova (T2) recebe `TRANSACAO_ABORTADA repetir=false`. Na seguinte, T1 fica `COMMITTED`, com 3 gravadas |
| C1-A: a que cede solta as travas na hora? | — | Sim. Uma escrita solta na linha que ela travava passa em **0 ms**, antes do ROLLBACK dela. O COMMIT e a instrução seguintes dela dão `TRANSACAO_ABORTADA`, e o ROLLBACK sai |
| C1-B: ciclo de três | — | Só a mais nova cede (T3, na rodada 1). T1 e T2 confirmam na rodada 2 |
| C1-C: o prazo da outra estoura no meio, e a varredura a mata | — | O `estourar_prazo` passa pelo `abortar_soltando`, que limpa a aresta e solta as travas. T1 confirma, e a morta responde com o prazo |
| C1-E: prazo fora de ciclo, base × depois | — | **Idênticos**: a mensagem, a escrita alheia imediata, o COMMIT recusado e o ROLLBACK. O `abortar_soltando` não mudou o prazo velho |
| O dado, nos cinco | — | A FK fica íntegra. A filha só leva a cascata de quem confirmou. `rowstamp` e `rownum` ficam iguais aos de antes, e nenhum slot é reaproveitado |

Lido no código:

- **A cessão acontece antes da marca.** A volta do `op_commit` (`servidor.rs:16526–16535`) roda antes de `marca_em_voo` e de `gravar_marca`. Não há marca nem diário a reconciliar, e a lista já tinha saído pelo `take`.
- **Não há ciclo que escape.** Uma corrente que chega a uma transação já terminada volta `None` no `por_id`, e o id não se reusa (`proximo += 1`). Cada transação tem uma aresta só, então há no máximo um ciclo por corrente, e só a de id maior cede.

### C2 — cumprida

| Auto-referência (`sonda c2`) | Excluir a folha de vez | Excluir a folha suave | Excluir o auto-laço | Dentro da transação |
|---|---|---|---|---|
| Conferida, **sem** índice em `chefe` | recusa | recusa | recusa | recusa no COMMIT |
| `verificar:false` | sai | sai | sai | sai |
| Com índice | sai | sai | sai | sai. O chefe com subordinado recusa |

- A recusa nomeia a chave (`fk_chefe`), a coluna (`chefe`) e os dois consertos. É o que dizem o `MANUAL`, o `INTEGRIDADE.md` §7.4 e o `CHANGELOG`.
- O texto não diz que, dentro da transação, a recusa só chega no COMMIT e leva a transação inteira. Não viro isso em condição: é o regime do 448 para toda FK, e não do 491.

### C3 — cumprida, conferida contra o código do 522

- **O HEAD andou.** Era `41c06a5`, e agora é `8a9814c`. O 522 já está nele, como `b5fc11c`.
- **O arranque confere com o texto.** O `reconstruir_os_marcados` roda **depois** das marcas e imprime `indices reconstruidos ......... N`. Por isso vale a primeira frase da linha G do `CONTRATO-1.0` (a recuperação recusa e denuncia), e vale o texto novo: o pânico faz a filha recusar enquanto o processo vive, o reinício reconstrói e deixa só a contagem, e as filhas continuam na chave velha. O `MANUAL`, o `ACID.md` §2.4, o `FORMATO.md` e o `SEGURANCA.md` §24.3/§24.5 dizem o mesmo.
- **As frases do P1 e do P2 estão corrigidas.** O `ACID.md` agora tem o update perdido medido e o OLD −4 × −2.
- **Para o integrador:** o texto só é verdade em cima de `b5fc11c`, e o worktree está sobre `96fa9b0`, sem o 522. Integre sobre o HEAD vivo. Na árvore unida, rode `panico_no_meio_da_cascata_fora_da_transacao_deixa_a_filha_recusando` e `fechar-nao-baixa-a-marca`: a convergência mecânica da Q3 é só de leitura.

### Achados da re-checagem

| # | Estado | Achado | Medido | Conserto |
|---|---|---|---|---|
| R1 | ⏸ | **Aresta velha: a mais nova cede sem haver ciclo** | C1-D: T1 é barrada por T2, volta ao SAVEPOINT e passa a escrever só `cb`. O COMMIT de T2 recebe `TRANSACAO_ABORTADA`, «ciclo com T1», e T1 confirma logo depois sem precisar de nada de T2. Não há dado errado: nada é gravado e as travas saem. Da mesma família, só por leitura: uma transação em ABORT_ONLY pelo teto segura as travas até o ROLLBACK e guarda a aresta | O `op_rollback_para` limpa `commit_barrado_por`, e a corrente não atravessa transação fora de `Ativa`. O teste é o próprio C1-D. Cabe no mesmo commit; se não couber, vira pedido ⏸ |
| R2 | texto | «desempate por idade dos motores com detecção de impasse», no `ACID.md` e no comentário de `Transacoes::commit_barrado`, **é falso sobre os motores**. O PG aborta uma das transações, sem ordem garantida («should not be relied upon»), e o InnoDB aborta a mais leve (linhas alteradas) | lido na documentação dos dois | A idade é escolha nossa, determinística e que garante progresso, no molde do wait-die: a frase tem de dizer isso |
| R3 | texto | O `MANUAL` não diz que o COMMIT pode voltar `EM_TRANSACAO` (516) nem `TRANSACAO_ABORTADA` por ciclo (C1). A lista «transacao: teto, E/S, prazo» ficou incompleta | leitura do `MANUAL.txt:2005–2027` | uma linha |

**O que sobra não é do C1** (`sonda c1misto`): o ciclo misto não entra no grafo. Nele o COMMIT de T1 é barrado por T2, e a instrução de T2 espera uma linha de T1.

- Cada tentativa de T2 cai no LOCK TIMEOUT, de 500 ms.
- O ciclo só termina no TIMEOUT da transação de T2: 2.000 ms na prova, e T1 confirmou aos 2.008 ms.
- É o mesmo regime que a base já tinha entre duas instruções. Com o TIMEOUT padrão, o teto é 5 min.
- O P3 continua ☐.
