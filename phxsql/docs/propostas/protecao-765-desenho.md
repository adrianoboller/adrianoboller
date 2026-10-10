# Proteção — pedidos 765, 766 e 767: desenho do papel J

> **PRECISÃO DO DONO, 09/10/2026:** cada comando perigoso vai à **trilha** e ao **aquário** com o **desfecho** — `executou`, `bloqueado` (com o motivo) ou `trancou` — e fica no **log** (`ocorrencias.log` para o bloqueado; linha de trilha para o executado). A TV continua sem login nem IP.

> **PRECISÃO DO DONO, 09/10/2026:** «Pode ter um comando que reative a segurança na mesma sessão.» Na P14 entra o **trancar de novo** (como o `sudo -k`): op `trancar_execucao` / SQL `LOCK EXECUTION` / botão na tela, um motor só. Não exige senha, é idempotente e vai à trilha; o próximo comando perigoso volta a pedir a segunda senha.

> **PRECISÃO DO DONO, 09/10/2026:** «Uma vez informada, a senha mestre fica na sessão; não é necessário para cada comando.» Na P14, a segunda senha **libera a sessão** até ela terminar (logout, inatividade do 770, queda da conexão) — no lugar do token de 5 min e uso único. A liberação não passa para outra sessão nem outro IP; cada comando perigoso continua registrado na trilha e no aquário.

> **PRECISÃO DO DONO, 09/10/2026:** «Não é a senha do usuário, é uma **segunda senha**.» Na P14, a senha de execução é uma **credencial própria**: cadastro à parte, PBKDF2 e sal próprios, recusada se igual à de login, troca e bloqueio por tentativas próprios. Ter a sessão ou a senha de login **não basta**. Substitui a «reautenticação pelo desafio-resposta do login» proposta abaixo; o token de 5 min, uso único e escopo (op, base, tabela, linhas do plano) continua — ele é emitido **por essa segunda senha**.

> **PRECISÃO DO DONO, 09/10/2026:** «Sem a senha de execução esses comandos perigosos não são executados.» O token de passo acima (P14) é **obrigatório em qualquer modo** para todo comando da lista de perigo: sem ele, **recusa**. O modo «observar» não se aplica a esses comandos; vale só para os sinais que não são comando (perfil habitual, IP novo). O job de manutenção tem senha de execução própria, com escopo e prazo (P12); a réplica fica isenta (P5).

> **DECISÃO DO DONO, 09/10/2026, depois deste desenho:** a promessa ao cliente é **«protege e bloqueia»**, e o modo **proteger é o padrão de fábrica** das linhas de `phxsys.protecao`. É exceção explícita à pétrea «guarda nova entra pedida, não imposta», só para 765/766/767. Onde este documento diz «observar de fábrica», vale **proteger de fábrica**; a consequência (job e cliente que hoje rodam DROP/TRUNCATE/alteração em massa passam a precisar do token de passo acima — P12/P14) é aceita pelo dono.

09/10/2026. Árvore `3cfc27e5`. Só documento; nada comitado. Pedidos 765/766/767 são ordem do dono.
Eixos: (a) evitar o que derruba/danifica/para · (b) IP nunca visto · (c) perfil habitual · (d) bloqueio de IP
por código malicioso, como **regra no firewall do SO** (766) · (e) **camada de análise única, tabela de
configuração e token de passo acima** (767).

Fan-out: `pesquisa-motor` e `pesquisa-bancada` **dispensados** — esta instância não tem a ferramenta de
convocar subagente; a pesquisa e as medições abaixo foram feitas pelo próprio J. A revisão cruzada fica faltando
(lacuna L7).

---

## 0. Decisão, em dez linhas

1. **Um interruptor, três estados por linha:** `desligado` / `observar` / `proteger`, numa tabela de sistema
   `phxsys.protecao`, uma linha por comando perigoso. **Fábrica: `observar` em todas** — é o modelo
   DETECTING/PROTECTING do MySQL Enterprise Firewall, `supervise`/`enforce` do MaxScale e SIMULATION/ENABLED do
   Oracle Database Vault. **Não há choque com a pétrea «guarda nova entra pedida»: ela é cumprida pelo próprio
   desenho** (observar não muda resposta nenhuma; proteger só liga quem pedir).
2. **Uma camada só**, no `executar_e_contar_escrita_local` (`servico_permissao_01.rs:101`) — onde já passam a
   rede, a op `sql` derivada e o job, e onde a F3 já mora. Ela classifica pela **op que vai executar**, não pelo
   texto: `DROP TABLE` vira `excluir_tabela` antes de chegar lá, então o DROP dentro de rotina, de gatilho AFTER
   e de job chega com o mesmo nome.
3. **Em `proteger`, o comando perigoso exige um token de passo acima** (767): emitido à parte por reautenticação
   (o desafio-resposta que já existe), 5 min, escopo (op, base, tabela), **1 uso**, guardado só como SHA-256 em
   memória. É o **mesmo motor** da confirmação do plano largo — não há um segundo «confirme».
4. **Prazo de comando fora de transação** (o que falta de verdade para «parado»): hoje toda operação fora de
   transação **não tem prazo nenhum** (`telemetria.rs:486`, «que e toda operacao fora de transacao»), e a trava
   de dados é global.
5. **IP nunca visto** = primeiro login **com sucesso** de (usuário, base, IP) em 90 dias → ocorrência. Nunca
   recusa login.
6. **Perfil habitual só observa, nesta versão.** Medido: 40,5% dos pedidos de teste trariam (verbo, tabela)
   novo; por digital, 98,9%. Proteger por perfil travaria o trabalho.
7. **766: o código malicioso conta pela política leve que já existe** (5 em 10 min → bloqueio de 60 min),
   **com escalonamento ×2 até 7 dias**, e vai ao firewall do SO por **conjunto com prazo no kernel**
   (nftables `set ... flags timeout`; `ipset ... timeout` na reserva), não uma regra por IP.
8. **Nunca se auto-bloqueia** loopback, whitelist, IP de administrador logado e **IP compartilhado** (≥ 2
   usuários distintos autenticados em 30 dias, lido da memória do eixo b) — este último vira encerrar a sessão,
   não firewall.
9. **Defeito ativo achado (vai para a conta):** `Alarme::FirewallBloqueou` existe e **tem 0 produtores** — nenhum
   bloqueio vira ocorrência nem pedra no aquário hoje (`grep` em `crates/`: 5 citações, todas em
   `aquario/mod.rs`).
