# Inventário QA — guarda × pétrea, replicação/cluster/quórum (17/09/2026)

Papel G (QA), ordem do dono de 17/09/2026 02:27 UTC («Replicação: bateria de
testes, revisão e conclusão»). Frente **estática**: nenhum `cargo`, nenhum
`provar-guardas.py`, nenhum servidor subido — só leitura do catálogo, do
último veredito gravado e das réguas estáticas que já existem, para não
disputar o `flock` nem os soquetes com a bancada do papel F, que corria em
paralelo. Todas as medições estáticas abaixo têm hora (UTC) do momento em que
rodei o comando nesta sessão.

Fontes lidas: `docs/CATRACAS.md` §12 (as sete réguas do catálogo) e §15 (o
catálogo contra as pétreas), `docs/QA-PDCA.md`, `docs/REPLICACAO.md`,
`docs/CLUSTER.md`, `docs/INTEGRIDADE.md` §3, `bancada/guardas/catalogo.py`
(180 entradas, lidas com `python3 -c "import catalogo"`, sem compilar nada) e
`bancada/guardas/ultima-corrida.json`.

---

## 1. As entradas do catálogo que tocam replicação, cluster ou quórum

Treze das 180 entradas de `bancada/guardas/catalogo.py` reproem defeito em
replicação/cluster (varredura por `id`, `arquivo` e conteúdo do `porque`
contra `replica`, `cluster`, `pulso`, `posicao`, `bidi`, `colisao`,
`credencial`, `trava-atras-da-rede`). Veredito da **última corrida do
provador, 2026-09-16 15:25** (`bancada/guardas/ultima-corrida.json`) — nenhuma
delas foi reproduzida nesta frente, que é estática por ordem do dono.

