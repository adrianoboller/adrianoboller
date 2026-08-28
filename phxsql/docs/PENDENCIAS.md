# Tudo que foi pedido: feito, parcial, planejado

Revisão de 28/08/2026. Uma linha por pedido seu, desde a primeira mensagem, com
o estado real medido contra o código — não contra a lembrança.

A regra é a mesma do dossiê: **número medido, nunca estimado**. Onde há número,
ele saiu de `cargo test`, de `wc -l` ou de `bancada/resultados.json`.

Estado do repositório ao fim desta revisão: **20.337** linhas de Rust,
**283** testes passando, zero avisos de clippy, zero dependências externas,
versão **0.4.1**.

---

## 1. Feito

Pronto, testado, e no ar. A última coluna aponta onde a peça mora, para você
poder conferir sem perguntar. Os **283 testes** do projeto passam, com zero
avisos de clippy.

| # | O que você pediu | Como está | Onde |
|---|---|---|---|
| 1 | `Tabela.reg` — tabela física **na ordem de digitação** | slot fixo, CRC por registro, esquema embutido; slot excluído **nunca** é reaproveitado | `store/reg.rs` |
| 2 | `Tabela.ndx` — índices | B+tree com divisão de páginas, chave composta, ASC/DESC/NOCASE/único | `store/ndx.rs` |
| 3 | `Tabela.bin` — binários | blocos com CRC e contabilidade de espaço morto | `store/blob.rs` |
| 4 | `Tabela.memo` — textos longos | mesmo mecanismo do `.bin` | `store/blob.rs` |
| 5 | `Tabela.log` — **toda** inclusão, alteração e exclusão com data e hora | append-only, 36 bytes por evento | `store/log.rs` |
| 6 | Paginação `Nome_001.reg`, `_002`… | volume = `(rowid−1)/por_arquivo + 1`, abertura preguiçosa; o `.ndx` **não** pagina | `store/volume.rs`, `core/paginacao.rs` |
| 7 | Pastas separando tabelas **e** bancos | `base/ → database → tabelas na raiz → schema/` | `store/catalogo.rs` |
| 8 | Reindex criando o `.ndx` **do zero** | varre o `.reg` e reconstrói | `store/table.rs` |
| 9 | Aceitar linha de comando | `phxsql` com 10 comandos, `phxsqld` com 9 chaves | `cli/main.rs`, `server/main.rs` |
| 10 | Porta 5000, configurável no `config.json` | campo `bind`; campo com nome errado agora **avisa** em vez de silenciar | `server/config.rs` |
| 11 | Log de IPs que acessaram a 5000, com IP, data e hora | JSON Lines, para caber `fail2ban` por cima | `server/acesso.rs` |
| 12 | Cadastro com nome, login, senha, email, telefone, supervisor e poder por base | tudo isso, mais nível e chave pública | `server/usuarios.rs` |
| 13 | Usuário root e senha no `config.json` | e a senha nunca em texto puro: PBKDF2-HMAC-SHA256, 210.000 voltas | `core/senha.rs` |
| 14 | **Nível admin** no `config.json` e no usuário | cinco níveis: nenhum, leitor, operador, dono, admin | `server/usuarios.rs` |
| 15 | Login e senha em **Base64**, não em claro | feito — e o padrão é melhor: desafio-resposta, em que a senha **não sai da máquina** | `core/desafio.rs`, `core/base64.rs` |
| 16 | Chave assimétrica no `config.json` como parâmetro extra | Ed25519 (RFC 8032) como segundo fator, conferido contra os quatro vetores oficiais | `core/ed25519.rs` |
| 17 | Login com servidor/porta/usuário/senha/chave/database | a chave é opcional e o `config.json` decide se é exigida | `ui/index.html` |
| 18 | `blacklist.json` com IP, data, hora e comando bloqueado | mais recarga automática quando o arquivo muda entre processos | `server/blacklist.rs` |
| 19 | Seção de comandos proibidos no `config.json` | e a auditoria achou ali um furo real: travessia de caminho era recusada mas **não contava violação**. Corrigido: bloqueia na primeira tentativa | `server/blacklist.rs`, `store/catalogo.rs` |
| 20 | Criar regra de firewall em quem tenta o proibido | conferido com um `iptables` falso que grava o que recebeu | `server/blacklist.rs` |
| 21 | Interface web parecida com o Centro de Controle | embutida no `phxsqld`, sem servidor web para instalar; cinco abas por tabela e três telas de administração | `server/http.rs`, `ui/index.html` |
| 22 | Ícones ☀️/🌓 para claro e escuro | e o console guarda mais de um servidor | `ui/index.html` |
| 23 | Tabela em memória tipo Redis(R), com `SelectMemory` | **87× mais rápido**, medido | `store/memoria.rs` |
| 24 | Backup agendado **ou ao vivo**, em zip `Banco_Admin_Data_Horamin.zip` | manifesto SHA-256 e conferência; o ZIP e o DEFLATE são escritos aqui | `store/backup.rs`, `core/zip.rs` |
| 25 | `tabela.bkp` como clone do `.reg`, se ligado no `config.json` | segunda chance na falha de CRC; e o espelho **não** sobrescreve um `.bkp` bom com um `.reg` corrompido | `store/volume.rs` |
| 26 | DataGrid com faixa de agrupamento acima das colunas, tipo Excel(R) | phx-grid: arrastar o cabeçalho agrupa, com contagem e agregados; vários níveis empilham | `ui/grid/phx-grid.js` |
| 27 | Dashboard com gráficos de bancos, usuários, conexões… | **sete** gráficos e oito números, tudo de **uma** chamada, e contando só o que o login enxerga | `server/servidor.rs` (op `painel`), `ui/index.html` |
| 28 | Organograma, fluxograma e dossiê | 18 seções, 15 figuras, todas em SVG escrito à mão | `docs/dossie/` |
| 29 | `Config_exemplo_01/02/03.json` | isolado, réplica e origem | `exemplos/` |
| 30 | Manual de uso | `MANUAL.txt`, 18 seções | `MANUAL.txt` |
| 31 | Usar a marca onde precisar | capa, cabeçalho, favicon e paleta; `phxsql/marca/` | `marca/` |
| 32 | `(R)` nas marcas de outros bancos | conferido em todo o repositório | — (varrido no repositório) |
| 33 | Revisar regras, corrigir defeitos e registrar em `changelog.md` | `CHANGELOG.md`, com **Corrigido** primeiro e uma seção *Sabido* do que não funciona | `CHANGELOG.md` |
| 34 | Comparar com o MySQL(R) em 10.000.000 de registros, com gráficos de IO, memória e CPU | seção 16 do dossiê e `bancada/`; **refeita** nesta rodada, porque a montagem anterior comparava trabalho diferente | `bancada/`, seção 16 do dossiê |
| 35 | Download dos fontes e do compilado para Linux e Windows | `./empacotar.sh` monta os três zips; conferidos com `unzip -t` | `empacotar.sh` |

