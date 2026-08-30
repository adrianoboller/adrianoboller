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
| ☑️ | 6 | **Servidor MCP** | `phxsqld --mcp` fala JSON-RPC por stdio, com `ExecutorLocal` chamando o `despachar` — o portão continua sendo um — e `stdio` no lugar do IP no log de acessos. O `tools/list` **lê o catálogo de operações** em vez de uma segunda lista escrita à mão. Teste roda o binário de verdade; senha via `PHXSQL_SENHA`, nunca em argumento |
| ☑️ | 7 | **Driver ODBC e OLE DB** | **driver ODBC 3.x de verdade**, `crates/phxsql-odbc/` — uma `cdylib` de ABI C que o gerenciador carrega por `dlopen`/`LoadLibrary` e que por dentro é um cliente comum da porta de dados (`lib.rs:398`, `SQLDriverConnect`). DSN-less, `SQLExecDirect`/`SQLPrepare`, colunas descritas com tipo, `PWD=***` na string devolvida. Provado com **73 conferências, zero falhas** pela ABI literal (`ctypes`/`dlopen` chamando o que o gerenciador chamaria) mais `isql` de verdade — e com o defeito reposto (truncamento **calado** no `SQLGetData`) a prova falha em 4. **OLE DB nativo é recusa fundamentada, não pendência**: um provider COM é só Windows e **impossível de provar aqui**; o caminho suportado é a ponte oficial `MSDASQL`, que transforma qualquer driver ODBC em origem OLE DB. `docs/ODBC.md` §6 e §7 |
| ☑️ | 8 | Porta 5000, configurável no `config.json` | campo `bind`; campo com nome errado agora avisa |
| ☑️ | 9 | Tudo em Rust, sem dependência | zero crates externas; compila offline |
| ☑️ | 10 | Reindex criando o `.ndx` do zero | varre o `.reg` e reconstrói |
| ☑️ | 11 | Aceitar linha de comando | `phxsql` com 10 comandos, `phxsqld` com 9 chaves |
| ☑️ | 12 | Pastas separando tabelas **e** bancos | `base/ → database → raiz → schema/` |
| ☑️ | 13 | Paginação `Nome_001.reg`, `_002`… | volume = `(rowid−1)/por_arquivo + 1` |
| ☑️ | 14 | **Quantidade de registros e arquivos no create table** | op `criar_tabela` no protocolo e tela **Nova tabela** com registros por arquivo, dígitos do sufixo e teto de volumes. A CLI ainda não tem o comando |
| ☑️ | 15 | Organograma, fluxograma e dossiê | 19 seções, 18 figuras, tudo em SVG à mão |
| ☑️ | 16 | Log de IPs na porta 5000, com data e hora | JSON Lines, para caber `fail2ban` |
| ☑️ | 17 | Download dos fontes e do compilado Linux/Windows, com manual | `./empacotar.sh`, três zips **conferidos de verdade**: cada um traz `MANIFESTO.sha256` de todos os seus arquivos, e o conferidor é o próprio `phxsql conferir-pacote`, que viaja dentro do pacote e roda igual no Windows — sete testes o reprovam com o defeito reposto, inclusive o arquivo **a mais**, que conferência de hash comum não vê. `pacotes/SHA256SUMS` fecha o download por fora. Antes de qualquer zip sair, quatro travas: versão igual em `Cargo.toml`/`Cargo.lock`/`MANUAL`/`CHANGELOG`, alvo e ligador de Windows conferidos com o comando exato de quem não os tem, árvore limpa para o pacote de fontes, e o `Cargo.toml` na raiz do zip. Provado no que o dono vai fazer: `cargo build --offline --release` num diretório limpo com `CARGO_HOME` vazio — 28,6 s, 30,3 s e 34,3 s em três medições —, e desse diretório extraído o `./empacotar.sh linux` remonta o pacote inteiro. `docs/EMPACOTAMENTO.md` |
| ◐ | 18 | **Subir o PhxSql no GitHub** | está na branch `claude/capacidades-disponiveis-y6auxh` de `adrianoboller/adrianoboller`, com histórico completo. **A causa não é permissão que falta: é identidade**, e isso foi medido nesta revisão. A credencial desta sessão autentica como **`EnginePrint`** (id 322529492, criada em 2026-08-29, zero repositórios públicos) — não como você. Ela **lê** `adrianoboller/adrianoboller` e enxerga a branch; o que ela não faz é criar repositório em nome de outra pessoa, e é daí que sai o 403 do `create_repository`. E um repositório criado por ela **não seria seu** — seria dela, com você de fora. Destrava com **você** criando `adrianoboller/phxsql` e dando acesso a essa app |
| ☑️ | 19 | **Replicação como a do MySQL(R)**, com porta de envio e de retorno | **funcionando**: `.log` v2 com a imagem da linha, ops `posicao`/`replicar`/`aplicar`, e o laço da réplica dentro do `phxsqld`. Medido com quatro servidores — master 28.914 linhas/s, atraso de 1,3 a 2,1 s, retrato SHA-256 de **cada linha** idêntico. Depois disso entraram os **quatro modos** (A source→réplica, B multi-master com «mais recente vence», C spare que não atende ninguém até `spare_promover`, D read replica que recusa escrita apontando o master), o **agendamento por origem** (`cada_minutos`/`hora`; ausente = streaming, e o teste que trava isso é `origem_sem_agendamento_continua_streaming`), o **assistente na tela** e a **cascata** medida. A réplica alcança: 4.273 → 17.450 eventos/s (4,08×). Continuam faltando: **long-poll** no source, espera crescente na reconexão, **TLS** no transporte, gravar a configuração pela tela, e bidirecional com mais de dois servidores. `docs/REPLICACAO.md` §13 |
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
| ☑️ | 30 | **Interface web parecida com o Centro de Controle HFSQL(R)** | árvore, abas, painel, administração, menu, ferramentas, **View Database com edição**, **gestão de tabelas** e **gestão do banco**. O «36 das 39» que estava aqui envelheceu junto com o catálogo: medido nesta revisão, o protocolo tem **108 operações** (`OPERACOES`, em `server/src/catalogo.rs`) e a tela chama **96** delas pelo nome. As 12 que ficam fora não são buracos — são as de máquina (`replicar`, `aplicar`, `bulkinsert`, `cluster_pulso`, `cluster_estado`, `spare_promover`, `catalogo`), a que acontece sozinha (`criar_schema`, quando a tela cria tabela dentro de um schema) e as que a tela alcança por expressão em vez de literal (`telemetria_ligar`/`telemetria_desligar`, no botão que alterna). Refazer a conta: casar os `api("…")` de `ui/` com os `nome:` de `OPERACOES` |
| ☑️ | 66 | **[+] na árvore** para criar database, **About no menu Ajuda**, **tela de créditos** com a fênix, e **View Database** com grade de tabelas e edição | fecha a edição de dados: `ler`, `inserir`, `atualizar` e `excluir` ganharam tela |
| ☑️ | 65 | **Barra de ferramentas** com Start/Stop, Query, Usuários, Diretivas, Bancos, Duplicar, Conexões, Transações, Importar, Repair, Backup, Replicação, Server Mail, Blockchain e Ajuda | **30 ferramentas hoje, e 27 funcionam** — medido nesta revisão contando a lista `FERRAMENTAS` de `ui/index.html`, não lembrado: as 3 apagadas são *Duplicar*, *Server Mail* e *Blockchain*, e cada uma abre uma tela dizendo o que falta e de que depende. O número anterior (20 e 16) estava parado havia rodadas — entraram Telemetria, Profiler, Diagrama ER, LGPD, DbLink, Jobs, Junção, Pivot e outras sem ninguém recontar |
| ☑️ | 31 | Tabela em memória tipo Redis(R), com `SelectMemory` | **87× mais rápido**, medido |
| ☑️ | 32 | Revisar regras, corrigir defeitos, registrar em `changelog.md` | `CHANGELOG.md`, com *Corrigido* primeiro |
| ☑️ | 33 | Chave assimétrica no `config.json` como parâmetro extra | Ed25519 (RFC 8032), contra os quatro vetores oficiais |
| ☑️ | 34 | `(R)` nas marcas de outros bancos | varrido no repositório inteiro |
| ☑️ | 35 | O desenvolvimento foi adequado? Qual o custo? | respondida |
| ☑️ | 36 | Tabelas PhxSql em Android, iOS e IoT | respondida |
| ☑️ | 37 | Ele **realmente** cria a regra e bloqueia? | respondida — e a auditoria achou um furo de verdade |
| ☑️ | 38 | Ícones ☀️/🌓 para claro e escuro | 🌙 no tema claro, ☀️ no escuro, no canto direito da barra |
| ☑️ | 39 | Login com servidor/porta/usuário/senha/chave/database | a chave é opcional, conforme o `config.json` |
| ☑️ | 40 | **Parar e subir o serviço pela interface**, trocando a porta | **feito, e o impedimento resolvido de verdade**: despertador que conecta no próprio endereço, em vez de *polling* — 100 ms de intervalo poriam 100 ms em toda conexão nova. A porta nova é **presa antes** de a velha ser solta, e o processo não é derrubado, então a web é sempre o caminho de volta. 5 testes por soquete, um deles derrubando a porta de dados e a levantando pela web |
| ☑️ | 41 | Sistema de backup | ao vivo e agendado |
| ☑️ | 42 | Checklist das perguntas feitas e respondidas | este documento |
| ☑️ | 43 | Backup em zip `Banco_Admin_Data_Horamin.zip` | manifesto SHA-256; o ZIP e o DEFLATE são escritos aqui |
| ☑️ | 44 | **Nível admin** no `config.json` e no usuário | cinco níveis: nenhum, leitor, operador, dono, admin |
| ☑️ | 45 | Comparar com o MySQL(R) em 10.000.000 | seção 17 do dossiê; refeita duas vezes |
| ☑️ | 46 | Gráficos comparativos de IO, memória e CPU | `bancada/comparacao-phxsql-mysql.html` |
| ☑️ | 47 | `tabela.bkp` clone do `.reg`, se ativo no `config.json` | e provar isso achou um defeito grave, corrigido |
| ☑️ | 48 | DataGrid com faixa de agrupamento acima das colunas | phx-grid: arrastar o cabeçalho agrupa |
| ☑️ | 49 | **Triggers** | a escolha de linguagem foi feita — a do MySQL(R)/MariaDB(R), sintaxe similar e não idêntica — e **um interpretador só** serve gatilho e procedimento (`crates/phxsql-sql/src/rotina.rs`: árvore de expressão, avaliador, e um `Numero` de mantissa `i128` com escala, **sem `f64` em ponto nenhum** — `1.10 * 3` dá `3.30`). `BEFORE`/`AFTER` × `INSERT`/`UPDATE`/`DELETE` `FOR EACH ROW`, com o `BEFORE` rodando **com a trava de dados na mão**, entre a conversão da linha e a gravação, podendo alterar `NEW` e cancelar por `SIGNAL` (`servidor.rs:7019` e `:7076`; registro por database em `rotinas.rs:336`). `docs/TRIGGERS.md`, com **17 recusas nomeadas e testadas** — quem cola um corpo do MySQL(R) descobre o que trocar pelo nome |
| ☑️ | 50 | **Stored procedures** | `CREATE PROCEDURE`/`CALL` com `IN`/`OUT`/`INOUT` (`rotinas.rs:402`), no **mesmo** interpretador do gatilho. O portão continua sendo um: cada pedido que o corpo produz sai pelo `executar_derivado` com a sessão de quem chamou, e o teste `call_nao_e_a_porta_dos_fundos_para_a_tabela_negada` trava isso. Criar e excluir exigem `administrar` e não `criar` — senão quem cria tabela penduraria um `AFTER INSERT` na tabela alheia e desviaria as linhas dos outros; `CALL` não pede nada próprio, porque nunca dá poder que a pessoa já não tinha |
| ☑️ | 51 | **Jobs de execução** | `jobs.rs`, cadastro em `jobs.json`, corridas append-only em `jobs.log`, relógio de 30 s e 4 operações, todas exigindo `administrar`. **O job roda com o poder do usuário dele** — e isso obrigou a extrair os portões comuns do `despachar` para uma função só, em vez de copiar a conferência. 8 testes por soquete, incluindo o que prova que um job de `so_le` não cria database |
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
| ☑️ | 63 | **Barra de menu superior tradicional** | nove menus e 53 itens hoje, Alt/setas/Esc |
| ☑️ | 64 | Cadê o sol e a lua? | respondida — estavam lá, o recorte da captura é que cortava |
| ☑️ | 68 | **Copiar e colar tabela** de um lugar para outro | `copiar_tabela` atravessa databases e schemas; a permissão de criar é conferida **no destino** |
| ☑️ | 69 | **Configurações gerais do servidor, do banco e dos usuários**, cada uma com sua tela | três telas, três alcances. **Leem, não gravam** — gravar o `config.json` pela web daria a uma sessão roubada o poder de abrir o firewall e criar supervisor |
| ☑️ | 70 | **SysTables e SysColumns** | o catálogo em forma de dado, e o dicionário de dados com id, caption, descrição, máscara e papel na chave |
| ☑️ | 71 | **Gerir database**: conexões, triggers, procedures, arquivos bloqueados, modo exclusivo, transações, backup/restaure e jobs | 18 itens numa tela; 14 funcionam, 4 apagados dizendo o que falta e de que dependem. O *backup/restaure* passou a restaurar de verdade (pedido 133) |
| ☑️ | 72 | **Diretivas de acesso ao banco e diretivas de acesso** | os seis portões na ordem em que fecham, e quem alcança o banco resolvido pelas três regras |
| ☑️ | 73 | **Editor de menu** para trocar o nome exibido | 81 rótulos; fica no navegador de quem mexeu, não no servidor |
| ☑️ | 74 | **Configurações e diretivas das tabelas** | a geometria decidida na criação, os índices e chaves, e o que a tabela herda do servidor |
| ☑️ | 75 | **Cadastro de campos** com id automático, nome, caption, descrição, tipo, tamanho, máscara e chave primária/estrangeira/composta | **mudança de formato**: esquema `PSCH` v3. O `id` é UUID v7 e nunca muda; o papel na chave é derivado dos índices |
| ☑️ | 76 | **Tabela particionada** com grade de gestão: por faixa de quantidade, mensal, bimestral, semestral ou anual | **mudança de formato**: o volume corta pelo calendário, e cada volume grava a própria fronteira no cabeçalho |
| ☑️ | 79 | **Seção de cache, memória, CPU, threads e usuários no `config.json`** | seção `recursos`, com sete ajustes. `cache_paginas` passou a **valer de verdade** na 0.17.0 — era lido e mostrado sem nada por trás, e agora é o teto do cache de páginas do `.ndx`. `memoria_max_mb` continua lido e não imposto |
| ☑️ | 80 | **Validar e revisar o motor de insert; deixar a gravação mais rápida** | medido: **95% do tempo era `fsync`**. Durabilidade configurável dá **20,4×**. E a medição achou uma **perda silenciosa de dado** sob gravação concorrente, corrigida |
| ☑️ | 81 | **Tabela `sequences` na raiz do banco**, com todas as tabelas e um BigInt ajustável pelo admin | operações `sequencias` e `ajustar_sequencia`. O contador continua no cabeçalho de cada `.reg`: a operação junta para mostrar, e não cria uma segunda cópia que divergiria |
| ☑️ | 82 | **Bancos em pastas, cada schema uma subpasta** | já era assim desde o início — conferido: `dados/loja/matriz/estoque.reg` |
| ☑️ | 83 | **Comandos SQL reconhecem `matriz.estoque` e `filial.estoque`** | op `sql` ligada ao servidor pelo portão que já existe (`executar_derivado`), com o teste `o_sql_nao_e_a_porta_dos_fundos_para_a_tabela_negada` provando nos dois sentidos. Ligar achou o que os 44 testes da crate não podiam achar: `WHERE id = 2` chegava como texto e era recusado — o motor alargou (coluna inteira aceita inteiro em texto, que é o que ODBC vai mandar), o tradutor não apertou |
| ☑️ | 77 | **Group dinâmico pelas colunas na grade**, como o Janus GridEX(R) e o DevExpress(R) | já havia arrastar e multinível; entraram ordem por nível, rodapé por grupo com o total na coluna, total geral e expandir/recolher tudo |
| ☑️ | 78 | **Botão que monta pivot dinâmico com assistente**, pedindo as tabelas envolvidas | operação `pivotar` no servidor com *hash join*, seis resumos e granularidade de data; assistente de três passos na tela |
| ☑️ | 84 | **Botão DbLink na barra** e **definições do DbLink** no menu Configurações | cadastro com apelido, endereço, credencial e teto; a senha nunca sai em JSON. Nasce **somente-leitura** |
| ☑️ | 85 | **Conectar em banco de fora e ver as tabelas na grade tipo Janus(R)** — MySQL(R) primeiro | protocolo do MySQL(R) escrito à mão, só `std`; testado contra um MySQL(R) 8.0.46 de verdade. A grade é a **mesma** das tabelas daqui |
| ◐ | 86 | **Depois testar com PostgreSQL(R) e outros** | **cliente, dialeto e ligação prontos** — SCRAM contra o vetor do RFC 7677, SQL por motor, e as cinco operações do DbLink reescritas para não saberem qual motor atendem (o `servidor.rs` delega). Provado por soquete contra um servidor de protocolo próprio, byte a byte, nos dois sentidos. **O que falta é só o que o nome do pedido diz**: a prova contra um PostgreSQL(R) de verdade, que não existe nesta máquina — o que ela exige está em `docs/DBLINK.md` |
| ☑️ | 87 | **Monitor de espaço em disco no dashboard** | uma barra por caminho que o servidor usa — o `base`, o destino do backup e o que estiver em `alertas.caminhos`. A conta é sobre `usado+livre`, como a do `df` |
| ☑️ | 88 | **Definir no config o local de armazenamento** (`C:\database`, `D:\database`) | é o campo `base`, e sempre aceitou caminho absoluto. O que faltava era a tela mostrar o caminho **já resolvido**: relativo vale a partir de onde o servidor foi iniciado, e subir por outro caminho passa a ver outro banco |
| ☑️ | 89 | **Alerta de falta de espaço por e-mail**, configurado no config | seção `alertas`, com dois limites no OU e silêncio entre avisos. Cliente SMTP escrito aqui — **sem TLS**, serve para relé interno |
| ☑️ | 90 | **Monitores de placa de rede, CPU, memória e HDs no dashboard** | do `/proc`, com taxa entre duas amostras; renovam sozinhos a cada quatro segundos. **Só no Linux** — fora dele a tela diz que não sabe medir, em vez de mostrar zero |
| ☑️ | 91 | **Operações básicas de union, inner join e as outras do diagrama** | as sete figuras (`interna`, `esquerda`, `direita`, `completa`, `so_esquerda`, `so_direita`, `so_dos_lados`) mais `UNION` e `UNION ALL`. Na tela se escolhe **clicando no desenho de Venn**, com o SQL equivalente escrito embaixo. Chave composta, e nulo que não casa com nulo, como no SQL |
| ☑️ | 92 | **Revisar o help do MySQL(R) e do MariaDB(R) e ver o que melhorar** | comparado contra os dois help embutidos rodando (705 e 833 tópicos). Entraram: erro com **código estável**, `sessoes` (PROCESSLIST), `encerrar_sessao` (KILL), `estatisticas` com percentis/histograma/mais-lentas/por-tabela, `checksum` e tempo no ar. O que ficou fora está em `docs/COMPARACAO.md` **com o motivo** |
| ☑️ | 93 | **Exportar as tabelas para xlsx, json, xml, html, csv, docx e txt** | os sete, escritos aqui. XLSX e DOCX são ZIP de XML, e o projeto já escrevia ZIP com DEFLATE — planilha com cabeçalho pintado, zebra, painel congelado e autofiltro; data como número com formato, não como texto. Conferido com leitores independentes |
| ☑️ | 94 | **O dossiê estava esquecendo o `.bkp`** | e no pior lugar: a seção do **fluxo de gravação**. O espelho não aparecia no desenho, e parecia uma cópia feita depois — ele é escrito no mesmo instante. Corrigido no dossiê, no `FORMATO.md`, no `MANUAL.txt` e no `README` |
| ☑️ | 95 | **Integrar o MULTILINK no DbLink** | **bloqueado como está**: o pacote traz só binários (`.rlib`), sem fonte, compilados com rustc 1.98 contra o 1.94 daqui — provado, não suposto. E um `.rlib` é dependência externa, que a regra do projeto proíbe. O caminho que funciona está descrito em `docs/MULTILINK.md`: falar com ele por **protocolo**, e não por link. **Fechado nesta revisão, e não por desistência**: o destino do pedido — ver a tabela do outro banco pelo DbLink — foi alcançado pelo **terceiro caminho** que o próprio `MULTILINK.md` descreve, «ler o driver e portar o que se precisa». O cliente MySQL(R) tem 728 linhas escritas aqui (`server/src/dblink/mysql.rs`) e o PostgreSQL(R) 721 mais 278 de SCRAM (`server/src/pg/`), tudo só com a `std`; o pedido 131 provou contra um MySQL(R) 8.0.46 real e o 132 pôs a sincronia por cima. **Não sobra trabalho aqui: sobra uma recusa medida e um destino entregue** |
| ☑️ | 96 | **Registro apagado fisicamente vai para o `.trash` antes de sair do `.reg`** | e o disco **confirma** antes de o slot ser liberado. Guarda o *payload* byte a byte **mais o conteúdo dos anexos** — com ponteiro, a foto voltaria sendo a de outra linha, porque o bloco do `.bin` é liberado na exclusão. Só quem tem `administrar` lê |
| ☑️ | 97 | **Coluna `SOFTDELETED` em todas as tabelas** | entra sozinha na criação, no fim da lista para não deslocar as colunas do usuário. Marcar tira a linha das listas e ela continua inteira no `.reg`; `restaurar` desfaz. Esquema `PSCH` v3 → v4, e tabela v3 continua abrindo |
| ☑️ | 98 | **`.reason` com UUID, data, hora, motivo e quem excluiu** | UUID v7 do próprio evento, e a identidade da linha em texto — «rowid 4173» não diz nada seis meses depois. Sobrevive à linha: o expurgo é registrado antes de o dado sair. Só `administrar` |
| ☑️ | 99 | **Motivo de exclusão obrigatório, marcado na criação da tabela** | caixa na tela de Nova tabela; marcada, o motor recusa qualquer exclusão sem frase escrita, **antes** de qualquer gravação |
| ☑️ | 100 | **Botões e combos no ambiente** | diálogo de exclusão com os dois modos e o campo do motivo (não um `confirm()`, que só sabe perguntar sim ou não); par «ativas / excluídas» na grade com botão de restaurar; telas de Lixeira e de Motivos no menu Tabelas e na barra |
| ☑️ | 101 | **Cifrar e compactar `.log`, `.trash` e `.reason`** | **cifra ligada; compactação medida duas vezes e recusada duas vezes.** ChaCha20-Poly1305 (RFC 8439, vetores oficiais) nos três diários, **desligada por padrão** — com o defeito «cifra imposta» reposto, 43 testes antigos quebram. Nonce do offset que o arquivo já tem, chave por PBKDF2 e por volume, replicação continuando com imagens decifradas pela sessão. E o corte do diário virou `recursos.diario_volume_mib`: remedido, compactar poupa 14,7% no melhor caso contra 2,1× mais que o `.ndx` daria — a recusa ficou com dois números em vez de um |
| ☑️ | 102 | **Paginação de Big Table por cursor (keyset)** | `depois`/`antes` no `varrer`, cursor bidirecional na grade, `pular` como compatibilidade. E o defeito que estava embaixo: o `varrer` lia a **tabela inteira com os anexos** para devolver 200 linhas — 3.176 ms numa tabela de 800 mil. Pelo cursor, não mensurável |
| ☑️ | 103 | **Campo `rownum` sequencial e automático em todas as tabelas** | coluna de sistema, o motor preenche, nunca reaproveita número, alterar não renumera. `rowid_do_rownum` acha por bissecção — 20 leituras num milhão, sem índice |
| ☑️ | 104 | **Partição alfanumérica: `Clientes_A.reg` … `Clientes_Outros.reg`** | 37 volumes fixos, o rowid sai de `(balde−1) × rpa + slot` — a inversa exata da conta de sempre, então nenhum caminho de leitura mudou. A ordem de digitação sai do rowid e vai para o `rownum` |
| ☑️ | 105 | **Arquivo `.pag` com a instrução da partição em JSON** | descritor **gerado**, com a conta do endereço por extenso; o motor nunca o lê. Segunda cópia seria segunda verdade |
| ☑️ | 106 | **Integrar o MULTILINK — segunda análise, agora com os fontes** | o motivo anterior caiu: os fontes vieram. O novo é maior e medido: o `Cargo.lock` resolve **596 pacotes, 14 locais → 582 crates externas**, e cinco são obrigatórias mesmo sem nenhuma *feature* (`serde`, `serde_json`, `log`, `tokio`, `ml-driver-api`). Linkar traria um runtime assíncrono inteiro para dentro do `phxsqld`. Há um caminho novo que os fontes abrem: os `ml-driver-*-ffi` são `cdylib` com ABI C limpa, e ABI C se chama da `std` sem crate nenhuma — mas põe código proprietário com licença por máquina dentro do processo do banco. O caminho recomendado continua sendo **por protocolo**, agora como executável separado; `docs/MULTILINK.md`. **Fechado pelo mesmo motivo do 95**: o DbLink nativo chegou ao destino sem as 582 crates, e a recusa está medida em vez de suposta. Se um dia um driver do pacote (Sybase, AS/400) for preciso, ele volta como pedido **novo** — e o caminho já está escrito, inclusive o preço de cada um deles |
| ☑️ | 107 | **Salto para uma página específica** | `pular` deixou de andar: quando a posição de uma linha **é** o `rownum` dela, o começo da página sai de uma bissecção. Medido em 200.000 linhas pelo protocolo: 6 ms contra 131 ms no fim da tabela, e **plano** com a profundidade. Caixa «ir para a página» na grade — 116 ms no navegador, com o desenho. Contar voltou a ser barato: `visiveis = registros − marcadas`, os dois do cabeçalho |
| ☑️ | 108 | **Carga em lote — várias linhas de uma vez** | `inserir_lote` no protocolo, `phxsql importar` na linha de comando. Medido com 20.000 linhas pela rede: **2.715 → 25.985 linhas/s (9,6×)**. O ganho não é do disco: é de abrir a tabela, tomar a trava e sincronizar UMA vez em vez de vinte mil |
| ☑️ | 109 | **Tela para colar JSON, CSV, TXT, HTML ou XML** | os cinco formatos, com o motor adivinhando qual é. A primeira linha manda, e as colunas casam pelo **nome** e não pela posição. `importar_conferir` mostra o que entendeu antes de gravar; o botão de gravar só acende depois disso |
| ☑️ | 110 | **Teste de replicação com três servidores espelho** | `bancada/replicacao/`: `montar.py` sobe Master + Slave01/02/03, `medir.py` mede atraso por tipo de escrita, vazão, queda e retomada. Compara um SHA-256 de **cada linha**, com o rowid junto — e não a contagem, que não acharia uma linha que atravessou errada. Cascata Master → Slave01 → Slave03 também medida |
| ☑️ | 111 | **A réplica acompanhar a escrita do master** | **acompanha: 4.273 → 17.450 eventos/s por réplica (4,08×)**, e as três juntas aplicam ~52.000/s contra 34.048 que o master escreve. O alcance de 100.000 eventos caiu de 18,7 s para **5,7 s**. E a causa registrada aqui estava **errada**: acusava `aplicar` de reencodar o payload, e isso custa **0,27 µs** — `aplicar_evento` são 16,15 µs contra 15,88 de uma inserção local. Os 229 µs por evento estavam **no source**: servir «500 eventos a partir de P» varria os P anteriores lendo o cabeçalho de cada um, e alcançar 100.000 custava **4,07 s só ali** (`--example custo-do-desde`); a marca de posição levou a **0,09 s, 45×**. Mais o laço, que dormia depois de **toda** rodada e não só das vazias, e o `bytes_para_hex`, que fazia um `format!` por byte |
| ☑️ | 112 | **Analisar as sugestões de arquitetura (WAL, MemTable, group commit, LSM)** | `docs/DESEMPENHO.md`, com a medição que muda o alvo: **83,5% do tempo de uma inserção está no `.ndx`**, e o arquivo de dados — o que as propostas querem substituir — já é *append-only* e custa 16,5%. Das dez propostas, cinco já existem, duas miram um gargalo que não é o nosso, uma quebraria a ordem de digitação, e duas são reais |
| ☑️ | 113 | **Atacar os 83,5% do `.ndx`** | **medido, e o alvo era outro** — não era localidade de chave, era reler e recalcular CRC-32 da mesma página. Um cache de páginas levou a inserção de **44,4 → 18,5 µs**; o cabeçalho do `.reg` que reserializava o esquema, a 17,0; o do `.log`, a 15,9; o do `.ndx`, que gravava 4 KiB por chave, a 14,5; o CRC slice-by-16, a 13,1; e o **cache write-back**, a **7,5 µs — 2,19× só nesta rodada**. `docs/DESEMPENHO.md` §2 a §4.8 |
| ☑️ | 114 | **Índice não único fora do caminho crítico** | **a peça que faltava está feita, e o item em si foi medido e recusado.** `construir_em_lote` monta a B+tree sem descer nenhuma vez — 7,72 s → **0,31 s** num milhão de chaves (**23× a 25×**), e todo `reindexar` e todo reparo de índice andam nisso. O enchimento de folha, 80%, é medido e não herdado. Já o **adiar** em si: o 1,59× vale para tabela vazia, mas `reindexar` refaz sobre a tabela **inteira** — carregando M numa tabela de N, o ganho é 1,22× quando M=N e vira **prejuízo abaixo de M≈N/3** (`--example adiar-vale-quando`). E cobraria marcar **índice suspenso no formato**, cujo defeito é busca respondendo errado em silêncio depois de uma queda. Fica fora com o número na mesa; o que o faria valer é **fundir** a série ordenada na árvore existente, e não refazê-la |
| ☑️ | 115 | **Vídeo longo em MP4, do login à replicação** | `docs/video/`: 5m13s gravados contra o servidor de verdade, com o Playwright dirigindo a interface e a legenda injetada na própria página. Dezessete capítulos, e o 16 é o que nenhum vídeo de produto tem — o que ainda falta. **Ele achou três defeitos** que ler o código não acharia |
| ☑️ | 116 | **Profiler na barra de ferramentas** | vê o que chega pela porta **antes de virar dado** — o ponto de captura é uma linha antes do despacho, então o pedido que trava aparece como «em curso». Filtra por banco, usuário, operação e só-escrita; grava num `.txt` no caminho escolhido. A senha é redigida **analisando** o pedido, nunca recortando o texto; pedido que não é JSON vira o tamanho em bytes. Observa as duas portas e não observa a si mesmo |
| ☑️ | 117 | **Cores da ação nos botões** | verde inclui, amarelo altera, rosa marca, vermelho exclui de vez, azul consulta. Contorno e não fundo cheio — fundo laranja com texto escuro em cima já tinha ficado ilegível uma vez. No diálogo de excluir o botão troca de cor junto com o texto |
| ☑️ | 118 | **Rodar em Docker** | imagem `scratch`, sem shell nem gerenciador de pacotes — só possível por não haver dependência externa. Exige o alvo **musl**: medi, o padrão linka `libc.so.6` e o carregador dinâmico, e `FROM scratch` não subiria. **O `docker build` deixou de ser hipótese**: com o daemon no ar (29.3.1), o pedido 142 construiu e rodou a imagem `scratch` mais de dez vezes por corrida — 6,42 MB de camada, 2,69 comprimidos, 9,11 no `docker images` (três comandos, três números). Duas coisas quebradas apareceram ao tentar: o `phxsql/Dockerfile` **não construía** (`COPY exemplos/Config_docker.json` com os modelos morando só na raiz do repositório) e o par de modelos **não replicava entre si** (o do master não tinha bloco `replicacao`, então subia isolado e a réplica não teria o que aplicar). As duas corrigidas. O que continua **não** provado aqui é a compilação DENTRO do contêiner: o `rustup target add` do estágio construtor não alcança `static.rust-lang.org`, porque a saída desta máquina passa por um proxy que intercepta TLS e o contêiner de build não confia na CA dele — limite do ambiente, não do arquivo |
| ☑️ | 119 | **Várias instâncias em portas diferentes** | já era assim: cada `phxsqld` lê o `config.json` do diretório em que foi iniciado. Provado com quatro de uma vez em `bancada/replicacao/` e com três em contêineres |
| ☑️ | 120 | **Chave composta livre e única** | as duas já existiam no formato; faltava teste que as separasse. A única recusa **antes de gravar**, e a recusa não consome slot |
| ☑️ | 121 | **Analisar o PDF do HFSQL(R) contra o projeto** | `docs/HFSQL.md`, item por item. O que falta, em ordem de valor: direito no nível da **tabela**, índice de texto completo, índice parcial, ordenação linguística, e a **janela de conflito de escrita** |
| ☑️ | 122 | **Analisar o DBeaver: o que dá para reaproveitar** | `docs/DBEAVER.md`. Código: não vale — Apache 2.0 permite, mas seria trazer o Eclipse inteiro. Ferramenta: vale muito, e os três caminhos exigem a **mesma** camada SQL |
| ☑️ | 123 | **Janela de conflito de escrita** | feito **sem mudar formato**: a versão por registro do `.reg` estava lá desde a v1. `ler` devolve a versão com `"com_versao"`, `atualizar`/`excluir`/`restaurar` conferem a versão que o cliente mandar, e a recusa é o erro **3004 `CONFLITO`**. A janela mostra as três colunas do PDF e vai além dele: **já vem marcado quem mexeu em cada coluna**, então dois que editaram campos diferentes saem com os dois trabalhos. A conferência é **pedida, não imposta** — cliente antigo continua gravando |
| ☑️ | 124 | **Direito no nível da tabela** | `"tabelas"` dentro do objeto da base, e a regra da tabela **substitui** a da base ali — o que permite tirar `folha` de quem lê o banco inteiro **e** dar `clientes` a quem não lê o banco nenhum (interseção só resolveria o primeiro). O portão continua sendo um só; `juntar` e `unir` ganharam conferência própria porque não têm o campo `"tabela"` que ele lê. A árvore e o catálogo passaram a listar só o que dá para abrir. 9 testes |
| ☑️ | 125 | **Marcar coluna como dado pessoal (LGPD/GDPR)** | PSCH **v6**, três graus (`nao`/`pessoal`/`sensivel`, LGPD art. 5º I e II), com o byte no **fim** do bloco para quem lê v5 parar antes. Op `dados_pessoais` audita a base — e como ela **não tem campo `tabela`** (o furo do `juntar`/`unir`), filtra tabela a tabela por dentro. Não adivinha por nome; devolve quantas colunas ficaram sem classificação. Mais a tela que audita, que diz *que não sabe* quando o esquema não traz a marca |
| ☑️ | 126 | **Cluster: endereço único, eleição e promoção automática** | `crates/phxsql-server/src/cluster.rs`: pulso, época, eleição por maioria com desempate por prioridade (`vencedor`, `:115`), promoção (`:334`) e **rebaixamento sozinho** ao ver época maior (`:348`). Escrita numa réplica devolve `REDIRECIONA host:porta` (erro 4003) — endereço único **pela semântica do protocolo**, e não por VIP, que é infraestrutura e não banco. Medido em `bancada/cluster/`: a escrita volta em **3,9–4,3 s** com janela de 4 s. **Sem o bloco `cluster` no `config.json` nada muda** — nenhuma thread sobe, nenhum portão muda —, e o teste que trava isso é `sem_o_bloco_cluster_nada_muda`. O `docs/CLUSTER.md` §2 diz também o que ele **não** garante: não é multi-master, não há balanceador embutido, e a lista de nós é do arquivo |
| ☑️ | 127 | **Diagrama ER e editor de modelo** | **as três metades entraram.** O diagrama é `ui/diagrama-er.js` (sete defeitos achados abrindo no navegador, e não lendo). O **editor** é o mesmo arquivo em modo de edição: arrastar a caixa pelo título move a tabela — e a pega é um retângulo transparente por cima do título, senão o alvo seria a letra —, e arrastar a linha de uma coluna até a coluna de outra **declara a chave estrangeira**, que o cartão da tabela também exclui. Por baixo, `criar_tabela` declara FK pelo protocolo, `duplicar_tabela` preserva, e um teste trava que *declarar não é aplicar*. **A terceira, que faltava, é o sprint 25:** `acrescentar_coluna` altera a estrutura de uma tabela que já tem dado, e o cartão deixou de dizer que não dá — ele abre o formulário que funciona. O `.reg` é reescrito slot a slot na mesma ordem, e por isso o **rowid não muda** e o `.ndx` **não é tocado**. Medido, e não inferido: **0,553 µs por linha, dez milhões de linhas em 5,53 s** (`--example custo-do-alter`) — o sprint dizia «a casa dos minutos». Continua faltando trocar tipo/largura de coluna existente e tirar coluna |
| ☑️ | 128 | **`BULKINSERT(true/false)`: a tabela reservada para a carga** | reserva exclusiva por conexão, com erro **4002 `EM_CARGA`** para os outros — nomeando quem reservou e com `repetir: true`, que é o que separa «espere» de «você não pode». **1,53× medido** (43.500 → 66.500 linhas/s), porque reservada a janela de durabilidade não fecha e a carga vira um `fsync` só. Duas redes contra reserva órfã: a queda da conexão solta na hora, e `recursos.carga_prazo_min` solta o soquete pendurado. Só pela porta de dados. 10 testes mais a prova pelo soquete em `bancada/carga/bulkinsert.py` |
| ☑️ | 129 | **O motor SQL tem de conhecer o `BULKINSERT`; e o prazo, no `config.json` e na tela** | o prazo já era `recursos.carga_prazo_min` (padrão 30 min) desde o 128; entrou a **tela de configuração explicando cada ajuste** — com a seção «Cargas em andamento» listando quem reservou o quê — e o **`docs/SQL.md`**, que diz o que a camada SQL precisa saber antes de existir. `BULKINSERT` não é açúcar sintático: é palavra reservada, vale para a **sessão** (um driver que multiplexa conexões quebra a exclusividade sem avisar) e o `EM_CARGA` tem de virar *serialization failure* no SQLSTATE, não *access denied*. E a frase que o documento repete alto: **não é transação** — ele reserva a tabela, não desfaz nada |
| ☑️ | 130 | **`phxsqlcmd`: interface terminal com todos os comandos, `/help` e `/help comando`** | crate `phxsql-cmd`, autenticando pelo mesmo desafio-resposta da réplica. O `/help` **vem do servidor** (op `catalogo`: **108** operações descritas por dados (eram 79 quando o `phxsqlcmd` nasceu; a contagem é da constante `OPERACOES` em `server/src/catalogo.rs`), com um teste que deriva a lista do próprio `despachar` — operação nova não nasce sem descrição, e ajuda escrita à mão não existe para envelhecer). 9 testes por soquete; o soquete achou o que a unidade não achava (o partidor comia as aspas do JSON). Sem histórico/setas nesta rodada, dito no `--help` |
| ☑️ | 67 | **Botão e menu Tabelas** para gerir as tabelas do banco: nova, estrutura, editar conteúdo, partições, duplicar, reparar tabela, reparar índice e excluir — e **Gestão de transações** no menu de ferramentas | as oito operações funcionam de ponta a ponta; três delas (`criar_tabela`, `duplicar_tabela`, `excluir_tabela`) nasceram aqui, e `criar_schema` — prometido na documentação e nunca despachado — junto |
| ☑️ | 131 | **Prioridade no MULTILINK: conectar no MySQL(R) e ver a tabela `clientes` do outro lado do DbLink** | provado contra o **MySQL(R) 8.0.46 real**, não o servidor falso: `caching_sha2_password` pelos dois caminhos documentados, `dblink_tabelas`/`dblink_estrutura`/`dblink_ler` trazendo as 5 linhas com o DECIMAL certo — e a grade da tela somando sobre dado que nunca esteve num arquivo nosso. Roteiro de refazer em `docs/DBLINK.md`. É o DbLink **nativo, zero dependências** — o destino do pacote MULTILINK, alcançado sem as 582 crates dele |
| ☑️ | 132 | **Assistente de conexão DbLink**: cria a conexão, testa, escolhe base e tabelas ligadas, e o **job de sincronia** gravando entre si automaticamente | cinco passos que só avançam com o anterior provado, e por baixo a **sincronia de tabelas primas**: convergência de estado pela chave, sentido (puxar/empurrar/dois) e dono (aqui/lá) **por linha**, colunas casadas **por nome**, empurrão reentrável (`ON DUPLICATE KEY UPDATE`), teto com recusa clara. Exclusão **não** viaja — limite de desenho, provado no estágio 5 da prova. `bancada/dblink/prova-sincronia.py` roda os 7 estágios contra o MySQL(R) vivo (inclusive o job puxando sozinho); o assistente foi exercitado no navegador e o exercício achou a árvore que não se remontava. 15 testes de unidade das partes puras, com o defeito do sinal do decimal provado nos dois sentidos. `docs/DBLINK.md` |
| ☑️ | 133 | **Validar o profiler e o log dele em `.txt`** | validado por **soquete**, contra servidor de verdade, e a validação achou quatro coisas. A **redação passou**: 20 pedidos torcidos — chave escapada em `\u0073enha`, maiúscula, espaço antes dos dois-pontos, aninhamento fundo, lote de 200 linhas, corpo malformado — e a senha não apareceu em nenhum, nem no anel nem no arquivo; repondo o defeito (recorte no lugar da análise) caem sete testes. O que **não** passou: (1) o `.txt` aceitava **linha forjada** — um `"op"` com quebra de linha dentro deixava no arquivo uma segunda linha que se lia como evento de outro IP; (2) o profiler **não era só do administrador**, apesar de a ficha dizer que era — quem tem `bases: {"*": {administrar}}` e é leitor passava pelo portão geral, e por aí leu a **linha** de uma tabela que o servidor acabara de negar a ele *e* mandou o servidor escrever num arquivo escolhido; (3) com o **disco cheio** (tmpfs de 64 KB, 400 pedidos, 223 linhas gravadas) a tela seguia dizendo «gravando em …»; (4) `{"senha ":…}` e `["senha",…]` escapavam da redação. Consertados os quatro, mais o `terminou` que varria o anel **do mais antigo** quando o evento procurado é sempre o mais novo. o que ficara **de fora e anotado** — o `.txt` não rotacionava, 345 B por pedido, sem teto, 1,2 GB/hora a 1.000 pedidos/s — **entrou nesta rodada**: rodízio **por tamanho** (`profiler.arquivo_mib` × `profiler.arquivos`, padrão 64 × 4 = teto de 320 MiB), com a conta saindo do servidor já multiplicada e a tela dizendo quantas vezes o arquivo já virou. Por tamanho e não por tempo porque o perigo é disco: um rodízio diário daria 29 GB a mil pedidos/s e 30 MB a um — a mesma política com mil vezes de diferença. `arquivo_mib: 0` devolve o comportamento de antes, com teste. E escrever o rodízio achou um **furo que já existia**: o cabeçalho de `ligar` interpola a descrição do filtro sem reduzir a uma linha, e o filtro vem do pedido — era a linha forjada de volta, pela porta que o evento não cobria. `docs/SEGURANCA.md` §10, `docs/DESEMPENHO.md` §2.3, 6 sondas em `bancada/profiler/` |

