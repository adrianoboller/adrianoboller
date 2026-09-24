# Pesquisa 495 — a IA do PhxSql analisa crime dentro do banco (papel J)

24/09/2026. Papel J, com a cláusula «o pesquisador decide; o dono é o impasse».
Base: `HEAD ba0e62a`; servidor medido: `target/release/phxsqld` das 08:02 — o
analisador e o avaliador (`crates/phxsql-sql`, `phxsql-core/src/expressao.rs`)
não mudaram desde 02:53 (`git diff --quiet c544247 HEAD -- …` limpo nesses
caminhos). Lido junto: `parecer-sec-495-ameaca-2026-09-24.md` (SEC) e o pedido
do integrador de camada comum com o 496.

Ordem do dono: *«a IA do PhxSql é importante para analisar crimes de
cibersegurança dentro do banco de dados. Evitando SQL injector ou qualquer coisa
que seja anormal.»*

---

## 0. Decisão, em seis linhas

1. **Detector estrutural no motor (H1) entra — só observa.** Nunca recusa, nunca
   bloqueia IP. Quatro classes, medidas com **1** acusação falsa em 1.186 SQL
   legítimos do repositório (contra **107** do mesmo detector feito por recorte).
2. **Firewall por lista permitida (H2) morre como recurso** (voto 2 × 8); a
   **impressão digital** que ele usa entra por convergência dos três maduros, e
   vira o texto redigido do evento e o que vai à IA.
3. **IA é analista da fila de eventos, nunca portão** — o portão custaria de
   463 ms a 14 s por consulta contra 0,9 µs do detector.
4. **Uma camada só para 495 e 496**: evento tipado → silêncio → fila →
   carteiro → canais que já existem. O 496 pluga produtores nela.
5. **Achado com número, antes de tudo:** o `comando_empilhado` do pedido 215
   acusa SQL de um comando só, e **bloqueou 127.0.0.1 por 60 min em 5 pedidos
   legítimos** com o interruptor ligado. Defeito ativo — fatia F0.
6. **A premissa do SEC morreu medida:** a tautologia do próprio arsenal do
   repositório **executa hoje e devolve todas as linhas** (2 de 2). Em 07/09 ela
   era recusada; a gramática cresceu, e o detector tem de olhar o **sucesso**.

Sobe ao dono **uma** coisa, e é de **produto** (§9).

---

## 1. Hipóteses, escritas antes de medir

| # | hipótese | previsão escrita antes | veredito medido |
|---|---|---|---|
| H1 | detector determinístico sobre o léxico do motor; registra por padrão, bloqueia só pedido | detecção alta, falso positivo baixo mas não zero | **entra**, 4 classes, só observa (§5) |
| H2 | impressão digital + lista permitida por usuário (molde MySQL Enterprise Firewall / MaxScale) | detecta tudo o que muda estrutura; falso positivo alto sem aprendizado | **morre** como firewall (voto 2 × 8; 94,3% de digital nova no corpo de teste); a digital **entra** |
| H3 | anomalia estatística por usuário/IP em `std` pura | barata; falso positivo não mensurável sem tráfego | **PENDENTE** — volume é cego (SEC R1); fica atrás da F4 |
| H4 | IA como analista do evento, nunca portão | portão morre por latência | **confirmada**: 5 a 7 ordens de grandeza (§4.5) |
| H5 *(J)* | a prevenção de verdade são os parâmetros `?`; o detector acha quem não os usa | — | **confirmada** por leitura (SEC §1.2) e pelos quatro motores (§3, linha A) |
| H6 *(J)* | detectar pelo léxico (analisar) acusa menos que pelo texto (recortar) | analisar < recortar | **confirmada**: 1 × 107 no legítimo; 0 × 30 no dado escapado |
| H7 *(J)* | o 215 só vê pedido recusado, e por isso não vê a injeção que deu certo | confirmar no código | **confirmada** (`servidor.rs:10381-10388`, `r.is_err()`), e agravada pela §2 |

---

