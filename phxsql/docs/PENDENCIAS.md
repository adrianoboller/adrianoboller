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
| ☑️ | 14 | **Quantidade de registros e arquivos no create table** | op `criar_tabela` no protocolo e tela **Nova tabela** com registros por arquivo, dígitos do sufixo e teto de volumes. A CLI ainda não tem o comando |
| ☑️ | 15 | Organograma, fluxograma e dossiê | 19 seções, 18 figuras, tudo em SVG à mão |
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
| ☑️ | 30 | **Interface web parecida com o Centro de Controle HFSQL(R)** | árvore, abas, painel, administração, menu, ferramentas, **View Database com edição**, **gestão de tabelas** e **gestão do banco** — 36 das 39 operações. Fora: `buscar`, `desbloquear` e `criar_schema`, que acontece sozinho quando a tela cria tabela dentro de um schema |
| ☑️ | 66 | **[+] na árvore** para criar database, **About no menu Ajuda**, **tela de créditos** com a fênix, e **View Database** com grade de tabelas e edição | fecha a edição de dados: `ler`, `inserir`, `atualizar` e `excluir` ganharam tela |
| ☑️ | 65 | **Barra de ferramentas** com Start/Stop, Query, Usuários, Diretivas, Bancos, Duplicar, Conexões, Transações, Importar, Repair, Backup, Replicação, Server Mail, Blockchain e Ajuda | 20 ferramentas hoje, ícone colorido; **16 funcionam**, 4 apagadas dizendo o que falta |
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
| ☑️ | 63 | **Barra de menu superior tradicional** | nove menus e 53 itens hoje, Alt/setas/Esc |
| ☑️ | 64 | Cadê o sol e a lua? | respondida — estavam lá, o recorte da captura é que cortava |
| ☑️ | 68 | **Copiar e colar tabela** de um lugar para outro | `copiar_tabela` atravessa databases e schemas; a permissão de criar é conferida **no destino** |
| ☑️ | 69 | **Configurações gerais do servidor, do banco e dos usuários**, cada uma com sua tela | três telas, três alcances. **Leem, não gravam** — gravar o `config.json` pela web daria a uma sessão roubada o poder de abrir o firewall e criar supervisor |
| ☑️ | 70 | **SysTables e SysColumns** | o catálogo em forma de dado, e o dicionário de dados com id, caption, descrição, máscara e papel na chave |
| ☑️ | 71 | **Gerir database**: conexões, triggers, procedures, arquivos bloqueados, modo exclusivo, transações, backup/restaure e jobs | 15 itens numa tela; 11 funcionam, 4 apagados dizendo o que falta e de que dependem |
| ☑️ | 72 | **Diretivas de acesso ao banco e diretivas de acesso** | os seis portões na ordem em que fecham, e quem alcança o banco resolvido pelas três regras |
| ☑️ | 73 | **Editor de menu** para trocar o nome exibido | 81 rótulos; fica no navegador de quem mexeu, não no servidor |
| ☑️ | 74 | **Configurações e diretivas das tabelas** | a geometria decidida na criação, os índices e chaves, e o que a tabela herda do servidor |
| ☑️ | 75 | **Cadastro de campos** com id automático, nome, caption, descrição, tipo, tamanho, máscara e chave primária/estrangeira/composta | **mudança de formato**: esquema `PSCH` v3. O `id` é UUID v7 e nunca muda; o papel na chave é derivado dos índices |
| ☑️ | 76 | **Tabela particionada** com grade de gestão: por faixa de quantidade, mensal, bimestral, semestral ou anual | **mudança de formato**: o volume corta pelo calendário, e cada volume grava a própria fronteira no cabeçalho |
| ☑️ | 79 | **Seção de cache, memória, CPU, threads e usuários no `config.json`** | seção `recursos`, com sete ajustes. `cache_paginas` e `memoria_max_mb` são lidos e mostrados mas **ainda não impostos** — o buffer pool é o trabalho seguinte |
| ☑️ | 80 | **Validar e revisar o motor de insert; deixar a gravação mais rápida** | medido: **95% do tempo era `fsync`**. Durabilidade configurável dá **20,4×**. E a medição achou uma **perda silenciosa de dado** sob gravação concorrente, corrigida |
| ☑️ | 81 | **Tabela `sequences` na raiz do banco**, com todas as tabelas e um BigInt ajustável pelo admin | operações `sequencias` e `ajustar_sequencia`. O contador continua no cabeçalho de cada `.reg`: a operação junta para mostrar, e não cria uma segunda cópia que divergiria |
| ☑️ | 82 | **Bancos em pastas, cada schema uma subpasta** | já era assim desde o início — conferido: `dados/loja/matriz/estoque.reg` |
| ◐ | 83 | **Comandos SQL reconhecem `matriz.estoque` e `filial.estoque`** | o **endereçamento** funciona hoje em toda operação: `tabela: "matriz.estoque"` abre a pasta certa. O que falta é o **SQL** — não há parser, e ele é o planejado nº 6 |
| ☑️ | 77 | **Group dinâmico pelas colunas na grade**, como o Janus GridEX(R) e o DevExpress(R) | já havia arrastar e multinível; entraram ordem por nível, rodapé por grupo com o total na coluna, total geral e expandir/recolher tudo |
| ☑️ | 78 | **Botão que monta pivot dinâmico com assistente**, pedindo as tabelas envolvidas | operação `pivotar` no servidor com *hash join*, seis resumos e granularidade de data; assistente de três passos na tela |
| ☑️ | 84 | **Botão DbLink na barra** e **definições do DbLink** no menu Configurações | cadastro com apelido, endereço, credencial e teto; a senha nunca sai em JSON. Nasce **somente-leitura** |
| ☑️ | 85 | **Conectar em banco de fora e ver as tabelas na grade tipo Janus(R)** — MySQL(R) primeiro | protocolo do MySQL(R) escrito à mão, só `std`; testado contra um MySQL(R) 8.0.46 de verdade. A grade é a **mesma** das tabelas daqui |
| ◐ | 86 | **Depois testar com PostgreSQL(R) e outros** | a definição já pode ser guardada e o cadastro reconhece o motor; o **cliente ainda não existe**. Os tijolos estão prontos: o SCRAM-SHA-256 do PostgreSQL(R) usa SHA-256, HMAC e PBKDF2, que o projeto já tem |
| ☑️ | 87 | **Monitor de espaço em disco no dashboard** | uma barra por caminho que o servidor usa — o `base`, o destino do backup e o que estiver em `alertas.caminhos`. A conta é sobre `usado+livre`, como a do `df` |
| ☑️ | 88 | **Definir no config o local de armazenamento** (`C:\database`, `D:\database`) | é o campo `base`, e sempre aceitou caminho absoluto. O que faltava era a tela mostrar o caminho **já resolvido**: relativo vale a partir de onde o servidor foi iniciado, e subir por outro caminho passa a ver outro banco |
| ☑️ | 89 | **Alerta de falta de espaço por e-mail**, configurado no config | seção `alertas`, com dois limites no OU e silêncio entre avisos. Cliente SMTP escrito aqui — **sem TLS**, serve para relé interno |
| ☑️ | 90 | **Monitores de placa de rede, CPU, memória e HDs no dashboard** | do `/proc`, com taxa entre duas amostras; renovam sozinhos a cada quatro segundos. **Só no Linux** — fora dele a tela diz que não sabe medir, em vez de mostrar zero |
| ☑️ | 91 | **Operações básicas de union, inner join e as outras do diagrama** | as sete figuras (`interna`, `esquerda`, `direita`, `completa`, `so_esquerda`, `so_direita`, `so_dos_lados`) mais `UNION` e `UNION ALL`. Na tela se escolhe **clicando no desenho de Venn**, com o SQL equivalente escrito embaixo. Chave composta, e nulo que não casa com nulo, como no SQL |
| ☑️ | 92 | **Revisar o help do MySQL(R) e do MariaDB(R) e ver o que melhorar** | comparado contra os dois help embutidos rodando (705 e 833 tópicos). Entraram: erro com **código estável**, `sessoes` (PROCESSLIST), `encerrar_sessao` (KILL), `estatisticas` com percentis/histograma/mais-lentas/por-tabela, `checksum` e tempo no ar. O que ficou fora está em `docs/COMPARACAO.md` **com o motivo** |
| ☑️ | 93 | **Exportar as tabelas para xlsx, json, xml, html, csv, docx e txt** | os sete, escritos aqui. XLSX e DOCX são ZIP de XML, e o projeto já escrevia ZIP com DEFLATE — planilha com cabeçalho pintado, zebra, painel congelado e autofiltro; data como número com formato, não como texto. Conferido com leitores independentes |
| ☑️ | 94 | **O dossiê estava esquecendo o `.bkp`** | e no pior lugar: a seção do **fluxo de gravação**. O espelho não aparecia no desenho, e parecia uma cópia feita depois — ele é escrito no mesmo instante. Corrigido no dossiê, no `FORMATO.md`, no `MANUAL.txt` e no `README` |
| ◐ | 95 | **Integrar o MULTILINK no DbLink** | **bloqueado como está**: o pacote traz só binários (`.rlib`), sem fonte, compilados com rustc 1.98 contra o 1.94 daqui — provado, não suposto. E um `.rlib` é dependência externa, que a regra do projeto proíbe. O caminho que funciona está descrito em `docs/MULTILINK.md`: falar com ele por **protocolo**, e não por link |
| ☑️ | 96 | **Registro apagado fisicamente vai para o `.trash` antes de sair do `.reg`** | e o disco **confirma** antes de o slot ser liberado. Guarda o *payload* byte a byte **mais o conteúdo dos anexos** — com ponteiro, a foto voltaria sendo a de outra linha, porque o bloco do `.bin` é liberado na exclusão. Só quem tem `administrar` lê |
| ☑️ | 97 | **Coluna `SOFTDELETED` em todas as tabelas** | entra sozinha na criação, no fim da lista para não deslocar as colunas do usuário. Marcar tira a linha das listas e ela continua inteira no `.reg`; `restaurar` desfaz. Esquema `PSCH` v3 → v4, e tabela v3 continua abrindo |
| ☑️ | 98 | **`.reason` com UUID, data, hora, motivo e quem excluiu** | UUID v7 do próprio evento, e a identidade da linha em texto — «rowid 4173» não diz nada seis meses depois. Sobrevive à linha: o expurgo é registrado antes de o dado sair. Só `administrar` |
| ☑️ | 99 | **Motivo de exclusão obrigatório, marcado na criação da tabela** | caixa na tela de Nova tabela; marcada, o motor recusa qualquer exclusão sem frase escrita, **antes** de qualquer gravação |
| ☑️ | 100 | **Botões e combos no ambiente** | diálogo de exclusão com os dois modos e o campo do motivo (não um `confirm()`, que só sabe perguntar sim ou não); par «ativas / excluídas» na grade com botão de restaurar; telas de Lixeira e de Motivos no menu Tabelas e na barra |
| ◐ | 101 | **Cifrar e compactar `.log`, `.trash` e `.reason`** | **não feito, e o motivo é técnico**: compactar arquivo *append-only* exige rotacionar e reescrever, e cifrar exige uma cifra de bloco que o projeto não tem — há SHA-256, HMAC e PBKDF2 escritos aqui, nenhum AES. Hoje a proteção é a permissão: as três operações exigem `administrar`, e no disco vale a permissão do sistema de arquivos |
| ☑️ | 102 | **Paginação de Big Table por cursor (keyset)** | `depois`/`antes` no `varrer`, cursor bidirecional na grade, `pular` como compatibilidade. E o defeito que estava embaixo: o `varrer` lia a **tabela inteira com os anexos** para devolver 200 linhas — 3.176 ms numa tabela de 800 mil. Pelo cursor, não mensurável |
| ☑️ | 103 | **Campo `rownum` sequencial e automático em todas as tabelas** | coluna de sistema, o motor preenche, nunca reaproveita número, alterar não renumera. `rowid_do_rownum` acha por bissecção — 20 leituras num milhão, sem índice |
| ☑️ | 104 | **Partição alfanumérica: `Clientes_A.reg` … `Clientes_Outros.reg`** | 37 volumes fixos, o rowid sai de `(balde−1) × rpa + slot` — a inversa exata da conta de sempre, então nenhum caminho de leitura mudou. A ordem de digitação sai do rowid e vai para o `rownum` |
| ☑️ | 105 | **Arquivo `.pag` com a instrução da partição em JSON** | descritor **gerado**, com a conta do endereço por extenso; o motor nunca o lê. Segunda cópia seria segunda verdade |
| ◐ | 106 | **Integrar o MULTILINK — segunda análise, agora com os fontes** | o motivo anterior caiu: os fontes vieram. O novo é maior e medido: o `Cargo.lock` resolve **596 pacotes, 14 locais → 582 crates externas**, e cinco são obrigatórias mesmo sem nenhuma *feature* (`serde`, `serde_json`, `log`, `tokio`, `ml-driver-api`). Linkar traria um runtime assíncrono inteiro para dentro do `phxsqld`. Há um caminho novo que os fontes abrem: os `ml-driver-*-ffi` são `cdylib` com ABI C limpa, e ABI C se chama da `std` sem crate nenhuma — mas põe código proprietário com licença por máquina dentro do processo do banco. O caminho recomendado continua sendo **por protocolo**, agora como executável separado; `docs/MULTILINK.md` |
| ☐ | 107 | **Salto para uma página específica** | o cursor sabe ir e voltar; ir direto para «a página 500» exigiria contar a tabela, que é o que foi removido. Quem precisa de ponto certo usa `rownum` com a bissecção |
| ☑️ | 67 | **Botão e menu Tabelas** para gerir as tabelas do banco: nova, estrutura, editar conteúdo, partições, duplicar, reparar tabela, reparar índice e excluir — e **Gestão de transações** no menu de ferramentas | as oito operações funcionam de ponta a ponta; três delas (`criar_tabela`, `duplicar_tabela`, `excluir_tabela`) nasceram aqui, e `criar_schema` — prometido na documentação e nunca despachado — junto |

