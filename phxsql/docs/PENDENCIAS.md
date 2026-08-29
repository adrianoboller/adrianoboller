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
| ☑️ | 17 | Download dos fontes e do compilado Linux/Windows, com manual | `./empacotar.sh`, três zips conferidos |
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
| ☑️ | 118 | **Rodar em Docker** | imagem `scratch`, 4,7 MB, sem shell nem gerenciador de pacotes — só possível por não haver dependência externa. Exige o alvo **musl**: medi, o padrão linka `libc.so.6` e o carregador dinâmico, e `FROM scratch` não subiria. O binário musl roda; o `docker build` **não foi executado** (sem daemon na máquina) |
| ☑️ | 119 | **Várias instâncias em portas diferentes** | já era assim: cada `phxsqld` lê o `config.json` do diretório em que foi iniciado. Provado com quatro de uma vez em `bancada/replicacao/` e com três em contêineres |
| ☑️ | 120 | **Chave composta livre e única** | as duas já existiam no formato; faltava teste que as separasse. A única recusa **antes de gravar**, e a recusa não consome slot |
| ☑️ | 121 | **Analisar o PDF do HFSQL(R) contra o projeto** | `docs/HFSQL.md`, item por item. O que falta, em ordem de valor: direito no nível da **tabela**, índice de texto completo, índice parcial, ordenação linguística, e a **janela de conflito de escrita** |
| ☑️ | 122 | **Analisar o DBeaver: o que dá para reaproveitar** | `docs/DBEAVER.md`. Código: não vale — Apache 2.0 permite, mas seria trazer o Eclipse inteiro. Ferramenta: vale muito, e os três caminhos exigem a **mesma** camada SQL |
| ☑️ | 123 | **Janela de conflito de escrita** | feito **sem mudar formato**: a versão por registro do `.reg` estava lá desde a v1. `ler` devolve a versão com `"com_versao"`, `atualizar`/`excluir`/`restaurar` conferem a versão que o cliente mandar, e a recusa é o erro **3004 `CONFLITO`**. A janela mostra as três colunas do PDF e vai além dele: **já vem marcado quem mexeu em cada coluna**, então dois que editaram campos diferentes saem com os dois trabalhos. A conferência é **pedida, não imposta** — cliente antigo continua gravando |
| ☑️ | 124 | **Direito no nível da tabela** | `"tabelas"` dentro do objeto da base, e a regra da tabela **substitui** a da base ali — o que permite tirar `folha` de quem lê o banco inteiro **e** dar `clientes` a quem não lê o banco nenhum (interseção só resolveria o primeiro). O portão continua sendo um só; `juntar` e `unir` ganharam conferência própria porque não têm o campo `"tabela"` que ele lê. A árvore e o catálogo passaram a listar só o que dá para abrir. 9 testes |
| ☑️ | 125 | **Marcar coluna como dado pessoal (LGPD/GDPR)** | PSCH **v6**, três graus (`nao`/`pessoal`/`sensivel`, LGPD art. 5º I e II), com o byte no **fim** do bloco para quem lê v5 parar antes. Op `dados_pessoais` audita a base — e como ela **não tem campo `tabela`** (o furo do `juntar`/`unir`), filtra tabela a tabela por dentro. Não adivinha por nome; devolve quantas colunas ficaram sem classificação. Mais a tela que audita, que diz *que não sabe* quando o esquema não traz a marca |
| ☑️ | 126 | **Cluster: endereço único, eleição e promoção automática** | `crates/phxsql-server/src/cluster.rs`: pulso, época, eleição por maioria com desempate por prioridade (`vencedor`, `:115`), promoção (`:334`) e **rebaixamento sozinho** ao ver época maior (`:348`). Escrita numa réplica devolve `REDIRECIONA host:porta` (erro 4003) — endereço único **pela semântica do protocolo**, e não por VIP, que é infraestrutura e não banco. Medido em `bancada/cluster/`: a escrita volta em **3,9–4,3 s** com janela de 4 s. **Sem o bloco `cluster` no `config.json` nada muda** — nenhuma thread sobe, nenhum portão muda —, e o teste que trava isso é `sem_o_bloco_cluster_nada_muda`. O `docs/CLUSTER.md` §2 diz também o que ele **não** garante: não é multi-master, não há balanceador embutido, e a lista de nós é do arquivo |
| ◐ | 127 | **Diagrama ER e editor de modelo** | **as duas metades entraram.** O diagrama é `ui/diagrama-er.js` (sete defeitos achados abrindo no navegador, e não lendo). O **editor** é o mesmo arquivo em modo de edição: arrastar a caixa pelo título move a tabela — e a pega é um retângulo transparente por cima do título, senão o alvo seria a letra —, e arrastar a linha de uma coluna até a coluna de outra **declara a chave estrangeira**, que o cartão da tabela também exclui. Por baixo, `criar_tabela` declara FK pelo protocolo, `duplicar_tabela` preserva, e um teste trava que *declarar não é aplicar*: o motor não impõe a FK, e o teste falha no dia em que isso mudar em silêncio. **O que falta agora tem nome:** alterar a estrutura de uma tabela que já tem dado. O slot é de largura fixa e o `slot_size` decide o endereço de cada linha gravada, então acrescentar coluna é reescrever o `.reg` preservando o rowid — e a tela diz isso em vez de oferecer um formulário que fingiria salvar. É o item **25** de `docs/SPRINTS.md` |
| ☑️ | 128 | **`BULKINSERT(true/false)`: a tabela reservada para a carga** | reserva exclusiva por conexão, com erro **4002 `EM_CARGA`** para os outros — nomeando quem reservou e com `repetir: true`, que é o que separa «espere» de «você não pode». **1,53× medido** (43.500 → 66.500 linhas/s), porque reservada a janela de durabilidade não fecha e a carga vira um `fsync` só. Duas redes contra reserva órfã: a queda da conexão solta na hora, e `recursos.carga_prazo_min` solta o soquete pendurado. Só pela porta de dados. 10 testes mais a prova pelo soquete em `bancada/carga/bulkinsert.py` |
| ☑️ | 129 | **O motor SQL tem de conhecer o `BULKINSERT`; e o prazo, no `config.json` e na tela** | o prazo já era `recursos.carga_prazo_min` (padrão 30 min) desde o 128; entrou a **tela de configuração explicando cada ajuste** — com a seção «Cargas em andamento» listando quem reservou o quê — e o **`docs/SQL.md`**, que diz o que a camada SQL precisa saber antes de existir. `BULKINSERT` não é açúcar sintático: é palavra reservada, vale para a **sessão** (um driver que multiplexa conexões quebra a exclusividade sem avisar) e o `EM_CARGA` tem de virar *serialization failure* no SQLSTATE, não *access denied*. E a frase que o documento repete alto: **não é transação** — ele reserva a tabela, não desfaz nada |
| ☑️ | 130 | **`phxsqlcmd`: interface terminal com todos os comandos, `/help` e `/help comando`** | crate `phxsql-cmd`, autenticando pelo mesmo desafio-resposta da réplica. O `/help` **vem do servidor** (op `catalogo`: **108** operações descritas por dados (eram 79 quando o `phxsqlcmd` nasceu; a contagem é da constante `OPERACOES` em `server/src/catalogo.rs`), com um teste que deriva a lista do próprio `despachar` — operação nova não nasce sem descrição, e ajuda escrita à mão não existe para envelhecer). 9 testes por soquete; o soquete achou o que a unidade não achava (o partidor comia as aspas do JSON). Sem histórico/setas nesta rodada, dito no `--help` |
| ☑️ | 67 | **Botão e menu Tabelas** para gerir as tabelas do banco: nova, estrutura, editar conteúdo, partições, duplicar, reparar tabela, reparar índice e excluir — e **Gestão de transações** no menu de ferramentas | as oito operações funcionam de ponta a ponta; três delas (`criar_tabela`, `duplicar_tabela`, `excluir_tabela`) nasceram aqui, e `criar_schema` — prometido na documentação e nunca despachado — junto |
| ☑️ | 131 | **Prioridade no MULTILINK: conectar no MySQL(R) e ver a tabela `clientes` do outro lado do DbLink** | provado contra o **MySQL(R) 8.0.46 real**, não o servidor falso: `caching_sha2_password` pelos dois caminhos documentados, `dblink_tabelas`/`dblink_estrutura`/`dblink_ler` trazendo as 5 linhas com o DECIMAL certo — e a grade da tela somando sobre dado que nunca esteve num arquivo nosso. Roteiro de refazer em `docs/DBLINK.md`. É o DbLink **nativo, zero dependências** — o destino do pacote MULTILINK, alcançado sem as 582 crates dele |
| ☑️ | 132 | **Assistente de conexão DbLink**: cria a conexão, testa, escolhe base e tabelas ligadas, e o **job de sincronia** gravando entre si automaticamente | cinco passos que só avançam com o anterior provado, e por baixo a **sincronia de tabelas primas**: convergência de estado pela chave, sentido (puxar/empurrar/dois) e dono (aqui/lá) **por linha**, colunas casadas **por nome**, empurrão reentrável (`ON DUPLICATE KEY UPDATE`), teto com recusa clara. Exclusão **não** viaja — limite de desenho, provado no estágio 5 da prova. `bancada/dblink/prova-sincronia.py` roda os 7 estágios contra o MySQL(R) vivo (inclusive o job puxando sozinho); o assistente foi exercitado no navegador e o exercício achou a árvore que não se remontava. 15 testes de unidade das partes puras, com o defeito do sinal do decimal provado nos dois sentidos. `docs/DBLINK.md` |
| ☑️ | 133 | **Validar o profiler e o log dele em `.txt`** | validado por **soquete**, contra servidor de verdade, e a validação achou quatro coisas. A **redação passou**: 20 pedidos torcidos — chave escapada em `\u0073enha`, maiúscula, espaço antes dos dois-pontos, aninhamento fundo, lote de 200 linhas, corpo malformado — e a senha não apareceu em nenhum, nem no anel nem no arquivo; repondo o defeito (recorte no lugar da análise) caem sete testes. O que **não** passou: (1) o `.txt` aceitava **linha forjada** — um `"op"` com quebra de linha dentro deixava no arquivo uma segunda linha que se lia como evento de outro IP; (2) o profiler **não era só do administrador**, apesar de a ficha dizer que era — quem tem `bases: {"*": {administrar}}` e é leitor passava pelo portão geral, e por aí leu a **linha** de uma tabela que o servidor acabara de negar a ele *e* mandou o servidor escrever num arquivo escolhido; (3) com o **disco cheio** (tmpfs de 64 KB, 400 pedidos, 223 linhas gravadas) a tela seguia dizendo «gravando em …»; (4) `{"senha ":…}` e `["senha",…]` escapavam da redação. Consertados os quatro, mais o `terminou` que varria o anel **do mais antigo** quando o evento procurado é sempre o mais novo. fica **de fora e anotado**: o `.txt` não rotaciona — 345 B por pedido, sem teto, o que dá 1,2 GB/hora num servidor com 1.000 pedidos/s; o que entrou foi a tela mostrar o tamanho e avisar quando para de gravar. `docs/SEGURANCA.md` §10, `docs/DESEMPENHO.md` §2.3, 6 sondas em `bancada/profiler/` |

