# Cognições de 08/10/2026: estado e evidência (papel J)

Data: 08/10/2026, conferido no HEAD `ed582124`. Só leitura: nenhum arquivo de cognição foi
editado. Quem aplica é o integrador (pétrea de 24/09: nada promove sozinho).

## Hipóteses e régua

- **H1:** basta a referência existir (o teste, o commit), que é o que o
  `avoid-e-reuse.py --catraca` confere.
- **H2:** é preciso um RED **independente**. Isso quer dizer o provador de guardas
  (`bancada/guardas/provar-guardas.py`) com veredito `PROVADA` em `ultima-corrida.json`, que
  repõe o defeito, vê o teste cair e o vê passar com o conserto.

**Venceu H2.** H1 confere que o teste existe, mas não confere que ele cai com o defeito.
O «RED medido à mão», escrito pelo próprio autor, é a prosa que a pétrea recusa. Por
isso **FRUTÍFERO só se recomenda onde há guarda `PROVADA`**. As outras ficam «promovíveis»:
falta abrir a guarda no catálogo e rodar o provador.

Fonte dos vereditos: `bancada/guardas/ultima-corrida.json` (corrida de 08/10/2026, 20:17).
Catraca hoje: 438 cognições, 417 PENDENTE, 11 FRUTÍFERO, 10 INFRUTÍFERO, verde.

## Matriz