10. **Sobe ao dono: uma coisa, de produto** — o texto da promessa ao cliente com `proteger` ligado (§8).

---

## 1. Hipóteses, escritas antes de medir

| # | hipótese | previsão escrita antes | veredito |
|---|---|---|---|
| **a1** | recusar `UPDATE`/`DELETE` sem chave no `WHERE` e sem `LIMIT` (`sql_safe_updates`) | barato, sintático | **morre**: o próprio manual do MySQL aceita «key constraint in the WHERE clause» — `WHERE id > 0` passa, e é exatamente o caso que a F8 pega pelo **plano medido** (≥ 50% e ≥ 1.000). Sintaxe erra para os dois lados |
| **a2** | recusa seca do plano largo / DDL destrutiva em `proteger` | protege | **morre**: guarda sem saída é desligada no primeiro job de manutenção que trava; entra recusa **com saída** (token de passo acima, a5) |
| **a3** | limitar `SELECT` sem `LIMIT` (`sql_select_limit=1000`) | corta a consulta que segura a trava | **morre**: devolve 1.000 linhas dizendo que são todas — mentira sobre o dado; o que segura a trava é o **tempo**, então entra prazo (a4) |
| **a4** | prazo de comando também **fora** de transação, pelo mesmo `siga`/`prazo_ate_ms` | a trava global faz um comando lento parar todos | **entra** (P2). Divergência nomeada em §5 |
| **a5** | confirmação em dois passos por HMAC próprio do plano | — | **morre na 767**: seria o segundo motor de token. Confirmação = token de passo acima com escopo |
| **a6** | prazo de trava novo para «trava longa» | — | **morre**: já existe — teto da transação 5 min (pedido 607), `lock_timeout` 500 ms de fábrica (`config.rs:4194`). A lacuna real é a a4 |
| **b1** | IP novo recusa o login | protege conta roubada | **morre**: IP de celular/DHCP muda toda semana; recusar tranca o dono da conta. Observa; em `proteger` só exige passo acima para o comando perigoso |
| **b2** | memória por (usuário, base, IP), primeira vez = login **com sucesso** | barato; falha de login já é força bruta | **entra** (P6) |
| **c1** | perfil por **digital** (allowlist MySQL/MaxScale/ProxySQL) protegendo | fiel | **morre medido**: 98,9% de novidade (§3, M3); 94,3% em 24/09 (`bancada/seguranca/495/medida-1.txt`) |
| **c2** | perfil por (verbo, tabela) protegendo | grosso o bastante | **morre medido**: 40,5% de novidade (M3). Fica **observar**, com aprendizado mínimo |
| **c3** | perfil por (categoria de op, tabela) + histograma de 24 horas UTC, só observa | aceitável como aviso | **entra** (P7) |
| **d1** | código malicioso bloqueia na 1ª (como «grave») | é ataque | **morre**: atrás de NAT o IP é de muitos, e a única acusação do corpo legítimo de hoje (1 de 1.403, M2) seria um operador trancado — o estrago do pedido 203 |
| **d2** | conta pela política **leve** que já existe (5 / 10 min / 60 min) | o fail2ban de fábrica é 5 / 10 min / 10 min | **entra** (P8) — nenhum segundo contador |
| **d3** | uma regra de firewall por IP | simples | **morre**: conjunto com prazo no kernel tira a classe inteira da regra órfã por prazo; 10.000 elementos entram num lote em **27 ms** (M5) |
| **d4** | o processo PhxSql com `CAP_NET_ADMIN` | sem sudo | **morre**: dá poder de rede ao servidor inteiro para servir um comando; o privilégio fica com um ajudante de um comando só |
| **d5** | `sudoers` com curinga direto no `nft` | uma linha | **morre medido**: o `nft` junta o argv e aceita `;` — **sem shell nenhum**, um IP forjado apagou uma tabela (M4); e o `sudoers` avisa que «wildcards can match any character, including white space» |
| **d6** | «sem shell» basta contra injeção no firewall | — | **morre medido** (M4). A barreira real é o `IpAddr` analisado; hoje ela segura (o `parse` da `std` recusa espaço, zona, zero à esquerda) |
| **e1** | a camada de análise lê o **texto** do SQL | pega o DROP onde estiver | **morre**: é a lição do portão — o campo que se lê é o furo. Lê-se a **op derivada**, que é a que executa |
| **e2** | a tabela de configuração protegida só por direito `Administrar` | — | **morre**: quem administra desliga a guarda e depois faz o estrago; baixar proteção é ela mesma comando perigoso (P13) |

---

## 2. Matriz de evidência — fonte → o que resolve → custo

