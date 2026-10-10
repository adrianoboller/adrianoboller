# REUSE -- o que ja deu certo aqui, comprovado

<!-- GERADO por docs/cognicao/avoid-e-reuse.py -- NAO EDITE A MAO.
     `--catraca` reprova se este arquivo nao bater com o que o extrator
     geraria agora; rode o comando sem flag para atualizar. -->

Gerado dos `cognicao_*.md` com `**Estado:** FRUTIFERO` -- 18 hoje, de 472 cognicoes no total.

## A calculada é a coluna protegida por outro nome — e o preenchimento em lote promove o vazamento de linha a vazamento de tabela

- Evidencia: `crates/phxsql-store/tests/cifra-dos-dados.rs::calculada_sobre_externo_selado_nasce_marcada_e_nao_vaza_no_reg`; `crate::servidor::testes_direito_por_coluna::calculada_que_cita_coluna_negada_e_recusada_na_declaracao`; `crate::servidor::testes_direito_por_coluna::calculada_derivada_de_coluna_negada_nao_se_le`; `crates/phxsql-store/tests/acrescentar-coluna.rs::a_recusa_da_calculada_sobre_coluna_marcada_nao_diz_a_linha`
- Arquivo: [cognicao_calculada-e-a-coluna-protegida-por-outro-nome_20261001_1930.md](cognicao_calculada-e-a-coluna-protegida-por-outro-nome_20261001_1930.md)

## Camada nova antes da marca esconde a rede velha: a guarda de uma camada passa por engano

- Evidencia: `crates/phxsql-server/tests/commit-pelo-soquete.rs::o_commit_contra_a_tabela_congelada_nao_sai_pela_metade`; `ba65032e`
- Arquivo: [cognicao_camada-nova-antes-da-marca-esconde-a-rede-velha_20261008_1650.md](cognicao_camada-nova-antes-da-marca-esconde-a-rede-velha_20261008_1650.md)

## O carimbo do arquivo faz o papel do catálogo reverso sem cobrar de ninguém — e o medidor do diretório recém-criado não o vê

- Evidencia: `crates/phxsql-store/src/irmas.rs::a_segunda_exclusao_nao_rele_o_esquema_das_irmas`; `crates/phxsql-store/src/irmas.rs::a_chave_declarada_depois_tranca_o_pai`; `crates/phxsql-store/src/irmas.rs::carimbo_recente_nao_e_confiavel_e_velho_e`
- Arquivo: [cognicao_carimbo-de-arquivo-no-lugar-do-catalogo-reverso_20261001_1758.md](cognicao_carimbo-de-arquivo-no-lugar-do-catalogo-reverso_20261001_1758.md)

## Chave por caminho não segue o `rename`: mudar o nome nas sujas não bastava

- Evidencia: `crates/phxsql-server/src/servidor.rs::tabela_excluida_ou_renomeada_na_janela_nao_segura_as_marcas` falha com `mudar_pendentes_de_nome` desligado (`familias_devendo_em` = 12) e passa com ele ligado; guarda `renomear-deixa-o-registro-no-nome-velho` em `bancada/guardas/catalogo.py`, provada pelo `provar-guardas.py --so`. Pedido 536, 30/09/2026.
- Arquivo: [cognicao_chave-por-caminho-nao-segue-o-rename_20260930_1930.md](cognicao_chave-por-caminho-nao-segue-o-rename_20260930_1930.md)

## Congelar a filha não basta: a varredura da chave congela a MÃE também

- Evidencia: `crates/phxsql-server/src/servidor.rs::a_janela_da_varredura_solta_a_vizinha_e_segura_filha_e_mae`
- Arquivo: [cognicao_congelar-a-filha-nao-basta-a-mae-tambem_20260929_1600.md](cognicao_congelar-a-filha-nao-basta-a-mae-tambem_20260929_1600.md)

## A conta que a origem faz para recusar tem de ser um TETO da conta que a réplica faz para partir