| ☑️ | 134 | **Falta o botão restaurar** — backup que não restaura não é backup | `restaurar_backup` com dois modos: **com outro nome** (o padrão — não destrói nada, não para serviço nenhum e não segura a trava durante a cópia) e **por cima** (exige a porta de dados parada, nenhuma conexão aberta e `"confirmar":true`; o database substituído **não é apagado**, sai da raiz e o caminho volta na resposta). O SHA-256 de cada arquivo é conferido **antes** de o destino ser tocado, num palco fora da raiz: backup podre não vira database pela metade. Mais `backups`, que lista as cópias lendo só o manifesto de cada ZIP; o leitor de ZIP com INFLATE completo (os três blocos da RFC 1951, o dinâmico provado por vetor da zlib); a tela com as duas formas lado a lado; e o **botão na barra**, ao lado do Backup — botão que não se acha não existe. O portão próprio fecha a porta dos fundos que o campo `"database"` não enxerga: o banco que vem DENTRO do backup. 26 testes novos (1.132 no total), cada guarda provada com o defeito reposto, mais a prova pelo navegador nos dois temas. `docs/RESTAURACAO.md` |

| ☑️ | 135 | **Bateria de testes de backend e de frontend, e avaliação do design** | `testes-web/`: treze casos que sobem o próprio servidor (portas 6200/6201), entram pela tela de login e percorrem **120 telas** nos **dois temas**, reprovando em três canais de erro — `pageerror`, o recado vermelho da barra e o do painel — porque o `ligarMenu` captura toda exceção e ela nunca vira `pageerror`. A bateria **recusa rodar com binário velho**: a página é `include_str!`. Achou seis defeitos que 1.106 testes verdes não achavam — a tela de LGPD lendo um campo que o servidor nunca mandou, a tela de entrada em branco por **12,7 s** onde a rede engole a fonte da marca (**116×**), três mordidas do CSS global e um contraste de 3,85:1 no tema claro. E a varredura do portão de permissão achou **três** operações que leem a base inteira sem o campo `tabela` que o portão confere: `pivotar` (a tabela negada como lado da junção — dado, nome e contagem), `sequencias` e `posicao`. `docs/TESTES.md` |