| fonte (primária) | o que diz | resolve aqui | custo / divergência |
|---|---|---|---|
| MySQL 8.4, *Using MySQL Enterprise Firewall* — dev.mysql.com/doc/refman/8.4/en/firewall-usage.html | modos OFF/RECORDING/PROTECTING/DETECTING por perfil; DETECTING «writes suspicious statements to the error log but accepts them»; casa por **digital** | o modelo observar/proteger | allowlist por digital morre medida (c1) |
| MaxScale 25.10, *Firewall filter* — mariadb.com/docs/maxscale/reference/maxscale-filters/maxscale-firewall-filter.md | `idle`/`learn-*`/`supervise`/`enforce`, padrão `idle`; `action` = `return-error`/`disconnect` | o mesmo modelo, lado MariaDB | idem |
| ProxySQL, *Firewall Whitelisting* — proxysql.com/documentation/firewall-whitelist/ | OFF/DETECTING/PROTECTING por usuário | 3ª família, mesmo modelo | não é motor maduro: não vota |
| Oracle Database Vault 26, *Command Rules* — docs.oracle.com/en/database/oracle/oracle-database/26/dvgsg/command-rules.html | regra por comando (ex. DROP TABLE), modo SIMULATION registra sem bloquear | **uma linha por comando** + modo (P12) | não vota (fora da régua) |
| MySQL 8.4 `max_execution_time` (server-system-variables) | só `SELECT` só-leitura, padrão 0 | prazo de comando | MySQL cobre só leitura |
| MariaDB `max_statement_time` (mariadb.com/kb/en/server-system-variables/) | **todos** os comandos, padrão 0 | prazo de comando | — |
| PostgreSQL `statement_timeout`/`lock_timeout`/`transaction_timeout` (postgresql.org/docs/current/runtime-config-client.html) | padrão 0; «not recommended» em `postgresql.conf` porque afeta todas as sessões | prazo de comando nasce 0 | aqui a trava é global — §5 |
| MySQL `mysql-tips` (`--safe-updates`) e MariaDB `sql_safe_updates` | recusa sem chave no WHERE nem LIMIT; padrão OFF; é opção de **cliente** no MySQL | — | morre (a1) |
| MySQL 8.4 *host cache* / `max_connect_errors` | 100 erros seguidos bloqueiam o host; loopback **não** usa o host cache | guarda do loopback | — |
| MariaDB `max_connect_errors` | 100 | — | — |
| PostgreSQL `auth_delay` (postgresql.org/docs/current/auth-delay.html) | atrasa a falha; «does nothing to prevent denial-of-service attacks, and may even exacerbate them» | PG **não** bloqueia IP | — |
| PostgreSQL `log_connections` (runtime-config-logging) | padrão `''`; registra, não decide | eixo b: o PG só registra | — |
| pgaudit README (github.com/pgaudit/pgaudit) | classes READ/WRITE/FUNCTION/ROLE/DDL/MISC/MISC_SET/ALL; padrão `none`; só registra | classes ≈ nossas categorias de op (P7) | — |
| fail2ban `config/jail.conf` (github.com/fail2ban/fail2ban) | bantime 10m, findtime 10m, maxretry 5; `bantime.increment` (×1,2,4,8…, `ban.Count<20`), `maxtime`, `rndtime`; `ignoreself` padrão true | escalonamento (P10) e «não se trancar» (P9) | fail2ban roda como root — aqui não (d4) |
| `sudoers(5)` (sudo.ws) | `timestamp_timeout` 5 min; curinga casa espaço | prazo do token (5 min) e recusa do d5 | — |
| RFC 9470 (OAuth step-up) | erro `insufficient_user_authentication`, `acr_values`, `max_age` | formato da recusa que pede passo acima | — |
| código: `blacklist.rs` | leve 5/10/60; grave 1; whitelist fixa+editável; firewall por argv sem shell, `IpAddr::parse`, prazo e `kill`; fora do mutex (638) | base pronta do 766 | falta: escalonamento, loopback/admin/compartilhado, ocorrência, canonicalizar, reconciliar |
| código: `http.rs:645` `Sessoes::nova`, desafio do login (`servico_web_01.rs:1116`), `hash::iguais_em_tempo_constante` | motor de credencial: 24 bytes de `/dev/urandom` em hexa; comparação em tempo constante | motor do token de passo acima | **`token_de_servico` não existe no repositório** (0 ocorrências) — a referência da ordem estava errada; o motor é este |

### A régua

| pergunta | PG 4 | MariaDB 3 | MySQL 2 | SQLite 1 | soma | decisão |
|---|---|---|---|---|---|---|
| prazo por comando existe e nasce **0** | sim | sim | sim (só SELECT) | não (API `progress_handler`) | **convergência dos 3** | entra; nasce 0 |
| bloqueio automático de IP no servidor | não | sim | sim | n/a | 5 × 5 | **empate** — não decide; decide a **ordem do dono 766**, não sobe |
| `safe_updates` opcional | não (só extensão) | sim | sim | não | 5 × 5 | **empate**, e a a1 morre medida antes da régua |
| allowlist por digital com modos | não | sim (MaxScale) | sim (Enterprise) | não | 5 × 5 | **empate**; morre medida (c1) |
| monitorar comando perigoso **de fábrica** | não (`pgaudit.log=none`) | não | não | não | **0 × 10** contra | ver §5: observar de fábrica diverge, com motivo |
| re-autenticação por comando (passo acima) | não | não | não | não | 0 | **não é aceite pela régua: é produto, já decidido pelo dono na 767** |

A ausência na última linha é **raciocinada**: nas três listas de variáveis lidas não há variável de
reautenticação por comando; o controle dos três é por privilégio (`GRANT`). Ausência não se prova citando.

---

## 3. Medido hoje (09/10/2026, este contêiner, root, nftables v1.0.9, rustc 1.94.1)

| # | o quê | número | refazer |
|---|---|---|---|
| M1 | produtores de `Alarme::FirewallBloqueou` | **0** (5 citações, todas em `aquario/mod.rs`) | `grep -rn FirewallBloqueou crates` |
| M2 | detector F3 no corpo legítimo / ataques | **1 de 1.403** acusado (um 2º comando de verdade); **8 de 8** ataques | `python3 bancada/seguranca/495/catraca_sinais.py --catraca` |
| M3 | novidade na metade de teste, 53 arquivos-fonte como «usuários», 662 pedidos | digital **98,9%**; (verbo, tabela) **40,5%**; só verbo **18,9%** | script em §Apêndice A |
| M4 | injeção no `nft` **sem shell** | IP `127.0.0.3 }; delete table inet vitima; add element … { 127.0.0.4` → `rc 0`, **tabela `vitima` apagada**; `IpAddr::parse` recusa o mesmo texto, recusa `fe80::1%eth0`, `010.1.1.1`, espaço e `\n` | Apêndice B |
| M5 | conjunto nftables com prazo | SYN do IP bloqueado **não chega** (timeout 1 s), outro IP chega em 0,12 ms; após 3,5 s **volta a chegar**; conjunto vazio sozinho; 10.000 elementos num lote **27 ms**; `nft` avulso **3 ms** por chamada; `::ffff:127.0.0.3` entra no conjunto v4 como `127.0.0.3` | Apêndice B (`unshare --net`; `ip` não existe no contêiner, o `lo` sobe por `ioctl`) |

Citados, não medidos hoje: custo do detector com símbolos prontos 878 ns (24/09); desligado 0,36 ns (24/09).
**Raciocinado, não medido:** quanto tempo uma varredura sem prazo segura a trava global (decide o padrão do
prazo em `proteger`; bancada: `--example` de varredura de 1 M linhas com um segundo cliente medindo a espera).

---

## 4. O desenho

### 4.1 A camada única (767.1)

`protecao::analisar(op, pedido, sessao) -> Veredito` chamada **uma vez**, no topo de
`executar_e_contar_escrita_local`, ao lado da vez da F3. Portão antes do trabalho: `op` fora da lista → um
`match` de texto e volta (o laço quente não paga nada). Na lista:

1. lê a linha da op em `phxsys.protecao` (cópia em memória, recarregada por geração, como a ficha da sessão);
2. `desligado` → segue; `observar` → segue e emite ocorrência; `proteger` → exige passo acima, senão
   `ELEVACAO_EXIGIDA` com o escopo pedido (forma da RFC 9470);