- Evidencia: `crates/phxsql-server/tests/transacao-acima-do-teto.rs::a_transacao_acima_do_teto_e_recusada_no_commit_e_a_que_cabe_chega_inteira`; `4b388e62`
- Arquivo: [cognicao_conta-da-origem-tem-de-ser-teto-da-conta-da-replica_20261008_0100.md](cognicao_conta-da-origem-tem-de-ser-teto-da-conta-da-replica_20261008_0100.md)

## Corpo de falso positivo tirado de uma fonte só mede essa fonte

- Evidencia: `crates/phxsql-sql/src/sintaxe.rs::comando_empilhado_nao_acusa_o_legitimo`; `crates/phxsql-sql/src/sintaxe.rs::comando_empilhado_acha_o_segundo_comando`; `bancada/seguranca/495/prova_215.py`. Validada pelo integrador, que não é o autor do conserto, em 24/09/2026, na árvore exata. Com o defeito ORIGINAL reposto, o teste do legítimo cai (`nao devia acusar`, `sintaxe.rs:2079`). Com a 1a versão do conserto, o do ataque cai (`devia acusar`, `sintaxe.rs:2035`). Com o conserto final, os dois passam (2/2).
- Arquivo: [cognicao_corpo-de-falso-positivo-de-uma-fonte-so-mede-essa-fonte_20260924_1340.md](cognicao_corpo-de-falso-positivo-de-uma-fonte-so-mede-essa-fonte_20260924_1340.md)

## O `eprintln!` não é uma escrita só: a linha lida de outro processo só vale com o `\n`

- Evidencia: `crates/phxsql-server/tests/porta-lida-pela-metade.rs::o_endereco_pela_metade_espera_o_resto`; `crates/phxsql-server/tests/porta-lida-pela-metade.rs::a_porta_pela_metade_nao_vira_outra_porta`
- Arquivo: [cognicao_eprintln-nao-e-uma-escrita-so-a-linha-so-vale-com-o-fim_20260930_2017.md](cognicao_eprintln-nao-e-uma-escrita-so-a-linha-so-vale-com-o-fim_20260930_2017.md)

## A idempotência do evento da réplica é pela POSIÇÃO do diário, conferida — e a recusa tem de parar o resto

- Evidencia: `crates/phxsql-store/src/marca.rs::a_marca_do_grupo_nao_grava_por_cima_de_outra_historia`; `crates/phxsql-store/src/marca.rs::a_recuperacao_completa_o_grupo_da_replica_pela_posicao`; `550a1f2a`
- Arquivo: [cognicao_idempotencia-do-evento-da-replica-e-pela-posicao-conferida_20261008_1200.md](cognicao_idempotencia-do-evento-da-replica-e-pela-posicao-conferida_20261008_1200.md)

## «Já aplicada» na réplica deixa o diário um evento atrás

- Evidencia: `crates/phxsql-server/tests/venda-inteira-na-queda-da-replica.rs::o_sigkill_entre_o_reg_e_o_diario_nao_duplica_a_linha`; `9e067e1b`
- Arquivo: [cognicao_ja-aplicada-na-replica-deixa-o-diario-atras_20261008_1500.md](cognicao_ja-aplicada-na-replica-deixa-o-diario-atras_20261008_1500.md)

## Motor com prazo não cabe no lançador de uma linha: o binário se reexecuta

- Evidencia: `crates/phxsql-server/tests/queda-nao-prende-a-trava.rs::o_filho_do_gancho_nasce_do_lancador_com_as_garantias_do_motor`; `crates/phxsql-server/tests/queda-nao-prende-a-trava.rs::o_sigkill_com_o_gancho_ligado_nao_deixa_a_trava_no_filho_do_gancho`
- Arquivo: [cognicao_motor-com-prazo-nao-cabe-no-lancador-de-uma-linha-o-binario-se-reexecuta_20261009_0950.md](cognicao_motor-com-prazo-nao-cabe-no-lancador-de-uma-linha-o-binario-se-reexecuta_20261009_0950.md)

## O terceiro da tabela que nasce se mede com `strace`, sem derrubar nada