## 2. A premissa que morreu primeiro

O SEC escreveu, por leitura: *«as classes clássicas (tautologia de comparação,
comentário de fim de linha, UNION de leitura) não são contadas — mas também não
executam, porque viram dado/erro de sintaxe»*. Medido contra o servidor de pé,
com o `ARSENAL` do próprio `bancada/seguranca/injecao.py`:

| pedido (do repositório) | ok | op | linhas |
|---|---|---|---|
| controle: `nome = ''` | sim | `buscar` | 0 |
| **arsenal «aspa solta / tautologia»** | **sim** | **`varrer`** | **2 de 2** |
| arsenal «aspa escapada como DADO» | sim | `buscar` | 0 |
| controle: sem `WHERE` | sim | `varrer` | 2 |

Em 07/09 (`docs/pdf/respostas/Y.md`) o mesmo texto voltava *«o WHERE aceita UMA
comparação»*. Hoje `onde_da_selecao` (`sintaxe.rs:1501`) manda o que não é
comparação simples para `Onde::Expressao`, e o avaliador executa. **Não é defeito
do motor** — PostgreSQL e MySQL executariam igual, porque quem concatenou foi o
cliente. É o motivo de o detector olhar o **caminho de sucesso**, e é um recado
para G/SEC: o caso 1 do `injecao.py` julga por «tabelas e linhas antes e
depois», e **não vê leitura** — a tautologia passa nele como PASSOU.

A parte do SEC que se sustenta: **o servidor não concatena nada** (parâmetro é
troca de token, `lexico.rs:363`). A injeção existe no cliente, e o banco só pode
**vê-la**.

---

## 3. Matriz dos quatro motores (e das duas fontes de fora)

Fonte primária de cada linha ao lado. **A** = o que o banco faz; o voto usa PG 4,
MariaDB 3, MySQL 2, SQLite 1.

| comportamento | PostgreSQL | MariaDB | MySQL | SQLite | régua | nós |
|---|---|---|---|---|---|---|
| **A. um comando por pedido preparado** | protocolo estendido: *«cannot include more than one SQL statement»* (doc 18, §54.2.3); o simples aceita vários | `CLIENT_MULTI_STATEMENTS` desligado por padrão (KB `mysql_real_connect`) | idem, e *«cannot be used with the prepared statement interface»* (C API 8.4) | `sqlite3_prepare` *«only compile the first statement»* | **convergem** | já temos: *«um comando por vez»* |
| **B. normalizar a consulta: literal vira marcador** | `pg_stat_statements` (contrib, `shared_preload_libraries`): constante vira `$1`; `queryid` da árvore pós-análise; lista `IN` espremida no 18 (não no 17) | `events_statements_summary_by_digest`, `DIGEST_TEXT` | digest do Performance Schema, SHA-256, *«literal values replaced»*; trunca em `max_digest_length` | `sqlite3_normalized_sql()`, só com `SQLITE_ENABLE_NORMALIZE` | **convergem** → aceite automático | entra (§5, F1) |
| **C. lista permitida que bloqueia/avisa no servidor** | não tem no núcleo | `dbfwfilter` era do **proxy** MaxScale, obsoleto na 2.6 e **removido na 22.08** (MXS-3189) | Enterprise Firewall, **edição comercial**, opt-in por perfil: RECORDING / PROTECTING / DETECTING | não tem (o autorizador é por **ação**, na preparação) | sim 2 × não 8 | **morre** (⏸) |
| **D. registro de auditoria das consultas** | `log_statement` (núcleo) + `pgaudit` (extensão, só registra) | `server_audit` (plugin), só registra | `general_log`; auditoria na edição comercial | não tem (só o gancho `trace`) | convergem: **existe e só registra** | o Profiler (desligado) e o `acessos.log` |
| **E. detector heurístico de injeção no núcleo** | não | não | não | não | 0 × 10 | entra **só por ordem do dono**, e por isso no modo mais conservador: observa |

Fora dos quatro:

- **libinjection** (BSD-3): tokeniza e casa uma «impressão» dos primeiros
  símbolos contra uma lista conhecida — e trabalha sobre o **parâmetro** que o
  usuário digitou, antes de virar consulta. **Diverge aqui:** o motor nunca vê o
  parâmetro isolado, só a consulta pronta; por isso as classes daqui são
  estruturais (o que a consulta inteira faz), e não uma lista de impressões.
- **OWASP, Prevention Cheat Sheet:** a defesa primária é consulta
  parametrizada; validação de entrada é *«secondary defense»*. É a H5.
- **Cassandra (base permanente):** a auditoria 5.0 registra 8 categorias e
  *«Actual values bound for prepared statement execution will not show up»*; não
  detecta nada. A senha no texto é tapada pelo `PasswordObfuscator`
  (`cql3/PasswordObfuscator.java`) por `indexOf("password")` e regex — **recorte**.
  **Diverge aqui**, pela pétrea *redigir analisando*: nosso evento leva a
  consulta **sem literal nenhum**, pelo léxico — a senha some porque é literal,
  não porque alguém lembrou o nome do campo.

**Onde a digital diverge da origem, e por quê:**

- **sem truncar.** O MySQL documenta: *«Statements differing only after the
  truncation point are considered identical.»* Numa digital de **segurança**,
  isso é um rabo invisível. A nossa percorre todos os símbolos (FNV-1a de 64 bits
  sobre o texto normalizado) — restrição nossa: integridade do evento.
- **pelo léxico, não pela árvore pós-análise** (como o `queryid` do PG): metade
  do que interessa aqui é o pedido que o analisador **recusou**, e ele não tem
  árvore. Restrição nossa: o evento existe para a tentativa, não só para a
  consulta que deu certo.

---

## 4. Números medidos, e como refazer

Corpo **legítimo**: 1.186 SQL distintos extraídos do código (literais Rust de
`crates/`, constantes Python de `bancada/` e `testes-web/`, literais JS da
`ui/`), mais os 20.000 do `bancada/comando.sql` à parte. Dos 1.186, 391 passam
no `analisar_comando` — o resto é DDL, rotina, transação, SQL de outro motor
(prova de DbLink, comparativos) ou teste de erro de propósito. Isso torna o corpo
**pessimista** para falso positivo, e é o que se quer.

Corpo de **detecção**: só o que o repositório já tem — o `ARSENAL` (12, dos quais
o próprio repositório rotula 7 como não-ataque) e os testes do
`comando_empilhado`. **5 ataques, de 2 classes.** Ver lacuna L1.

### 4.1 Falso positivo: analisar × recortar

| detector | acusa (forte) no legítimo | dos quais são testes de injeção de propósito | falso positivo forte |
|---|---|---|---|
| **analisar** (léxico do motor, 4 classes, 215 corrigido) | 8 | 7 | **1** — `SELECT 1 /* sem fim ?`, teste de comentário aberto que o léxico já recusa |
| **recortar** (as mesmas perguntas no texto cru) | 114 | 7 | **107** |

O «215 corrigido» foi contado sobre as 17 acusações do `comando_empilhado` de hoje: ficam as 6 que
têm `;` (todas testes de empilhado de propósito). O protótipo tinha também um empilhado **mais
largo** (qualquer símbolo depois de `;`), e ele acusava **mais 5** — roteiros de várias linhas
feitos para o MySQL/MariaDB das bancadas. Morreu por isso e pela lei do motor único: o
detector usa o `comando_empilhado` consertado, e não um segundo.

Dado escapado (os 12 textos do `ARSENAL` postos como **valor**, aspa dobrada, em
três modelos — o que uma aplicação segura manda): **analisar acusa 0 de 36;
recortar acusa 30 de 36.**

Classes **recusadas com número** (não entram):

