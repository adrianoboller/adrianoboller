# O que falta

Revisão de 28/08/2026, contra o commit `6839aed`. Escrita para responder uma
pergunta só: de tudo que foi pedido nesta conversa, o que ainda não existe no
código?

A regra aqui é a mesma do dossiê — **número medido, nunca estimado**. Onde há
número, ele saiu de `cargo test`, de `wc -l` ou da bancada em
`bancada/resultados.json`.

Estado do repositório ao fim desta revisão: **20.337** linhas de Rust,
**283** testes passando, zero avisos de clippy, zero dependências, versão
**0.4.1**.

---

## 0. O que a revisão achou de errado — e já consertou

Revisar serve para achar. Nove coisas apareceram, e quase nenhuma delas era um
recurso faltando: eram o projeto se descrevendo errado.

- **A bancada media coisas diferentes dos dois lados, e o número saía a nosso
  favor.** Na varredura por faixa o MySQL(R) recebia
  `COUNT(*) + SUM(valor)` sobre **1.250.000** linhas, e o PhxSql lia
  **20.000** — 1,6% do trabalho. O «5× mais rápido» que estava no roteiro não
  era o motor sendo rápido; era ele fazendo um sexagésimo do serviço. A fase
  `varrer` de `examples/carga.rs` passou a ler a faixa inteira e somar o
  valor, e a bancada foi refeita do zero. É o segundo erro deste tipo achado
  aqui — o primeiro favorecia o MySQL(R), este favorecia o PhxSql —, e por
  isso o `bancada/LEIA-ME.md` ganhou uma quarta regra: **mesma quantidade de
  trabalho**, não só mesma forma de pergunta.

- **Campo escrito errado no `config.json` era silencioso.** Quem quisesse
  trocar a porta escreveria `"porta": 5001` — mas o campo se chama `bind`. O
  servidor subia na 5000, sem uma palavra, e tudo *parecia* certo até ninguém
  conseguir conectar. Agora o arranque diz quais campos não foram
  reconhecidos e avisa que o valor foi ignorado. Não é erro: config antigo
  continua subindo.

- **Seis marcas de terceiros estavam sem o `(R)`.** A regra é sua e é clara.
  Escapavam `MySQL` em `docs/REPLICACAO.md` e no próprio dossiê, `HFSQL` na
  documentação de dois módulos, `SQLite` e `Clarion` no `docs/PLANO.md`.
  Marcadas. Fica de fora, de propósito, uma citação literal do `Cargo.toml` do
  rusqlite — citação se transcreve como está — e o changelog do phx-grid, que
  é documento de terceiro e fala de planilha, não de banco.

- **Os 2,4 GB da bancada não estavam no `.gitignore`.** Um `git add -A` numa
  hora ruim mandaria a tabela inteira para o repositório.

- **Não havia como refazer os pacotes.** Os compilados de Linux e Windows das
  rodadas anteriores foram montados à mão — ninguém consegue reproduzir o que
  foi entregue. Virou `empacotar.sh`, que monta os dois pacotes e o zip de
  fontes a partir do `git archive` (que respeita o `.gitignore` de graça).

- **O servidor anunciava a versão errada.** `Cargo.toml` do workspace em
  `0.1.0` enquanto o changelog ia em 0.4.0 e os pacotes saíam com 0.4.0 no
  nome. Como `VERSAO` é `env!("CARGO_PKG_VERSION")`, o `ping` e o rodapé do
  Centro de Controle respondiam `0.1.0`. Quem decidisse compatibilidade pela
  versão recebia a resposta errada há três lançamentos. Corrigido — e o
  exemplo de arranque no `MANUAL.txt`, que mostrava a mesma `0.1.0`, junto.