| ☑️ | 136 | **Criptografia dos dados: cifrar o valor da coluna marcada como dado pessoal** | escolha **(c)** do dono, entre as quatro medidas: (a) slot inteiro 0,59 µs/linha, (b) página do `.ndx` 0,23 µs, **(c) coluna marcada 0,10 µs (1% da inserção)**, (d) arquivo inteiro **194 ms para ler uma linha** — 320.000×, a saída que não existe. Cifra as faixas marcadas **no lugar** (o ChaCha20 é de fluxo: não muda o tamanho, nenhum offset se move), com **uma** etiqueta por linha no fim do slot; `.reg` versão **5**, cabeçalho de 192 bytes. `Memo`/`Bin` marcados são selados antes de virar bloco, com nonce de 24 bytes à frente. Nonce = rowid + volume + versão + **8 bytes sorteados nos que já eram reservados** — zero de formato. AAD amarra o endereço: copiar o slot 5 sobre o 9 com o CRC certo **não passa**. Entrou junto **XChaCha20-Poly1305/HChaCha20** com os vetores do draft-irtf-cfrg-xchacha-03 e um gerador de bytes do processo (apagamento rápido de chave). **O `.ndx` continua em claro, e há teste que prova o vazamento** — `SEGURANCA.md` §11.3 |
| ☑️ | 137 | **FrogCript como modo escolhido, com o aviso escrito** | `cifra.modo: "frogcript"` — transposição, duas camadas e a direção escondida, **salto e separador parametrizáveis**. O padrão continua sendo o AEAD. O documento diz o que ele acrescenta (formato), o que **não** acrescenta (força: é a §9 do próprio autor) e o que custa, **medido**: 2,77 µs e 189 bytes contra 0,10 µs e 38 do AEAD; o `frogcript.py` de referência custa **1.137 ms por valor** (410.000×) porque deriva a chave 4 vezes **por valor**, e ~397 bytes (18× o texto). **Sem AES**: a estrutura roda sobre o ChaCha20 da casa, e por isso **não há compatibilidade com o que foi cifrado em Python** — escrito, não escondido. Escrever AES é decisão do dono e está na mesa com o custo (`SEGURANCA.md` §11.4) |