| # | Arquivo (`docs/cognicao/…_20261008_*`) | Estado hoje | Evidência validada? | INFRUTÍFERO? | Pétrea reafirmada? | Recomendação |
|---|---|---|---|---|---|---|
| 1 | `camada-nova-antes-da-marca-esconde-a-rede-velha_1650` | PENDENTE | **Sim.** A guarda `commit-sem-as-duas-recusas-antes-da-marca` deu PROVADA em 08/10 às 20:17. Teste: `crates/phxsql-server/tests/commit-pelo-soquete.rs::o_commit_contra_a_tabela_congelada_nao_sai_pela_metade`. Commit: `ba65032e`. | Não. A hipótese (b) morreu e está registrada. | Não. É o **alcance** de «teste que passa por engano» (papel F): a guarda de uma camada só fica válida com a camada vizinha também tirada. | **FRUTÍFERO.** `**Evidência:** \`crates/phxsql-server/tests/commit-pelo-soquete.rs::o_commit_contra_a_tabela_congelada_nao_sai_pela_metade\`; \`ba65032e\`` |
| 2 | `conta-da-origem-tem-de-ser-teto-da-conta-da-replica_0100` | PENDENTE | **Sim.** As guardas `custo-da-transacao-sem-a-imagem` e `commit-acima-do-teto-aceito` deram PROVADA. Teste: `crates/phxsql-server/tests/transacao-acima-do-teto.rs::a_transacao_acima_do_teto_e_recusada_no_commit_e_a_que_cabe_chega_inteira`. Commit: `4b388e62`. | Não | Não | **FRUTÍFERO** com essas duas referências. Trocar ainda a frase «o provador não rodou», que **envelheceu** (ele rodou, e as duas deram PROVADA). |
| 3 | `expurgo-errado-no-diario-para-o-central-em-vez-de-perder-venda_1900` | PENDENTE | **Não validada.** Os testes existem: `expurgo-do-diario-pelo-soquete.rs::o_diario_do_caixa_encolhe_e_o_central_que_volta_nao_perde_venda`, `crates/phxsql-store/tests/expurgo-do-diario.rs::base_perdida_no_primeiro_volume_recusa` e o commit `ba65032e`. Mas nenhuma guarda do catálogo os cita (0 menções), e o RED (2 de 2, 3 de 9, 6 de 9) foi medido só à mão. | Não | Não | **PENDENTE.** Abrir a guarda `expurgo-trata-tudo-como-confirmado` (`confirmado_por_todos` → `u64::MAX`). Com PROVADA, vira FRUTÍFERO. |
| 4 | `fase-b-e-o-rename-por-cima-nao-o-fsync_1738` | PENDENTE | **Parcial.** O teste `crates/phxsql-store/tests/migracao-da-cifra.rs::a_fase_b_segura_o_volume_velho_ate_soltar` existe (`ba65032e`) e o número tem arquivo: `bancada/cifra-migracao/resultados.json` (chaves `grandes` e `grandes_antes_do_647`). Mas nenhuma guarda repõe o `segurar_o_velho`. O RED de 0 contra 3 descritores foi medido à mão. | Sim, **em parte.** A H2 (o custo é do `rename` em si) morreu medida: 797–802 ms no `rename` contra 0,0–0,1 ms segurando. Isso é falha observada com causa e prevenção, só que escrita em prosa, sem os campos. | Não | Manter **PENDENTE**. Abrir a guarda `fase-b-sem-segurar-o-velho`. Registrar a H2 morta como INFRUTÍFERO à parte (`**Causa:**` liberação do inode na última referência; `**Prevenção:**` medir o `rename` e o `close` antes do `fsync`), para ela alimentar o avoid. |
| 5 | `id-zero-do-volume-velho-engana-a-conferencia-pelo-id_1720` | **Sem a linha** (PENDENTE por omissão) | **Não validada.** O teste `crates/phxsql-store/src/log.rs::a_marca_com_id_no_diario_sem_id_nao_duplica_a_inclusao` existe (`ba65032e`), com RED de 2 contra 1 descrito à mão. Nenhuma guarda. | Não | Não | Acrescentar `**Estado:** PENDENTE` (hoje falta a linha). Abrir a guarda `diario-da-linha-sem-olhar-id-zero`. |
| 6 | `idempotencia-do-evento-da-replica-e-pela-posicao-conferida_1200` | PENDENTE | **Sim.** A guarda `grupo-da-replica-sem-marca` deu PROVADA. Testes: `crates/phxsql-store/src/marca.rs::a_marca_do_grupo_nao_grava_por_cima_de_outra_historia` e `crates/phxsql-store/src/marca.rs::a_recuperacao_completa_o_grupo_da_replica_pela_posicao`. Commit: `550a1f2a`. | Não. Os dois erros de §2 foram corrigidos dentro da mesma frente. | Não | **FRUTÍFERO** com esses três itens. O buraco (o bidi sem marca) foi fechado pelo `9e067e1b` («o bidirecional ganha a mesma marca»). §5 envelheceu e pede uma linha dizendo isso. |
| 7 | `ja-aplicada-na-replica-deixa-o-diario-atras_1500` | PENDENTE | **Sim.** A guarda `replica-reaplica-inclusao-sem-olhar-o-reg` deu PROVADA. Teste: `crates/phxsql-server/tests/venda-inteira-na-queda-da-replica.rs::o_sigkill_entre_o_reg_e_o_diario_nao_duplica_a_linha`. Commit: `9e067e1b`. | Não | Não | **FRUTÍFERO.** O buraco da exclusão continua nomeado e não impede a promoção, porque a regra vale para a inclusão. |
| 8 | `latencia-de-chegada-misturada-com-a-janela-da-queda_0816` | PENDENTE | **Número reproduzível**, sem prova nos dois sentidos. `bancada/caixa-offline/resultados.json` tem `antes_pega_pela_queda`, e o `medir.py` o separa (`7faf2c7f`). É lição de método de medida, então não tem teste que caia. | **Sim, a hipótese.** «A trava atrasa o central» morreu medida: p95 de 10.719 ms → 1.015–1.025 ms quando separado. | Não | Pode ir a FRUTÍFERO com `**Evidência:** \`bancada/caixa-offline/resultados.json\`; \`7faf2c7f\``. Recomendo esperar uma segunda corrida que reproduza o 1.015–1.025, porque a corrida de 08:16 rodou com load alto. A hipótese morta merece entrada INFRUTÍFERO à parte. |
| 9 | `ordem-de-aplicacao-nao-e-ordem-do-id-da-marca_1700` | **Sem a linha** | **Não validada.** Os testes `crates/phxsql-server/tests/ordem-da-recuperacao.rs::o_commit_que_entrou_antes_do_grupo_e_completado_antes_dele` e `recuperacao-nao-volta-o-valor.rs::completar_a_marca_de_passada_terminada_nao_acrescenta_eventos` existem (`ba65032e`). Os dois RED foram medidos à mão, e nenhuma guarda os cita. | **Sim, duas mortes:** a E5 do desenho («por id através das famílias») e a conclusão «o F5 morreu». As duas trazem causa medida, mas sem os campos. | Não | Acrescentar `**Estado:** PENDENTE`. Abrir a guarda `marcas-pela-ordem-do-id` (a ordem `(nasce_fora_da_trava, id)` virando só `id`). Registrar a E5 como INFRUTÍFERO à parte, no avoid, para ela não voltar pelo desenho. |
| 10 | `prova-de-tls-exige-destino-que-recuse-o-noise_1925` | PENDENTE | **Parcial.** As guardas irmãs `remoto-em-claro-para-quem-exige` e `pulso-do-cluster-em-claro` deram PROVADA, mas cobrem o Remoto e o cluster. O caso que **fundou** o arquivo é o DbLink (`crates/phxsql-server/src/servidor/testes_dblink_cifra.rs::a_ligacao_phxsql_fala_tls_pelo_pino_tls_e_o_salvar_o_herda`, `ba65032e`), e ele não tem guarda. O próprio arquivo diz isso. | Não | Sim, em parte. Reafirma «teste que passa por engano», e o **alcance** novo é este: com dois caminhos de cifra, só o destino que recusa o velho prova o novo. O arquivo diz o caso, mas não o nomeia como alcance. | **PENDENTE.** Abrir a guarda `dblink-ignora-pino-tls` (`pino_tls()` → `None` em `dblink/phx.rs`). Acrescentar uma linha dizendo que o aprendizado é o alcance da pétrea do papel F. |
| 11 | `quando-a-chamada-sai-do-navegador-o-servidor-desliga-pelo-csp_1730` | PENDENTE | **Parcial.** Os testes `crates/phxsql-server/src/http.rs::desligada_pelo_administrador_a_pagina_nao_alcanca_a_anthropic` e `config.rs::integracao_claude_e_lida_e_nasce_ligada` existem (`ba65032e`). O 12/12 contra 9 vermelhos é da `testes-web/prova-339-chave.mjs`, que **continua fora** do `bateria.mjs` (0 menções). Nenhuma guarda. | Não. §2.1 é hipótese morta (`sessionStorage` passaria no ASVS), com a fonte citada. | Não | **PENDENTE.** É o caso mais fraco do lote: a prova do navegador só roda para quem a chamar. Primeiro pôr a `prova-339` na bateria, depois abrir a guarda (o CSP de volta com a Anthropic). |
| 12 | `regua-por-arquivo-muda-de-numero-quando-o-arquivo-se-divide_1300` | PENDENTE | **Sim, por número reproduzível.** `trecho-vivo.py --autoteste` dá «todos passaram». O conserto está no `a0325c80`, e a catraca foi aposentada e renasceu como `TETO_MENSAGEM_AMBIGUA_COM_O_SERVIDOR_INTEIRO` (pedido 718). | Não | **Sim.** Reafirma «a receita de um número também envelhece» (H) e «régua que muda aposenta a catraca» (G). O **alcance** está no arquivo («uma régua nunca tinha lido o `servidor.rs`»), mas não está dito como alcance. | Promover a FRUTÍFERO com `**Evidência:** \`a0325c80\``. §5 **envelheceu**: o «buraco continua» foi fechado pelo 718 (catraca aposentada e renascida em 130). Corrigir isso antes de promover. Acrescentar a linha «alcance da pétrea H/G». |
| 13 | `regua-que-corta-o-teste-precisa-ver-todo-cfg-test_1730` | PENDENTE | **Sim.** O caso «a cópia dentro de `#[cfg(all(test, unix))]` não conta» em `trecho-vivo.py --autoteste` passa. O número 137 → 131 é reproduzível. Commit: `ba65032e`. | Não | **Sim, mesma régua do #12.** É o segundo defeito do mesmo `producao()` no mesmo dia. | FRUTÍFERO com `**Evidência:** \`ba65032e\``. Pôr uma referência cruzada com o #12: os dois são o alcance da mesma régua de corte por texto, e ler um sem o outro repete o erro. O buraco `cfg(any(test…))` e o `cfg(test)` em item solto continua sem medir. |