3. o veredito do **plano largo** (F8) e da **F3** passa a ser consultado pela camada, não pelos dois ganchos em
   separado: uma decisão, um lugar.

**Quem esconde o comando da camada — a varredura que a lição do portão manda fazer:**

| caminho | chega à camada? | prova exigida |
|---|---|---|
| rede (TCP, `/api`, REST, MCP, ODBC — todos viram linha do protocolo) | sim (`servidor.rs:1534`) | RED por porta: REST e MCP |
| op `sql` (`DROP TABLE` → `excluir_tabela` derivada) | sim (`servico_permissao_01.rs:88`) | RED: o DROP pelo SQL cai igual ao `excluir_tabela` JSON |
| job | sim (`servico_jobs_01.rs:636`) | RED: job com `excluir_tabela` em `proteger` **não roda** sem escopo pré-autorizado (P12) |
| rotina (`CALL`) e gatilho AFTER | pelo `sql` interno (comentário em `servico_sql_01.rs:1049`: «pelos MESMOS portoes») | RED: rotina com `DROP` e gatilho AFTER com `DELETE` largo — **raciocinado, não provado** até o RED |
| gatilho BEFORE | `MotorNulo` — não fala com o motor | RED: confirmar que não alcança |
| `aplicar` (réplica) | sim, e **isenta por decisão**: o comando já passou pela guarda na origem; exigir passo acima na réplica pararia a replicação | teste do isento: `aplicar` de `excluir_tabela` na réplica não pede token |
| `juntar`/`unir`/`pivotar` | só leem | fora (não são perigosas) — mas a **consulta sem prazo** delas entra pelo P2 |
| manutenção interna (expurgo, arranque) | não passa por `executar_e_contar` | fora — não é pedido de ninguém |

### 4.2 A tabela `phxsys.protecao` (767.2)

Uma linha por comando: `op`, `categoria`, `monitorar` (bool), `modo` (`observar`|`proteger`), `piso_linhas`
(para as de massa), `prazo_ms` (para as de parar). Semeada no arranque como o `phxsys.mensagens`
(`servico_avisos_01.rs:531`), **só se falta** — nunca sobrescreve a escolha do dono do banco.

Inventário (o da ordem, conferido contra as 68 atividades do `usuarios.rs`; os marcados **+** são os que a
varredura achou e a lista não tinha):

| categoria | ops | fábrica |
|---|---|---|
| destruir estrutura | `excluir_tabela`, `excluir_visao`, `excluir_sequencia`, `excluir_fk`, `usuario_excluir`, **+**`cluster_no_remover` | monitorar, observar |
| apagar em massa | `excluir` com plano largo (F8), **+**`esvaziar_lixeira`, **+**`expurgar_trilha` | monitorar, observar |
| alterar em massa | `atualizar` com plano largo, cascata larga (F8) | monitorar, observar |
| reescrever arquivo | `acrescentar_coluna`, `redeclarar_indices_texto`, `criptografar`, `descriptografar`, `migrar_esquema` acima do piso | monitorar, observar |
| parar o banco | comando sem prazo acima do `prazo_ms`, `begin` com escopo exclusivo, **+**`servico_parar`, **+**`restaurar_backup` | monitorar, observar; `prazo_ms` 0 |
| acesso | `usuario_criar` (admin), `usuario_alterar`, direitos por tabela/coluna, **+**`config_gravar`, **+**`diretiva_gravar`, **+**`whitelist_salvar`, **+**`desbloquear`, **+**`replicacao_pular` | monitorar, observar |
| injeção | as 4 classes da F3 | monitorar, observar (como hoje) |
| a própria guarda | escrita em `phxsys.protecao` | **monitorar sempre; baixar exige passo acima se houver qualquer linha em `proteger`** (P13) |

Por que `observar` de fábrica, contra o 0 × 10 da régua: os três não monitoram de fábrica porque monitorar lá é
**log em disco por comando** (custo e volume); aqui é ocorrência só para o que está na lista, silenciada por
`jobs::pode_avisar`, e o precedente já está decidido (F3 e F8 nascem ligados para observar, 24/09). Não muda
resposta nenhuma — a pétrea fica cumprida.

### 4.3 O token de passo acima (767.3)

- **Emissão:** op `elevar {op, database, tabela, usos}` pela sessão já logada **mais** a senha de novo, pelo
  desafio-resposta que o login já usa (o PBKDF2 roda uma vez por elevação). Devolve o token **uma vez**.
- **Motor:** os 24 bytes de `phxsql_core::senha::bytes_aleatorios` em hexa, como o `Sessoes::nova`
  (`http.rs:645`); o servidor guarda **só o SHA-256** do token num mapa em memória, com teto e coringa como o
  `pode_avisar`. Conferência por `hash::iguais_em_tempo_constante`. Não vai a disco: reiniciar invalida todos,
  que é o certo.
- **Amarração:** login + IP + escopo (op, base, tabela ou `*` só se o pedido disser) + prazo **5 min**
  (o `timestamp_timeout` do `sudo`; teto 15) + **1 uso** (teto 10).
- **Plano largo:** o escopo carrega também o **número de linhas do plano** medido pela F8; se o plano mudar
  entre a recusa e o reenvio, o token não serve (é a confirmação em dois passos, pelo mesmo motor).
- **Redação:** o campo `elevacao` entra na forma do pedido como `?` (o Profiler e a ocorrência já redigem
  analisando); a resposta do `elevar` nunca se repete no log de acessos.
- **Fora da régua:** os três maduros não têm isso; é produto, **já decidido pelo dono na 767**. Referências:
  Oracle DV (regra por comando + fator), `sudo` (timestamp), RFC 9470 (`max_age`).

### 4.4 IP nunca visto (765.b)

`ips-vistos.jsonl` no diretório do sistema, chave (usuário, base, IP canônico), `primeiro`/`último`, teto
50.000 chaves + coringa. **Semente do `acessos.log`** no primeiro arranque (`LogAcessos::resumo_por_ip` já
existe), para não acusar todo mundo no dia em que liga. Primeira vez = login com sucesso nunca visto em 90 dias
→ ocorrência amarela `IpNovo` (login e IP só na tela do administrador, decisão LGPD do dono de 09/10). Em
`proteger`: a sessão de IP novo pede passo acima até para comando perigoso que o perfil já aceitava.
A mesma memória responde ao eixo d: **IP com ≥ 2 usuários distintos em 30 dias é compartilhado.**