- **A capa e o rodapé do dossiê estavam defasados.** A capa dizia 276 testes
  onde eram 280, e a contagem de linhas de doc não batia com nenhuma receita
  reproduzível; o rodapé estava parado inteiro em *0.3.0 · 19.242 linhas ·
  69 KB de interface*. Remedidos, e a receita de medição no
  `docs/dossie/LEIA-ME.md` passou a listar exatamente os arquivos contados —
  antes ela dava um número diferente do publicado.

- **A bancada não estava no dossiê.** A maior medição já feita no projeto — dez
  milhões de registros contra o MySQL(R) — existia só em `bancada/` e como uma
  linha «pronto» no roteiro. Virou a seção 16, com figura, tabela e o
  diagnóstico da inserção.

- **Três pedidos não estavam nem registrados como ausentes.** Triggers, stored
  procedures e jobs não constavam do roteiro do dossiê — nem como «a fazer».
  Entraram.

## 1. Pedido, e não existe

Estas cinco são pedidos explícitos do Adriano que nunca viraram código. Não
estão pela metade: não começaram.

| Pedido | Onde foi pedido | Situação | O que falta de verdade |
|---|---|---|---|
| **Triggers** | rodada da bancada | não começou | Onde disparar já existe: `Table::inserir`, `atualizar` e `excluir` são os três pontos, e já escrevem no `.log`. O que falta é decidir **em que linguagem** o gatilho é escrito. Sem camada SQL não há `BEGIN … END` para hospedar. |
| **Stored procedures** | rodada da bancada | não começou | Mesmo bloqueio, maior: procedimento é código guardado, e código guardado precisa de um executor. Ou se escolhe uma linguagenzinha própria, ou se espera a camada SQL. |
| **Jobs de execução** | rodada da bancada | não começou | Este é o barato dos três. O agendador do backup (`subir_backup_agendado`, `hora_de_rodar`, `minuto_do_dia`) já é exatamente o desenho: linha no `config.json`, laço que acorda de minuto em minuto, marca do último disparo. Falta generalizar de "rodar backup" para "rodar operação nomeada". |
| **Parar e subir o serviço de dados pela interface**, trocando a porta | rodada do checklist | não começou | O `accept` bloqueia. Derrubar a porta sem derrubar o processo exige acordar o laço — o jeito honesto é conectar no próprio endereço para o `accept` retornar, e aí conferir um sinalizador. Mexe no coração do servidor; melhor inteiro do que pela metade. |
| **Replicação transportando evento** | rodada da replicação | desenhada, não transporta | As três portas entram no `config.json` e são validadas (nenhuma pode repetir endereço). O desenho está na seção 8 do dossiê e em `docs/REPLICACAO.md`. Falta o `.log` v2 **com imagem da linha** — hoje o diário registra que houve alteração, não o que a linha virou; sem isso a réplica não tem o que aplicar. |

Nota sobre as três primeiras: no arranque de 2026-08-27 eu disse que traria o
desenho escrito de triggers e procedures antes de escrever código, porque a
escolha da linguagem é do Adriano. Continua valendo — e continua sendo a coisa
que trava as duas.

## 2. Pedido em rodadas antigas, e ainda não existe

| Pedido | Situação | Comentário |
|---|---|---|
| **Servidor MCP** | a fazer | O protocolo do PhxSql já é JSON por linha; o MCP é uma tradução de vocabulário sobre o que já existe. |
| **Camada SQL** | a fazer | O caminho decidido é tabela virtual do rusqlite atrás de um recurso do Cargo — o que dá SQL completo sem escrever parser. Repare que isso **fura a regra de zero dependências**, e por isso fica atrás de um `feature`: quem não liga, compila sem. |
| **Driver ODBC de saída, depois cliente ODBC e OLE DB** | a fazer | Depende da camada SQL: driver ODBC que não fala SQL não serve para o que o Adriano quer ligar nele. |
| **Integração no FraseSQL como `engine = "phxsql"`** | a fazer | Depende do cliente. |
| **Subir num repositório próprio no GitHub** | **bloqueado de fora** | `create_repository` responde `403 Resource not accessible by integration`. Não é escolha minha nem defeito do código: a credencial desta sessão só alcança `adrianoboller/adrianoboller`. Enquanto isso o projeto vive em `phxsql/` na branch `claude/capacidades-disponiveis-y6auxh`. Destravar exige o Adriano criar o repositório e dar acesso. |
| **Compactação, transações, concorrência fina, TLS** | a fazer | Registrado em "o que este motor ainda não faz", no fim do dossiê. |

