# REUSE -- o que ja deu certo aqui, comprovado

<!-- GERADO por docs/cognicao/avoid-e-reuse.py -- NAO EDITE A MAO.
     `--catraca` reprova se este arquivo nao bater com o que o extrator
     geraria agora; rode o comando sem flag para atualizar. -->

Gerado dos `cognicao_*.md` com `**Estado:** FRUTIFERO` -- 2 hoje, de 311 cognicoes no total.

## Prova diferencial contra o `HEAD` expira no proprio commit

- Evidencia: `42e1bf3`; `docs/dossie/prova-do-depois-da-versao.py`
- Arquivo: [cognicao_prova-contra-o-head-expira-no-proprio-commit_20260924_0955.md](cognicao_prova-contra-o-head-expira-no-proprio-commit_20260924_0955.md)

## Aviso de corte por teto: 01000, nunca 01004 -- e onde ele mora sozinho

- Evidencia: `crates/phxsql-odbc/src/lib.rs::sql_truncado_pelo_teto_avisa_01000`; `crates/phxsql-odbc/src/lib.rs::sql_sem_truncado_continua_sql_success_puro`; `testes-web/prova-truncado-sql.mjs` (rodada em 24/09/2026: 3/3 passos verdes com o conserto, 2/3 com a leitura de `r.truncado` removida do `claude.js` -- prova nos dois sentidos, pelo navegador, contra o `phxsqld` de verdade).
- Arquivo: [cognicao_sqlstate-do-truncado-em-composicao_20260924_1308.md](cognicao_sqlstate-do-truncado-em-composicao_20260924_1308.md)