## 2. Parcial

Existe, funciona no que promete, mas **não faz tudo** o que o pedido queria.
Cada linha diz exatamente onde para.

| # | O que você pediu | O que existe | O que falta |
|---|---|---|---|
| 1 | **Replicação como a do MySQL(R)**, com porta de acesso, de envio e de retorno | as três portas entram no `config.json` e são validadas — duas no mesmo endereço não sobem. O desenho está na seção 8 do dossiê e em `docs/REPLICACAO.md`. O `.log` **é** o binlog | o `.log` **v2 com imagem da linha**. Hoje o diário registra que houve alteração, não o que a linha virou — sem isso a réplica não tem o que aplicar. O servidor avisa alto no arranque que as portas são configuração, não serviço |
| 2 | **Chave estrangeira** com CASCADE / RESTRICT / SET NULL | declarada, validada, gravada no cabeçalho do `.reg`, sobrevive a fechar e abrir, e aparece na aba Estrutura | **não é aplicada**. Nenhuma gravação consulta a chave: `Restringir` e `Cascata` são intenção guardada, não comportamento. Estava marcada «pronto» no README e no dossiê — corrigido nesta revisão |
| 3 | **Quantidade de registros e arquivos definida no create table** | a paginação é parâmetro do esquema e funciona; `criar_tabela` existe na biblioteca | não há **op no protocolo nem comando na CLI** para criar tabela. Hoje só se cria escrevendo Rust. Criar *database* pela rede já dá |
| 4 | **Gráficos comparativos** de IO, memória e CPU | `bancada/graficos.py` gera a página inteira a partir do `resultados.json` | a página gerada não estava **versionada** — existia só na máquina de quem rodou. Passa a entrar no repositório |
| 5 | **Subir o PhxSql no GitHub** | está em `adrianoboller/adrianoboller`, na branch `claude/capacidades-disponiveis-y6auxh`, com histórico completo | repositório **próprio**: `create_repository` responde `403 Resource not accessible by integration`. Não é escolha minha nem defeito do código — a credencial desta sessão só alcança esse repositório. Destravar depende de você criar o repositório e dar acesso |

## 3. Planejado

Pedido e **não começado**. Não estão pela metade: não têm código nenhum.