## 3. Ninguém pediu, mas a medição aponta

A bancada de 10 milhões de registros achou um buraco só, e é grande:

**A inserção é o ponto fraco do motor.** 3.685 linhas/s contra 95.301 do
MySQL(R) — **25,9× mais devagar**. E o diagnóstico é incômodo: são
**2.699 s de CPU para 2.714 s de relógio**, com **zero MB lidos do disco**.
Não é disco, é processador. Pior: a taxa **piora com o tamanho** — o primeiro milhão entra a
4.558/s, o décimo a 3.414/s, e a média acumulada cai de 4.558 para 3.685. Isso
é assinatura de estrutura de índice degradando, não de I/O.

Nas outras quatro operações o motor se defende: varredura por faixa 5,0×
**mais rápido** que o MySQL(R) (3,6 s contra 18,0 s), atualização empatada
(4,7 s contra 4,8 s), busca pontual 3,4× mais devagar, exclusão 1,9× mais
devagar. E escreve muito menos: 2,29 GiB contra 38,9 GiB na carga.

Contrapartida honesta: **ocupa 2,27 GiB em disco contra 0,88 GiB** do MySQL(R),
porque o `.reg` é de slot fixo — o preço do endereçamento O(1) e da ordem de
digitação.

Se algum dia sobrar uma rodada para o motor em vez de para recurso novo, é
aqui que ela rende.

## 4. As perguntas que você fez, e onde está a resposta

O checklist que ficou só na conversa. Fica aqui para não depender de rolagem.

| Pergunta | Respondida | Onde |
|---|---|---|
| O que você sabe fazer aqui? | sim | conversa |
| Você tem acesso ao meu celular? | sim — **não tenho** | conversa |
| Precisa de agentes e subagentes para agilizar? | sim | conversa |
| Dá para ter replicação parecida com a do MySQL(R)? | sim — dá, e o desenho está escrito | seção 8 do dossiê, `docs/REPLICACAO.md` |
| O desenvolvimento foi conduzido de forma adequada? Poderia ter sido diferente? Qual o impacto de custo? | sim | conversa |
| Como ter tabelas PhxSql em Android, iOS e IoT? | sim | conversa |
| Ele **realmente** cria a regra de firewall e bloqueia quem tenta injeção ou comando da blacklist? | sim — e a auditoria achou um buraco de verdade | seção 10 do dossiê; `docs/SEGURANCA.md` |
| Como o PhxSql se compara ao MySQL(R) em 10 milhões de registros? | sim — e o número **estava errado a nosso favor**; refeito | seção 16 do dossiê, `bancada/` |

Sobre a de firewall, vale repetir a parte que corrigiu a pergunta: **não há SQL
no PhxSql**, então injeção de SQL não tem superfície. A superfície real é o
nome de database e de tabela virando caminho de arquivo — e foi exatamente ali
que a auditoria achou o furo: as sondas de travessia (`../../../etc`, `/etc`,
`C:\dados`) eram *recusadas* mas não contavam violação. Seis sondas, seis
linhas de log, zero bloqueios. Hoje nome hostil é violação grave e bloqueia na
primeira tentativa.

## 5. Duas afirmações da folha de marca que continuam falsas

Registrado no `CLAUDE.md` e repetido aqui porque é fácil esquecer: a folha diz
*ACID compliant* e *built-in replication*. **Nenhuma das duas é verdade hoje** —
não há transação e a replicação não transporta evento. Não repetir em documento
técnico enquanto não forem.