| ☑️ | 134 | **Falta o botão restaurar** — backup que não restaura não é backup | `restaurar_backup` com dois modos: **com outro nome** (o padrão — não destrói nada, não para serviço nenhum e não segura a trava durante a cópia) e **por cima** (exige a porta de dados parada, nenhuma conexão aberta e `"confirmar":true`; o database substituído **não é apagado**, sai da raiz e o caminho volta na resposta). O SHA-256 de cada arquivo é conferido **antes** de o destino ser tocado, num palco fora da raiz: backup podre não vira database pela metade. Mais `backups`, que lista as cópias lendo só o manifesto de cada ZIP; o leitor de ZIP com INFLATE completo (os três blocos da RFC 1951, o dinâmico provado por vetor da zlib); a tela com as duas formas lado a lado; e o **botão na barra**, ao lado do Backup — botão que não se acha não existe. O portão próprio fecha a porta dos fundos que o campo `"database"` não enxerga: o banco que vem DENTRO do backup. 26 testes novos (1.132 no total), cada guarda provada com o defeito reposto, mais a prova pelo navegador nos dois temas. `docs/RESTAURACAO.md` |

| ☑️ | 135 | **Bateria de testes de backend e de frontend, e avaliação do design** | `testes-web/`: onze casos que sobem o próprio servidor (portas 6200/6201), entram pela tela de login e percorrem **112 telas** nos **dois temas**, reprovando em três canais de erro — `pageerror`, o recado vermelho da barra e o do painel — porque o `ligarMenu` captura toda exceção e ela nunca vira `pageerror`. A bateria **recusa rodar com binário velho**: a página é `include_str!`. Achou seis defeitos que 1.106 testes verdes não achavam — a tela de LGPD lendo um campo que o servidor nunca mandou, a tela de entrada em branco por **12,7 s** onde a rede engole a fonte da marca (**116×**), três mordidas do CSS global e um contraste de 3,85:1 no tema claro. E a varredura do portão de permissão achou **três** operações que leem a base inteira sem o campo `tabela` que o portão confere: `pivotar` (a tabela negada como lado da junção — dado, nome e contagem), `sequencias` e `posicao`. `docs/TESTES.md` |