| ☑️ | 138 | **Os data grids devem ter esses recursos** | Levantadas as **quatro** grades do console (Conteúdo, DbLink tabela, DbLink SQL, Junção/União) e cruzadas com as duas fontes do pacote. O `phx-grid` do pacote (0.6.0 e 0.7.0) é **mais velho** que o nosso, e o `phoenix_data_grid_x` v1–v38 é outro produto (DataFusion, Timescale, OTLP, Dioxus) — **recusa fundamentada**, não pendência: é a parede do #95. O que o cruzamento achou foi que **a capacidade existia e a tela não ligava**: linha de filtro no cabeçalho, congelar coluna e seleção estavam escritos no `phx-grid` desde a 0.6.0 e desligados nas quatro telas. Ligados, mais **exportar a vista** (o que está na tela, com filtro, ordem e colunas — 44 linhas na vista, 44 no arquivo, provado pelo download de verdade), **layout lembrado** (largura/ordem/ocultas/congeladas/página; filtro e ordenação **não** — filtro que volta sozinho é a mesma mentira com uma noite de intervalo) e **duplo clique abre a ficha**. Exercitar achou **quatro defeitos já em produção** que ler o código não acha: (1) o funil da coluna estava **ilegível** — caixinha de marcar de **204 px** por cima do valor e «Blumenau» como «BLUMENAU», a quarta vez que o CSS global morde componente novo; (2) o menu de Colunas **nunca montava** (defeito do `phx-grid` de origem, igual na 0.6.0, 0.7.0 e 0.8.0), então esconder coluna nunca funcionou; (3) seleção + agrupamento contavam cabeçalho de grupo como linha — 100 na página, 93 de dado, e o «marcar todas» nunca fechava; (4) a linha de filtro engordava toda coluna de 68 para **237 px**, e `size` **não vale** para `input[type=number]` (118 px depois). E a versão **mentia em três lugares ao mesmo tempo** (cabeçalho `v0.1.0`, `versao: 0.8.0`, código além da 0.8.0): hoje há `grade_versao_nao_mente`. **Edição na célula é recusa fundamentada**: duplicaria a guarda de escrita concorrente da ficha, ou não a teria. **Fica anotado o que falta**: o `varrer` não tem `WHERE`, então filtro e busca respondem sobre a janela que a tela trouxe e não sobre a tabela (medido: 25 de Blumenau quando a tabela tem 2.500) — o contrato de *pushdown* existe na grade desde a 0.7.0 e o servidor não o atende. Enquanto não atende, a tela **diz**. `docs/GRADE.md` |
| ◐ | 139 | **Prints do console em celular, tablet, desktop e «desktop gamer» — e «é importante poder usar as telas em multi-monitores»** | Fotografado e **medido** nas seis larguras (390, 820, 1180, 1920, 3440 ultrawide, 5120 dois monitores). A responsividade segurava: **zero rolagem lateral em largura nenhuma**. O que não existia era **teto**, e a medição achou quatro coisas. (1) Um **defeito**: no cartão «A máquina» o caminho do diretório passava **por cima** do «livres de 37,0 GB» — dois `<text>` do mesmo `<g>` de SVG, e texto de SVG não quebra nem corta. Aparecia já a 1920, e medindo estava em **todas** as larguras, 390 inclusive: quem decide é o comprimento do caminho, não o monitor. (2) **Texto corrido sem teto**: 5.040px a 5120, umas 630 letras numa linha. (3) **Par rótulo→valor esticado**: 4.553px entre «estado» e «executando» na ficha da telemetria. (4) **Dois regimes de escala**: texto em SVG multiplicado por **5,83×** (11px desenhados com 67px) ao lado de um menu que ficava com 13px. A saída é a **mista**, confirmada pelo dono com a foto de um IDE ocupando um ultrawide em três painéis: a largura extra vira **mais painel, não linha mais comprida** — teto de 74ch no texto corrido, teto na célula de grade, par virando **coluna** (`columns:300px`), e texto em SVG que **não cresce com o monitor** (as barras viraram HTML; o gráfico de horas nasce na largura medida, e aí a largura extra vira mais gráfico). Os tetos são **do contêiner, não da janela** (`ch` e `auto-fill`, nenhum `@media` novo), de propósito: no dia em que a área central virar regiões lado a lado, regra presa à largura da janela estaria medindo a coisa errada. Depois: maior parágrafo **453px**, maior vão **328px**, escala **1,40× constante**, zero sobreposição. A bateria de tela ganhou **3440 e 5120** e as quatro medidas; com o defeito reposto ela reprova nas cinco larguras. **Continua parcial**: a divisão da área de trabalho em regiões com aba por região — o que a foto do dono mostra — é de outra frente. `docs/DESIGN.md` §4.1, §4.2 e §6.1 |

| ☑️ | 140 | **Modo multitela no molde do WINDEV(R): abas dinâmicas, telas lado a lado e janelas soltas redimensionáveis, guardando x/y/largura/altura** | Três modos dentro do **mesmo `index.html`**, que é como o dono fechou a questão («é um site, então estica o navegador por todas as telas e distribui as janelas dentro da mesma page»): **abas** vivas por região, **2–4 regiões** lado a lado com calha arrastável, e **janelas flutuantes dentro da página** (arrasta pelo cabeçalho, redimensiona pelo canto, ordem de sobreposição ao clicar). As quatro telas nomeadas — Diagrama ER, Telemetria, Profiler e Query — abrem juntas e **vivas**. Três decisões sustentam tudo: os ids (`#painel`, `#titulo`) moram **só na tela com foco**, então nenhuma das centenas de `$("#painel")` mudou; aba escondida **sai do documento**, o que mata id repetido e faz todo laço que já perguntava «ainda estou na tela?» parar sozinho; e o `est` é separado em **do servidor** e **da tela**, trocado no foco. **Medido**: telemetria visível 4 pedidos/8 s, escondida **0**; fechada **0**; as quatro telas visíveis em 3240 px custam **15 pedidos/10 s (≈90/min)** — número que fica escrito porque no modo lado a lado ninguém está escondido. `MIN_REGIAO = 660 px` saiu de medição (`testes-web/medir-regiao.mjs`), não de palpite. O pino é o **mesmo glifo e o mesmo significado** do painel lateral e guarda no `localStorage` regiões, larguras, abas pinadas e a geometria das janelas — em **pixel CSS**, com o porquê escrito. Monitor pinado que sumiu cai para o principal **e diz**; janela que não cabe é presa dentro da vista **e diz**. **Recusa fundamentada**: arrastar uma janela do sistema de volta para a barra de abas **não é implementável em navegador nenhum** — não há evento —, então há «⤺ devolver» e «⇤ acoplar» em vez de fingir. A `Window Management API` virou luxo com um uso só: **alinhar as calhas com as emendas físicas** do daisy chain; sem ela, partes iguais. Dois casos novos na bateria (24 execuções), com prova real nos dois sentidos. `docs/MULTITELA.md` |

| ☑️ | 141 | **Permitir mudar as cores do SQL check bolhas pelo `config.json` e pela tela de configuração** | bloco `telemetria` com as quatro cores e os **dois limiares** — e os limiares não são um par novo: são os que o servidor já mandava em `limiares`, que a legenda já escrevia. **Campo vazio = cor de fábrica, e a de fábrica é a variável do tema** (ela escurece sozinha no tema claro); sem cor escolhida a resposta nem ganha o campo `cores`, e é isso que `sem_cor_configurada_nada_muda` trava, no unitário e por soquete. O leitor é o `definir_pintura` do `Servidor::novo` e do `config_gravar` — removido, `a_cor_e_o_limiar_do_config_chegam_pelo_soquete` cai. **O que não é cor não se configura**: traço da borda, glifo e palavra do estado ficam, e a legenda passou a dizer o traço por extenso e a **largar a palavra da cor** quando a cor foi trocada — «amarelo» ao lado de uma bolha roxa é a mesma mentira do «BLUMENAU». A tela mostra a **bolha de verdade** ao lado do seletor, desenhada pela mesma função do painel, e avisa (não proíbe) abaixo de 4,5:1. E aqui uma **premissa medida e corrigida**: «branco sobre amarelo claro dá 2,x:1» não acontece neste painel, porque a tinta é escolhida entre duas — o pior caso possível é **4,35:1**, só no meio-tom (`#797979`); o amarelo claro `#fff2b0` dá 16,74:1. Exercitando apareceu um defeito que ler não acha: a tela gravava a cor, o painel obedecia, e a própria tela anunciava «vale no próximo arranque» — o `config` respondia com a pintura do arranque. `docs/TELEMETRIA.md` §3.3.1, `bancada/telemetria/prova-das-cores.mjs` |
| ☑️ | 145 | **Uma bateria só que rode tudo e diga o que passou, o que falhou e o que foi pulado — e um jeito de PROVAR que cada prova ainda pega o defeito que a motivou** | Duas metades. **(1) `python3 phxsql/provar.py`** orquestra as **vinte** partes que já existiam espalhadas por seis diretórios e três linguagens, sem refazer nenhuma: cronometra cada uma, guarda o log, **recusa rodar com binário velho** (a página é `include_str!`, e a recusa é `exit 2` — não rodar não é reprovar) e imprime **o que foi pulado com o motivo**, porque bateria que esconde o que não rodou mente por omissão. Porta ocupada vira pulo, e não reprovação: há outras frentes na mesma máquina, e uma bateria que acusa a vizinha de defeito é pior que uma que não roda. **(2) `bancada/guardas/`** é o catálogo dos **defeitos repostos**: cada entrada traz o arquivo, o trecho de hoje, o trecho do dia do estrago e **quais testes têm de cair**. O executor copia a árvore, repõe um defeito por vez, roda só o binário nomeado, desfaz e julga. **Medido: **37 guardas** no catalogo depois de integrar as seis frentes.md` sai de um gerador, não da mão. Os achados que só apareceram rodando: **dois conferidores da telemetria saíam com código 0 imprimindo «FALHAS» na tela** — lidos por gente acusavam, chamados por programa mentiam verde —, e a ficha do teste da cifra afirmava que **tirar o AAD do slot o derrubava**, quando medido **não derruba**: o endereço está amarrado duas vezes (AAD **e** nonce), cada uma segura sozinha, e o teste só cai quando as duas somem. Nada foi removido — o AAD é defesa em profundidade — mas as fichas passaram a dizer a verdade medida e três entradas do catálogo travam a conta. Mais dois, que a bateria trouxe para a luz: a prova da replicação **estava reprovando** num estágio (nome de erro renomeado num commit que atualizou o teste unitário e não a bancada, porque ela não estava em portão nenhum), e a tela **mente sobre si mesma** quando o Painel demora — título de Configurações, corpo do Painel, medido com sonda. A primeira foi consertada; a segunda está registrada em §3.2 item 11, porque a tela tem dono. Da rodada final: **16 partes, 13 passaram, 1 reprovou (essa), 1 pulada, 1 sonda, 14m35s**. `docs/TESTES.md` §7 a §9, `docs/SEGURANCA.md` §11.11, `bancada/guardas/LEIA-ME.md` |

| ☑️ | 142 | **GPU CUDA ativar para ajudar em processamento pesado** | **Respondido com medição, e o veredito é «não compensa» — com o limiar escrito para o dia em que mudar.** `docs/GPU.md`, medidor `--example onde-a-gpu-ajudaria`. Três testes independentes derrubam o item: **Amdahl** (o CRC-32 é **0,58%** de uma inserção — instantâneo daria 1,006×; o SHA-256 é **12,1%** do backup — de graça daria 1,14×), **barramento** (o `SUM` anda a **28.234 MiB/s**, **1,79× o pico teórico do PCIe 3.0 x16** — a CPU come os bytes mais rápido do que o barramento os entrega, então **não há tamanho que conserte**) e **forma** (B+tree, SHA-256 e DEFLATE são cadeias seriais). O maior custo achado não era o suspeito: **63,0% do backup é DEFLATE**, a 42 MiB/s, e ele é o **menos** paralelizável de todos. **Não há GPU nesta máquina** (sem `/dev/nvidia*`, sem `nvcc`; o único acerto de `grep -i cuda` é o `libicudata` do ICU), então ativar CUDA aqui é impossível de fato — o que se entrega é a análise e o desenho. **O ganho que o pedido quer existe, e é da CPU:** 4 núcleos com a `std` dão **3,90× no ChaCha20-Poly1305, 3,59× no CRC-32 e 2,51× no SHA-256**, medidos. Custo declarado de adotar CUDA em `GPU.md` §8 — `cargo build --offline` (hoje código 0) e a compilação cruzada param, e o caminho GPU **não é testável nesta bancada**. Limiar que reabre o caso: **armazenamento colunar e plano** com o conjunto quente residente em VRAM (§7) |

| ☑️ | 143 | **O dossiê está desatualizado, falta o `.bkp`, não é responsivo, precisa de download e de capturas do login até replicação, profiler e SQL Check** | Refeito conferindo **seção por seção contra o código**, e a conferência achou seis afirmações erradas — cada uma com o número certo ao lado do errado no `CHANGELOG.md`. O `.bkp` entrou nos **dois** lugares em que faltava: no organograma dos arquivos (a figura mostrava cinco; hoje mostra os **sete que sempre existem** mais os **três condicionais** `.lgpd`/`.bkp`/`.pag`, com o que decide a existência de cada um) e nos cartões da seção 3. **Responsivo e medido** nas seis larguras (390, 820, 1180, 1920, 3440, 5120), nos dois temas: zero rolagem lateral, texto corrido parando em 74ch, **nada centralizado** (o corpo começa a 310px em qualquer largura — num monitor duplo o meio da janela é a emenda física) e a largura extra virando **mais coluna** na galeria (1 → 3 → 6). **Download é `window.print()`** com uma folha `@media print` própria (fundo branco, índice e botão fora, figura/tabela/captura sem quebra no meio, galeria em duas colunas): `<a download>` seria inerte, porque o visualizador do artefato bloqueia todo download que a própria página começa — e a página **diz** o que o botão faz. **Vinte capturas** contra o servidor de verdade (`docs/dossie/capturar-dossie.mjs`, portas 6700/6701, derrubado pelo PID), dez telas nos dois temas: login → painel → tabelas → grade → query → diagrama ER → telemetria → profiler → replicação, mais o **multitela com as quatro telas lado a lado** numa janela de 2.800px. Entraram **doze seções novas**, e a de «Estado e roteiro» **deixou de ser digitada**: eram oitenta linhas à mão com a contagem de testes de cada peça, e viraram dois blocos de gerador. `docs/dossie/dossie-phxsql-0.18.html` |
| ☑️ | 144 | **Os números do dossiê que ainda não saíam de gerador** | Cinco viraram bloco gerado no mesmo commit: o **painel da replicação** (que dizia 28.914/4.357 enquanto a seção da bancada, no mesmo documento, mostrava 34.048/17.450 — sai do `bancada/replicacao/resultados.json`), o **`<title>`** (passou a versão inteira dizendo «0.15» com o selo logo abaixo dizendo outra coisa), o **painel dos idiomas**, o **painel dos pedidos** e a **tabela de cobertura por área**. E a receita do KiB de interface deixou de ser uma **lista copiada**: ela sai do `http.rs`, porque a lista de três arquivos envelheceu calada enquanto o `http.rs` passou a embutir nove — o rodapé publicava **780 KiB** quando a interface tinha **1.032**. O `include_str!` que mora dentro de `#[cfg(test)]` fica de fora, senão 25 KiB de markdown entrariam na conta |

