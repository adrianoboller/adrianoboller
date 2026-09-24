# Parecer do DBA (papel C): lote «integridade 2» — 537, 538, 539, 540, 559 — 24/09/2026

- **Árvore conferida:** o worktree `agent-a99cd40ca4e973b20` já com a árvore INTEGRADA (base `d76cd66`, fáceis C/D, travas em `.travar()`), sincronizada às 19:48. O diff só da frente é o `integridade2.patch` sobre o `a494f33`. As funções vão citadas pelo nome, porque as linhas andaram.
- **Sondas:** ficam em `scratchpad/dba-integridade-2/`. A `sonda/` foi compilada contra a árvore integrada, e a `sonda-base/` contra o `a494f33` (`git archive`). O mesmo `main.rs` roda nas duas. A `sonda-frente/` é a da frente, recompilada contra a integrada.
- **Motores:** MySQL 8.0.46 local, usado como o outro lado do DbLink (recriado por `mysql -uroot < scratchpad/dba-integridade-2/mysql.sql`) e como referência. PG 16.13 e SQLite 3.45.1 também locais. O MariaDB não foi medido: vale a KB dele.

## Veredito

| Pedido | Veredito | Em uma linha |
|---|---|---|
| **537** | **LIBERA** | Trava a linha e refaz o elo. O limite de 4 voltas custa ≤ 2% das instruções, só sob repontamento hostil. Condição de texto: C3 |
| **538** | **LIBERA** | O OLD dá −2, como no PG e no MySQL, agora também na árvore integrada |
| **539** | **LIBERA** | O COMMIT feito 600 ms depois de um prazo de 200 ms volta `TRANSACAO_ABORTADA`, com nada gravado |
| **559** | **LIBERA** | Lido: a varredura não toca o `Confirmando`, e o `devolver_a_lista` só devolve a quem continua nesse estado |
| **540** | **BLOQUEIA** | Pela `op_atualizar` e pelo upsert, a cascata sai inteira. **Pela sincronia do DbLink, corrompe o índice único da mãe: 5 de 5 rodadas.** Libera com C1 + C2 |

O 540 entra com C1 no mesmo commit. Se não couber, o integrador pode levar 537, 538, 539 e 559 sem ele: nenhum dos quatro depende do `alterar_solto`.

## Q1 — Cai alguma garantia? No 540 cai, pelo terceiro irmão

O `alterar_solto` abre a passada num **segundo punho** da mãe enquanto o punho `t` de quem chama ainda está vivo. Pela `op_atualizar` e pelo upsert, esse `t` está limpo. Na **sincronia**, ele já inseriu ou alterou linhas antes, e está com páginas sujas.

O que acontece em sequência:

1. O punho novo abre o `.ndx` e acha o byte 52 em 1, sem atestado. Recusa com `CORROMPIDO`, na escrita 1.
2. Esse erro não é «do pedido». Então a passada chama o `completar_marca`, que **reindexa a mãe a partir do `.reg`** e completa a marca.
3. Em seguida, `*t = abrir_travada(..)` derruba o punho velho. O `Drop`/`fechar` dele grava as páginas sujas e o cabeçalho **velhos** por cima do `.ndx` recém-reconstruído, que é o mesmo inode (o `criar` trunca). E atesta.

É o mesmo defeito que o comentário do `completar()` avisa: «no caminho normal, índice sujo quer dizer outro descritor com escrita pendente». O comentário do próprio `alterar_solto` diz «o velho não escreveu nada», e isso vale para dois dos três chamadores.

Medido na sincronia `puxar` de 41 linhas (40 novas e a mãe 5→6 com 2 filhas). Arquivos `base-sinc.txt`, `depois-sinc.txt`, `*-sincdano.txt`:

| | Base (`a494f33`) | Integrada |
|---|---|---|
| Resposta do `dblink_sincronizar` | OK | OK, com aviso «quebrou depois da marca (… ficou para trás numa queda …)» |
| `buscar por_codigo 6` / `5` | 1 / 0 | **0 / 1** (5 de 5 rodadas) |
| `inserir` cliente com `codigo 6`, que é único | `DUPLICADO` | **aceito: dois clientes com código 6** |
| `inserir` pedido com `cod_cliente 5`, sem mãe | `INTEGRIDADE` | **aceito: órfã** |
| Reabrir em outro processo | limpo | o arranque não reconstrói (`chave duplicada … por_codigo`), e a tabela **recusa tudo** até alguém limpar à mão |
| `verificar` | 41 / 41 | 41 / 41: **não enxerga** o problema |

O resto de Q1, pela `op_atualizar` e pelo upsert:

- **Continua valendo:** FK, unicidade, `rowstamp`/`rownum` (o elo não os toca), ordem de digitação (nenhum slot é reaproveitado) e a atomicidade da marca (as provas da frente de pânico e de `SIGKILL`).
- **Formato:** não muda. O `elo_do_empilhar` fica só em memória, e o `gravar_marca` não o lê.
- **451/490/522:** a prova de loja do 490 (`panico_entre_duas_filhas_deixa_a_filha_recusando_como_um_sigkill`) continua em duas guardas. Trocar a prova do 490 pelo soquete por `…_sai_com_a_cascata_inteira` é honesto: por ali a garantia ficou **mais forte**.
- **O `a1_a_filha_da_propria_lista…`:** mudá-lo de `caem` para `seguem` na `commit-sem-pre-conferencia` também é honesto. Ele passa com só uma das duas redes (515 ou 448), e continua em `caem` na `passada-replaneja-a-cascata`.

## Q2 — A marca solta na recuperação, na réplica e no diário

- **Arranque:** é a mesma leitura e o mesmo `completar`. O `id` sai do contador semeado pelo relógio, e não colide. A prova de `SIGKILL` da frente vale.
- **Diário:** a passada escreve pelos punhos do `abrir_travada`, com `imagem_da_linha` do config, igual à cascata antiga.
- **O que a recuperação grava vai sem imagem.** O `completar()` abre as tabelas por `db.abrir_qualificada` e nunca liga `ligar_imagem_no_diario`. A réplica recusa evento de alteração sem imagem («veio sem imagem», `Table::aplicar_evento_interno`). Isso já valia para o COMMIT. O 540 põe a cascata solta nesse caminho, e o defeito do C1 põe **toda** linha de sincronia com cascata nele. Lido, **não medido**: P4.
- **Réplica:** não julga. A atomicidade entre tabelas não atravessa o fio (299), igual ao COMMIT.

## Q3 — O limite de 4 voltas do 537 sob carga

T1 muda a mãe 5↔6. N conexões soltas repontam filhas entre 8, 5 e 6 sem parar. 100 tentativas (`*-voltas.txt`):

| Filhas × escritores | Recusa por 4 voltas | O COMMIT recusa pela 448 (base / integrada) | `empilhar` p50, base → integrada | Órfãs |
|---|---|---|---|---|
| 20 × 4 | 0 | 54% / 48% | 3 → 4 ms | 0 |
| 5 × 8 | 0 | 53% / 55% | 3 → 3 ms | 0 |
| 50 × 8 | **2** (`repetir:true`, a transação segue) | 70% / 61% | 6 → 12 ms | 0 |

Recusa escrita legítima, mas só quando alguém reponta filhas para a chave velha a cada fresta. Sob essa mesma carga, o COMMIT já recusa mais da metade. Aceito.

Custo menor: as linhas que saíram do plano entre uma volta e outra ficam travadas até o fim. É trava a mais, sem dado errado.

## Q4 — O `refazer_o_elo` sobre uma linha excluída suave no meio

Por leitura:

- **Pela `excluir`, solta ou em transação, não acontece:** as duas esbarram na trava X da linha (537a e o `conflito_de_linha`).
- **Pela sincronia, acontece:** ela grava a linha inteira de lá, marca inclusive, sem olhar trava nenhuma (P1).
- **Mesmo assim, o refazer preserva a exclusão:** só move a coluna em que `agora == antigo`. A filha fica excluída e com a chave nova, restaurável com a FK válida (a decisão do 515c).
- **Excluída de vez no meio:** o `ler` volta `None`, e a pré-conferência recusa com zero gravado.

## Q5 — 539, 559 e 538 na árvore integrada

Rodei a `sonda-frente` (`integrada-bordas.txt`):

- `prazo`, `old` e `perdida` saem iguais aos `depois-*` da frente.
- C1-A…E saem iguais linha a linha (52 de 52, com id e tempo normalizados), e o `c1misto` também: T1 confirma aos 2.011 ms, com prazo de 2.000.
- No `ciclo3`, T1 tenta de novo depois de o prazo dela vencer e recebe `TIMEOUT`, em vez de confirmar. É o 539 funcionando.

## Condições do 540

**C1 — o punho de quem chama não pode ter escrita pendente quando a passada abre o dela.**

- **Onde:** no `alterar_solto`, a porta única, e não na sincronia.
- **Duas formas servem:**
  - a passada recebe o punho `t` da mãe já aberto (sem segundo descritor);
  - ou `t` desce as páginas antes do plano ir à marca.
