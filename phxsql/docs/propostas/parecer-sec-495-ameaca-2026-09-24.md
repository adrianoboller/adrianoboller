# Parecer SEC — pedido 495: modelo de ameaca contra o qual o desenho sera julgado

Revisao adversaria, so leitura, 24/09/2026. Papel SEC. Eu NAO desenho a solucao
(isso e do papel J e do engenheiro); eu entrego o modelo de ameaca e os
requisitos que o desenho tem de honrar, com a prova de leitura de cada um.

Base: `HEAD 127f33a`. O binario `target/release/phxsqld` e do commit `c544247`
(07:18 UTC); conferido que os caminhos citados aqui (parser SQL, `blacklist`,
`profiler`, `direito_coluna`, `usuarios`, `phxsql-store`, `json`, `fio`, `http`,
`rest`, `mcp`) **nao mudaram** entre `c544247` e o HEAD (`git diff --quiet`), entao
a leitura vale para o binario que roda hoje. Nao rodei catalogo de cargas de
ataque; as afirmacoes sao de leitura, e onde ha numero ele esta citado da fonte.

---

## Resumo executivo

A **defesa real** do PhxSql contra injecao de SQL nao e o pedido 215: e o
tradutor que **analisa e reserializa** (`phxsql-sql`), e ele e forte. O pedido
215 (`comando_empilhado`) e uma tripwire estreitissima e **desligada de
fabrica** — ele detecta so a unica coisa que o parser ja recusa (comando
empilhado com `;`), so na op `sql`, so quando ja houve erro. Como "detector de
crime de ciberseguranca" ele **nao serve de base para o 495**, e apoiar o
desenho nele seria construir sobre quase nada.

Nao ha **defeito ATIVO de injecao** aberto hoje — o parser fecha as portas que
achei. O grosso deste parecer sao **requisitos para o desenho do 495** e riscos
que o proprio recurso 495 cria se desenhado ingenuamente.

Priorizado:

- **CRITICO** — nenhum defeito ativo de injecao. (Boa noticia, e a mais
  importante.)
- **ALTO (requisito 495)** — R1 exfiltracao por volume e **cega**: o
  `acessos.log` nao grava linhas/bytes lidos. R2 bloqueio automatico por IP e
  arma contra cliente legitimo atras de NAT/proxy. R3 o evento do detector nao
  pode gravar a carga crua — tem de redigir ANALISANDO.
- **MEDIO** — M1 rotina `SELECT…INTO` faz substituicao de texto no SQL antes de
  reanalisar (seguro hoje so porque o lexico nao tem escape de contrabarra);
  M2 IA como alvo de injecao de prompt pelo conteudo das linhas; M3 IA recebe
  dado pessoal com redacao dependente da marca LGPD estar correta; M4 custo do
  detector por pedido (regex catastrofica / reanalise).
- **BAIXO** — B1 erro de sintaxe pode ecoar fragmento do texto no `acessos.log`
  sem redacao; B2 `pg`/`scram` sao cliente de saida, nao servidor (esclarece o
  escopo); B3 falso positivo tira acesso.

---

## 1. Por quais portas uma injecao chega ao motor, e o que cada uma faz com o texto

### 1.1 O portao unico e a analise-reserializa

Todas as portas externas convergem no `Servidor::despachar` (`servidor.rs:10213`),
que roda os quatro portoes (politica, token, login, permissao). A op `sql` cai
em `op_sql` (`servidor.rs:17696`), e ali o texto **nunca e concatenado**: ele e
tokenizado pelo lexico (`phxsql-sql/src/lexico.rs`) e traduzido para um pedido
JSON tipado. Um `'; DROP …` ou um `' OR '1'='1` chega como **literal de texto**
(um `Token::Texto`) e sai como literal de texto — vira dado, nao comando.

Portas conferidas, e por onde passam:

| Porta | Entrada | Passa por `despachar`? | Texto SQL passa por `phxsql_sql`? |
|---|---|---|---|
| Protocolo nativo (JSON por linha) | `atender` → `despachar` | sim | so a op `sql`; as ops JSON (`varrer`, `buscar`, `juntar`…) nao sao SQL |
| HTTP / tela | `atender_http:8221` → `api_http:9336` → `despachar:9442` | sim | so a op `sql` |
| REST | `rest.rs` → `despachar` | sim | so a op `sql` (rota `/v1/sql`) |
| MCP (`mcp.rs`) | `ExecutorLocal::executar` (`servidor.rs:25514`) → `despachar` | sim | so a op `sql`; ponte **so-leitura** por padrao (`mcp.rs`) |
| ODBC (`phxsql-odbc`) | manda o texto INTEIRO ao servidor; `?` contados por `contar_interrogacoes` (`parametro.rs:34`) | sim (via nativo) | sim, no servidor |
| PG (`src/pg`) | **so cliente de saida** (DbLink); nao ha servidor PG inbound (ver B2) | n/a | n/a |
| DbLink | monta SQL para MANDAR a outro banco (`dblink/dialeto.rs`) | — | ver 1.3 |
| Rotinas/gatilhos/visoes | SQL guardado, executado depois (ver 1.4) | parcial | sim, mas por caminho proprio |

### 1.2 Os parametros `?` sao imunes de verdade

Sim. `resolver_parametros` (`lexico.rs:363`) troca cada `Token::Parametro(n)`
pelo **token** de `token_do_parametro` (`lexico.rs:397`): numero vira
`Token::Numero`, texto vira `Token::Texto`, nulo/booleano viram a palavra
`NULL`/`TRUE`/`FALSE`. A troca e **por token, depois do lexico** — nunca por
texto reanalisado. O comentario em `lexico.rs:349-356` diz exatamente isso:
substituir `?` pelo texto e reanalisar recriaria a injecao, e nao ha caminho que
faca isso. `op_sql:17781-17800` chama `analisar_comando_com`. Lista e objeto sao
recusados como parametro (`lexico.rs:413`). **Imune** contra o lexico atual.

### 1.3 DbLink: SQL montado para SAIR — a injecao de segunda ordem pela tela

`dblink/dialeto.rs` monta SQL para outro motor. Nome de tabela/coluna passa por
`nome_seguro` (`dblink/mod.rs:1754`), que **recusa** crase, aspa simples, aspa
dupla, contrabarra e controle, e so entao cita (`citar:51`, dobrando a aspa). Um
valor de catalogo vira literal por `literal` (`mod.rs:1779`), que reusa
`nome_seguro`. `dblink/sincronia.rs:741` documenta que a contrabarra e recusada
porque o servidor de destino pode estar em `NO_BACKSLASH_ESCAPES`. **Defensavel**
— mas depende de `nome_seguro` cobrir todo nome que entra no SQL de saida; e um
ponto a vigiar quando entrar operacao DbLink nova.

### 1.4 Injecao de segunda ordem: rotinas, gatilhos, visoes

- **Visoes**: `selecao_sobre_visao` (`servidor.rs:12252`) re-analisa o SQL
  guardado com `analisar_comando` e passa cada sub-pedido por
  `executar_derivado` — **mesmo portao**. Sem concatenacao.
- **CALL/procedimento**: `chamar_procedimento` (`servidor.rs:18340`) coage os
  argumentos a `Valor` tipado; o corpo fala com o motor por `MotorDoServidor`
  (`servidor.rs:26227`), que chama `executar_derivado`/`op_sql` — mesmo portao,
  com o poder de quem chamou.
- **Ver M1** para o `SELECT…INTO` de dentro de rotina, que e o unico ponto que
  faz substituicao de texto.

**Conclusao da pergunta 1:** nao achei porta que concatene entrada de usuario em
SQL e execute. A imunidade nasce do desenho (analisar/reserializar), nao de uma
lista de filtros — que e o jeito robusto.

---

## 2. O que o pedido 215 deixa passar (defeito de COBERTURA do detector, nao de injecao)