| ☑️ | 136 | **Criptografia dos dados: cifrar o valor da coluna marcada como dado pessoal** | escolha **(c)** do dono, entre as quatro medidas: (a) slot inteiro 0,59 µs/linha, (b) página do `.ndx` 0,23 µs, **(c) coluna marcada 0,10 µs (1% da inserção)**, (d) arquivo inteiro **194 ms para ler uma linha** — 320.000×, a saída que não existe. Cifra as faixas marcadas **no lugar** (o ChaCha20 é de fluxo: não muda o tamanho, nenhum offset se move), com **uma** etiqueta por linha no fim do slot; `.reg` versão **5**, cabeçalho de 192 bytes. `Memo`/`Bin` marcados são selados antes de virar bloco, com nonce de 24 bytes à frente. Nonce = rowid + volume + versão + **8 bytes sorteados nos que já eram reservados** — zero de formato. AAD amarra o endereço: copiar o slot 5 sobre o 9 com o CRC certo **não passa**. Entrou junto **XChaCha20-Poly1305/HChaCha20** com os vetores do draft-irtf-cfrg-xchacha-03 e um gerador de bytes do processo (apagamento rápido de chave). **O `.ndx` continua em claro, e há teste que prova o vazamento** — `SEGURANCA.md` §11.3 |
| ☑️ | 137 | **FrogCript como modo escolhido, com o aviso escrito** | `cifra.modo: "frogcript"` — transposição, duas camadas e a direção escondida, **salto e separador parametrizáveis**. O padrão continua sendo o AEAD. O documento diz o que ele acrescenta (formato), o que **não** acrescenta (força: é a §9 do próprio autor) e o que custa, **medido**: 2,77 µs e 189 bytes contra 0,10 µs e 38 do AEAD; o `frogcript.py` de referência custa **1.137 ms por valor** (410.000×) porque deriva a chave 4 vezes **por valor**, e ~397 bytes (18× o texto). **Sem AES**: a estrutura roda sobre o ChaCha20 da casa, e por isso **não há compatibilidade com o que foi cifrado em Python** — escrito, não escondido. Escrever AES é decisão do dono e está na mesa com o custo (`SEGURANCA.md` §11.4) |

