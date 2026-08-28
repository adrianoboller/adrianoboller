# Tudo que foi pedido, do início até aqui

Uma linha por pedido seu, na ordem em que você pediu. O estado é medido contra
o código, não contra a lembrança — foi assim que a chave estrangeira saiu de
«pronto» para «parcial», e o Centro de Controle de «pronto» para «só navega».

☑️ feito · ◐ parcial · ☐ planejado

| | # | O que você pediu | Estado |
|---|---:|---|---|
| ☑️ | 1 | `Tabela.reg` — tabela física **na ordem de digitação** | slot fixo com CRC; slot excluído nunca reaproveitado |
| ☑️ | 2 | `Tabela.ndx` — índices | B+tree, chave composta, ASC/DESC/NOCASE/único |
| ☑️ | 3 | `Tabela.bin` — binários | blocos com CRC, até 4 GiB por bloco |
| ☑️ | 4 | `Tabela.memo` — textos longos | mesmo mecanismo do `.bin` |
| ☑️ | 5 | `Tabela.log` — **toda** inclusão, alteração e exclusão com data e hora | append-only, 36 bytes por evento |
| ☐ | 6 | **Servidor MCP** | não começou. O protocolo já é JSON por linha; falta a tradução de vocabulário |
| ☐ | 7 | **Driver ODBC e OLE DB** | não começou. Depende da camada SQL |
| ☑️ | 8 | Porta 5000, configurável no `config.json` | campo `bind`; campo com nome errado agora avisa |
| ☑️ | 9 | Tudo em Rust, sem dependência | zero crates externas; compila offline |
| ☑️ | 10 | Reindex criando o `.ndx` do zero | varre o `.reg` e reconstrói |
| ☑️ | 11 | Aceitar linha de comando | `phxsql` com 10 comandos, `phxsqld` com 9 chaves |
| ☑️ | 12 | Pastas separando tabelas **e** bancos | `base/ → database → raiz → schema/` |
| ☑️ | 13 | Paginação `Nome_001.reg`, `_002`… | volume = `(rowid−1)/por_arquivo + 1` |
| ◐ | 14 | **Quantidade de registros e arquivos no create table** | a paginação funciona, mas **não há op no protocolo nem comando na CLI para criar tabela** — só escrevendo Rust |
| ☑️ | 15 | Organograma, fluxograma e dossiê | 19 seções, 16 figuras, tudo em SVG à mão |
| ☑️ | 16 | Log de IPs na porta 5000, com data e hora | JSON Lines, para caber `fail2ban` |
| ☑️ | 17 | Download dos fontes e do compilado Linux/Windows, com manual | `./empacotar.sh`, três zips conferidos |
| ◐ | 18 | **Subir o PhxSql no GitHub** | está na branch com histórico completo; **repositório próprio bloqueado**: `create_repository` responde 403 |
| ◐ | 19 | **Replicação como a do MySQL(R)**, com porta de envio e de retorno | as três portas entram e validam, o desenho está escrito; falta o **`.log` v2 com imagem da linha** |
| ☑️ | 20 | `Config_exemplo_01/02/03.json` | isolado, réplica e origem |
| ☑️ | 21 | Precisa de agentes e subagentes? | respondida |
| ☑️ | 22 | Atualizar o dossiê ao fim de cada rodada | regra permanente, no `CLAUDE.md` |
| ☑️ | 23 | Usar os logotipos onde precisar | capa, cabeçalho, favicon e paleta |
| ☑️ | 24 | Cadastro com nome, login, senha, e-mail, telefone, supervisor e poder por base | tudo isso, mais nível e chave pública |
| ☑️ | 25 | Usuário root e senha no `config.json` | PBKDF2-HMAC-SHA256, 210.000 voltas |
| ☑️ | 26 | `blacklist.json` com IP, data, hora e comando bloqueado | mais recarga automática entre processos |
| ☑️ | 27 | Comandos proibidos no `config.json` | e a auditoria achou ali um furo real, corrigido |
| ☑️ | 28 | Criar regra de firewall em quem tenta o proibido | conferido com um `iptables` falso que grava |
| ☑️ | 29 | **Base64 no login**, não em claro | feito — e o padrão é melhor: desafio-resposta, a senha não sai da máquina |
| ◐ | 30 | **Interface web parecida com o Centro de Controle HFSQL(R)** | árvore, cinco abas, painel, administração, barra de menu e barra de ferramentas — **26 das 32 operações**. Falta a edição: `inserir`, `atualizar`, `excluir`, `ler`, `buscar` |
| ☑️ | 65 | **Barra de ferramentas** com Start/Stop, Query, Usuários, Diretivas, Bancos, Duplicar, Conexões, Transações, Importar, Repair, Backup, Replicação, Server Mail, Blockchain e Ajuda | 15 ferramentas, ícone colorido; **10 funcionam**, 5 apagadas dizendo o que falta |
| ☑️ | 31 | Tabela em memória tipo Redis(R), com `SelectMemory` | **87× mais rápido**, medido |
| ☑️ | 32 | Revisar regras, corrigir defeitos, registrar em `changelog.md` | `CHANGELOG.md`, com *Corrigido* primeiro |
| ☑️ | 33 | Chave assimétrica no `config.json` como parâmetro extra | Ed25519 (RFC 8032), contra os quatro vetores oficiais |
| ☑️ | 34 | `(R)` nas marcas de outros bancos | varrido no repositório inteiro |
| ☑️ | 35 | O desenvolvimento foi adequado? Qual o custo? | respondida |
| ☑️ | 36 | Tabelas PhxSql em Android, iOS e IoT | respondida |
| ☑️ | 37 | Ele **realmente** cria a regra e bloqueia? | respondida — e a auditoria achou um furo de verdade |
| ☑️ | 38 | Ícones ☀️/🌓 para claro e escuro | 🌙 no tema claro, ☀️ no escuro, no canto direito da barra |
| ☑️ | 39 | Login com servidor/porta/usuário/senha/chave/database | a chave é opcional, conforme o `config.json` |
| ☐ | 40 | **Parar e subir o serviço pela interface**, trocando a porta | não começou. O `accept` bloqueia; exige mexer no laço |
| ☑️ | 41 | Sistema de backup | ao vivo e agendado |
| ☑️ | 42 | Checklist das perguntas feitas e respondidas | este documento |
| ☑️ | 43 | Backup em zip `Banco_Admin_Data_Horamin.zip` | manifesto SHA-256; o ZIP e o DEFLATE são escritos aqui |
| ☑️ | 44 | **Nível admin** no `config.json` e no usuário | cinco níveis: nenhum, leitor, operador, dono, admin |
| ☑️ | 45 | Comparar com o MySQL(R) em 10.000.000 | seção 17 do dossiê; refeita duas vezes |
| ☑️ | 46 | Gráficos comparativos de IO, memória e CPU | `bancada/comparacao-phxsql-mysql.html` |
| ☑️ | 47 | `tabela.bkp` clone do `.reg`, se ativo no `config.json` | e provar isso achou um defeito grave, corrigido |
| ☑️ | 48 | DataGrid com faixa de agrupamento acima das colunas | phx-grid: arrastar o cabeçalho agrupa |
| ☐ | 49 | **Triggers** | não começou. Falta decidir **em que linguagem o gatilho é escrito** — a escolha é sua |
| ☐ | 50 | **Stored procedures** | não começou. Código guardado precisa de executor |
| ☐ | 51 | **Jobs de execução** | não começou. **O mais barato dos três**: o agendador do backup já é o desenho |
| ☑️ | 52 | Dashboard com gráficos de bancos, usuários, conexões | sete gráficos e oito números, de uma chamada só |
| ☑️ | 53 | Revise o que falta | onze defeitos achados, todos corrigidos |
| ☑️ | 54 | Tabelas de feito, parcial e planejado | esta |
| ☑️ | 55 | Serve para Blockchain e servidor de e-mail por socket? | respondida, com o esquema e a restrição do SMTP |
| ☑️ | 56 | **UUID v7, UUID de 128 bits, 256 bits e `Sequence`** | conferidos contra o vetor do RFC 9562 |
| ☑️ | 57 | Por que o insert é tão lento perto do MySQL(R)? | diagnosticado e medido: era o CRC da página inteira |
| ☑️ | 58 | **Multithreads para acelerar** | onde divide: varredura em memória **1,8×** em 4 núcleos |
| ☑️ | 59 | Novo teste de 3.000.000 | feito duas vezes, isolando cada rodada |
| ☑️ | 60 | O `.bkp` espelhado existe se ativo? | sim — e provar achou o defeito do byte de status |
| ☑️ | 61 | O que dá para melhorar no insert | **3,1× no CRC** e **1,31× na unicidade**, medidos |
| ☑️ | 62 | Parar a carga de 10 milhões | parada e limpa |
| ☑️ | 63 | **Barra de menu superior tradicional** | seis menus, 22 recursos, Alt/setas/Esc |
| ☑️ | 64 | Cadê o sol e a lua? | respondida — estavam lá, o recorte da captura é que cortava |