| classe | acusações no legítimo | motivo |
|---|---|---|
| função fora do motor | 24 | quase tudo SQL de outro motor (`version()`, `RANK()`); o analisador já recusa |
| aspa aberta | 23 | 19 são corte do extrator (SQL montado por concatenação na bancada); as 4 reais são testes de erro; o `acessos.log` já guarda o código |
| comentário vazio como espaço | 1 | a única ocorrência é a que o próprio repositório rotula legítima: 100% falso |
| catálogo do sistema | 3 | as 3 são SQL de outro motor; reconhecimento pelas ops já está no `acessos.log` (SEC §4) |
| constante sob `AND` | 0 | zero acusação **e** zero detecção no corpo — sem prova nos dois sentidos; PENDENTE |

### 4.2 O defeito do 215 (vira F0)

`comando_empilhado` promete na doc *«só responde true quando há símbolo DEPOIS
de um ponto-e-vírgula»* (`sintaxe.rs:571-572`), e o código não confere que houve
`;`: `while p.aceitar(PontoEVirgula) {}` e depois `espiar().is_some()`. No corpo
legítimo ele acusa **17**: 6 empilhados de verdade e **11 comandos únicos** com
sobra — `RETURNING id`, `LIMIT 0, 2`, `EXCEPT`, `INTERSECT`, `INTO OUTFILE`,
`IF NOT EXISTS`. Pelo soquete, com `contar_injecao_sql: true` e a política de
fábrica do `injecao.py`:

```
5 pedidos legitimos de bancada/gaps-sql/sondar.py (RETURNING, LIMIT 0, 2, EXCEPT)
-> bloqueio: 127.0.0.1, motivo "comando SQL empilhado", 60 min, firewall: true
-> ping seguinte: acesso negado
```

O caso 5b do `injecao.py` media zero bloqueio porque o `exercitar.py` não tem
nenhum desses. `LIMIT a, b` é a sintaxe de todo cliente MySQL — ligado, o
interruptor derruba o cliente honesto pela porta do pedido 203.

### 4.3 Impressão digital

| corpo | textos | digitais |
|---|---|---|
| `bancada/comando.sql` (carga repetitiva, o jeito de uma aplicação) | 20.000 | **1** |
| legítimo do repositório (feito para ser diverso) | 1.083 lexicáveis | 988 |
| aprende metade, testa a outra | 564 | **532 novas (94,3%)** |
| os 5 ataques contra o modelo de onde saíram | 5 | 5 diferentes |

Leitura: a lista permitida **só serve a quem tem consultas fixas** (1 digital para
20.000); num console, 94,3% do que chega seria «novo». É o que o MySQL resolve
com treino por perfil, e o que o voto da linha C recusa.

### 4.4 Custo (protótipo, ns por comando, 41 voltas, 49 bytes médios)

| o quê | mín / mediana / máx |
|---|---|
| `analisar_comando` — o que a op `sql` já paga | 2.192 / 2.262 / 2.654 |
| detector **relexando** + o 215 reanalisando | 4.787 / 4.861 / 6.671 |
| **detector com os símbolos já prontos** | **867 / 885 / 1.085** |
| impressão digital (normalizar + FNV-1a) | 714 / 741 / 816 |
| **desligado: leitura de um `AtomicBool`** | **0,35 / 0,37 / 0,44** |

Em escala: um pedido `sql` pelo soquete leva **322 µs** de ida e volta (mediana de
5 × 2.000; o `ping` leva 40 µs). O detector com os símbolos reaproveitados é
**~0,3%** do pedido — e relexar custaria 2,1× o próprio parse, que é o que o SEC
M4 previa. O protótipo aloca uma `String` maiúscula por símbolo
(`palavra_chave()`); a versão do motor compara sem alocar, então 885 ns é teto.
A máquina tinha outras compilações rodando: a razão detector/parse, medida na
mesma corrida, vale mais que o ns absoluto.

### 4.5 IA no laço quente — número citado, não remedido

Bancada do PhxJev (`plugins/phxjev/bancada/resultados.json`, 24/09 11:45 UTC,
esta máquina, sem GPU): modelo local de 1,5 B — **463 ms** por pergunta, acerto
0,667; 3 B — **6.474 ms**, acerto 0,833; 7 B — **14.194 ms**. Contra 0,885 µs do
detector: **5 a 7 ordens de grandeza**. O que decidiria na bancada: latência do
modelo por evento, medida aqui, com o texto normalizado do evento como entrada.

