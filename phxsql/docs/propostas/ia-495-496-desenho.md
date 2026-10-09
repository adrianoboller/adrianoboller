# Desenho 495 + 496 — a IA do PhxSql: crime no banco e catástrofe antes dela (papel J)

09/10/2026. Papel J, cláusula «o pesquisador decide; o dono é o impasse». Árvore: `HEAD 9a25642f`.
Consolida e **remede** o que já foi decidido em 24/09 — `pesquisa-495-ia-seguranca-2026-09-24.md` (J),
`parecer-sec-495-ameaca-2026-09-24.md` (SEC) e `parecer-dba-496-catastrofes-2026-09-24.md` (C) — contra a
árvore de hoje. Não repete as provas de lá; aponta para elas.

## 0. Decisão, em oito linhas

1. **Um motor só para 495, 496 e 707**: `Ocorrencia` tipada → silêncio (`jobs::pode_avisar`) → fila com carteiro
   (o da `saude_do_disco.rs`, extraído) → canais que já existem. O critério «anormal» das bolhas do 707 é a
   **mesma** linha de base da F4 — não um segundo.
2. **No servidor, sem dependência:** regras determinísticas (4 classes, só observam), **linha de base por
   impressão digital** (duração e linhas devolvidas, Welford sobre `ln`), **previsão por tendência** (mínimos
   quadrados, duas janelas, vale a menor) e **contagem regressiva** até tetos duros.
3. **Fora do servidor, pela tela:** a IA **explica e sugere** o evento marcado, com o corpo aprovado pela pessoa
   (339(a)). **Nunca portão, nunca ação.**
4. **A premissa do pedido envelheceu, medida por leitura:** «o servidor não consegue chamar IA externa porque a `std`
   não tem TLS» deixou de valer na 0.20 — o TLS 1.3 de saída existe (`tls_saida.rs`, pedido 572 ☑️, cadeia RSA/P-256/
   P-384/Ed25519 em `phxsql-core/src/cadeia.rs:13-14`). O que mantém a IA na tela **não é mais o TLS**: é a custódia
   da chave e a aprovação do conteúdo do 339(a). Isso é **produto** e sobe (§8, item 2).
5. **Custo no laço quente:** desligado **0,51 ns** (medido hoje); ligado ~**1,7 µs** por pedido `sql` ≈ **0,5%** de
   um pedido de 322 µs (parte medida hoje, parte citada — §3).
6. ~~Linha de base nasce DESLIGADA (voto 7 × 2)~~ — **morreu na A0** (`aquario-707.md` §11.2): a base nasce
   **ligada atrás do portão da telemetria**, porque o 707 (08/10, posterior ao voto) pediu «anormal pelo habitual,
   não por tamanho fixo»; ligada custa 37 ns no protocolo, ≈ 0,5% com digital e detector. O **detector de 4
   classes nasce ligado para observar** (decisão de 24/09 mantida).
7. **Previsão compra antecedência para o lento; o rápido é conserto.** Os cinco consertos ativos do catálogo do C
   (498, 509, 510, 511, 512) estão ☑️ — o terreno que o C exigia antes do previsor existe.
8. **Falta da versão, pelo gerador** (`pagina-dos-pedidos.py`, lido sem gravar): **0,0%** — 642 feitos, 0 abertos,
   108 ⏸. **495 e 496 ainda estão ⏸**: o integrador tem de passá-los a ☐ para a 0.21 entrar na conta.

---

## 1. Hipóteses, escritas antes de medir (as de 24/09 não se repetem; só as novas e as reabertas)