### 4.5 Perfil habitual (765.c)

Por usuário: contagem por (categoria de op — as do pgaudit: ler, escrever, ddl, papel, outros — × tabela) e
histograma de 24 horas UTC (a tela converte; o motor não tem fuso, A0). Ocorrência `ForaDoPerfil` só depois de
**7 dias e 200 pedidos** do usuário, e só para combinação nunca vista ou hora com < 1% da massa. **Só observa**
(c1/c2 mortas). Números 7/200/1% **raciocinados, não medidos** — falta corpo de tráfego real (L2 do 495).

### 4.6 Bloqueio por código malicioso, como regra do firewall do SO (766)

- **Entrada:** a ocorrência `InjecaoSuspeita` (F3, 4 classes) passa a contar no `violacao_leve` quando
  `protecao.bloquear_por_codigo` = true (**nasce false** — guarda nova pedida; mesmo motivo escrito no
  `contar_injecao_sql`). O `contar_injecao_sql` velho (só o empilhado recusado) vira caso particular; uma
  política só.
- **Guardas de não se trancar (P9):** loopback (`is_loopback` no IP canônico — convergência MySQL host cache +
  fail2ban `ignoreself`), whitelist (já), IP com sessão de administrador viva ou login de administrador em 24 h,
  IP compartilhado (§4.4) → **encerra a sessão do usuário e emite ocorrência**, sem firewall.
  O loopback é isento **no caminho automático e no firewall**; o caminho velho (senha errada etc.) fica como é
  — 5 arquivos de teste de `servidor/` citam bloqueio e 127.0.0.1 (contado por `grep`; se dependem de bloquear o loopback, não conferido).
- **Escalonamento (P10):** `bloqueio_minutos × 2^n`, n = bloqueios anteriores do IP em 30 dias (persistido no
  `blacklist.json`), `n ≤ 20` como o fail2ban, teto **7 dias**. Sem `rndtime` nesta versão (raciocinado: o
  atacante que calcula o fim do prazo ganha 1 tentativa a cada 2^n × 60 min).
- **Forja de IP / negação por falso positivo:** em TCP o handshake impede forjar a origem sem estar no caminho
  (raciocinado; o detector só roda **depois** do login, então quem acusa tem credencial). O risco real é o IP
  de muitos (NAT/proxy) — fechado pela guarda do compartilhado. O servidor **não lê** `X-Forwarded-For`
  (0 ocorrências em `crates/`): o IP vem só do `peer_addr`. Atrás de proxy, todos são o proxy — que é IP
  compartilhado e nunca vai ao firewall.
- **Por onde passa:** o gancho do 638 pelo lançador do 759 (`gancho::rodar`: `env_clear`, `PATH` fixo, prazo,
  `kill`, saída descartada, fora do mutex da lista). Placeholders novos: `{ip}` **reserializado** de
  `IpAddr::to_canonical()` (hoje entra a string original validada — seguro, mas `::ffff:a.b.c.d` chega ao
  `netsh`/`iptables` na forma v6), `{familia}` (`4`/`6`) e `{segundos}` (de `u64`).
- **Privilégio:** o PhxSql nunca root. O pacote traz um ajudante `phxsql-fw` (dono root, 0755) que **analisa o
  IP de novo** e só aceita `add|del|list`, e uma linha de `sudoers` sem curinga de argumento livre apontando
  para ele. No Windows, o serviço roda numa conta com o direito de regra de firewall, ou um serviço do SO
  separado; o PhxSql chama `netsh advfirewall firewall add rule name=phxsql-{ip} dir=in action=block
  remoteip={ip}`.
- **Plataformas:** Linux nftables (`add element inet phxsql negros{familia} { {ip} timeout {segundos}s }`),
  reserva iptables+`ipset ... timeout`; Windows `netsh`/`New-NetFirewallRule` (**sem prazo no SO** — quem
  desfaz é o PhxSql).
- **Ciclo de vida (P11):** vencimento (o varredor de `servico_avisos_01.rs:380` já solta), escalonamento
  (re-`add` com prazo maior), liberação manual (`servico_admin_01.rs:157` já solta), e **reconciliação no
  arranque** pelo comando `listar` novo: a saída é **analisada** token a token como `IpAddr`; o que não é IP é
  ignorado; IP no SO e não ativo na lista → `del`; ativo na lista e ausente no SO (reboot esvazia o conjunto) →
  `add` com o prazo restante. No nftables o kernel já expira sozinho; no Windows é a reconciliação que limpa.

---

## 5. Onde diverge da origem, e a restrição que causou

| origem | aqui | restrição nossa |
|---|---|---|
| prazo de comando nasce 0 e o PG desaconselha global | nasce 0 em `observar`; em `proteger` a linha «parar o banco» traz `prazo_ms` próprio | **trava de dados global**: lá um SELECT lento não para os outros (MVCC); aqui para |
| MySQL/MaxScale protegem por allowlist de digital | protegem por **lista de comandos perigosos** + plano medido | medição (98,9%) e a ordem do dono (a lista é o inventário dele) |
| fail2ban roda como root e fala com o firewall direto | ajudante de um comando, IP reanalisado nele | PhxSql nunca root; zero dependências (nada de biblioteca de netlink) |
| fail2ban: uma regra ou `ipset` por jail | conjunto com prazo no kernel e reconciliação | regra órfã de processo morto; integridade do que o SO diz |
| Oracle DV: regra por comando com fator | linha por op + token de passo acima de uso único | o «mesmo motor» (pétrea): a confirmação do plano largo é o mesmo token |
| `sudo`: timestamp vale para qualquer comando | escopo (op, base, tabela, nº de linhas) e 1 uso | o perigo é por comando, não por pessoa |
| MySQL: bloqueio por erros de **conexão** | por **código malicioso** pós-login, com guarda do IP compartilhado | ordem 766; quem acusa tem credencial, então o NAT é o risco, não a forja |

---

## 6. Fatias para o papel B — prova (RED) nos dois sentidos