### 4.6 Como refazer

Medidores em
`/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/j-495/`
(**têm de ir para `bancada/seguranca/495/` pelo integrador** — script que
resolveu não morre com a sessão):

```bash
python3 extrair_legitimo.py        # legitimo.jsonl, do codigo
python3 extrair_deteccao.py        # deteccao.jsonl + escapado.jsonl, do ARSENAL
(cd detector && CARGO_PROFILE_DEV_DEBUG=0 /home/user/adrianoboller/phxsql/cargo-da-frente.sh \
   build --release --offline --target-dir ../target)   # prototipo: phxsql-sql por path
./target/release/detector495 .     # 4.1, 4.3, 4.4
python3 premissa_sec.py            # secao 2 (servidor de pe, porta 6795)
python3 prova_215.py               # 4.2 (porta 6797)
python3 rtt_sql.py                 # escala de 4.4 (porta 6799)
```

---

## 5. Decisão, com a hipótese que morreu

**D1 — H1 entra, e só observa.** As 4 classes: **empilhado** (pelo
`comando_empilhado` **corrigido** — um motor só, não um segundo detector),
**constante sob `OR`** (`OR` seguido de literal isolado ou de literal comparado
a literal), **UNION de sondagem** (o `SELECT` depois do `UNION` projeta só
literal/`NULL`, ou não tem `FROM`), **comentário que engole aspa** (comentário
com número **ímpar** de aspas, `#` fora de literal, ou `/*` sem fecho). Roda no
**sucesso e no erro**, na op `sql` e nos três campos de expressão das ops nativas
(`expressao_do_pedido`, com 4 chamadores; `tendo_do_pedido`, com 2; e o `expressao` do `op_consultar`), sobre os
**símbolos que a análise já produziu** (SEC M4). Nunca recusa, nunca bloqueia (SEC
R2/B3 e o voto 0 × 10 da linha E).

**D2 — ligado de fábrica para observar; o aviso segue pedido.** O registro nasce
ligado: não muda a resposta de cliente nenhum, custa ~0,3% do pedido `sql`, e no
corpo legítimo produz 1 evento falso em 1.186. O e-mail segue o `avisar_seguranca`, que
já nasce falso. A convergência da linha D (auditoria nasce desligada) **não
alcança**: o motivo dela é o volume de registrar **toda** consulta, e aqui se
registra só a marcada. Guarda nova entra pedida — e esta não guarda nada, só
conta.

**D3 — H2 morre como firewall; a digital fica.** Voto da linha C: 2 × 8. A lista
permitida vai para ⏸ (depois da versão), por usuário, **nunca** por IP (SEC R2).
A **digital** entra por convergência (linha B) e tem três usos, que é o que a lei
«função e comando não se duplicam» pede: texto redigido do evento (SEC R3 por
construção: não sobra literal), chave do silêncio (a mesma digital no mesmo
usuário é um aviso só), e texto que vai à IA (SEC M3).

**D4 — H3 fica PENDENTE.** Os sinais que já têm fonte (força bruta, catálogo,
escalada, horário/IP novo — SEC §4) entram na camada como produtores quando
houver quem os leia. Volume **não entra** sem a F4, porque sem dado-fonte seria
afirmação sem número.

**D5 — H4: IA analista, fora do laço.** Padrão: **nenhuma** no servidor. O
Centro de Controle lê a fila de eventos e, se a pessoa pedir, a Claude explica,
**pelo navegador** (`docs/CLAUDE-IA.md` §1 — a `std` não tem TLS). Vai à IA:
família, tipo, classes, texto **normalizado**, tabelas, contagens — **nunca
linha de dado**, e usuário e IP **pseudonimizados** (o analista precisa saber «é
o mesmo», não «quem é» — LGPD, necessidade). A resposta é parecer para gente,
**nunca ação** (SEC M2). Modelo local em `127.0.0.1` não fere pétrea (é HTTP puro,
como o relé SMTP), mas vai para ⏸: acerto medido de 0,667 no modelo que cabe em
meio segundo.