| # | hipótese | previsão escrita antes | veredito |
|---|---|---|---|
| A1 | com o TLS de saída da 0.20, o servidor pode chamar a IA sozinho, fora do laço, pelo carteiro | possível sem ferir pétrea; latência irrelevante fora do laço | **tecnicamente viva, decidida contra por padrão**: fere a custódia da chave e a aprovação do 339(a) → **produto, sobe** |
| A2 | a IA continua só pela tela, explicando o evento redigido | já é o desenho do `CLAUDE-IA.md` | **entra** (F9) |
| B1 | linha de base por **(usuário, op)** basta — sem digital | barata, mas mistura consulta de 1 ms com relatório de 2 s | **morre**: os três maduros agregam **por digital** (§2, linha 3), 9 × 0 |
| B2 | linha de base **por digital**, média e desvio (PG) | z-score sobre latência de cauda longa dá falso alarme | **entra com divergência**: Welford sobre `ln(duração)` (§5) |
| B3 | linha de base por digital com **quantis por histograma** (MySQL `QUANTILE_95/99`) | mais fiel na cauda, mais memória | **morre no voto**: dispersão PG 4 (desvio) × MySQL 2 (quantil) × MariaDB 3 (nenhuma) → desvio |
| C1 | tabela de digitais cheia **despeja** a menos usada (PG) | atacante que inunda digitais novas apaga a base de quem é normal | **morre**: 4 × 5 contra a **linha-coringa** do MySQL/MariaDB, e a nossa restrição (integridade do evento) concorda |
| D1 | tendência por **mínimos quadrados** (Prometheus `predict_linear`) | erro baixo a taxa constante; otimista em rajada | **entra**, com a correção medida pelo C: duas janelas, vale a menor |
| D2 | tendência por **média móvel exponencial** | reage mais rápido | **morre sem número novo**: o C mediu que a janela curta erra para o lado **perigoso** (5/5 otimista, até +1.281%); EWMA é janela curta com outro nome |
| E1 | previsão substitui o conserto do disco cheio | — | **morre** (C §8.2): 64 MiB a 16 MiB/s enche em 4,07 s, abaixo de qualquer amostra |

---

## 2. Matriz de evidência — fonte → o que resolve → custo

