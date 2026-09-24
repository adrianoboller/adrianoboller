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