**D6 — H5: evitar é parametrizar.** O banco não impede o cliente de concatenar;
ele oferece o `?` que torna isso desnecessário e **vê** quem não usa. É isso que
se diz ao cliente (ver §9).

---

## 6. A camada comum 495/496

Hoje, medido no fonte: **8** chamadas a `email::enviar` no `servidor.rs`;
**4** chamadores do `jobs::pode_avisar` (o silêncio **já é um só**); **1** fila
com carteiro, a da saúde do disco (`saude_do_disco.rs`: `entregar` sob trava,
`Condvar`, thread fora de toda trava, `TETO_DA_FILA`). O pedaço que falta ser
comum é a **fila com carteiro**, e ele já existe num lugar só.

```
produtor (sob trava ou nao)              carteiro (thread propria, sem trava)
  saude do disco  (249, existe)  ─┐
  ataque          (495, F2)      ─┼─► Ocorrencia ─► pode_avisar ─► fila ─► canais:
  catastrofe      (496, do C)    ─┘   (tipada)      (silencio)            ocorrencias.log (redigido)
                                                                          op `ocorrencias` (painel, IA)
                                                                          e-mail / SMS (249)
```

`Ocorrencia`: `familia` (`saude` | `ataque` | `catastrofe`), `tipo` (da família;
o do 496 vem do catálogo do C), `gravidade`, `quando_ms`, `usuario`, `ip`,
`database`, **`tabelas` pela árvore inteira** (`profiler::colher_tabelas` —
SEC §5: a tabela escondida no lado B de uma junção não pode passar crua),
`texto` **só normalizado**, `digital` e `contagem` (o silêncio soma em vez de
descartar).

**O que o 496 pluga:** cada previsão de catástrofe do catálogo do C vira um
produtor que chama `entregar(Ocorrencia{familia: Catastrofe, …})`. Silêncio,
fila, carteiro, arquivo, painel, e-mail, SMS e a IA analista vêm de graça, e
não se escreve um segundo carteiro. **A blacklist não é canal da camada**:
bloqueio continua só nos portões que já existem (política, 215 ligado), e
nenhuma família bloqueia por heurística (SEC R2).

As 7 outras chamadas a `email::enviar` migram para a camada **depois da
versão** (⏸): funcionam hoje, e mexer nelas agora é escopo sem defeito.

---

## 7. Fatias entregáveis

| fatia | o quê | prova real nos dois sentidos | nível de modelo |
|---|---|---|---|
| **F0** | `comando_empilhado` exige ter consumido `;` antes de acusar | os 3 textos do `sondar.py` entram em `comando_empilhado_nao_acusa_o_legitimo`: **falham hoje**, passam com o conserto; o `prova_215.py` pelo soquete: bloqueio hoje, nenhum depois; o `acha_o_segundo_comando` segue verde | **médio** — conserto pequeno numa guarda de segurança |
| **F1** | `phxsql_sql::sinais(&[Simbolo], texto)` (4 classes) e `normalizar` (digital), funções puras | os 5 ataques do repositório acusados; 0 no dado escapado; catraca do falso positivo no corpo legítimo **extraído do código**, teto 1; defeito reposto (classe devolvendo 0, ou implementação por recorte) derruba o teste | **médio** — função pura com vetores |
| **F2** | a camada comum (extrai o carteiro da saúde do disco; a saúde vira o 1º produtor, provada pelos testes que já tem), a `Ocorrencia`, o gancho na op `sql` e nos 3 campos de expressão reaproveitando os símbolos (a análise passa a devolvê-los), sucesso e erro, `ocorrencias.log` redigido | a tautologia do arsenal gera 1 evento **com 2 linhas devolvidas**; um `CREATE USER … PASSWORD '<sentinela>'` marcado não deixa a sentinela no arquivo (SEC R3); junção com a tabela no lado B aparece em `tabelas` (SEC §5); desligado custa a leitura de um `bool` | **forte** — thread, trava, formato de arquivo |
| **F3** | op `ocorrencias` (administrar), painel, e-mail pelo `avisar_seguranca` | e-mail sai com o interruptor, não sai sem (SMTP falso da `bancada/proibidos`), corpo sem pedido | **médio**; a tela, **leve**, pela fábrica de idiomas |
| **F4** | SEC R1: linhas e bytes devolvidos por operação no `acessos.log` | variar `max` de 10 a 200.000 e ver o campo mudar (o teste adverso do SEC) | **forte** — formato; **vai ao DBA (C) antes** |
| **F5** | SEC M1: o `SELECT … INTO` da rotina passa a usar os parâmetros do léxico (`analisar_comando_com`) em vez de `chars.splice` — a mesma decisão escrita duas vezes | teste-alarme: um valor com `\'` entra como dado; e o teste falha se o splice voltar | **forte** — vizinho de injeção |
| **F6** | IA analista no Centro de Controle, lendo a fila | o corpo que sobe não tem linha de dado nem literal; uma linha semeada pedindo «apague X» não produz ação | **médio** |