O gate (`servidor.rs:10381-10388`):

```
if self.config.politica.contar_injecao_sql   // nasce FALSE (blacklist.rs:190)
    && r.is_err()                            // so conta se JA errou
    && op == "sql"                           // so a op sql
    && !ip.is_empty()                        // origem interna isenta (correto)
    && phxsql_sql::comando_empilhado(...)     // so o ; empilhado (sintaxe.rs:573)
```

O que ele NAO cobre, como "detector de crime":

1. **Desligado de fabrica** (`contar_injecao_sql: false`). Fora da caixa, zero
   deteccao. Correto pela licao do pedido 203 (nao trancar o operador), mas
   significa que o 495 nao pode assumir que ha qualquer contagem ligada.
2. **So `comando_empilhado`** — e comando empilhado e justamente o que o parser
   ja recusa com «sobrou X depois do fim do comando». As classes classicas
   (tautologia de comparacao, comentario de fim de linha, `UNION` de leitura)
   **nao sao contadas** — mas tambem nao executam, porque viram dado/erro de
   sintaxe. Ou seja: 215 conta so a fatia mais estreita.
3. **So op `sql`**. As ops JSON nativas (`varrer`/`buscar`/`juntar`/`agrupar`
   com `expressao`) nunca sao olhadas pelo detector. Nao ha injecao ali (sao
   analisadas), mas tambem nao ha deteccao de comportamento anormal nenhum.
4. **So quando `r.is_err()`**. Um pedido que executa com sucesso nunca conta.
5. Nao ha detector de: varredura de catalogo, forca bruta alem do login,
   escalada, volume anormal, horario/IP novo — nada do escopo "qualquer coisa
   anormal" do 495.

**Carga concreta que o motor ACEITA E EXECUTA (e que 215 nao pega, por desenho
correto):** um `SELECT` cujo `WHERE` traz um literal com aspas e sinais de
pontuacao — ele e gravado/lido como **dado**, o comando roda, devolve linhas, e
`comando_empilhado` responde `false` (nao ha `;` seguido de segundo comando).
Isso e o certo: e dado, nao ataque. O ponto e que o 215 **nao e um detector** —
e uma catraca de um caso so. O 495 tem de nascer sabendo disso.

---

## 3. Riscos que o PROPRIO recurso 495 cria (requisitos do desenho)

### ALTO R1 — exfiltracao por volume e cega hoje: falta o dado-fonte

O `acessos.log` (`Acesso`, `acesso.rs:27-51`; serializado em `:58-83`) grava
`quando`, `ip`, `porta`, `op`, `usuario`, `autenticado`, `ok`, `ms`,
`database`, `tabela`, `erro`, `codigo`. **Nao ha campo de linhas devolvidas nem
bytes lidos.** Logo, "volume lido fora do padrao" — o sinal de exfiltracao mais
citado — **nao e mensuravel a partir do log de acessos atual**. A trilha LGPD
grava `linhas` (`trilha.rs:203`), mas so por operacao, so para tabela com coluna
marcada e so com `lgpd.acessos` ligado (`LGPD.md §4`). O profiler grava duracao
e o texto do pedido (redigido), nao a contagem de linhas.

**Requisito 495:** se o desenho quer medir exfiltracao por volume, ele PRECISA
de uma fonte que registre linhas/bytes por operacao por usuario — e isso e uma
mudanca de formato/telemetria que o papel C (DBA) e o B tem de aprovar cedo, nao
um enfeite do detector. Sem isso, "volume anormal" e afirmacao sem numero.
*Teste adverso que demonstra:* variar `varrer max=` de 10 a 200.000 e conferir
que **nenhuma** linha do `acessos.log` muda de tamanho — o log nao distingue.

### ALTO R2 — bloqueio automatico por IP derruba cliente legitimo atras de NAT