| # | o quê | prova (RED) | escalão |
|---|---|---|---|
| **P0** | **defeito ativo:** todo bloqueio (leve e grave) emite `FirewallBloqueou` pelo produtor único e vira pedra | 5 senhas erradas → 1 ocorrência `FirewallBloqueou`; tirar a chamada → teste falha | médio |
| P1 | `protecao/mod.rs`: `analisar` único no topo de `executar_e_contar_escrita_local`; portão por `match` antes do trabalho | `config.json` sem a seção → resposta byte a byte a de antes (teste do comportamento **velho**); op fora da lista custa ≤ 1 comparação (bancada) | forte |
| P2 | prazo de comando **fora** de transação pelo mesmo `prazo_ate_ms`/`siga` | varredura de 200 k linhas com `prazo_ms` 10 ms em `proteger` → `Cancelado`, e um 2º cliente pega a trava; em `observar` → completa e emite 1 ocorrência; prazo no relógio de outro lugar → teste de «um relógio só» falha | forte |
| P3 | plano largo e cascata larga (F8) em `proteger` pedem passo acima com o nº de linhas no escopo | `UPDATE … WHERE id > 0` 2.000/2.000 → recusa e **0 linhas mudadas** (conferir `rowstamp`); com token → 2.000; token de plano de 2.000 com plano agora 2.001 → recusa | forte |
| P4 | DDL destrutiva/reescrita acima do piso em `proteger` | `excluir_tabela` de 1.000 linhas sem token → os 5 arquivos intactos; com token → some; `DROP TABLE` pelo `sql` cai igual | forte |
| P5 | os caminhos escondidos de §4.1 | REST, MCP, job, rotina com DROP, gatilho AFTER com DELETE largo: cada um recusado em `proteger`; `aplicar` na réplica **não** pede token | forte |
| P6 | `ips-vistos.jsonl` + semente do `acessos.log` + `IpNovo` | 1º login de IP novo → 1 ocorrência; 2º → 0; IP já no `acessos.log` → 0; 50.001 IPs → coringa, mapa não cresce | médio |
| P7 | perfil por usuário, só observa | 300 leituras em `clientes`, depois `excluir` em `folha` → `ForaDoPerfil`; mesma leitura → 0; usuário com n < 200 → 0 | médio |
| P8 | `InjecaoSuspeita` conta no `violacao_leve` sob `bloquear_por_codigo` (nasce false) | desligado: 50 tautologias → 0 bloqueio; ligado: 5 de 203.0.113.9 em 10 min → bloqueio | forte |
| P9 | guardas de não se trancar | loopback, whitelist, IP com admin logado, IP com 2 usuários em 30 d: cada um → **nenhuma** regra e a sessão do usuário encerrada; tirar a guarda → teste falha | forte |
| P10 | escalonamento ×2^n, n ≤ 20, teto 7 d, persistido | 3º bloqueio = 240 min; 25º = 7 d sem estouro; reinício preserva n | médio |
| P11 | firewall do SO: `{ip}` canônico, `{familia}`, `{segundos}`, `listar` analisado, reconciliação no arranque; ajudante `phxsql-fw` + `sudoers` no pacote | **injeção:** IP `127.0.0.3 }; delete table inet vitima; …` → `aplicar` recusa e a tabela `vitima` **existe** (contra o `nft` real, em `unshare --net`); sem o `parse` → a tabela some (M4 é o RED já visto). **SO:** SYN do IP bloqueado não chega, outro chega, após o prazo volta (M5). **Reconciliação:** `kill -9` com bloqueio ativo, `nft flush set` (reboot) → no arranque o elemento volta com o prazo restante; elemento sem bloqueio → removido. **Windows:** `netsh` **não exercitado** neste contêiner — a fatia diz isso e roda no CI Windows ou fica PENDENTE | forte |
| P12 | tabela `phxsys.protecao` semeada só se falta; job com escopo pré-autorizado (o dono do job eleva uma vez para o job, com teto de usos) | semear duas vezes não sobrescreve a escolha; job sem escopo em `proteger` não roda e emite ocorrência | médio |
| P13 | a guarda guarda a si mesma | com uma linha em `proteger`, `observar`/`desligado` sem token → recusa e linha intacta; **subir** proteção não pede token; toda mudança emite ocorrência vermelha | forte |
| P14 | `elevar` + mapa de SHA-256 em memória, prazo 5 min, 1 uso, escopo, IP | 2º uso → recusa; outro IP → recusa; tabela B com token de A → recusa; vencido → recusa; `grep` do token em `acessos.log`, `ocorrencias`, Profiler e resposta de outra op → **0** | forte |
| P15 | tela (fábrica de idiomas), `docs/SEGURANCA.md`, `MANUAL`, cognição | conferidor de textos sem subir catraca | leve |

Ordem: P0 (defeito, na conta) → P1 → P14 → P2/P3/P4/P5 → P12/P13 → P6/P7 → P8/P9/P10/P11 → P15.

**Andamento P2–P5 (09/10/2026, papel B).** Provas em `servidor/testes_do_prazo_de_comando.rs` e
`servidor/testes_dos_caminhos_da_protecao.rs`, e oito guardas no catálogo.

- **P2:** `protecao.prazo_comando_ms` (fábrica 0, porque a L1 não foi medida) e
  `protecao.prazo_comando_modo` (`proteger` cancela; `observar` emite uma `PrazoEstourado` e deixa
  terminar). O prazo é armado a cada pedido no `despachar` e usa o relógio do STATEMENT TIMEOUT,
  do qual é **teto**.
- **P2, achado:** o prazo de uma transação já confirmada vazava para o pedido seguinte. Corrigido
  pelo mesmo armar.
- **P3/P4:** a mecânica já tinha entrado na P1. Esta fatia acrescentou as provas com o número.
  - O aceite «conferir `rowstamp`» virou conferir **versão e conteúdo**: o `rowstamp` não muda no
    `atualizar`.
  - O aceite «token de 2.000 com plano 2.001» **caiu** pela precisão do dono: a liberação é da
    sessão, e não um token com escopo.
  - `DROP TABLE` pelo SQL não existe. A linguagem recusa.
- **P5:**
  - REST (P1), MCP, job (4009), motor das rotinas (CALL e gatilho AFTER, que a linguagem de hoje
    não deixa escrever DROP/DELETE) e réplica isenta.
  - A catraca `TETO_EXECUTAR_DIRETO` reprova `.executar(` novo.
  - **Achado:** a ponte MCP somente de leitura oferece `phx_sql`, que escreve (cognição de
    09/10 19:30).
- **Limite:** sem telemetria ligada não há relógio, e o prazo não morde.

---

### 6.1 O que o papel B entregou de P6, P9, P10 e P11 — e o RED (09/10/2026)