⏸ depois da versão: lista permitida por usuário (H2), modelo local (H4b),
anomalia por z-score (H3, depois da F4 e de um corpo de tráfego), migração das 7
chamadas antigas de e-mail.

---

## 8. Lacunas

- **L1 — taxa de detecção por classe não foi medida.** O corpo de ataque que o
  repositório tem são 5 cargas de 2 classes (tautologia e empilhado); UNION de
  sondagem e comentário que engole aspa têm **0** caso de detecção aqui. Quem
  fecha: SEC e F, na `bancada/seguranca/injecao.py`, que é onde as cargas já
  moram. Até lá essas duas classes são **PENDENTE**, não FRUTÍFERO.
- **L2 — falso positivo em tráfego real** não existe como corpo. O do repositório
  é pessimista (testes de erro, SQL de outros motores), mas não é uma aplicação.
- **L3 — MariaDB** conferida pela KB, não pelo fonte; o padrão de ligação do
  Performance Schema dela não foi conferido.
- **L4 — fan-out não executado:** esta instância não tem ferramenta para
  convocar `pesquisa-motor` e `pesquisa-bancada`; os dois domínios foram
  cobertos aqui, e a revisão cruzada que o fan-out daria ficou faltando.
- **L5 — latência da IA** é número citado da bancada do PhxJev, não remedido.

## 9. O que sobe ao dono

**Produto — a promessa ao cliente.** A ordem diz «evitando SQL injector». O que o
banco **pode** prometer, medido: **evita** quem usa parâmetros (`?`, troca de
token, imune por desenho); **detecta e avisa** quem concatena (4 classes, 1
falso em 1.186); a IA **explica** o evento. Ninguém aqui **impede** um cliente
que concatena — o único que faria isso é a lista permitida, e ela morreu no voto.
Pergunta ao dono: o texto para o cliente pode dizer «detecta e avisa», e não
«a IA evita SQL injection»? É o mesmo cuidado do *ACID compliant*.

Nada mais sobe: nenhuma decisão aqui fere pétrea (zero dependências intacto,
IA pelo navegador ou HTTP local), e nenhum empate (os votos são 9 × 0, 2 × 8 e
0 × 10).

## 10. Aprendizados para a cognição (todos PENDENTE até a prova da fatia)

- a premissa «tautologia não executa» valia em 07/09 e envelheceu com a
  gramática — **afirmação de segurança feita por leitura envelhece quando o
  analisador cresce**; a evidência é o `premissa_sec.py`;
- o 215 passou no 5b porque o corpo do 5b não tinha comando com sobra — **corpo
  de falso positivo escolhido de uma fonte só mede essa fonte**; a evidência é o
  `prova_215.py`.