| id | o defeito que motivou (`porque`, resumido) | `caem` | pacote / alvo | veredito (16/09 15:25) |
|---|---|---|---|---|
| `posicao-sem-portao` | `posicao` (o `SHOW MASTER STATUS` daqui) entregava eventos e o esquema cru de toda tabela sem a conferência própria de `Atividade::Replicar` — `docs/TESTES.md` 3.3 | `servidor::testes_direito_por_tabela::posicao_esconde_a_tabela_negada` | `phxsql-server` `--lib` | **PROVADA** (37,96 s) |
| `trava-atras-da-rede` | o laço da réplica segurava a trava global de dados enquanto lia do soquete — `ping` 4 ms, `varrer` 30.079 ms; REPLICACAO.md §18 | `source_mudo_nao_prende_a_trava_de_dados` | `phxsql-server` `--test trava-atras-da-rede` (prazo 120 s) | **PROVADA** (17,15 s) |
| `pulso-do-cluster-em-claro` | pulso da eleição saindo em claro com `cluster.cifra: true` ligada — metade do tráfego cifrada engana mais que nenhuma | `pulso_do_cluster_cifrado_atravessa_no_que_exige_tunel` | `phxsql-server` `--test cluster-cifrado` | **PROVADA** (20,36 s) |
| `replicacao-do-cluster-em-claro` | a outra metade do defeito acima: a replicação entre nós do cluster saindo em claro | `servidor::testes_config_gravar::origem_do_cluster_carrega_a_cifra_e_o_pino` | `phxsql-server` `--lib` | **PROVADA** (38,33 s) |
| `replica-julga-fk` | a réplica voltava a conferir FK no evento que aplica — sem ordem global entre tabelas, isso é **perda de dado**: `pedidos` ficava com 0 de 2 eventos nas ordens mãe-primeiro e filha-primeiro | `a_replica_converge_nas_tres_ordens_de_tabela`, `a_marca_de_replica_nao_vaza_para_a_escrita_local` | `phxsql-store` `--test replicacao-integridade` | **PROVADA** (1,71 s) |
| `cascata-sem-imagem-no-diario` | a filha que a cascata do `ao_alterar` abre nascia sem imagem no diário, e a réplica recusava o evento com «veio sem imagem» nas três ordens | `o_evento_da_cascata_carrega_a_imagem_da_linha`, `a_replica_converge_nas_tres_ordens_de_tabela` | `phxsql-store` `--test replicacao-integridade` | **PROVADA** (1,67 s) |
| `replica-refaz-a-cascata` | a réplica voltava a refazer a cascata que o source já tinha mandado — grava a filha duas vezes, diverge no diário | `a_replica_converge_nas_tres_ordens_de_tabela` | `phxsql-store` `--test replicacao-integridade` | **PROVADA** (1,68 s) |
| `marca-de-replica-fica-acesa` | a marca `como_replica` não se apagava na volta do `aplicar_evento` — o handle deixaria de conferir integridade na escrita LOCAL seguinte | `a_marca_de_replica_nao_vaza_para_a_escrita_local` | `phxsql-store` `--test replicacao-integridade` | **PROVADA** (1,54 s) |
| `bidirecional-julga-fk` | o bidirecional não passa pelo `aplicar_evento` (casa por chave, não rowid) e caía no mesmo buraco com consequência pior: o erro sobe pelo `?`, `desde` nunca anda, o mesmo lote volta para sempre — **o par de servidores para**, não é uma linha só | `o_bidirecional_aceita_a_filha_que_chega_antes_da_mae` | `phxsql-store` `--test bidirecional-no-store` | **PROVADA** (1,49 s) |
| `bidirecional-julga-as-filhas` | idem, do lado do excluir: recusar a mãe cuja filha já saiu (em outra ordem legítima) travaria o par pela mesma razão | `o_bidirecional_apaga_a_mae_cuja_filha_ainda_nao_saiu` | `phxsql-store` `--test bidirecional-no-store` | **PROVADA** (1,38 s) |
| `colisao-de-sequence-calada` | dois masters na mesma faixa de sequência perdiam linha **sem contar a ninguém** — `docs/AUTONUMBER.md` bloco 24: 4 inserções viraram 2 linhas | `bidirecional::testes::inclusao_de_outra_origem_sobre_chave_viva_e_colisao` | `phxsql-server` `--lib` | **PROVADA** (33,50 s) |
| `replica-insiste-na-credencial-recusada` | a réplica com credencial recusada insistia a cada `reconectar_em` e bloqueava o próprio IP — pedido 203, filmado 07/09, medido pelo soquete 09/09: 75 tentativas/min, master bloqueou em 4 s por 60 min, e o login do operador caiu junto | `replica::testes_do_ritmo::credencial_recusada_estaciona_na_primeira` | `phxsql-server` `--lib` | **PROVADA** (32,34 s) |
| `cluster-devolve-a-credencial-na-tela` | o resumo do cluster na op `config` levava o `token` e o `senha_hash` do replicador — a mesma família da `ficha-do-usuario-devolve-o-hash`, numa seção onde ninguém esperaria | `config::tests::a_credencial_do_cluster_nao_sai_em_json`, `config::tests::nenhuma_credencial_do_config_sai_pela_op_config` | `phxsql-server` `--lib` | **NÃO JULGADA** — entrada nasceu em 17/09/2026 ("RAIO MEDIDO (17/09/2026)" no próprio `porque`), depois da última corrida (16/09 15:25). Nomeada, não escondida: o `trecho-vivo.py --catraca` desta sessão (§3) confirma `TETO_NAO_JULGADA_ESCONDIDA: 0` — as 37 entradas sem veredito, incluindo esta, estão nomeadas na tabela publicada |

Doze de treze estão **PROVADAS**; a treze, mais nova que a última corrida,
está **corretamente nomeada como não julgada** — é exatamente o caso que a
sexta régua do §12.6 do `CATRACAS.md` (`TETO_NAO_JULGADA_ESCONDIDA`) existe
para não deixar sumir calada.

---

## 2. Pétrea × guarda — o que cobre, e o que não cobre

### 2.1 Cobertas

| pétrea | guarda(s) | veredito |
|---|---|---|
| «a réplica **aplica**, ela não **julga**» (INTEGRIDADE §3) | `replica-julga-fk`, `cascata-sem-imagem-no-diario`, `replica-refaz-a-cascata`, `marca-de-replica-fica-acesa`, `bidirecional-julga-fk`, `bidirecional-julga-as-filhas` — **6 guardas** | todas PROVADAS 16/09 15:25 |
| «nenhuma leitura de rede acontece com a trava de dados na mão» (REPLICACAO §18) | `trava-atras-da-rede` (catálogo) **+** a catraca `rede-ou-espera` do `mapa-da-trava.py` | guarda PROVADA 16/09 15:25 (17,15 s); catraca medida **0 (teto 0)** nesta sessão, 17/09 02:34 UTC (§3) |
| «credencial recusada estaciona» (pedido 203) | `replica-insiste-na-credencial-recusada` | PROVADA 16/09 15:25 (32,34 s) |
| «cluster inteiro cifrado» (H1 — cifrar só metade é pior que nenhuma) | `pulso-do-cluster-em-claro` + `replicacao-do-cluster-em-claro` | ambas PROVADAS 16/09 15:25 |