- Evidencia: `crates/phxsql-server/src/servidor.rs::o_terceiro_so_ouve_ok_depois_do_fsync_da_pasta_e_espera_fora_da_trava`; `crates/phxsql-store/src/catalogo.rs::a_tabela_que_nasce_so_abre_para_outro_depois_do_fsync`; `bancada/durabilidade/terceiro-605.py`
- Arquivo: [cognicao_o-terceiro-da-tabela-que-nasce-se-mede-com-strace-sem-derrubar-nada_20261001_1720.md](cognicao_o-terceiro-da-tabela-que-nasce-se-mede-com-strace-sem-derrubar-nada_20261001_1720.md)

## Prova diferencial contra o `HEAD` expira no proprio commit

- Evidencia: `42e1bf3`; `docs/dossie/prova-do-depois-da-versao.py`
- Arquivo: [cognicao_prova-contra-o-head-expira-no-proprio-commit_20260924_0955.md](cognicao_prova-contra-o-head-expira-no-proprio-commit_20260924_0955.md)

## Recusa simétrica de sufixo de balde não é «só a letra 1»

- Evidencia: `crates/phxsql-store/src/catalogo.rs::criar_recusa_sufixo_de_letra_da_particao_sem_perguntar_ao_disco`; `crates/phxsql-store/src/catalogo.rs::criar_recusa_ponto_no_nome_por_colidir_com_o_qualificado`. Validada pelo integrador, que nao e o autor do conserto, em 24/09/2026 na arvore exata: com as duas guardas novas de `exigir_nome_que_volta` neutralizadas, os dois testes caem (`catalogo.rs:2281` e `:2321`); com o conserto, os tres passam.
- Arquivo: [cognicao_recusa-simetrica-de-sufixo-nao-e-so-a-letra-1_20260924_1438.md](cognicao_recusa-simetrica-de-sufixo-nao-e-so-a-letra-1_20260924_1438.md)

## Régua que lê «o arquivo» muda de número quando o arquivo se divide — e uma delas nunca tinha lido o `servidor.rs`

- Evidencia: `a0325c80`
- Arquivo: [cognicao_regua-por-arquivo-muda-de-numero-quando-o-arquivo-se-divide_20261008_1300.md](cognicao_regua-por-arquivo-muda-de-numero-quando-o-arquivo-se-divide_20261008_1300.md)

## A régua que separa produção de teste precisa ver todo `cfg(test)`, não só o literal

- Evidencia: `ba65032e`
- Arquivo: [cognicao_regua-que-corta-o-teste-precisa-ver-todo-cfg-test_20261008_1730.md](cognicao_regua-que-corta-o-teste-precisa-ver-todo-cfg-test_20261008_1730.md)

## Aviso de corte por teto: 01000, nunca 01004 -- e onde ele mora sozinho

- Evidencia: `crates/phxsql-odbc/src/lib.rs::sql_truncado_pelo_teto_avisa_01000`; `crates/phxsql-odbc/src/lib.rs::sql_sem_truncado_continua_sql_success_puro`; `testes-web/prova-truncado-sql.mjs` (rodada em 24/09/2026: 3/3 passos verdes com o conserto, 2/3 com a leitura de `r.truncado` removida do `claude.js` -- prova nos dois sentidos, pelo navegador, contra o `phxsqld` de verdade).
- Arquivo: [cognicao_sqlstate-do-truncado-em-composicao_20260924_1308.md](cognicao_sqlstate-do-truncado-em-composicao_20260924_1308.md)

## TLS: confira o tipo do registro ANTES do tamanho

- Evidencia: `crates/phxsql-core/src/tls.rs::http_em_claro_recebe_o_alerta_na_hora` e `crates/phxsql-server/tests/tls-das-portas-http.rs::a_porta_tls_nao_responde_em_claro` — com a conferência do cabeçalho tirada, os dois ficam vermelhos (mutante `sem-conferir-cabecalho`, 30/09/2026).
- Arquivo: [cognicao_tls-o-tipo-antes-do-tamanho_20260930_1500.md](cognicao_tls-o-tipo-antes-do-tamanho_20260930_1500.md)