**55 feitos · 4 parciais · 6 planejados**, de 65 pedidos.

Fora do que você pediu, entraram por medição: o CRC slice-by-8, o `descer` sem
reler a folha, a conferência de unicidade sem descida dupla, e onze correções
de defeito — três delas de perda silenciosa de dado.

---

## O detalhe de cada parcial e de cada planejado

## 2. Parcial

Existe, funciona no que promete, mas **não faz tudo** o que o pedido queria.
Cada linha diz exatamente onde para.

| # | O que você pediu | O que existe | O que falta |
|---|---|---|---|
| 1 | **Replicação como a do MySQL(R)**, com porta de acesso, de envio e de retorno | as três portas entram no `config.json` e são validadas — duas no mesmo endereço não sobem. O desenho está na seção 9 do dossiê e em `docs/REPLICACAO.md`. O `.log` **é** o binlog | o `.log` **v2 com imagem da linha**. Hoje o diário registra que houve alteração, não o que a linha virou — sem isso a réplica não tem o que aplicar. O servidor avisa alto no arranque que as portas são configuração, não serviço |
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
  seção 17.

- **Subseções do `MANUAL.txt` numeradas 10.x e 11.x** dentro das seções 14 e 15
  — sobra de quando eram outras seções.

- **Um arquivo `pid` com um número de processo velho** estava versionado desde
  a rodada da tabela em memória, e ia dentro do zip de fontes.