### 2.2 SEM GUARDA no catálogo — nomeadas

Para cada uma: o teste real que já existe em `crates/` (a maioria com prova
real documentada, "nos dois sentidos", no próprio comentário do fonte) e por
que ele nunca vira uma reprova periódica: **nenhum é chamado por
`bancada/guardas/catalogo.py`**, então o `provar-guardas.py` nunca repõe o
defeito e confere se ele ainda cai. Confirmado por três buscas independentes:
grep de `spare`/`read_replica`/`SPARE_EM_ESPERA`/`ESCRITA_NA_REPLICA` no
catálogo (zero), grep de `replicas_autorizadas`/`cluster_no_acrescentar`/
`incompleta` no catálogo (zero), e grep de `"arquivo": ".../cluster.rs"` no
catálogo (**zero entradas — nenhuma das 180 toca `crates/phxsql-server/src/cluster.rs`**).

| pétrea | teste real que existe hoje (não catalogado) | o que a guarda precisaria repor | qual teste cairia |
|---|---|---|---|
| «guarda nova entra pedida, não imposta» — `replicas_autorizadas` vazia libera (REPLICACAO §7, portão 2a-bis) | `sem_replicas_autorizadas_nada_muda`, `replica_de_fora_da_lista_nao_le_o_diario`, `caminho_interno_sem_ip_nao_e_barrado_pela_lista`, `a_replicacao_aberta_se_anuncia_e_some_quando_a_lista_enche` (`crates/phxsql-server/src/servidor.rs`, ~linha 24830 em diante) | tirar a conferência do portão 2a-bis (voltar a aceitar qualquer IP com token) | `replica_de_fora_da_lista_nao_le_o_diario` cairia na hora; e sem o `sem_replicas_autorizadas_nada_muda` continuar passando, a mesma troca poderia ter fechado a lista por padrão sem ninguém perceber a quebra do comportamento velho |
| `incompleta:false` por omissão — «a posição não encolhe em silêncio» (pedido 211) | `tabela_que_nao_abre_nao_pode_encolher_a_posicao_em_silencio` (`servidor.rs`, `mod testes_posicao_do_diario`) — nasceu VERMELHA 05/09, virou VERDE 10/09 (CATRACAS.md §7) | fazer `posicao_do_diario` engolir o erro de abrir e devolver a posição encolhida sem marcar `incompleta` | o teste cairia dizendo que a posição voltou menor sem a bandeira, exatamente como fazia antes do pedido 211 |
| a eleição prefere completa (pedido 211) | `eleicao_prefere_completa_a_incompleta` (`crates/phxsql-server/src/cluster.rs:677`) | fazer `cluster::vencedor` comparar só a posição numérica, ignorando `incompleta` | cairia elegendo o nó incompleto quando há um completo com posição menor |
| «réplica não atende escrita» — portão 2b-bis (REPLICACAO §6) | `aplicar_num_source_trancado_por_administracao_e_recusado` **+** o teste do comportamento velho `replica_trancada_continua_aceitando_o_diario_do_source` (`servidor.rs`) | tirar o `!recebe_replicacao` do portão de `aplicar` | um Source em `somente_leitura` voltaria a aceitar `aplicar` pela rede — o furo medido pela bateria `porta.py` caso 4d-ii (07/09/2026) |
| `spare` não atende ninguém (modo C) | `spare_nao_atende_cliente_nem_de_leitura`, `spare_promover_vira_primario_e_abre_a_escrita` (`servidor.rs`) | tirar `SPARE_EM_ESPERA` da lista de recusa do spare | `spare_nao_atende_cliente_nem_de_leitura` cairia aceitando `varrer`/`inserir` num nó que deveria ficar mudo até a promoção manual |
| read replica recusa escrita apontando o master (modo D) | `read_replica_recusa_escrita_apontando_o_primario_e_serve_leitura` (`servidor.rs`) | fazer o `read_replica` aceitar `inserir` em vez de devolver `REDIRECIONA`/4003 com o endereço do primário | cairia aceitando escrita numa réplica de leitura, ou perdendo o endereço do primário na mensagem |
| «o pulso de id fora da lista é recusado» **e** «nó novo não precisa reiniciar» (pedido 217) | **um único teste prova as duas pontas**: `no_acrescentado_a_quente_passa_a_ser_aceito_no_pulso` (`servidor.rs:31756`) — pulso de `no3` desconhecido recusado (`ACESSO_NEGADO`), `cluster_no_acrescentar` aplica a quente, o mesmo pulso passa sem reiniciar nada | tirar a conferência de `id` contra a lista viva no `op_cluster_pulso`, ou fazer `cluster_no_acrescentar` exigir reinício | cairia ou aceitando o pulso do nó fantasma desde o início, ou falhando ao aceitar o pulso depois do acréscimo a quente |
| `TETO_DO_LOTE_SERVIDO` 16 MiB e `TETO_DA_RESPOSTA` 128 MiB (pedido 147, REPLICACAO §18) | **nenhum teste encontrado** para `TETO_DO_LOTE_SERVIDO` (grep da constante em `crates/` e `bancada/` devolve só a declaração e o próprio uso, `servidor.rs:567`/`21798`/`21802` — zero ocorrência em teste). `TETO_DA_RESPOSTA` é o teto genérico do fio (`TETO_DO_REGISTRO`, `crates/phxsql-core/src/fio.rs:495`, 128 MiB), exercido por `bancada/seguranca/porta.py` (linha GIGANTE contra `ping`) — uma bancada de **servidor vivo**, fora do alcance estático do catálogo | escrever um teste que sirva um lote com uma linha maior que 16 MiB e confira que ele é cortado (com o primeiro evento entrando sempre) — hoje não há prova nenhuma disso, nem em unidade nem em bancada | sem teste, não há o que cair — este é o buraco mais cru dos treze: não é "sem guarda no catálogo", é **sem prova alguma** do corte por bytes do lado do source |

