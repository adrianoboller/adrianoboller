# Parecer do DBA (papel C): pedido 368, terceira volta — 24/09/2026

**Veredito: LIBERA.** B1, B2 e P3 estão fechados, e isso foi medido. Há uma condição de integração, que é só texto (seção C). Este parecer existe porque o item 4 pede **mudança de formato**, que vai para um pedido próprio. O 368 não espera por ele.

Worktree revisado: `.claude/worktrees/agent-a88017bcd787ed40c`, diff contra `82a17ef`.

Provas de leitura: programa externo em scratch (`dba-368c/prova`), ligado no `phxsql-store` do worktree. Build de depuração, Linux, ext4.

## A) O que foi pedido, conferido

| item | medido | estado |
|---|---|---|
| **B1**: `exigir_nome_que_volta`, função única nas quatro portas | `x_001`, `vendas_2024`, `nfe_2023` e `x_0` são recusados no `criar_tabela`. `x_002` é recusado no `duplicar_tabela`, `x_003` no `copiar_tabela_para` e `x_004` no `renomear_tabela`. A cópia da regra que havia no renomear saiu. Não há quinta porta no `catalogo.rs`: as chamadas a `Table::criar` no servidor são todas de teste. Vermelho: na `82a17ef`, `criar_tabela("x_001")` era aceito (P2c da 2ª volta) | ok |
| **B2**: ativo curto é nascimento interrompido | O ativo de **0 byte** e o de **100 bytes v3** abrem, com total 3 e volumes `[1]`. O evento seguinte nasce no **2** (o mesmo N), e o total vai a 4. Duas instâncias que viram o interrompido: a segunda nasce, e a primeira recusa com «deixou de ser», **sem apagar** o arquivo da segunda. No servidor isso nem acontece, porque o `abrir_travada` abre a tabela de novo a cada pedido. Vermelho: na 2ª volta, `Table::abrir` falhava com `failed to fill whole buffer` (P1) | ok |
| catraca `alcancam-fsync-2` | `mapa-da-trava.py --catraca`: **23 (teto 23)**. `Volumes::criar` e `apagar_volume` não fazem `sync_all` | ok |
| **P3**: o plano faz nascer o ativo que falta, e o selo o leva ao disco | Queda entre o `rename` e o nascimento, depois expurgo: saem `[1, 2]`, `sincronizados` = **2** (o rastro e o ativo), e o evento seguinte vai para o volume **3**. Na 2ª volta ia para o 1 | ok |
| testes da frente | lib do `phxsql-store` **229/229**, `trilha-lgpd` **23/23**, `cifra-dos-diarios` **16/16** | verde |

**O que um SIGKILL deixa no expurgo.** Foi medido com um filho morto por `Child::kill()`, que é SIGKILL de verdade:

| morto depois de | no disco | reaberta | expurgo de novo |
|---|---|---|---|
| fase 1 (plano: ativo nascido e rastro escrito, sem `fsync`) | `.lgpd` (64 bytes), `_001`, `_002` | volumes `[1,2,3]`, total 4, 1 rastro | saem `[1,2]`, ficam 2 rastros, e o próximo evento vai para o 3 |
| fase 2 (selo) | o mesmo | o mesmo | o mesmo |
| no meio dos `unlink` (simulado: só o `_001` saiu) | `.lgpd` (64 bytes), `_002` | — | o próximo evento vai para o 3, e os volumes ficam `[2,3]` |

O rastro duplicado é o «rastro sem apagamento» já documentado. Numa queda de energia antes da fase 2, o ativo nascido pode voltar com 0 byte, e aí a regra do B2 o absorve. Nada sai antes dos dois `fsync` da fase 2.

## B) Achados novos, todos ANTERIORES ao 368 e nenhum agravado por ele

| # | medido | estado |
|---|---|---|
| **N1** sufixo de **letra** passa pela porta | `criar_tabela("x_A")` é **aceito**, e logo depois `todas_as_tabelas` = `["x"]` e `existe_tabela("x_A")` = **false**. `x_B`, sem `x_A` ao lado, é aceito; o `excluir_tabela("x")` de uma `x` sem partição apagou **16 arquivos**, **8 deles de `x_B`**. Com `tipo_B` e depois `tipo_A`, a árvore mostra só `["tipo"]`. **Causa:** a regra da letra no `nome_da_tabela` pergunta ao disco se `<nome>_A.reg` existe, e na declaração esse arquivo ainda não existe. O `pertence` (do DROP) nem pergunta. O expurgo da trilha só lê dígitos, então o 368 não o alcança | ☐ **defeito ativo** (o DROP apaga outra tabela), na conta. Entra no pedido do formato (D) |
| **N3** ponto no nome, na raiz | `criar_tabela("a.b")` é aceito e aparece na árvore, mas `abrir_qualificada("a.b")` dá «a tabela a.b não existe»: o `separar_qualificado` o lê como schema `a`, tabela `b` | ☐ mesmo pedido |