| # | comportamento | PostgreSQL (4) | MariaDB (3) | MySQL (2) | SQLite (1) | régua | nós |
|---|---|---|---|---|---|---|---|
| 1 | **auditoria registra, não detecta** | `pgaudit`: «detailed session and/or object audit logging»; «best-effort and not transactional»; exige `shared_preload_libraries`; avisa que o volume «may still noticeably affect latency» ([github.com/pgaudit/pgaudit](https://github.com/pgaudit/pgaudit)) | `server_audit`, só registra (fonte de 24/09) | auditoria na edição comercial | não tem | converge: **registra** | `acessos.log` + Profiler; nada novo |
| 2 | **consulta lenta por limiar fixo, desligado de fábrica** | `log_min_duration_statement` = `-1`, «disables»; há amostragem (`log_statement_sample_rate`) ([runtime-config-logging](https://www.postgresql.org/docs/current/runtime-config-logging.html)) | idem ao MySQL | `slow_query_log` «disabled» de fábrica; `long_query_time` = 10 s ([slow-query-log](https://dev.mysql.com/doc/refman/8.4/en/slow-query-log.html)) | não tem | converge: **limiar fixo, desligado** | o limiar fixo não serve a quem tem consulta de 1 ms e de 2 s → linha de base (F4) |
| 3 | **estatística por digital** (literal vira marcador) | `pg_stat_statements`: `mean/stddev/min/max_exec_time`, `rows`; teto `max` = 5000; exige preload ([pgstatstatements](https://www.postgresql.org/docs/current/pgstatstatements.html)) | `events_statements_summary_by_digest`: `AVG/MIN/MAX_TIMER_WAIT`, `SUM_ROWS_SENT`, `SUM_ERRORS`, **sem quantil** ([KB](https://mariadb.com/kb/en/performance-schema-events_statements_summary_by_digest-table/)); Performance Schema «disabled by default for performance reasons» ([KB](https://mariadb.com/kb/en/performance-schema-overview/)) | mesma tabela **com** `QUANTILE_95/99/999` por histograma ([8.4 §29.12.20.3](https://dev.mysql.com/doc/refman/8.4/en/performance-schema-statement-summary-tables.html)) | `sqlite3_normalized_sql()` só com opção de compilação | **converge** em digital + média/mín/máx + linhas → **aceite** | F4 |
| 4 | **tabela cheia de digitais** | despeja as menos usadas | linha-coringa `DIGEST = NULL` | linha-coringa; se ela passa de ~50% «the summary is not very representative» (mesma página) | — | coringa 5 × despeja 4 | coringa (C1 morre) |
| 5 | **estatística ligada de fábrica?** | não (preload) | **não** | sim | — | desligada 7 × 2 | ~~F4 nasce desligada~~ **ligada atrás do portão: o voto é alcançado pela ordem do dono do 707 (A0, §11.2 do aquário)** |
| 6 | **lista permitida por conta, com treino** | não tem | MaxScale `dbfwfilter` removido (24/09) | Enterprise Firewall, comercial: «recording, protecting, or detecting mode» por conta ([firewall](https://dev.mysql.com/doc/refman/8.4/en/firewall.html)) | autorizador **por ação**, na preparação — «disallows everything except SELECT» ([set_authorizer](https://www.sqlite.org/c3ref/set_authorizer.html)) | 2 × 8 | morta em 24/09; ⏸ |
| 7 | **detector heurístico de SQLi no núcleo** | não | não | não | não | 0 × 10 | entra **só por ordem do dono**, e por isso só observa |
| 8 | **mercado: SQLi por regra** | `libinjection` (BSD-3): «tokenizes the input, folds the token stream, and looks up the result in a table of known-bad token patterns»; «Only the first five folded tokens are examined»; acusa prosa como `total (5); tax included` ([github](https://github.com/libinjection/libinjection)) | | | | — | **diverge**: o motor nunca vê o parâmetro isolado; olha a consulta inteira, todos os símbolos (restrição: o evento sem rabo invisível) |
| 9 | **mercado: previsão de esgotamento** | Prometheus `predict_linear`: «using simple linear regression» ([functions](https://prometheus.io/docs/prometheus/latest/querying/functions/)) | | | | — | D1; **diverge** na janela dupla (medida do C) |
| 10 | **contagem regressiva até teto duro** | xid: aviso a 40 M, recusa a 3 M | não conferido | `auto_increment_ratio` | — | 6 × 1 (C §4) | F6 |
| 11 | **atraso da réplica exposto** | `replay_lag` | `Seconds_Behind_Master` | `Seconds_Behind_Source` | não replica | **converge** → aceite | F7 |
| 12 | **prever por tendência** | não achado | não achado | não achado | não | — | **ordem do dono (496)**, não convergência |

Base permanente — **Cassandra**: a auditoria 5.0 registra e não detecta, e tapa senha por recorte
(`PasswordObfuscator`, lido em 24/09). Diverge aqui pela pétrea *redigir analisando*: o texto do evento é a
digital — literal nenhum sobrevive. Nada do Cassandra prevê esgotamento no núcleo (não procurado nesta rodada: lacuna L5).

---

## 3. Números — medidos hoje × citados

Medidos hoje (`rustc -O`, 4 núcleos, carga 5,4 de outras frentes; mediana de 9 voltas × 2 M; fonte no Apêndice A):

| o quê | mín / mediana / máx |
|---|---|
| **desligado: `AtomicBool::load(Relaxed)`** | 0,40 / **0,51** / 0,58 ns |
| 2 × `Instant::now()` (a duração do pedido) | 45,1 / 56,6 / 60,5 ns |
| linha de base: mutex sem disputa + `HashMap<u64,_>` + Welford + z | 22,4 / **28,6** / 33,2 ns |
| mínimos quadrados, 96 amostras (fora do laço; a cada 15 min) | 57,8 / 58,1 / 63,9 ns |

Citados, **não remedidos** (24/09, `bancada/seguranca/495/`, árvore `ba0e62a`): detector de 4 classes com os
símbolos prontos **885 ns** (teto: o protótipo aloca); digital (normalizar + FNV-1a) **741 ns** (idem, monta
`String`); pedido `sql` pelo soquete **322 µs**; IA local 463 ms–14 s por evento.

Conta (raciocinada): ligado ≈ 885 + 741 + 57 + 29 ≈ **1,7 µs** ≈ **0,53%** de 322 µs. O que decidiria na bancada:
remedir 885/741 com a digital calculada **sobre os símbolos, sem `String`** — o protótipo é teto.

---

## 4. O que roda onde

| camada | o quê | onde | custo | quando |
|---|---|---|---|---|
| **portão** | `if !ligado { return }` antes de tudo (pétrea da instrumentação) | laço quente | 0,51 ns | sempre |
| **regras** | 4 classes de 24/09 (empilhado — o `comando_empilhado` consertado no 501, um motor só —, constante sob `OR`, UNION de sondagem, comentário que engole aspa), sobre os símbolos que a análise já fez, no sucesso **e** no erro | laço quente | 885 ns (teto) | ligado de fábrica, só observa |
| **linha de base** | **a mesma do 707** (A0): chave op+tabela ou digital; Welford de `ln(µs)` de serviço e de `ln(1+linhas)` em duas metades de 30 min; máx, erros, primeira/última vez; teto 5.000 + coringa; sem `OPS_DE_REPLICACAO` | laço quente, no `anotar` | 37 ns + digital | **ligada atrás do portão da telemetria** (A0) |
| **previsão** | `esgota_em` (disco, `.log`, RSS × `MemAvailable`, descritores); contagem regressiva (tabela, diário); atraso da réplica | thread do vigia, 15 min | 58 ns por série | o vigia **amostra sempre**; `alertas.ligado` decide só o e-mail (C §6) |
| **camada** | `Ocorrencia` → silêncio → fila → carteiro → `ocorrencias.log` (redigido), op `ocorrencias`, e-mail/SMS | fora de toda trava | — | sempre que houver produtor |
| **IA** | explicar o evento, sugerir índice/parâmetro, dizer «parece ataque / parece defeito» | **navegador**, com aprovação do corpo (339(a)) | 0 no servidor | pedida por gente |

**O que vai à IA**: família, tipo, classes, digital (texto normalizado), tabelas pela árvore inteira
(`profiler::colher_tabelas`), contagens, z, a série da previsão. **Nunca** linha de dado, nunca literal; usuário e
IP pseudonimizados. A resposta é parecer, **nunca ação** — o «apague X» semeado num nome de tabela não vira nada.

**Sinal «anormal» (F4), escrito — revisto pela A0:** `n ≥ 20` **e** `z = (ln x − média)/desvio ≥ 4` **e** serviço
≥ 250 ms, ou `z ≥ 4` **e** linhas ≥ 1.000 → `Alarme::ForaDoHabitual` (amarelo), silenciado por (chave, usuário).
O `n ≥ 30` morreu (aquecimento 0,018–0,020% com 20 e com 30); sem o piso, 40/9.317 alarmes nos 24 logs contra 1. Raciocinado, não medido: a taxa de
falso alarme com z ≥ 4 sobre `ln` depende do tráfego real (lacuna L2). Digital **nova** de usuário que só tinha
digitais conhecidas há 7 dias **não** vira ocorrência sozinha: 94,3% de digital nova no console (24/09, §4.3).

---

## 5. Onde diverge da origem, e a restrição que causou

| de onde | o que fizemos diferente | restrição nossa |
|---|---|---|
| `pg_stat_statements` (média/desvio em ms) | desvio sobre `ln(µs)`, não sobre µs | a base serve a **alarme**; z sobre cauda longa alarma à toa, e o «anormal» do 707 é mostrado ao dono numa TV — alarme falso ali é mentira sobre o servidor |
| MySQL digest (trunca em `max_digest_length`) | não trunca | integridade do evento: rabo invisível numa digital de segurança (24/09) |
| PG (despeja a menos usada) | linha-coringa | quem inunda digitais não apaga a base dos outros |
| PG/MySQL (estatística só da consulta que deu certo) | conta também a recusada | o evento existe para a tentativa |
| Prometheus `predict_linear` (uma janela) | duas janelas, vale a menor | medida do C: janela curta otimista 5/5 |
| maduros (contagem regressiva) | tendência **e** contagem | o `.reg` não reaproveita slot e o `.log` não se poda: num motor que só cresce, a pergunta útil é «quando acaba» (ordem de digitação) |

---

## 6. Catálogo do 496 — sinal, se já se mede, horizonte

Base: catálogo do C (24/09), reconferido hoje. Horizonte = antecedência que o sinal compra.

| # | catástrofe | sinal | já mede? (hoje) | horizonte | entra por |
|---|---|---|---|---|---|
| C4 | disco enche pelo `.log` que não se poda | `livre_kb(t)` → `esgota_em` até `livre_minimo_mb` | `df` em `sistema.rs:213`, só com o vigia ligado (`servico_telemetria_01.rs:85-135`) | horas a dias; erro ≤ 3,4% a taxa constante (C, 6 corridas); antecedência 63–91% da vida do disco | **F5** |
| C3 | diário bate o teto de volumes | `(fechados + fim/corte) / max_arquivos`; dias até o teto a 184 B/evento | **não** | dias a meses | F6 |
| C7 | tabela paginada cheia | `slots / capacidade` (os dois já saem no `esquema`) | razão **não** | dias a meses | F6 |
| C8 | réplica para trás = RPO da promoção | `atraso = eventos_na_origem − posicao`; 3 amostras crescendo ou > 60 s | **não** (`bidirecional.rs` guarda só a posição consumida) | minutos a horas | F7 (converge 9 × 0) |
| C11/C10 | `UPDATE`/`DELETE` em massa por faixa; cascata larga | `linhas_do_plano / vivas` antes da 1ª escrita; ≥ 50% e ≥ 1.000 | número existe, ninguém olha | **total** (antes do dano) | F8, só observa |
| C6 | backup que não roda (job desligado, nunca agendado) | `agora − último_ok > 1,5 × período` | a **falha** avisa desde o 510 ☑️; a **ausência** não | 1 período | F6 |
| C12 | descritores esgotados | `len(/proc/self/fd) / Max open files` | **não** (só `/proc/self/fdinfo` no `gancho.rs:872`) | minutos a horas | F5 (mesma função) |
| C14 | memória | RSS × `MemAvailable` | `MemAvailable` lido (`sistema.rs:186`) | minutos a horas | F5 (mesma função) |
| C5 | janela do backup passa da aceita | `bytes_da_base / vazão_medida` | trava de leitura desde o 513 ☑️; `duracao_ms` do backup não medido aqui | 1 período | F6 |
| C9 | trava presa/envenenada | `espera_maior_ms`, `trava_us` (telemetria) | sim | **zero** — ocorrência, não previsão | F2 (produtor) |
| C1 | `fsync` recusado | o próprio evento | sim (509 ☑️ derruba o servidor) | zero | F2 (produtor) |
| C13 | carimbo empurrado | salto por evento | recusa no teto desde o 511 ☑️ | zero | F2 (família ataque/defeito) |
| C2a | erro que grava | — | conserto no 498 ☑️ | — | fora do previsor |
| — | rajada (job em fuga) | — | — | **nenhum**: 4 s | conserto, nunca previsão (E1) |

---

## 7. Fatias para o papel B (aceite e prova nos dois sentidos)

Ordem: a camada antes dos produtores; o 496 abre pela F5 porque não depende da F1.

| fatia | o quê | aceite | RED → GREEN (defeito reposto derruba) | nível |
|---|---|---|---|---|
| **F1** | `phxsql_sql::sinais(&[Simbolo])` (4 classes) e `digital(&[Simbolo]) -> u64` **sem `String`** | 5 ataques do repositório acusados; 0 de 36 no dado escapado; ≤ 1 no corpo legítimo extraído do código (catraca) | classe devolvendo 0 derruba a detecção; implementação por recorte derruba o escapado (30/36); digital que trunca derruba o teste do rabo | médio |
| **F2** | camada: `Ocorrencia` (o tipo é o `enum Alarme` do 707, A0 §11.3), extrai a fila/carteiro da `saude_do_disco.rs` (ela vira o 1º produtor), `ocorrencias.log` redigido por rodízio | os testes da saúde do disco seguem verdes **sem mudar**; `CREATE USER … PASSWORD '<sentinela>'` marcado não deixa a sentinela no arquivo; tabela no lado B de `juntar` aparece em `tabelas` | segundo carteiro reposto → teste «um carteiro só» (contagem de `Condvar`) falha; recorte no lugar da digital → sentinela aparece | **forte** |
| **F3** | gancho em `executar_e_contar_escrita_local` (`servico_permissao_01.rs:101`) e nos 3 campos de expressão; a análise devolve os símbolos | tautologia do arsenal gera 1 ocorrência **com 2 linhas devolvidas**; resposta ao cliente byte a byte igual com e sem o detector | relexar no gancho → medidor de custo passa de 2× o parse e falha; detector só no `is_err()` (o erro do 215) → a tautologia, que dá certo, some | médio |
| **F4** (≡ **A4** do 707) | `BaseDeConsultas` única (A0): chave op+tabela ou digital, Welford sobre `ln(µs)` de serviço e linhas, duas metades de 30 min; teto 5.000 + coringa; `Alarme::ForaDoHabitual`; **é ela que pinta o 707** | 20 execuções de ~1 ms (com variação) e a 21ª de 400 ms → 1 ocorrência; 20 de 1 ms **exatos** e a 21ª de 10 s → 1 (o chão do desvio; o `z` do Apêndice A devolve 0 e derruba este caso); a 21ª de 200 ms → nenhuma (piso); a 21ª de 1,3 ms → nenhuma; `replicar_aguardar` de 1 s nunca; 5.001ª chave vai à coringa e a base das outras não muda | desvio sobre µs em vez de `ln` → o caso 1,3 ms alarma; despejo no lugar da coringa → inundação apaga a base; portão depois do `Instant::now` → bancada «desligado custa > 1 ns» falha | **forte** (trava no laço quente) |
| **F5** | `esgota_em(amostras, piso) -> Option<Previsao>` (C §6) + mesma função para RSS e descritores; vigia amostra sempre | série linear acerta ± 2%; constante/crescente → `None`; R² < 0,6 → `None`; rajada: nunca otimista | sinal invertido; sem portão de R²; só a janela curta — cada um derruba o seu teste; contra o SO (`unshare -m`, tmpfs 64 MiB, 8 MiB/s): aviso **antes** do `ENOSPC` com ≥ 50% de antecedência, produtor desligado → nenhum aviso → vermelho; sem `CAP_SYS_ADMIN` diz **«não provado»** | médio |
| **F6** | contagem regressiva: tabela (C7), diário (C3), idade do backup (C6), janela do backup (C5); aviso 80%/30 dias, crítico 95%/3 dias | tabela a 81% → aviso; a 79% → nada | limiar no `>` trocado por `>=`, e o teste de fronteira cai | leve |
| **F7** | atraso da réplica por tabela + tendência | réplica parada com mestre gravando → 3 amostras crescendo → ocorrência | réplica em dia → nenhuma ocorrência (o teste do comportamento velho) | médio — **C revisa** |
| **F8** | tamanho do plano antes da 1ª escrita (C10/C11), só observa | `UPDATE … WHERE id > 0` em 2.000 de 2.000 → ocorrência; 10 de 2.000 → nenhuma; resposta igual | contar depois de aplicar → o teste que mede «ocorrência antes da 1ª escrita» falha | médio |
| **F9** | op `ocorrencias` (administrar), painel, e-mail pelo `avisar_seguranca`; botão «explicar» manda o evento à Claude **pelo navegador**, com o corpo aprovado | corpo que sobe sem literal e sem linha; e-mail sai com o interruptor, não sai sem (SMTP falso) | `testes-web`: chave em pedido ao PhxSql → falha (prova do 339(a) estendida); envio sem clique → falha | médio; tela leve, **fábrica de idiomas** |

⏸ (fora da conta): lista permitida por conta (morta 2 × 8), modelo local de IA, IA autônoma no servidor (até o
dono decidir §8.2), migração dos outros `email::enviar` para a camada.

---

## 8. O que sobe ao dono — só produto

1. **A promessa ao cliente (aberta desde 24/09, sem resposta registrada).** Medido: o banco **evita** a injeção de
   quem usa `?`; **detecta e avisa** quem concatena; a IA **explica**. Ninguém aqui **impede** o cliente que
   concatena. O texto ao cliente pode dizer «detecta e avisa» em vez de «a IA evita SQL injection»? — o mesmo
   cuidado do *ACID compliant*.
2. **IA autônoma no servidor (nova).** Desde a 0.20 o servidor **consegue** chamar a Claude por HTTPS sem
   dependência. Fazer isso — o aviso das 3 h da manhã já chegar explicado — exige guardar a chave da API **no
   servidor** e mandar o evento **sem uma pessoa aprovar o corpo**, que é o contrário do que o 339(a) promete hoje
   (chave só na memória da aba; «nada sai sem você aprovar»). Não é choque com pétrea (zero dependências fica
   intacto); é a promessa de privacidade do produto. **Padrão sem resposta: fica na tela.**

Não sobe: nenhum empate (votos 9 × 0, 7 × 2, 5 × 4, 2 × 8, 0 × 10); nenhuma pétrea ferida.

## 9. Lacunas

- **L1** — taxa de detecção de UNION de sondagem e comentário que engole aspa: **0** caso no repositório (24/09).
  PENDENTE até SEC/F escreverem as cargas no `injecao.py`.
- **L2** — falso alarme da F4 em tráfego real: não existe corpo. O limiar z ≥ 4 é raciocinado.
- **L3** — 885 ns / 741 ns / 322 µs são de 24/09, árvore `ba0e62a`; não remedidos.
- **L4** — aperto TLS do servidor contra `api.anthropic.com` **não exercitado** (o ambiente sai por proxy que
  reassina); a capacidade foi lida, não provada contra aquele host.
- **L5** — fan-out: esta instância não convoca `pesquisa-motor`/`pesquisa-bancada`; a revisão cruzada ficou faltando.
  MariaDB `server_audit` e a retenção do binlog: KB, não fonte. Cassandra: previsão não procurada no fonte.

## 10. Aprendizados (PENDENTE)

- **A premissa citada no pedido envelheceu em uma versão:** «a `std` não tem TLS» era verdade em 24/09 e deixou de
  ser na 0.20 — e o 495 a carregava como razão. A razão certa para a IA ficar na tela é a do 339(a). Evidência: o
  `tls_saida.rs` e a cadeia de `cadeia.rs:13-14`. Vale para a cognição: **razão de uma decisão se remede quando a
  base muda**, mesmo quando a decisão sobrevive.

---

## Apêndice A — o medidor de hoje (refazer: `rustc -O --edition 2021 linha_de_base.rs && ./linha_de_base`)

Tem de ir para `bancada/seguranca/495/linha_de_base.rs` pelo integrador (script que resolveu não morre com a sessão).

```rust
use std::collections::HashMap;
use std::hint::black_box;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Instant;

#[derive(Default, Clone, Copy)]
struct Base { n: u64, media: f64, m2: f64, max: f64 }
impl Base {
    fn somar(&mut self, x: f64) { self.n += 1; let d = x - self.media; self.media += d / self.n as f64; self.m2 += d * (x - self.media); if x > self.max { self.max = x; } }
    fn z(&self, x: f64) -> f64 { if self.n < 30 { return 0.0 } let s = (self.m2 / (self.n - 1) as f64).sqrt(); if s == 0.0 { 0.0 } else { (x - self.media) / s } }
}
fn fnv(s: &[u8]) -> u64 { let mut h: u64 = 0xcbf29ce484222325; for b in s { h ^= *b as u64; h = h.wrapping_mul(0x100000001b3); } h }
fn mediana(mut v: Vec<f64>) -> (f64, f64, f64) { v.sort_by(|a, b| a.partial_cmp(b).unwrap()); (v[0], v[v.len() / 2], v[v.len() - 1]) }

fn main() {
    const N: usize = 2_000_000;
    let ligado = AtomicBool::new(false);
    let digitais: Vec<u64> = (0..5000u64).map(|i| fnv(format!("SELECT * FROM t{} WHERE id = ?", i % 1000).as_bytes())).collect();
    let mut res: HashMap<&str, Vec<f64>> = HashMap::new();
    for _ in 0..9 {
        let t = Instant::now(); let mut c = 0u64;
        for i in 0..N { if black_box(&ligado).load(Ordering::Relaxed) { c += i as u64 } }
        black_box(c); res.entry("desligado (AtomicBool)").or_default().push(t.elapsed().as_nanos() as f64 / N as f64);
        let t = Instant::now(); let mut acc = 0u128;
        for _ in 0..N { let a = Instant::now(); let b = Instant::now(); acc += (b - a).as_nanos(); }
        black_box(acc); res.entry("2x Instant::now").or_default().push(t.elapsed().as_nanos() as f64 / N as f64);
        let tab: Mutex<HashMap<u64, Base>> = Mutex::new(HashMap::with_capacity(5000));
        let t = Instant::now(); let mut zs = 0.0;
        for i in 0..N { let d = digitais[i % digitais.len()]; let x = (i % 97) as f64 * 3.0 + 40.0; let mut g = tab.lock().unwrap(); let b = g.entry(d).or_default(); zs += b.z(x); b.somar(x); }
        black_box(zs); res.entry("linha de base (mutex+mapa+Welford+z)").or_default().push(t.elapsed().as_nanos() as f64 / N as f64);
    }
    let pts: Vec<(f64, f64)> = (0..96).map(|i| (i as f64 * 900.0, 1e9 - i as f64 * 1e6)).collect();
    let mut v = Vec::new();
    for _ in 0..9 { let t = Instant::now(); let mut s = 0.0; for _ in 0..100_000 { let n = pts.len() as f64; let (sx, sy, sxx, sxy) = black_box(&pts).iter().fold((0.0,0.0,0.0,0.0), |a, p| (a.0+p.0, a.1+p.1, a.2+p.0*p.0, a.3+p.0*p.1)); s += (n*sxy - sx*sy) / (n*sxx - sx*sx); } black_box(s); v.push(t.elapsed().as_nanos() as f64 / 100_000.0); }
    res.insert("minimos quadrados, 96 amostras (por chamada)", v);
    let mut ks: Vec<_> = res.keys().cloned().collect(); ks.sort();
    for k in ks { let (a, m, z) = mediana(res[k].clone()); println!("{k:48} min {a:8.2} ns  med {m:8.2} ns  max {z:8.2} ns"); }
}
```

## 11. A0 — unificação com o 707 (09/10/2026)

A decisão inteira, com hipóteses, números e scripts, mora em **um lugar só**: `aquario-707.md` §11 e
Apêndice B. Aqui, o que mudou neste desenho:

| hipótese daqui | destino | número |
|---|---|---|
| Welford **cumulativo** | morre → duas metades de 30 min | raciocinado (o cumulativo não esquece) |
| n ≥ 30 | morre → n ≥ 20 | aquecimento 0,018% (30) × 0,020% (20), 4.000 chaves |
| sem exclusão | morre → sem `OPS_DE_REPLICACAO` | 54 × 40 alarmes em 24 logs; 14 `replicar_aguardar` |
| sem piso | morre → serviço ≥ 250 ms | 40/9.317 × 1/9.317 |
| desligada (7 × 2) | morre → ligada atrás do portão | o 707 alcança o voto; ligada ≈ 0,5% |
| `Ocorrencia` com tipo próprio | morre → o tipo é `Alarme` | lei «função e comando vêm do mesmo motor» |
| um `ocorrencias.log` para tudo | fica, **com papel**: o fato e a segurança; a linha do tempo e a contagem vão ao `aquario.log` | separar por papel 5 × 4; retenção sob ataque |
| Welford sobre `ln(µs)` | **fica** e vira a base do 707 | 37 ns × 119 ns do histograma; 0,004% × 0,612% em cauda larga |

A premissa «µs, não ms» foi **medida**: no log ODBC, 170/184 linhas têm `ms = 0`, 152 delas `sql`.

## Decisões do dono, 09/10/2026

1. **A promessa ao cliente é «detecta e avisa».** O servidor detecta por regra
   e por linha de base, registra, avisa e conta a violação; a IA explica na
   tela. Não se escreve «bloqueia» nem «a IA evita SQL injection» em
   documento, tela ou material de venda.
2. **IA só na tela.** O servidor não chama IA sozinho: não guarda chave da
   API e não manda evento sem uma pessoa aprovar o conteúdo — o desenho do
   339(a) vale também para o 495 e o 496. O servidor detecta e prevê sem IA.