Cada linha: o defeito reposto e os testes que caíram com ele (medido por um
script da frente que aplica a troca, roda só os testes nomeados e restaura;
as mesmas trocas viraram entradas do `bancada/guardas/catalogo.py`). **26 de
26 trocas derrubaram o teste que as acusa**; o 27.º, `fw-pendura-sem-premissa`,
é de teste de integração e foi provado pelo provador.

| fatia | defeito reposto | caiu |
|---|---|---|
| P6 | sem a chamada `ip_visto_no_login` no `login` | `o_primeiro_login_de_um_ip_novo_gera_uma_ocorrencia_e_o_segundo_nenhuma` |
| P6 | semente vazia no lugar do `acessos.log` | `o_ip_que_ja_esta_no_acessos_log_nao_gera_ocorrencia` |
| P6 | memória só em processo (o arquivo não recebe a linha) | os dois «segundo login não é novo», depois do reinício |
| P6 | sem o teto de 50.000 | `com_o_teto_cheio_o_mapa_nao_cresce` |
| P9 | sem o ramo do loopback | `cinco_tokens_errados_do_loopback_nao_bloqueiam_o_loopback`, `o_loopback_bloqueado_antes_da_guarda_entra_de_novo` |
| P9 | `poupar_loopback` sem leitor | `os_interruptores_novos_vem_do_config` |
| P9 | sem a guarda do administrador | `o_ip_de_quem_administra_nao_e_bloqueado` |
| P9 | sem a guarda do compartilhado | `o_ip_de_dois_usuarios_nao_e_bloqueado` |
| P9 | sem a whitelist editável na guarda única | `whitelist_por_cidr_e_a_dinamica_do_arquivo` |
| P9 | guarda que poupa todo mundo (comportamento velho) | `o_ip_desconhecido_e_o_de_um_usuario_so_continuam_bloqueando` |
| P10 | histórico fora do `blacklist.json` | `o_terceiro_bloqueio_dura_240_e_o_vigesimo_quinto_sete_dias`, `o_bloqueio_reincidente_dobra_e_o_reinicio_preserva_a_conta` |
| P10 | fator sempre 1 | `o_prazo_escalona_por_dois_ate_sete_dias` |
| P10 | histórico sem teto | `o_historico_de_reincidencia_tem_teto` |
| P11 | IP cru no argv (o M4) | `os_marcadores_saem_do_endereco_analisado`, `o_nft_de_verdade_recusa_a_injecao_e_reconcilia` |
| P11 | IP não canônico | `os_marcadores_saem_do_endereco_analisado` |
| P11 | firewall sem a conferência de caminho absoluto | `o_firewall_ligado_exige_programa_por_caminho_absoluto` |
| P11 | reconciliação que não tira o órfão / não devolve o ativo | `a_reconciliacao_poe_o_que_falta_e_tira_o_que_sobra`, `o_nft_de_verdade_recusa_a_injecao_e_reconcilia` |

O teste contra o `nft` de verdade (`unshare --net`) carrega o **controle
positivo dentro dele**: o mesmo argv com o texto cru no lugar do `{ip}` apaga a
tabela `controle` a cada corrida — o M4 reposto, sem depender de mutação.

**Divergência do desenho, com o motivo.** O §4.6 dizia que o loopback é isento
«no caminho automático e no firewall» e que «o caminho velho (senha errada etc.)
fica como é». O caso medido em 09/10/2026 é justamente do caminho velho (cinco
tokens errados de `127.0.0.1` trancando a tela e a TV por 60 min), então a
guarda vale para **todo** caminho de bloqueio — leve, grave e o `barrado()` —,
com `poupar_loopback: false` devolvendo o comportamento de antes. Os dois testes
de soquete que dependiam do bloqueio do loopback ganharam o escape escrito; um
deles passou a passar por engano antes do escape (cognição de 09/10/2026 18:30).

**De fora, e por quê:** encerrar a sessão do usuário do IP poupado (nos caminhos
de hoje não há sessão autenticada no limite; entra com a P8); o ajudante
`phxsql-fw` + `sudoers` do pacote (empacotamento, não código do servidor); o
SYN do IP bloqueado medido pela suíte (o M5 deste desenho continua sendo a
medida); o `netsh` do Windows, não exercitado neste contêiner — PENDENTE.

### 6.2 P12, P13 e a brecha do primeiro cadastro — o que o papel B entregou, e o RED (10/10/2026)

Provas em `servidor/testes_da_guarda_da_protecao.rs`, `protecao::testes`,
`jobs::testes` e `config::testes_da_protecao`; 19 entradas novas no catálogo,
provadas pelo provador com `--so` (verde na árvore limpa, cada defeito reposto
derruba o `caem` e deixa o `seguem` de pé).

**Decisões desta fatia, com o motivo:**

- **O modo da linha liga e desliga o MONITORAMENTO; a senha segue exigida.**
  A ordem do 767 («a tabela habilita ou não o monitoramento») e a precisão
  «sem a senha esses comandos não executam, em qualquer modo» só cabem juntas
  assim. Se a linha `observar`/`desligado` também DISPENSA a senha é **produto
  e sobe ao dono**; implementado atrás de `protecao.modo_dispensa_a_senha`,
  fábrica `false` (o lado seguro).
- **A tabela nasce pelo `protecao_semear`, não no arranque.** Medido: semear
  em todo arranque derrubou **9** testes do comportamento velho (listas de
  bases e tabelas; `phxsys nao pode nascer sem alguem pedir`). A ausência já
  vale `proteger`; o arranque só **completa** a tabela onde `phxsys` existe.
- **O leitor é o motor da grade** (`varrer_a_pagina`), e dentro da trava (o
  plano da cascata) a ficha vai adiante — a leitura pelo `varrer` lá dentro
  seria reentrante e cairia calada em `proteger` (cognição de 10/10 14:00).
- **O job:** `job_autorizar` por quem administra o servidor com a sessão
  liberada; escopo = SHA-256 de usuário+pedido; teto 366 corridas e 366 dias;
  a corrida gasta um uso antes de rodar; o `job_salvar` nunca traz a
  autorização. «O dono do job» virou «quem administra»: todas as ops de job já
  pedem administrar, e quem autoriza prova a PRÓPRIA segunda senha.
- **A brecha do primeiro cadastro:** fechada atrás de
  `protecao.primeiro_cadastro_pelo_administrador` (fábrica `true`) — é produto
  (muda a entrada de cada usuário) e sobe ao dono. Só o primeiro administrador
  do servidor se cadastra sozinho; risco residual nomeado: servidor recém-criado.