| ☑️ | 146 | **Testar os quatro tipos de replicação em Docker** | `bancada/replicacao/docker/` — cinco `compose` (um por modo mais o do firewall), imagem `scratch` de **6,42 MB**, e um `provar.py` que sobe, mede e **remove tudo**, inclusive quando falha. Um comando só, ~18 min. O que ele achou é o que a bancada de processos não tinha como achar: **(1)** `replicas_autorizadas` estava no `config.json`, no `REPLICACAO.md` §7 e na tela, e **nenhuma linha de código o lia** — um vizinho com o `config.json` de réplica vazado levava **200 de 200 eventos** do diário com a lista preenchida; consertado no portão único, com o teste do comportamento **velho** (lista vazia = como sempre) travando a regressão; **(2)** o **abraço mortal do bidirecional**: cada lado segura a própria trava de dados enquanto espera a resposta do outro, e com fila dos dois lados eles se trancam em ciclos do prazo de leitura de 30 s — medido em `b-abraco`, **sem corte nenhum**; **(3)** o `REDIRECIONA` devolve o endereço da **origem configurada**, que num contêiner é um nome de serviço que o cliente do hospedeiro não resolve; **(4)** `bind: 127.0.0.1` dentro do contêiner não replica nada e **não avisa** — 0 evento, zero erro. Convergência provada com soma do servidor, contagem de linhas, slots e retrato SHA-256 de **cada linha**; contêiner e processo medidos com o **mesmo código** e a diferença ficou no ruído. `docs/REPLICACAO.md` §17 |
| ☑️ | 148 | **`ALTER TABLE ADD COLUMN` preservando o rowid** (sprint 25) | `acrescentar_coluna`, e o que ela resolve é o que falta a toda base no segundo mês. O `.reg` é reescrito **slot a slot, na mesma ordem** — inclusive os livres, que continuam livres e continuam ocupando o lugar deles: como o rowid *é* a posição, preservar a posição preserva o rowid, e por isso o `.ndx` **não é tocado** (há teste que compara o arquivo byte a byte antes e depois). **Medido, não inferido:** 0,553 µs/linha, **dez milhões de linhas em 5,53 s** — o sprint dizia «a casa dos minutos», errado por quase duas ordens de grandeza. As outras duas saídas caíram com número: slot de duas larguras o formato **não permite** (o `slot_size` é um campo só) e cobraria **2,36×** em toda leitura, trocando uma multiplicação por uma descida de árvore; «só em tabela vazia» custa 0,6 ms e não resolve nada. **A linha antiga recebe o padrão declarado ou nulo**, nunca um zero inventado: coluna obrigatória sem padrão numa tabela com linha é recusada, e a mensagem diz por quê. A coluna entra **depois da última coluna do usuário** — antes de `softdeleted` e `rownum` —, e as três coisas que guardam POSIÇÃO (índice, chave estrangeira, coluna de partição) são remapeadas num lugar só; os dois testes que provam isso passavam por acaso até o defeito ser reposto. **A morte no meio tem resposta:** duas fases (escreve todos os `*.novo`, depois troca), com o **volume 1 como ponto de compromisso** — a abertura termina a troca que ficou pela metade, e recusa o conjunto misturado nomeando o volume em vez de ler o volume 3 com a largura do volume 1. 17 testes de formato, 1 de cifra, a prova por soquete com **replicação** (`bancada/alter/provar.py`, 33 passos) e o caso de navegador nos dois temas — que achou, no primeiro minuto, a caixa de marcar esticada a 834px pelo `input{width:100%}` global. `docs/FORMATO.md` §1.1, `docs/DESEMPENHO.md` §4.12 |
| ☑️ | 149 | **Um zelador que mantenha espaço em disco** | `phxsql/zelador.sh`. A regra que decide se ele ajuda ou destrói é uma só: **nada é apagado sem antes se provar que nenhum processo vivo está usando aquilo** — um zelador que apaga o `target` de quem está compilando não economiza espaço, perde uma rodada de trabalho. Cada worktree é conferida por processo com `cwd` dentro dela, e não por data ou nome. Ele **não mata processo nenhum** (matar o `phxsqld` de um agente vizinho já derrubou a própria sessão aqui), não apaga fonte, e não apaga o pacote da versão corrente. A primeira corrida achou o que vinha estrangulando o ambiente a sessão inteira: **80.088 diretórios de teste soltos em `/tmp`, 6,4 GB** — e o disco foi de 6,4 GB para 19 GB livres. Dois critérios guardam o que pode estar em uso, e erram para o lado seguro: PID vivo no nome, ou mexido nos últimos 30 minutos (1.439 preservados). Duas lições ficaram no script: conferir 80 mil diretórios chamando `/proc` um a um estourou o tempo — a lista de processos se levanta **uma** vez; e o total somado das partes disse **362 MiB** numa corrida que liberou quase 10 GB, então ele passou a sair da diferença medida no próprio disco |
| ⏳ | 150 | **A bateria não limpa o que cria** | é a causa-raiz do pedido 149, e enquanto ela ficar o zelador trata sintoma para sempre. São 80.088 diretórios `/tmp/phxsql-*` deixados para trás, o mais antigo de três dias. O padrão certo é o diretório temporário morrer com o teste que o criou (um guarda que apaga no `Drop`, e não um `rm` no fim do corpo — teste que falha no meio nunca chega ao fim). Vale medir antes quantos testes usam o padrão e quantos não: **medir a premissa do item vem antes de implementar o item** |

| ☑️ | 151 | **PhxSql embutido no aparelho: o motor como biblioteca, com ABI de C** («no HFSQL(R) não roda o servidor… julgo que poderia ter um mini servidor para rodar no Android e no iOS off-line e se conectar por TCP/IP com o servidor») | **O objetivo está certo e a forma foi corrigida**, e a correção não é nossa: o iOS **proíbe** processo de longa duração em segundo plano e app escutando porta para outros apps, e o Android mata processo em segundo plano com liberdade — um «mini servidor» ali não é difícil, é contra a forma do sistema. O certo é a mesma máquina com **outra porta de entrada**: biblioteca embutida no processo do app, sem porta e sem daemon. E a boa notícia conferida antes de escrever uma linha: **o `phxsql-store` já é o banco embutido** — o `basico.rs` cria tabela, insere, busca por índice e varre sem soquete nenhum —, então esta rodada não reescreveu motor, **expôs o que existe**. Nasceu `crates/phxsql-ffi`, `cdylib` (o `.so` do Android) **e** `staticlib` (o `.a` que a Apple exige), **44 funções** contadas com `nm -D`, `.so` de **1.155.480 B** — 962.664 B depois do `strip` — com B+tree, CRC-32, ChaCha20-Poly1305, SHA-256 e diário dentro, que é consequência direta do zero dependências. As seis decisões estão justificadas em `docs/EMBUTIDO.md`: **nenhum pânico atravessa** (e o punho fica **envenenado** depois de um, porque capturar salva o processo e não conserta o objeto), erro em **código de retorno** com os **mesmos números** da porta de dados mais último-erro **por thread**, **quem alocou libera** (a biblioteca nunca devolve ponteiro para o `free()` do chamador), **UTF-8 com tamanho explícito** e nunca `NUL`-terminado, e a segurança de thread dita com todas as letras — inclusive o que **não** foi testado. Prova: **26 testes**, um **programa em C** rodado **três vezes** (contra o `.a`, contra o `.so`, e em **ARM64 sob `qemu-aarch64-static`**), 40 passos e zero falhas nas três, e **6 guardas novas** no catálogo (37 → 43), todas PROVADAS. Exercitar achou **quatro defeitos** que ler não acharia — o melhor deles só na perna ARM: sem `--eh-frame-hdr` no ligador, todo `catch_unwind` vira aborto e a garantia principal some **calada**. **O que fica de fora, com o motivo**: a camada **JNI** (o NDK não está nesta máquina) e a **Swift/ObjC** (o SDK da Apple só existe em macOS) — **só o desenho**, escrito em `docs/EMBUTIDO.md` §10; e o cliente de sincronia, que é decisão de produto. `docs/EMBUTIDO.md`, `bancada/embutido/provar.sh`, `docs/EMPACOTAMENTO.md` §7.5 e §7.6 |
| ☑️ | 147 | **A trava de dados presa atrás de uma leitura de rede** (achado 2 do pedido 146) | O laço da réplica tomava a trava global na **primeira linha** de `alcancar_tabela` e a segurava atravessando `replica::puxar`, que é uma ida e volta de rede — e `alcancar_tabela_bidi` fazia o mesmo tomando `self.dados.lock()` **cru**, então o pior caminho do servidor era justamente o que a telemetria não conseguia cronometrar. Hoje as duas estão partidas em **três fases** (abrir e ler a posição com a trava; ler o lote do soquete **sem** ela; reabrir, reler a posição e aplicar com ela), e a regra que sai daí vale para o que vier: *nenhuma leitura de rede acontece com a trava de dados na mão*. Medido antes e depois na bancada nova `bancada/replicacao/trava.py` (quatro estágios, ~1,5 min, portas 7050-7055, com um **tubo** em Python que emudece no lugar do `iptables`): com corte silencioso, pior `varrer` na réplica **30.079 → 6 ms** enquanto o `ping` ficou em 4-5 ms; no bidirecional sem corte nenhum, 200.000 linhas nos dois lados **33,0 s → 1,7 s** (de **14,0×** para **0,71×** do servidor sozinho) e os `EAGAIN` de 30 s sumiram do diário dos dois; e num alcance **de rotina, com a rede sã**, o pior `varrer` do cliente caiu de **2.727 → 76 ms** — esse não precisava de corte nenhum para machucar, só ninguém tinha olhado. **Vazão de aplicação inalterada** (67.406 → 68.000 eventos/s), e aqui uma hipótese que morreu: a queda de 17% que a primeira versão mostrou não era da abertura de tabela por lote, era de um `sincronizar()` deixado na fase 3 — **400 `fsync` em vez de um**. Entraram dois tetos declarados de memória (`TETO_DO_LOTE_SERVIDO` = 16 MiB de imagem por resposta no source, com o primeiro evento entrando sempre; `TETO_DA_RESPOSTA` = 128 MiB por linha lida na réplica, com recusa `LIMITE_EXCEDIDO` que traz o número), e a **queda de conexão entre a leitura e a aplicação** está provada por soquete: dez cortes de verdade no meio de um alcance de 200.000 eventos, soma, linhas e **slots** iguais dos dois lados. Guarda `trava-atras-da-rede` **PROVADA**, com prazo próprio de 8 s por sonda porque o defeito reposto **pendura** em vez de falhar. **O que ficou por fazer:** a bancada de contêiner (`bancada/replicacao/docker/provar.py`) **não foi refeita** — o daemon do Docker desta máquina estava fora do ar e não pôde ser levantado —, então os `resultados.json` de lá continuam sendo o retrato do defeito. Os dois números que ela mediu estão reproduzidos no loopback com o mesmo tamanho (30.079 contra 29.456 ms; 33,0 contra 33,3 s), que é a razão de confiar no resultado; refazer lá é o que fecha a conta. `docs/REPLICACAO.md` §18, `docs/DESEMPENHO.md` §4.12 |
| ☑️ | 152 | **«Como o PhxSql mobile pode ser melhor que o SQLite(R) e o HFSQL(R) no celular?»** | Respondida **medindo**, e a resposta tem duas metades que o `docs/MOBILE.md` diz com o mesmo cuidado. **Em velocidade de motor, não é**: com os dois em processo, a mesma sincronização no fim e 200.000 linhas, o SQLite(R) insere **3,6×** mais rápido, varre a faixa **3,8×** mais rápido (**2,7×** quando obrigado a materializar a linha inteira, que é o que o nosso lado faz) e exclui **4,2×** mais rápido, e ocupa **4,3×** menos disco. O PhxSql ganha nas duas operações que um aplicativo faz o dia inteiro — **ler por chave** (1,9×) e **atualizar** (1,5×) —, e ganha pelo mesmo motivo que perde no disco: `offset = base + (rowid−1) × slot_size` acha a linha por multiplicação **porque** o slot é de largura fixa. As duas metades são a mesma decisão vista de dois lados. **Onde ele pode ser melhor não é velocidade:** o problema de um aplicativo de celular é sincronizar, e a replicação com imagem da linha, o `.log` por tabela (que **já é** o diário do que aconteceu offline, na forma que a réplica aplica — e que é **17% do disco medido**, a única linha do documento em que um custo e um recurso são a mesma coisa), a janela de conflito por versão, a cifra em repouso e a trilha `.lgpd` estão no motor e medidos; no SQLite(R) cada um é código à mão ou produto pago. **E onde o SQLite(R) ganha está escrito com a mesma clareza**: já está no aparelho (zero byte contra 6,8 MB), 269.649 linhas de C com vinte anos de canto contra 1.328 testes, SQL muito mais completo, **é ACID de verdade e o PhxSql não é**, e é biblioteca sem porta e sem daemon — que no iOS não é preferência, é a regra. A **forma certa** no aparelho é biblioteca embutida mais cliente de sincronia, **não** mini-servidor escutando porta. A bancada é `bancada/sqlite/`, com cinco partes porque este par exige separar o motor do transporte — e o transporte foi decomposto em três, porque atribuir tudo «ao soquete» seria diagnóstico plausível em vez de medido. As tabelas do documento **saem de gerador** (`medir.py --documento`), que reprova se um bloco marcado não existir ou um bloco existente não estiver marcado. Sete armadilhas ficaram escritas em `bancada/sqlite/LEIA-ME.md`, e duas delas mentiram **a nosso favor** antes de serem achadas: a janela trocada no modo `sistema` (20.000 transações do outro lado contra 100 janelas do nosso, sem `fsync` para denunciar) e o piso do transporte medido com um eco que dividia o GIL com o cliente — ele marcou 73,78 µs contra 72,75 µs do `phxsqld` e a subtração deu **−1,03 µs**, um servidor que custa menos que nada. Sobre o HFSQL(R), o que se sustenta do material que existe aqui é que a folha lista replicação **móvel e offline** entre os quatro tipos; o resto está na §7 como **não apurado**, porque não há folha do HFSQL Classic do WINDEV(R) Mobile neste repositório e a frase «tabelas soltas sem cuidado» é impressão do dono, não fato medido |
| ☑️ | 153 | **Criar VM para provar o binário Windows e o Android** | **VM completa recusada com o motivo medido, e o objetivo alcançado por outro caminho.** Não há `/dev/kvm` nesta máquina e o processador não expõe flag de virtualização — ela própria é uma VM sem aninhamento —, então subir Windows não era questão de disco nem de tempo. O que destravou foi a mesma percepção que o `qemu-user-static` trouxe para o ARM, por outra porta: **o `wine` não emula nada.** O `.exe` é x86-64 e a máquina é x86-64, o código roda nativo, e o que ele reimplementa são as DLLs. Com 150 MB de download, a §6 do `docs/EMPACOTAMENTO.md` deixou de dizer «o que **não** dá: rodar»: `bancada/windows/provar.sh` sobe o `phxsqld.exe`, faz login com o hash que o **próprio `.exe`** gerou (o PBKDF2 roda sob `wine`), cria banco e tabela, grava 50 linhas e lê as 50 de volta — e o binário provado pode ser o **do pacote**, que é melhor, porque é o arquivo que o usuário baixa. A sonda é a **mesma** da bancada ARM, que passou a receber o rótulo de fora: uma sonda que se anuncia «ARM64» numa corrida de Windows mente no lugar que mais importa, que é o VEREDITO. Três decisões ficaram no script: o prefixo do `wine` mora no descartável da corrida (um `~/.wine` de outro dia é como a prova passa por engano); quem subiu se confere **pela porta**, porque o `wine` troca o processo e o `kill -0` no PID do lançador diz «não subiu» com o servidor no ar; e o **RSS sai do dono do soquete**, achado pelo inode em `/proc/net/tcp` — a primeira versão pegava o primeiro `/proc` que casasse com `phxsqld.exe` e o número pulou de **6.148 para 17.236 kB** entre duas corridas iguais, porque às vezes achava o lançador. Corrigido, ficou em **8.796 e 8.800 kB**. *Número que muda 3× sem nada mudar não está medindo o que diz.* **Android continua sem prova**: o alvo compila e para no ligador por falta do NDK, e a forma que faltava (`staticlib` + FFI em C + JNI) virou a §8 do `docs/MOBILE.md` em vez de continuar sendo uma linha de tabela. O que o `wine` **não** prova está na §6.2: desempenho no Windows (as 50 linhas variaram 4,5 → 60,3 ms/linha com a máquina carregada — é ruído, não custo do `wine` nem do motor), compatibilidade completa, e o driver ODBC, que exige o gerenciador do Windows carregando a DLL |