## Contagem

- **FRUTÍFERO agora**, com guarda PROVADA ou número reproduzível e o commit: **6** (#1, #2, #6, #7, #12, #13).
- **PENDENTE, promovível ao abrir a guarda**: **6** (#3, #4, #5, #9, #10, #11). O #11 pede antes a `prova-339` na bateria.
- **Promovível com cautela**: **1** (#8). Falta uma segunda corrida da bancada.
- **Hipóteses mortas sem entrada INFRUTÍFERO**: 4, nos #4, #8, #9 e #11. Hoje elas não chegam ao `AVOID.md`, porque o extrator só lê o estado do arquivo.
- **Sem a linha `**Estado:**`**: 2 (#5, #9).
- **§5 envelhecida no mesmo dia**: 3 (#2, #6, #12).
- **Reafirmações de pétrea que deviam se dizer alcance**: 3 (#10, #12, #13).

## Lacuna da própria régua (recomendação, não código)

O `avoid-e-reuse.py --catraca` aceita como evidência um teste que **existe**, sem conferir
que ele tem guarda `PROVADA`. Isso deixaria passar justamente o «RED medido à mão» que esta
matriz recusa (H1). Se o integrador concordar, a referência de teste só deveria valer quando
há guarda no catálogo apontando para ela com `PROVADA` na última corrida. A decisão é do G (QA);
não precisa ir ao dono.