## C) Condição de integração: só texto, e nenhum código

**O que o texto afirma, e a medição do N1 desmente:**

- `docs/FORMATO.md:1749` diz que a declaração recusa o sufixo «de letra da partição»;
- `catalogo.rs:137` diz que recusa «o nome que o catálogo NÃO leria de volta».

Hoje as duas coisas valem **só para dígitos**. O texto deve dizer isso e citar o pedido novo. É o mesmo erro da 2ª volta (`FORMATO.md:1731`): comentário que se declara resolvido é o motivo de ninguém olhar de novo.

- `trilha.rs:1326` diz «1 com volume a derrubar, 0 sem». Com o ativo nascido, o medido é **2**.

## D) Item 4: recusar na declaração, ou separador no formato?

### O custo das duas

| | **A: recusa na declaração (hoje)** | **B: separador que nome de tabela não aceita** |
|---|---|---|
| nomes perdidos | todo `*_<dígitos>`: `vendas_2024`, `nfe_2023`. Para ficar completa, perde também os **37** sufixos de letra (`_A`–`_Z`, `_0`–`_9`, `_Outros`) do N1 e o `.` do N3 | **um** caractere. `#`, `@`, `~`, `+` e `=` são aceitos hoje (medido). O `.` já está quebrado na raiz (N3) |
| comportamento | **diverge** dos quatro. PG, MariaDB, MySQL e SQLite aceitam `vendas_2024` como nome de tabela. O PG nomeia arquivo por OID; MySQL e MariaDB usam `#P#`/`#p#` nas partições e codificam o `#` do nome (`@0023`), e por isso o separador nunca aparece num nome. Nenhum deles restringe o nome para proteger o arquivo | converge: o nome fica livre, e o separador é do formato |
| buraco estrutural | a letra se decide pelo disco na **leitura**, e por isso a declaração só a fecha recusando os 37 sufixos | some: o `nome_da_tabela` deixa de precisar de `stat` |
| código | pronto | **12 sítios em 9 arquivos** compõem ou leem `_<volume>` (grep desta revisão): `paginacao.rs:525`, `volume.rs:429/435`, `catalogo.rs:81/177`, `trilha.rs:740`, `reg.rs:2846`, `restaurar.rs:336`, `pag.rs:176`, `servidor.rs:18981/24178` e `phxsql-cli/main.rs:221`. Mais fixtures e testes |
| migração | nenhuma | **não basta ler os dois nomes.** O catálogo lista pelo nome do arquivo: enquanto reconhecer o `_NNN` antigo, `vendas_2024.reg` continua ambíguo e o ganho não chega. O ganho pede **renomear os volumes antigos** uma vez, com marca de versão, com estado misto legível (o `rename` é atômico por arquivo) e com teste de queda no meio. Só as tabelas paginadas e os fechados da trilha têm sufixo |

**Hipótese descartada sem medir:** desambiguar pelo conteúdo, isto é, o catálogo ler o cabeçalho do `.reg` sufixado. Ela põe uma leitura por volume na listagem, e a listagem roda a cada exclusão pela busca reversa da FK. É o custo que a casa escolheu manter fora de lá.

### O que recomendo

**B, num pedido próprio, antes de haver dado em produção.** A convergência dos quatro diz que `vendas_2024` é nome de tabela, e nenhuma pétrea se opõe, então o comportamento entra por aceite automático. O meio é formato, e formato entra **cedo**: hoje custa código e um teste de migração; depois do primeiro cliente, custa a migração no disco dele.

**Até o B entrar, a recusa A fica como guarda.** Ela não tira nome que funcione. Medido: uma `vendas_2024` criada antes já some da árvore (`todas` = `["vendas"]`, `existe_tabela` = false). A recusa só troca perda calada por erro na declaração.

**Primeiro passo barato do pedido novo, por causa do N1**, que é perda de dado:

- estender a mesma função aos 37 sufixos de letra;
- depois, o separador os libera todos de volta.

**A escolha do caractere é do J**, com duas hipóteses:

- `#`, pelo precedente do MySQL e do MariaDB;
- `.`, que tem custo zero na raiz, mas é o separador do nome qualificado e mora ao lado da extensão.

Minhas restrições para a escolha:

- o caractere é válido em NTFS, ext4 e APFS;
- fica fora do conjunto do `validar_nome`, porque todo caractere de lá é ilegal em nome de arquivo no Windows;
- passa a ser recusado **só** em nome de tabela.

**Sobe ao dono: nada.** A convergência resolve o comportamento, e a pétrea «formato entra cedo» resolve o momento.