**84 feitos · 5 parciais · 6 planejados**, de 95 pedidos.

Fora do que você pediu, entraram por medição: o CRC slice-by-8, o `descer` sem
reler a folha, a conferência de unicidade sem descida dupla, e dezoito
correções de defeito — três delas de perda silenciosa de dado, e quatro
achadas **rodando** o que tinha acabado de ser escrito (o percentual de disco
que dividia pelo total, o assunto de e-mail com acento cru no cabeçalho, o
decimal que a grade arredondava, e o `criar_tabela` que gravava
`filial.clientes.reg` na raiz do banco e devolvia uma tabela que nenhuma outra
operação conseguia abrir).

---

## O detalhe de cada parcial e de cada planejado

## 2. Parcial

Existe, funciona no que promete, mas **não faz tudo** o que o pedido queria.
Cada linha diz exatamente onde para.

As três primeiras são os ◐ da tabela lá em cima. A que trata de chave
estrangeira **não é um pedido seu** — é um buraco achado na revisão, dentro de
um pedido marcado feito, e fica aqui para não sumir de vista.

| # | O que você pediu | O que existe | O que falta |
|---|---|---|---|
| 1 | **Replicação como a do MySQL(R)**, com porta de acesso, de envio e de retorno | as três portas entram no `config.json` e são validadas — duas no mesmo endereço não sobem. O desenho está na seção 9 do dossiê e em `docs/REPLICACAO.md`. O `.log` **é** o binlog | o `.log` **v2 com imagem da linha**. Hoje o diário registra que houve alteração, não o que a linha virou — sem isso a réplica não tem o que aplicar. O servidor avisa alto no arranque que as portas são configuração, não serviço |
| 2 | **Subir o PhxSql no GitHub** | está em `adrianoboller/adrianoboller`, na branch `claude/capacidades-disponiveis-y6auxh`, com histórico completo | repositório **próprio**: `create_repository` responde `403 Resource not accessible by integration`. Não é escolha minha nem defeito do código — a credencial desta sessão só alcança esse repositório. Destravar depende de você criar o repositório e dar acesso |
| 4 | **DbLink para PostgreSQL(R) e outros** | o cadastro reconhece o motor, guarda a definição e a tela mostra «sem cliente» em vez de fingir que conecta | o **cliente**. O caminho é curto: a autenticação `scram-sha-256` do PostgreSQL(R) se monta com SHA-256, HMAC e PBKDF2, que já estão escritos aqui, e o protocolo de consulta simples (`Q` → `T`/`D`/`C`) é menor que o do MySQL(R) |
| 3 | **Chave estrangeira** com CASCADE / RESTRICT / SET NULL | declarada, validada, gravada no cabeçalho do `.reg`, sobrevive a fechar e abrir, e aparece na aba Estrutura | **não é aplicada**. Nenhuma gravação consulta a chave: `Restringir` e `Cascata` são intenção guardada, não comportamento. Estava marcada «pronto» no README e no dossiê — corrigido nesta revisão |