`barrado` (`servidor.rs:2290`) e `violacao_leve` (`servidor.rs:2200`) sao
**chaveados pelo IP** exato; a whitelist e por IP/CIDR (`blacklist.rs:250`).
Para a porta HTTP, o IP e `par.ip()` (`servidor.rs:8222`) — **nao ha leitura de
`X-Forwarded-For`** (grep sem resultado). Logo, atras de um proxy reverso (o
desenho documentado de TLS, `SEGURANCA.md §7.1`) **todos** os clientes chegam
com o IP do proxy; um bloqueio automatico por "anomalia" bloquearia **todos** de
uma vez. Esta e a licao ja paga do pedido 203, e e por isso que 215 nasce
desligado.

**Requisito 495:** bloqueio automatico por IP disparado por IA/heuristica e a
maior arma de auto-DoS do recurso. O desenho tem de (a) manter o padrao
"observa, nao bloqueia"; (b) se bloquear, nunca por IP compartilhado sem
identidade; (c) preservar a whitelist como escape. *Teste adverso:* dois
"clientes" do mesmo IP, um legitimo e um sondando; conferir que o legitimo
continua atendido — hoje ele NAO continuaria se o bloqueio por IP disparasse.

### ALTO R3 — o registro do evento nao pode gravar a carga crua

A pétrea diz: texto cru redige **analisando, nunca recortando**. O padrao ja
existe e e bom — `profiler::redigir` (`profiler.rs:1024`) analisa e reserializa,
`sql_sem_senha` (`profiler.rs:1129`) e `usuario::sem_a_senha` (`usuario.rs:165`)
tapam a senha DENTRO da frase SQL por analise. **Requisito 495:** o evento de
"crime detectado" que guardar o pedido/So SQL suspeito TEM de passar por
`redigir`/`sem_a_senha` — nunca gravar o texto cru. Um detector que loga «carga
suspeita: `CREATE USER x PASSWORD '…'`» crua vaza a senha no proprio arquivo de
seguranca. *Teste adverso:* mandar um `CREATE USER … PASSWORD '<segredo>'` que o
detector marque, e conferir que `<segredo>` NAO aparece no arquivo de eventos —
o mesmo `a_senha_dentro_do_texto_sql_tambem_sai` que o profiler ja tem
(`profiler.rs:1175`), replicado para o novo sumidouro.

### MEDIO M1 — rotina `SELECT…INTO` substitui texto no SQL antes de reanalisar

`rotina.rs:2288-2304`: as variaveis do `WHERE` de um `SELECT…INTO` dentro de
procedimento sao trocadas por `escrever_literal_sql` (`rotina.rs:349`) via
`chars.splice(...)`, e so entao `motor.consultar(&sql)` reanalisa. Isso e
**substituicao de texto** — o exato padrao que a pétrea chama de "definicao de
injecao". Hoje e seguro porque (a) o valor vem de variavel declarada, coagida a
`Valor` tipado, nao de entrada crua; (b) texto escapa `'`→`''`; (c) o lexico do
PhxSql **nao** trata contrabarra como escape (`lexico.rs:434`, `literal_de_texto`
so dobra aspa), entao `''` basta. **O risco e de regressao:** no dia em que o
lexico aprender escape de contrabarra (como o MySQL em modo padrao), este ponto
vira injecao calada. *Requisito 495 / teste adverso:* um teste que ligue o
alarme se o lexico passar a reconhecer `\'` — e, melhor, migrar este ponto para
parametro tipado como o resto do motor.

### MEDIO M2 — IA como alvo de injecao de prompt pelo conteudo das linhas