| # | O que você pediu | Por que ainda não | O que destrava |
|---|---|---|---|
| 1 | **Jobs de execução** | é o mais barato dos três, e ficou para depois do painel | o agendador do backup (`hora_de_rodar`, `minuto_do_dia`, o laço que acorda de minuto em minuto) já é exatamente o desenho. Falta generalizar de «rodar backup» para «rodar operação nomeada». Uma rodada |
| 2 | **Triggers** | onde disparar já existe — `inserir`, `atualizar` e `excluir` são os três pontos, e já escrevem no `.log` | falta decidir **em que linguagem o gatilho é escrito**, e essa escolha é sua. Sem camada SQL não há `BEGIN … END` para hospedar |
| 3 | **Stored procedures** | mesmo bloqueio, maior | procedimento é código guardado, e código guardado precisa de executor. Ou uma linguagem própria pequena, ou esperar a camada SQL |
| 4 | **Parar e subir o serviço de dados pela interface**, trocando a porta | mexe no coração do servidor | o `accept` bloqueia. Derrubar a porta sem derrubar o processo exige acordar o laço — conectar no próprio endereço para o `accept` retornar e então conferir um sinalizador. Melhor inteiro do que pela metade |
| 5 | **Servidor MCP** | não depende de nada; é fila | o protocolo já é JSON por linha. O MCP é tradução de vocabulário sobre o que existe |
| 6 | **Camada SQL** | é a peça de que três outras dependem | tabela virtual do rusqlite atrás de um recurso do Cargo — dá SQL completo sem escrever parser. Repare que **fura a regra de zero dependências**, e por isso fica atrás de um `feature`: quem não liga, compila sem |
| 7 | **Driver ODBC de saída** | depende de (6) | driver ODBC que não fala SQL não serve para o que você quer ligar nele |
| 8 | **Cliente ODBC e OLE DB** | depende de (7) | — |
| 9 | **Integração no FraseSQL** como `engine = "phxsql"` | depende de (8) | — |
| 10 | Compactação | o formato já prevê e **mede** o espaço morto | falta o comando. O reindex já cobre a parte do índice |
| 11 | Transações | — | hoje a inserção desfaz o que gravou se um índice falhar, mas não há journal nem `commit`/`rollback` de várias operações |
| 12 | Concorrência fina | — | uma trava única serializa todo acesso a dados |
| 13 | TLS | — | o tráfego depende de túnel. A credencial já não vai em claro quando se usa desafio-resposta; os dados, sim |

---

## 4. O que esta revisão achou de errado — e já consertou

Revisar serve para achar. Onze coisas apareceram, e quase nenhuma era recurso
faltando: era o projeto se descrevendo errado.

- **A bancada media coisas diferentes dos dois lados, e o número saía a nosso
  favor.** Na varredura por faixa o MySQL(R) recebia `COUNT(*) + SUM(valor)`
  sobre **1.250.000** linhas, e o PhxSql lia **20.000** — 1,6% do trabalho. O
  «5× mais rápido» que estava no roteiro não era o motor sendo rápido. A fase
  `varrer` de `examples/carga.rs` passou a ler a faixa inteira e somar o valor,
  e a medição foi refeita do zero. É o **segundo** erro deste tipo aqui — o
  primeiro favorecia o MySQL(R), este favorecia o PhxSql —, e por isso o
  `bancada/LEIA-ME.md` ganhou uma quarta regra: **mesma quantidade de
  trabalho**, não só mesma forma de pergunta.

  A prova de que agora está igual é a **soma**: os dois motores devolvem
  1.250.000 linhas e 5.576.201.000,00 — o mesmo total até o centavo, por dois
  códigos sem uma linha em comum. E o resultado **sobreviveu ao conserto**: a
  varredura continua a favor do PhxSql, por 3,3× em vez dos 5× que a montagem
  errada prometia.

- **A chave estrangeira estava marcada «pronto» e não é aplicada.** Declarada,
  gravada e reportada — mas nenhuma gravação a consulta. Virou «parcial».

- **Campo escrito errado no `config.json` era silencioso.** Quem quisesse
  trocar a porta escreveria `"porta": 5001` — o campo é `bind`. O servidor
  subia na 5000, sem uma palavra, e tudo *parecia* certo até ninguém conseguir
  conectar. Agora o arranque diz o que não reconheceu e avisa que o valor foi
  ignorado. Não vira erro: config antigo continua subindo.

- **O servidor anunciava a versão errada.** `Cargo.toml` do workspace em
  `0.1.0` enquanto o changelog ia em 0.4.0. Como `VERSAO` é
  `env!("CARGO_PKG_VERSION")`, o `ping` e o rodapé do Centro de Controle
  responderam `0.1.0` por três lançamentos. Corrigido — e o exemplo de arranque
  do `MANUAL.txt`, que mostrava a mesma `0.1.0`, junto.