| ☑️ | 138 | **Os data grids devem ter esses recursos** | Levantadas as **quatro** grades do console (Conteúdo, DbLink tabela, DbLink SQL, Junção/União) e cruzadas com as duas fontes do pacote. O `phx-grid` do pacote (0.6.0 e 0.7.0) é **mais velho** que o nosso, e o `phoenix_data_grid_x` v1–v38 é outro produto (DataFusion, Timescale, OTLP, Dioxus) — **recusa fundamentada**, não pendência: é a parede do #95. O que o cruzamento achou foi que **a capacidade existia e a tela não ligava**: linha de filtro no cabeçalho, congelar coluna e seleção estavam escritos no `phx-grid` desde a 0.6.0 e desligados nas quatro telas. Ligados, mais **exportar a vista** (o que está na tela, com filtro, ordem e colunas — 44 linhas na vista, 44 no arquivo, provado pelo download de verdade), **layout lembrado** (largura/ordem/ocultas/congeladas/página; filtro e ordenação **não** — filtro que volta sozinho é a mesma mentira com uma noite de intervalo) e **duplo clique abre a ficha**. Exercitar achou **quatro defeitos já em produção** que ler o código não acha: (1) o funil da coluna estava **ilegível** — caixinha de marcar de **204 px** por cima do valor e «Blumenau» como «BLUMENAU», a quarta vez que o CSS global morde componente novo; (2) o menu de Colunas **nunca montava** (defeito do `phx-grid` de origem, igual na 0.6.0, 0.7.0 e 0.8.0), então esconder coluna nunca funcionou; (3) seleção + agrupamento contavam cabeçalho de grupo como linha — 100 na página, 93 de dado, e o «marcar todas» nunca fechava; (4) a linha de filtro engordava toda coluna de 68 para **237 px**, e `size` **não vale** para `input[type=number]` (118 px depois). E a versão **mentia em três lugares ao mesmo tempo** (cabeçalho `v0.1.0`, `versao: 0.8.0`, código além da 0.8.0): hoje há `grade_versao_nao_mente`. **Edição na célula é recusa fundamentada**: duplicaria a guarda de escrita concorrente da ficha, ou não a teria. **Fica anotado o que falta**: o `varrer` não tem `WHERE`, então filtro e busca respondem sobre a janela que a tela trouxe e não sobre a tabela (medido: 25 de Blumenau quando a tabela tem 2.500) — o contrato de *pushdown* existe na grade desde a 0.7.0 e o servidor não o atende. Enquanto não atende, a tela **diz**. `docs/GRADE.md` |