<!-- pedidos:contagem:inicio -->
**152 pedidos: 149 feitos · 3 parciais · 0 planejados.**

*(Gerado por `docs/dossie/pagina-dos-pedidos.py` — não conte à mão. A
conta sai da primeira coluna da tabela acima, e é a mesma que a página
dos pedidos mostra: se as duas discordarem, é porque alguém digitou uma
delas.)*
<!-- pedidos:contagem:fim -->

Nenhum pedido continua na coluna «planejado». Isso **não** quer dizer que não
falta nada — quer dizer que o que falta deixou de ser um pedido seu parado e
virou ou uma **parcial nomeada** (§2), ou um item da lista de propostas
(`docs/SPRINTS.md`), ou uma das pendências sem número da §3.

Fora do que você pediu, entraram por medição: o CRC slice-by-8, o `descer` sem
reler a folha, a conferência de unicidade sem descida dupla, o cache de
páginas *write-back* do `.ndx`, e dezoito correções de defeito — três delas de
perda silenciosa de dado, e quatro achadas **rodando** o que tinha acabado de
ser escrito (o percentual de disco que dividia pelo total, o assunto de e-mail
com acento cru no cabeçalho, o decimal que a grade arredondava, e o
`criar_tabela` que gravava `filial.clientes.reg` na raiz do banco e devolvia
uma tabela que nenhuma outra operação conseguia abrir).

E entraram **frentes inteiras sem número de pedido**, porque nasceram de
conversa e não de lista: a telemetria ao vivo no molde do SQL Check da
Idera(R) (`server/src/telemetria.rs`, `ui/telemetria.js`), a integração com a
Claude no console (`ui/claude.js`, `docs/CLAUDE-IA.md` — a chave fica no
navegador, o esquema viaja, o dado não), as mensagens multilíngues
(`server/src/mensagens.rs`, `idiomas.rs`), o console em três tamanhos com o
sistema visual escrito (`docs/DESIGN.md`, `docs/CONSOLE.md`), o assistente de
replicação (`docs/ASSISTENTE-REPLICACAO.md`) e o firewall/blacklist revisto.
Elas não entram na contagem acima porque a contagem é dos **seus pedidos**,
numerados na ordem em que você fez — mas ficam registradas aqui para não
parecerem que apareceram sozinhas.

---

## O detalhe de cada parcial, e o que continua faltando

## 2. Parcial

Existe, funciona no que promete, mas **não faz tudo** o que o pedido queria.
Cada linha diz exatamente onde para.

As primeiras são os ◐ da tabela lá em cima. A que trata de chave
estrangeira **não é um pedido seu** — é um buraco achado numa revisão, dentro
de um pedido marcado feito, e fica aqui para não sumir de vista.

O 127 saiu daqui nesta rodada: o que faltava a ele era alterar a estrutura de
uma tabela com dado, e o sprint 25 entregou `acrescentar_coluna`.

| # | O que você pediu | O que existe | O que falta |
|---|---|---|---|
| 18 | **Subir o PhxSql no GitHub** | a branch `claude/capacidades-disponiveis-y6auxh` em `adrianoboller/adrianoboller`, com o histórico completo. A credencial **lê** o repositório e enxerga a branch — conferido nesta revisão, não suposto | um repositório **próprio**, e o impedimento é de **identidade, não de permissão**: esta sessão autentica como `EnginePrint` (id 322529492, criada em 2026-08-29, zero repositórios públicos), que não é você. `create_repository` responde 403 porque ninguém cria repositório em nome de outra pessoa — e, se criasse, o repositório seria **dela**, não seu. Destrava com você criando `adrianoboller/phxsql` e dando acesso a essa app. **Não há trabalho de engenharia esperando aqui** |
| 86 | **DbLink para PostgreSQL(R) e outros** | **cliente, dialeto e ligação prontos**: `server/src/pg/` (721 linhas de protocolo mais 278 de SCRAM-SHA-256 conferido contra o vetor do RFC 7677), SQL por motor em `dblink/dialeto.rs`, e as cinco operações do DbLink reescritas para não saberem qual motor atendem. Provado por soquete contra um servidor de protocolo próprio, byte a byte, nos dois sentidos | só o que o nome do pedido diz: **a prova contra um PostgreSQL(R) de verdade**, que não existe nesta máquina. O roteiro do que ela exige está em `docs/DBLINK.md`, e o precedente é o pedido 131, que fez exatamente isso contra um MySQL(R) 8.0.46 real e achou o que o servidor falso não acharia |
| — | **Chave estrangeira** com CASCADE / RESTRICT / SET NULL | declarada, validada, gravada no cabeçalho do `.reg`, preservada pelo `duplicar_tabela`, e mostrada na aba Estrutura e no diagrama | **não é aplicada**. Nenhuma gravação consulta a chave: `Restringir` e `Cascata` são intenção guardada, não comportamento. Há um teste que trava que *declarar não é aplicar*, e ele falha no dia em que isso mudar em silêncio. O Teradata(R) chama isso de *soft RI* e mostra por que é desejável e não preguiça: ela existe **para o otimizador** — e isso só vira valor quando houver planejador |

## 3. O que continua faltando

Aqui não há mais pedido seu parado — o que sobrou não tem número, ou é
consequência do que entrou. A lista é curta de propósito: cada linha diz onde
o buraco está no código, e não «o que seria bom ter».

### 3.1 Os oito que saíram desta lista nesta revisão

Estavam escritos como «não começado, sem código nenhum», e é falso desde
algumas rodadas atrás. Ficam registrados para a próxima leitura não procurar:

| era planejado | onde está hoje |
|---|---|
| Jobs de execução | pedido 51 — `server/src/jobs.rs`, cadastro, corridas, relógio de 30 s, e o job rodando com o poder do usuário dele |
| Triggers | pedido 49 — `phxsql-sql/src/rotina.rs` + `server/src/rotinas.rs` |
| Stored procedures | pedido 50 — o mesmo interpretador, `CALL` com `IN`/`OUT`/`INOUT` |
| Parar e subir o serviço pela tela | pedido 40 — despertador no próprio endereço, porta nova presa antes de a velha ser solta |
| Servidor MCP | pedido 6 — `phxsqld --mcp`, `tools/list` lendo o catálogo |
| Camada SQL | pedido 83 — crate `phxsql-sql`, **escrita aqui**. O plano antigo dizia «tabela virtual do rusqlite atrás de um `feature`», e teria furado a regra de zero dependências; não foi por ali |
| Driver ODBC de saída | pedido 7 — `crates/phxsql-odbc/`, provado pela ABI e pelo `isql` |
| Cliente ODBC e OLE DB | pedido 7 — ODBC entregue; **OLE DB é recusa fundamentada**, com a ponte `MSDASQL` documentada (`docs/ODBC.md` §6) |

### 3.2 O que falta de verdade