- **Custo:** nenhuma das duas pode pôr `fsync` na `op_atualizar`, cujo `t` está limpo. O ganho medido do 540 (2,0–2,2× em `por_lote`) faz parte do caso. Remeça com o `custo` da frente.
- **Prova real:** um teste em `servidor.rs`: `t.inserir(..)` e, no mesmo punho, `alterar_solto` com cascata. O `buscar por_codigo` pela chave nova tem de dar 1, e a duplicata e a órfã têm de ser recusadas. Tem de ficar **vermelho hoje**; a sonda mede 5 de 5.
- **Guarda:** entra no catálogo junto.

**C2 — texto:**

- o doc do `alterar_solto` («o velho não escreveu nada»);
- a frase do `ParouNoMeio` no nome `DA_CASCATA_SOLTA` («a conferência de antes da marca tinha aprovado esta lista»). Na cascata solta não há pré-conferência, só a árvore do plano;
- CHANGELOG, MANUAL, ACID, CONTRATO e INTEGRIDADE dizem que a sincronia do DbLink «grava a marca e completa». Só é verdade depois do C1.

**C3 — texto, 537:** o «Sabido» do CHANGELOG e o `INTEGRIDADE.md`/`ACID.md` dizem que, no achado (1), «o COMMIT refaz o elo e não há update perdido». **A medição desmente** (P1 b): o refazer cobre só o elo. A escrita **da própria** T1 na filha regrava a linha inteira e desfaz a cascata solta.

## Pedidos propostos (o número é o próximo livre)

**P1 — ☐ ALTO — «A escrita solta não pergunta pela trava de LINHA das linhas que ela grava sem nomear: a filha da cascata solta, a linha que o upsert solto altera, e as da sincronia do DbLink»**

- **É o achado (1) da frente, medido e mais largo.** Base = integrada, `*-trava-after.txt`.
- **(a)** T1 grava `x=1` na filha. A `atualizar` solta da filha recusa (4005), mas a cascata solta do vendedor 3→4 passa. O COMMIT de T1 recusa por `fk_vend`, e T1 perde o trabalho.
- **(b)** Igual, mas o vendedor 3 renasce antes do COMMIT. **T1 confirma, e a filha aponta para o vendedor NOVO 3.** A cascata sumiu: é mãe errada.
- **(c)** Leitura repetível: T3 lê `vend=3`, a cascata solta passa por cima da S, e T3 relê `vend=4`.
- **(d)** T1 segura X no cliente 1. A `atualizar` solta recusa, e **o upsert solto passa** (`atualizada:true`), porque o portão do `inserir` só olha o `FIM_DA_TABELA`. O COMMIT de T1 apaga o upsert que já tinha respondido OK: é update perdido.
- **Conserto:** desde o 540 o `alterar_solto` é a porta única dos três irmãos e tem o plano na mão. Ali se pergunta o `conflito_de_linha` da linha da mãe e de cada filha do plano, antes da marca, e se recusa 4005 `repetir` como a `atualizar` solta.
- **Formato:** não muda.

**P2 — ☐ BAIXO — «A mesma cascata dispara o AFTER da filha na transação e não fora dela»**

- **Medido:** solta, a auditoria fica com `mae:1`. Na transação, `af` dispara no COMMIT e cai no aviso do pedido 262.
- **Os motores empatam, 5×5:**
  - dispara: PG 16 (2 linhas, medido) e SQLite 3.45 (2 linhas, medido);
  - não dispara: MySQL 8.0 (0 linhas, medido) e MariaDB (KB, não medido).
- A §7.3 já decidiu «não dispara» e citou só um lado. O conserto alinha o motor à decisão escrita: a **passada** não junta o AFTER de elo, e a decisão sai de um lugar só, em vez de o `atualizar_com_a_marca` jogar a lista fora.
- A §7.3 passa a citar a matriz inteira. **Mudar a §7.3 é empate real e sobe ao dono.** Alinhar o código a ela não sobe.

**P3 — ⏸ — «O `Table::atualizar` do store embutido cascateia sem marca»** (achado 3 da frente)

- O preço está declarado no CONTRATO G: o 490 recusa até o `reindexar`, e a órfã fica.
- Levar a marca para o store é uma frente própria. Entra se o produto prometer atomicidade no embutido, e isso é decisão de produto.

**P4 — ☐ MÉDIO — «A recuperação grava o diário sem imagem: num source replicado, a réplica recusa o evento completado»**

- **Lido, não medido.** O `transacao::completar` abre por `abrir_qualificada` e nunca chama `ligar_imagem_no_diario`.
- Vale para todo COMMIT recuperado. O 540 e o defeito do C1 aumentam o alcance.
- **Medir antes de consertar:** um source com `imagem_da_linha` e uma réplica; um pânico na passada; ver se a réplica anda.
- **Conserto provável:** o `completar` liga a imagem pelo config, como o `abrir_travada`.

## Ao dono

Nada hoje. O empate 5×5 do P2 só sobe se alguém propuser trocar a §7.3.