<!-- pedidos:contagem:inicio -->
**138 pedidos: 135 feitos · 3 parciais · 0 planejados.**

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

As três primeiras são os ◐ da tabela lá em cima. A que trata de chave
estrangeira **não é um pedido seu** — é um buraco achado numa revisão, dentro
de um pedido marcado feito, e fica aqui para não sumir de vista.

| # | O que você pediu | O que existe | O que falta |
|---|---|---|---|
| 18 | **Subir o PhxSql no GitHub** | a branch `claude/capacidades-disponiveis-y6auxh` em `adrianoboller/adrianoboller`, com o histórico completo. A credencial **lê** o repositório e enxerga a branch — conferido nesta revisão, não suposto | um repositório **próprio**, e o impedimento é de **identidade, não de permissão**: esta sessão autentica como `EnginePrint` (id 322529492, criada em 2026-08-29, zero repositórios públicos), que não é você. `create_repository` responde 403 porque ninguém cria repositório em nome de outra pessoa — e, se criasse, o repositório seria **dela**, não seu. Destrava com você criando `adrianoboller/phxsql` e dando acesso a essa app. **Não há trabalho de engenharia esperando aqui** |
| 86 | **DbLink para PostgreSQL(R) e outros** | **cliente, dialeto e ligação prontos**: `server/src/pg/` (721 linhas de protocolo mais 278 de SCRAM-SHA-256 conferido contra o vetor do RFC 7677), SQL por motor em `dblink/dialeto.rs`, e as cinco operações do DbLink reescritas para não saberem qual motor atendem. Provado por soquete contra um servidor de protocolo próprio, byte a byte, nos dois sentidos | só o que o nome do pedido diz: **a prova contra um PostgreSQL(R) de verdade**, que não existe nesta máquina. O roteiro do que ela exige está em `docs/DBLINK.md`, e o precedente é o pedido 131, que fez exatamente isso contra um MySQL(R) 8.0.46 real e achou o que o servidor falso não acharia |
| 127 | **Diagrama ER e editor de modelo** | o diagrama (`ui/diagrama-er.js`, sete defeitos achados no navegador) **e o editor**: arrastar a caixa pelo título move a tabela, arrastar a linha de uma coluna até a coluna de outra **declara a chave estrangeira**, e o cartão da tabela exclui a declaração. `criar_tabela` declara FK pelo protocolo, com `duplicar_tabela` preservando e um teste que trava que *declarar não é aplicar* | **alterar a estrutura de uma tabela que já tem dado.** O slot é de largura fixa e o `slot_size` decide o endereço de cada linha (`offset = início + (rowid−1) × slot`), então acrescentar coluna é reescrever o `.reg` preservando o rowid. A tela diz isso em vez de oferecer um formulário que fingiria salvar. É o item **25** de `docs/SPRINTS.md`, com a armadilha nomeada: a coluna nova entra **antes** de `softdeleted` e `rownum` |
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
| 1 | **Restaurar backup** | não existe, e a tela diz isso: o botão nasce apagado em `crates/phxsql-server/ui/index.html:5046` (`class="op afazer"`, «ainda não existe — clique para ver o que falta»). Fazer o backup e **conferir** uma cópia funcionam; voltar não. E voltar é mais do que copiar: é decidir o que fazer com o que está lá. Sobrescrever um database em uso, com a trava tomada, precisa de desenho — parar, restaurar ao lado e trocar, ou restaurar com outro nome. **É a metade que falta de um pedido que já se anuncia como «Backup e restauração»** |
| 2 | **As 13 tomadas da trava de dados fora do ponto único** | `travar_dados()` (`server/src/servidor.rs:719`) é onde a telemetria cronometra a espera na fila — e o comentário dele diz ser «o **único** lugar que a toma». Medido nesta revisão: **não é.** Há 14 `self.dados.lock()` no arquivo, e 13 estão fora dele: `mensagens_atualizar` (:1095), `semear_mensagens` (:1151), `posicao_do_diario` (:2066), `alcancar_tabela_bidi` (:2344), `atender_http` (:3897), `op_mensagens` (:5315), `op_idiomas` (:5390), `op_idiomas_carga` (:5401), `op_idiomas_padrao` (:5434), `op_idiomas_exportar` (:5451), `op_idiomas_importar` (:5462), `descarregar_sujas` (:6268) e `executar_rotina` (:6741). As duas últimas são as que doem: o **despejo do cache** segura a trava por uma passada inteira, e o **corpo de um gatilho ou procedimento** a segura pelo tempo que quiser — que é exatamente a atividade longa que o painel existe para mostrar. Enquanto elas não passarem por `travar_dados()`, o `espera_ms_s` da telemetria é o de uma parte, e o `docs/TELEMETRIA.md` §2.1 afirma o contrário. **Refazer a conta: `grep -c 'self\.dados\.lock()' servidor.rs`** |
| 3 | **Transações** | tem tela (Ferramentas → Gestão de transações), e ela diz o que existe e o que não existe em vez de fingir. Hoje a inserção desfaz o que gravou se um índice falhar, e a trava única serializa as escritas — mas não há journal com a **imagem anterior** da linha, nem identificador de transação na sessão, nem `commit`/`rollback` de várias operações. O primeiro tijolo é o item **21** de `docs/SPRINTS.md`, e ele é deliberadamente a metade que **não** depende da transação |
| 4 | **Concorrência fina** | uma trava única serializa todo acesso a dados. É o que o `docs/SPRINTS-TERADATA.md` §4.5 aponta ao recusar *workload management*: prioridade sobre uma fila de um só não é prioridade |
| 5 | **Modo exclusivo** | tela apagada em *Gerir banco* (`ui/index.html:4799`). Meio caminho existe e não estava escrito aqui: o `BULKINSERT` **já reserva uma tabela por conexão**, com erro 4002 `EM_CARGA` nomeando quem reservou e `repetir: true` (pedido 128). O que falta é reservar **por período** e para outra coisa que não carga — e isso depende da trava por tabela, que é o item 4 |
| 6 | **Compactação** | o formato prevê e **mede** o espaço morto; falta o comando. E há dois números contra: compactar renumeraria rowid, e rowid é endereço (`docs/COMPARACAO.md`); e a compactação do **diário** foi medida e recusada **duas vezes** — 14,7% no melhor corte contra 2,1× que o mesmo esforço compraria no `.ndx` (`docs/DESEMPENHO.md` §4.7.3). O que sobrou de vivo nesse assunto é o item **19** de `docs/SPRINTS.md`, que é outra conta |
| 7 | **Editar `config.json` e usuários pela web** | **metade entrou** e não estava escrito aqui: a op `config_gravar` existe (`servidor.rs:2752`), com portão próprio de `administrar` e rastro no log de quem mudou o quê. O que ela **não** grava é deliberado e está na lista `CAMPOS_EDITAVEIS`: token, segurança, cadastro de usuários, cifra, credencial de e-mail e replicação — uma sessão roubada não abre o firewall, não cria supervisor e não vira este servidor para outro source. **Falta o cadastro de usuários pela web**, que é a metade difícil: senha nunca em claro em ponto nenhum do caminho |
| 8 | **TLS** | o tráfego depende de túnel. A credencial já não vai em claro quando se usa desafio-resposta; os dados, sim. Aparece também no `docs/REPLICACAO.md` §13 como o que falta ao transporte da replicação |
| 9 | **Integração no FraseSQL** como `engine = "phxsql"` | dependia do ODBC, que entrou. O catálogo do `.reg` já é «o mesmo formato que o catálogo do FraseSQL espera» (`store/src/catalogo.rs:21`), e o contrato de integração está lido em `docs/PLANO.md` §2. Nunca foi tentado |
| 10 | **A interface em seis idiomas — 11% dela** | a máquina existe e agora há **medida**: `cargo run --example textos-fora-da-fabrica -p phxsql-server` conta **258 textos na fábrica e 1.994 ainda cravados em português**, com arquivo e linha de cada um. Traduzido 100%: a tela de entrada, o cromo inteiro (menu, barra, abas, árvore, painel lateral), a tela de Idiomas e o cabeçalho da tela de Configurações. Falta o miolo das telas — por ordem do que se vê: os 99 títulos e subtítulos de `folha(`, os 136 cabeçalhos de coluna, os 39 recados de `avisar(` e as 19 perguntas de `confirm(`. A **catraca** do `conferidor.rs` (`TETO`) impede que o número suba: tela nova em português cravado reprova `cargo test`. `docs/MENSAGENS.md` |

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
  documentação.

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

E, ao contrário de antes, o buraco que sobrou **já tem proposta com número**: o
`fsync` da lixeira mede 6,5 s → 0,83 s (7,8×) em 20.000 exclusões, e é o item
**1** de `docs/SPRINTS.md` — o único da lista de 27 cujo valor está medido em
vez de julgado.

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