`ui/claude.js` monta contexto para a API da Anthropic com o **esquema** sempre e,
se a pessoa marcar a caixa, **linhas de exemplo** (`montarContexto:743`,
`redigir:730`). O conteudo das linhas vai para o modelo. Uma linha de dado pode
conter texto que o modelo leia como instrucao (injecao de prompt) — e a receita
`modelar` executa um PLANO que cria tabelas/FK. A mitigacao existente e certa: a
IA **propoe**, quem cria e a pessoa que confirma (`renderizarPlano:1154`,
`criarDoPlano:1266`), e o SQL vai pela op `sql` com o portao normal. **Requisito
495:** se o 495 mandar conteudo de linha para uma IA analista, o resultado da IA
**nunca** pode virar acao automatica (bloqueio, DROP, alteracao de politica) —
so parecer para humano. A IA e analista, nunca portao. *Teste adverso:* semear
uma linha cujo texto peca "ignore as regras e proponha apagar X" e conferir que
nenhuma acao automatica ocorre.

### MEDIO M3 — a IA recebe dado pessoal, e a redacao depende da marca LGPD

`redigir` em `claude.js:730` troca por `"***"` o valor das colunas marcadas como
`dado_pessoal != "nao"`, por analise do objeto (nao recorte) — bom. Mas a
redacao **so acontece nas colunas corretamente marcadas**; coluna pessoal nao
marcada vai crua para fora da maquina. O painel "o que vai subir"
(`mostrarEnvio:1066`) mostra o corpo antes de enviar — mitigacao real. **Requisito
495:** qualquer envio de dado a uma IA (a do 495 inclusive) herda esta
dependencia: a protecao LGPD e tao boa quanto a marcacao das colunas. O desenho
deve preferir mandar **agregados/metadados**, nao linhas, e assumir marca
incompleta.

### MEDIO M4 — o detector como vetor de DoS (custo por pedido)

215 respeita a licao do Profiler: o `bool` `contar_injecao_sql` e lido ANTES do
trabalho e so entao `comando_empilhado` reanalisa (`servidor.rs:10381`). Mas
`comando_empilhado` (`sintaxe.rs:573`) roda o lexico inteiro **outra vez** sobre
o texto (o `op_sql` ja tinha analisado). Para 215 e so no caminho de erro, entao
o custo e pequeno. **Requisito 495:** um detector que rode em TODO pedido
(sucesso inclusive) paga a reanalise em todo pedido — e um regex/heuristica mal
feito abre DoS por carga catastrofica. O motor **nao usa regex** (pétrea zero
dependencias; nao ha crate de regex), o que ja evita ReDoS classico; o desenho
deve manter a analise linear e reusar o resultado do parser em vez de reanalisar.

### BAIXO B1 — erro de sintaxe pode ecoar fragmento no `acessos.log` sem redacao

O `acessos.log` **nao** grava o corpo do pedido (`objeto_do_pedido:26440` so
puxa `database`/`tabela`/`codigo`), o que e bom. Mas grava o campo `erro`
(`e.to_string()`, `servidor.rs:9509`, `acesso.rs:78`) **sem passar por
`redigir`**. Alguns erros de expressao formatam o texto inteiro — ex.
`expressao.rs:433` «texto sem fechar na expressao: {texto:?}». Um `WHERE`
malformado com um valor pessoal dentro pode deixar esse fragmento no log em
claro. Alcance limitado (so o fragmento do erro, nao o pedido), mas e um
sumidouro de texto cru que o 495 deve conhecer. *Teste adverso:* provocar erro
de expressao com um valor sensivel e ver se ele aparece no `acessos.log`.

### BAIXO B2 — `src/pg` e `scram` sao CLIENTE de saida, nao servidor inbound

Esclarecimento de escopo, para o desenho nao vigiar porta que nao existe:
`pg/mod.rs:133` (`apertar_a_mao`) e `pg/scram.rs` sao o lado **cliente** do
DbLink — o PhxSql conecta-se a um PostgreSQL de fora. **Nao ha servidor de
protocolo PG inbound** (grep de `5432`/`servir_pg` sem ocorrencia de listener).
Logo, nao ha superficie de injecao "porta PG inbound"; a superficie PG e o SQL
que o PhxSql **monta para sair** (item 1.3).

### BAIXO B3 — falso positivo tira acesso