- **Hipótese que morreu medida:** «o `aplicar` é porta dos fundos da guarda».
  Fora da réplica o papel já o recusa (4001) —
  `o_aplicar_na_origem_ja_recusa_pelo_papel`; ele segue isento, como no P5.

| guarda | defeito reposto | caiu |
|---|---|---|
| `guarda-sem-guarda` | sem o `toque_na_guarda` | baixar pelo JSON/SQL; os caminhos opacos |
| `subir-a-guarda-pede-senha` | `Sobe` vira `PodeBaixar` | subir; `o_toque_na_guarda`; o torto que passa |
| `modo-torto-baixa-a-guarda` | torto vale `desligado` | o leitor estrito; o torto do P13 |
| `linha-desligada-ainda-monitora` | o modo não decide a trilha | `a_linha_desligada_tira_da_trilha…` |
| `linha-baixa-dispensa-sem-interruptor` | dispensa sem o interruptor | idem |
| `tabela-da-protecao-cega-sob-a-trava` | leitura sob a trava devolve erro | a cascata com dispensa |
| `semear-a-protecao-por-cima` | semear sem conferir o que existe | semear de novo |
| `arranque-cria-o-sistema` | o arranque cria `phxsys` | `o_arranque_completa…` |
| `arranque-nao-completa-a-protecao` | o arranque não semeia | idem |
| `job-autorizado-sem-liberacao` | a corrida não libera | o job autorizado; o escopo |
| `autorizacao-do-job-sem-teto` | o uso não gasta | o job; `a_autorizacao_vale…` |
| `autorizacao-herdada-por-outro-pedido` | `salvar` herda sempre | o escopo |
| `autorizacao-perdida-ao-regravar` | `salvar` nunca herda | o escopo |
| `autorizacao-pela-rede` | `job_salvar` aceita o campo | a forjada |
| `autorizacao-sem-impressao` | sem conferir a impressão | `a_autorizacao_vale…` |
| `job-autorizar-sem-a-senha` | `job_autorizar` sem `julgar` | o job |
| `primeiro-cadastro-pela-senha-de-login` | sem a regra | `o_primeiro_cadastro_e_do_administrador` |
| `segundo-administrador-se-cadastra-sozinho` | sem conferir o cofre | idem |
| `interruptores-do-767-sem-leitor` | o campo não é lido | `os_interruptores_do_767…` |

**De fora, e por quê:** a tela (botões de semear, de autorizar o job e de
baixar a linha) e o MANUAL — P15; a isenção da réplica no P13 provada contra
um nó réplica de verdade (o teste usa só o papel isolado); o `restaurar_backup`
de `phxsys` provado de ponta a ponta (provado só na classificação).

## 7. Choque com pétrea?

**Nenhum.** «Guarda nova entra pedida»: tudo nasce `observar` (não muda resposta), o `bloquear_por_codigo`
nasce desligado, o firewall já nascia desligado. Zero dependências: nftables/ipset/netsh são programas do SO
chamados pelo gancho que já existe, não crates. Senha nunca em texto puro: o token de passo acima mora só como
SHA-256 e sai uma vez.

## 8. O que sobe ao dono — só produto

1. **A promessa ao cliente com `proteger`.** Em 09/10 o dono decidiu «detecta e avisa», e «não se escreve
   bloqueia». Com `proteger` ligado pelo dono do banco, o motor **recusa** o comando perigoso sem passo acima e
   o 766 **bloqueia** o IP no firewall. A pergunta: o material pode dizer «bloqueia, quando o modo proteger
   estiver ligado»? Padrão sem resposta: continua «detecta e avisa».

Não sobe: o padrão de fábrica (`observar`, pela pétrea); os três empates 5 × 5 (decididos pela própria ordem
765/766 ou mortos medidos antes da régua); o passo acima (produto já decidido na 767).

## 9. Lacunas

- **L1** tempo que uma varredura sem prazo segura a trava global — não medido; decide o `prazo_ms` sugerido.
- **L2** corpo de tráfego real: M3 é proxy (código de teste, não carga). 7 dias / 200 pedidos / 1% do perfil
  são raciocinados.
- **L3** Windows (`netsh`, `New-NetFirewallRule`) não exercitado.
- **L4** iptables+ipset de reserva não exercitado (só o `nft`).
- **L5** rotina com DROP e gatilho AFTER chegarem à camada: lido no comentário, não provado (P5).
- **L6** MySQL Enterprise Firewall e Oracle DV: documentação, não fonte (produtos fechados). ProxySQL: a página
  não traz as colunas. Cassandra: não consultado — não tem firewall de SQL.
- **L7** revisão cruzada dos subagentes faltou (dispensados por falta da ferramenta).

## 10. Aprendizados (PENDENTE)

- «Sem shell» não protege um programa que tem a própria linguagem de comando: o `nft` executa `;` vindo de um
  único argumento (M4). A barreira é analisar o IP. PENDENTE até o RED da P11 rodar no CI.
- Alarme declarado sem produtor é guarda que não guarda e não aparece em nenhum teste (M1). PENDENTE até a P0.

---

## Apêndice A — M3 (novidade de perfil)

`cd bancada/seguranca/495 && python3 extrair_legitimo.py` e então: agrupar `legitimo.jsonl` por arquivo de
origem (≥ 4 comandos), aprender a primeira metade, contar na segunda o que é novo por digital (literal → `?`,
espaços), por (1ª palavra, tabela depois de FROM/INTO/UPDATE/TABLE/JOIN) e só pela 1ª palavra.
Resultado de 09/10: 53 «usuários», 662 testados — 655 / 268 / 125.

## Apêndice B — M4 e M5 (`unshare --net`, root)

Subir o `lo` por `ioctl(SIOCGIFFLAGS/SIOCSIFFLAGS)` (o contêiner não tem `ip`); `nft add table inet phxsql`;
`nft add set inet phxsql negros4 '{ type ipv4_addr; flags timeout; }'`; corrente `input` prioridade −10 com
`tcp dport 6790 ip saddr @negros4 drop`; ouvinte em `0.0.0.0:6790`; cliente com `bind` em 127.0.0.3 (alvo) e
127.0.0.2 (testemunha), prazo 1 s; `add element … { 127.0.0.3 timeout 3s }`; esperar 3,5 s. Injeção: o mesmo
`add element` por `subprocess.run(argv)` sem shell, com o IP do M4 no último argumento, e `nft list tables`
depois. Os scripts da corrida ficaram no scratch da sessão; a P11 os traz para `bancada/seguranca/766/`.