| | O que falta | Onde está o buraco, no código |
|---|---|---|
| 1 | ~~**Restaurar backup**~~ | **FECHADO nesta rodada** — virou o pedido 134. `restaurar_backup` com dois modos (com outro nome, que e o padrao, e por cima com a porta de dados parada), SHA-256 conferido antes de o destino ser tocado, e portao proprio para o banco que vem DENTRO do backup. Fica aqui riscado, e nao apagado, porque **esta lista ja se contradisse antes**: a §7 afirmava que a replicacao nao transportava evento enquanto o item 19 trazia a medicao do contrario. Item que fecha se risca na primeira leitura seguinte |
| 2 | ~~**As 13 tomadas da trava de dados fora do ponto único**~~ | **FECHADO nesta rodada.** As 13 entraram no `travar_dados()`, uma a uma e não no automático — cada conversão respondeu «quem chama isto já tem a trava?», que é a pergunta que a terceira reincidência do abraço mortal ensinou a fazer; a tabela com a resposta de cada uma está em `docs/TRANSACOES.md` §8.1. As três que doíam agora aparecem no `espera_ms_s`: o despejo do cache, o corpo do gatilho e o laço da replicação atravessando a rede — e o `docs/TELEMETRIA.md` §2.1 voltou a ser verdade. **A conta não se refaz mais à mão:** o teste `so_um_lugar_toma_a_trava` conta as tomadas no próprio fonte (pelo mesmo `include_str!` do conferidor de textos, e pela mesma razão) e reprova a décima-quarta. Junto entrou a guarda de reentrância: pedir a trava que a própria thread já tem virou **erro nomeado** em vez de pendurar o servidor inteiro, e ela custa **−0,05 ns** por tomada — abaixo da resolução do medidor (`docs/DESEMPENHO.md` §9) |
| 3 | **Transações** | continua **não existindo**, e a tela (Ferramentas → Gestão de transações) continua dizendo isso em vez de fingir. O que entrou nesta rodada foi o **desenho escrito antes do código**: `docs/TRANSACOES.md`, com escopo (uma conexão, um database, a web fora e por quê), o rollback de um `inserir` **sem queimar slot** — nada vai a disco antes do `COMMIT` —, o nível de isolamento dito sem enfeite (não é ANSI SERIALIZABLE), a marca `.tx` que hoje não existe e sem a qual a recuperação não sabe para onde ir, e a prova de replicação. A decisão que mais custou: o «slot que nasceu e morreu» foi **recusado com quatro motivos**, e o decisivo é da replicação — queimar o slot dos dois lados exigiria mandar a inclusão e a exclusão para a réplica, que é literalmente a transação revertida chegando aplicada lá. O pré-requisito (item 2 acima) era o terreno e está fechado |
| 2 | **As 12 tomadas da trava de dados fora do ponto único** | `travar_dados()` é onde a telemetria cronometra a espera na fila — e o comentário dele diz ser «o **único** lugar que a toma». Medido: **não é.** Há 13 `self.dados.lock()` no arquivo, e 12 estão fora dele: `mensagens_atualizar`, `semear_mensagens`, `posicao_do_diario`, `atender_http`, `op_mensagens`, `op_idiomas`, `op_idiomas_carga`, `op_idiomas_padrao`, `op_idiomas_exportar`, `op_idiomas_importar`, `descarregar_sujas` e `executar_rotina`. **A pior das três já saiu**: o laço da replicação segurava a trava atravessando uma ida e volta de rede, e `alcancar_tabela_bidi` a tomava crua — o pior caminho era justamente o que a telemetria não via. Consertado e medido: pior `varrer` na réplica cortada em silêncio de **30.079 → 6 ms**, o abraço do bidirecional de **14,0× → 0,71×**, e o pior `varrer` durante um alcance de rotina de **2.727 → 76 ms** (`docs/REPLICACAO.md` §18, `docs/DESEMPENHO.md` §4.12, guarda `trava-atras-da-rede`). **Sobram duas que doem**: o **despejo do cache** segura a trava por uma passada inteira, e o **corpo de um gatilho ou procedimento** a segura pelo tempo que quiser — que é exatamente a atividade longa que o painel existe para mostrar. Enquanto elas não passarem por `travar_dados()`, o `espera_ms_s` da telemetria é o de uma parte, e o `docs/TELEMETRIA.md` §2.1 afirma o contrário. **Refazer a conta: `grep -c 'self\.dados\.lock()' servidor.rs`** |
| 3 | **Transações** | tem tela (Ferramentas → Gestão de transações), e ela diz o que existe e o que não existe em vez de fingir. Hoje a inserção desfaz o que gravou se um índice falhar, e a trava única serializa as escritas — mas não há journal com a **imagem anterior** da linha, nem identificador de transação na sessão, nem `commit`/`rollback` de várias operações. O primeiro tijolo é o item **21** de `docs/SPRINTS.md`, e ele é deliberadamente a metade que **não** depende da transação |
| 4 | **Concorrência fina** | uma trava única serializa todo acesso a dados. É o que o `docs/SPRINTS-TERADATA.md` §4.5 aponta ao recusar *workload management*: prioridade sobre uma fila de um só não é prioridade |
| 5 | **Modo exclusivo** | tela apagada em *Gerir banco* (`ui/index.html:4799`). Meio caminho existe e não estava escrito aqui: o `BULKINSERT` **já reserva uma tabela por conexão**, com erro 4002 `EM_CARGA` nomeando quem reservou e `repetir: true` (pedido 128). O que falta é reservar **por período** e para outra coisa que não carga — e isso depende da trava por tabela, que é o item 4 |
| 6 | **Compactação** | o formato prevê e **mede** o espaço morto; falta o comando. E há dois números contra: compactar renumeraria rowid, e rowid é endereço (`docs/COMPARACAO.md`); e a compactação do **diário** foi medida e recusada **duas vezes** — 14,7% no melhor corte contra 2,1× que o mesmo esforço compraria no `.ndx` (`docs/DESEMPENHO.md` §4.7.3). O que sobrou de vivo nesse assunto é o item **19** de `docs/SPRINTS.md`, que é outra conta |
| 7 | **Editar `config.json` e usuários pela web** | **metade entrou** e não estava escrito aqui: a op `config_gravar` existe (`servidor.rs:2752`), com portão próprio de `administrar` e rastro no log de quem mudou o quê. O que ela **não** grava é deliberado e está na lista `CAMPOS_EDITAVEIS`: token, segurança, cadastro de usuários, cifra, credencial de e-mail e replicação — uma sessão roubada não abre o firewall, não cria supervisor e não vira este servidor para outro source. **Falta o cadastro de usuários pela web**, que é a metade difícil: senha nunca em claro em ponto nenhum do caminho |
| 8 | **TLS** | **mudou de forma nesta rodada.** A porta 5000 e o transporte da replicação ganharam a **cifra do fio** — aperto estilo Noise (X25519 + HKDF + ChaCha20-Poly1305), `docs/CIFRA-DO-FIO.md`. Não é TLS e não pretende ser: o navegador não fala isso, e a interface web continua dependendo de proxy TLS ou túnel. Continua faltando: o driver ODBC não fala o aperto, o pulso do cluster vai em claro, e a amarração da credencial ao canal ainda não é feita |
| 9 | **Integração no FraseSQL** como `engine = "phxsql"` | dependia do ODBC, que entrou. O catálogo do `.reg` já é «o mesmo formato que o catálogo do FraseSQL espera» (`store/src/catalogo.rs:21`), e o contrato de integração está lido em `docs/PLANO.md` §2. Nunca foi tentado |
| 10 | **A interface em seis idiomas — 28% dela** | `cargo run --example textos-fora-da-fabrica -p phxsql-server` conta **1.806 ainda cravados em português**, e agora eles estão TODOS num arquivo só: o `index.html`. Os outros quatro fecharam em zero nesta rodada — `claude.js` (126), `telemetria.js` (38), `grid/phx-grid.js` (24) e `diagrama-er.js` (2) —, e o `multitela.js` já estava. A catraca desceu de 1.996 para **1.806** no mesmo commit, e a fábrica passou de 303 para **645** chaves. **O `index.html` ficou de fora de propósito**: quatro frentes o estavam editando ao mesmo tempo, e mexer nos 1.806 no meio disso trocaria tradução por conflito de integração — é a próxima leva, depois que elas caírem. Três lições novas, e as três saíram de **exercitar**, não de ler: (a) lista lida no ARRANQUE precisa do par `rot:`/`txt:`, e o `diz:` da lista de modelos ganhou o par `dizTxt:` no conferidor pelo mesmo motivo — `txt(…)` ali resolveria em português para sempre; (b) **quem escreve por último manda**: o botão da legenda da telemetria já pedia o texto pela fábrica, e o `aplicarLegenda` o reescrevia em português logo depois — na captura em francês ele aparecia como «ocultar legenda» no meio de uma tela inteira em francês; (c) texto escrito com `\uXXXX` some das DUAS vias do conferidor: os seis operadores do filtro de número da grade («é maior que»…), a dica de arrastar coluna e dois `placeholder` estavam assim, e só apareceram no navegador. Falta ainda o miolo do `index.html` — por ordem do que se vê: os títulos e subtítulos de `folha(`, os cabeçalhos de coluna, os recados de `avisar(` e as perguntas de `confirm(` —, e as formas que o conferidor **não vê e declara**: o primeiro argumento de `linha(` (as vinte e cinco etiquetas do cartão da telemetria já foram traduzidas à mão, mas a forma continua fora do `RECEITAS`) e o rótulo em par solto. Duas guardas novas entraram em zero para pegar o outro estrago, o do texto **colado**: seis idiomas idênticos, e a mesma frase longa em três ou mais. `docs/MENSAGENS.md`
| 11 | **A tela é sobrescrita pela tela anterior quando o Painel demora** | `ui/index.html`, `abrirAdmin`: `p.innerHTML = await vPainel()` escreve **depois** do `await` sem perguntar se aquela ainda é a tela aberta. Quem entra e clica em Configurações antes de o Painel terminar de carregar fica com o **título de Configurações e o corpo do Painel** — a tela mentindo sobre si mesma. Medido com sonda: `#painel` vai de 31.092 para 13.818 caracteres 2,5 s depois, e o rastro aponta o `abrirAdmin`. A janela **cresce com a carga**, porque o `vPainel()` consulta os monitores da máquina — e é por isso que `bancada/telemetria/prova-das-cores.mjs` passava quando foi escrita e reprova hoje. O padrão certo existe três linhas acima de outro `innerHTML` no mesmo arquivo (`if (!vivo || !document.body.contains(alvo)) return;`). **Não consertado de propósito**: a tela tem dono, e havia frentes mexendo nela na rodada em que isto foi achado. `docs/TESTES.md` §9.8 |
| 12 | **A aba de segundo plano guarda o idioma em que foi pintada** | achado exercitando a troca de idioma com duas regiões abertas, e **deixado de fora de propósito**. Ao trocar o idioma, `aplicarIdioma` repinta o cromo, chama `PhxTelas.repintar()` (que repõe o rótulo de toda aba cujo nome vem da fábrica, **por chave**, nunca comparando a frase) e chama `est.repintar()` — que é o gancho da tela **com foco**. A aba do lado, cujo conteúdo foi pintado por um `folha(` e cujo rótulo veio do título que aquela tela escreveu, continua no idioma anterior até ser repintada. Consertar isso é repintar tela **escondida**, e isso contraria a decisão [2] do `ui/multitela.js` — *aba escondida sai do documento e para de trabalhar* —, que é o que faz a telemetria de uma aba escondida custar zero pedido. Vale medir antes: o caso só aparece com duas ou mais regiões abertas E uma troca de idioma no meio da sessão. `ui/multitela.js`, `reporRotulos` |

### 3.3 E a lista de propostas, que não é pendência

Quatro agentes leram os manuais do Cassandra(R), do Redis(R), do MariaDB(R) e
do Teradata(R) e escreveram **31 propostas de sprint**. Elas não são
pendências — são candidatas esperando o seu sim, cada uma com a premissa que
pode matá-la antes da primeira linha de código.

A lista única, com as duplicatas fundidas, as contradições apontadas e o que
já existe fora dela, está em **`docs/SPRINTS.md`**: 27 itens aprováveis, 2
fechados como «medição, não sprint», e 6 contradições que precisam de decisão
antes de dois itens entrarem juntos. Os quatro documentos de origem continuam
existindo (`docs/SPRINTS-CASSANDRA.md`, `-REDIS.md`, `-MARIADB.md`,
`-TERADATA.md`).

---

## 4. O que cada revisão achou de errado — e já consertou

Revisar serve para achar, e quase nunca o que aparece é recurso faltando: é o
projeto se descrevendo errado. As seções abaixo estão da mais nova para a mais
antiga.

### O que a rodada dos pacotes de download achou

O pedido 17 estava marcado como feito e o empacotador existia. Rodar o
empacotador achou três defeitos, e nenhum deles aparecia lendo o código.

- **`./empacotar.sh` não rodava num checkout limpo.** O `monta()` compila com
  `--target`, que grava em `target/<alvo>/release`; o config de demonstração
  pede o hash da senha ao `./target/release/phxsqld`, que é o binário do
  **hospedeiro** e que `--target` nunca produz. Quem tivesse rodado
  `cargo build --release` antes não via nada de errado — e é exatamente o caso
  de quem escreveu o script. Reposto o defeito (o binário do hospedeiro fora
  do lugar), o empacotador morre em `No such file or directory` **antes de
  montar zip nenhum**. Agora há um `garante_host()`, e é o mesmo naipe do
  binário velho da bancada: *o que se usa todo dia esconde o que só falha do
  zero*.

- **O `config.json` de demonstração escrevia um campo que o servidor não
  lê.** `web.sessao_min` — o campo é `sessao_minutos`. O servidor avisa e
  **ignora**, e como o padrão também é 60 minutos a tela ficava idêntica: o
  aviso rolava para fora do terminal e ninguém conferia. Só apareceu subindo o
  binário empacotado de verdade. É a lição do `recursos.cache_paginas` numa
  casa nova: **configuração que não é lida mente**, e mente mais quando o
  valor que ela promete por acaso coincide com o padrão.

- **Não havia conferidor nenhum.** Os três zips saíam sem manifesto. O backup
  de dados tem `backup.json` com SHA-256 por arquivo desde o pedido 43; o
  pacote de distribuição, que é o que sai da máquina, não tinha nada. Entrou o
  `MANIFESTO.sha256` e o `phxsql conferir-pacote` — e o caso que ele pega e
  quase nenhum conferidor pega é o **arquivo a mais**: conferência de hash só
  olha o que o manifesto lista, então acrescentar um binário ao pacote não
  mexe em nenhuma linha e passaria batido.

E uma trava que nasceu desta rodada porque o `MANUAL.txt` **não dizia versão
nenhuma**: não havia como o pacote se contradizer, mas também não havia como
ele se conferir. O cabeçalho ganhou o selo, e `confere_versoes()` reprova o
empacotamento se `Cargo.toml`, `Cargo.lock`, o `MANUAL` e o `CHANGELOG` não
disserem a mesma coisa. **Número visível ou sai de gerador, ou tem quem o
confira** — o selo da capa do dossiê passou quatro lançamentos porque não
tinha nem um nem outro.

### O que a revisão das quatro listas de sprint achou

Esta rodada não tocou em código de produção: leu as quatro propostas de sprint
e reconferiu esta lista **contra o código**. Achou sete coisas, e cinco delas
são a mesma família — **número certo com palavra errada em cima**.

- **O gerador da §5 calculava a razão certa e imprimia a palavra errada.** O
  bloco dizia *«a inserção é o ponto fraco do motor […] 0,8× mais devagar»* com
  os próprios números ao lado mostrando 109.300 linhas/s contra 88.994 do
  MySQL(R) — o insert **ganhando**. As palavras «ponto fraco», «mais devagar»,
  «nas outras quatro o motor se defende» e «a atualização empata» estavam
  **fixas** no `resumo_md` do `numeros-da-bancada.py`; só os números vinham da
  medição. Quando o sinal virou (a rodada do cache de páginas e do
  *write-back*), o texto não virou junto. Agora o veredito de cada fase sai do
  próprio fator, e não há frase fixa que possa discordar dele. **Número gerado
  com veredito digitado ainda envelhece calado** — é o selo de capa uma casa
  adiante.

- **A contagem dos pedidos estava digitada no arquivo que a produz.** «123
  feitos · 5 parciais · 4 planejados» com a tabela logo acima dizendo outra
  coisa havia rodadas. Agora o `pagina-dos-pedidos.py` soma e **escreve de
  volta** entre marcas, como o `numeros-da-bancada.py` já fazia.

- **E o próprio script que existe para impedir número digitado tinha um.** O
  `<title>` da página era `Os 129 pedidos do PhxSql`, com o 129 fixo no código
  — errado por dois motivos ao mesmo tempo: são 132 pedidos, e 129 é a
  contagem dos *feitos*. Também estava fixa a frase «três deles estão parados
  por coisa de fora — um 403 do GitHub e um pacote sem fonte», que descrevia um
  estado que mudou. As duas passaram a sair da contagem.

- **Quatro pedidos estavam marcados «não começou» e estavam prontos:** 7
  (driver ODBC), 49 (triggers), 50 (stored procedures) e 126 (cluster). O 65
  dizia «20 ferramentas, 16 funcionam» quando são **30 e 27**, e o 130 dizia
  «79 operações» quando o catálogo tem **108**. Estado medido contra a
  lembrança do documento é estado errado — a regra estava escrita no topo desta
  página e não estava sendo cumprida.

- **A trava de dados tem 13 tomadas fora do ponto único, e dois documentos
  afirmam o contrário.** O comentário de `travar_dados()`
  (`server/src/servidor.rs:719`) diz ser «o **único** lugar que a toma», e o
  `docs/TELEMETRIA.md` §2.1 diz que «as 50 tomadas de trava do `servidor.rs`
  passaram a chamar `travar_dados()`». Contado: são 14 `self.dados.lock()` no
  arquivo, e **13 estão fora**. A consequência não é estética — o
  `espera_ms_s` que a telemetria mostra é o de uma parte, e as duas piores
  ausências são o despejo do cache e o corpo de um gatilho, que é exatamente a
  atividade longa que o painel existe para mostrar. Está na §3.2, item 2, com
  a lista de linhas e o comando de recontar. **Não foi consertado aqui**: esta
  rodada é de documento, e mexer em 13 caminhos de trava não é edição de
  documentação. *(Fechado na rodada das transações: as 13 entraram, e o comando
  de recontar virou o teste `so_um_lugar_toma_a_trava` — porque o comando
  ninguém roda e o teste roda sozinho.)*