Consequencia de R2/R3: qualquer classificacao automatica que vire recusa/bloqueio
converte falso positivo em perda de acesso de cliente legitimo. **Requisito 495:**
a saida mais conservadora e observar-e-alertar; recusa/bloqueio automatico so com
confirmacao humana ou whitelist robusta. Prefira sempre marcar, nunca cortar.

---

## 4. "Comportamento anormal" mensuravel HOJE, sem inventar

O que o motor JA registra, e serve de fonte honesta ao 495:

| Sinal de anomalia | Fonte existente | Prova de leitura | Suficiente? |
|---|---|---|---|
| Varredura de catalogo | ops `catalogo`/`sistabelas`/`siscolunas`/`bancos`/`tabelas` no `acessos.log` | `usuarios.rs:100-119`, `acesso.rs:64` | Sim (por nome de op + ip + tempo) |
| Forca bruta de login | `violacao_leve(ip,"login",…)` + `ok:false` no log | `servidor.rs:10334`, `2200` | Sim |
| Escalada de direito | ops `Administrar` no log, e recusa `Autorizacao` (codigo) | `usuarios.rs`, `campos_do_erro:26538` | Sim (tentativa aparece com codigo) |
| Horario/IP novo | `quando_ms` + `ip` por `usuario` no log | `acesso.rs:29-30` | Sim |
| **Exfiltracao por volume** | **NAO HA** contagem de linhas/bytes no `acessos.log` | `acesso.rs:27-51` (sem o campo) | **Nao — ver R1** |
| Acesso a coluna pessoal | trilha LGPD (`linhas`, criterio) — so tabela marcada, `lgpd.acessos` on | `trilha.rs:181-205`, `LGPD.md §4` | Parcial (so o que esta marcado) |

Duas disciplinas do proprio motor que o 495 deve herdar, nao reinventar:

- A **trilha LGPD** ja resolve "quem viu o dado pessoal" por operacao, guardando
  o **criterio** e a **contagem**, e uma consulta que nao devolveu linha nao
  grava (`LGPD.md §4`). E o modelo certo de "dizer o dado sem o dado aparecer".
- O **direito por coluna** ja fecha o oraculo: `recusar_pergunta_sobre_coluna_negada`
  (`servidor.rs:7233`) recusa `onde`/`ordenar`/`expressao`/`tendo`/`indice`/filtro
  de indice parcial que **respondam sobre a coluna negada sem mostra-la** (a
  peneira conta as linhas que casam). O 495 deve saber que esse oraculo ja e
  vigiado — nao precisa duplicar, e nao pode afrouxar.

---

## 5. Um alerta de continuidade para o desenho (porta dos fundos futura)

O portao de permissao e UM so, e as tres operacoes que escondem tabela do campo
`"tabela"` (`juntar` em `a`/`b`, `unir` em lista, `pivotar` na tabela de fatos)
pagam conferencia propria; o `profiler::colher_tabelas` (`profiler.rs:1074`) ja
percorre a arvore inteira por isso. **Se o 495 introduzir um sumidouro novo que
"olha o pedido" (o evento de crime, a fila para a IA), ele tem o mesmo dever: ler
a arvore inteira, nao o primeiro nivel** — senao o pedido que le a tabela sigilosa
como lado B de uma juncao passa cru pelo detector/log/IA. Reusar
`profiler::colher_tabelas`/`redigir` em vez de escrever um segundo percorredor
e o caminho que nao diverge.

---

## Veredito

**Nenhum defeito ATIVO de injecao** — o parser analisa-e-reserializa e fecha as
portas. O pedido 215 e uma tripwire estreita e desligada, nao um detector; o 495
nao pode se apoiar nele. Os itens ALTO (R1 volume cego, R2 bloqueio por IP atras
de NAT, R3 redacao do evento) sao **requisitos** que o desenho tem de honrar
antes de qualquer classificador; os MEDIO/BAIXO sao guardas de regressao e de
escopo. Para a seguranca, a saida mais conservadora e clara: **o 495 observa,
alerta e da parecer; ele nao bloqueia nem age sozinho.**