- **O painel tem sete gráficos, não nove.** README e dossiê diziam nove.
  Contados: um de área, um de anel, cinco de barras. Curiosamente o `MANUAL.txt`
  estava certo — ele lista os sete sem afirmar um total.

- **Seis marcas de terceiros sem o `(R)`**: `MySQL` em `docs/REPLICACAO.md` e no
  próprio dossiê, `HFSQL` em dois módulos, `SQLite` e `Clarion` no
  `docs/PLANO.md`. Fica de fora, de propósito, uma citação literal do
  `Cargo.toml` do rusqlite.

- **A capa e o rodapé do dossiê estavam defasados.** A capa dizia 276 testes, e
  o rodapé estava parado inteiro em *0.3.0 · 19.242 linhas · 69 KB de
  interface*. E a receita de medição do `docs/dossie/LEIA-ME.md` dava um número
  de linhas de doc **diferente do publicado** — ou seja, ninguém conseguia
  reproduzir a capa. As duas coisas corrigidas.

- **A bancada não estava no dossiê.** A maior medição já feita no projeto
  existia só em `bancada/` e como uma linha «pronto» no roteiro. Virou a
  seção 16.

- **Subseções do `MANUAL.txt` numeradas 10.x e 11.x** dentro das seções 14 e 15
  — sobra de quando eram outras seções.

- **Um arquivo `pid` com um número de processo velho** estava versionado desde
  a rodada da tabela em memória, e ia dentro do zip de fontes.

- **Os 2,4 GB que a bancada cria não estavam no `.gitignore`.** Um `git add -A`
  numa hora ruim mandaria a tabela inteira para o repositório.

Três coisas entraram para que nada disso volte a acontecer calado:

- `docs/dossie/numeros-da-bancada.py` — a figura, a tabela e o diagnóstico da
  seção 16 passam a ser **gerados** de `bancada/resultados.json`. Número
  digitado envelhece calado.
- `empacotar.sh` — os pacotes de Linux e Windows das rodadas anteriores foram
  montados à mão. Pacote que ninguém consegue refazer é pacote em que não se
  deve confiar. O zip de fontes sai de `git archive`, que respeita o
  `.gitignore` de graça.
- A receita de medição do `LEIA-ME.md` do dossiê agora lista **exatamente** os
  arquivos contados.

## 5. Ninguém pediu, mas a medição aponta

<!-- pendencias:insercao:inicio -->
A bancada de 10 milhões achou um buraco só, e é grande.

**A inserção é o ponto fraco do motor.** 4.039 linhas/s contra
83.492 do MySQL(R) — **20,7× mais devagar**. E o
diagnóstico é incômodo: **2.460 s de CPU para 2.476 s de relógio** (99%), com
**0,0 MiB lidos do disco**. Não é disco, é processador — a
B+tree do `.ndx` reescrita nó a nó a cada linha, sem lote. E **piora com o tamanho**: o primeiro milhão entra a 5.089/s, o último a 3.626/s — 29% mais devagar no fim do que no começo.

Nas outras quatro o motor se defende: a varredura por faixa é
**3,3× mais rápida** (6,06 s contra 19,71 s), lendo as
1.250.000 linhas dos dois lados e chegando à mesma soma; a
atualização empata (6,66 s contra 6,49 s); a busca
pontual é 2,6× mais devagar e a exclusão 2,0×. E escreve muito
menos: 2,29 GiB contra 32,03 GiB na carga.

Contrapartida honesta: **ocupa 2,27 GiB em disco contra
0,88 GiB**, porque o `.reg` é de slot fixo — o preço do
endereçamento O(1) e da ordem de digitação.

Se algum dia sobrar uma rodada para o motor em vez de para recurso novo, é
aqui que ela rende.

*(Gerado por `docs/dossie/numeros-da-bancada.py` — não edite à mão.)*
<!-- pendencias:insercao:fim -->

## 6. As perguntas que você fez, e onde está a resposta

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

Sobre a do firewall, vale repetir a parte que corrigiu a pergunta: **não há SQL
no PhxSql**, então injeção de SQL não tem superfície. A superfície real é o nome
de database e de tabela virando caminho de arquivo — e foi exatamente ali que a
auditoria achou o furo: as sondas de travessia (`../../../etc`, `/etc`,
`C:\dados`) eram *recusadas* mas não contavam violação. Seis sondas, seis linhas
de log, zero bloqueios. Hoje nome hostil é violação grave e bloqueia na
primeira tentativa.

## 7. Duas afirmações da folha de marca que continuam falsas

Registrado no `CLAUDE.md` e repetido aqui porque é fácil esquecer: a folha diz
*ACID compliant* e *built-in replication*. **Nenhuma das duas é verdade hoje** —
não há transação, e a replicação não transporta evento. Não repetir em documento
técnico enquanto não forem.