- **A trava de dados tinha 13 tomadas fora do ponto único, e dois documentos
  afirmavam o contrário.** O comentário de `travar_dados()` diz ser «o
  **único** lugar que a toma», e o `docs/TELEMETRIA.md` §2.1 diz que «as 50
  tomadas de trava do `servidor.rs` passaram a chamar `travar_dados()`».
  Contado: eram 14 `self.dados.lock()` no arquivo, e **13 estavam fora**.
  **Uma delas saiu**, e era a que mais doía: `alcancar_tabela_bidi` tomava a
  trava crua e a segurava atravessando uma leitura de rede, então o pior
  caminho do servidor era exatamente o que a telemetria não conseguia
  cronometrar. Hoje são 13 tomadas, 12 fora. As outras 12 continuam na §3.2,
  item 2, com o comando de recontar — e a lição fica: **ponto de medição que
  pula um chamador mente do mesmo jeito que campo de configuração que ninguém
  lê**, e o chamador que ele pula tende a ser justamente o interessante.

- **A premissa de um dos sprints já estava morta, e dava para saber lendo.** O
  `SPRINTS-REDIS.md` propõe matar o TTL por linha se «um job horário com
  `UPDATE … SET SOFTDELETED = 1 WHERE prazo < NOW()` der conta hoje». Não dá: a
  camada SQL recusa `UPDATE` pelo nome (`phxsql-sql/src/sintaxe.rs:269`) e o
  corpo de rotina também (`docs/TRIGGERS.md` §8, com o motivo medido). A
  alternativa barata **não existe**, então ela não pode matar o sprint.

- **E metade de outro sprint já funciona.** O `SPRINTS-MARIADB.md` pede «um job
  cujo corpo seja `CALL procedimento(...)`». O corpo de um job é um pedido do
  protocolo despachado pelo despachar de sempre (`servidor.rs:3410`), e `CALL`
  entra pela op `sql` (`:6671` → `executar_rotina`). Um job com
  `{"op":"sql","texto":"CALL fecha_mes()"}` já é um job que chama procedimento
  — falta o teste que prova isso pelo soquete, porque ler o código não é
  provar.

As sete estão em `docs/SPRINTS.md`, que é o que esta rodada produziu de novo.

### O que a revisão dos onze achou

Onze coisas apareceram, e quase nenhuma era recurso faltando: era o projeto se
descrevendo errado.

### Antes delas: três que a bateria de ponta a ponta achou

`bancada/bateria/` faz os seis itens do pedido **como um usuário faria** — cria
o banco, cria as tabelas, gera as chaves v7, pendura os gatilhos, chama os
procedimentos e carrega 5.000 linhas —, pelo soquete **e pela tela**. Todos os
testes desta casa passavam. Ela achou três defeitos, e nenhum aparecia por
leitura:

- **O servidor travava de vez, e não era dos gatilhos.** Ao fechar a janela de
  durabilidade, `gravar_de_verdade` pedia a **trava de dados que já estava na
  mão de quem chamou** — `Mutex` não é reentrante, e a thread parava para
  sempre segurando o servidor inteiro. A reprodução mínima não tem gatilho
  nenhum: **duas tabelas, inserções alternadas, e ele trava na de número
  200** — exatamente quando a janela fecha. Com uma tabela só o conjunto de
  sujas ficava vazio e a função voltava antes de pedir a trava; é por isso que
  as bancadas de uma tabela só nunca esbarraram nele. Detalhe em
  `docs/TRIGGERS.md` §9.2.

- **Um gatilho derrubava o processo inteiro.** `AFTER INSERT ON t` gravando em
  `t` chamava a si mesmo sem fundo, e estouro de pilha no Rust não vira erro:
  **aborta o processo**. Como o corpo mora no `gatilhos.json`, ele voltava a
  derrubar na próxima tentativa. A cadeia ganhou teto de oito níveis e um aviso
  que sobe até a resposta que alguém lê (§9.1).

- **`excluir_tabela` não apagava a tabela inteira.** A lista de extensões tinha
  seis e a tabela já tinha nove: o `.trash`, o `.reason` e o `.pag` ficavam para
  trás. Recriar a tabela com o mesmo nome virava **impossível** («`t.trash` já
  existe; use `Table::abrir`»), e o conteúdo das linhas excluídas de uma tabela
  sobrevivia num arquivo que só `administrar` deveria abrir. É a mesma
  armadilha da peça nova no fim de uma lista que o `rownum` armou na tela — o
  comentário logo acima da lista já explicava por que o `.bkp` entrou nela, e
  ninguém voltou lá quando `.trash` e `.reason` nasceram.

E uma quarta, na tela: a ficha nova de uma tabela de chave `Uuid` **não
gravava**. O campo prometia «em branco … gera um v7» e em branco o servidor
recusava a linha com «obrigatória e recebeu NULL». Hoje a chave primária nasce
com a palavra `novo` escrita — e **só ela**: a primeira versão do conserto
preenchia toda coluna `Uuid`, inclusive a estrangeira, o que geraria um id
sorteado apontando para nada. Quem viu isso foi a captura de tela.

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

### O que a rodada da gestão de tabelas achou

Quatro defeitos, três deles nas próprias operações novas — construir a tela é o
que os fez aparecer.

- **Um servidor `somente_leitura` teria deixado apagar tabela.** As três
  operações novas entraram no despacho e ficaram de fora de `OPS_ESCRITA`, a
  lista que o modo somente-leitura consulta. Criar e *excluir* tabela passariam
  num servidor marcado como só de leitura. A lista é escrita à mão, então o
  conserto veio com um teste que a percorre: quem acrescentar operação que
  grava e esquecer da lista quebra o teste.

- **`criar_schema` estava prometido em dois lugares e não existia.** Aparecia na
  tabela de permissões do `docs/USUARIOS.md` e na lista de operações de
  escrita; pedir pela rede respondia «operacao desconhecida». A biblioteca já
  sabia criar a pasta — faltava a porta. Agora existe, e a tela de nova tabela
  tem o campo.

- **A largura do sufixo entrava depois do teto de volumes.** `Paginacao::nova`
  confere o teto contra os três dígitos do padrão, então pedir 9.999 volumes
  era recusado *antes* de o quarto dígito existir. Entrou
  `com_max_arquivos`, e a ordem passou a ser: largura primeiro, teto depois.
  Também virou explícito que **«sem teto» não existe**: o sufixo tem largura
  fixa, e com três dígitos o volume 1000 não teria nome de arquivo. Teto
  omitido agora vira o maior que cabe, em vez de zero — que o validador
  recusava com uma mensagem que não ajudava quem preencheu a tela.

### O que a rodada da gestão do banco achou

Dois defeitos, e o segundo é uma armadilha que qualquer tela nova podia repetir.

- **Um `onclick` no `#painel` vazava para a tela seguinte.** A gestão do banco
  pendurou o clique no próprio painel; o `folha()` troca o *conteúdo* do painel,
  não o *elemento*, então o tratador sobreviveu à troca de tela e disparava na
  próxima — clicar em «Configurações e diretivas» abria SysColumns. Corrigido
  em dois lugares: o tratador foi para o container das operações, e o `folha()`
  passou a limpar o `onclick` do painel por garantia.

- **O botão primário ocupava a linha inteira.** O `.botao` nasceu com
  `width:100%` para o cartão de entrada, onde é o único da linha. Numa barra de
  ações ele empurrava os outros para baixo. Agora `.acoes .botao` cabe no
  próprio texto.

- **O volume 1 nascia sem período.** Na partição por calendário o volume 1 é
  criado antes da primeira linha, então não havia período para gravar — e
  reabrir a tabela recusava com «não tem fronteira gravada». Agora ele nasce com
  uma sentinela e a primeira linha **adota** o volume em vez de cortar um novo,
  senão a tabela nasceria com um arquivo vazio.

- **A tela de partições calculava por divisão.** Ela dividia `slots` por
  `registros_por_arquivo` — a conta certa para a partição por faixa, e errada
  para a por período, onde o corte depende do calendário. Quatro meses apareciam
  como um volume só. Agora ela lê as fronteiras que o `esquema` devolve.

### O que a revisão do motor de insert achou

- **Perda silenciosa de dado sob gravação concorrente.** O servidor tomava a
  trava para abrir a tabela, **soltava**, e só então tomava de novo para
  gravar. Abrir lê o cabeçalho, e o cabeçalho traz o `slot_count` — o contador
  que decide onde a próxima linha vai. Nessa fresta duas operações abriam a
  tabela, as duas guardavam `slot_count = N`, e as duas gravavam no rowid N+1:
  a segunda por cima da primeira, **sem erro nenhum**.

  Com índice único sobre a coluna, o índice pegava e virava «chave duplicada» —
  foi assim que apareceu. Sem índice único, a linha sumia em silêncio.

  A trava passa a cobrir abrir *e* gravar, num bloco só. Um teste deixa o
  contrato escrito.

- **95% do tempo da inserção era `fsync`.** O diagnóstico anterior — «97% CPU,
  disco parado, a culpa é da B+tree» — foi medido com a *biblioteca*, que não
  sincroniza por linha. Pelo *servidor*, que sincronizava, o gargalo era outro.
  As duas medições estavam certas; era a conclusão que estava sendo aplicada ao
  caminho errado.

- E um susto meu que não era defeito: caçei por meia hora um contador de
  sequência que «zerava sozinho». Era o meu próprio teste, com um caso rotulado
  «tabela sem Sequence» apontando para uma tabela que tinha Sequence — ele
  zerou o contador porque foi exatamente isso que eu pedi.

### O que a revisão do dossiê achou

Duas coisas erradas na própria página, nenhuma no código.

- **Os números do índice lateral estavam fora de ordem.** Do item 4 ao 10 eles
  ficaram um atrás — «04 Paginação» quando Paginação é a 05 —, e o item 4
  perdeu o zero à esquerda. Os *links* apontavam certo o tempo todo; só o número
  exibido divergia. Aconteceu quando uma seção entrou no meio e os
  `<section>` foram renumerados sem o índice. Agora o número sai do próprio
  alvo (`#s7` mostra 07), então não tem como divergir de novo.

- **Duas afirmações defasadas**: «9 comandos» na linha de comando (são 11) e
  «30 das 33 operações na tela» (são 33 das 36). As duas em dois lugares cada.

E uma correção de arrumação: o texto sobre os metadados de campo, a chave
primária e a partição por calendário tinha entrado dentro de *Estado e
roteiro*, que é o roteiro — não o lugar de explicar formato. Foi para onde
pertence: campo e chave na seção 3 (*A tabela, peça a peça*), partição na 5
(*Paginação*). As figuras foram renumeradas em ordem de leitura.

- **A árvore roubava a tela de quem pintasse depois dela.** `montarArvore`
  terminava sempre clicando no Painel; criar uma tabela redesenhava a árvore,
  voltava para a grade — e meio segundo depois o painel chegava por cima. Só
  apareceu no teste de navegador, e só depois que o formulário ficou maior.
  Quem vai pintar a própria tela agora passa `montarArvore(false)`.

## 5. Ninguém pediu, mas a medição aponta

O bloco abaixo é **gerado** de `bancada/resultados.json` — e nesta revisão o
gerador foi consertado, porque ele calculava a razão certa e imprimia a palavra
errada (§4). O que ele diz mudou de sinal: a inserção deixou de ser o buraco, e
sobrou a exclusão.

E o buraco que sobrou **deixou de ser buraco nesta rodada**: o item **1** de
`docs/SPRINTS.md` entrou, e o bloco gerado abaixo ainda não sabe disso porque
o `resultados.json` do repositório continua sendo a corrida de 10.000.000 **com
o comportamento padrão** — que não mudou, e não muda: o `fsync` por exclusão
continua sendo o que um servidor de fábrica faz.

Quem **pedir** `recursos.exclusao_na_janela` vira a fase. Medido a 1.000.000
nesta máquina, duas corridas de cada: **6,30 s / 16,59 s → 0,91 s / 0,96 s**,
contra 1,45 s / 1,90 s do MySQL(R). Ou seja, a fase sai de perder por 4,3× para
ganhar por 1,9×, e o motor passa a ganhar **nas cinco**. O caso a caso do que
se arrisca — inclusive o **quarto caso** de queda que o sprint dizia não
existir — está em `docs/DESEMPENHO.md` §4.12.

O que **continua** valendo do parágrafo antigo: com o padrão de fábrica, a
exclusão é a única fase em que o motor perde, e é ela que o bloco abaixo
descreve.

<!-- pendencias:insercao:inicio -->
A bancada de 10 milhões mede 5 fases: o motor ganha em 4 e perde em 1.

**A exclusão é a única fase em que o motor perde:** **1,3× mais devagar** (6,27 s contra 4,73 s, 20.000 linhas).

Onde ele ganha, em ordem de folga: busca pontual **13,1× mais rápida** (0,20 s contra 2,64 s, 20.000 linhas); atualização **12,1× mais rápida** (0,45 s contra 5,51 s, 20.000 linhas); varredura por faixa **11,1× mais rápida** (1,41 s contra 15,70 s, 1.250.000 linhas); inserção **1,2× mais rápida** (91,49 s contra 112,37 s, 10.000.000 linhas).

Na carga: **109.300 linhas/s contra 88.994** do MySQL(R), com 77 s de CPU para 91 s de relógio (84%) e 0,0 MiB lidos do disco — é processador, não disco. E escreve muito menos: 1,81 GiB contra 32,07 GiB. A taxa **cai com o tamanho**: o primeiro milhão entra a 47.847/s, o último a 22.026/s — 54% mais devagar no fim do que no começo.

Contrapartida honesta: **ocupa 2,43 GiB em disco contra 0,88 GiB**, porque o `.reg` é de slot fixo — o preço do endereçamento O(1) e da ordem de digitação.

Se sobrar uma rodada para o motor em vez de para recurso novo, é na **exclusão** que ela rende — é o que sobrou.

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

## 7. Uma afirmação da folha de marca que continua falsa — e uma que virou verdade

Registrado no `CLAUDE.md` e repetido aqui porque é fácil esquecer: a folha diz
*ACID compliant* e *built-in replication*.

- ***ACID compliant* continua falso.** Não há transação: sem `commit` e sem
  `rollback` de várias operações, e a própria tela de Transações diz isso em
  letra grande. Existe o desfazer de UMA inserção quando o índice recusa, e é
  outra coisa. **Não repetir em documento técnico enquanto não for.**
- ***built-in replication* deixou de ser falso.** O pedido 19 registra a
  medição: `.log` v2 com a imagem da linha, ops `posicao`/`replicar`/`aplicar`,
  o laço dentro do `phxsqld`, quatro servidores medidos e retrato SHA-256 das
  quatro tabelas idêntico. Os quatro modos estão em `REPLICACAO.md` §9.

Esta seção mesma é a lição: ela afirmava «a replicação não transporta evento»
enquanto o item 19, no MESMO documento, trazia a medição do contrário. **A lista
do que falta também é palpite até alguém medir** — inclusive a lista de
afirmações falsas.