**O achado que atravessa a lista inteira**: `crates/phxsql-server/src/cluster.rs`
tem, hoje, **zero entradas em `bancada/guardas/catalogo.py`** — confirmado por
`grep '"arquivo": ".*cluster.rs"' bancada/guardas/catalogo.py`, zero
resultados —, apesar de ser o arquivo com a eleição (pedido 211), o
escalonamento a quente (pedido 217) e os quatro modos A–D (pedido 214), todos
com testes reais e comentados como prova. O catálogo tem, ao todo, **uma**
entrada em `replica.rs` e nenhuma em `cluster.rs`; as outras doze moram em
`servidor.rs` (dispatch) ou `table.rs`/`bidirecional.rs` (store). O alcance da
pétrea "cada guarda catalogada" descrita no `CLAUDE.md` para a permissão por
tabela (§15.4 do CATRACAS.md) se repete aqui por outro caminho: **um arquivo
inteiro nunca entrou no catálogo**, não uma operação isolada.

### 2.3 Fora de escopo de guarda, por decisão já registrada

- **Transação com quórum** (REPLICACAO §19, CLUSTER.md §2.4): `quorum_minimo`
  está no formato e a op `config` devolve `"quorum_imposto": false` — é
  **pesquisa/plano** (papel J, `docs/propostas/quorum-de-escrita.md`), ainda
  não implementado. Não há pétrea para guardar aqui: guardar "o campo existe e
  não faz nada" não protege coisa nenhuma até o dono decidir a via.
