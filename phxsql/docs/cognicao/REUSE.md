# REUSE -- o que ja deu certo aqui, comprovado

<!-- GERADO por docs/cognicao/avoid-e-reuse.py -- NAO EDITE A MAO.
     `--catraca` reprova se este arquivo nao bater com o que o extrator
     geraria agora; rode o comando sem flag para atualizar. -->

Gerado dos `cognicao_*.md` com `**Estado:** FRUTIFERO` -- 4 hoje, de 334 cognicoes no total.

## Corpo de falso positivo tirado de uma fonte só mede essa fonte

- Evidencia: `crates/phxsql-sql/src/sintaxe.rs::comando_empilhado_nao_acusa_o_legitimo`; `crates/phxsql-sql/src/sintaxe.rs::comando_empilhado_acha_o_segundo_comando`; `bancada/seguranca/495/prova_215.py`. Validada pelo integrador, que não é o autor do conserto, em 24/09/2026, na árvore exata. Com o defeito ORIGINAL reposto, o teste do legítimo cai (`nao devia acusar`, `sintaxe.rs:2079`). Com a 1a versão do conserto, o do ataque cai (`devia acusar`, `sintaxe.rs:2035`). Com o conserto final, os dois passam (2/2).
- Arquivo: [cognicao_corpo-de-falso-positivo-de-uma-fonte-so-mede-essa-fonte_20260924_1340.md](cognicao_corpo-de-falso-positivo-de-uma-fonte-so-mede-essa-fonte_20260924_1340.md)

## Prova diferencial contra o `HEAD` expira no proprio commit

- Evidencia: `42e1bf3`; `docs/dossie/prova-do-depois-da-versao.py`
- Arquivo: [cognicao_prova-contra-o-head-expira-no-proprio-commit_20260924_0955.md](cognicao_prova-contra-o-head-expira-no-proprio-commit_20260924_0955.md)

## Recusa simétrica de sufixo de balde não é «só a letra 1»

- Evidencia: `crates/phxsql-store/src/catalogo.rs::criar_recusa_sufixo_de_letra_da_particao_sem_perguntar_ao_disco`; `crates/phxsql-store/src/catalogo.rs::criar_recusa_ponto_no_nome_por_colidir_com_o_qualificado`. Validada pelo integrador, que nao e o autor do conserto, em 24/09/2026 na arvore exata: com as duas guardas novas de `exigir_nome_que_volta` neutralizadas, os dois testes caem (`catalogo.rs:2281` e `:2321`); com o conserto, os tres passam.
- Arquivo: [cognicao_recusa-simetrica-de-sufixo-nao-e-so-a-letra-1_20260924_1438.md](cognicao_recusa-simetrica-de-sufixo-nao-e-so-a-letra-1_20260924_1438.md)

## Aviso de corte por teto: 01000, nunca 01004 -- e onde ele mora sozinho

- Evidencia: `crates/phxsql-odbc/src/lib.rs::sql_truncado_pelo_teto_avisa_01000`; `crates/phxsql-odbc/src/lib.rs::sql_sem_truncado_continua_sql_success_puro`; `testes-web/prova-truncado-sql.mjs` (rodada em 24/09/2026: 3/3 passos verdes com o conserto, 2/3 com a leitura de `r.truncado` removida do `claude.js` -- prova nos dois sentidos, pelo navegador, contra o `phxsqld` de verdade).
- Arquivo: [cognicao_sqlstate-do-truncado-em-composicao_20260924_1308.md](cognicao_sqlstate-do-truncado-em-composicao_20260924_1308.md)