- **Os 2,4 GB que a bancada cria não estavam no `.gitignore`.** Um `git add -A`
  numa hora ruim mandaria a tabela inteira para o repositório.

Três coisas entraram para que nada disso volte a acontecer calado:

- `docs/dossie/numeros-da-bancada.py` — a figura, a tabela e o diagnóstico da
  seção 17 passam a ser **gerados** de `bancada/resultados.json`. Número
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

**A inserção é o ponto fraco do motor.** 11.308 linhas/s contra
86.748 do MySQL(R) — **7,7× mais devagar**. E o
diagnóstico é incômodo: **870 s de CPU para 884 s de relógio** (98%), com
**0,0 MiB lidos do disco**. Não é disco, é processador — a
B+tree do `.ndx` reescrita nó a nó a cada linha, sem lote. E **piora com o tamanho**: o primeiro milhão entra a 16.051/s, o último a 9.311/s — 42% mais devagar no fim do que no começo.

Nas outras quatro o motor se defende: a varredura por faixa é
**4,8× mais rápida** (3,94 s contra 18,97 s), lendo as
1.250.000 linhas dos dois lados e chegando à mesma soma; a
atualização empata (4,44 s contra 6,06 s); a busca
pontual é 1,9× mais devagar e a exclusão 0,9×. E escreve muito
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
| Dá para ter replicação parecida com a do MySQL(R)? | sim — dá, e o desenho está escrito | seção 9 do dossiê, `docs/REPLICACAO.md` |
| O desenvolvimento foi conduzido de forma adequada? Poderia ter sido diferente? Qual o impacto de custo? | sim | conversa |
| Como ter tabelas PhxSql em Android, iOS e IoT? | sim | conversa |
| Ele **realmente** cria a regra de firewall e bloqueia quem tenta injeção ou comando da blacklist? | sim — e a auditoria achou um buraco de verdade | seção 11 do dossiê; `docs/SEGURANCA.md` |
| Como o PhxSql se compara ao MySQL(R) em 10 milhões de registros? | sim — e o número **estava errado a nosso favor**; refeito | seção 17 do dossiê, `bancada/` |

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