- **`alcancam-fsync`**: catraca **vermelha por decisão do dono** (pendência
  #252, medido 23 contra teto 22) — não é gap de guarda, é dívida já
  registrada e presa ao motivo escrito no `CATRACAS.md` §13.

---

## 3. Catracas de replicação/concorrência — medidas agora

Rodadas nesta sessão, estáticas (sem `cargo`, sem servidor), horário UTC:

| catraca | medidor | teto | medido (17/09/2026, hora) | estado |
|---|---|---:|---:|---|
| `TETO_TRECHO_MORTO` | `trecho-vivo.py --catraca` | 0 | **0** (02:34) | ok |
| `TETO_TRECHO_AMBIGUO` | idem | 0 | **0** (02:34) | ok |
| `TETO_TESTE_MORTO` | idem | 0 | **0** (02:34) | ok |
| `TETO_TESTE_FORA_DO_BINARIO` | idem | 0 | **0** (02:34) | ok |
| `TETO_TESTE_SEM_MODULO` | idem | 0 | **0** (02:34) | ok |
| `TETO_NAO_JULGADA_ESCONDIDA` | idem | 0 | **0** (02:34) — 37 de 180 sem veredito na corrida de 16/09 15:25, **0 escondidas** (todas nomeadas, inclusive `cluster-devolve-a-credencial-na-tela`, §1) | ok |
| `PISO_DAS_ENTRADAS` (piso, só sobe) | idem | — | **180** (02:34) | catálogo cresceu desde os 177 do último retrato em `CATRACAS.md` §12.3 (17/09, antes desta frente) — piso sobe junto, não é violação |
| `codigo-do-dono` | `mapa-da-trava.py --catraca` | 5 | **5** (02:34) | sem folga |
| `alcancam-fsync` | idem | 22 | **23** (02:34) | **VERMELHA**, decisão do dono, pendência #252 — não mexida por esta frente |
| **`rede-ou-espera`** | idem | 0 | **0** (02:34) — a catraca que guarda REPLICACAO §18 | **ok**, sem folga |
| `spawn-sem-teto` | `mapa-das-threads.py --catraca` | 0 | **0** (02:34) | ok |
| `catalogo-envelhecido` | idem | 0 | **0** (02:34) | ok |
| `TETO_PKILL_SEM_PID` | `pkill-sem-pid.py --catraca` | 0 | **0** (02:34) | ok |

**O que falta como catraca, e não existe hoje**: não há régua numérica
nenhuma para `TETO_DO_LOTE_SERVIDO`/`TETO_DA_RESPOSTA` (§2.2, última linha) —
nem catraca de qualidade nem limite de funcionamento com teste. E não há
catraca contando quantas entradas do catálogo tocam cada arquivo de
`servidor.rs`/`cluster.rs`/`replica.rs`, que é o que teria acusado o zero de
`cluster.rs` sem depender de alguém vir procurar à mão, como fiz aqui.

Nenhuma catraca subiria indevidamente com a mudança que está em revisão
(bateria/revisão de replicação, papel F): as réguas dos §12/§13 do
`CATRACAS.md` são de **código-fonte e catálogo**, não de resultado de
bancada, e nada nesta frente mexeu em `crates/` ou em `bancada/guardas/`. A
única catraca que se moveria com a chegada de guardas novas de replicação é
`PISO_DAS_ENTRADAS` — e ela **sobe**, o que é o comportamento correto (§12.2
do `CATRACAS.md`): um piso que não sobe junto com o catálogo voltaria a
aceitar apagamento silencioso das entradas novas.

---

## 4. Os `--so` para A rodar depois da bateria — em ordem de valor

Perda de dado primeiro, depois parada total do par, depois confidencialidade,
depois indisponibilidade operacional. Tempo estimado = `segundos` da última
corrida (16/09 15:25) para a mesma entrada; onde não há corrida, uso a faixa
de entradas do mesmo pacote/alvo.

```
python3 bancada/guardas/provar-guardas.py --so replica-julga-fk                       # ~1,7 s  — perda de dado medida (pedidos 0/2 eventos)
python3 bancada/guardas/provar-guardas.py --so cascata-sem-imagem-no-diario            # ~1,7 s  — evento sem imagem = recusado = perdido
python3 bancada/guardas/provar-guardas.py --so colisao-de-sequence-calada              # ~33,5 s — 4 inserções viravam 2 linhas, caladas
python3 bancada/guardas/provar-guardas.py --so replica-refaz-a-cascata                 # ~1,7 s  — filha gravada 2x, diário diverge
python3 bancada/guardas/provar-guardas.py --so bidirecional-julga-fk                   # ~1,5 s  — par de servidores para PARA SEMPRE
python3 bancada/guardas/provar-guardas.py --so bidirecional-julga-as-filhas            # ~1,4 s  — idem, do lado do excluir
python3 bancada/guardas/provar-guardas.py --so marca-de-replica-fica-acesa             # ~1,5 s  — portão de integridade apagado sozinho
python3 bancada/guardas/provar-guardas.py --so trava-atras-da-rede                     # ~17,2 s — disponibilidade (30 s travado), prazo 120 s no catálogo
python3 bancada/guardas/provar-guardas.py --so pulso-do-cluster-em-claro               # ~20,4 s — confidencialidade do cluster
python3 bancada/guardas/provar-guardas.py --so replicacao-do-cluster-em-claro          # ~38,3 s — confidencialidade da replicação
python3 bancada/guardas/provar-guardas.py --so posicao-sem-portao                      # ~38,0 s — vazamento de esquema/eventos de tabela negada
python3 bancada/guardas/provar-guardas.py --so replica-insiste-na-credencial-recusada  # ~32,3 s — autobloqueio operacional (indisponibilidade auto-infligida)
python3 bancada/guardas/provar-guardas.py --so cluster-devolve-a-credencial-na-tela    # sem estimativa (NAO JULGADA); entradas irmãs (mesmo pacote/alvo, --lib) levam 20-38 s -- rodar para tirá-la do estado "nao julgada" antes que fique escondida numa proxima corrida parcial
```

Soma aproximada das doze já medidas: **~189 s** (~3,2 min); a treze ainda sem
registro deve ficar na mesma ordem de grandeza (`phxsql-server --lib`, como as
outras seis desse pacote/alvo, 20-38 s cada). Bem abaixo da hora que o
provador completo leva (§12 do `CATRACAS.md`), porque `--so` pula a
recompilação e a mutação das outras 167 entradas.

---

## 5. O que este inventário NÃO viu

- **A corrida do provador nas 13 entradas.** Só li o veredito da última
  corrida registrada (16/09 15:25); não reproduzi defeito nenhum nesta
  frente, por ordem explícita ("NÃO rode `cargo`, NÃO rode
  `provar-guardas.py`"). A treze-ésima entrada (`cluster-devolve-a-credencial-
  na-tela`) segue **não julgada** até A rodar o `--so` da §4.
- **A bateria de contêiner** (`docs/REPLICACAO.md` §17) e a bancada de
  quórum/cluster com processos de verdade (`bancada/cluster/`,
  `bancada/replicacao/`, `bancada/quorum/`) — são exatamente o que o papel F
  está rodando em paralelo agora; este inventário é sobre o **catálogo**, não
  sobre o resultado da bancada de hoje.
- **Se os sete testes "sem guarda" nomeados na §2.2 ainda passam.** Eles
  existem no fonte e têm prova real documentada no próprio comentário
  (`servidor.rs`, `cluster.rs`), mas essa prova foi feita **quando o pedido
  foi fechado** (203/211/214/217) — sem entrar no catálogo, ninguém os
  reproduz de novo a cada rodada. Só `cargo test --workspace` (que a
  bateria de F já roda) confirma que continuam verdes hoje.
- **O corte por bytes de `TETO_DO_LOTE_SERVIDO`.** Não achei prova nenhuma —
  nem no catálogo, nem em teste de unidade avulso, nem em bancada. Não posso
  afirmar que o defeito documentado em REPLICACAO.md §18 ("o primeiro evento
  entra sempre, senão uma linha maior que o teto pararia a replicação para
  sempre") está coberto por prova real nos dois sentidos — só posso afirmar
  que não achei a prova, com as buscas descritas na §2.2.
- **Se as 37 entradas "não julgadas" da corrida de 16/09 continuam
  compilando** — isso só o próprio `provar-guardas.py` (ou um `cargo test
  --workspace`) confirma; a régua estática (§3) só sabe dizer que elas estão
  nomeadas, não que ainda passam.

---

## Arquivos

- Este inventário: `docs/propostas/inventario-qa-replicacao-2026-09-17.md`.
- Cognição nova (o alcance descoberto — `cluster.rs` fora do catálogo):
  `docs/cognicao/cognicao_cluster-fora-do-catalogo-de-guardas_20260917_0234.md`.
