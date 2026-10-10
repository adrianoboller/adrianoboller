# Pedido 770 — os padrões de fábrica endurecidos, com a régua

Papel J (ciclo embutido na frente do papel B), 10/10/2026. Decisão do dono, 09/10/2026:
endurecer os padrões **mesmo quebrando cliente antigo** — exceção explícita, só para este
pedido, à pétrea «guarda nova entra pedida, não imposta». A pétrea não é revogada.

O que o dono decidiu é **se** endurece (produto). O que a régua decide é **o valor** e **o
detalhe**: convergência dos três maduros entra sem pergunta; divergência pela média ponderada
**PG 4, MariaDB 3, MySQL 2, SQLite 1**. Toda config que **declara** o valor velho por escrito
continua valendo, com um aviso `770:` no arranque; o que muda é o padrão de quem não declarou.

## As fontes lidas

| Motor | Onde | O que diz |
|---|---|---|
| PostgreSQL 16 | `postgresql.conf.sample` e `pg_hba.conf.sample` do pacote instalado neste contêiner (`/usr/share/postgresql/16/`) | `#ssl = off`; `#listen_addresses = 'localhost'`; `#idle_session_timeout = 0`; `#scram_iterations = 4096`; o `pg_hba` de fábrica tem `host all all 127.0.0.1/32` **sem** `hostssl` |
| PostgreSQL 17 | `contrib/passwordcheck/passwordcheck.c` (REL_17_STABLE) e `passwordcheck.so` do 16 | `#define MIN_PWD_LENGTH 8`; «password must not equal user name»; «must not contain user name»; «must contain both letters and nonletters». Módulo **não** carregado de fábrica |
| MySQL 8.4 | refman `validate-password-options-variables.html` e `server-system-variables.html` | `validate_password.length` 8, `.policy` 1 (MEDIUM), `mixed_case_count` 1, `number_count` 1, `special_char_count` 1, `check_user_name` ON («the same as the user name or its reverse»); `require_secure_transport` OFF («only TCP/IP connections encrypted using TLS/SSL, or connections that use a socket file … or shared memory»); `auto_generate_certs` ON; `wait_timeout` 28800 |
| MariaDB 11.4 | `plugin/simple_password_check/simple_password_check.c` (ramo 11.4) e a página do plugin | `minimal_length` 8, `digits` 1, `letters_same_case` 1, `other_characters` 1; recusa «the password equal to the user name» (`strncmp`); plugin **não** instalado de fábrica. `require_secure_transport` OFF (secure = TLS, socket Unix ou named pipe). 11.4 liga o TLS de fábrica com certificado gerado (blog «mission impossible: zero-configuration SSL», já citado em `plano-tls13-572.md`) |
| SQLite | — | embarcado, sem rede, sem usuário: não vota em nenhuma linha (peso 0 em todas) |
| NIST SP 800-63B-4 | `pages.nist.gov/800-63-4/sp800-63b.html` | **não é régua** — entra só onde os quatro não alcançam (#11, #12) e como dissidência registrada (#6) |

## A tabela

| # | Padrão | Hipóteses (escritas antes de medir) | Régua | Decisão | Muda? |
|---|---|---|---|---|---|
| 1 | Cifra exigida na porta de **dados** | H1 exigir; H2 não exigir | os três: **não exigem** (`ssl=off`, `require_secure_transport=OFF`) — convergência contra | **já exigida** desde 18/09 (ordem do dono, pedido 370); o 770 só confirma. Choque já resolvido pelo dono, não sobe de novo | não |
| 2 | Loopback TCP **isento** da exigência | H1 isento (a frase do pedido: «fora do loopback»); H2 não isento | MySQL 2 + MariaDB 3 = **5**: «secure transport» é TLS ou **IPC local** (socket, pipe, memória), TCP no 127.0.0.1 **não**; PG 4: o `pg_hba` de fábrica trata `host 127.0.0.1` como o `local`, sem `hostssl` → isento | **5 × 4 → não isento.** O PhxSql não tem IPC local, então não há o que isentar. **H1 morreu.** O loopback continua utilizável porque o TLS de fábrica (#3) o atende, não por exceção | não (já era assim) |
| 3 | TLS nativo nas portas **HTTP** (`web`, `rest` e o explorador) | H1 nasce ligado, autoassinado; H2 nasce desligado (a porta recusa todo pedido sob `exigir`) | MySQL `auto_generate_certs` ON (2) + MariaDB 11.4 gera e liga (3) = **5** × PG `ssl=off` (4) | **5 × 4 → nasce ligado.** Exceção: seção com `"atras_de_proxy": true` nasce **sem** TLS — quem termina é o proxy, e TLS para ele quebraria a ponte. **H2 morreu**: sob `exigir` ela era uma porta que não servia a ninguém | **sim** |
| 4 | TLS nativo na porta de **dados** | H1 nasce ligado (mesma régua do #3, 5 × 4); H2 fica pedido | a régua é do **comportamento** (conexão cifrada de fábrica), não do meio | o comportamento **já é entregue** pelo túnel do fio exigido (#1). Ligar o TLS aqui gravaria um par de chaves ao lado do `config.json` em todo arranque, por nada que o cliente ganhe. **H1 morreu** com este motivo | não |
| 5 | Comprimento mínimo da senha | H1 8; H2 15 (NIST 800-63B-4, fator único); H3 sem mínimo | sem módulo, os três **não têm política** (convergência em H3) — mas «senha forte» é decisão do dono, então a régua decide o **valor** dos módulos: PG 8, MySQL 8, MariaDB 8 → **convergência** | **8**, em caracteres (não bytes). H2 morreu: não está em motor nenhum. H3 morreu pela decisão do dono | **sim** |
| 6 | Composição | H1 quatro classes (maiúscula, minúscula, algarismo, símbolo); H2 letra + não-letra; H3 nenhuma | MySQL MEDIUM 2 + MariaDB 3 = **5** (H1) × PG `passwordcheck` 4 (H2) | **5 × 4 → quatro classes**, por classe Unicode («Ação2026!» passa). Dissidência registrada: o NIST 800-63B-4 diz *SHALL NOT impose composition rules* — fica escrito, não decide, porque não é régua | **sim** |
| 7 | Senha igual ao login | H1 recusar igual; H2 recusar se **contém**; H3 recusar também o **reverso** | igual: PG + MySQL + MariaDB → **convergência**; contém: só PG, 4 × 5; reverso: só MySQL, 2 × 7 | **recusa a igual**, comparação exata. **H2 e H3 morreram** | **sim** |
| 8 | Senha vazia | — | os três não autenticam senha vazia | já recusada no `usuario_criar`/`alterar`, no `--senha` e no `config.json` | não |
| 9 | Senha que já está gravada | H1 conferir no login; H2 conferir só na definição | os três conferem **só na definição** → convergência | **H2.** H1 trancaria fora quem já trabalha, e morreu | — |
| 10 | Expiração periódica da senha | H1 expirar; H2 não | `default_password_lifetime` 0 no MySQL e no MariaDB; PG sem `VALID UNTIL` → **convergência** | **não expira.** H1 morreu | não |
| 11 | Sessão web **sem uso** | H1 60 min (hoje; NIST-4 AAL2 «≤ 1 h»); H2 30 min (NIST-3 AAL2); H3 15 min (NIST-4 AAL3) | **os quatro não alcançam**: não têm sessão de navegador; o análogo mais perto (`wait_timeout` 8 h, `idle_session_timeout` 0) mede conexão ociosa e iria contra «curta» | **15 min** (H3), pela norma, porque o dono pediu «curta» e «alta segurança». **Sobe ao dono para confirmar o número** | **sim** |
| 12 | Sessão web, **teto absoluto** | H1 sem teto (hoje); H2 12 h (NIST AAL3); H3 24 h (NIST-4 AAL2) | não alcança, idem #11 | **12 h** (H2). Sem teto, a tela que pergunta sozinha (aquário, painel na TV) renova a sessão para sempre — a inatividade nunca dispara. **Sobe junto com o #11** | **sim** |
| 13 | Modo proteger do 765 | — | — | já é o de fábrica (`protecao.ligada: true`, `prazo_comando_modo: proteger`, decisão do dono no 765) | não |
| 14 | Exigir a amarração ao canal (`exigir_amarra`) | H1 exigir de fábrica; H2 pedida | PG: o **cliente** decide (`channel_binding=prefer`), o servidor não exige; MySQL e MariaDB não têm → convergência em não exigir | **H2**, continua pedida. H1 morreu | não |
| 15 | Iterações do PBKDF2 | — | PG `scram_iterations` 4096 | as 210.000 de hoje já estão acima; nada a fazer | não |

## O que quebra, e como o cliente antigo se ajusta

| Quem | O que acontece agora | O ajuste |
|---|---|---|
| Interface web ou REST ligada, sem `tls` escrito e com `"exigir": false` | a porta passa a falar **https** (autoassinado); o `http://` deixa de responder | usar `https://`, ou escrever `"tls": false` na seção (aviso no arranque) |
| Interface web ou REST ligada, sem `tls` e com a exigência de fábrica | antes **recusava todo pedido**; agora atende por https | nenhum — passou a funcionar |
| Script que cria ou altera usuário com senha curta, sem as quatro classes, ou igual ao login | `usuario_criar`/`usuario_alterar` recusam, dizendo a regra e o campo | senha que passe, ou `"politica_de_senha": {"minimo": 0, "classes": false, "diferente_do_login": false}` (aviso no arranque) |
| Quem deixa a tela aberta | a sessão cai com 15 min sem uso e, de todo jeito, 12 h depois do login | `"sessao_minutos": 60` e `"sessao_teto_horas": 0` na seção `web` (aviso no arranque) |

## O que sobe ao dono

- **#11 e #12 — o que a pesquisa não alcança.** Os quatro motores não têm sessão de
  navegador; o número saiu da NIST 800-63B-4 (AAL3), não da régua. Implementado com 15 min e
  12 h; confirmar ou trocar é editar duas constantes (`SESSAO_MINUTOS_DE_FABRICA`,
  `SESSAO_TETO_HORAS_DE_FABRICA` em `config.rs`).

Nada mais sobe: os itens 1–10 e 13–15 a régua resolve, e nenhum bate em pétrea.

## As provas

Testes em `crates/phxsql-server/src/servidor/testes_dos_padroes_770.rs` e nos módulos do
`config.rs` e do `http.rs`, um par por mudança: o padrão novo e quem declara o velho por
escrito (continua valendo, com aviso). O RED de cada um, medido com o padrão velho reposto, está
na seção seguinte.

## O RED, medido

`provar-guardas.py --so` nas sete, 10/10/2026: **7 provadas, 0 não pegaram, 0 estragaram**.

| Guarda | Defeito reposto | Caíram |
|---|---|---|
| `770-web-sem-tls-de-fabrica` | `Web::de_json` com o TLS padrão `false` | 1/1 (pelo soquete, `tls-das-portas-http`) |
| `770-rest-sem-tls-de-fabrica` | `Rest::de_json` com o TLS padrão `false` | 1/1 |
| `770-sessao-sem-teto` | `Sessoes::usar` ignorando o teto | 1/1 |
| `770-sessao-de-60-min` | `SESSAO_MINUTOS_DE_FABRICA = 60` | 1/1 |
| `770-politica-de-senha-frouxa` | `PoliticaDeSenha::default()` em 0/false/false | 2/2 |
| `770-cadastro-sem-politica` | o cadastro sem chamar a política | 1/1 |
| `770-valor-velho-calado` | sem os avisos `770:` | 3/3 |