## 3. Planejado

Pedido e **não começado**. Não estão pela metade: não têm código nenhum.

| # | O que você pediu | Por que ainda não | O que destrava |
|---|---|---|---|
| 1 | **Jobs de execução** | é o mais barato dos três; tem tela apagada em *Gerir banco* dizendo o que falta | o agendador do backup (`hora_de_rodar`, `minuto_do_dia`, o laço que acorda de minuto em minuto) já é exatamente o desenho. Falta generalizar de «rodar backup» para «rodar operação nomeada». Uma rodada |
| 2 | **Triggers** | tem tela apagada em *Gerir banco*; onde disparar já existe — `inserir`, `atualizar` e `excluir` são os três pontos, e já escrevem no `.log` | falta decidir **em que linguagem o gatilho é escrito**, e essa escolha é sua. Sem camada SQL não há `BEGIN … END` para hospedar |
| 3 | **Stored procedures** | mesmo bloqueio, maior | procedimento é código guardado, e código guardado precisa de executor. Ou uma linguagem própria pequena, ou esperar a camada SQL |
| 4 | **Parar e subir o serviço de dados pela interface**, trocando a porta | mexe no coração do servidor | o `accept` bloqueia. Derrubar a porta sem derrubar o processo exige acordar o laço — conectar no próprio endereço para o `accept` retornar e então conferir um sinalizador. Melhor inteiro do que pela metade |
| 5 | **Servidor MCP** | não depende de nada; é fila | o protocolo já é JSON por linha. O MCP é tradução de vocabulário sobre o que existe |
| 6 | **Camada SQL** | é a peça de que três outras dependem | tabela virtual do rusqlite atrás de um recurso do Cargo — dá SQL completo sem escrever parser. Repare que **fura a regra de zero dependências**, e por isso fica atrás de um `feature`: quem não liga, compila sem |
| 7 | **Driver ODBC de saída** | depende de (6) | driver ODBC que não fala SQL não serve para o que você quer ligar nele |
| 8 | **Cliente ODBC e OLE DB** | depende de (7) | — |
| 9 | **Integração no FraseSQL** como `engine = "phxsql"` | depende de (8) | — |
| 10 | Compactação | o formato já prevê e **mede** o espaço morto | falta o comando. O reindex já cobre a parte do índice |
| 11 | Transações | tem **tela** (Ferramentas → Gestão de transações), e a tela diz o que existe e o que não existe em vez de fingir | hoje a inserção desfaz o que gravou se um índice falhar, e a trava única serializa as escritas — mas não há journal com a imagem anterior da linha, nem identificador de transação na sessão, nem `commit`/`rollback` de várias operações. É o que o uso como livro-razão exigiria primeiro |
| 12 | Concorrência fina | — | uma trava única serializa todo acesso a dados |
| 13 | Modo exclusivo | tem tela apagada em *Gerir banco* | reservar uma tabela por um período. Hoje a trava única já serializa as escritas, mas não há como RESERVAR — depende da trava por tabela, que é o mesmo trabalho da concorrência fina |
| 14 | Restaurar backup | o *Backup e restauração* mostra o item apagado | copiar de volta é mais do que copiar: é decidir o que fazer com o que está lá. Sobrescrever um database em uso, com a trava tomada, precisa de um desenho — parar, restaurar ao lado e trocar, ou restaurar com outro nome |
| 15 | Editar `config.json` e usuários pela web | as telas leem e dizem qual campo mexer | gravar credencial e política por HTTP precisa de desenho próprio: quem pode, o que fica no log, e a senha nunca em claro em ponto nenhum do caminho |
| 16 | TLS | — | o tráfego depende de túnel. A credencial já não vai em claro quando se usa desafio-resposta; os dados, sim |

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
