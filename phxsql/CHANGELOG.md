# Changelog

Tudo que mudou no PhxSql, do mais novo para o mais antigo.

Formato: cada versão traz **Corrigido** primeiro — defeito é o que o leitor
precisa achar rápido —, depois **Adicionado**, **Mudado** e **Sabido**. A
seção *Sabido* lista o que ainda não funciona, para ninguém descobrir sozinho.

Os números são **medidos**, nunca estimados.

---

## Não lançado — A rodada dos gaps de segurança: quatro decisões do dono, quatro frentes, e um bloqueio que a medição achou

Rodada de 18/09/2026, aberta pela ordem «fazer os gaps» e fechada com quatro
frentes de código, dois pareceres e uma ordem nova do dono no meio dela — «a
comunicação deve obrigatoriamente ser cifrada».

O que a rodada mais ensinou não foi nenhum dos consertos: foi **o campo que
anuncia proteção que o canal ao lado não presta**. Ligar `cifra_fio.exigir`
recusa o texto claro na porta de dados, e no mesmo servidor, no mesmo
instante, a tela devolve `200 OK` para um login com senha em texto puro.

### Corrigido

- **A comunicação passou a ser obrigatoriamente cifrada, e os clientes desta
  casa aprenderam o aperto junto** (ordem do dono de 18/09; pedidos 366 e
  370). `cifra_fio.exigir` nasce `true`, com `"exigir": false` como escape
  escrito. As três portas HTTP recusam o texto claro pelo **mesmo** portão de
  rede que já era delas — e o `atender_http`, que carregava uma cópia própria
  do portão, passou a chamá-lo. Porta de família desconhecida falha
  **fechado**. O escape das portas HTTP é `"atras_de_proxy": true`, que é o
  meio escolhido pelo dono: TLS terminado no proxy reverso, motor sem
  dependência nenhuma.
  E o `phxsql-cmd` — o console — foi **consertado, não o teste dele**: oito de
  nove provas ficaram vermelhas com a virada, e a resposta foi ensinar o
  aperto ao produto, pelo mesmo `Cliente::cifrar` da réplica e do cluster.
  Quatro testes mudaram de **significado**, com o nome novo escrito ao lado e
  nenhum apagado.
- **O campo `encryption_exigida` parou de anunciar proteção que o canal ao
  lado não presta** (pedido 370). Passou a valer `exigir && entrada ==
  Dados`: numa porta HTTP é `false`, porque quem cifra ali é o TLS do proxy, e
  **proteção que o servidor não consegue conferir não se anuncia**. É a
  família do `recursos.cache_paginas`.
- **A receita do driver ODBC nasce cifrada** (pedido 373), com `CIFRA=0` como
  escape escrito. O padrão mora no `impl Default` e **não** no analisador,
  porque o `SQLConnect` com `host:porta/database` não passa pelo analisador —
  padrão só ali deixaria esse irmão falando claro, calado. O interruptor
  passou de dois para três estados: com o padrão ligado, «valor não
  reconhecido → false» deixa de ser inofensivo e vira rebaixamento por dedo
  errado. Provado contra o sistema operacional: `.so` por `dlopen`, soquete e
  `phxsqld` reais, **89 conferências**, e um `"op":"cifrar"` no `acessos.log`
  sem ninguém ter escrito `CIFRA=`.
- **A trilha `.lgpd` de coluna externa parou de mentir sobre o valor antigo**
  (pedido 367). A causa era um sentinela com dois significados: `Value::Null`
  de «coluna vazia» e de «não carreguei» são o mesmo byte e fatos opostos, e a
  trilha herdou o sentinela dos índices sem herdar o significado. Três
  mentiras morreram: valor antigo em branco, apagar sem gerar registro, e
  salvar sem tocar em nada gerando **4.000** registros onde eram 2.000.
  Custo medido, 7 rodadas com faixa min–max: ler o bloco velho custa
  **0,59 µs/KiB** contra os **0,99 µs/KiB** que o `atualizar` já pagava na
  mesma coluna — no caso comum o conserto saiu **mais barato que o defeito**.
- **A partição por posição sobre coluna marcada recusa nas duas portas**
  (pedido 358). O rowid sai do volume, então dividi-lo devolve o volume — e o
  volume revela o **primeiro caractere** da coluna: uma classe entre 37, de
  graça, em toda leitura, inclusive a quem tem aquela coluna negada pelo
  direito por coluna. A recusa entra na declaração, no `CREATE TABLE` e na
  marcação posterior, num funil só.

- **A marca `.tx` do COMMIT gravava a linha inteira em claro, em `0644`, fora
  da cifra** (pedido 354). Passa a ser selada **por operação** — não a marca
  inteira, condição do DBA: marca inteira selada vira tudo-ou-nada na
  recuperação. Nasce `0600`, e o `ler_marca` ganha a **terceira resposta**:
  cifrada e sem chave **PARA e não apaga**. Sem ela o selo trocaria
  confidencialidade por durabilidade.
- **A restauração PITR pagava `fsync` com a trava global na mão** (pedido
  252). A reaplicação passou para o palco, antes de o database entrar na
  raiz. A catraca `alcancam-fsync` caiu de **25 para 24**, por mérito — teto
  não subiu e seção nenhuma foi isentada. E a garantia **melhorou**: o
  primeiro tamanho do `.reg` restaurado que um terceiro consegue ler passou
  de **570 B** para **17.912 B**, o final, em 10 de 10 corridas.
- **O `Schema::com_coluna` perdia os índices de texto** (pedido 353), e o
  irmão no `table.rs` junto.
- **O Profiler decidia o sigilo pela lista declarada e não pelo disco**
  (pedido 356). Custo medido: **+2,17 µs** por pedido observado que grava
  arquivo.

### Adicionado

- **Modo ledger deixa de aceitar coluna marcada como dado pessoal** (pedido
  355). O `hash` de cada bloco é um SHA-256 **sem sal** do conteúdo em claro,
  numa coluna que não é marcada e por isso não é cifrada — oráculo de
  confirmação para CPF (~10⁹ candidatos). A recusa entra na **declaração**, e
  **não desfaz cadeia que já exista**: ali o oráculo já queimou.
- **A porta web nasce em `127.0.0.1`** (pedido 366), abrir para fora é escolha
  escrita, e quem já tem proxy declara `"atras_de_proxy": true`. O TLS do
  navegador é terminado por proxy reverso — o motor continua **zero
  dependências**, e com isso o choque «TLS na conexão» que o `CLAUDE.md`
  registrava como vivo deixou de estar vivo.
- **O arranque passa a dizer o alcance da cifra em voz alta**: o `exigir` vale
  só para a porta de dados, as três portas HTTP continuam em claro com o mesmo
  token e o mesmo login, e ele **também não alcança o que o servidor conecta**
  — réplica, cluster e web→remoto têm interruptores próprios, os três nascendo
  desligados.
- **O portão dos geradores passou de 22 para 28 entradas** (pedido 360), e
  achou quatro números errados no `SEGURANCA.md` de uma vez.
- **A gravação do `--json` do provador de guardas ganhou trava** (pedido 361),
  cobrindo o read-modify-write inteiro.

### Sabido

- ~~**`cifra_fio.exigir` é lido em UM lugar que decide alguma coisa**, e por
  isso não vira padrão de fábrica~~ — **fechado na mesma rodada** (pedido
  370): as portas HTTP passaram a recusar pelo portão delas, o
  `encryption_exigida` passou a dizer a verdade por canal, e só então o padrão
  virou. A previsão dos «62 testes derrubados» era da ordem certa: caíram os
  do servidor e mais **oito do console**, que não estavam na conta porque
  `phxsql-cmd` não era suspeito de ninguém.
- **Ligar o `exigir` não alcança a SAÍDA, e isso é decisão do dono** (a outra
  metade do 366). `replicacao.origens[].cifra`, `cluster.cifra` e
  `web.servidores[].cifra` nascem **desligados**: um source de fábrica recusa
  uma réplica de fábrica, e a suíte fica verde do mesmo jeito, porque as
  bancadas escrevem o escape dos dois lados. Por ora há só um **aviso de
  arranque** nomeando cada saída em claro configurada. Virar também os três
  tem custo próprio — réplica nova deixa de falar com source anterior ao
  aperto — e por isso não entrou de carona.
- **Os 63 scripts de `bancada/` que abrem soquete ainda não escrevem o
  escape** e vão bater na recusa. Medido: dos 69 que montam config/token, 63
  abrem soquete e só 4 mencionam `cifra_fio`. Enquanto não entrar, **corrida
  de bancada perde número** — e página de desempenho sem bancada é painel
  velho anunciando sucesso.
- **Ligar o `exigir` quebra todo DbLink → PhxSql**, que não tem como pedir o
  túnel, e não há escape por ligação (pedido 371). E a frase «a `std` não traz
  TLS» que o `phx.rs` carrega **nasceu falsa**: o túnel é oito dias mais velho
  que ela.
- ~~**A trilha `.lgpd` de coluna externa marcada mente nos três sentidos**~~ —
  **fechado** (pedido 367), com o custo medido e um atalho recusado com
  número.
- **Não existe expurgo de `.lgpd`** (pedido 368): a trilha cresce para sempre,
  com a chave primária em texto em cada registro. O caminho existe sem mudar
  formato — expurgo por **volume** —, e falta só o prazo, que não é técnico.
- ~~**O `rowid` é o balde, e o balde é o primeiro caractere** (pedido 358)~~ —
  **fechado na declaração**, nas duas portas do protocolo. Segue **aberto pela
  API Rust e pelo FFI** (pedido 376), de propósito: é o que torna possível
  escrever o teste do comportamento velho, e é decisão a tomar com o DBA.
- **Os arquivos do `phxsql-store` continuam nascendo `0644`** — 12 abridores
  fora de teste, e o único `0o600` do crate aperta **depois** de criar. O
  `.reg` cifrado nasce aberto para a máquina.


## Não lançado — A matriz do comparativo passa a dizer contra o quê (pedido 335, metade 1)

Rodada de 17/09/2026. Um parecer técnico de fora achou em duas linhas o furo
que seis revisões desta casa não acharam: **a matriz do comparativo publicava
o veredito e não publicava contra o quê**. Sem ambiente ninguém refaz a
corrida; sem a saída crua ninguém confere a célula; sem caso negativo, `tem`
não distingue «o motor entendeu» de «o motor ignorou o que não entendeu».

### Adicionado

- **Os seis campos de evidência no `resultados.json`, nenhum digitado.** O
  `bancada/comparativo/medir.py` grava agora `ambiente.commit` e `branch`;
  `ambiente.arvore`; `ambiente.sha256_phxsqld` do binário que **respondeu**;
  `ambiente.uname`, `cpus` e `memoria_total`; a
  `ambiente.configuracao_do_phxsqld` **tarjada**; a `linhas[].cru` com
  comando, código de saída e os dois canais por motor; e a
  `linhas[].negativo`. Medido nesta corrida: `Linux 6.18.44-fc-v33 x86_64`,
  4 CPUs, 16.482.220 kB, `phxsqld` `sha256 bab749c9684b…`. A `cru` substitui
  o recorte de **90 caracteres**, que cabia na tabela e não cabia numa
  auditoria.
- **O caso NEGATIVO por item — a metade da prova real que faltava nesta
  bancada.** Cada item ganhou um gêmeo que **tem de ser recusado**, em dois
  formatos: por **efeito** (a linha que viola o `CHECK`, a escrita na coluna
  calculada, a chave repetida **sem** o `ON CONFLICT`) e por **resolução**
  (nomear dentro do construto algo que não existe). Ele roda só onde a
  positiva deu `tem`, porque é o veredito **afirmativo** que pode ser falso.
  Primeira corrida: **43** gêmeos recusaram como devia, **32** sem caso,
  **1** aceitou o que devia recusar. As recusas do nosso motor são
  específicas e não genéricas — `[SP000020] chave duplicada: indice unico
  porId ja tem essa chave` no upsert, e `[SP000018] … depois de ISOLATION
  LEVEL` no nível inventado.
- **Portão novo, provado nos dois sentidos: `config-com-segredo`.** Reposto o
  defeito, a tarja sai e o medidor tem de **parar** com `SEGREDO NO
  ARTEFATO` — «senha nunca em texto puro, nem em arquivo» alcança artefato
  versionado, e o `config.json` da oficina carrega `token` e `senha_hash`. O
  conferidor roda sobre o JSON **já serializado**, não sobre o dicionário:
  o que vaza é o que se grava, e tarja aplicada no ramo errado passaria por
  uma conferência feita no ramo certo. São **4** portões provados nos dois
  sentidos, e a tarja deixa a **chave visível** com o valor omitido — apagar
  a chave esconderia que a corrida rodou com token.
- **O `COMPARATIVO.md` publica o ambiente e a conta dos gêmeos**, e **diz que
  fez menos** quando a corrida é antiga e não tem `ambiente`, em vez de omitir
  o bloco em silêncio.

### Corrigido

- **A premissa do gêmeo por resolução morreu medida, e o achado FICA.** Eu
  supus que ela valesse em todo motor. O SQLite(R) **aceitou** `CREATE VIEW
  v_neg AS SELECT nao_existe FROM c`, porque resolve o corpo da visão na
  **consulta** e não na criação; MySQL(R) («Unknown column 'nao_existe' in
  'field list'») e PostgreSQL(R) («column "nao_existe" does not exist»)
  recusaram. Não quer dizer que o SQLite(R) não tenha visão: quer dizer que
  **nele a aceitação da criação não prova o corpo**. Não consertei no escuro,
  porque o gêmeo «óbvio» (`SELECT * FROM v_c WHERE nao_existe = 1`) recusaria
  **também** se o `CREATE VIEW` fosse um nada-a-fazer — aí a recusa seria «no
  such table» —, e *recusa pelo motivo errado é a forma mais barata de um
  controle negativo mentir a favor*.
- **O campo `arvore` nunca poderia dizer `limpa`.** O `resultados.json` e o
  `COMPARATIVO.md` são **saídas** da própria corrida e sujam a árvore ao serem
  escritos: a primeira corrida disse `SUJA: 3` e a segunda `SUJA: 7`, e o
  leitor não tinha como separar «a fonte divergiu do commit» de «a corrida
  gravou o que era o trabalho dela gravar». Campo com um único valor possível
  não ensina nada. Agora as saídas saem da conta, **com o nome**, e os
  arquivos de **entrada** divergentes vão **nomeados** em vez de contados —
  lista curta se lê, contagem não.
- **O leitor do `git status --porcelain` comia o `p` da primeira linha.** O
  formato é `XY<espaço>caminho` e o arquivo só modificado no disco sai como
  ` M caminho`; o `.strip()` do ajudante tirava esse espaço **na primeira
  linha**, e o recorte `[3:]` passava a cortar um caractere a mais. Saiu
  publicado `hxsql/bancada/comparativo/LEIA-ME.md`. **E a minha conferência
  do leitor passou por engano**: eu escrevi a entrada à mão, com o espaço no
  lugar, que é a entrada que o chamador nunca produz. Reprovado agora contra
  a saída **real** do comando, nos dois sentidos.
- **A citação de bloco quebrava da segunda linha em diante.** `p("> …")`
  prefixa só a primeira, e o `textwrap` quebra depois — o resto do parágrafo
  saía **fora** da citação. Não aparece lendo o gerador; aparece lendo o
  markdown que ele escreveu. Nasce o `Texto.cita()`, com `>` em toda linha e
  a lista dentro da **mesma** citação.
- **Colisão de nome dentro do `por_phxsql`**: já existia ali um `cru` querendo
  dizer «o valor **cru**, sem `lower()`», e ele apagava o dicionário das
  saídas cruas. Compila e some — o erro saiu como `KeyError: 'view'` no
  `main`, 350 linhas longe da causa. O dicionário novo passou a se chamar
  `cruas`, com o motivo escrito no lugar.

### Sabido

- **A catraca continua de fora, e é metade 2 do pedido 335**: nada reprova
  ainda a publicação quando a prosa do dossiê contradiz estas células. A
  escolha entre fonte única interpolada e catraca que só detecta está na mesa
  do dono; gravar a evidência vale nas duas formas.
- **Duas sondas de código seguem sem gêmeo, e isso aparece como LACUNA** em
  vez de sumir da conta: `trava_por_linha` e `tls_no_transporte` saem de
  leitura de fonte, e leitura de fonte não tem gêmeo que se recuse.
- **`flock(1)` não é reentrante**, e a lei «todo `cargo` sob `flock`» se
  aplica **no ponto que invoca o `cargo`, e em um ponto só**: chamar o medidor
  sob um segundo `flock` do mesmo caminho travou a corrida na primeira linha,
  com o pai segurando a trava e esperando o filho. O aviso ficou na função
  que trava. `docs/cognicao/cognicao_flock-nao-e-reentrante-e-eu-apliquei-a-lei-a-quem-ja-a-cumpria_20260917_1912.md`

---

## Não lançado — As pétreas ganham guarda: vetor, portões, senha — e o `Debug` que vazava

Rodada da noite de 16/09 (`b6f55ee` … `f8b6c92`). Os números abaixo são os
das mensagens de commit, que são medidos; onde um número do briefing saiu
errado, está dito qual.

### Corrigido

- **Nove structs vazavam segredo no `Debug` derivado — e a guarda já existia**
  (`74de67e`, pedido 270). A varredura de `crates/` achou **9 estruturas, 14
  campos, 3 crates**: `Config`, `Origem`, `Cluster`, `Email`, `Rest`,
  `Usuario` e `dblink::Definicao` no servidor, `usuario::Comando` no SQL,
  `Receita` no ODBC. Um `{:?}` no `Config` despeja **oito** segredos; um
  `dbg!` no `Registro` do DbLink despeja a credencial de todas as ligações.
  O catálogo tinha `debug-da-cifra-mostra-a-senha` desde a frente G-CRIPTO —
  a guarda travou a **estrutura**, não a **lei**. Conserto: `impl Debug` à
  mão que **desestrutura sem `..`**, então campo novo para de compilar em vez
  de entrar calado. Prova nos dois sentidos: `derive` de volta (241
  inserções, 0 remoções) → as **6** provas novas caem nomeando o segredo;
  conserto → passam, `clippy` zero avisos, **2.400** testes verdes. As três
  linhas `.field("senha", &"(oculta)")` novas deixaram ambíguo o `trecho` da
  guarda velha e `TETO_TRECHO_AMBIGUO` acusou — alongado em uma linha.
  `docs/SEGURANCA.md` §16.
- **A tabela publicada das guardas mentia o tamanho** (`1e3e7e1`, pedido
  269). `docs/TESTES.md` dizia «143 guardas» sobre um catálogo que já tinha
  160 (hoje **170**); agora diz **143 das 170** e **nomeia as 27** que a
  corrida publicada nunca julgou, sob um cabeçalho que não é linha de êxito.
  O `tabela-no-testes.py` recusa pela **cobertura**, não pelo tamanho —
  medido contra o código de então, um `--json` de 9 encolhia a tabela de 143
  para 9, escondia 151 e publicava «332 s de mutação» onde a bateria custa
  3.374.
- **O R19 estava escrito ao contrário** (`23b0cbd`, pedido 264). Dizia «a
  contagem de dívida sobe sozinha»; `git log -S` devolve **um** commit
  (`f64b822`) — as 19 marcas nasceram juntas e a série tem um ponto. O risco
  medido é o inverso: a marca **sumir** de arquivos com **197 commits em 14
  dias**, com a contagem melhorando quando ela some. `docs/RISCOS.md`.
- **Quatro inventários digitados que envelheceram**: `docs/CATRACAS.md`
  dizia «as duas do `trecho-vivo.py`» (eram cinco, são **seis**);
  `docs/NUMEROS.md` publicava «77 guardas» de 03/09 citando
  `docs/TESTES.md:720` — citar a **linha** deixou o número parado treze
  dias, hoje aponta para as marcas —, e depois «143 das 169» (são 170); a
  linha do 264 em `docs/PENDENCIAS.md` dizia «4 citam pedido do dono» e são
  **5** — o #268 entrou pelo `config.rs` na mesma tarde.
- **Três números de briefing, medidos e corrigidos por quem os cumpriu**:
  «11 testes de vetor» são 11 **vetores** — o `phxsql-core` tem **28 funções
  de teste contra vetor publicado, em 9 normas** (`b6f55ee`); «senha nunca em
  texto puro: 0 entradas e 11 testes» eram **2 entradas e 40 provas**,
  contadas pela asserção e não pelo nome (`1e3e7e1`); e o campo do DbLink é
  `token`, não `token_remoto` (cognição do 270). Briefing de orquestrador
  também é número citado.
- **`replica::ligar` caía em `TcpStream::connect` sem prazo** (`49a3af7`,
  pedido 282, parcial). `Cliente::conectar` virou casca sobre
  `conectar_com_prazo` (`PRAZO_DE_CONEXAO = 10 s`, `replica.rs:456`) —
  alcança também o dblink para outro PhxSql e o console, achados pela própria
  compilação. Prova contra o sistema operacional: fila de `accept` cheia no
  loopback pendura o `connect` de forma determinística. Fica de fora:
  classificar o erro sem o texto cru do SO e contar host solto como
  violação leve.
- **`replicar` com `"max":0` lia o diário inteiro com imagens sob a trava
  global; o teto de 16 MiB cortava a resposta, não a leitura** (`49a3af7`,
  pedidos 279 e 303-parcial). `max` ausente/zero/negativo passa a valer o
  padrão (`LOTE_PADRAO_DE_REPLICACAO = 500`), acima de 5.000 vale o teto
  (`TETO_DE_EVENTOS_POR_LOTE`), e o teto de bytes desceu para
  `Log::percorrer` (`log.rs:655`), que decide pelo cabeçalho antes de
  alocar — primeiro evento sempre entra. Testes:
  `log::tests::percorrer_com_limite_zero_nao_le_tudo`,
  `o_primeiro_evento_entra_sempre_e_o_teto_so_conta_imagem`,
  `servidor::testes_do_lote_de_replicacao::{max_zero_ou_negativo_vale_o_padrao_e_o_absurdo_vale_o_teto,
  replicar_com_max_zero_serve_o_lote_padrao_e_nao_o_diario_inteiro,
  o_lote_servido_corta_por_bytes_e_o_primeiro_evento_entra_sempre,
  diario_sem_rowid_devolve_a_cauda_com_o_total_do_diario_inteiro}`. Os
  irmãos `op_diario` e `absorver_diario_local` deixaram de carregar o diário
  inteiro.
- **`cluster_pulso` não estava em `OPS_DE_REPLICACAO`, e época/posição do
  pulso não tinham teto** (`49a3af7`, pedido 278, parcial). Entrou na lista
  (`servidor.rs:320`), e `registrar` ignora pulso com época acima de
  `maior_epoca_vista + FOLGA_DE_EPOCA` (`cluster.rs:161`, **1.000.000**) ou
  posição fora do inteiro exato. Testes:
  `cluster::testes::{um_pulso_de_epoca_absurda_nao_destrona,
  pulso_dentro_da_folga_ainda_conta_e_espelha_a_epoca}`,
  `servidor::testes_papel::o_pulso_do_cluster_passa_pela_lista_de_replicas`.
  Fica de fora: a identidade do nó pela chave do fio (`known_hosts`).
- **O crivo do portão 2b-bis só rodava com `somente_leitura`; num source
  aberto, `aplicar` pela rede desligava FK/CHECK/cascata e matava o pai com
  filhos** (`49a3af7`, pedido 280). O crivo passa a valer independentemente
  do `somente_leitura`; mensagem nova na fábrica,
  `erro.aplicar_fora_de_replica`. Medido antes de mexer: zero chamadores
  legítimos em source/isolado. Testes:
  `servidor::testes_papel::{aplicar_num_source_aberto_tambem_e_recusado,
  aplicar_pela_rede_num_source_nao_mata_o_pai_com_filhos,
  replica_destrancada_continua_aceitando_o_diario_do_source}`.
- **`alcancar_tabela` devolvia `Ok(0)` em silêncio quando o source apagava e
  recriava a tabela** (`49a3af7`, pedido 295). A réplica passa a conferir o
  evento `posição-1` do source contra o seu (`diario_local_continua`, irmã
  de `diario_vivo_continua` do PITR); rompida, grava a recusa em
  `replicacao_estado.origens.*.recusas` e a tabela sai da rodada sem
  derrubar as outras. Teste pelo soquete:
  `crates/phxsql-server/tests/continuidade-da-replica.rs`. Não entrou no
  bidirecional — decisão do dono.
- **`numerar_linha` consumia o contador do `rownum` ANTES da sequência, do
  `CHECK`, da unicidade e da coluna obrigatória — uma linha recusada queimava
  número e deixava buraco atrás dela** (`eeb9925`, pedido 291). Passou a
  **reservar** o número (a linha o carrega, para a chave poder indexá-lo) e
  só **consumir** — o contador andando de fato — depois da última guarda que
  pode recusar a linha; o irmão `atualizar_com_maes_opt` recebeu o mesmo
  tratamento. Não é retroativo. Testes:
  `paginacao::{linha_recusada_no_lote_nao_consome_rownum,
  insercao_recusada_nao_consome_rownum, lote_sem_recusa_numera_como_antes}`.
- **`replicar` numa tabela com coluna marcada não deixava rastro na trilha
  `.lgpd`** (`eeb9925`, pedido 285). Passou a gravar um acesso por chamada,
  critério `replicar desde=N ate=M`, `linhas` = eventos servidos; **+154
  bytes por lote** medidos. Testes:
  `testes_da_ficha_compartilhada::{replicar_numa_tabela_marcada_deixa_rastro_na_trilha,
  replicar_sem_coluna_marcada_nao_grava_trilha}`.
- **`"propagar": false` no escalonamento do cluster valia de qualquer
  cliente, e permitia dois masters graváveis sem partição de rede** (`eeb9925`,
  pedido 281). Passou a valer só como ordem interna (IP vazio) ou com a
  credencial do cluster vinda de um nó da lista viva (por IP ou por nome de
  host — irmão achado pela própria frente: a lista aceita host, e a trava só
  olhava IP). Cliente comum é recusado pela fábrica,
  `erro.escalonar_sem_propagar`.
- **`cluster_estado` entregava endereço, época e posição de cada nó a quem só
  tinha `ler`** (`eeb9925`, pedido 283). `master`/`papel`/`epoca`/
  `escrita_liberada`/`degradado` continuam para `ler`; `nos[]` — a lista com
  endereço e posição — passou a exigir `administrar`, saindo **ausente** da
  resposta (nunca uma lista vazia) para quem não tem o direito.
- **`replicacao_testar` vazava o texto cru do sistema operacional, e não
  tinha prazo de conexão** (`49a3af7` deu o prazo; `eeb9925` fechou a
  classificação, pedido 282). O erro de rede vira uma de quatro chaves da
  fábrica (`erro.sonda_recusada/prazo/sem_rota/caiu`) sem texto do SO; host
  fora da configuração que não responde conta violação leve.
- **O carimbo do modo bidirecional vinha do outro lado sem teto — um par
  hostil ganhava todo conflito para sempre** (`eeb9925`, pedido 286). Carimbo
  além de `FOLGA_DO_CARIMBO_MS` (5 min) no futuro entra com o relógio local e
  é contado em `replicacao_estado.carimbos_do_futuro`, sem recusar o evento.
- **A op `config` publicava a lista de nós do arranque, não a viva** (pedido
  287) — **fechado por PROVA, não por conserto**: a injeção da lista viva já
  existia desde `a446c7a` (07/09/2026); faltavam o teste e o campo `tem_pino`
  na lista viva, que `eeb9925` completou.
- **`cluster_pulso` era oráculo de ids de nó** — «não está na lista» e «é
  este servidor» respondiam frases diferentes, e nenhuma contava violação
  leve (`eeb9925`, pedido 288). As duas passaram a responder a mesma
  mensagem (`erro.pulso_de_no_desconhecido`); a contagem de violação leve
  entra só com `seguranca.contar_pulso_desconhecido`, que **nasce
  desligado** (ligado de fábrica bloquearia o próprio nó novo durante um
  escalonamento a quente).
- **O `empilhar` montava o mapa da transação sob a trava de dados para
  apagá-lo na linha seguinte** (`20d2c59`, pedido 164). O caminho que
  EMPILHA abria a tabela pela porta de sempre — que monta a sobreposição
  percorrendo o conjunto de escrita **inteiro** da transação —, e chamava
  `ver_so_o_disco()` logo depois, jogando o mapa fora: O(pendentes) por
  operação, O(n²) por transação, com a trava global na mão. Nasce a porta
  `abrir_travada_sem_sobrepor`, e o irmão (`empilhar_atualizar_com_cascata`,
  fase 3) entrou junto. Medido com 1.600 escritas pendentes: **625,62 →
  40,62 µs/operação (15,4×)**, curva plana — e **8,6× é a razão de MIL
  pendentes** (335–341 → 39–40), que este arquivo publicou por engano ao lado
  do par de 1.600. Corrigido em 17/09/2026, junto com o irmão que ficou: a
  correção de madrugada alcançou o `DESEMPENHO.md` e o `PENDENCIAS.md` e
  **não** alcançou este arquivo, e foi daqui que a frente seguinte copiou o
  número errado. Catraca estrutural nova: **zero**
  chamadas de `ver_so_o_disco()` no servidor. `docs/DESEMPENHO.md` §25,
  `docs/PENDENCIAS.md` #164.
  **Fechado em 17/09/2026 com a medição final em máquina parada, e com a prova
  nos dois sentidos.** Duas corridas com o `quieta.Vigia` aprovando as duas — a
  limpa às 09:43, a do defeito reposto às 11:50, ambas guardadas em
  `bancada/concorrencia/corridas/`. O par completo de cinco pontos: **60,00 ·
  75,00 · 127,50 · 261,25 · 593,75 µs/op** com o defeito contra **40,00 · 40,00
  · 37,50 · 38,75 · 41,25** sem ele. E as duas razões que este arquivo já
  confundiu uma vez **são as duas reais**: 8,63× com mil operações na
  transação, **14,39×** com mil e seiscentas — o defeito era O(pendentes) por
  operação, então *razão sem o tamanho da transação ao lado não diz nada*. A
  medição carrega o próprio controle: o `op_inserir`, que o defeito não tocava,
  ficou em 50,00 µs (faixa 49,00..53,50) contra 52,50 (46,25..53,00), faixas que
  se cruzam. E é isso que torna o resultado NULO do §25.1 confiável — o mesmo
  instrumento que não vê o aparato do gatilho subir acima do próprio ruído em
  quatro leituras enxerga 14,39× quando há o que enxergar: *nulo medido não é
  cegueira*. `docs/DESEMPENHO.md` §25.4.
- **A tela vazia de DbLink nascia com os dois únicos botões mortos**
  (`6319396`, pedido 190). Um `return folha(...)` deixava as duas linhas de
  `onclick` seguintes inalcançáveis — era a primeira tela de quem ainda não
  tem ligação nenhuma, e ela não tinha saída.
- **Copiar o pivô como CSV morria calado** (`6319396`, pedido 190). Um
  `const txt` sombreava a função `txt()` da fábrica de idiomas dentro do
  mesmo bloco; a cópia acontecia e os três recados quebravam em «txt is not
  a function». Única ocorrência do tipo em toda a `ui/` — o conferidor de
  textos fora da fábrica não o vê, porque a chave *está* na fábrica; quem
  quebra é o escopo.
- **«Backup agora» e «Conferir backup» ficavam presos na tela de progresso
  para sempre** (`6319396`, pedido 190). O pedido de backup saía sem o
  `destino` obrigatório, a exceção subia sem tratamento na tela, e a folha
  nunca mudava — sem cópia e sem erro, as duas piores notícias juntas; o
  irmão `conferirBackup` tinha o mesmo defeito. As duas fichas de resultado
  também pediam campos que a resposta não tem (`segundos` em vez de `ms`;
  `conferidos`/`diferentes`/`faltando` em vez de
  `arquivos`/`bytes`/`divergencias`) e mostravam travessão onde havia
  número medido.

### Adicionado

- **Nove guardas de criptografia e portão** (`b6f55ee`): cinco de norma
  (`sha256-sem-somar-o-estado`, `sha256-com-o-tamanho-em-little-endian`,
  `hmac-com-a-chave-longa-truncada`, `pbkdf2-com-o-contador-de-bloco-parado`,
  `pbkdf2-sem-o-xor-acumulado`) e quatro de portão (`juntar-sem-portao`,
  `unir-sem-portao`, `diferencas-sem-portao`, `derivado-sem-portao`) —
  **9 provadas, 0 «não pegaram», 0 estragaram, 147,8 s** de mutação. A
  criptografia tinha **0** entradas; as portas dos fundos foram de 2 para
  **13 de 13** (5 por entrada própria, 8 pelo `derivado-sem-portao`, cujo
  ponto único de conferência foi medido com sonda: **8 caíram**). A coluna
  de quem **não** pega é a pétrea em tabela: auto-consistência, ida-e-volta e
  propriedade sobrevivem a um motor de criptografia quebrado, porque as três
  perguntam ao próprio motor. `docs/CATRACAS.md` §15.
- **Nove guardas da pétrea da senha** (`1e3e7e1`):
  `ficha-do-usuario-devolve-o-hash`, `senha-em-claro-no-cadastro`,
  `senha-velha-fica-no-arquivo`, `cifra-reserializa-a-senha`,
  `debug-da-cifra-mostra-a-senha`, `profiler-sem-a-senha-dentro-do-sql`,
  `comando-invalido-vira-texto-cru`, `trilha-sem-o-nome-de-segredo`,
  `trilha-so-olha-o-nome-da-coluna` — as nove provadas com a árvore limpa
  verde nos quatro binários (1.103 / 253 / 187 / 5); cobertura **14 de 40**.
  O raio de cada uma foi medido com sonda que lê o veredito de **todos** os
  testes do binário: o `Debug` da cifra imprimindo a senha derruba 1 dos 5 de
  um binário e **zero dos 1.103** do `--lib`. `docs/CATRACAS.md` §15.7.
- **A quinta régua do catálogo, `TETO_NAO_JULGADA_ESCONDIDA`** (`1e3e7e1`,
  pedido 269) — e ela **recusou a forma pedida, com número**: um teto sobre
  «ids fora da última corrida» nasceria em 17 e subiu para **26 em duas
  horas** sem defeito nenhum (a frente da senha escreveu nove guardas); os
  únicos caminhos de volta ao verde seriam o provador inteiro (3.374 s) ou
  subir o teto. O buraco virou inventário, sempre impresso; a dívida virou
  catraca sobre as não julgadas que a página **nem nomeia** — nasceu em 26 e
  desceu a **0** no mesmo passo, republicando a mesma corrida em 0,2 s. São
  **seis réguas: cinco tetos e um piso**, com `PISO_DAS_ENTRADAS` em
  **170**. Prova sem compilar: 8 e 16 casos de autoteste, mais oito mutações;
  **um caso passava com o defeito reposto** — a mutação achou, não a leitura.
  `docs/CATRACAS.md` §12.
- **Entrada `debug-da-ligacao-mostra-a-senha`** (`74de67e`): o defeito
  reposto não devolve o `derive` (não compilaria) — desfaz o conserto por
  dentro, em duas trocas. Com ela, **2 structs com catraca de 9**.
- **Parecer «uma catraca sobre `// DIVIDA:` protege ou estraga?»**
  (`docs/propostas/catraca-da-divida.md`, `23b0cbd`): das **19** marcas,
  **9 não se pagam por engenharia** — pétrea, recusa certa do motor, espera
  por demanda, decisão do dono. Recomendação: **piso que sobe** sobre marcas
  vivas mais baixas escritas, no molde do `PISO_DAS_ENTRADAS`. Duas
  hipóteses mortas: **0 de 25** linhas lidas ao acaso (peneira larga: 455
  linhas, 112 arquivos) eram dívida não marcada; peneira estreita, 27
  candidatos e **1** legítimo.
- **As duas páginas que leem o `git log` alcançam o commit que as carrega**
  (`44d3271`): a corrida que roda antes do commit de integração nunca vê o
  commit que a comitou; rodar depois e comitá-las sozinhas é o único ponto
  fixo, e o atraso publicado é **um** commit, por escolha.
- `docs/MODELOS.md` ganhou as três rodadas que o papel A não tinha
  registrado, com os papéis dispensados nomeados (`e89aa93`), e o time
  inteiro da rodada (`f8b6c92`).

### Mudado

- **Fecho da rodada** (`0a8b606`): **269 pedidos** (era 268), 8 planejados
  (era 7); **201.319** linhas de Rust e **66.200** de documentação; terceira
  linha da série histórica com `medido_em` de 22:52. A ordem que a noite
  ensinou duas vezes: `medir.py --gravar`, os dezenove geradores,
  `status-html.sh`, `extrair.py`, e o **portão por último** — os dois
  últimos medem o que os anteriores escrevem.
- `docs/PENDENCIAS.md`: 269 fechado; 270 nasce e fecha no mesmo commit;
  **271** (o conferidor de `derive(Debug)`) entra parcial; 264 e 268 ganham a
  nota da revisão. A contagem do rodapé sai do `pagina-dos-pedidos.py`.
- **Cluster com `replicas_autorizadas` preenchida agora precisa listar TODOS
  os nós** (`49a3af7`): desde que `cluster_pulso` entrou em
  `OPS_DE_REPLICACAO`, um nó que falte na lista de outro tem o próprio pulso
  barrado — cada nó pulsa para cada outro. Medido: nenhuma bancada desta
  casa preenche a lista, então o caso não foi exercitado antes deste
  conserto. `docs/CLUSTER.md` §2.2.
- **A réplica fiel passou a gravar o carimbo e a origem do source no próprio
  diário**, e não mais `agora_ms()`/`origem:0` (`49a3af7`,
  `servidor.rs:2831`) — o mesmo que o PITR e o bidirecional já faziam
  (parecer do DBA, §2.3). É o que faz a conferência de continuidade valer:
  comparar `posição-1` só funciona se os dois lados gravarem o mesmo
  carimbo/origem.
- **Quem já automatiza `cluster_no_remover`/`cluster_no_acrescentar` com
  `"propagar": false` de um script cliente passa a ser recusado** (`eeb9925`)
  — o campo virou ordem interna do cluster; quem precisa dele de fora agora
  precisa da credencial do cluster e de estar na lista viva. Quem nunca usou
  o campo não muda nada.
- **Quem lê `cluster_estado` com um usuário só de `ler` deixa de ver `nos[]`**
  (`eeb9925`) — endereço, posição e idade de pulso de cada nó agora exigem
  `administrar`. Ferramenta de monitoramento que dependia da lista com um
  usuário de leitura precisa passar a usar um usuário administrador.
- **O par bidirecional passa a tolerar um carimbo até 5 minutos no futuro
  sem sobrescrever calado para sempre** (`eeb9925`, `FOLGA_DO_CARIMBO_MS`) —
  quem já rodava com relógios fora de sincronia por mais que isso passa a ver
  o evento contado em `replicacao_estado.carimbos_do_futuro`, o que não
  existia antes.
- **Uma carga com `parar_no_erro:false` que já convivia com buracos no
  `rownum` deixa de gerá-los a partir de agora** (`eeb9925`) — quem tinha
  ferramenta própria contando com o número de ordem *de antes* do conserto
  para calcular quantas linhas foram recusadas precisa rever a conta: o
  `rownum` volta a ser contíguo em toda escrita nova (buracos já gravados
  antes do commit continuam no disco).

### Sabido

- **27 das 170** entradas do catálogo estão sem veredito da corrida
  publicada (16/09 15:25) — nomeadas na tabela do `TESTES.md`; fecham quando
  o provador inteiro rodar (≈ 3.374 s de mutação).
- **Não existe régua sobre `derive(Debug)` com segredo**: 2 structs com
  catraca de 9, as outras sete só com prova — a décima nasce derivando.
  Pedido 271, em construção.
- O bloco `catracas:` do `docs/QA-PDCA.md` mostra quatro tetos e piso 160 até
  o `docs/qa/medir.py --gravar` do fecho, que chama `cargo`.
- Pedido 268 (migração `Criptografar`/`Descriptografar`): parecer do papel C
  em curso.
- Pétreas ainda sem guarda no catálogo (`docs/CATRACAS.md` §15): «bancada
  compara trabalho igual», «merge marca quem mexeu», «interface só se prova
  exercitando», «medidor com binário velho» — o catálogo só sabe repor
  defeito em arquivo compilado e conferido por `cargo test`.
- **Este changelog não tem entrada para as rodadas diurnas de 16/09** — 33
  commits entre `c4a47c5` (08:22) e `4eaff53` (22:26): pedidos 245, 247,
  252, 254, 256, 258–263 e 267, a sétima página e seus geradores, a auditoria
  SEC. Lacuna nomeada, não preenchida nesta entrada.
- **Rodada da replicação — bateria, revisão e conclusão, 17/09/2026 02:27
  UTC.** A revisão adversária (SEC), o parecer de DBA (C) e o inventário de
  QA (G) voltaram só de leitura, sem conserto; a bateria (F) voltou verde,
  dez bancadas de dez, com os três achados medidos de C confirmados pelo
  soquete (§21.4). Duas ondas de conserto entraram no mesmo dia: `49a3af7`
  fechou A2/A3 inteiros e A1 parcial; `eeb9925` fechou mais sete (A4, A5
  resto, A6, A8, A9, A10, A11) e o `rownum` (291, via a) — sobrando só A1
  pleno (parecer) e A7 abertos entre os onze achados de SEC. `docs/REPLICACAO.md`
  §21, `docs/PENDENCIAS.md` 278–309.
- **SEC A1, parcial — o pulso do cluster ainda não amarra a identidade do nó a
  uma prova criptográfica.** Parecer de B2 (17/09/2026, `eeb9925`): não é
  possível no aperto de mão atual — o cluster cifrado usa Noise **NX**, em
  que só o respondedor apresenta chave estática; quem manda o pulso é o
  **iniciador**, anônimo por decisão já registrada em `docs/CIFRA-DO-FIO.md`
  §12. Fecharia com prova por Diffie-Hellman das estáticas que já existem
  (`chave_do_fio` já é um `known_hosts`) ou trocando o aperto para XX/IK —
  as duas são desenho de protocolo cifrado, decisão do dono. O que já foi
  corrigido (época/posição sem teto, pulso fora do portão) está em
  Corrigido. Pedido 278.
- **C — a via (b) do `rownum`: a réplica ainda gera o número dela, em vez de
  honrar o que vem na imagem.** A via (a) — consumir o número só depois da
  última guarda que recusa — fechou em `eeb9925` (pedido 291, ver Corrigido)
  e resolve toda escrita nova; um source com um buraco **já gravado antes**
  desse commit continua com o buraco, e só a via (b) o alcançaria. Pedido 309.
- **C — unicidade num índice secundário trava o par de servidores no
  bidirecional para sempre**: `[SP000020] chave duplicada` recusa o evento e
  o lote nunca avança (§2.5, idem). Pedido 292.
- **C — 12 eventos gravados num único milissegundo**: o `.log` carimba em ms
  e uma passada de commit empata, o que muda o desenho da coluna de
  data/hora de sistema por linha que o dono pediu em 11/09 (§4.2, idem).
  Pedido 289.
- **G — `crates/phxsql-server/src/cluster.rs` tem ZERO entradas no catálogo
  de guardas** (`docs/propostas/inventario-qa-replicacao-2026-09-17.md`
  §2.2, 17/09 02:34 UTC) — a eleição (pedido 211), o escalonamento a quente
  (217) e os quatro modos A–D (214) têm teste real e nenhuma guarda
  catalogada. Pedido 302.
- **164 — falta a medição final em máquina parada.** O que o pedido pedia
  (encurtar as 5 seções do gatilho `BEFORE`) morreu medida com o número —
  ver Corrigido; o conserto do `empilhar` entrou de bônus. Falta só rodar de
  novo com `quieta.Vigia` aprovando, em máquina livre.
- **`ler` dentro de uma transação paga O(pendentes) sob a trava global** —
  38 µs com zero escritas pendentes, **1.118,50 µs com 1.600** (medido pelo
  mesmo medidor do 164). A sobreposição ali é a funcionalidade
  (read-your-own-writes, pedido 162), então não se remove — falta guardar o
  mapa por transação e invalidá-lo no empilhamento. Pedido 310.
- **O assistente de replicação mostra «PhxSql» com um buraco no lugar da
  versão** — `sondar_origem` (`servidor.rs:22638`) não devolve `versao` nem
  `ms`, e a tela (`ui/index.html:13037`) escreve os dois. Achado da frente do
  190, conserto é do servidor. Pedido 311.
- **190 — 119 botões ainda sem prova**: 12 em `ui/claude.js` (pedem chave de
  API — decisão em aberto entre interceptar a rota ou dispensa registrada),
  7 do assistente de replicação e 6 do DbLink (exigem um segundo servidor de
  verdade do outro protocolo, fora desta bateria), e 16 num rabo parelho de
  quatro em quatro (`cartaoNovaTabelaER`, `desenharNovaTabela`, `editarJob`,
  `telemetria.js`) que já são exercitáveis nesta máquina.

## Não lançado — Colmeia × SQLite × padrão nas quatro operações (bancada)

### Adicionado

- **`bancada/colmeia/medir-crud.py`** e o modo `crud` do exemplo
  `custo-da-colmeia`: ler, inserir, atualizar e excluir de ponto nos três
  lados, mesma máquina, dois regimes de durabilidade casados, linha de base
  do Python publicada, syscalls por operação medidas por `strace`. 24
  combinações, nenhuma não medida (16/09/2026). Ler: colmeia 6,8×–11,2×
  sobre o padrão e 13×–31× sobre o SQLite. Escrita com fsync: colmeia
  2,1×–4,4× fazendo menos (2 fsync contra 8–9 do padrão e 4 do SQLite).
  Escrita sem fsync: a colmeia ganha a 1.000 e 10.000 e **perde para o
  padrão a 100.000**, porque a cópia de caminho cobra o fanout da raiz (391
  grupos, 5.144 bytes por inserção). `docs/propostas/colmeia.md` §1.1.

### Sabido

- O padrão paga 8 a 9 `fsync` por operação no regime por operação, porque
  `Volumes::sincronizar` sincroniza todo descritor aberto sem pular os
  limpos — é o que o põe atrás do SQLite em toda escrita com fsync
  (pendência #258, medir antes de consertar).
- O excluir do padrão custa 24–28 µs sem fsync, seis vezes o inserir, com 8
  `write` e cerca de 5 `openat` por linha (pendência #259).
- O portão «está medindo?» casa o invólucro `bash -c` que só menciona uma
  bancada, e duas bancadas que se esperam por ele travam uma à outra
  (pendência #260).

## Não lançado — Chutar a tomada: a bancada da queda na transação e no BULKINSERT

### Adicionado

- **`bancada/tomada/`**: SIGKILL num `phxsqld` próprio em varreduras de atraso
  — transação aberta com SAVEPOINT e sem COMMIT, BULKINSERT linha a linha,
  `inserir_lote`, `reindexar` de 10.000 linhas, transação dentro da tabela
  reservada — e o banco reaberto depois, com o byte de «sujo» do `.ndx` lido
  antes de reabrir e a contagem de `fsync` antes do «ok» por `strace`. **408
  quedas, 22 conferências, 0 desfechos inválidos** (16/09/2026). Quatro
  guardas novas no catálogo, provadas vermelhas com o defeito reposto.

### Sabido

- `bulkinsert(false)` não drena a marca `.tx` de um COMMIT feito dentro da
  tabela reservada: a marca sobrevive ao «ok» e o arranque seguinte relata uma
  recuperação de um commit já durável. Nenhuma linha perdida ou duplicada
  (pendência #254).
- Queda no meio de um BULKINSERT ou de um `reindexar` deixa o `.ndx` marcado
  sujo e a tabela recusando toda operação de índice, nomeando o conserto, até
  um `reindexar` manual — e o arranque não avisa, porque só reconstrói índice
  de tabela nomeada numa marca (pendência #255, decisão do dono).
- `inserir_lote` fora de transação não é atômico sob queda: lote parcial
  possível, sem duplicata. Comportamento medido, não defeito de contrato.
- BULKINSERT dentro de transação é recusado pelo motor (SP000018); a
  varredura foi feita na ordem que o portão aceita.
- `bancada/carga/bulkinsert.py` faz `pkill -x phxsqld` ao subir, contra a
  regra de matar só o próprio PID (pendência #256).

## Não lançado — Saúde do disco do banco (pedido 249)

### Adicionado

- **Sonda canário no disco do banco** (`saude_do_disco.rs`): a cada
  `alertas.disco.checar_segundos` (60 s) escreve, sincroniza, lê e apaga um
  arquivo próprio de 64 bytes no diretório de dados, e classifica o que falha
  por `ErrorKind` **e** errno — nesta `std`, `EIO` cai em `Uncategorized`, e
  só o errno o distingue. Detecta EROFS, disco cheio e erro de E/S.
- **Erro de E/S no caminho de gravação avisa na hora.** Um `PhxError::Io` que
  passa pelo sumidouro de erros do servidor vira evento de saúde e, fora do
  silêncio por tipo, um e-mail imediato — e um SMS por e-mail-para-SMS da
  operadora (`alertas.sms`), sem caminho do disco no texto. Sob a trava global
  só se conta e se entrega numa fila; quem manda é a thread da sonda, que é
  também o carteiro e acorda na hora. Provado pelo soquete com um relé SMTP
  falso: um e-mail no primeiro erro, nenhum no segundo dentro da janela.
- `op saude_disco` (direito de leitura; texto do erro só para quem administra),
  bloco no `painel`, cartão ao lado do espaço em disco, 24 chaves pela fábrica
  de idiomas, caso de navegador `29-saude-do-disco.mjs` nos dois temas.
- Quatro guardas no catálogo, provadas vermelhas com o defeito reposto.

### Sabido

- O meio do SMS quando a operadora não oferece gateway por e-mail é decisão do
  dono: programa externo sem shell (ponto de segurança) ou HTTPS (crate, que a
  pétrea de zero dependências não deixa entrar calada). SMART, `/proc/mounts`
  e `df -i` não entraram.
- A catraca `rede-ou-espera` do mapa da trava pegou a primeira versão desta
  frente mandando e-mail com a trava na mão (0 → 1) antes do hand-back — o
  desenho foi refeito e a catraca voltou a 0. Catraca que roda sozinha pega.

## Não lançado — Semáforo e teto das threads (pedido 248)

### Corrigido

- **Vaga que não voltava depois de um pânico.** A porta de dados contava
  conexões com `fetch_add`/`fetch_sub`; um pânico no corpo da thread pulava o
  `fetch_sub` e a vaga nunca voltava — depois de N pânicos a porta recusava
  todo mundo com o servidor de pé. Hoje a vaga é uma `Permissao` RAII que morre
  no `Drop`, inclusive no desenrolar do pânico. Guarda
  `permissao-de-dados-sem-raii`, provada vermelha com o defeito reposto.
- **Thread em pânico continuava «viva» na telemetria.** O `fio_morreu` era
  chamada depois do corpo — o mesmo defeito, no irmão que chama as mesmas
  funções na mesma ordem. A ficha passou a morrer no `Drop` (`FichaViva`).
  Guarda `ficha-do-fio-pulada-no-panico`.

### Adicionado

- **`Semaforo` da casa** (`phxsql-core/src/semaforo.rs`): `Mutex<usize>` +
  `Condvar`, zero crate — a `std` não tem semáforo. `tentar`, `adquirir`,
  `adquirir_ate`, `em_uso`, `esperando`, `teto`; mutex envenenado recuperado
  por `into_inner`; dez testes, com `catch_unwind` e veneno de propósito.
- **Teto nas portas web e REST**, que não tinham nenhum:
  `recursos.conexoes_web_max` (64; 0 = sem teto) e `fila_web_ms` (2.000). Acima
  do teto e esgotada a fila, **503 com `Retry-After`** e linha no
  `acessos.log`. Medido com 500 conexões segurando 3 s: pico de 504 threads e
  14,6 MiB antes, **68 threads e 7,6 MiB** depois, 436 recusas com
  `Retry-After` e zero reset (`bancada/concorrencia/resultados.json`).
- **Monitor de threads em runtime**: `op_telemetria.tetos` por família (em
  uso / teto / esperando) e `totais.threads_do_so` (`/proc/self/status`,
  `null` fora do Linux) ao lado das registradas; régua no resumo do gestor da
  telemetria, quatro chaves pela fábrica de idiomas.
- **Mapa das threads com catraca** (`bancada/concorrencia/mapa-das-threads.py`):
  todo `spawn`/`Builder`/`scope`/`subir` fora dos testes tem de estar no
  catálogo com o teto que o segura — 19 sítios, `spawn-sem-teto = 0`; item 0c
  da bateria. E `enxurrada-web.py`, a bancada das 500 conexões.
- `erro.porta_cheia` nos seis idiomas; `MANUAL.txt` e `Config_exemplo_01.json`
  com os dois campos; `docs/CONCORRENCIA.md` §17.

### Mudado

- A porta de dados recusa na hora acima do teto, como `max_connections` no
  PostgreSQL, MySQL e MariaDB — e o comportamento abaixo do teto é o de antes
  (`abaixo_do_teto_a_web_nao_muda`, o teste do comportamento velho).
- `subir_web` deixou de ter laço próprio: chama o `aceitar_http` — era o irmão
  com cópia.

### Sabido

- O fecho da janela **já tinha teto** (`FIOS_DO_FECHO = 16`); o quadro da
  rodada leu «sem teto» porque o `grep` acha o spawn e não o teto. Medido com
  tetos 4/8/16/sem em três corridas: nenhum ganha fora do ruído, fica 16.
- Na onda rápida o teto da web enfileira: p99 de 63 ms contra 3 ms sem teto,
  no mesmo binário. Uma corrida por braço, escrito na §17.6.
- Os dois campos novos exigem reinício, como `conexoes_max`. Não há thread
  pool na porta de dados (a trava global entrega concorrência 1 — medir
  primeiro) nem série histórica de `em_uso` (só o instante).

## Não lançado — Leitura repetível pela trava, pedida

### Adicionado

- **Leitura repetível, sem MVCC.** O gap «isolamento acima de READ COMMITTED»
  (pendência #239) foi reaberto pelo dono e resolvido pela via (b) já nomeada
  em `docs/SOMBRA.md` §5b: quem **pede** ganha leitura repetível e ausência de
  fantasma pela trava, sem construir a Sombra. Protocolo: `begin` com
  `"leitura_repetivel": true` (alias `"repeatable_read": true`). SQL: `BEGIN
  [TRANSACTION] ISOLATION LEVEL REPEATABLE READ`, em qualquer ordem com
  SCOPE/TIMEOUT/LOCK TIMEOUT/LOCK MODE/STATEMENT TIMEOUT.
- **A trava compartilhada (S).** `Trava::Compartilhada`
  (`crates/phxsql-server/src/travas.rs`), tomada em cada tabela que a
  transação LÊ, pelo portão único `dentro_da_transacao` →
  `travar_leitura_repetivel` → `esperar_trava` (respeita LOCK TIMEOUT e o
  TIMEOUT da transação), solta em COMMIT/ROLLBACK/estouro de prazo/queda da
  conexão. Leitores compartilham a S entre si; ela barra e é barrada por
  intenção/exclusiva/linha alheia — o `INSERT` disputa `FIM_DA_TABELA` e,
  sob S, não entra: sem fantasma, de graça.
- **A ficha nomeia o nível.** `op transacao` devolve `"leitura_repetivel":
  true/false` e `transaction_isolation` com o texto do nível que está de fato
  valendo (`NIVEL_DE_ISOLAMENTO`/`NIVEL_DE_ISOLAMENTO_REPETIVEL`,
  `crates/phxsql-server/src/transacao.rs`).
- **Prova real nos dois sentidos**
  (`crates/phxsql-server/src/servidor.rs::testes_leitura_repetivel`):
  `sem_pedir_a_leitura_continua_nao_repetivel` (controle),
  `pedindo_a_leitura_e_repetivel_e_o_escritor_espera`,
  `pedindo_nao_ha_fantasma`, `a_recusa_e_do_leitor_que_pediu`,
  `duas_repetiveis_que_leram_a_mesma_tabela_nao_se_atropelam` e
  `pelo_sql_o_select_tambem_toma_a_s` (um `SELECT` pelo SQL, que não tem
  campo `tabela` e vira `varrer`/`buscar` derivados, toma a S pelo mesmo
  portão — a prova de que o gancho está no lugar único). Com o gancho
  removido do portão, 3 falharam e o controle passou. No tradutor,
  `crates/phxsql-sql/src/transacao.rs::isolation_level_na_abertura` e
  `set_isolation_level_nomeia_o_nivel_real`.

### Mudado

- **`READ UNCOMMITTED` deixou de ser recusa muda.** É aceito e vale `READ
  COMMITTED` — como o PostgreSQL(R), o motor nunca lê sujo.
- **A mensagem do `SET TRANSACTION ISOLATION LEVEL X`** continua recusando (o
  tradutor não guarda estado de sessão), mas agora aponta o caminho que
  funciona: `BEGIN ISOLATION LEVEL REPEATABLE READ`.
- **Textos de tela novos pela fábrica de idiomas:** `tela.tx_isolamento_a` e
  `tela.tx_isolamento_b`.

### Sabido

- **Não há detector de impasse.** Duas transações repetíveis que leram a
  mesma tabela e tentam escrever recebem `LOCK TIMEOUT` nos dois sentidos — o
  prazo resolve, sem nenhuma completar com um resultado quebrado.
- **Uma falha não reproduzida em `uuid::tests::v7_nunca_repete_nem_anda_para_tras`.**
  Na primeira corrida da suíte inteira desta rodada, com clippy e uma segunda
  suíte rodando ao mesmo tempo em 4 CPUs, esse teste do `phxsql-core` falhou
  uma vez; o log da corrida só guardou a linha do pânico, não os dois ids. Em
  seguida: 5 corridas do teste sozinho, 6 da suíte do crate e 1 da suíte
  inteira sem fail-fast, todas verdes (12 verdes, 1 vermelho). O gerador é
  monotônico sob o mutex pela leitura do código, e ninguém tocou nesse
  arquivo desde o commit que o criou. Fica registrado como não explicado
  (pendência #247) em vez de sumir — teste que falha uma vez sem motivo é
  pior que teste que falta, porque ninguém sabe em que confiar.
- **`SERIALIZABLE` não se reivindica.** `ISOLATION LEVEL SERIALIZABLE` recusa
  nomeando o que existe; o que se afirma é só o que os testes medem.
- **A Sombra/MVCC continua parada.** A via (b) resolveu o gap sem precisar
  dela; o território que sobra é só o leitor longo que não pode pagar o
  escritor esperando.

---

## Não lançado — SQL: `UPDATE`/`DELETE` por faixa

### Adicionado

- **`UPDATE`/`DELETE` deixaram de exigir chave única** (item 1 do roteiro «SQL
  para nota 9»). O `= ` sobre um índice único de uma coluna continua no caminho
  rápido de três passos; qualquer outra condição — `= ` sobre coluna sem chave
  única, ou faixa (`<>`, `<`, `<=`, `>`, `>=`) — segue o caminho **por faixa**:
  uma op de leitura nova, `coletar_rowids`, colhe **todos** os `rowid` que casam
  (aplicando o mesmo `memoria::passa` do `varrer`), e o servidor
  (`executar_dml_por_faixa`) percorre a lista fazendo `ler`+`atualizar` com a
  linha mesclada, ou `ler`+`excluir` suave — os **mesmos** pedidos do caminho por
  chave, um por linha. Colher antes de aplicar **fecha o Halloween**: a lista de
  `rowid` está fechada antes da primeira gravação, então uma linha que o próprio
  `UPDATE` tira do filtro não é reprocessada. `docs/SQL.md` §6.1.
- **Prova real nos dois sentidos.** No nível do servidor,
  `update_por_faixa_muda_todas_as_que_casam_e_so_elas` e o irmão do `DELETE`
  medem `afetadas` e o dado gravado; com o defeito reposto (tratar só a primeira
  linha colhida — o `find` no lugar do `filter` que esta casa já pagou),
  `afetadas` cai para 1 e as duas falham. No tradutor, o `= ` sem chave única e a
  desigualdade viram `AtualizarPorFaixa`/`ExcluirPorFaixa` com o filtro certo.

### Mudado

- **A recusa não sumiu — mudou de motivo.** Antes o `UPDATE`/`DELETE` de faixa
  ou de coluna sem chave única era recusado por não haver índice único; agora a
  recusa é o **teto** do `coletar_rowids` (`TETO_COLETA_ROWIDS`, um milhão): uma
  faixa maior do que ele consegue prometer inteira é recusada **nomeando o
  limite**, em vez de gravar sobre o começo com cara de ter gravado sobre tudo.
  A régua passou de «tem índice?» para «cabe inteiro?». Cinco testes que
  afirmavam a recusa antiga foram **reescritos** para provar o novo caminho — a
  proteção que guardavam continua, no teto.

### Sabido

- **Fora de transação, o `UPDATE`/`DELETE` por faixa não é atômico** (cada linha
  é um `atualizar`/`excluir` avulso). Dentro de `BEGIN`/`COMMIT` cada uma empilha
  e o `COMMIT`/`ROLLBACK` as alcança juntas. O comportamento de uma linha
  empilhada na mesma transação sob faixa **não foi medido** nesta rodada.

---

## Não lançado — Ledger: `ALTER TABLE ADD COLUMN` travado em tabela-cadeia

### Corrigido

- **O motor RECUSA acrescentar coluna numa tabela em modo ledger.** O hash de
  cada bloco cobre o conteúdo canônico na ordem do esquema; acrescentar coluna a
  uma tabela-cadeia já gravada deslocava esse conteúdo para toda linha antiga, e
  `verificar_cadeia` passaria a acusar **falso positivo de adulteração** numa
  cadeia intacta (ou a mascarar uma real). A guarda entra no topo de
  `Table::acrescentar_coluna` — o único ponto por onde o `op_acrescentar_coluna`
  do servidor passa, portão único e não espalhado. O modo ledger é reconhecido
  por **convenção de esquema** (`ledger::e_tabela_ledger`): `hash`/`anterior`
  Uuid256, `altura` Sequence e o índice único `porAltura`, não por flag gravada.
  A recusa é **absoluta** — nem nula, nem com padrão, nem em tabela vazia —, e
  não a régua relaxada do SQL Server («só no fim, nullable, fora do hash»): o
  conteúdo canônico desta casa pula coluna por **nome**, não por posição, então
  coluna nova mesmo nula entra na conta. Prova real nos dois sentidos, mais três
  provas em `ledger.rs`. Era o achado URGENTE do §2.1 da pesquisa do DBA
  (`docs/propostas/dba-bases-2026-09.md`); os outros três itens (recusar
  `UPDATE`/`DELETE` no motor, âncora externa, `DROP` redobrado) seguem abertos.

---

## Não lançado — ACID-C: a cascata do `ao_alterar` entra na transação

### Corrigido

- **A cascata do `ao_alterar` entra no conjunto de escrita da transação
  (ACID-C).** Antes, dentro de uma transação, alterar a chave de um pai
  cascateava só no `COMMIT`: a filha ficava para trás na visão da transação, o
  `COMMIT` respondia `gravadas:1` com duas tabelas mudadas, e o `ROLLBACK` não
  alcançava a cascata. Agora a mãe e cada filha viram uma escrita própria da
  lista (no molde do *super-journal* do SQLite): o read-your-own-writes mostra a
  filha acompanhando a chave nova, o `COMMIT` conta as duas e o `ROLLBACK` as
  desfaz. Continua a fundação do P0 (a conferência de FK dentro da transação
  enxerga o pai empilhado). Provas `acidc_*`, prova real nos dois sentidos.

### Mudado

- **Marca `.tx` v3.** Cada operação carrega, no fim do payload, um byte
  `cascata_na_lista`: `1` aplica sem re-cascatear (a corrente já é a lista), `0`
  mantém o `recascatear` da v2. As marcas v1 e v2 continuam sendo lidas — marca é
  commit que já começou. Sem migração de dado. Ver `docs/FORMATO.md`.
- **Escopo efetivo.** Uma alteração que de fato cascateia expande o escopo da
  transação **na hora** (as tabelas filhas entram em `tabelas_expandidas` e são
  travadas); em `SCOPE MODE STRICT`, uma filha não declarada é recusada nomeando
  a tabela. Custo zero para quem não cascateia.

### Sabido

- Fora de transação, a cascata segue acontecendo dentro do `atualizar` e não é
  atômica por desenho. E uma filha inserida por outra conexão sob a chave velha,
  entre o `empilhar` e o `COMMIT`, é um fantasma que a cascata não vê —
  consistente com o `READ COMMITTED` desta casa.

---

## Não lançado — a revisão do motor: segurança e integridade do que a rodada trouxe

### Corrigido

- **Direito por coluna, upsert e a ficha (A1 + o achado crítico da tela):**
  uma coluna que o usuário não pode ALTERAR passa a ser **mantida no valor
  gravado**, nunca zerada nem motivo de recusa. Antes, o upsert `atualizar`
  por quem tinha a coluna negada a ZERAVA (perda de dado calada), e a ficha
  não conseguia salvar nem incluir NADA para um usuário com regra de coluna,
  porque a presença da coluna no pedido era lida como alteração e recusava a
  operação inteira — proteção que quebrava todo cliente. A resposta agora diz
  `colunas_mantidas`. Presença não é intenção.
- **`INSERT … ON CONFLICT DO UPDATE SET` / `ON DUPLICATE KEY UPDATE` (A2):** o
  SET era ignorado e o VALUES gravava por cima com NULL onde faltava. Agora o
  `op_inserir` e o empilhar da transação leem `atualizar` e mesclam o SET sobre
  a linha existente; `atualizar` desconhecido recusa em vez de ser ignorado.
- **Índice parcial vira oráculo (A3):** `varrer`/`buscar` por um índice cujo
  `onde` cita a coluna negada respondia «quem tem salario > 5000?». As colunas
  do `onde` do índice entram no crivo do direito por coluna.
- **Junção materializava antes do teto (A4):** um `interno` 1000×1000 alocava
  +561 MiB para recusar 1 milhão de linhas; agora `consultar::juntar` para na
  linha teto+1 (+0,3 MiB).
- **`SELECT coluna_negada` devolvia `{coluna: null}` (A8)** em vez de recusar,
  e `consultar.em` com campo inexistente respondia 0 linhas calado (A14): os
  dois agora recusam nomeando.
- **Literal negativo em `SET`/`VALUES` (A10)** passou a parsear; **tabela
  inexistente (A13)** é nomeada em vez de vazar o caminho do disco.
- **O front-end do direito por coluna e da coluna calculada (a metade de tela
  do A1 mais três achados da revisão de tela):** a ficha passa a mandar **só as
  colunas que o usuário mexeu** — objeto por nome, não array posicional —,
  então a coluna negada em `alterar` não vai mais como `null` e o operador com
  regra de coluna volta a salvar e a incluir pela tela (o servidor já protegia
  a linha; o que muda é o pedido deixar de mencionar o que ele não tocou).
  `colunas_sem_leitura` esconde a coluna na grade, na ficha e na aba Estrutura;
  a **coluna calculada nasce read-only** e sai do diálogo de conflito, para não
  gerar divergência falsa contra o que o servidor recalcula sempre; os
  `prompt()`/`confirm()` nativos de restaurar, backup e conferir-backup viram o
  diálogo da marca. Prova viva nos dois sentidos: caso `27-direito-por-coluna`
  novo e o `22-botoes` estendido, bateria de tela **51/51 nos dois temas**.
  Dois defeitos pré-existentes que travavam a prova também caíram, confirmados
  contra `e27775b`: o laço da barra do passeio nunca conferia o denylist
  `FORA`, e o `14-acrescentar-coluna` mandava literal sem aspas a um campo de
  expressão. Catracas intactas: `TETO_BOTAO_SEM_PROVA` em 194,
  `TETO_ROTULOS_E_CRASE` em 1.049.

### Sabido

- Ficam nomeados os pedidos 240-245: EXISTS correlacionado por apelido de fora,
  visão que perde a projeção sob `SELECT *`, CHECK julgado só no commit da
  transação, índice por expressão com operador, `CREATE VIEW` com JOIN, e as
  seis observações menores.

## Não lançado — as dezoito do comparativo, junções, subconsultas e os limites nomeados

### Corrigido

- **`LEFT JOIN` com a direita vazia perdia as colunas da direita** (pedido
  237): os nomes saíam da primeira linha da direita, e com a direita vazia a
  coluna sumia em vez de vir nula — a forma da linha mudava entre o caso
  casado e o órfão. Agora os nomes vêm do **modelo tipado** do lado, e a linha
  órfã traz todas as colunas do outro lado, nulas.
- **`Decimal` no `consultar.expressao` comparava como texto** (pedido 237):
  `"9.50" > "10.00"` era verdadeiro. A célula passa a ser convertida pelo tipo
  do modelo antes de a expressão vê-la.
- **Regra de direito por coluna que citava coluna inexistente carregava
  calada** (pedido 235): `colunas: {salrio: …}` entrava sem aviso e `salario`
  ficava sem regra. Recusa na carga quando a tabela existe, nomeando as
  colunas que existem; tabela que ainda não existe aceita com aviso, e
  `criar_tabela` avisa a regra inerte.
- **`threads_e_cpu_viram_o_teto_do_paralelo` caía na suíte inteira e passava
  sozinho** (pedido 234): estado global de processo (`paralelo::TETO`)
  disputado com `Config::ler`. Conserto por contrato, verde em 300 corridas.
- **A lei e a marca diziam que «não há transação»**, e há desde o pedido 162;
  o motivo de *ACID compliant* continuar falso é o isolamento `READ COMMITTED`
  e o C parcial (`docs/ACID.md` §0). Mais dezessete pontos da revisão de
  documentação de 09/09: três telas do `MANUAL.txt` descritas como apagadas
  quando funcionam, seis contagens de operações digitadas e envelhecidas, o
  bloco morto do `LEIA-ME.md` do dossiê, quatro operações de idiomas sem
  manual, e os dois números da cobertura da tela que viviam à mão no dossiê e
  agora saem do gerador.

### Adicionado

- **Junções `direito`, `completo` e `cruzado`** no `consultar`
  (`RIGHT`/`FULL`/`CROSS JOIN` no SQL, 1:1), com o teto do produto conferido
  **antes** de materializar.
- **`existe`**: `[NOT] EXISTS` correlacionado por igualdade, como semijunção
  por espalhamento — cada `de` pelo portão único e visível ao direito por
  coluna.
- **`COUNT(coluna)`** contando não nulos, **`ORDER BY p.id`** qualificado, e
  **`colunas: [{nome, tipo}]`** na resposta do `consultar` e do `agrupar`.
- **`SQL_C_WCHAR`** no driver ODBC, nos dois sentidos (UTF-16 ↔ UTF-8 na
  borda), com par substituto.
- **Aviso de regra de coluna inerte** nas três operações de cadastro e em
  `criar_tabela`.

- **Expressões no esquema**: `padrao` (DEFAULT no inserir), `check`
  (restrição avaliada no inserir e no atualizar), coluna `calculada`
  (sempre recalculada na gravação), índice parcial por `onde` e índice por
  expressão de uma coluna (`lower(nome)`) — todos pela mesma gramática de
  `phxsql_core::expressao`, a que os gatilhos já usavam.
- **`agrupar`**: `GROUP BY` genérico, com os mesmos agregadores do
  `pivotar` (soma, média, contagem, mínimo, máximo, contagem distinta).
- **`consultar`**: composição de sub-pedidos — `de` para a fonte, `em`
  para `IN (SELECT …)`, `escalar` para subconsulta escalar, `janela` para
  `ROW_NUMBER() OVER`, e `juntar` para `JOIN`/`LEFT JOIN` por igualdade de
  colunas, com nome QUALIFICADO (`p.id`). Cada sub-pedido roda pelo mesmo
  portão de permissão de qualquer cliente.
- **Visões**: `criar_visao`, `visoes`, `excluir_visao` — a visão guarda
  texto, analisado a cada uso dentro de um `consultar`.
- **`diferencas`**: diz ONDE duas tabelas com a mesma chave única
  divergem — a chave, as colunas e os dois lados —, não só SE divergem.
- **`inserir.se_existir`** (`ignorar`/`atualizar`): upsert por chave
  única, extraído do `aplicar_para_ca` do DbLink para um lugar só, usado
  pelos dois.
- **`sql.parametros`**: o `?` na instrução preparada, resolvido no léxico
  por TOKEN — nunca por substituição de texto.
- **SQL novo**: `GROUP BY` com agregação, expressão no `WHERE`, `WITH`,
  `IN (SELECT …)`, `ROW_NUMBER() OVER`, `CREATE`/`DROP VIEW`,
  `INSERT … ON CONFLICT`/`ON DUPLICATE KEY UPDATE`, `?`,
  `[INNER]`/`LEFT JOIN` e subconsulta escalar no `WHERE`.
- **Direito por COLUNA**: `tabelas.<t>.colunas` no cadastro, com
  `ler`/`alterar` por coluna. Sem `"colunas"`, nada muda — é o teste que
  mais importa.
- **PITR**: `restaurar_backup` ganhou `ate`/`ate_ms`, reaplicando o diário
  vivo de cada tabela pelo mesmo `Table::aplicar_evento` da replicação —
  aplica, não julga.
- **ODBC com parâmetros**: `SQLBindParameter` liga o `?` do lado do
  driver; a op `sql` do servidor passou a ler `parametros`, fechando a
  ponta a ponta.
- **`bancada/comparativo/`**: quatro sondas que eram código com veredito
  cravado (direito por coluna, PITR, parâmetro, diferenças) viraram sonda
  VIVA — exercitam o servidor pelo soquete e medem o EFEITO, com o
  controle na mesma corrida.

### Sabido

- Correlação que não seja igualdade, `IN (SELECT …)` correlacionado, escalar
  correlacionada e `EXISTS` sem par recusam nomeando: rodar a subconsulta por
  linha seria N passagens pelo portão.
- Parâmetro ODBC de saída recusa: nenhuma operação do servidor devolve valor
  além da linha.
- Isolamento acima de `READ COMMITTED` e TLS no transporte continuam NÃO, por
  decisão do dono (pedido 239).
- A média de inteiro no `agrupar` sai como `Real8`, não como o tipo da
  coluna — medido no acumulador, não no contrato.

## Não lançado — a auditoria externa, medida; e os números que ninguém digita mais

Uma auditoria técnica externa da 0.18.0 chegou com 844 linhas. **Medida antes
de virar plano**, que é a regra da casa para receita de fora — e ela se
sustentou em quase tudo: **nove das dez** contradições de documentação que
aponta são verdadeiras. A falsa é a que diz que `docs/SQL.md` ainda alega
ausência de transação: ele as documenta extensamente.

### Corrigido

- **O §19 dizia «política de commit por origem», e estava errado — escrito por
  mim uma hora antes.** `replicacao.origens` é a lista da **réplica**: de onde
  *ela* puxa. Quem espera o quórum é o **master**, e do lado dele `origens` não
  existe. Das duas listas que o source tem, `replicas_autorizadas` é **ACL de
  IPs** (sem identidade nem saúde) e só `cluster.nos` serve de **M** — é sobre
  ela que a maioria já é contada. O campo do quórum mora no bloco `cluster`, e
  o corolário desagradável está escrito junto: **quórum de escrita passaria a
  exigir o bloco `cluster`**, e instalação com replicação simples não teria
  onde declarar o M.

- **E uma armadilha que quase virou defeito publicado.** O assistente de
  replicação chama `replicacao_configurar`, e provei contra o motor vivo que
  essa operação **não existe**. Li isso como «o botão quebra» — **e não
  quebra**: a chamada é guardada, e o `catch` desvia para `aplicarPeloConfig()`
  exatamente no `NAO_ENCONTRADO`. O assistente é compatível com o motor futuro
  e hoje entrega o bloco do arquivo, manda reiniciar e confere. *Provei uma
  coisa e concluí outra* — e foi a **segunda vez no mesmo dia**, depois do
  `grep` que contou dois `<a download>` que eram os comentários dizendo que não
  há nenhum. O padrão é medir um proxy e ler o proxy como a coisa.

- **O `atraso_ms` da bancada de replicação soma duas coisas e só anuncia uma
  — e isso quase virou a resposta errada a uma pergunta do dono.** Ele pergunta
  sobre transação com quórum; o número à mão eram os **826–2014 ms** de atraso
  daquela bancada, e lido como transporte ele inviabilizaria qualquer commit
  síncrono. **Não é o que ele mede:** o laço da réplica **dorme**
  `reconectar_em` quando não acha nada, e aquela bancada roda com 2 s. Medido
  separado na `bancada/quorum/` nova: gravar no master **0,209 ms**, levar até
  uma réplica **0,475 ms**, commit esperando 2-de-3 **0,661 ms (3,16×)** e
  3-de-3 **0,704 ms (3,37×)**, 60 voltas em localhost — que é o **piso**, com o
  aviso viajando junto do número. **O sono era 99,9% do valor publicado.** O
  `resultados.json` da replicação passou a carregar um campo
  `atraso_ms_inclui` dizendo o que ele soma, e o `reconectar_em` virou
  constante **no `montar.py`**, que é quem escreve o config — digitá-lo no
  medidor seria a receita de um número envelhecendo noutro arquivo. *Medição
  honesta com rótulo incompleto engana melhor que palpite, porque vem com
  autoridade.*

### Adicionado

- **`INSERT`, `UPDATE` e `DELETE` por chave pela camada SQL — o passo 2 do
  roteiro do `docs/SQL.md`, e o CRUD fechado.** A tabela da §1 mapeava
  `UPDATE` para `atualizar`, e uma tradução direta teria **zerado as colunas
  que o `SET` não citou**: o `atualizar` recebe a linha inteira, e coluna
  ausente entra como NULL (`json_para_linha`). Por isso `UPDATE` e `DELETE`
  são três passos — `buscar` pela chave única, `ler` com a `versao`, e
  `atualizar`/`excluir` com a linha mesclada e a `versao` lida —, cada um
  pelo mesmo `executar_derivado` do `SELECT`: o portão continua um, e há
  teste de que a tabela negada não se grava escrevendo SQL. Só chave
  **única**; índice comum recusa pelo nome, porque alcançaria N linhas sem
  dizer quantas. Uma linha por `INSERT`, porque `inserir_lote` não empilha
  numa transação aberta — e há teste de que o `INSERT` pelo SQL empilha.
  Prova real nos dois sentidos: tire a mescla e o teste falha na coluna que o
  `SET` não citou; tire a `versao` do pedido e o teste das funções puras
  falha. O console da tela mostra `afetadas`. `docs/SQL.md` §6.

- **`bancada/quorum/` — quanto custaria esperar as réplicas confirmarem.** Ela
  **não prova recurso**: mede o preço de uma decisão que ainda não foi tomada,
  e essa distinção está escrita na página de testes. Sobe master e duas
  réplicas com `reconectar_em` de uma hora, para o laço delas não competir com
  o cronômetro, e **para** quando uma réplica puxa sozinha (zero eventos
  significa que ela chegou antes, e o medidor estaria cronometrando o próprio
  concorrente) ou quando o esquema não a alcança antes da primeira volta.

- **`docs/REPLICACAO.md` §19 — a análise do quórum, com o número.** Três
  conclusões que não são opinião: quórum **não** substitui replicação (ele
  conta confirmações, e alguém tem de levar os bytes); os outros servidores
  **já existem e já votam** — o `cluster.rs` tem mapa, época e maioria, só que
  a maioria decide *quem é o master*, não *se a escrita chegou*, e o cabeçalho
  do módulo já confessava o buraco; e o obstáculo real **não é o custo, é a
  direção** — a replicação aqui é *pull* por firewall, e um quórum síncrono
  precisa do master sabendo, no instante do commit, que N réplicas têm o dado.
  A rota recomendada preserva o firewall: canal que **a réplica** abre e
  mantém. Pedido 207.

- **A bancada das transações existia, passava, e era INVISÍVEL.** O dono
  perguntou se a transação atômica funciona; a resposta certa era rodar o
  medidor, não citar o documento. Ele deu **36 conferências, 0 falhas** pelo
  soquete — inclusive `SIGKILL` no meio de um `COMMIT` de 3.000 linhas, com o
  banco reaberto e recuperando em 22 ms. Mas o medidor **não gravava
  `resultados.json`** e a bancada **não estava declarada** na
  `pagina-dos-testes.py`: a página que existe justamente para dizer o que este
  banco prova não listava a prova das transações. Ela não estava «NÃO MEDIDA» —
  ela não estava. *Medidor que não grava resultado não aparece nem como não
  medido: some, e a pergunta que ele responde volta pela boca do dono.* Hoje
  grava, está declarada, e ganhou `prova-dos-portoes.py` com controle positivo.

- **E o defeito reposto ensinou mais que a corrida limpa.** Apagando a marca
  `transacao_<id>.tx` antes de reabrir, o banco volta com **43 de 3.000** —
  pela metade. Isso **não é defeito do motor, é a demonstração do contrário**:
  o `SIGKILL` cai no meio da passada de commit, então há mesmo meia gravação no
  `.reg` naquele instante, e a marca é a única coisa que a resolve. *O «nunca
  metade» não é acidente do caminho de escrita — é comprado pela marca, e o
  preço dela fica invisível enquanto ela está lá.* `docs/ACID.md` §2.3.1.

### Mudado

- **Os botões da barra de ferramentas, ≥10% mais estreitos** — pedido do dono.
  Medido **antes** de tocar no CSS: 23 botões, 1.476,52 px somados, média 64,2.
  O número que decidiu o conserto foi outro: **15 dos 23 estavam exatamente em
  62 px**, o `min-width`; nos outros 8 manda o **rótulo**. Mexer numa
  propriedade só consertaria metade da barra. Três peças: `min-width` 62→55,
  `padding` horizontal 9→5 e `letter-spacing:-.02em` no rótulo — tracking, e
  não fonte menor, que custaria legibilidade nos 23 para consertar 8. **O irmão
  custou uma medição:** depois da primeira mudança a soma abaixo de 1025 px
  mudou **0,0%**, porque a `@media (max-width:1024px)` reescreve as duas
  propriedades — e aqui o irmão **não é uma função**: em folha de estilo, irmão
  é quem **redeclara a mesma propriedade**. Resultado em 10 larguras, de 1920 a
  360 px: **todos os 23 encolheram ≥10%**, pior caso 11,3% no desktop e 10,7%
  no celular, com **zero rótulos cortados**. Soma 1.476,52 → **1.293,56 px**.
  E um ganho não pedido: em 1440 px a barra caiu de **duas fileiras para uma**
  (102 → 56 px) — «Diretivas» e «Repair» transbordavam.

- **Três painéis do dossiê estavam parados sem um único dígito digitado.** 198
  pedidos onde eram 203, 428 testes na maior área onde eram 451, e 26.762
  linhas/s de replicação onde o `resultados.json` medido diz **37.810** — um
  retrato inteiro atrás. A causa não estava na conta e sim na **chamada**: o
  `pagina-dos-pedidos.py` recebe dois alvos, a página e o dossiê, e só o
  primeiro tinha padrão. Chamado sem argumento ele gravava a página, gravava a
  contagem de volta no `PENDENCIAS.md`, imprimia **três linhas de êxito** e
  pulava o painel do dossiê — e foi por anunciar sucesso que ninguém olhou de
  novo. O irmão era o `cobertura-por-area.py`, com o mesmo laço sobre
  `sys.argv` e a mesma falta. Outros quatro tinham padrão, mas era o **nome do
  arquivo digitado**, em sete lugares, que morreria na próxima refação do
  dossiê; e o `tetos-da-trava.py` era o único que **exigia** argumento, o que é
  a mesma armadilha por outro lado — quem repete a receita nua deixa aquele
  bloco para trás. Hoje há **um dono só**, o `docs/dossie/dossie_da_pasta.py`,
  que acha o dossiê varrendo `dossie-phxsql-*.html`, com a pétrea «só existe um
  por vez» virando portão: zero é parada com o motivo, dois é parada com os
  dois nomes, nunca um palpite sobre qual atualizar (medido pelo **código de
  saída**, 1 e 0, e não pelo texto — a primeira medição leu o `$?` depois de um
  cano e recebeu o status do `tail`). Os nove geradores importam esse dono, a
  receita do `LEIA-ME.md` perdeu o nome do arquivo, e restam **zero** nomes
  digitados no código da pasta. Prova real nos dois sentidos: com o painel
  forçado de volta para 198 e a chamada nua, antes do conserto o script imprime
  três linhas e o painel fica em 198; depois, imprime `painel dos pedidos
  regravado` e o painel volta a 204. *A lei «todo número visível sai de um
  gerador» prova a origem do número; a chegada dele é outra prova.*
  `docs/cognicao/cognicao_o-gerador-certo-chamado-pela-metade_20260907_0846.md`

- **O batimento de comunicação de 15 em 15 minutos, que passou rodadas sem
  cumprir.** Estava montado como `Monitor` dentro da sessão, e o `Monitor`
  morre: o runtime trunca o `timeout_ms` em 1.800.000 ms e ignora o
  `persistent`, então ele caía a cada ~30 minutos e era rearmado, e caía de
  novo. **A limitação estava medida e estava certa — e foi por isso que ela
  prendeu**: eu remedia «o `Monitor` sobrevive?», que já tinha resposta, em vez
  de «existe outro mecanismo?», que nunca foi perguntada. Medido: o **cron** é
  recusado abaixo de uma hora («the minimum interval is 1 hour»), mas o **tiro
  único** (`send_later`) de 15 minutos é aceito e guardado pelo servidor, então
  sobrevive à sessão. O piso é da expressão de intervalo, não da frequência de
  disparo. Hoje o batimento é uma **corrente de tiros únicos**, cada elo
  forjando o seguinte, com o gatilho de hora em hora como piso que a refaz —
  e a instrução dele carrega as duas recusas medidas, para ninguém repetir a
  tentativa. Pedido 204, com o buraco que fica nomeado: nenhuma guarda acusa a
  corrente arrebentada.
  `docs/cognicao/cognicao_remedir-o-caminho-fechado-nao-acha-o-aberto_20260907_0838.md`

- **O sexto veredito de ausência do `docs/HFSQL.md`, e o pior dos seis.** A §3.3
  dizia que a trava por linha «viria depois». Ela veio: há gestor próprio em
  `crates/phxsql-server/src/travas.rs` — intenção na tabela, exclusivo na linha,
  ordem canônica e `LOCK TIMEOUT` —, ligado ao servidor e **pedido** no caminho
  de escrita. O agravante que os cinco anteriores não tinham: eles diziam «não
  há» sobre coisa que passou a haver, e este dizia **«viria depois»** — promessa
  de futuro envelhece pior que negação, porque ninguém a lê como afirmação sobre
  o presente. A frase certa é mais estreita: vale **dentro de transação**, e fora
  dela a trava global continua serializando.

- **Um pedido invisível na página dos pedidos.** O 150 estava marcado `⏳`, que
  não está na legenda; o gerador não casava a linha e **seguia em silêncio** —
  200 pedidos publicados de 201 existentes. Hoje o `pagina-dos-pedidos.py`
  **para** diante de estado desconhecido, nomeando o arquivo e a linha.

- **Cinco defeitos no próprio medidor comparativo, e o pior produzia frase
  verdadeira.** As sondas de efeito liam `r["linha"]` do envelope quando a
  resposta vem dentro de `resultado`, e criavam tabelas com o índice num formato
  que o servidor recusa — a recusa não era lida. Resultado: «o campo `padrao` foi
  aceito e IGNORADO» publicado sobre tabela que nunca nasceu. *O errado sobrevive
  melhor quando o conserto funcionou por outro motivo.*

- **O número de operações que a tela alcança deixou de se digitar.** Ele já
  envelheceu três vezes — «36 das 39», «108 das 96», «104 das 122», esta última
  em **um dia**. Hoje sai de `bancada/cobertura-da-tela/medir.py`, com as duas
  listas tiradas do código: **123 operações**, **105 alcançadas**, **18 fora**.

### Adicionado no mesmo passo

- **`docs/COMPARATIVO.md`** — o que ainda falta aqui, contra quem tem.
  **18 de 19 capacidades** faltam ou estão pela metade no PhxSql; **13** foram
  perguntadas **por SQL a quatro motores vivos** nesta máquina (PhxSql 0.18.0,
  MySQL(R) 8.0.46, PostgreSQL(R) 16.13, SQLite(R) 3.45.1) e o resto por sonda de
  código com arquivo, linha e trecho citado. HFSQL(R) e Cassandra(R) entram
  **citados**, marcados célula a célula. O documento **não se edita**: a prosa
  mora em `bancada/comparativo/documento.py`.
  Achado que a pergunta não pedia: **quatro campos de esquema desconhecidos**
  (`onde`, `check`, `padrao`, `calculada`) são aceitos pelo `criar_tabela` e não
  fazem nada — *configuração que não é lida mente*.
  Achado medido nos outros: **MySQL(R) 8.0.46 não tem índice parcial**;
  PostgreSQL(R) e SQLite(R) têm.

- **Portão de medidor, provado nos dois sentidos.** `PHX_CMP_DEFEITO` repõe cada
  defeito do medidor e `bancada/comparativo/prova-dos-portoes.py` exige a parada
  certa — **e que sem defeito ele vá até o fim**, senão um medidor que parasse
  sempre passaria em tudo. Um quarto portão **morreu na prova**: o defeito que eu
  culpei (pedir o catálogo sem `database`) não reproduzia o sintoma.

- **Duas figuras do MOTOR, com as listas lidas do código.** O **fluxograma** do
  caminho de um pedido (§9): os **9 portões** que o `servidor.rs` numera no
  próprio comentário, o trilho de recusa, a trava, e os **9 passos** que o
  `inserir` segue no disco. O **workflow** do ciclo de operação (§31): modelar,
  gravar, consultar, replicar, salvaguardar, com a garantia que cada etapa impõe.
  O gerador **para** quando a lista do desenho diverge da do código nos dois
  sentidos — e pagou por si na primeira corrida, derrubando dois passos escritos
  de memória: `proximo_rowid` não existe (são `numerar_linha` e `numerar`), e
  faltava o `montar_payload`, **que é onde o `.bin` e o `.memo` são gravados**.

- **A numeração das figuras deixou de se digitar.** Enquanto figura só entrava
  no fim, o número escrito à mão batia por sorte; duas no **meio** do documento
  viraram **16** legendas erradas de uma vez. `numerar-figuras.py` renumera todas
  na ordem do documento, e achou uma desordem que já existia: as figuras vinham
  na ordem 14, 17, 18, 15, 19, 20, 21, 16. Legenda errada não quebra nada — é por
  isso que ninguém confere.

- **Duas figuras do medidor, desenhadas à mão em SVG e sem biblioteca** — o
  **fluxograma** do caminho de uma célula até o veredito, com os quatro portões
  que param a medição, e o **diagrama de workflow** da rodada inteira, da
  medição à página. Saem do mesmo gerador da tabela, então os rótulos com número
  (19 capacidades, 123 operações, 105 alcançadas) são medidos, e o número da
  figura sai da contagem das legendas anteriores. Entregues embutidas no dossiê
  **e** como `.svg` avulso — e as duas coisas não são o mesmo arquivo: o solto
  exige `xmlns`, só aceita as cinco entidades do XML e quer o `<svg>` como raiz.
  Três defeitos achados **abrindo no navegador**, nenhum visível no código: um
  rótulo invadindo a caixa vizinha, um texto riscado pela própria seta, e o
  arquivo avulso com `naturalWidth` = 0 sem erro nenhum.

- **O sétimo gerador do dossiê**, `docs/dossie/comparativo-no-dossie.py`, escreve
  a tabela comparativa na §33 — a seção do «o que este motor não faz» deixa de
  ser prosa inteira, que é onde ausência envelhece.

### Corrigido

- **O `varrer` ganha `WHERE`.** A grade filtrava o que já estava nela: pedia
  `varrer max=2500`, recebia 2.500 linhas e jogava fora 2.475 no navegador. A
  premissa foi medida **antes** de o predicado existir, porque um `WHERE` sem
  índice não remove varredura nenhuma — o motor lê as mesmas linhas para decidir
  quais passam. Medido em 100.000 linhas, mediana de sete rodadas intercaladas
  pelo soquete: a leitura é **48,0%** do tempo e o transporte é **52,0%**. Teto
  previsto 2,06×, medido **2,07×**, com **532.777 bytes virando 5.638** no fio;
  com metade da tabela casando o ganho cai para 1,35×. O `max` **continua sendo
  linhas examinadas**, e isso é decisão: trocar para «devolvidas» faria um filtro
  pouco seletivo varrer a tabela inteira **com a trava global na mão**, e esse
  custo ninguém mediu. Desce **só o que prova concordar** — há dois motores do
  mesmo filtro e eles discordam em silêncio (o `contem` da grade ignora acento,
  o do servidor não), então a grade reaplica tudo e o servidor só pode diminuir
  o que atravessa o fio, **nunca mudar a resposta**.

- **Seletor de teste por posição acertava o vizinho em vez de ficar vazio.** A
  prova dos idiomas abria Configurações pela barra com `… >> nth=13`. O botão
  «Config» saiu da barra em 02/09, com o caminho do menu conferido no lugar — e o
  seletor **não ficou vazio**: passou a acertar o 14º dos 23 botões, hoje
  «Restaurar». Cinco passos reprovavam há dois dias esperando dez segundos por
  bandeiras numa tela que não tem nenhuma, e a parte levava 1m38s para dizer
  isso; hoje leva **12,1 s**. O caso 17 da parte `tela` **já afirmava** que o
  Config estava no menu: duas baterias discordavam há dois dias, e a certa era a
  que falhava alto. O item passa a ser achado pela **chave** da fábrica de
  idiomas. E o diagnóstico que veio primeiro estava errado sobre este defeito e
  **certo sobre outro**: o `id` repetido é inofensivo numa região só, mas com a
  tela **dividida** as duas ficam anexadas e `querySelector` só desenhava a
  primeira — seis bandeiras numa paina e **zero** na outra.

- **A bancada do cluster saía cara ou coroa, e o commit culpado não era
  culpado.** As três asserções que reprovavam tinham **uma** causa, e não era a
  que parecia: a mensagem `NAO promovo` nunca foi reescrita (`git log -S` mostra
  um commit só, o que criou o cluster), e a bancada rodada sem tocar em nada
  passou verde. O que decide é a **ordem das mortes**: a eleição conta quem
  pulsou dentro da janela e o silêncio do master sai do mesmo relógio, então um
  par que morre **depois** do master ainda está dentro da janela quando o master
  é declarado calado — e o nó que sobra vê **2 de 3** e se elege. **Não há
  split-brain nem perda de dado**, e está medido: o portão é `escrita_liberada`,
  recalculado a cada 500 ms e independente do papel; nas duas ordens a escrita
  foi recusada e os retratos SHA-256 ficaram idênticos. O que a fresta custa é a
  **liderança** — os dois nós que juntos *eram* a maioria voltam seguindo o que
  esteve sozinho. Ela ganhou roteiro próprio (`bancada/cluster/fresta.py`, que
  **mede** sem afirmar) e o conserto no motor está em **parecer, não feito**.

- **Dois testes-sentinela dispararam como projetados, e ninguém foi colher.** O
  do `ponta-a-ponta` anunciava o próprio disparo — *«o dia em que o motor a
  impuser, eles falham»* — e o pedido 171 impôs a chave. O do `transacoes`
  afirmava que a leitura **não** vê o que a transação empilhou, e o pedido 162
  trocou isso em 02/09. Os dois reprovavam há dois dias porque a bateria inteira
  não era rodada. Virados para o comportamento de hoje com a mesma disciplina, e
  a recusa passa a ser conferida pelo **código `SP000008`**, nunca pela frase.

- **Duas guardas do catálogo apontavam para código que não existe mais.** O
  campo `fks_conferidas` saiu numa refatoração que **aposentou corretamente** a
  guarda que a motivou e deixou **duas irmãs** citando o campo morto. Corrida
  cheia: **77 guardas, 73 provadas, 0 quebradas** (era 71 + 2). E havia um
  terceiro irmão, num comentário de teste que ainda descrevia o campo **no
  presente** — datado como história, sem reescrevê-la.

- **Três pacotes intactos reprovavam, e o segundo conferidor passava verde.**
  `./empacotar.sh conferir` reprovava `dossie`, `conhecimento` e `kit` — todos
  de 30/08 — com **duas divergências por arquivo**, uma `A MAIS` e uma `FALTA`,
  e nenhum byte errado: era a **grafia do caminho**. A receita que grava o
  `MANIFESTO.sha256` existia em **quatro** lugares do `empacotar.sh`; o conserto
  que tira o `./` do `find .` entrou em **um** e os três irmãos ficaram com a
  receita velha. Ninguém viu por cinco dias porque o outro conferidor que o
  pacote oferece, `sha256sum -c`, **aceita** o `./` e passava verde: *conferidor
  que discorda de conferidor não acusa, acalma.* Hoje há **uma** receita —
  `dossie()`, `conhecimento()` e `kit()` chamam `fecha()` —, os **oito** pacotes
  saem `INTEGRO`, e a prova entrou na bateria como parte `pacote`, valendo nos
  dois sentidos: a receita antiga, reposta de propósito, tem de reprovar.

- **O MANUAL prometia o que a pétrea proíbe, e ninguém executava os exemplos
  dele.** A seção de chave estrangeira listava as quatro ações numa tabela só,
  sem dizer o lado — e `ao_excluir` aceita **só** `restringir`, recusando na
  declaração. O exemplo do `declarar_fk` mandava `"ao_excluir":"cascata"`:
  **quem copiasse do manual tomava erro**. Mais três: dizia que o motor «ainda
  NÃO impõe» a chave (falso desde o pedido 171), dizia «ausente é restringir»
  sem separar os lados (no `ao_alterar` o padrão é **cascata**), e omitia o
  `"verificar"`, o nasce-conferida e a exigência de índice dos dois lados.
  **Só um dos quatro tinha envelhecido — os outros três nasceram errados**, e
  sobreviveram porque nunca ninguém rodou o que o manual manda rodar. Hoje
  roda: `bancada/manual/provar-manual.py` sobe um servidor próprio, executa os
  exemplos, confere **12** afirmações e é derrubado pelo PID; entrou na bateria
  única. Prova real nos dois sentidos — reposto o exemplo antigo, o motor
  responde *«"ao_excluir": "cascata" não existe no PhxSql»* e a prova reprova.
  Duas armadilhas ficaram escritas na própria prova, porque as duas passaram
  por engano na primeira versão: **conferir o veredito em vez do motivo** (dois
  `ok:false` eram «acesso negado», não a FK) e **depender da ordem das
  corridas** (a segunda reprovava por «chave duplicada»).

- **A ligação de DbLink sem base padrão listava zero tabelas no MySQL®, calada.**
  O dialeto perguntava `TABLE_SCHEMA = DATABASE()`, e sem base padrão
  `DATABASE()` é **NULO** — em SQL `x = NULL` nunca é verdadeiro. Como o ramo só
  é alcançado quando *não* há base padrão, **ele estava sempre vazio**. Não era
  caso de laboratório: `dblink_salvar` aceita `database` vazio, e num servidor
  de MySQL® uma conexão enxerga todas as bases, o que faz da ligação sem base
  padrão a forma natural de navegar por várias. A tela escapava por sorte
  (`DBL.database = … || bases[0]`); quem fala pela porta de dados, pelo MCP ou
  por script, não. É o **gêmeo** do defeito que a prova contra o PostgreSQL®
  achou, e pelo mesmo motivo: uma consulta montada com um qualificador que não
  existe.

- **A tela do DbLink dizia que TODA coluna do PostgreSQL® era obrigatória.**
  Ela lia a estrutura **por posição**, e as posições do `SHOW FULL COLUMNS`
  (nove colunas) não são as do lado do PostgreSQL® (seis). A posição 3 é o
  *Null* no MySQL® e a **chave** no PostgreSQL®: como `'PRI' != 'YES'`, as
  quatro colunas que aceitam nulo apareciam como obrigatórias. Isso é
  **mentira sobre o dado** — quem olha não tem como saber que o banco diz outra
  coisa. Na mesma folha: «chave» mostrava o padrão, «extra» e «comentário» liam
  fora da linha e a tela imprimia a palavra `undefined`, e a chave primária
  aparecia como *duplicado ok* porque a coluna do «único» lia o nome da coluna.

  Consertado pelo padrão que a casa já tinha para o texto de tela:
  **resolve-se por CHAVE, nunca por posição.** Os nomes são os do
  `SHOW FULL COLUMNS` e do `SHOW INDEX` — contrato publicado do MySQL® —, e o
  dialeto apelida o lado do PostgreSQL® com eles no lugar dos `case` e
  `coalesce` que o servidor inventava, e que vinham **repetidos**. O ramo do
  MySQL® **não muda uma letra**. Um valor mudou, e vale dizer qual: a
  **polaridade do único** — o MySQL® publica `Non_unique`, que vale **0 quando
  o índice É único**, e o ramo do PostgreSQL® publicava 1.

- **Quatro documentos publicavam números digitados, e todos envelheceram.** O
  README dizia **390 testes** no motor e **619** no projeto (são **1.440**); o
  `docs/TESTES.md` dizia **1.229** — na mesma frase em que afirma «somado dos
  `test result:` de uma rodada, e não digitado»; o `docs/REST.md` dizia **113
  operações** na linha seguinte a «especificação digitada à mão envelhece na
  primeira operação nova».

  O número real é **120**, e ele aparecia como **108** no `PENDENCIAS.md` e
  **121** no catálogo que a auditoria leu do binário: **quatro valores,
  nenhum certo**. *Os documentos que pregam a regra eram os que a violavam.*

- **O README marcava restauração e transações como pendentes** havia rodadas.
  As duas estão prontas desde os pedidos 134 e a frente 37.

- **`--version` não respondia em binário nenhum**, e cada um falhava de um
  jeito: `phxsql` dizia «comando desconhecido», `phxsqld` tentava **ler o
  `config.json`**, e `phxsqlcmd` tentava **conectar num servidor** — só para
  dizer quem era. Perguntar a versão é a primeira linha de todo roteiro de
  operação, e não pode exigir ambiente montado.

### Adicionado

- **Leitor deixa de esperar leitor no `varrer`.** A trava de dados passou de
  `Mutex` a `RwLock`, e a leitura de grade — a única operação movida, por
  decisão — atende agora com uma ficha **compartilhada**. Medido em quatro
  baterias limpas com o binário de antes guardado: quatro clientes lendo a
  mesma tabela rendiam **1,59×–1,76×** sobre um e passam a render
  **3,81×–3,93×**; em vazão, **3.412 → 7.465 op/s** (2,19×) e
  **3.349 → 7.708 op/s** (2,30×), com o `ping` de um cliente ancorando as duas
  baterias a 2,4% de distância. **O cliente sozinho ficou igual** (0,97× e
  0,98×), que é o esperado: sem disputa as duas fichas custam o mesmo. A
  escrita não pagou a conta (0,95×–1,06×).

  A garantia é **do compilador, não de convenção**: `RwLock<Instancia>`
  continua não compilando — o marcador `!Sync` está lá para isso —, e o que
  entrou na trava é a `Raiz`, que separa as duas fichas pelo tipo do
  empréstimo. A ficha compartilhada devolve uma `TabelaLeitura` **sem um único
  método de escrita**, provada por um par de doctests (`compile_fail` mais o
  controle que tem de compilar, senão o primeiro passaria por erro de
  digitação).

  E o achado que quase custou a entrega: **abrir uma tabela para LER escreve**
  — em quatro lugares dentro do próprio construtor (criar o `.trash` e o
  `.reason` que faltam, curar o `.log`, terminar uma troca de volume
  interrompida), mais o espelho `.bkp` e a trilha de dado pessoal fora dele.
  São seis escritas num caminho que todo mundo chama de leitura, e a fachada
  teria compilado sem cobrir nenhuma das quatro primeiras. Hoje cada uma vira
  recusa nomeando o componente, e a tabela recuada é atendida pela ficha
  exclusiva **exatamente como antes** — `docs/CONCORRENCIA.md` §16.

- **`CAPABILITIES.json`**, gerado: versão, commit, árvore suja, branch, testes,
  operações, crates, linhas e idiomas. É a recomendação da §15 da auditoria, e
  ela está certa pelo motivo que esta casa já conhecia — enquanto cada
  documento guarda a própria cópia do número, eles divergem.

- O `numeros-do-projeto.py` passou a escrever **fora do dossiê**: README,
  `docs/TESTES.md` e `docs/REST.md`. Ele já contava o que o `cargo test`
  **reporta** (e não `grep #[test]`) e **aborta se a suíte falhar** — que é o
  portão que a auditoria pediu; só faltava alcançar esses três.

- **Commit embutido nos binários** por um `build.rs` de doze linhas, sem
  dependência nenhuma, com a marca **`-sujo`** quando a árvore não corresponde
  ao commit anunciado. Versão sem commit não identifica build: dois pacotes
  «0.18.0» podem ser árvores diferentes.

- Guarda `version_responde_sem_config_e_sem_servidor`, que roda o binário num
  diretório **sem** `config.json` — que é exatamente onde o defeito aparecia.

### Sabido

- **O que a auditoria aponta e não é novidade:** a chave estrangeira é
  declarativa e não aplicada. Já estava escrito no pedido 127 — «um teste trava
  que *declarar não é aplicar*». É limitação conhecida e documentada.

- **O que caducou entre a auditoria e hoje:** ela lista o DbLink como não
  provado contra um PostgreSQL® real. Foi provado nesta mesma rodada, e a prova
  achou três defeitos.

- **O que fica para decisão do dono**, com o custo na mesa e sem começar por
  conta própria: aplicar FK em todos os caminhos de escrita, trocar a trava
  única global por travas por tabela, e TLS de verdade no lugar da cifra
  própria do fio. São três frentes arquiteturais, e nenhuma cabe numa rodada.

---

## Não lançado — o DbLink provado contra um PostgreSQL® de verdade

### Corrigido

- **`dblink_tabelas`, `dblink_estrutura` e `dblink_ler` estavam quebrados
  contra PostgreSQL®** — e dois deles **em silêncio**. Uma causa só, no
  chamador: `base` quer dizer o *database* no MySQL® (onde database **é** o
  esquema) e o **esquema** no PostgreSQL®, onde a conexão já está dentro do
  database. `base_escolhida` entregava o database nos dois.

  Os sintomas: lista de tabelas **vazia sem erro**, colunas **vazias sem
  erro**, e `relation "bancada_phx.clientes" does not exist`.

  **O pior caso era o da tela**: ela lista os bancos com `dblink_bancos` — que
  no PostgreSQL® devolve *bancos* — e manda o escolhido de volta no campo
  `database`. A grade do DbLink com PostgreSQL® mostrava **nenhuma tabela**,
  calada.

- **Um teste podia reprovar pelo comportamento certo.** O
  `ip_bloqueado_tem_a_proxima_conexao_recusada_e_soltar_devolve` falhou uma vez
  num `--workspace` com `BrokenPipe`, e passou 8 de 8 isolado nos dois perfis.
  Não é azar: o servidor recusa o IP bloqueado **antes de ler o pedido** —
  escreve a recusa e fecha —, e o teste só tolerava uma das duas formas de o
  sistema expressar isso. Com a máquina carregada, o fechamento ganha a corrida
  do `write`. As asserções sobre a resposta continuam idênticas: ela já está no
  soquete.

### Adicionado

- **`bancada/dblink/prova-postgres.py`** — as cinco operações contra um
  PostgreSQL® **16.13 real**, com **19 conferências, cada uma contra o
  `psql`**. O oráculo tinha de ser o cliente oficial do outro motor: conferir
  contra o que o script espera provaria só que o script e o servidor
  concordam.

  Duas armadilhas que o `docs/DBLINK.md` já nomeava passaram de previstas a
  **vistas acontecer**: o booleano chega como `t`/`f` e não `1`/`0`, e o
  `reltuples` de tabela nunca analisada é **`-1`** da 14 em diante — o DbLink
  publica `0`.

- Os testes que travam a volta: `no_postgres_a_base_da_ligacao_nao_vira_esquema`
  e, ao lado, **`no_mysql_nada_muda`** — o teste do comportamento *velho*, que é
  o que mais importa numa mudança destas.

### Mudado

- O `docs/DBLINK.md` perdeu a seção «o que ainda falta provar» e ganhou a
  tabela do que `database` quer dizer **por motor**.

### Sabido

- **A premissa que mantinha o pedido 86 parcial havia rodadas caducou**: o
  documento dizia «não há PostgreSQL® instalado nesta máquina», e há. *A lista
  do que falta também é palpite até alguém medir* — inclusive quando o palpite
  é nosso.

- **Fica de fora**: repetir contra outras versões. O `unnest(...) WITH
  ORDINALITY` pede 9.4+ e o `reltuples` mudou na 14; o código trata os dois, e
  o provado é a 16.13.

---

## Não lançado — os três motores no mesmo trabalho, a um milhão de linhas

A terceira bancada de comparação. Ela existe porque somar as duas que já havia
daria **três colunas e nenhuma comparação**: medidas de dias diferentes
carregam o ambiente junto, e parte da diferença deixa de ser do motor.

### Corrigido

- **A regra 1 da bancada estava sendo violada, e nenhum tempo denunciava.** A
  `bancada/medir.py` grava `'2024-10-04'` em **toda** linha, enquanto o
  `carga.rs` e a bancada do SQLite(R) gravam `20000 + (i % 400)`. Dado
  diferente, do mesmo tamanho — invisível em qualquer medida de tempo. O que o
  achou foi ter de conferir **três** motores em vez de dois.

- **O rótulo do valor caía por cima do bigode** no gráfico, e o conserto dele
  criou o defeito seguinte: empurrado para depois do bigode, o texto saía
  **fora do painel** e `12,3 s (pico 17,1 s)` era publicado como `12`. Hoje ele
  só vai para fora se couber; se não couber, entra na barra, alinhado à direita.

- **Uma rodada fora da curva esmagava o painel inteiro.** Com o eixo ancorado
  no máximo, os 22,97 s de uma rodada do `UPDATE` do MySQL(R) faziam as barras
  de 277 ms e 1,03 s virarem lascas de 3 px. O eixo passou a ser a maior
  **mediana**, com o bigode cortado por uma seta quando estoura — a excursão
  continua dita, no rótulo, em vez de mandar no desenho.

- **O contorno declarava vencedor com as faixas sobrepostas** — 164 ms contra
  166 ms, faixas em 151–215 e 158–232. Marcar um dos dois é publicar ruído da
  máquina como resultado. E a regra do vencedor estava em **duas cópias**:
  consertei a do gráfico e a tabela do dossiê continuou marcando vencedor na
  busca, com o documento se contradizendo a dois centímetros de distância. Hoje
  ela mora num lugar só, e o dossiê a importa de lá.

- **A nota do `UPDATE` dizia «trocar o valor de uma coluna»** quando os três
  regravam a linha inteira, e a **página não dizia quantas operações eram** —
  quem lia via «164 ms» sob o título «1.000.000 linhas» e entendia que achar
  *uma* linha custava isso.

- **Ponto decimal numa página em português.** O gráfico escrevia `9.93 s`, que
  quem lê em português lê como nove mil e noventa e três.

- **Três seções do `docs/DESEMPENHO.md` tinham o mesmo número** (`4.12`), com
  **17 citações** espalhadas pelo código, pelos LEIA-ME, pelo CHANGELOG e pelos
  sprints apontando para lá. Referência ambígua não avisa que é ambígua: leva à
  seção errada e o leitor acredita. As duas últimas viraram **4.13** e **4.14**,
  e as quatro citações que as queriam foram atrás.

### Adicionado

- **`bancada/comparacao/`** — PhxSql × MySQL(R) 8.0.46 × SQLite(R), três
  rodadas, tabela de 1.000.000 de linhas, 20.000 operações nas fases pontuais,
  os três **intercalados na mesma rodada**.

  Medianas: inserir **9,93 s** contra 2,56 do SQLite(R) e 12,34 do MySQL(R);
  buscar **164 ms** contra 166 e 2,48 s; atualizar **277 ms** contra 1,03 e
  3,54 s; excluir **1,05 s** contra 574 ms e 4,06 s.

- **A fase `conferir` do `carga.rs`**, que não mede tempo — mede se os motores
  chegaram ao **mesmo estado**. Contagem, soma de `valor` e soma de `cadastro`,
  em três marcos: depois de inserir, depois de atualizar e depois de excluir. O
  marco do meio não é enfeite: `atualizar` e `excluir` mordem os mesmos 20.000
  alvos, então no marco final o efeito do `atualizar` já sumiu junto com as
  linhas excluídas.

  Os totais conferem contra a **forma fechada** calculada à parte
  (410.099.600.000 e 20.199.500.000), e não só entre si. Divergiu, a bancada
  **recusa publicar**.

- **A medição do piso do formato.** Os três não têm a mesma forma e não há como
  dar: o SQLite(R) é biblioteca em processo, o `carga` também, e o MySQL(R) é
  daemon que recebe **texto** por soquete — não existe MySQL(R) embutido nesta
  máquina. Então mede-se: 20.000 instruções que não fazem nada (`DO 1;`) custam
  **1,479 s**, que são **59,6% da barra de busca dele**.

  Sem esse número teríamos publicado «15,16× mais rápido»; entre motores são
  **6,12×**. Mais da metade da vitória era do formato.

- **O sexto gerador do dossiê** (`docs/dossie/trio-de-motores.py`). Ele **não
  redesenha nada** — insere o SVG que o `grafico.py` produz, porque duas
  receitas para a mesma figura divergem —, e **recusa** se a figura for mais
  velha que a medição: um gráfico desenhado da corrida anterior publica o
  passado com data de hoje, e nada no desenho denuncia isso.

- **O modo `--so-prosa`** do medidor, que refaz as ressalvas a partir dos
  números já guardados, **sem remedir**. Sem ele, corrigir uma palavra do texto
  que aparece na página custaria quinze minutos de bancada — e o atalho seria
  editar o JSON à mão, que é como número gerado vira número digitado.

### Mudado

- `docs/DESEMPENHO.md` ganhou a **§13**, e a seção da bancada no dossiê passou a
  se chamar «dez milhões de linhas, **e os três motores a um milhão**».

### Sabido

- **Onde perdemos, dito no mesmo tamanho de letra:** a inserção para o
  SQLite(R) por **3,88×**, a exclusão por **1,83×**, e o disco — **253,6 MiB**
  contra 57,3 e 104,0, que é **4,42×** e **2,44×**. É o preço do modelo de
  arquivos separados, e no celular essa é a pergunta inteira. A busca
  **empata**.

- **A carga inicial não tem a mesma forma nos três**, e a do MySQL(R) é a mais
  barata por linha (20 instruções de 50.000 contra um milhão de chamadas) — a
  barra dele nessa fase é **otimista**, e isso está nas ressalvas do JSON e da
  página.

- **O SQLite(R) publicado é a variante `rowid`**, que casa com o InnoDB e é a
  que **nos desfavorece**. A `2ind`, que estruturalmente se parece com o nosso,
  é mais lenta em todas as quatro fases (1,04× a 1,31×) e corre na mesma rodada,
  guardada no JSON.

- **Durabilidade casada** — uma sincronização por fase nos três. Não é o regime
  de quem grava pedido a pedido.

---

## Não lançado — o PhxSql embutido: o motor como biblioteca, com ABI de C
## Não lançado — as transações

O pedido é *transações*. A rodada anterior entregou o **pré-requisito** e o
**desenho escrito antes do código**; esta entregou o código, e ele obedece o
desenho — com uma seção reescrita, e o motivo dito.

### Adicionado

- **`BEGIN` / `COMMIT` / `ROLLBACK` / `SAVEPOINT`**, pelo protocolo e pelo SQL.
  Três sinônimos de abertura (`BEGIN`, `BEGIN TRANSACTION`,
  `START TRANSACTION`), `ROLLBACK TO SAVEPOINT` com a palavra do meio
  facultativa, e `RELEASE SAVEPOINT`.

  **Nada vai a disco antes do `COMMIT`.** A transação empilha o conjunto de
  escrita em RAM; o `ROLLBACK` joga a lista fora. **Zero slot queimado, zero
  rowid consumido, a ordem de digitação intacta** — que é a regra pétrea desta
  casa, e a razão de o desenho ser este e não outro.

- **A abertura declarada, com parâmetros nomeados e cláusulas sem ordem:**

  ```sql
  BEGIN TRANSACTION
    SCOPE (clientes, pedidos, pediditens, estoque)
    SCOPE MODE STRICT      -- DYNAMIC é o padrão
    TIMEOUT 5s
    LOCK TIMEOUT 500ms
    STATEMENT TIMEOUT 2s
    LOCK MODE AUTO;        -- AUTO, ROW, TABLE ou EXCLUSIVE
  ```

  A forma posicional foi recusada e o motivo está escrito: ela não estende
  (onde caberia o segundo prazo?) e mistura tabela com duração na mesma lista.

- **A máquina de estados com `ABORT_ONLY`.** Depois de um erro de TRANSAÇÃO o
  `COMMIT` **recusa** dizendo que a transação não pode ser confirmada, em vez
  de confirmar trabalho meio inválido. Em `ABORT_ONLY` até a leitura recusa,
  como no PostgreSQL(R) — e `ROLLBACK TO SAVEPOINT` **não resgata**, porque
  aqui erro de instrução não aborta nada, então só chega ali o que põe em
  dúvida o próprio conjunto de escrita.

- **Duas classes de erro, e o erro diz qual é** — pelo código, e não pelo
  texto: `4005 EM_TRANSACAO` (acesso, `repetir: true`) e
  `6002 TRANSACAO_ABORTADA` (execução). A classe sai da **faixa do código**, e
  não de uma lista escrita à mão: erro novo cai na classe certa sozinho.

- **A marca `transacao_<id>.tx`** e a recuperação que **anda para a frente**.
  Ela é sincronizada antes de a passada tocar em qualquer arquivo de dado, e é
  o **ponto de compromisso**: antes dela a transação não aconteceu, depois dela
  aconteceu. Formato em `docs/FORMATO.md` §16, com CRC por operação — uma marca
  que não confere é um commit que **nunca começou**.

- **O relatório de recuperação no arranque**, e ele só imprime o que mede: não
  há linha de «páginas refeitas», porque não há página suja confirmada para
  refazer. E ele **não sai quando não há marca nenhuma** — um bloco dizendo
  zero em toda subida treina quem opera a não ler o relatório.

- **A tela de Gestão de transações**, reescrita inteira: o nível de isolamento
  pelo nome certo, quem está segurando o quê (declarado **e** efetivo,
  separados), e o que continua não existindo. 43 chaves novas na fábrica de
  idiomas, nos seis idiomas.

- **`recursos.transacao_prazo_min`, `transacao_max_linhas`,
  `transacao_lock_timeout_ms` e `transacao_statement_ms`** no `config.json`, no
  MANUAL e na tela — e **os quatro são lidos**.

### Mudado

- **A §4.2 do `docs/TRANSACOES.md` foi reescrita, e a decisão anterior ficou
  registrada.** O desenho escolhia **reserva de tabela sem espera**, e o
  argumento era forte: sem espera não há grafo de espera, e sem grafo não há
  ciclo. O que a derrubou foi um conflito **artificial** — quinhentos caixas
  vendendo, um no pedido 9001 e outro no 18223, sem disputa nenhuma de verdade.

  Entrou a hierarquia de duas alturas: **intenção na tabela, exclusiva na
  linha**. E o que paga pela volta da espera é a **declaração prévia do
  escopo**: com as tabelas conhecidas na abertura, elas são tomadas sempre na
  mesma ordem canônica, e o ciclo entre tabelas deixa de existir.

  **A garantia não é total, e está dito:** a ordenação mata o ciclo entre
  TABELAS; entre LINHAS da mesma tabela ele continua possível, e a resposta é o
  `LOCK TIMEOUT` — espera limitada e erro nomeado, nunca uma thread pendurada.
  **Este motor não promete «sem deadlock».**

- **O `INSERT` trava o FIM da tabela**, e não uma linha: o próximo slot é
  `slots() + 1` e ele é um só. Sem essa trava, duas transações que anexam ao
  mesmo tempo preveem o mesmo rowid e a segunda descobriria isso na passada de
  commit, com metade do trabalho gravado.

- **DDL dentro de transação é recusado, e não confirma a transação pelas
  costas.** O MySQL(R) e o Oracle confirmam, e é uma armadilha conhecida: quem
  escreveu `BEGIN; …; CREATE TABLE; ROLLBACK` acha que desfez e não desfez.

- **O detector de transação passou a vir antes do de rotina na op `sql`.** O
  detector de rotina analisa o texto pelo léxico comum, e o léxico recusa
  `500ms` — número colado em identificador. Ele erra antes de o de transação
  ser consultado, e um `LOCK TIMEOUT 500ms` nunca chegaria lá.

### Corrigido

- **Uma escrita comum podia anexar no slot que a transação já tinha
  prometido**, e o estrago não era o erro visível no `COMMIT`: era a
  **recuperação** encontrar aquele slot ocupado pela linha do outro, tratá-la
  como «já aplicada» e **descartar a nossa em silêncio**.

  A trava do fim da tabela era pedida *depois* de a trava de dados sair, e o
  rowid previsto era calculado com ela na mão — uma fresta de milissegundos
  entre prever e proteger. Hoje as travas vêm **antes** da trava de dados: o
  que elas precisam saber (a tabela e o rowid alvo, ou o fim, no anexar) sai do
  pedido e não do esquema, então cabem ali. Achado pela revisão, não por teste.

- **Dois testes de prazo mediam o relógio da máquina, e não o servidor.**
  Abriam com `TIMEOUT 1ms` e dormiam 30 ms; com a bateria inteira em paralelo,
  a **primeira** inserção — a que precisa passar — já chegava atrasada, e o
  teste reprovava na linha errada, acusando código correto. Vencer a transação
  movendo o relógio dela prova o mesmo caminho sem corrida nenhuma.

- **O executor das guardas dava veredito de mentira quando duas rodadas
  corriam juntas.** A cópia mora num caminho fixo — de propósito, porque é o
  que guarda o `target/` quente —, e uma rodada encontrava o defeito plantado
  pela outra: quatro vereditos falsos numa rodada só, todos com cara de
  «entrada do catálogo envelheceu». Agora o executor **tranca** a cópia com um
  `flock` e a segunda rodada espera a primeira.

  Descontada a contaminação sobraram duas entradas **de verdade** envelhecidas
  (`aad-fora-do-slot`, `endereco-fora-da-amarracao`): a cifra do slot virou
  função livre e o `rustfmt` recolheu a chamada. Atualizadas, e a amarração do
  slot cifrado ao endereço voltou a estar provada.

- **A recuperação não conseguia completar um commit depois de um `SIGKILL`, e
  quem achou foi a prova por SOQUETE.** A queda no meio da passada levanta a
  marca de «o índice ficou para trás», e enquanto ela estiver lá **toda**
  operação de índice recusa. A recuperação reabria a tabela, tentava inserir e
  recebia «reconstrua com reparar índice»: o commit ficava pela metade e a
  tabela inutilizável até alguém reparar à mão — **sem ninguém ser avisado**,
  porque o servidor subia normalmente.

  Agora ela reconstrói o índice antes de completar, e o relatório **conta**
  quantos foram reconstruidos. Nenhum teste unitário via isso, e a entrada
  `recuperar-sem-reindexar` do catálogo de guardas afirma exatamente isso.

- **A chave única de uma escrita recusada pela trava ficava na lista.** A
  tentativa seguinte, com a mesma linha, era acusada de duplicada **por si
  mesma**. Perguntar e guardar viraram duas coisas separadas: a chave só entra
  depois de a escrita estar empilhada de verdade.

### Medido

- **O *group commit*: 2,63×, e o passo seguinte morre.** Receita de fora se
  mede contra o nosso gargalo antes de virar plano, e o critério de morte
  (1,5×) foi acordado **antes** da medição.

  A decomposição de um commit de uma linha: **1,199 ms** no total, dos quais
  0,289 ms são a marca e 0,050 ms o trabalho — sobravam **0,860 ms, 74% do
  commit, só no `fsync` da tabela**. Uma inserção solta, sem transação, custa
  0,061 ms, justamente porque passa pela janela de durabilidade.

  **A receita mirava outro gargalo:** *group commit* clássico amortiza `fsync`
  entre commits **concorrentes**, e a trava única deste servidor nunca tem dois
  em voo para agrupar. O que havia para amortizar era o `fsync` da tabela
  contra a janela que já existia — e adiá-lo é seguro porque **quem decide se a
  transação aconteceu é a marca, não o `fsync`**. 1,199 → **0,457 ms**.

  E o passo seguinte — agrupar o `fsync` das *marcas* — **morre medido**: o
  piso irredutível é 0,341 ms contra os 0,455 de hoje, **1,34×**, abaixo do
  critério. A marca não se adia; ela é o ponto de compromisso.

- **`LOCK MODE AUTO` contra `EXCLUSIVE`, 64 caixas em linhas diferentes:**
  50,5 ms contra 78,4 ms, **1,55×**. Nenhum dos dois perde trabalho — o
  `EXCLUSIVE` não recusa ninguém, ele serializa. O número é o preço de uma
  disputa que não existia.

- **Otimista contra pessimista, 64 clientes na MESMA linha:** o otimista
  (`versao`) levou 28,0 ms gastando **133 tentativas** para 64 gravações; o
  pessimista levou 71,8 ms gastando **exatamente 64**. Nenhum dos dois é o
  certo sempre, e é a subida dessa razão — não o relógio — que diz quando
  trocar.

### Sabido

- ***ACID compliant* continua falso, e mudou de motivo.** O **A** e o **I**
  passaram a existir; o **D** já existia. O que segura a frase é o **C**: a
  integridade referencial **não é imposta**, e há teste travando isso.

  E o **I** só se escreve com o nome certo: *escrita serializável por tabela,
  leitura confirmada e não bloqueante, sem leitura repetível*. **Não é ANSI
  SERIALIZABLE.**

- **A transação não vê as próprias escritas**, e não vai ver nesta rodada:
  exigiria sobrepor o conjunto de escrita em todo caminho de leitura, que é o
  antipadrão do «portão espalhado por quarenta operações».

- **Tabela com partição alfanumérica recusa `INSERT` dentro de transação.** Ali
  o slot depende do balde, e o balde sai de uma regra que mora dentro do
  `Table`; sem o rowid alvo a marca não é idempotente.

- **O `STATEMENT TIMEOUT` morde nos pontos de cancelamento que existem** — os
  laços longos que já chamam `Atividade::siga`. Uma inserção de uma linha não
  tem ponto de cancelamento no meio, e não poderia ter.

- **Um caso da recuperação continua sem conserto, e ele está nomeado:** se a
  passada gravou o slot e depois o liberou, o `.reg` não o reaproveita e a
  linha não volta. A recuperação **não esconde**: a operação entra em
  `operacoes IMPOSSIVEIS` com a tabela e o rowid.

---

## Não lançado — o terreno das transações, e o desenho delas

O pedido era *«um mini servidor para rodar no Android e no iOS off-line e se
conectar por TCP/IP com o servidor»*. O **objetivo** está certo e é o alvo
desta rodada; a **forma** foi corrigida, e a correção não é nossa: o iOS
proíbe processo de longa duração em segundo plano e app escutando porta para
outros apps, e o Android mata processo em segundo plano com liberdade. A forma
que os dois apoiam é biblioteca embutida no processo do aplicativo — sem
porta, sem daemon.

E a conclusão que veio antes de qualquer código: **o `phxsql-store` já é o
banco embutido**; o `phxsql-server` é um envelope de rede em volta dele. Esta
rodada não reescreveu motor — **expôs o que existe** por uma ABI de C.
`phxsql-server` não foi tocado.

### Corrigido

- **«Não há essa linha» voltava de duas formas diferentes conforme o motivo.**
  Achado pelo programa em C na **primeira rodada dele**, não lendo o código:
  slot livre devolve `Ok(None)` e rowid além do fim devolve `NaoEncontrado`.
  A diferença é real dentro do motor e invisível para quem chama — o
  aplicativo mostraria caixa vermelha para metade dos «não achei» e lista
  vazia para a outra metade. `phx_ler` e `phx_versao_da_linha` dobram as duas
  em `PHX_NAO_HA`; o `phx_buscar` **não** dobra, porque lá o mesmo 3001 quer
  dizer «esse índice não existe».

- **No ARM64, ligando à mão, o `catch_unwind` era enfeite.** Sem
  `--eh-frame-hdr` o binário sai sem `PT_GNU_EH_FRAME`, o desenrolador não
  acha a tabela de FDE, e todo pânico capturado vira `fatal runtime error:
  failed to initiate panic` seguido de aborto. A garantia central da camada
  sumia **calada**, por uma bandeira do ligador. O `cc`, o `clang` e o Xcode
  passam sozinhos; um script de ligação próprio pode não passar — e é o item
  mais importante para quem for escrever a camada JNI ou a de Swift.

- **«Quantas linhas tem a tabela» tinha três respostas e a ABI dava uma.**
  `phx_tabela_registros` contava slots ocupados em vez de consultar a visão:
  com exclusão suave, a tela diria 2 e listaria 1. A visão virou parâmetro.

- **Um teste que passava por engano.** O executor de guardas devolveu
  `NAO PEGOU` no `ffi-erro-global`: com a vaga de erro global — o defeito — o
  teste da vaga por thread **continuava passando**, porque os outros testes
  rodam em paralelo e o `limpar()` de qualquer um esvaziava a vaga bem a
  tempo. Trocado por uma ordem estrita entre duas threads.

### Adicionado

- **`crates/phxsql-ffi`** — `cdylib` (o `.so` que o Android carrega) **e**
  `staticlib` (o `.a` que a Apple exige, porque ela não aceita biblioteca
  dinâmica de terceiros dentro do app). **44 funções** exportadas, contadas
  com `nm -D`. O `.so` tem **1.155.480 B** — 962.664 B depois do `strip` —
  com B+tree, CRC-32, ChaCha20-Poly1305, SHA-256, JSON e diário dentro, o que
  é consequência direta da regra de zero dependências.

  A superfície: abrir e fechar base, construtor de esquema, criar e abrir
  tabela, inserir, atualizar (com e **sem** a janela de conflito), excluir de
  vez e suave, restaurar, ler por rowid, buscar por índice, cursor de
  digitação e de índice, verificar, e os **ganchos de replicação** — imagem no
  diário, posição, leitura de evento com imagem, `aplicar_evento` e a origem
  que mata o laço do bidirecional.

- **`crates/phxsql-ffi/include/phxsql.h`** — o cabeçalho de C, com as seis
  regras da fronteira escritas nele. Dois testes o mantêm honesto: um confere
  que biblioteca e cabeçalho declaram **as mesmas funções** (nos dois
  sentidos), outro que **nenhuma constante diverge** entre o Rust e o `.h` —
  uma que divergisse compilaria, rodaria, e gravaria a coluna com o tipo
  errado.

- **`docs/EMBUTIDO.md`** — o desenho, escrito **antes** do código, com as seis
  decisões justificadas: nenhum pânico atravessa (e o punho fica **envenenado**
  depois de um, porque capturar salva o processo e não conserta o objeto);
  erro em código de retorno com os **mesmos números** da porta de dados, mais
  último-erro **por thread**; quem alocou libera (a biblioteca nunca devolve
  ponteiro para o `free()` do chamador — em `.dll` do Windows isso derruba o
  processo); UTF-8 com tamanho explícito, nunca `NUL`-terminado, porque dado
  de cliente tem byte zero; e a segurança de thread dita com todas as letras,
  **inclusive o que não foi testado**.

- **`bancada/embutido/provar.sh` e `crates/phxsql-ffi/c/prova.c`** — um
  programa em C que liga contra a biblioteca de verdade e roda **três vezes**:
  contra o `.a` em x86-64, contra o `.so` em x86-64 (o formato do Android) e
  em **ARM64 sob `qemu-aarch64-static`**. **40 passos, zero falhas** nas três.

- **6 guardas novas** no `bancada/guardas/catalogo.py` (37 → **43**), todas
  PROVADAS. A `ffi-panico-atravessa` é a segunda de toda a lista que espera
  **aborto** em vez de falha: o tamanho do estrago é a prova.

### Sabido

- **A camada JNI (Android) e a Swift/ObjC (iOS) não existem** — só o desenho,
  em `docs/EMBUTIDO.md` §10. O NDK não está nesta máquina (o alvo
  `aarch64-linux-android` compila e falha no ligador) e o SDK da Apple só
  existe em macOS. Escrever código que não se pode ligar nem rodar seria
  entregar promessa: *«compila» não é «rodou»*.
- **O cliente de sincronia não existe** — os ganchos existem; o laço que
  decide *quando* sincronizar é decisão de produto.
- **O desempenho num aparelho de verdade continua sem medida.** O que se mede
  sob `qemu-user` é o custo da emulação.
- **A ABI não fala SQL** e não gerencia usuários: dentro do processo do
  aplicativo não há a quem negar. Permissão continua sendo do servidor
  central, do outro lado da rede.

---

## Não lançado — o terreno das transações, e o desenho delas
## Não lançado — o webservice REST, e o terreno das transações

Duas frentes. O **webservice REST com OpenAPI** (pedido 149) entrou inteiro,
com a especificação saindo da tabela de despacho em vez da mão. E o pedido de
*transações* recebeu o **pré-requisito** e o **desenho escrito antes do
código** — e não meia transação, que é o que a pressa produziria.

### Corrigido

- **A trava de dados tinha 13 tomadas fora do ponto único, e o comentário do
  `travar_dados()` afirmava ser «o único lugar que a toma».** Era mentira
  medida, e a consequência não era estética: o `espera_ms_s` da telemetria
  mostrava a fila de uma parte, e as três piores ausências eram exatamente a
  atividade longa que o painel existe para mostrar — o despejo do cache, o
  corpo de um gatilho e o laço da replicação **atravessando uma ida e volta
  de rede**. As 13 entraram, uma a uma, cada uma respondendo «quem chama isto
  já tem a trava?». A tabela com a resposta de cada uma está em
  `docs/TRANSACOES.md` §8.1.

- **Pedir a trava que a própria thread já tem parava o servidor inteiro.**
  `std::sync::Mutex` não é reentrante, e o abraço mortal já aconteceu **três
  vezes** neste projeto — a última em configuração padrão, com escrita comum
  em duas tabelas. Não havia log, não havia pilha, e as outras conexões
  paravam junto, porque a trava fica presa na thread pendurada. Agora
  `travar_dados` pergunta antes, numa `Cell` de thread, e devolve erro
  nomeado: o pedido culpado falha dizendo o que houve, e o servidor continua
  atendendo.

- **A recusa por lista negra era engolida por um `RST` e virava «connection
  reset».** Cinco tokens errados bloqueiam o IP — certo. O pedido seguinte
  devia trazer o `403` com o motivo e o prazo; trazia
  `ConnectionResetError: [Errno 104]`. A causa é do TCP: fechar um soquete com
  bytes por ler no buffer de recepção faz o sistema mandar um RST, e o RST
  **descarta a resposta em voo**. As recusas por bloqueio e por IP fora dos
  permitidos respondem **antes** de ler o pedido, de propósito — e é por isso
  que o corpo fica sem ler. Valia para **todas** as portas HTTP, e valia desde
  que a interface web existe: a recusa cuidadosamente redigida e traduzida
  nunca chegava em quem mais precisava dela. Consertado com `http::escoar`,
  com prazo curto e teto igual ao de um pedido legítimo. Achado pela bancada,
  não por leitura — e nenhum teste de unidade sente isto, nem poderia; a guarda
  está no catálogo marcada REDUNDANTE, com o motivo escrito e o passo do
  soquete que a cobre. `docs/REST.md` §9.

### Adicionado

- **O webservice REST com OpenAPI, e o visualizador da especificação** —
  pedido 149, `docs/REST.md`. `POST /v1/<operação>` para as **113 operações**
  do protocolo, `GET /openapi.json` para a especificação, e um explorador na
  porta ao lado.

  **A especificação sai da tabela de despacho**, e essa é a decisão que
  sustenta o resto: uma spec digitada à mão envelheceria na primeira operação
  nova e passaria a **mentir com aparência de documento oficial**. Ela é
  gerada de `catalogo::OPERACOES` — que já é travado contra o `match` do
  `despachar` — e há **duas guardas, uma para cada lado do laço**: operação
  que existe e a spec não documenta reprova; rota documentada que o servidor
  não atende reprova. Mais a terceira, por soquete, percorrendo as 113 rotas
  que o servidor **serviu**.

  **Não há um segundo caminho de dados.** Toda rota passa pelo mesmo
  `despachar`, com os mesmos portões. As duas metades da sessão HTTP saíram de
  dentro do `/api` (`sessao_do_cabecalho` e `acertar_sessao`) e são chamadas
  pelos dois caminhos: duas cópias seriam duas ideias do que é estar logado.

  **As duas portas nascem desligadas** — 6000 o serviço, 7000 as docs —, e
  são duas de propósito: quem sobe numa placa quer o REST sem o visualizador.
  O teste que mais importa é o do comportamento velho, e ele existe em dois
  níveis: `config_sem_a_secao_rest_nao_escuta` no unitário, e o passo 1 da
  bancada com um `config.json` em que a palavra `rest` não aparece, provando
  contra o sistema operacional que as portas de fábrica estão **fechadas**.

- **O explorador da especificação, escrito aqui em vez do Swagger UI** — e a
  escolha saiu de número medido, não de gosto. Baixando o `swagger-ui-dist`
  5.17.14 de verdade e compilando com ele dentro: o binário vai de
  **7.296.144** para **8.900.968 bytes (+1.604.824, +22,0%)** com o mínimo
  funcional, e para **9.131.896 (+25,2%)** com o preset junto. Apontar para
  CDN custaria zero byte e **quebraria o uso offline**, que é justamente o
  caso do IoT. Este explorador são **13.629 bytes** — 118× menor —, faz busca,
  mostra parâmetros com tipo e obrigatoriedade, marca quem grava e monta o
  `curl` pronto. **Sem «Try it out»**: um console executável exigiria abrir
  CORS da porta REST para esta origem, e isso é folga de segurança que ninguém
  pediu. Foi a medição que dispensou uma *feature* de compilação: a 1,53 MiB
  ela seria obrigatória, a 13,3 KiB não se paga.

- **A seção `rest` do `config.json` e a tela dela**, com o que o dono pediu:
  nome do serviço, qual banco, quais tabelas e qual token. `database` e
  `tabelas` **só ESTREITAM** — aplicados **antes** do portão e nunca no lugar
  dele. Tabela fora da lista **não existe** para o REST (responde como
  inexistente, sem vazar que existe); tabela dentro da lista continua passando
  pelo direito do usuário **exatamente como hoje**. Dois testes, porque são
  dois erros opostos: `tabela_fora_da_lista_nao_aparece` e
  `tabela_na_lista_ainda_pede_direito_do_usuario` — o segundo é o que mais
  importa, e é o mesmo padrão do `sem_regra_de_tabela_nada_muda`.

  E a varredura das tabelas é **estrutural**, não por campo: ela desce a árvore
  do pedido inteira atrás de `tabela`, `tabelas` e `tabela_ref`. Sem isso,
  pedir a tabela escondida como o **lado B de uma junção** seria a porta dos
  fundos — literalmente o furo que esta casa já pagou quatro vezes.

- **`rest.token`, e a resposta de por que ele existe.** Vazio (o padrão) é o
  **mesmo token do protocolo** — um segredo só. Preenchido, **só ele** abre o
  REST, e o token do protocolo deixa de abrir esta porta: serve para entregar
  uma credencial de webservice sem entregar o token mestre, e para revogá-la
  sem tocar em cliente nenhum da porta 5000. Ele **não é identidade e não é
  poder** — é a chave da porta, e quem entra continua sendo quem faz `login`.
  E ele **não se edita pela tela**, pelo mesmo motivo que já mantinha `token`
  fora: campo que carrega credencial se edita no arquivo. A tela diz **se
  existe um**; nunca qual é.

- **`bancada/rest/provar.py`** — 57 passos por soquete, com um cliente HTTP
  escrito na hora porque `urllib` esconde justamente o que precisa ser provado
  (ele segue redirecionamento, decide sozinho o que fazer com 401 e levanta
  exceção em vez de devolver o código — e o código **é** o resultado).

- **`bancada/rest/arm.sh`** — o mesmo REST sob `qemu-aarch64-static`, porque
  «compilou» não é «atendeu um pedido»: 113 rotas geradas na placa emulada,
  RSS 12,8 MB, e o portão continuando a fechar.

- **`testes-web/prova-rest.mjs`** — a seção nova exercitada no navegador, que
  é a única forma de achar o que o CSS global faz com componente novo. O
  `<textarea>` da lista de tabelas mediu **260×66px**: nem esmagado a uma
  linha, nem esticado pelo `input{width:100%}` global.

- **40 chaves novas na `FABRICA_TELA`**, nos seis idiomas: os rótulos e as
  explicações da seção nova, e todo o texto do explorador. Ele lê `/idiomas` —
  a **mesma** resposta da interface web — e por isso trocar de idioma nele
  funciona sem uma segunda tabela de rótulos. A catraca do `conferidor.rs`
  **não subiu**: continua em 1.996. E a guarda `a_lista_cobre_tudo_que_o_http_serve`
  passou a ler os **dois** módulos que embutem tela, porque o `rest.rs` virou o
  segundo — uma guarda que continuasse lendo só o `http.rs` deixaria a tela
  nova fora da conta, que é exatamente o defeito que ela existe para não
  repetir.

- **Seis guardas novas no catálogo** (43 no total), cinco PROVADAS e uma
  declarada REDUNDANTE com o motivo medido.

- **`TipoDoCampo::Lista`**, o primeiro campo de lista que a tela grava. O tipo
  recusa o que não for lista de textos: sem isso, gravar
  `"rest.tabelas": "clientes"` passaria, o leitor cairia em vazio calado — e
  vazio, ali, quer dizer **expõe tudo**.

- **`docs/TRANSACOES.md`** — o desenho, com os seis pontos respondidos:
  escopo (uma conexão, um database; a web fora, e por quê), o rollback de um
  `inserir` **sem queimar slot**, o isolamento dito sem enfeite (não é ANSI
  SERIALIZABLE), a marca `.tx` sem a qual a recuperação não sabe para onde ir,
  a replicação e o custo para quem não usa.

  A decisão que mais custou foi a recusa do «slot que nasceu e morreu», com
  quatro motivos — e o decisivo é da replicação: queimar o slot dos dois lados
  exigiria mandar a inclusão **e** a exclusão para a réplica, que é
  literalmente a transação revertida chegando aplicada lá.

- **`so_um_lugar_toma_a_trava`**, a catraca que conta as tomadas **no próprio
  fonte** — pelo mesmo `include_str!` do conferidor de textos, e pela mesma
  razão: assim não há como contar um arquivo e compilar outro. Comentário que
  se afirma único precisa de quem conte.

- **Duas guardas novas no catálogo** (`trava-fora-do-ponto-unico` e
  `trava-sem-guarda-de-reentrancia`), para que as duas provas feitas à mão
  nesta rodada não se percam na próxima.

- **`--example custo-da-trava`** e a **§9 do `docs/DESEMPENHO.md`**: a guarda
  de reentrância está no caminho de **toda** leitura e **toda** escrita, então
  medi-la não era opcional.

### Mudado

- **O teste `duas_tabelas_na_mesma_janela_nao_travam_o_servidor` ganhou a
  asserção da consequência.** Ele provava o defeito pelo *travamento*, e a
  guarda de reentrância acabou com o travamento: o defeito reposto passou a
  devolver erro, que o `else { return }` do `descarregar_sujas` engole em
  silêncio. Ele agora confere que a janela de durabilidade **esvaziou o
  conjunto de sujas**, e reprova em 0,12 s em vez de 30. **Guarda que troca um
  travamento por um erro engolido enfraquece todo teste cujo único sintoma era
  o travamento** — quem a acrescenta tem de olhar a consequência no lugar.

### Sabido

- **Continua não havendo `BEGIN`, `COMMIT` nem `ROLLBACK`**, e a tela
  *Ferramentas → Gestão de transações* continua dizendo isso. Ela **não foi
  tocada de propósito**: nada passou a existir, e ela continua verdadeira.

- ***ACID compliant* continua falso.** Sem transação não há o **A** nem o
  **I**.

- **O REST não tem TLS**, e isso não é pendência escondida: é a mesma decisão
  da §7 do `SEGURANCA.md`. `Bearer` sobre HTTP em claro entrega o token a quem
  escuta o fio, em todo pedido. A saída é um proxy que termine TLS à frente,
  ou um túnel — e a frase está escrita no documento, na especificação e na
  tela, em vez de escondida.

- **Não há CORS, nem `GET` para leitura, nem «Try it out» no explorador.** As
  três ausências são decisão, com o motivo em `docs/REST.md` §10.

- **macOS continua sem alvo.** O caminho do REST não tem nada de específico de
  plataforma, mas o SDK da Apple só existe em macOS: expectativa não é
  medição, e esta casa não escreve «entregue» para o que ninguém executou.

- **Android e iOS não são caso de webservice**, e a razão é do sistema, não
  nossa: o iOS proíbe app escutando porta para outros apps, e o Android mata
  processo em segundo plano. Lá o caminho é o `phxsql-ffi`, de outra frente.

### Medido

| | |
|---|---:|
| guarda de reentrância, por tomada da trava | **−0,05 ns** (abaixo da resolução do medidor) |
| `lock` + `unlock` sem disputa | 15,45 ns |
| a operação mais barata do servidor (`ler`) | 41,31 µs |
| a guarda, como fração dela | **< 0,01 %** |
## Não lançado — o `.txt` do Profiler para de crescer
## Não lançado — a cifra do fio, por aperto de mão estilo Noise
## Não lançado — `ALTER TABLE ADD COLUMN`, com o rowid intacto

Escrito em paralelo com as outras seções «Não lançado» abaixo: na integração
todas viram uma só, e o número da versão sai de lá.

### Corrigido

- **O cabeçalho do arquivo do Profiler aceitava linha forjada — e o furo era
  antigo.** Ele interpola a descrição do filtro, e o filtro vem do pedido: um
  `"operacao": "ping\n2000-01-01T00:00:00 9.9.9.9 …"` punha no `.txt` uma
  segunda linha que se lê como evento de outro IP. É exatamente o defeito que
  o *evento* já fechava desde a validação anterior, pela porta que ela não
  cobria — o cabeçalho não é evento, e por isso escapou. Só apareceu ao
  escrever o rodízio, que precisa de um cabeçalho por arquivo novo. Os dois
  cabeçalhos e o rodapé passam pelo mesmo `de_uma_linha` dos campos do evento.

- **`escrever_linha` voltava calada quando não havia descritor.** Com o
  profiler só em memória isso está certo; com um **caminho escolhido** e sem
  descritor — que é o que uma reabertura falhada deixa — é o defeito do disco
  cheio de volta: a tela seguiria dizendo «gravando em …» com nada sendo
  gravado. As duas situações passaram a ser distinguidas, e a segunda conta a
  linha perdida.

### Adicionado

- **Rodízio do `.txt` do Profiler, por tamanho.** Ele media **345 bytes por
  pedido** e não parava nunca — **1,2 GB por hora** a mil pedidos/s, num
  arquivo que enche a partição do servidor inteiro. Cheio o teto,
  `perfil.txt` vira `perfil.txt.1`, o `.1` vira `.2`, e o mais velho sai. O
  gasto máximo deixa de ser «depende» e vira uma conta que se compara com o
  `df` — e ela sai do **servidor** já multiplicada, porque uma segunda
  multiplicação escrita no JavaScript envelheceria calada:

  ```text
  profiler.arquivo_mib x (profiler.arquivos + 1)
  ```

  Padrão `64 × (4 + 1)` = **320 MiB**, que a 345 B por pedido são ~970.000
  pedidos. `profiler.arquivo_mib: 0` devolve o comportamento de antes — o
  arquivo cresce sem teto —, e há teste cobrando que continue valendo.

  **Por tamanho e não por tempo**, porque o perigo é disco e disco se mede em
  bytes: um rodízio diário não põe teto nenhum — a mil pedidos/s o arquivo do
  dia tem 29 GB, a um pedido/s tem 30 MB, a *mesma* política com mil vezes de
  diferença, decidida pelo movimento do servidor e não por quem configurou. E
  o Profiler é ferramenta de diagnóstico, ligada por minutos: um rodízio «todo
  dia à meia-noite» quase nunca dispararia, e quando disparasse seria no meio
  da única sessão que alguém estava lendo.

- **A tela do Profiler diz o teto e quantas vezes o arquivo já virou.** Sem
  isso, «gravando em `perfil.txt`» seria verdade e mentira ao mesmo tempo — o
  começo da sessão não está mais lá, e quem fosse investigar procuraria no
  arquivo errado. `rodizios` e `falhas_de_rodizio` entram na resposta ao lado
  de `gravados_bytes` e `falhas_de_escrita`, e um rodízio que falha pinta a
  caixa de vermelho como uma linha perdida pinta. Os oito textos novos entram
  **pela fábrica de idiomas**, nos seis idiomas — a catraca continua em 1.999.

- **`profiler.arquivo_mib` e `profiler.arquivos`** pelo ponto único
  (`CAMPOS_CONHECIDOS`, `SECOES_CONHECIDAS`, `CAMPOS_EDITAVEIS`,
  `editaveis_json`), no grupo «Arquivo do Profiler» da tela de configuração, e
  **a quente**: quem está vendo o arquivo crescer e abaixa o teto quer o efeito
  agora, não no próximo `profiler_ligar`.

- **Três guardas novas**, as três **PROVADAS**: o zero deixando de desligar o
  rodízio, o cabeçalho aceitando linha forjada, e a linha sumindo sem ser
  contada.

### Sabido

- O rodízio **apaga** o arquivo mais velho, e isso é uma escolha declarada. A
  regra da casa manda pensar duas vezes antes de ligar guarda nova por padrão;
  o que se pesou está em `docs/SEGURANCA.md` §10: o `.txt` nunca prometeu ser
  completo — com o disco cheio ele já perdia linha, e o conserto de então foi
  **contar** a perda, não evitá-la. Um arquivo com teto que avisa quando vira é
  melhor que um sem teto que morre junto com a partição. E a saída para quem
  discordar é um campo: `arquivo_mib: 0`.

---

## Não lançado — o `fsync` da exclusão, e quem escolhe pagá-lo

Escrito em paralelo com as outras seções «Não lançado» abaixo: na integração
todas viram uma só, e o número da versão sai de lá.

É o sprint nº 1 de `docs/SPRINTS.md` — o único da lista de 27 cujo valor
estava **medido** em vez de julgado. Ele entrou **pedido, e não imposto**, e o
número foi refeito antes de uma linha de código.

### Adicionado

- **`recursos.exclusao_na_janela`: a exclusão física passa a respeitar a
  janela de durabilidade — quando o dono pede.** O `fsync` que
  `LixeiraFile::guardar` fazia **por exclusão** sai do caminho e passa a
  fechar com o resto da tabela. **O campo nasce desligado**, e essa é a
  decisão inteira: com o padrão de fábrica, um `excluir` que responde OK
  continua já estando no disco, byte por byte como antes. Ligá-lo por padrão
  mudaria o significado da resposta para todo cliente já escrito, sem ninguém
  ter pedido — e retirar garantia sem pedido é o mesmo estrago de impor guarda
  nova, pelo outro lado. Editável pela tela, na seção «Gravação e
  durabilidade», pelo ponto único (`CAMPOS_EDITAVEIS`).

  Medido em máquina **disputada**, e a condição está escrita: sete corridas
  alternadas de cada lado, com a janela de 200 gravações que o `config.json`
  traz por padrão — **4,52 s → 1,46 s, 3,10×**, e a pior corrida com a janela
  (1,981 s) ainda é 2,17× melhor que a melhor sem ela (4,293 s). O teto, com a
  janela que não fecha (o caso do `BULKINSERT`), é **3,75 s → 0,58 s, 6,50×**
  — o mesmo par que o `SPRINTS-CASSANDRA.md` §3 mediu como 7,8× noutra
  máquina. O critério de morte combinado antes era 2×.

- **A fase `excluir` da bancada vira, e por dois motivos.** Medido a 1.000.000
  nesta máquina, duas corridas de cada: **6,30 s / 16,59 s → 0,91 s / 0,96 s**,
  contra 1,45 s / 1,90 s do MySQL(R) — de perder por 4,3× para ganhar por 1,9×.
  O segundo motivo é que a fase **não comparava trabalho igual**: do lado do
  MySQL(R) as 20.000 instruções vão dentro de um `START TRANSACTION … COMMIT`,
  que é **um** `fsync` para as vinte mil, e do nosso lado eram vinte mil. É a
  mesma família dos dois erros que a `bancada/LEIA-ME.md` conta, e desta vez o
  erro era contra nós. O padrão da bancada continua sendo o de fábrica;
  `PHX_EXCLUSAO_NA_JANELA=1` roda a comparação de durabilidade equivalente.

- **`bancada/exclusao/prova-da-queda.py`: a prova pelo processo.** Sobe um
  `phxsqld` de verdade na porta 7100 com a janela **aberta durante a corrida
  inteira**, manda 150 exclusões físicas pelo soquete, mata o processo com
  `SIGKILL` e reabre — nos dois modos, e conferindo linha a linha os quatro
  estados possíveis. **Nenhuma linha some dos dois lados numa queda de
  processo**, e o número de casos «em nenhum» é zero nos dois modos. Teste
  unitário não provaria: quem fecha uma `Table` executa `Drop` e volta ao
  teste, e nada disso é uma queda.

- **Três guardas novas** no catálogo de defeitos repostos, as três **PROVADAS**
  (`python3 bancada/guardas/provar-guardas.py --so exclusao-na-janela-por-padrao`):
  a janela virando padrão, o campo sem leitor, e o `.reg` fechando antes do
  `.trash`.

### Corrigido

- **`Table::sincronizar` fechava o `.reg` antes do `.trash`.** Enquanto o
  `fsync` da lixeira acontecia por exclusão, a ordem era indiferente — o
  `.trash` já estava no disco muito antes. Com a exclusão na janela os dois
  passam a fechar ali, e o `.reg` na frente é a única ordem em que uma queda
  no meio do próprio fechamento deixa a linha liberada sem a cópia de
  recuperação. Agora o `.trash` abre a lista e o `.reg` a fecha, e o teste
  `o_trash_fecha_antes_do_reg` trava isso com um selo de ordem por conjunto de
  volumes.

- **`bancada/medir.py` media o binário de OUTRA árvore.** O caminho do
  `examples/carga` era um absoluto escrito à mão apontando para
  `/home/user/adrianoboller/phxsql/…`; rodada numa árvore de trabalho, a
  bancada media o binário do repositório principal. É a armadilha do binário
  velho num degrau mais alto — nem recompilar na própria árvore resolveria.
  Agora o caminho sai de onde o arquivo está, com `PHX_CARGA` para quem
  precisar apontar noutro lugar.

### Sabido

- **A premissa 2 do Sprint 1 estava errada, e a conferência a derrubou.** Ela
  afirmava que uma queda dentro da janela só produz dois estados: a linha só no
  `.reg` (a exclusão não aconteceu) ou só no `.trash` (duplicada). São
  **quatro**. Sem o `fsync` entre a escrita do `.trash` e a liberação do slot,
  a ordem de **chegada ao disco** passa a ser do sistema operacional — e o
  `.reg` de uma tabela em uso já tem páginas sujas mais velhas que as do
  `.trash`. Existe, portanto, o estado em que o `.reg` foi liberado e o
  `.trash` não chegou.

  Queda de PROCESSO **não** produz esse estado, e isso está provado a
  `kill -9`. Queda de ENERGIA produz. Nenhum `fsync` nosso ao fechar a janela
  conserta, porque o problema não é a ordem em que nós sincronizamos — é a que
  o núcleo escolhe antes de alguém nos perguntar.

  Foi por isso que o item entrou **pedido**: ligado, `exclusao_na_janela` é a
  escolha declarada de trocar uma rede de recuperação estreita por 3,10×, e a
  linha em risco é uma que **alguém mandou apagar**, com o motivo já gravado
  no `.reason`. Nenhuma linha não excluída corre risco em caso nenhum. O caso
  a caso está em `docs/DESEMPENHO.md` §4.12 e no `MANUAL.txt`.
## Não lançado — a trava de dados saiu de trás da rede

Escrito em paralelo com as outras seções «Não lançado» abaixo: na integração
todas viram uma só, e o número da versão sai de lá.

**O achado 2 do pedido 146 virou conserto.** O laço da réplica tomava a trava
global de dados na **primeira linha** de `alcancar_tabela` e a segurava
atravessando `replica::puxar`, que é uma ida e volta de rede.

### Corrigido

- **A trava de dados ficava presa atrás de uma leitura de rede.** Com um corte
  silencioso — pacote que some, e não porta que recusa —, a leitura ficava
  pendurada até o prazo de leitura de 30 s e a trava ia junto: o servidor no ar
  e sem atender dado nenhum. Medido em `bancada/replicacao/trava.py`, com o
  source escrevendo sem parar e o tubo emudecido por 40 s: pior `varrer` na
  réplica **30.079 → 6 ms**, com o `ping` (que não toca na trava) em 4-5 ms nos
  dois casos. A telemetria da própria réplica, que é a testemunha de dentro,
  contava **35,8 s de trava na mão numa janela de 40 s**.

- **No bidirecional, os dois lados se trancavam um ao outro — sem corte
  nenhum.** Cada um segurava a própria trava esperando a resposta do outro, que
  só podia vir depois de o outro soltar a dele. Um abraço mortal de verdade,
  desfeito apenas pelo prazo de 30 s dos dois lados. 200.000 linhas escritas
  metade em cada lado ao mesmo tempo: **33,0 s → 1,8 s**, de **14,0×** para
  **0,72×** do servidor sozinho, e os `EAGAIN` sumiram do diário dos dois. Com
  1.000.000 de linhas o antes era **240,7 s contra 12,1 s — 19,8× — com sete
  `EAGAIN` de cada lado**, a mesma assinatura simétrica que o contêiner tinha
  visto.

- **E o que doía todo dia, sem corte nenhum:** num alcance de rotina de 200.000
  eventos, com a rede perfeitamente sã, o cliente da réplica esperava **2.727 ms**
  por um `varrer`, porque a trava ficava presa pelo alcance inteiro. Hoje espera
  **48–98 ms**, que é o `fsync` final.

- **`alcancar_tabela_bidi` tomava `self.dados.lock()` cru**, sem passar pelo
  `travar_dados()`. O pior caminho do servidor era justamente o que a telemetria
  não conseguia cronometrar. Ponto de medição que pula um chamador mente do
  mesmo jeito que campo de configuração que ninguém lê. As tomadas fora do
  ponto único caíram de 13 para 12.

### Adicionado

- **`bancada/replicacao/trava.py`** — quatro estágios, ~1,5 min, portas
  7050-7055, sem Docker. É a versão de loopback dos estágios `a3-congelamento`
  e `b-abraco` da bancada de contêiner: no lugar do cabo cortado há um **tubo**
  em Python que repassa byte a byte até mandarem emudecer e a partir daí segura
  os dois soquetes abertos sem repassar nada. Os números batem com os do
  contêiner (30.079 contra 29.456 ms; 33,0 contra 33,3 s), o que confirma que o
  mecanismo nunca dependeu do Docker. Entrou como a **17ª parte** do
  `provar.py`.

- **Dois tetos declarados de memória**, que passaram a ser obrigatórios porque
  o lote agora mora inteiro na memória da réplica até a trava chegar:
  `TETO_DO_LOTE_SERVIDO` = 16 MiB de imagem por resposta no source (com o
  primeiro evento entrando **sempre**, senão uma linha maior que o teto pararia
  a replicação em vez de atrasá-la) e `TETO_DA_RESPOSTA` = 128 MiB por linha
  lida na réplica, com recusa `LIMITE_EXCEDIDO` que traz o número dentro. Antes
  o `read_line` era ilimitado e quem escolhia o tamanho era o outro lado do fio.

- **`crates/phxsql-server/tests/trava-atras-da-rede.rs`** — dois testes por
  soquete, com um source de mentira que responde `posicao` e **emudece** no
  `replicar`. O segundo é o do comportamento **velho**, sem o qual um conserto
  que quebrasse a replicação inteira passaria com louvor. Cada sonda tem prazo
  próprio de 8 s, porque com o defeito reposto ela **pendura** por 30 s em vez
  de falhar — e bateria que pendura não reprova ninguém, trava.

- **Guarda `trava-atras-da-rede`** no catálogo dos defeitos repostos.
  **PROVADA**: 13,2 s com o defeito reposto, e a mensagem de reprovação já traz
  o diagnóstico pronto. O catálogo passou de 18 para 19 guardas — 17 provadas,
  2 redundantes, zero «não pegou», 182 s de mutação.

### Mudado

- `alcancar_tabela` e `alcancar_tabela_bidi` estão partidas em **três fases**:
  abrir e ler a posição com a trava, ler o lote do soquete **sem** ela, reabrir
  e aplicar com ela. A regra que sai daí vale para o que vier: *nenhuma leitura
  de rede acontece com a trava de dados na mão*. A posição é **relida** na fase
  3, e o lote é descartado quando ela andou — descartar custa uma ida e volta,
  aplicar torto custaria o dado.

### Sabido

- **A bancada de contêiner não foi refeita**, porque o daemon do Docker desta
  máquina estava fora do ar. Os `resultados.json` de
  `bancada/replicacao/docker/` continuam sendo o retrato do **defeito**, e está
  escrito lá que são. Refazer numa máquina com Docker é o que fecha a conta.

- **A hipótese que morreu, registrada porque a recusa com número vale tanto
  quanto o ganho:** a primeira versão do conserto mediu **−17% de vazão** de
  aplicação e culpou a abertura de tabela por lote. Era plausível, tinha
  número, e estava errada — o custo era um `sincronizar()` deixado dentro da
  fase 3, **400 `fsync` num alcance de 200.000 eventos em vez de um**. Com o
  `fsync` de volta ao fim do alcance a vazão é a de antes (67.406 → 68.000
  eventos/s). E a segunda hipótese morreu junto: subir `LOTE` de 500 para 2.000
  para amortizar a abertura foi **recusada** — o source lê `max` eventos antes
  do corte por bytes, então quadruplicar o lote quadruplica o pior caso de
  memória de quem serve, na direção contrária do teto que este mesmo trabalho
  acabou de declarar.
## Não lançado — a frase que não se traduz picada

Escrito em paralelo com as outras seções «Não lançado»: na integração todas
viram uma só, e o número da versão sai de lá.

A catraca dos idiomas tinha acabado de subir de 1.999 para **2.068**, porque o
`multitela.js` era servido pelo `http.rs` e não estava no `FONTES` do
conferidor — 69 textos cravados que nunca contaram. Esta rodada traduz os 69.
E ao traduzi-los apareceu uma lição de desenho que vale para todo o resto da
interface.

### Adicionado

- **O `ui/multitela.js` inteiro passa pela fábrica de idiomas**: 68 rótulos
  viraram **70 chaves** `tela.mt_*` nos seis idiomas — as dicas da tira de
  abas, os botões da janela solta, o `aria-label` da calha, os dezesseis
  recados e a tela «Sobre o modo multitela». É o **primeiro arquivo de
  interface com zero texto cravado**. O sexagésimo nono, `devicePixelRatio`,
  entrou nos isentos com a razão escrita: nome de propriedade do navegador não
  se traduz, e ele aparece dentro de um `<code>`.

- **`marcado()` e `preencher()`, na página** — a resposta ao achado desta
  rodada, que está escrito por inteiro no `docs/MENSAGENS.md`: **frase picada
  por marcação é intraduzível por construção.** Trinta e nove dos 68 eram uma
  frase só, partida em treze literais pelos `<b>` e `<code>` do meio —
  `"funcionam em"` + `<b>qualquer navegador</b>` + `"— é layout. Destacar em
  janela também, com"`. Nenhum desses pedaços é uma frase, e não existe ordem
  de pedaços que sirva para as seis línguas: em alemão o verbo vai para o fim.
  Hoje a frase inteira é **uma chave** e a ênfase é uma **marca dentro do
  texto** — `**assim**` para `<b>`, a palavra entre crases para `<code>`,
  `{nome}` para o dado. O corte em etiquetas acontece **depois** da tradução,
  e na captura alemã a ênfase caiu em `**gleich auf dem richtigen Monitor**`,
  onde o português não tem nada.

- **Três testes que travam o mecanismo**, os três com o defeito reposto:
  `nenhum_texto_da_fabrica_traz_etiqueta_crua` (um `<b>` gravado na célula
  apareceria escrito, porque a página escapa antes de escrever — e escapar não
  se muda: a célula é editada pela grade, e célula editável é entrada de
  usuário), `as_marcas_de_enfase_fecham` (asterisco aberto e não fechado, o
  erro mais provável de quem reescreve a frase em alemão e o mais silencioso,
  porque só aparece naquele idioma) e
  `todo_idioma_tem_os_mesmos_marcadores_do_portugues` (o `{n}` perdido numa
  tradução, que deixaria o número sem aparecer).

- **Passo 7 da `prova-idiomas.mjs`**, pelo navegador: abre a tela do modo
  multitela em português, confere que a frase sai **inteira e na ordem**, que
  a marca virou `<b>`/`<code>` de verdade e que não sobrou marca crua; troca
  para alemão **sem sair da tela** e confere que nem o corpo, nem o título,
  nem o `title` da tira ficaram em português. Prova real nos dois sentidos:
  sem o gancho `est.repintar` a tela não troca de idioma, e sem a conversão de
  marcas a página mostra `**Multitela.**` com os asteriscos à mostra.

### Mudado

- **A catraca desce de 2.068 para 1.999**, medida e não digitada. O número é o
  mesmo de duas rodadas atrás por coincidência — e agora ele quer dizer o que
  dizia por engano: 1.999 sobre a interface **inteira**, e não sobre cinco
  sextos dela.

- **`PhxTelas.repintar()`**, chamado pelo `aplicarIdioma`: o cromo deste
  módulo é pintado uma vez e ficava na língua anterior. Ele repõe também o
  rótulo das abas cujo nome vem da fábrica — **por chave**, nunca comparando a
  frase com o rótulo antigo, porque quem compara frase quebra calado no dia em
  que alguém melhorar a redação.

- **O `CATALOGO` do `multitela.js` ganhou o par `rot`/`txt`**, e as seis
  chaves são as que **já existiam** (`tela.painel`, `tela.fer_query`,
  `tela.fer_diagrama`, `tela.fer_telemetria`, `tela.fer_profiler`,
  `tela.usuarios`). Chave nova para o mesmo botão seria uma segunda verdade, e
  o tradutor traduziria duas vezes a mesma palavra. A entrada `tabela` fica
  **sem** `txt`: o rótulo dela é o nome da tabela, que é dado.

### Sabido

- **A aba de segundo plano guarda o idioma em que foi pintada.** Achado
  exercitando, e deixado de fora de propósito: o `est.repintar` é o gancho da
  tela **com foco**, e repintar tela escondida contraria a decisão [2] do
  módulo — *aba escondida sai do documento e para de trabalhar* —, que é o que
  faz a telemetria de uma aba escondida custar zero pedido. Nomeado no
  `docs/PENDENCIAS.md` §3.2, item 12.

---
O tráfego da porta 5000 ia em claro — o token de serviço em **toda** linha
JSON, e os dados de volta. Faltava só a troca de chaves: o
ChaCha20-Poly1305, o SHA-256, o HMAC e o desafio-resposta já existiam e já
estavam conferidos contra vetor oficial. O desenho inteiro está em
[`docs/CIFRA-DO-FIO.md`](docs/CIFRA-DO-FIO.md).

### Adicionado

- **X25519 (RFC 7748)**, tempo constante e sem tabela, reaproveitando a
  aritmética de corpo do `ed25519.rs` — as duas curvas vivem no mesmo corpo
  finito, e um segundo `fe_mul` ao lado do primeiro dobraria a superfície de
  erro na parte que ninguém revisa duas vezes. Conferido contra **todos** os
  vetores que dá para exercitar: os dois de multiplicação escalar da §5.2, o
  iterado de 1 e de 1.000 vezes, e o Diffie-Hellman da §6.1. O de 1.000.000
  fica atrás de `#[ignore]`, por custar minutos. Segredo compartilhado
  todo-zeros (ponto de ordem pequena) é **erro**, e isso é teste, não
  comentário.
- **HKDF-SHA256 (RFC 5869)** sobre o HMAC que já existia, com os três casos
  SHA-256 do anexo A.
- **O aperto `Noise_NX_25519_ChaChaPoly_SHA256`** e a camada de registro, em
  `phxsql-core/src/fio.rs`. Duas mensagens, um ida-e-volta; a estática do
  servidor viaja cifrada e a etiqueta final só fecha se quem respondeu tiver a
  privada dela. O cliente pina a chave (estilo `known_hosts`) ou aprende na
  primeira vez.
- **A porta de dados atende `{"op":"cifrar"}`** e, da linha seguinte em diante,
  fala registros selados — inclusive o token, que hoje ia em texto puro.
- **A replicação puxa por dentro do túnel** com `"cifra": true` na origem e o
  pino em `"chave_do_fio"`.
- **`phxsqld --chave-do-fio`** imprime a chave pública do servidor, que é o
  que o cliente pina.
- **`bancada/cifra-do-fio/prova.py`**: um cliente escrito **de novo**, em
  Python puro — X25519, ChaCha20-Poly1305, HKDF e o aperto —, para os dois
  lados fecharem deixar de ser «o mesmo código concordando consigo mesmo».

### Mudado

- `docs/SEGURANCA.md` §7 era «Sem TLS», escrito como ausência. Virou decisão,
  com o limite escrito: **com `cifra_fio.exigir` desligado — o padrão — a
  proteção vale contra escuta PASSIVA e nada mais.** Cifra pedida é cifra que
  o atacante ativo apaga do pedido.
- `docs/REPLICACAO.md` §13 e o item 8 da tabela de danos do
  `docs/PENDENCIAS.md` deixaram de dizer «falta TLS» e passaram a dizer o que
  existe e o que continua faltando.

### Sabido — e é limite declarado, não esquecimento

- **Não é TLS, e o navegador não fala isto.** A interface web **não** ganha o
  túnel: um aperto em JavaScript seria teatro, porque o próprio script chega
  pelo canal em claro que se quer proteger. Para a porta web continuam valendo
  as duas saídas honestas: proxy TLS à frente, ou túnel.
- **Não interopera** com outras implementações de Noise: os tijolos são de
  norma e conferidos contra vetor oficial, mas a composição não foi rodada
  contra os vetores do *cacophony*.
- **O driver ODBC não fala o aperto**, e com `exigir: true` ele para.
- **O pulso do cluster continua em claro** — cifrar metade do tráfego do
  cluster é pior que não cifrar nenhuma, porque parece protegido.
- **A credencial ainda não é amarrada ao canal.** O hash da transcrição existe
  e está exposto; ninguém o consome.

### O que a prova real achou, e não a leitura

O executor das guardas (`bancada/guardas/provar-guardas.py`) devolveu **NÃO
PEGOU** em duas das cinco entradas novas, e as duas eram achados de verdade:

- **`cliente_sem_cifra_continua_como_antes` passava com o padrão trocado para
  `exigir: true`** — porque ele mesmo montava o `Config` e escrevia
  `exigir = false`, desfazendo a troca. Era um teste que passava por engano,
  justamente o que a casa considera pior que teste que falta. Ele agora sobe de
  um `config.json` **sem a seção `cifra_fio`**, que é literalmente o arquivo de
  quem atualizou o binário e não mexeu em nada.
- **`canal_leva_e_traz` não sente o contador parado.** Medido: com o contador
  congelado os dois lados usam nonce zero em todo registro, e uma conversa que
  vai e volta uma vez continua fechando — ela não repete registro nenhum, que é
  o único jeito de sentir a falta do contador. O teste não estava errado;
  errada estava a minha conta de quatro. Está escrito no catálogo, ao lado da
  entrada.

O sprint 25, que era o que faltava ao pedido 127. Até aqui **não dava para
acrescentar coluna a uma tabela que já tem dado** — e é disso que qualquer
sistema em produção precisa no segundo mês.

### Adicionado

- **`acrescentar_coluna`**: uma coluna nova numa tabela com dado, **com o
  rowid de cada linha preservado**. O `.reg` é reescrito slot a slot, na mesma
  ordem — inclusive os slots livres, que continuam livres e continuam ocupando
  o lugar deles. Como o rowid *é* a posição, preservar a posição preserva o
  rowid, e por isso o **`.ndx` não é tocado**: há teste que compara o arquivo
  byte a byte antes e depois, e ele cai no dia em que alguém resolver
  reconstruir o índice aqui. `docs/FORMATO.md` §1.1.

- **O medidor `--example custo-do-alter`**, e com ele a troca de uma
  inferência por uma medida. O sprint dizia «a casa dos minutos para dez
  milhões — inferido, não medido». **São 5,53 s**: 0,553 µs por linha, linear
  do primeiro tamanho ao último, dominado pelo disco (427 MiB/s somando o
  arquivo lido com o escrito). Para comparar, **construir** a mesma tabela de
  dez milhões levou 90,4 s — a alteração custa 6,1% do que custou digitar o
  dado. `docs/DESEMPENHO.md` §4.14.

- **As outras duas saídas caíram com número, e não por opinião.** Slot de duas
  larguras convivendo o formato **não permite** — o `slot_size` é um campo só,
  nos bytes 16..20 do cabeçalho — e cobraria uma busca onde hoje há uma
  multiplicação: medido, **2,36×** por linha lida (1,08 → 2,55 µs), em toda
  leitura, para poupar uma passada uma vez. «Só em tabela vazia» custa 0,6 ms
  e continua existindo — é o caminho de quem declara a coluna obrigatória sem
  padrão —, mas não resolve o problema, evita-o.

- **A resposta para a morte no meio da reescrita.** Duas fases: escreve
  **todos** os `*.novo` e sincroniza cada um; só então troca, com `rename`, o
  volume 1 primeiro. O volume 1 é o **ponto de compromisso**, e a abertura lê
  o estado nele: velho, os `*.novo` são lixo de uma fase que não decidiu nada;
  novo, a alteração está decidida e a abertura **termina** o `rename` que
  faltou. Se o `*.novo` também sumiu, a abertura **recusa** o conjunto
  nomeando o volume — em vez de ler o volume 3 com a largura do volume 1, que
  sairia com cada linha deslocada da anterior e sem nenhum CRC reclamando,
  porque os bytes lidos seriam bytes de outra linha.

- **O botão na tela**, na aba Estrutura e no cartão do editor de modelo. O
  cartão dizia, em português cravado, que alterar coluna «não existe no
  servidor»; agora ele abre o formulário que funciona — e continua dizendo a
  verdade sobre o que **não** dá: trocar tipo ou largura de coluna existente.
  Os textos nasceram na fábrica de idiomas, e os três parágrafos que saíram
  **baixaram a catraca de 1.999 para 1.996**.

### Mudado

- **A coluna nova entra depois da última coluna do usuário**, e não no fim da
  lista: as de sistema (`softdeleted`, `rownum`) entraram no fim para não
  deslocar as do usuário, e a coluna que o usuário acrescenta agora é dele. O
  preço é que a **posição** das de sistema anda, e as três coisas que guardam
  posição — `IndexColumn.coluna`, `ForeignKey.colunas` e a coluna de referência
  da partição — são remapeadas em `Schema::com_coluna`, num lugar só.

- **A decodificação de linha confere a largura do payload antes de ler.** Um
  payload guardado antes da alteração — a imagem de um evento do diário, a
  linha de uma lixeira, o que chega de uma réplica que ainda não alterou —
  passa a dar *«a estrutura da tabela mudou depois que ela foi gravada»* em
  vez de sair por índice fora da faixa ou, pior, com os campos deslocados.

- **`acrescentar_coluna` exige `administrar`**, e não `criar`: é a maior
  escrita de estrutura do motor e a que não tem desfazer barato — o mesmo
  poder do `excluir_tabela` e do `marcar_lgpd` ao lado dela.

### Corrigido

- **A caixa de marcar do cartão novo nascia com 834px de largura**, esticada
  pelo `input{width:100%}` da folha global. Achada no primeiro minuto em que o
  cartão existiu, pela bateria de navegador, e invisível lendo o código — é a
  mesma armadilha do rádio que virou bolinha do tamanho da célula, e a mesma
  lição do «Blumenau» virando «BLUMENAU», que o `text-transform:none` do
  rótulo fecha do outro lado.

### Sabido

- **Dois testes desta frente passavam por acaso.** Os que provam o
  remapeamento de posição foram escritos primeiro contra uma tabela comum, e
  ali nenhuma referência fica depois da coluna nova: com o remapeamento
  trocado pela identidade, os dois continuavam verdes. Foram reescritos contra
  um índice e uma chave sobre coluna de **sistema**, que é onde a posição
  realmente anda, e aí caem. Teste que passa por engano é pior que teste que
  falta.

- **A alteração não se replica.** Ela é local, e uma réplica só volta a
  aplicar eventos depois de receber a mesma alteração. Enquanto os dois lados
  diferem, a réplica **para** em vez de aceitar um payload de outra largura —
  medido em `bancada/alter/provar.py`, passos 9 e 10, que também mostram que
  ela retoma sozinha do ponto em que parou.

- **A lixeira anterior à alteração não volta.** O `.trash` guarda o payload com
  a largura de quando a linha saiu, e restaurá-la agora daria campo trocado; a
  mensagem diz isso em vez de devolver lixo. Migrar o `.trash` junto é
  possível e não entrou nesta rodada.

- **Não entram:** trocar tipo ou largura de coluna existente, tirar coluna,
  renomear coluna pelo protocolo, e criar índice sobre a coluna nova no mesmo
  gesto.

---

## Não lançado — os pacotes de download que se conferem
## Não lançado — a bateria única e o catálogo de defeitos repostos

Escrito em paralelo com as outras seções «Não lançado» abaixo: na integração
todas viram uma só, e o número da versão sai de lá.

**O pedido 17 estava marcado como feito, e o empacotador não rodava num
checkout limpo.** Rodá-lo achou três defeitos que ler o código não acharia — o
mesmo padrão do vídeo de demonstração, por outro caminho.

### Corrigido

- **`./empacotar.sh` morria num checkout limpo, e só ali.** O `monta()`
  compila com `--target`, que grava em `target/<alvo>/release`; o config de
  demonstração pede o hash da senha ao `./target/release/phxsqld`, que é o
  binário do **hospedeiro**, e `--target` nunca o produz. Quem tivesse rodado
  `cargo build --release` antes não via defeito nenhum — inclusive quem
  escreveu o script. Reposto o defeito (binário do hospedeiro fora do lugar),
  o empacotador para em `No such file or directory` antes de montar zip
  algum. Entrou o `garante_host()`, que o compila quando falta.

- **O `demonstracao/config.json` gerado escrevia `web.sessao_min`, e o campo é
  `web.sessao_minutos`.** O servidor avisa e **ignora** o valor; como o padrão
  também é 60 minutos, a tela ficava idêntica e o aviso rolava para fora do
  terminal. Só apareceu subindo o binário empacotado de verdade, numa porta da
  faixa 6750–6799, e lendo a primeira linha da saída. Configuração que não é
  lida mente — e mente melhor quando promete justamente o padrão.

### Adicionado

- **`MANIFESTO.sha256` em cada um dos três zips**, com o SHA-256 de todos os
  arquivos do pacote, e `pacotes/SHA256SUMS` com o hash dos próprios zips, que
  é a pergunta de quem acabou de baixar e ainda não abriu.

- **`phxsql conferir-pacote [<dir>]`**, o conferidor que viaja **dentro** do
  pacote. O Windows não tem `sha256sum`, e o `phxsql.exe` está ali do lado; o
  SHA-256 é o deste projeto, já conferido contra os vetores do FIPS 180-4. O
  formato do manifesto é o do `sha256sum` de propósito, para haver um segundo
  caminho (`sha256sum -c MANIFESTO.sha256`) que não depende de rodar o binário
  que está justamente conferindo.

  Ele reprova três coisas, e a terceira é a que quase nenhum conferidor pega:
  `DIFERE`, `FALTA` e **`A MAIS`**. Conferência de hash só olha o que o
  manifesto **lista** — quem acrescenta um arquivo ao pacote não mexe em
  nenhuma linha e passaria batido, que é exatamente o jeito de entregar um
  binário a mais junto do pacote legítimo. É a regra que o `backup.json` já
  seguia.

  Sete testes, cada um com o defeito reposto: um byte trocado, conteúdo
  diferente com o mesmo tamanho, arquivo a mais, arquivo faltando, pacote sem
  manifesto e manifesto ilegível. Conferidor que aprova tudo é pior que
  conferidor nenhum, porque quem baixou acha que conferiu.

- **Quatro travas antes de qualquer zip sair.** A versão tem de bater em
  `Cargo.toml`, `Cargo.lock`, no cabeçalho do `MANUAL.txt` e no título lançado
  mais novo do `CHANGELOG.md` — o `MANUAL` não dizia versão nenhuma e ganhou o
  selo, porque o que não se afirma também não se confere. O pacote de Windows
  confere alvo e ligador **antes** do `cargo build`, imprimindo o comando
  exato de quem não os tem. O pacote de fontes recusa árvore suja, porque o
  `git archive` lê o `HEAD` e um zip diferente do que o autor está vendo não
  avisa ninguém. E, depois de extrair, o empacotador confere que o
  `Cargo.toml` caiu na raiz do zip — o recorte de subdiretório do `git
  archive` é sutil demais para se confiar nele em silêncio.

- **`./empacotar.sh conferir`**, que desempacota o que está em `pacotes/` e
  confere os três com o próprio `phxsql`.

- **O que faltava nos pacotes de binário**: os três `Config_exemplo_*.json`
  (isolado, source e réplica), e `docs/ODBC.md` e `docs/CONSOLE.md` — os dois
  documentos que o `COMECE-AQUI.txt` citava pelo nome, sem que quem baixou o
  zip tivesse o repositório para ir buscá-los.

- **`docs/EMPACOTAMENTO.md`**: o que cada zip leva, o que ele deliberadamente
  não leva, e como quem baixou confere.

### Medido

- **Zero dependências externas continua verdade depois desta rodada.**
  `cargo metadata --offline` dá **7 pacotes no grafo, os 7 deste repositório,
  0 com `source`** — nenhum de registro, nenhum de git. O `Cargo.lock` inteiro
  cabe em 53 linhas.

- **O teste que o dono vai fazer.** Zip de fontes extraído num diretório limpo
  fora da árvore, `CARGO_HOME` vazio (zero entradas), `CARGO_NET_OFFLINE=true`
  e as variáveis de proxy apagadas: `cargo build --offline --release` em
  **28,6 s, 30,3 s e 34,3 s** em três medições, sete crates, quatro binários. E
  o laço fecha — desse diretório extraído, `./empacotar.sh linux` remonta o
  pacote de Linux inteiro.

- **O binário de Linux roda.** Subido do zip numa porta da faixa 6750–6799:
  `ping` pela porta de dados responde `{"ok":true,...,"phxsql":"0.18.0"}`, a
  interface web devolve 1.057.862 bytes de HTML, e o `phxsqlcmd` empacotado
  faz login e lista bancos. Alvo `x86_64-unknown-linux-gnu`, `rustc 1.94.1`.

- **O de Windows tem a forma certa.** Os três `.exe` e a `.dll` são PE32+
  x86-64, e as únicas DLLs que importam são do sistema — `KERNEL32`,
  `msvcrt`, `ntdll`, `WS2_32`, `bcryptprimitives`,
  `api-ms-win-core-synch-l1-2-0`. Nenhuma do mingw, então não há runtime para
  acompanhar o pacote. A `phxsql_odbc.dll` exporta os 21 símbolos ODBC.

### Sabido

- **Não há arquivo de licença no repositório**, e `Cargo.toml` e `README.md`
  dizem `MIT OR Apache-2.0`. Escolher e colar o texto é decisão do dono; o
  empacotador não inventa um.

- **Os zips não são reproduzíveis byte a byte** entre duas rodadas: o hash da
  senha de demonstração leva sal novo a cada execução, porque sai do próprio
  `phxsqld --senha` e não de uma constante colada no script. Trocar isso
  poria uma senha pré-computada em circulação; o preço não vale.

- **O `.exe` não foi executado.** Sem Windows e sem `wine`, o pacote de
  Windows é conferido pela forma (PE32+ x86-64), pelas DLLs que importa — só
  as do sistema, nenhuma do mingw para acompanhar — e pelos 21 símbolos ODBC
  que a `phxsql_odbc.dll` exporta. Dizer mais que isso seria inventar.
## Não lançado — o dossiê refeito contra o código

O pedido foi curto: *«o dossiê está desatualizado, falta o `.bkp`, não é
responsivo, precisa de download, e quero capturas do login até replicação,
profiler e SQL Check»*. Refazê-lo conferindo **seção por seção contra o
código** — que é o único jeito de achar o que envelheceu — devolveu seis
afirmações erradas, e duas delas estavam **dentro do produto**, não só no
documento.

### Corrigido

- **O painel da replicação dizia 28.914 linhas/s e 4.357 eventos/s** enquanto a
  seção da bancada, **no mesmo documento**, mostrava 34.048 e 17.450. Número
  digitado à mão em dois lugares é número que um dia diverge, e este já tinha
  divergido. Agora sai do `bancada/replicacao/resultados.json`.
- **A tela de Replicação do console** repetia os mesmos 18.773 e 4.273, de antes
  da marca de posição no source. Corrigida, com o motivo escrito ao lado.
- **O Gerir banco listava «Triggers · Procedures · Jobs» como *ainda não
  existe*** — depois de os três passarem a existir. Jobs tem tela e botão na
  barra desde o pedido 51; gatilho e procedimento entram pela op `sql` desde os
  pedidos 49 e 50. É a mesma falha da *configuração que não é lida*, virada do
  avesso: **uma tela que nega um recurso que o produto tem**. Entrou um grupo
  com o nome certo, *«Existe pelo comando, e não por esta tela»*, e a diferença
  entre as duas frases é a correção inteira.
- **A receita do KiB de interface era uma lista de três arquivos copiada dentro
  do gerador.** O `http.rs` passou a embutir **nove** — diagrama ER, telemetria,
  multitela e a integração com a Claude entraram depois —, e o rodapé publicava
  **780 KiB** quando a interface tinha **1.032**. A lista sai do próprio
  `http.rs` agora, ignorando o `include_str!` que vive dentro de `#[cfg(test)]`,
  senão 25 KiB de markdown entrariam na conta. *A receita de um número também
  envelhece.*
- **O `<title>` passou a versão inteira dizendo «Dossiê PhxSql 0.15»**, com o
  selo logo abaixo dizendo 0.18.0 — o mesmo defeito que o selo existe para
  impedir, uma linha acima dele. Agora ele é gerado.
- **O organograma dos arquivos mostrava cinco.** Faltavam o `.trash`, o
  `.reason`, o `.lgpd`, o `.pag` e — o que o dono apontou — o **`.bkp`**. A
  figura passou a separar os **sete que sempre existem** dos **três
  condicionais**, com o que decide a existência de cada um; e nos três, a
  ausência do arquivo é uma resposta, nunca um erro.
- **O organograma do código mostrava quatro crates**, e são sete: a
  `phxsql-sql`, a `phxsql-cmd` e a `phxsql-odbc` entraram depois. A seta cheia é
  *depende de* e a tracejada é *fala o protocolo* — o console de terminal e o
  driver ODBC são clientes da porta 5000, e não linkam o motor.
- **«57 das 60 operações do protocolo têm tela»** envelheceu junto com o
  catálogo, que quase dobrou. Medido casando os `api("…")` de `ui/` com os nomes
  de `OPERACOES`: **100 de 112**, e as 12 de fora estão nomeadas uma a uma.
- **«Ainda não há SQL escrito à mão»** e **«camada SQL: rusqlite atrás de uma
  *feature*»** ficaram no dossiê depois de a `phxsql-sql` existir — e ela não
  foi por ali justamente porque uma crate furaria o zero dependências.

### Adicionado

- **Doze seções novas**, cada uma conferida contra o código ou contra um número
  medido: o dado pessoal (a marca, a trilha `.lgpd` e a cifra da coluna), a
  restauração de backup, o cluster, o console em imagens, o multitela, as
  grades, a telemetria e o profiler, SQL/gatilhos/procedimentos/jobs, as portas
  de fora (ODBC, MCP e a Claude), os seis idiomas, e **«O que este motor não
  faz»** — a que diz, sem rodeio, que não há transação (logo não é ACID), que
  não há TLS, que o `excluir` ainda perde para o MySQL(R) por 1,3× e que a
  interface está 11% traduzida.
- **Vinte capturas do console**, contra o servidor de verdade: login → painel →
  tabelas → grade → query → diagrama ER → telemetria → profiler → replicação,
  mais o **multitela com as quatro telas lado a lado** numa janela de 2.800 px,
  nos dois temas. `docs/dossie/capturar-dossie.mjs` sobe um `phxsqld` só dele na
  faixa **6700/6701**, popula três tabelas ligadas por chave estrangeira mais um
  segundo banco, e derruba **pelo PID**.
- **O download**, e ele é `window.print()` com uma folha `@media print` própria
  — fundo branco, índice e botão fora, figura, tabela e captura sem quebra no
  meio, galeria em duas colunas. `<a download>` seria **inerte**: o visualizador
  do artefato bloqueia todo download que a própria página começa, `data:` e
  `blob:` inclusive, e sem erro visível. A página **diz** o que o botão faz.
- **Três geradores novos** — `capturas-no-dossie.py`, e o modo dossiê do
  `pagina-dos-pedidos.py` e do `cobertura-por-area.py` —, mais quatro blocos
  gerados nos que já existiam. **Nenhum número visível do dossiê se digita.**

### Mudado

- **Responsivo, e medido** nas seis larguras (390, 820, 1180, 1920, 3440 e
  5120), nos dois temas: **zero rolagem lateral** em todas, texto corrido
  parando em `74ch`, **nada centralizado** (o corpo começa a 310 px em qualquer
  largura — num monitor duplo o meio da janela é a emenda física entre os dois),
  e a largura extra virando **mais coluna** na galeria: 1 → 3 → 6.
- **A seção «Estado e roteiro» deixou de ser digitada.** Eram oitenta linhas à
  mão com a contagem de testes de cada peça ao lado, e envelheciam a cada
  rodada. Viraram dois blocos de gerador — a contagem dos pedidos e a cobertura
  por área — mais o roteiro apontando para onde ele é mantido.
- **A catraca dos idiomas desceu de 2.000 para 1.999**: o item «Jobs» ganhou o
  par `rot:`/`txt:` ao passar a apontar para a tela que já existia. Um só, e ele
  desce a catraca junto.
- **O dossiê 0.15 saiu do repositório.** Só existe um por vez, para que ninguém
  atualize o errado.

### Sabido

- **A seção do fluxo de gravação ainda não desenha o `.lgpd`.** A legenda diz
  que ele nunca é escrito numa inserção — que é a informação que importa —, mas
  o caminho da *alteração*, que é onde a trilha grava, não tem figura própria.
- **As capturas são de um servidor recém-populado**, então o painel mostra
  números pequenos e a telemetria mostra poucas bolhas. É honesto e é pouco
  vistoso: uma carga grande deixaria as telas mais bonitas e o dossiê menos
  reproduzível.
- **O dossiê fecha em ~2,4 MB** por causa das capturas embutidas. Elas precisam
  ser *data URI*: a página publicada é um arquivo só, e a política de conteúdo
  do visualizador bloqueia imagem de qualquer outra origem — ao lado, seriam
  vinte quadros quebrados e nenhum erro visível.
**As baterias estavam todas aqui; o relatório é que não estava.** Eram oito
comandos, em três linguagens, espalhados por seis diretórios. Quem chegava no
projeto não sabia o que rodar, e ninguém sabia dizer, num só lugar, se o
projeto estava verde. E a regra da casa — *todo teste novo tem de falhar com o
defeito reposto* — era cumprida à mão, uma vez, por quem escrevia o teste, e
depois se perdia: ninguém conseguia dizer, hoje, quais das 1.229 asserções
ainda pegariam o defeito que as motivou.

### Adicionado

- **`python3 phxsql/provar.py`** — um comando roda as **dezesseis** partes.
  Ele não refaz bateria nenhuma: chama, cronometra, guarda o log de cada uma e
  soma. **Recusa rodar com binário velho** (a página é `include_str!`), herdando
  a guarda que a bateria de frontend já tinha, e estendendo-a aos `examples`,
  que o `cargo build --release` não recompila sozinho. Recusa é `exit 2`: não
  rodar não é reprovar.
- **O que foi PULADO aparece no relatório, com o motivo.** Bateria que esconde
  o que não rodou mente por omissão. Porta ocupada vira pulo e não reprovação —
  há outras frentes na mesma máquina, e uma bateria que acusa a vizinha de
  defeito é pior que uma que não roda. `--exigir-tudo` transforma pulo em
  reprovação para quem quer o portão apertado.
- **`bancada/guardas/`** — o catálogo dos **defeitos repostos**, e o executor
  que os repõe. Cada entrada traz o arquivo, o trecho de hoje, o trecho do dia
  do estrago e **quais testes têm de cair**. O executor copia a árvore, repõe um
  defeito por vez, roda só o binário nomeado, desfaz num `finally` (com rede no
  `atexit`) e julga. Cinco vereditos, e o que importa é o `NAO PEGOU`: teste que
  passa por engano. **Medido: 18 guardas, 16 provadas, 2 redundantes, zero «não
  pegou», em 162 s de mutação.** A tabela no `docs/TESTES.md` sai de um gerador.
- **`bancada/odbc/provar.py`** — o passo do meio que faltava. A prova de ABI e a
  montagem dos dados já existiam; o que estava escrito só em prosa era «suba um
  phxsqld seu com token `prova-odbc`». Passo em prosa não entra em bateria, e
  por isso a parte `odbc` era um pulo permanente.
- **`bancada/guardas/tabela-no-testes.py`** — a tabela das guardas no
  `docs/TESTES.md` sai de um gerador, como as duas de cobertura. Número visível
  que não sai de gerador está errado e ninguém percebeu ainda.

### Corrigido

- **Dois conferidores da telemetria saíam com código 0 imprimindo «FALHAS» na
  tela.** `conferir-desenho.mjs` e `conferir-interacao.mjs` mediam certo e
  imprimiam certo, e nunca souberam reprovar. Lidos por gente, acusavam;
  chamados por uma bateria que soma códigos de saída, mentiam verde — e o
  buraco só existe a partir do dia em que aparece o orquestrador. O do desenho
  também passou a reprovar quando o contraste medido fica abaixo de 4,5:1, que
  ele já calculava e só imprimia.
- **A ficha do teste da cifra afirmava algo falso.**
  `trocar_o_corpo_de_uma_linha_pela_outra_nao_passa` dizia que tirar o `aad` do
  `montar_slot` e do `abrir_slot` o derrubava. Medido, com o defeito reposto de
  verdade: **não derruba.** O endereço está amarrado **duas vezes** — o
  `aad_do_slot` e o `nonce_de_pedaco` carregam os mesmos `(volume, rowid,
  versao)` —, cada uma segura sozinha, e o teste só cai quando as duas somem.
  Nada foi removido: o AAD é defesa em profundidade, e no dia em que o nonce
  virar sorteado ele passa a ser a única fechadura. O que mudou foram as
  fichas, que agora dizem a verdade medida, e três entradas do catálogo que
  travam a conta. `docs/SEGURANCA.md` §11.11.
- **A prova da replicação estava reprovando, e ninguém sabia.**
  `bancada/replicacao/modos.py`, estágio (g), exigia o nome de erro
  `ESCRITA_NA_REPLICA`. O commit *«um redirecionamento, não dois»* fundiu esse
  erro com o `Redireciona` do cluster — os dois sempre tiveram o código 4003 e
  sempre quiseram dizer a mesma coisa — e atualizou o teste unitário, não a
  bancada: ela não estava em portão nenhum, então ninguém a rodava. Agora
  aceita os **dois** nomes, como o `replica.rs` já faz ao ler do fio, para uma
  réplica de hoje continuar entendendo um source antigo. **Uma prova que não
  está em nenhum portão não é uma prova — é um arquivo.**

### Sabido

- **A tela mente sobre si mesma quando o Painel demora.** `abrirAdmin` faz
  `p.innerHTML = await vPainel()` e escreve sem perguntar se aquela ainda é a
  tela aberta. Quem clica em Configurações antes de o Painel carregar fica com
  o **título de Configurações e o corpo do Painel** — medido com sonda, o
  rastro está em `docs/TESTES.md` §9.8. A janela cresce com a carga da máquina,
  e é por isso que a prova das cores passava quando foi escrita e reprova hoje.
  **Não consertado de propósito**: `ui/index.html` é a tela, há frentes mexendo
  nela nesta rodada, e a decisão é de quem manda no Centro de Controle. A parte
  `telemetria-cores` fica vermelha até lá, que é o comportamento certo.
- A parte `dblink` continua pulando sem um MySQL(R) com o banco `crm` no ar, e
  a `profiler-disco` roda como **sonda** (sai zero sempre) e não como prova —
  as duas com o motivo declarado no relatório.
- As **medições** (`bancada/carga/`, `custo.py`, `replicacao/medir.py`) ficam
  fora de propósito: um número mais lento não é uma reprovação, é um número, e
  bateria que fica vermelha por causa da carga da máquina ensina a ignorar
  vermelho.
## Não lançado — os quatro modos de replicação em contêiner

O pedido era testar os quatro modos em Docker. O que ele valeu não foi
repetir o teste dentro de um contêiner: foi que **rede própria, endereço de
verdade e firewall de verdade acham defeito que loopback esconde**. A bancada
está em `bancada/replicacao/docker/` — cinco `compose`, imagem `scratch` de
6,42 MB, um comando só. `docs/REPLICACAO.md` §17.

### Corrigido

- **`replicas_autorizadas` não era lido por ninguém.** O campo estava no
  `config.json`, na §7 do `REPLICACAO.md` e na tela de configuração desde que
  os papéis novos entraram, e **nenhuma linha de código o lia** — a §7 dizia
  que o desenho era imposto «em dois lugares» e havia um. Medido em contêiner,
  com o modelo de ameaça real: um vizinho de rede com o `config.json` de
  réplica vazado (mesmo token, mesmo usuário, mesmo `senha_hash`) levou os
  **200 de 200 eventos** do diário do source *com a lista preenchida*. Hoje
  leva 0. O portão é **um só** (`portoes_do_pedido`, portão 2a-bis, sobre
  `posicao`/`replicar`/`aplicar`), a guarda entra **pedida e não imposta**
  (lista vazia libera todos, byte a byte o comportamento de sempre), e a
  pergunta obrigatória sobre o campo novo — o IP da sessão — foi respondida:
  job, rotina interna e a replicação chamada de dentro chegam com `ip` vazio e
  não são barrados. Três testes, e o que mais importa é o do comportamento
  velho: `sem_replicas_autorizadas_nada_muda`.

### Adicionado

- **`bancada/replicacao/docker/`**, com um `compose` por modo (A
  source→réplica, B multi-master, C spare, D read replica) e um quinto só para
  o firewall da §7: rede própria com IPs fixos, um **intruso** e `iptables` de
  verdade dentro do namespace de rede do source. Com as regras da §7, três
  coisas medidas juntas: o intruso leva *timeout* (e não recusa — `DROP` não
  responde), a réplica autorizada continua replicando, e **o source não
  consegue abrir conexão para ninguém** — a metade de saída do desenho, que
  nunca tinha sido provada.
- **`erro.replica_nao_autorizada`** na tabela de mensagens, nos seis idiomas.
- **`exemplos/Config_docker.json` e `Config_docker_replica.json`**, que o
  `phxsql/Dockerfile` copia e que **não existiam** — moravam só na raiz do
  repositório, e por isso o `docker build` oficial parava no `COPY` antes de
  compilar coisa alguma. Além de existirem, os dois agora **replicam entre si**:
  o modelo do master não tinha bloco `replicacao`, então subia isolado, e a
  réplica que ele acompanha não teria o que aplicar.

### Sabido

- **O abraço mortal do bidirecional.** `alcancar_tabela_bidi` toma a trava de
  dados **deste** servidor e, de dentro dela, pede `replicar` ao outro; do
  outro lado, servir `replicar` e `posicao` também precisa da trava de lá. Com
  fila nos dois ao mesmo tempo, cada um segura a própria trava esperando a
  resposta do outro, e ninguém sai até o prazo de leitura de **30 s** estourar
  nos dois — e eles podem reentrar em passo. Medido **sem corte nenhum**, só
  com escrita simultânea nos dois lados: 50.000 linhas em cada metade ao mesmo
  tempo levaram **33,3 s**, contra ~5,8 s das mesmas 100.000 num servidor só em
  modo A, com um `EAGAIN` no diário de cada um. É a consequência do item 2 da §3.2 do
  `PENDENCIAS.md` (as tomadas da trava fora do ponto único), agora com número.
  Não foi consertado nesta rodada: mexer no laço da replicação para buscar
  fora da trava é uma frente própria, com testes próprios.
- **O `REDIRECIONA` aponta o endereço da origem configurada**, que é «por onde
  *eu* alcanço o primário» e nem sempre «por onde *você* alcança». Em
  contêiner isso é um nome de serviço que o cliente do hospedeiro não resolve.
- **`bind: 127.0.0.1` dentro de um contêiner não replica e não avisa**: zero
  evento na réplica, zero erro em qualquer log. É o erro de configuração mais
  fácil de cometer ao pôr o PhxSql em produção.
- **A compilação DENTRO do contêiner continua não provada nesta máquina.** O
  `phxsql/Dockerfile` agora tem todos os arquivos que copia, mas o
  `rustup target add` do estágio construtor não alcança `static.rust-lang.org`:
  a saída passa por um proxy que intercepta TLS e o contêiner de build não
  confia na CA dele. É limite do ambiente, e o que ficou provado é o resto —
  a imagem `scratch`, o binário musl e os quatro modos em cima dela.

---

## Não lançado — a área de trabalho multitela

Escrito em paralelo com a restauração de backup, abaixo: na integração as duas
seções «Não lançado» viram uma só, e o número da versão sai de lá.

**O console tinha uma tela por vez.** `folha(...)` trocava o `innerHTML` do
`#painel`, e abrir a próxima matava a anterior — uma consulta com resultado,
uma grade rolada até a linha 800, a telemetria coletando, tudo perdido ao
trocar de tela. O pedido veio no molde do WINDEV(R), e o dono fechou a questão
por escrito: *«é um site, então tem que esticar o navegador para todas as telas
e dentro da página 1 ou índex distribuir as janelas dentro da mesma page»*.

### Adicionado

- **Abas dinâmicas**, com estado **por aba**. `est` passou a ter duas metades
  declaradas: o que é do servidor (sessão, usuário, bancos) continua único, e o
  que é da tela (`atual`, `aba`, `ordem`, `grade`, `pivot`…) troca junto com o
  foco. Duas tabelas em duas abas não brigam mais pelo mesmo `est.atual` — o
  defeito que só aparece com duas abertas, que é o que ninguém testa.
- **Regiões lado a lado**: a área central divide em 2, 3 ou 4 colunas, cada uma
  com a própria tira de abas e uma calha arrastável entre elas. É a foto da
  ultrawide com o WINDEV em três painéis — e é **layout**, então funciona em
  qualquer navegador. As quatro telas nomeadas (Diagrama ER, Telemetria,
  Profiler, Query) abrem juntas e vivas.
- **Janelas soltas dentro da página**: flutuantes, arrastadas pelo cabeçalho,
  redimensionadas pelo canto, com ordem de sobreposição ao clicar. Soltar e
  acoplar **movem o mesmo nó do DOM**, e por isso não perdem campo digitado
  nem resultado; a rolagem, que mudar de pai zera, é salva e reposta.
- **O pino** — o **mesmo glifo e o mesmo significado** do painel lateral, e a
  mesma frase de rodapé. Guarda no `localStorage` quantas regiões, a largura
  de cada uma, que aba em qual, e x/y/largura/altura das janelas. Em **pixel
  CSS**, porque é a unidade que decide se a grade cabe.
- **Rota por URL** para as telas com endereço próprio (`?tela=query`,
  `?tela=diagrama&db=…`, `?tela=tabela&db=…&tab=…`), e uma **janela destacada**
  do sistema para quem preferir. A ficha de sessão viaja pelo
  `BroadcastChannel`, em memória, e **nunca** encosta no `localStorage`.
- **Alinhar as regiões com as emendas físicas dos monitores**, quando a
  `Window Management API` estiver disponível — o melhor uso dela aqui, e a
  resposta ao monitor de 49" por daisy chain, que é «dois monitores num».
- Dois casos novos na bateria de frontend (**24 execuções**), e
  `testes-web/medir-regiao.mjs`, que mede de quanto uma região precisa.

### Mudado

- Os ids `#painel`, `#titulo`, `#subtitulo` e `#abas` passaram a morar **só na
  tela com foco**; as outras se vestem por classe. Foi o que permitiu quatro
  telas na tela sem tocar em nenhuma das centenas de `$("#painel")`.
- `folha(...)` deixou de parar o relógio **global**: agora solta o laço **da
  aba**. Parar o global mataria a telemetria da aba ao lado, que nem foi
  tocada.
- Ctrl/Cmd/botão do meio na barra de ferramentas, no menu e na árvore abre em
  **aba nova**. O clique simples continua trocando o conteúdo da aba de agora,
  que é o que sempre fez.

### Sabido

- **Arrastar uma janela do sistema de volta para a barra de abas não existe** —
  em navegador nenhum. Não há evento quando uma janela passa por cima de outra.
  Há «⤺ devolver» e «⇤ acoplar» no lugar, e o documento diz isso.
- **As janelas destacadas não reabrem sozinhas** ao carregar: `window.open` sem
  clique é bloqueio de popup em todo navegador.
- **Quatro telas visíveis custam ≈ 90 pedidos/min** (medido: 15 em 10 s). No
  modo lado a lado ninguém está escondido, e pausar seria mentir sobre o que a
  tela mostra. Os relógios da Telemetria (2 s) e do Profiler (1 s) **não são
  escalonados** — podem cair no mesmo instante. É pendência medida.
- A `Window Management API` **não é exercitada de verdade** pela bateria: ela
  pede a permissão `window-management`, que o Playwright 1.56 não sabe
  conceder. O caso a dubla e prova o caminho nosso. O DPI diferente, esse, se
  prova de verdade, com um contexto de 2×.
- A **troca de DPI em voo** tem o ouvinte escrito (`matchMedia` na resolução) e
  não tem prova automatizada: não há como mudar a densidade de uma página já
  carregada pelo Playwright.

---

## Não lançado — a restauração de backup

O número da versão fica para a integração: escrevê-lo aqui antes de o
`Cargo.toml` mudar seria criar um número que ninguém mediu.

**Backup que não restaura não é backup.** O botão *Restaurar* existia como
promessa apagada desde que a tela de backup nasceu, e o pedido chegou com essas
palavras. Agora ele restaura.

### Adicionado

- **`restaurar_backup`**, com dois modos. `novo` (o padrão) grava o backup com
  **outro nome**: não destrói nada, não precisa parar serviço nenhum e não
  segura a trava durante a cópia — o database de destino ainda não existe, e
  ninguém está lendo dele. `por_cima` substitui um database que já existe, e
  exige três coisas: a **porta de dados parada**, nenhuma conexão de dados
  aberta e `"confirmar":true`. O database substituído **não é apagado** — sai
  da raiz de dados e o caminho volta em `anterior_em`.
- **A conferência acontece antes de o destino ser tocado.** A cópia é extraída
  para um palco fora da raiz de dados e o SHA-256 de cada arquivo é conferido
  contra o `backup.json`; só então a troca acontece, com um `rename` e com a
  trava na mão. Backup corrompido não vira database pela metade — não vira
  database nenhum.
- **`backups`**, a lista das cópias de uma pasta com o que cada uma traz
  dentro. De um ZIP lê só o fim do arquivo e o manifesto: listar dez cópias de
  um gigabyte não custa dez gigabytes.
- **Leitura de ZIP e INFLATE completo** (`phxsql-core::zip`): o diretório
  central, o CRC-32 de cada entrada e os **três** tipos de bloco da RFC 1951 —
  sem compressão, Huffman fixo e Huffman **dinâmico**, que o nosso compressor
  nunca emite e todo compressor do mundo emite. O teste do dinâmico é um vetor
  produzido pela zlib, e não uma ida e volta com o próprio código: os dois
  lados podem estar errados juntos.
- **A tela**, com as duas formas lado a lado, o que a cópia tem dentro antes de
  decidir, e o botão **Restaurar na barra de ferramentas**, ao lado do Backup —
  botão que não se acha não existe. Também no menu *Arquivo*, junto de
  *Conferir um backup…*.
- `docs/RESTAURACAO.md`: o desenho, as três saídas possíveis, a que foi
  escolhida e **o que a restauração não garante**.

### Mudado

- **O manifesto do backup diz de que ele é cópia** — `escopo` (`raiz` ou
  `database`) e `database`. Os caminhos quase sempre bastariam para deduzir, e
  «quase sempre» numa restauração quer dizer *restaurar um schema como se fosse
  um banco*: um database que só tenha schemas se escreve igualzinho a uma raiz.
  Manifesto antigo continua valendo — cai na dedução, e a resposta diz que
  deduziu em vez de afirmar.

### Corrigido

- **O portão de permissão não enxergava o database que vem DENTRO do backup.**
  Ele confere o campo `"database"` do pedido, que na restauração é o destino;
  sem uma conferência própria, bastaria administrar um banco de rascunho para
  despejar nele o backup da folha e ler tudo. É a mesma porta dos fundos do
  `juntar` e do `unir`, e o conserto é o mesmo — um portão próprio dentro da
  operação. Tem teste, e o teste falha com o portão retirado.
- **O palco da restauração ia parar no `/tmp`.** Com `base: "dados"` — o padrão
  do `config.json` — o pai do caminho é vazio, e o código caía no temporário do
  sistema. `/tmp` costuma ser outro sistema de arquivos, às vezes um `tmpfs`: a
  troca deixaria de ser um `rename` para virar uma cópia do database inteiro
  para dentro da RAM, no meio da trava. **Achado exercitando pelo navegador**,
  não lendo — no teste unitário o `base` é sempre absoluto.
- **A tela mentia sobre o dado, de novo.** O título da seção usava a classe
  `.secao`, que é caixa-alta: um database chamado `Comercial` aparecia como
  `COMERCIAL`. É «Blumenau» virando «BLUMENAU» por outro caminho, e também só
  apareceu abrindo a página.

### Sabido

- O manifesto prova que o backup **não apodreceu**, não que ninguém o
  reescreveu de propósito: quem alterar um arquivo *e* recalcular o SHA dentro
  do `backup.json` passa. Assinar o manifesto ainda não é feito.
- Não há «restaurar a raiz inteira»: um database por vez, de propósito.
- Cópia em ZIP acima de 4 GiB não cabe no formato (deslocamentos de 32 bits) —
  já era assim antes. O que mudou é que agora **falha alto**: o leitor confere
  a assinatura e o nome do cabeçalho local contra o diretório central, em vez
  de restaurar lixo em silêncio.
## Não lançado

A rodada da **bateria de testes** — backend, frontend e avaliação de design.
O que ela mostra em uma linha: **1.106 testes verdes não provam uma tela**.
Os seis defeitos de interface abaixo aconteceram sem uma única exceção não
capturada, e o portão de permissão tinha três operações a mais que leem a base
inteira sem o campo que ele confere. `docs/TESTES.md` e `testes-web/LEIA-ME.md`.

### Corrigido

- **A tela de Dado pessoal (LGPD) nunca auditou nada.** Ela procurava um campo
  booleano `pessoal` por coluna, e o servidor nunca mandou esse campo — o
  esquema responde `dado_pessoal`, em texto, e existe uma op própria
  (`dados_pessoais`) feita para essa varredura. O efeito era o pior possível
  para o assunto: a tela dizia, em vermelho e para toda base, «o esquema deste
  servidor ainda não traz a marca» — **«não sei» sobre um motor que sabe**, numa
  tela de conformidade. Nenhum teste de `cargo test` podia pegar: o servidor
  estava certo dos dois lados, e quem lia errado era a página. De quebra, a op
  filtra tabela a tabela pelo direito de quem pergunta; o laço da tela refazia
  essa conferência por fora.
- **O `pivotar` era a porta dos fundos para a tabela negada.** O portão confere
  o campo `"tabela"` do pedido, e o pivot tem **dois** lugares com tabela: a de
  fatos em `tabela` e a lista `juntar`, com um `"tabela"` **dentro de cada
  item**. Bastava juntar a tabela negada e pedir um campo dela em `linhas` — os
  rótulos das linhas do cruzamento **são** os valores dela, e a resposta ainda
  dizia o nome da tabela e quantas linhas ela tem. É a terceira operação da
  mesma família do `juntar` e do `unir`; foi esquecida porque nela o campo tem
  o nome certo, só que **aninhado**.
- **`sequencias` e `posicao` mostravam a tabela que a árvore esconde.** As duas
  percorrem a base inteira sem campo `tabela`. A primeira devolvia nome,
  contador e quantidade de registros de toda tabela; a segunda, com
  `com_esquema`, devolvia o **esquema cru** de cada uma. Filtram por dentro
  agora — `ler` numa, `replicar` na outra, que é o direito que o portão aplica
  a cada uma. O teste que mais importa é o do comportamento velho: a réplica
  sem regra por tabela continua vendo tudo.
- **`duplicar_tabela` não conferia o destino.** O portão confere `criar` contra
  o campo `tabela`, que ali é a origem — e a tabela que nasce tem o nome do
  campo `destino`. Quem podia criar nominalmente uma tabela criava qualquer
  outra, duplicando a permitida. O `copiar_tabela` ao lado já conferia o
  destino dele.
- **A tela de entrada ficava em branco por 12,7 s** num servidor cuja rede
  **engole** o pedido da fonte da marca (firewall com DROP, proxy que só
  responde reset depois do prazo) — que é a rede em que um servidor de banco
  mora. A folha de estilo bloqueante parava o parser, e com ele o primeiro
  `<script>` e o `DOMContentLoaded`. Medido nas duas redes, três rodadas cada:
  12.778/12.743/12.677 ms contra 124/115/101 ms com recusa imediata; depois do
  conserto, 125/103/101 ms — **116×**. `media="print"` + `onload` faz o
  navegador buscar sem bloquear a pintura, e o `noscript` devolve o caminho
  antigo para quem desligou o script.
- **O `input{width:100%}` global mordeu a tela «Nova tabela»** — o checkbox
  `obrig.` media 57×13 px, o `único` 114×13 e o radio `primária` 161×13. Era o
  quarto caso; em vez do quarto remendo pontual, entrou uma regra para **toda
  célula** (`td/th input[type=checkbox|radio]`), para o próximo componente
  nascer certo.
- **A caixa de marcar separada do próprio texto.** `.criar .chk` trocava o
  `display` para `flex` e não o `flex-direction`, e `.criar label
  {flex-direction:column}`, vinte linhas abaixo, vencia: «exigir motivo
  escrito» e «tabela particionada» apareciam com a caixinha **em cima** do
  texto, os dois jogados na borda direita. No código as duas regras estão perto
  e cada uma está certa sozinha.
- **O nome do índice saía em caixa alta na tela de LGPD** — `porNome` virando
  `PORNOME`. É a mesma mentira sobre o dado do «BLUMENAU»: nome de índice é
  dado, e mostrar dado numa caixa que ele não tem faz quem lê não saber como
  está gravado.
- **O chip «ativas» da grade reprovava no contraste do tema claro.** Ele trazia
  uma tinta quase preta fixa sobre `var(--laranja)`, que no tema claro escurece
  para `#c63c0a`: **3,85:1**, abaixo dos 4,5:1. Era o único lugar que não usava
  o token `--tinta-botao`. E o número mostra por que se mede: a conta de cabeça
  dava 2,65:1.

### Adicionado

- **`testes-web/`, a bateria de frontend que roda sozinha.** Onze casos, dois
  temas, 22 execuções em ~2min20. Sobe um `phxsqld` próprio nas portas
  6200/6201, entra pela tela de login com o desafio-resposta de verdade,
  percorre **112 telas** clicando cada item dos nove menus e cada botão da
  barra, e derruba o servidor **pelo PID**. Ela **recusa rodar com binário
  velho** — a página é `include_str!`, e exercitar a versão anterior aprovaria
  uma correção que o servidor nem serve.
- **Três canais de erro, e não um.** O `ligarMenu` manda toda exceção de item
  de menu para `avisar(..., true)`: capturada, ela nunca vira `pageerror`. O
  passeio olha `pageerror`, `#aviso.mal` e `#painel .aviso.mal`, e limpa os
  dois últimos antes de cada clique.
- **Oito testes de backend**, todos com o defeito reposto provando que pegam:
  `pivotar_nao_e_a_porta_dos_fundos`, `sequencias_esconde_a_tabela_negada`,
  `posicao_esconde_a_tabela_negada`, `duplicar_confere_o_direito_no_destino`,
  os três «continua valendo» que travam o comportamento velho, e
  `nenhuma_credencial_do_config_sai_pela_op_config` — **um** teste para todas
  as credenciais do `config.json`, com dez marcas distintas, no lugar de um por
  campo que não pega o campo que alguém acrescentar amanhã.
- **`docs/dossie/cobertura-por-area.py`**, que regrava as duas tabelas de
  cobertura do `docs/TESTES.md` a partir do código. Tabela de cobertura
  digitada mente no dia seguinte ao primeiro teste novo.

### Sabido

- **`replica.rs` continua sem nenhum `#[test]`** — 352 linhas, o laço que faz a
  réplica alcançar o master, e a prova é o `bancada/replicacao/`, que precisa
  de quatro servidores no ar e não roda no portão de commit.
- **`duplicar_tabela` e `copiar_tabela` não conferem `ler` na origem.** Copiar
  uma tabela é ler os bytes dela, e as duas são `Atividade::Criar`. Exigir
  `ler` mudaria o significado de um `config.json` que já existe, e guarda nova
  entra pedida — o caminho exato está em `docs/TESTES.md` §5.1.
- **A gaveta fechada no celular não reabre ao voltar para o desktop**: o
  fechamento automático é gravado como se fosse escolha da pessoa. Os dois
  lados são ruins, e o conserto é desenho — `docs/TESTES.md` §5.3.

---

## 0.18.0 — 2026-08-29

A rodada dos concorrentes. Três motores lidos no fonte — InnoDB, Aria e
Cassandra (`docs/CONCORRENTES.md` e `docs/CASSANDRA.md`, toda citação com
`arquivo:linha`) — e cada ideia **medida aqui antes de entrar**. O resultado
que resume a versão: **a bancada de dez milhões passou a ganhar do MySQL(R) no
insert** — 91,5 s contra 112,4 (109.300 linhas/s contra 88.994), ganhando
também buscar (13×), varrer (11×) e atualizar (12×). Só excluir ainda perde.

### Corrigido

- **O medidor com binário velho media o passado.** `cargo build --release` não
  recompila os *examples*, e a bancada chama `target/release/examples/carga`
  direto: uma rodada inteira de ganhos ficou invisível, e a conclusão «o
  esquema custa 2,2×» nasceu — com tabela e tudo — dessa diferença. É o sétimo
  diagnóstico plausível que a medição derruba, e este era nosso duas vezes. A
  receita do `bancada/LEIA-ME.md` já mandava certo; a lição foi para o
  `CLAUDE.md`.
- **`Table::abrir` lia o volume inteiro do `.reg`** para tirar dele 128 bytes
  de cabeçalho e o bloco de esquema — 69 ms por milhão de linhas, a cada
  abertura, e o servidor abre a tabela a cada pedido. Duas leituras curtas:
  138,80 → 0,03 ms, e plano em vez de linear. Buscar na bancada caiu de 4,04
  para 0,20 s.
- **A réplica não ficava para trás por culpa dela.** A causa registrada
  («aplicar reencoda o payload») custa 0,35 µs de 229; o custo era o **source**
  varrendo o diário desde o começo a cada lote — quadrático. Marca de posição:
  45×, e 4.273 → 17.450 eventos/s por réplica. As três juntas passam o master.
- **Reabrir a tabela reescrevia o esquema**, e com o bloco v6 mais longo a
  primeira gravação comeria o slot 1 em silêncio, com CRC batendo. Agora os
  bytes do disco são preservados; o teste fabrica um arquivo antigo de verdade.
- **A tela de telemetria existia e ninguém a achava.** O pedido chegou como
  «falta o botão do SQL Check» com o botão no ar havia semanas — no terceiro
  grupo da barra, entre coisas que se fazem uma vez por mês, e sem aparecer em
  menu nenhum, embora o menu *Ferramentas* se anuncie como «a mesma lista pelo
  teclado». Telemetria e Profiler subiram para junto de *Conexões* (as três
  respondem à mesma pergunta: o que está acontecendo agora), entraram no menu
  *Ferramentas*, e a referência que o Adriano usa para nomear a tela passou a
  aparecer nela: o balão do botão e o subtítulo dizem **«no molde do SQL Check
  da Idera(R)»**. O nome de fábrica continua *Telemetria* — a marca é da Idera,
  e a casa cita marca de terceiro, não a adota; quem quiser outro rótulo troca
  no *Editor de menu*. Lugar errado na barra é o mesmo que não existir.

- **O Profiler não era só do administrador, apesar de a ficha dizer que era.**
  Nenhum pedido dele tem campo `"database"`, então o portão geral do
  `despachar` pergunta «pode administrar a base *vazia*?» — e
  `bases: {"*": {administrar: true}}` responde sim para quem é **leitor**. É o
  furo do `juntar`/`unir` com o sinal trocado: lá a operação escapava por não
  ter o campo, aqui a regra curinga a deixava passar. Provado por soquete: um
  usuário que levou **acesso negado** ao pedir `ler` em `folha.salarios` ligou
  o profiler no pedido seguinte e leu no anel o texto inteiro do `inserir` que
  o administrador fez naquela tabela, valor incluído — *e* mandou o servidor
  criar um arquivo no caminho que ele escolheu. Entrou o `portao_do_profiler`,
  irmão do `portao_da_telemetria`, nas quatro operações; o teste que trava é o
  do comportamento **velho**, `sem_cadastro_nada_muda`.
- **O `.txt` do Profiler aceitava linha forjada.** O `pedido` sai seguro do
  `redigir` porque JSON escapa a quebra de linha, mas `op`, `database`,
  `tabela` e `erro` iam crus para a linha do arquivo. Um pedido com
  `"op": "ping\n2000-01-01T00:00:00 9.9.9.9 forjado …"` deixava no arquivo uma
  **segunda linha indistinguível de um evento real**, com outro IP e outro
  usuário — quem lesse o log depois de um incidente estaria lendo o que o
  suspeito escreveu. Todo campo livre passou a entrar no evento reduzido a uma
  linha, com o controle **mostrado escapado** (apagar esconderia a tentativa) e
  com teto de tamanho.
- **Com o disco cheio o Profiler seguia dizendo «gravando em …».** O
  `let _ = writeln!(…)` engolia a falha. Medido num `tmpfs` de 64 KB: 400
  pedidos, **223 linhas** no arquivo, nenhum aviso. Agora `gravados_bytes` e
  `falhas_de_escrita` saem na resposta, a caixa de estado fica **vermelha** e
  diz quantas linhas se perderam, e o rodapé do arquivo registra o número.
- **Duas fugas da redação do Profiler**, achadas mandando pedido torcido:
  `{"senha ": …}` — com espaço dentro das aspas — aparecia inteiro, e a chave
  passou a ser comparada **aparada**; e `["op","senha","…"]` — JSON válido que
  **não é objeto** — virava texto, e passou a virar o tamanho em bytes, pelo
  mesmo motivo que o malformado já virava: a redação é por nome de campo, e ali
  não há nome para tapar.
- **`terminou` procurava o evento do lado errado do anel.** Ele acha pelo
  serial para costurar o desfecho, e varria **do mais antigo** quando o
  procurado é sempre o mais novo. Emparelhado, na carga uma a uma: **1,17×**
  com o anel em 20.000 e 1,00× com o anel padrão de 500 — a instrumentação
  ficava mais cara justamente para quem lhe dava mais memória.
- **A caixa de estado do Profiler nunca foi verde.** `class="aviso bem"`, e a
  classe verde desta interface chama-se `bom` — não existe `.bem` no CSS.
  Nenhum teste pega isso, e ler o código também não: apareceu abrindo a tela.

### Adicionado

- **Cache de páginas *write-back* no `.ndx`**, a ideia central dos três
  concorrentes: a página modificada fica suja em RAM e o CRC-32 e o `write`
  saem no despejo ou no `sincronizar` — não por chave. A garantia trocada é
  comprada de volta pela **marca de sujo** (byte 52 do cabeçalho): vai ao disco
  **antes** da primeira página suja, sai **depois** de todas, e um `.ndx`
  aberto sujo recusa toda operação e manda reconstruir — queda **detectada**,
  nunca silenciosa. Sem migração: arquivo antigo tem zero ali, que é a verdade.
  O empilhamento medido da rodada: 16,4 → 14,5 (cabeçalho do `.ndx` fora do
  caminho da chave — a **terceira vez** do mesmo defeito) → 13,1 (CRC
  slice-by-16, mesmo polinômio, nada muda de valor) → **7,5 µs por linha**.
- **Construção em lote da B+tree** (`construir_em_lote`): ordena, enche as
  folhas em sequência, monta os níveis por cima — 7,72 s → 0,31 s por milhão
  de chaves (23×), com o enchimento de 80% **medido** contra 70/90/95/100.
  Todo `reindexar` anda nisso. O adiamento de índice que ela destravaria foi
  medido e **recusado**: 1,22× no melhor caso, prejuízo abaixo de M≈N/3.
- **`BULKINSERT` medido no fio**: 43.500 → 66.500 linhas/s (1,53×) — a reserva
  mantém a janela de durabilidade aberta e a carga vira um `fsync` só.
- **Cifra nos diários** (pedido 101): ChaCha20-Poly1305 (RFC 8439, todos os
  vetores oficiais) ligada ao `.log`, `.trash` e `.reason` — **desligada por
  padrão**, arquivo antigo abre igual, nonce derivado do offset que o arquivo
  já tem, chave por PBKDF2 e por volume. Com o defeito «cifra imposta»
  reposto, 43 testes antigos quebram. A replicação continua: `posicao` conta
  pelos cabeçalhos claros e `replicar` devolve imagens decifradas pela sessão
  autenticada. E a **compactação foi medida de novo e recusada de novo**, agora
  com o corte do diário configurável (`recursos.diario_volume_mib`): mesmo a
  1 MiB ela poupa 14,7% — o `.ndx` sozinho pouparia 2,1× mais.
- **Marca de dado pessoal por coluna** (pedido 125): PSCH v6, três graus
  (LGPD art. 5º I e II), op `dados_pessoais` que audita a base — com
  conferência própria porque não tem o campo `tabela` que o portão lê — e a
  tela que diz *que não sabe* quando o esquema não traz a marca.
- **Jobs de execução** (pedido 51): agenda, corridas em diário próprio, e **o
  job roda com o poder do usuário dele** — os portões do `despachar` foram
  extraídos para uma função só em vez de copiados.
- **Parar e subir o serviço pela tela** (pedido 40), trocando a porta: um
  despertador no próprio endereço em vez de *polling*; a porta nova é presa
  antes de a velha ser solta, e a web é sempre o caminho de volta.
- **Diagrama ER** (pedido 127, primeira metade) — e `criar_tabela` passou a
  **declarar chave estrangeira pelo protocolo**, com o teste que trava que
  *declarar não é aplicar*. Sete defeitos de tela achados abrindo no navegador.
- **A camada SQL nasceu** (pedido 83): crate `phxsql-sql` (léxico, sintaxe,
  tradutor) e a op `sql` ligada **pelo portão que já existe** — com o teste
  `o_sql_nao_e_a_porta_dos_fundos_para_a_tabela_negada`. Ligar achou o que a
  unidade não achava: `WHERE id = 2` chegava como texto; o motor alargou, o
  tradutor não apertou.
- **O catálogo de operações** (`op catalogo`): as 79 operações do protocolo
  descritas por dados — parâmetros, permissão, exemplo — com um teste que
  deriva a lista do próprio `despachar`. Ajuda escrita à mão não existe para
  envelhecer.
- **`phxsqlcmd`** (pedido 130): console interativo com `/help` e
  `/help comando` vindos do catálogo pela rede, autenticando pelo mesmo
  desafio-resposta da réplica.
- **Servidor MCP com transporte** (pedido 6): `phxsqld --mcp` por stdio, com o
  `tools/list` lendo o catálogo e a senha por variável de ambiente.
- **Cliente e dialeto PostgreSQL(R) no DbLink** (pedido 86): SCRAM-SHA-256
  conferido contra o RFC 7677, dialeto de SQL por motor, e as operações do
  DbLink reescritas para não saberem qual motor atendem. A prova contra um
  PostgreSQL(R) de verdade fica pendente e está dita.
- **`docs/CONCORRENTES.md` e `docs/CASSANDRA.md`** — o que cada motor faz na
  inserção, o que cabe aqui, o que não cabe e por quê. Do Cassandra, a
  resposta à pergunta do quórum: o OK de `QUORUM` **não significa disco** no
  modo padrão — significa recebido em W processos.
- **Sincronia de tabelas primas pelo DbLink** (pedido 132): `dblink_ligar`
  cria a tabela local espelhando a remota (chave primária vira índice único;
  texto desconta os 4 bytes do utf8mb4; `DECIMAL` desconta sinal e ponto) e
  `dblink_sincronizar` converge os dois lados — sentido puxar/empurrar/dois,
  conflito **por linha** decidido pelo dono, colunas casadas **por nome**,
  empurrão reentrável por `ON DUPLICATE KEY UPDATE`, teto com recusa clara.
  Exclusão **não viaja**, por desenho, e a prova confere que o limite é
  verdade. Provada em 7 estágios contra o MySQL(R) 8.0.46 vivo
  (`bancada/dblink/prova-sincronia.py`), inclusive o job rodando sozinho.
- **Assistente de conexão DbLink na tela** (pedido 132): cinco passos que só
  avançam com o anterior provado — conexão, teste, base, tabelas ligadas com
  sentido e dono por linha, e o job `sincronia-<ligação>` com a primeira
  rodada disparada na hora. Exercitado no navegador de ponta a ponta; o
  exercício achou a árvore de databases que não se remontava quando a
  sincronia criava um database novo.

### Sabido

- **Excluir ainda perde** (6,27 contra 4,73 s) — próximo alvo.
- O `sincronizar` a cada 200 operações no servidor **dobra** o custo por linha
  (§4.9); tirá-lo do caminho muda o contrato de durabilidade e é decisão do
  Adriano.
- A prova do dialeto PostgreSQL(R) contra um servidor real está pendente.
- O editor visual do modelo (pedido 127, segunda metade) não começou.
- **O `.txt` do Profiler não rotaciona.** Medido: **345 B por pedido**, sem
  teto — 1,2 GB por hora num servidor com 1.000 pedidos/s. O anel de memória
  tem teto desde sempre; o arquivo não. Hoje a tela mostra o tamanho.
- **O Profiler não sobrevive a reinício**, e é escolha: ele é sessão de
  observação, não configuração. O arquivo sobrevive, e religar continua nele.
- **Senha escrita dentro do texto de um `SELECT` aparece no Profiler** — o
  campo se chama `texto` e nenhuma redação por nome de campo a alcança. Hoje a
  camada SQL não tem comando que carregue credencial; no dia em que tiver,
  isto vira defeito.

---

## 0.17.0 — 2026-08-29

Os gaps. Esta versão fecha itens que estavam na lista do que falta, e não
recursos novos inventados aqui.

### Adicionado

- **Janela de conflito de escrita** (pedido 123), a ideia que a leitura do
  HFSQL(R) apontou como a mais valiosa da lista. Duas pessoas com a mesma ficha
  aberta terminavam com a segunda gravação apagando o trabalho da primeira —
  sem erro, sem registro, sem ninguém perceber até faltar o dado.

  **Não mudou formato**: a versão por registro existe no cabeçalho do slot do
  `.reg` desde a v1 e ninguém a usava. `ler` devolve a versão com
  `"com_versao": true`; `atualizar`, `excluir` e `restaurar` conferem a versão
  que o cliente mandar; a recusa é o erro novo **3004 `CONFLITO`**. Conferir
  custa 24 bytes de leitura — o cabeçalho do slot, não a linha.

  A janela mostra as três colunas do PDF deles — «valor anterior», «o outro
  escreveu», «você escreve» — e vai um passo além: **já vem marcado quem mexeu
  em cada coluna**. Dois que editaram campos diferentes da mesma linha saem
  dali com os dois trabalhos preservados, sem escolher nada. Marcar tudo como
  «o meu» por omissão desfaria em silêncio o trabalho do outro nas colunas que
  eu nem toquei — o mesmo estrago de antes, com mais cliques.

  Três decisões que valem registro:

  - **Não é trava.** Travar na leitura prenderia a linha toda vez que alguém
    fechasse o navegador com a ficha aberta, e duas sessões que travam em ordem
    trocada se abraçariam.
  - **A conferência é pedida, não imposta.** Quem manda `"versao"` ganha a
    garantia; quem não manda continua com a última gravação vencendo. Imposta,
    todo cliente anterior a esta versão pararia de gravar de um dia para o
    outro. A interface web manda sempre.
  - **Excluída de vez é conflito**, e não «não encontrado»: quem leu a linha há
    um minuto precisa saber que ela foi apagada, e não que o rowid nunca
    existiu.

  17 testes novos — 10 no motor, 7 no protocolo —, e a tela conferida no
  navegador: com a ficha aberta, uma gravação alheia na cidade e a minha no
  telefone, o registro terminou com **as duas**.

- **Cache de páginas no `.ndx`** (pedido 113, e não pelo caminho que o pedido
  supunha). A inserção com dois índices caiu de **44,4 para 18,5 µs por linha —
  2,40×** —, e a carga em lote pela rede subiu de **25.985 para 39.287
  linhas/s** (com o §2.0 junto). Sem mudar formato, sem mudar garantia e sem tocar na B+tree.

  O pedido dizia «ordene as chaves do lote, para chaves vizinhas caírem na mesma
  folha». Medi antes: **a desordem custava 1,06×**. O custo não era de
  localidade — era de **reler do arquivo e recalcular o CRC-32 da mesma página**
  a cada descida da árvore, e a raiz é a mesma página em todas as inserções da
  carga. O medidor agora **conta** os toques em vez de citar um `strace`
  antigo: 8,80 páginas servidas de RAM, 2,06 gravadas, 10,86 no total — não os
  ~20 que estavam escritos. A 2,34 µs de CRC por página, eram **25,4 µs por
  linha só de CRC**, de 44,4 medidos.

  Com isso, a linha que mais mudou é a que confirma o diagnóstico: **conferir a
  chave única caiu de 20,5% para 2,3%** do tempo de uma inserção. É uma descida
  na árvore que não escreve nada — exatamente o trabalho que o cache serve de
  graça. E o `.ndx` caiu de 83,5% para 63,6% do total.

  **O cache é de leitura.** Toda gravação atravessa para o arquivo na hora.
  Segurar página suja daria mais e trocaria uma garantia por desempenho sem
  avisar: hoje só uma queda da máquina atrasa o `.ndx` em relação ao `.reg`, e
  não uma queda do processo. O despejo é por segunda chance, senão a raiz — a
  página mais visitada — sairia junto com as outras assim que o teto enchesse.
  O teto de 2.048 páginas (8 MiB) saiu de uma varredura de quatro tamanhos, em
  `docs/DESEMPENHO.md` §2.1.

  **Ordenar as chaves continua não feito**, agora com número: depois do cache a
  desordem passou a custar **1,19×** (a localidade só importa quando não se está
  pagando CRC de qualquer jeito). Implementar exige gravar o `.reg` antes de
  indexar, e aí uma falha no meio deixa linha sem chave, sem como desfazer.
  Está registrado com o preço para a decisão ser tomada com ele na mão.

- **`bancada/carga/medir.py`**, para a carga pela rede parar de ser um número
  medido à mão. As duas metades fazem o mesmo trabalho e a contagem é conferida
  no fim — a armadilha que esta bancada já caiu duas vezes.

- **`--example ordem-da-chave`**, que mede quanto a ordem das chaves custa. Foi
  ele que reprovou a hipótese do pedido 113 antes de ela virar código.

- **O cabeçalho do `.reg` parou de reserializar o esquema a cada linha.** Toda
  inserção chamava `gravar_cabecalho`, e ele fazia cinco coisas — serializar o
  esquema inteiro, calcular o CRC-32 dele, gravar os 128 bytes de cabeçalho com
  os contadores, gravar o **bloco de esquema outra vez** byte a byte igual, e
  perguntar o tamanho do arquivo. Das cinco, **uma** era necessária.

  O esquema não muda desde que a tabela é criada: passou a ser serializado uma
  vez, no construtor, com o CRC junto; e o caminho quente ganhou um irmão que
  grava só o cabeçalho. O bloco de esquema e o teste de tamanho ficaram onde
  importam, na criação do volume. **Só o `.reg`: 6,8 → 5,3 µs por linha
  (1,27×). Com dois índices: 18,5 → 17,0 µs.** Nenhum byte mudou de lugar no
  disco.

  Achado respondendo a uma pergunta sobre outra coisa — «e se o `.ndx` parasse
  durante a carga?» —, o que é onde essas coisas costumam aparecer.

- **`BULKINSERT`: a tabela reservada para a carga** (pedido 128). Uma carga
  longa quer duas coisas que o servidor não dava: ninguém mais mexendo naquela
  tabela enquanto ela entra, e uma sincronização só, no fim.

  ```
  {"op":"bulkinsert","database":"Z","tabela":"Clientes","ligado":true}
  ... as inserções ...
  {"op":"bulkinsert","database":"Z","tabela":"Clientes","ligado":false}
  ```

  **1,53× medido** — 43.044 e 44.026 sem reserva contra 65.737 e 67.339
  linhas/s com ela, dois pares de corridas. O ganho vem da janela de
  durabilidade: reservada, ela não fecha, e a carga inteira vira um `fsync` só.

  Os outros recebem **erro na hora**, e não espera: o novo **4002
  `EM_CARGA`**, dizendo **quem** reservou e **desde quando** — sem isso,
  «tabela em carga» manda a pessoa procurar sozinha quem está segurando. Ele
  vem com `repetir: true`, e passa a ser o **segundo** erro do protocolo que
  pede nova tentativa (o outro é o de E/S): é o que separa «espere um pouco»
  de «você não pode». A leitura também para, e é de propósito — deixar ler
  durante a carga é o que impediria adiar o índice mais tarde.

  Contra reserva órfã há **duas** redes, e não uma: a **queda da conexão**
  solta na hora, por qualquer caminho de saída; e o **prazo**
  (`recursos.carga_prazo_min`, padrão 30 min) solta o soquete que ficou
  pendurado vivo com o cliente morto do outro lado — que é exatamente o caso
  em que a primeira não pega.

  Só pela porta de dados: HTTP não tem conexão para cair. Pela tela,
  `inserir_lote` já é uma operação só.

  10 testes, mais a prova pelo soquete em `bancada/carga/bulkinsert.py` — e foi
  ela que achou o que os testes unitários não achavam.

- **O `.log` deixou de atrasar o `.reg`.** O diário fazia **duas escritas por
  evento**: os 44 bytes do evento, e os 64 do cabeçalho com `fim` e
  `qtd_eventos`. O evento tem de ir na hora; o cabeçalho é um contador, e a
  leitura sabe recalculá-lo varrendo os próprios eventos. Ele passou a ir no
  `sincronizar`: **1,22 → 0,67 µs por evento (1,82×)**, e a inserção completa
  com dois índices de **17,0 para 15,9 µs**.

  **O evento continua indo para o arquivo dentro da inserção** — o que ficou
  para depois foi só o contador. O que isso pediu foi um caminho de reparo:
  uma queda antes do `sincronizar` deixaria o cabeçalho atrasado, e a próxima
  gravação escreveria **por cima** dos eventos já gravados — evento destruído,
  não invisível. Então `abrir` varre para a frente a partir do `fim` gravado,
  validando cada evento pelo CRC que ele já carrega, e para no primeiro que não
  confere. Quatro testes travam isso; o que mais importa é
  `depois_da_cura_o_novo_evento_nao_sobrescreve`.

  **Segurar os eventos em RAM continua fora**, e a razão não é de tamanho (4,2%)
  e sim de natureza: índice perdido se reconstrói do `.reg`; evento perdido não
  se reconstrói — ele é a história e é a posição de que a replicação depende.

- **O Profiler desligado custava 7% da carga pela rede.** O ponto de captura
  fazia o trabalho **antes** de conferir se havia o que capturar: dois
  `Json::analisar` do corpo inteiro, três `String` e um mutex, para no fim
  `chegou` olhar `ligado` e devolver `None`. Num `inserir_lote` de 5.000 linhas
  isso é analisar meio megabyte de JSON duas vezes, para nada.

  O portão passou a ser um `AtomicBool` lido antes de qualquer trabalho:
  **40.600 → 43.450 linhas/s (1,07×)** na carga em lote, dois pares de corridas.

  Qual das duas coisas custava, medido em `--example quem-custava`: um
  `lock`/`unlock` sem disputa custa **13,2 ns**, e analisar o corpo de um lote
  de 5.000 linhas custa **3.456 µs**. Por lote eram 6.912 µs de parse contra
  0,03 de lock — **262.000×**. Não era o mutex; era analisar meio megabyte de
  JSON duas vezes para jogar fora. É também por isso que o caminho linha a
  linha quase não se moveu: lá o corpo tem 140 bytes.

  Cinco testes travam o que pode dar errado: o espelho atômico divergir do
  estado real. Preso em `true`, o servidor pagaria o parse para sempre; preso em
  `false`, o Profiler não veria nada estando ligado. Inclusive o caso do
  `profiler_ligar` que **falha** — ele não pode levantar o espelho.

- **`--example custo-do-log`**, que decompôs o bloco `.reg` + `.log` que este
  documento registrava como não decomposto — e foi ele que apontou onde estava
  a escrita de sobra.

- **`--example indice-adiado`**, que responde «e se o `.ndx` parasse durante a
  carga e fosse reconstruído no fim?» com a reconstrução **dentro da conta**:
  **1,02×**. O `reindexar` de hoje insere chave a chave — uma descida por
  chave, o mesmo trabalho do caminho de dentro, feito depois. O ganho está na
  **construção em lote** da B+tree (varrer, ordenar, encher as folhas em
  sequência), cujo piso medido é 0,24 s contra os 2,54 s que o `reindexar`
  cobra. A ordem de trabalho é a inversa da intuição: o lote primeiro, o
  adiamento depois. Está em `docs/DESEMPENHO.md` §4.2.

- **Direito no nível da tabela** (pedido 124), o primeiro item da lista que a
  leitura do HFSQL(R) apontou como faltando. Até aqui a permissão parava na
  base: quem lia a base lia **todas** as tabelas dela — e a folha de pagamento
  e a tabela de clientes moram no mesmo banco porque o negócio é um só.

  Dentro do objeto da base, `"tabelas"` escreve a regra de cada tabela, e ela
  **substitui** a da base ali — a mesma coisa que a base já fazia com o `"*"`.
  Substituir, e não interceder, é o que permite as duas coisas que a prática
  pede: **tirar** `folha` de quem lê o banco inteiro, e **dar** `clientes` a
  quem não lê o banco nenhum. Uma regra de interseção resolveria só a primeira.

  O portão continua sendo **um só** — espalhado por quarenta operações, a que
  alguém esquecesse de conferir viraria a porta dos fundos, e ninguém acharia
  isso por leitura. Duas operações precisaram de conferência própria porque não
  têm o campo `"tabela"` que o portão lê: **`juntar`**, cujas tabelas moram em
  `a.tabela` e `b.tabela`, e **`unir`**, cuja lista de tabelas está em
  `"tabelas"`. Sem isso bastaria pedir a tabela negada como o lado B de uma
  junção — há um teste com esse nome.

  A árvore e o catálogo (`tabelas`, `sistabelas`, `siscolunas`) passaram a
  listar **só o que dá para abrir**: o nome de uma tabela já conta parte da
  história, e descobrir a recusa só ao clicar é pior do que não ver.

  9 testes, e o que mais importa deles é `sem_regra_de_tabela_nada_muda`: um
  `config.json` escrito antes desta versão continua se comportando igual.

- **A réplica passou a acompanhar o master** (pedido 111): **4.273 → 17.450
  eventos/s por réplica (4,08×)**, e as três juntas aplicam ~52.000/s contra os
  34.048 que o master escreve. O alcance de 100.000 eventos caiu de 18,7 s para
  **5,7 s**, e a latência de uma exclusão física até as três, de 1.952 para
  **140 ms**.

  **A causa registrada estava errada, e a medição a derrubou.** Estava escrito
  em dois documentos que «aplicar decodifica a imagem para `Value` e reencoda o
  payload, em vez de gravar os bytes que vieram». Medido
  (`--example onde-doi-na-replica`): `aplicar_evento` custa **16,15 µs** e uma
  inserção local pura custa **15,88 µs** — a acusação vale **0,27 µs**. E os
  4.273/s eram **229 µs por evento**, enquanto o caminho de CPU inteiro dos dois
  lados custa 20,5.

  Os 208 µs que faltavam estavam **no source**, e não na réplica:

  - **O diário era varrido desde o começo a cada lote.** Desde que o evento
    deixou de ter largura fixa, chegar ao evento N é caminhar pelos N−1
    anteriores lendo o cabeçalho de cada um. Servir «500 a partir de P» custava
    1,11 µs por evento com P=0 e **72,65 µs** com P=90.000; alcançar 100.000 em
    lotes de 500 gastava **4,07 s só ali** (`--example custo-do-desde`). Com uma
    **marca de posição**, **0,09 s — 45×**.

    A marca é uma **dica**, e não uma verdade: uma errada faz a leitura começar
    no lugar errado e o CRC do evento recusar, ou cair depois do fim e devolver
    vazio. Nenhum dos dois entrega evento errado, e é isso que a torna segura.
    Ela mora no servidor, e não na tabela, porque a tabela é aberta e fechada a
    cada pedido — e são pedidos seguidos que ela serve. **São várias por
    tabela**: um source atende réplicas em posições diferentes, e uma marca só
    seria empurrada para frente pela mais adiantada e nunca serviria às outras.

  - **O laço dormia depois de toda rodada, inclusive das produtivas.** O
    `reconectar_em` é o intervalo entre perguntas **em vão**; uma rodada que
    aplicou eventos volta na hora, porque o source continuou escrevendo enquanto
    ela aplicava. Erro continua dormindo, de propósito.

  E um terceiro, menor: **`bytes_para_hex` fazia um `format!` — e uma alocação
  de `String` — por byte** da imagem. Tabela de dígitos no lugar: 3,48 → 0,24 µs
  por evento, **14,5×**.

  3 testes novos, e o que mais importa é `a_marca_da_exatamente_os_mesmos_eventos`:
  a marca é otimização num caminho onde errar não dá erro, dá **evento errado
  aplicado como se fosse o certo**.

- **Construção em lote da B+tree** (pedido 114): `NdxFile::construir_em_lote`
  monta a árvore sem descer nenhuma vez — ordena as chaves, enche as folhas em
  sequência e monta os níveis de cima por cima. Um milhão de chaves: **7,72 s →
  0,31 s, 23× a 25×**. Todo `reindexar` e todo *reparar índice* andam nisso.

  O **enchimento das folhas — 80% — é medido, e não herdado**. 70% é a folga
  clássica e não compra nada, porque inserção aleatória já assenta perto de 69%
  de ocupação sozinha; de 90% para cima a folha fica sem folga, e crescer aloca
  milhares de páginas e fica **mais lento** do que na árvore mais frouxa.

  A construção **exige índice vazio** e recusa em vez de aproveitar árvore
  existente: aproveitar pediria devolver as páginas velhas à lista de livres uma
  a uma, e vazar página em silêncio é pior que recusar.

  **O adiamento que ela deveria destravar foi medido e ficou de fora.** O 1,59×
  vale para tabela vazia; `reindexar` refaz sobre a tabela **inteira**, então
  carregar M numa tabela de N ganha 1,22× quando M=N e **vira prejuízo abaixo de
  M≈N/3**. E cobraria marcar índice suspenso no formato, cujo defeito é busca
  respondendo errado em silêncio depois de uma queda. O que o faria valer é
  **fundir** a série ordenada na árvore existente, e não refazê-la.

- **`docs/SQL.md`: o que a camada SQL precisa saber, antes de existir.** O
  motor tem hoje um protocolo de operações, e não uma linguagem. O documento
  mapeia cada construção de SQL na operação que já existe — e é curto de
  propósito: a maior parte de um `SELECT` já tem substrato, e o que **não**
  tem está listado com nome (expressão, planejador, `GROUP BY` geral,
  subconsulta, transação).

  Ele nasceu de uma pergunta específica: como o `BULKINSERT` entra numa
  linguagem. A resposta é que ele **não** é açúcar sintático, por três motivos
  que o analisador não pode ignorar — é palavra reservada; vale para a
  **sessão**, e não para o comando, então um driver que multiplexa conexões
  quebra a exclusividade sem avisar; e o `EM_CARGA` tem de virar
  *serialization failure* no SQLSTATE, e não *access denied*, senão o driver
  do outro lado desiste em vez de repetir.

  E a frase que o documento repete alto: **`BULKINSERT` não é transação.** Ele
  reserva a tabela; não desfaz nada. Quem ler «exclusiva até concluir» e
  entender `BEGIN` vai perder dado.

### Mudado

- **A tela de configuração explica cada ajuste, em vez de despejar o JSON.**
  Ela mostrava o `config.json` cru — o que serve para conferir, e não para
  decidir. Agora cada campo de `recursos` vem com uma linha dizendo o que ele
  muda de verdade (`cache_paginas`, `carga_prazo_min`, `nucleos_efetivos`…),
  e há uma seção **«Cargas em andamento»** listando as reservas de
  `BULKINSERT` — quem, qual tabela, desde quando. O JSON continua embaixo.

  Conferida no navegador, e não só lida: foi assim que `nucleos_efetivos`
  apareceu com a explicação em branco. Quem não tem `administrar` vê a tela
  sem a seção de cargas, e não um erro.

- **`recursos.cache_paginas` passou a valer.** O campo estava no `config.json`,
  no MANUAL e na tela desde a 0.13.0, e **nenhuma linha de código o lia** — ele
  dizia «páginas do `.ndx` mantidas em memória» quando não havia cache nenhum.
  Agora é o teto do cache, e o padrão baixou de 4.096 para **2.048 páginas
  (8 MiB)**, que é o joelho da curva medida.

- **O erro do protocolo chega inteiro à tela.** O `api()` da interface jogava
  fora `nome`, `codigo` e `classe` e guardava só o texto — então distinguir um
  conflito de qualquer outra recusa exigiria comparar a **redação** da
  mensagem, e melhorar essa redação quebraria a tela sem ninguém notar.

---

## 0.16.0 — 2026-08-28

**Profiler**, as **cores da ação**, **Docker** e a leitura do HFSQL(R) e do
DBeaver contra o projeto.

### Corrigido

- **Salvar e incluir pela tela estavam quebrados** desde que o `rownum` entrou:
  a ficha tirava só a *primeira* coluna de sistema e mandava 8 valores para uma
  tabela de 9 colunas. Achado **gravando o vídeo de demonstração**.
- **A tela da Replicação** dizia que a replicação não existia, e lia o campo
  errado da resposta de `bancos`.
- **O erro da réplica** saía sempre como «acesso negado», inclusive para um
  database que ainda não existe no master.

### Adicionado

- **Profiler.** O que está chegando pela porta, **antes de virar dado** — o
  ponto de captura é uma linha depois do `read_line` e uma antes do despacho.
  Por isso o pedido que *trava* aparece na lista como «em curso», que é
  justamente o que se quer achar. Filtra por banco, usuário, operação e «só
  escrita»; guarda num anel de tamanho fixo e grava num `.txt` no caminho que o
  administrador escolher. Observa **as duas portas** — deixar a interface web
  de fora faria ele mentir por omissão para quem está olhando por ela — e não
  observa a si mesmo.

  **A senha não passa por aqui**, e é a regra que mais importa neste arquivo:
  um profiler é exatamente onde uma senha vazaria sem ninguém notar. O texto é
  **analisado** e os campos sensíveis viram `"***"` antes de encostar na
  memória ou no arquivo — nunca recortado, porque recortar depende de o pedido
  estar escrito de um jeito. Pedido que não é JSON vira o tamanho em bytes.

- **As cores da ação**: verde inclui, amarelo altera, rosa marca (o excluir que
  volta), vermelho exclui de vez, azul consulta. **Contorno e não fundo cheio**
  — a lição já estava escrita no CSS: fundo laranja com texto escuro em cima
  ficava ilegível. No diálogo de excluir o botão troca de cor junto com o texto.

- **Docker**, com imagem `scratch`: sem shell, sem gerenciador de pacotes, só o
  binário. Exige o alvo **musl** — medido: o padrão linka `libc.so.6`,
  `libgcc_s.so.1` e o carregador dinâmico, e `FROM scratch` não subiria. Com
  musl são 3,4 MB o servidor e 1,2 MB o cliente, `static-pie`, e o binário roda.
  Um `docker-compose.yml` sobe um master e duas réplicas.

- **Teste da chave composta**, livre e única — as duas já existiam no formato e
  nenhuma tinha teste que as separasse.

- **`docs/HFSQL.md`**, **`docs/DBEAVER.md`** e **`docs/CLUSTER.md`**: a leitura
  da documentação do HFSQL(R) item a item contra o código, o que dá para
  reaproveitar do DBeaver, e o que exatamente falta para haver cluster.

### Sabido

- **O `docker build` não foi executado** — não há daemon Docker na máquina em
  que isto foi escrito. O que foi verificado: que o alvo musl produz binário
  estático, e que esse binário sobe e responde.
- **Não há cluster.** Há replicação, e com ela escala de leitura. Falta endereço
  único, eleição de primário e promoção automática.
- **A segunda gravação ainda vence em silêncio.** O HFSQL(R) mostra uma janela
  de conflito com «valor anterior / o outro escreveu / você escreve»; aqui não
  há detecção nenhuma. A peça está no formato — o `.reg` guarda uma versão por
  registro — e o item está em `PENDENCIAS.md`.

---

## 0.15.0 — 2026-08-28

**Replicação funcionando**, **carga em lote** e o **salto para a página 500** —
os três estavam escritos como o que faltava, e os três saíram.

### Corrigido

- **Salvar e incluir pela tela estavam quebrados desde que o `rownum` entrou.**
  A ficha tirava só a **primeira** coluna de sistema (`find(c => c.sistema)`),
  então o `rownum` continuava no formulário — editável, o que já é errado — e o
  cliente mandava 8 valores para uma tabela de 9 colunas. Toda gravação pela
  interface morria com «a lista tem 8 valores». Achado **gravando o vídeo de
  demonstração**: o erro aparece no canto da tela num quadro do capítulo 9.
  Teste novo trava a linha curta dos dois lados — inclusão e alteração — e
  confirma que as duas colunas de sistema são herdadas, não zeradas.

- **A tela da Replicação lia o campo errado da resposta de `bancos`.** A
  operação responde uma **lista** de nomes, não um objeto com `bancos` dentro;
  ler o campo devolvia vazio, e a tela dizia «nenhuma tabela ainda» numa
  réplica que tinha a tabela na árvore ao lado. Achado no mesmo vídeo, no
  quadro seguinte.

- **A tela da Replicação dizia que a replicação não existia.** Ela ainda
  explicava que «as portas são configuração, não serviço» e que faltava o
  `.log` v2 — texto verdadeiro na 0.14.0 e falso agora. Passou a mostrar o
  papel, se a imagem está ligada, de onde a réplica puxa e a **posição de cada
  tabela**, que é o número que diz se ela está em dia.

- **O erro da réplica saía sempre como «acesso negado».** Um database que ainda
  não existe no master aparecia no log como problema de autorização — o pior
  tipo de mensagem, a que manda procurar no lugar errado. O erro do outro lado
  já vem classificado (`nome` e `classe` estão na resposta) e agora é
  reembalado com a classe certa.

- **A bissecção pelo `rownum` estava errada na partição alfanumérica, e errada
  em silêncio.** Ali o `rownum` não cresce com o rowid: a Silva digitada
  primeiro mora no `_S`, com rowid alto, e a Alves digitada depois mora no
  `_A`, com rowid 1 — número de ordem 1 num rowid maior que o do número 2.
  Bissetar uma sequência que não está ordenada devolve a linha errada sem
  reclamar. Nesse modo o motor agora varre, procurando o **menor** número de
  ordem maior ou igual ao alvo. Teste novo em `tests/alfanumerica.rs` prova
  que os rowids saem fora de ordem — e falha se um dia saírem crescentes, para
  não continuar provando outra coisa.

- **`phxsql listar` lia a tabela inteira para mostrar vinte linhas.** Numa
  tabela de 200.000 com memo, 382 ms para uma tela que cabe no terminal.
  Agora o teto entra na leitura, e o comando ganhou `--pular`.

- **Duas sobras da versão anterior**: um comentário duplicado no caminho da
  importação e um doc-comment órfão de função que mudou de arquivo. Zero
  avisos do clippy de novo.

### Adicionado

- **A replicação Master → Réplica está no ar.** Quatro servidores medidos em
  `bancada/replicacao/`, com o Master e três espelhos:

  | | |
  |---|---|
  | Master, com a imagem no diário | 18.773 linhas/s |
  | Aplicação, por réplica (as três em paralelo) | 4.273 eventos/s |
  | Atraso de uma escrita até as três | 1,3 s a 2,1 s |
  | Réplica derrubada: voltar a atender e alcançar 4.000 eventos | 343 ms + 1,0 s |
  | Retrato SHA-256 das quatro tabelas, no fim | idênticos |

  A bancada não compara «quantas linhas»: compara um SHA-256 de **cada linha
  inteira**, com `rowid` e `rownum` juntos. O `rowid` entrar na conta é o
  ponto — ele não é transmitido: o `.reg` nunca reaproveita slot, então uma
  réplica que aplicou tudo na ordem chega ao mesmo número sozinha. Se não
  chegar, divergiu, e a replicação **para ali** em vez de espalhar.

- **`.log` v2 com a imagem da linha.** Era a única peça que faltava, e ela é
  o payload **cru** do `.reg` mais o **conteúdo** dos anexos — não os
  ponteiros, que são offsets desta máquina e apontariam para qualquer coisa na
  outra. Atrás de `replicacao.imagem_da_linha`, ligada sozinha num `source`.
  Medido, mesma tabela e mesmas 100.000 linhas: **10% mais devagar e um diário
  5,1× maior** (44 → 223 bytes por evento).

- **`posicao`, `replicar` e `aplicar` no protocolo**, e o laço da réplica
  dentro do próprio `phxsqld` — uma thread por origem, `papel: replica` e uma
  origem no `config.json` bastam. A tabela que ainda não existe na réplica
  nasce do **bloco de esquema cru** do source, e não de uma remontagem coluna a
  coluna a partir de JSON.

- **A senha da réplica não fica em claro nem viaja.** Ela se autentica pelo
  mesmo desafio-resposta do resto do protocolo, com a chave derivada do
  `senha_hash` que mora no `config.json` dela.

- **Cascata**: uma réplica pode ser origem de outra. Master → Slave01 → Slave03
  mediu 1.827 ms contra 1.679 ms do primeiro salto.

- **`inserir_lote`: várias linhas num pedido só.** Medido com 20.000 linhas
  pela rede, contra o mesmo trabalho linha a linha: **2.715 → 25.985 linhas/s
  (9,6×)**. O ganho não é do disco — cada linha custa o mesmo lá dentro — e sim
  de tudo que acontecia POR LINHA e passa a acontecer uma vez: abrir os sete
  arquivos, tomar a trava, o `fsync`.

- **Colar em vez de montar.** O mesmo pedido aceita texto em **JSON, CSV, TXT,
  XML e HTML**, e adivinha o formato pelo conteúdo. A primeira linha manda: as
  colunas casam pelo **nome**, não pela posição. `importar_conferir` lê e
  mostra o que entendeu sem gravar nada — é o que a tela de Importar usa, e o
  botão de gravar só acende depois que a conferência passa. Na linha de
  comando, `phxsql importar`.

- **`pular` deixou de andar até a posição.** Quando a posição de uma linha na
  lista *é* o `rownum` dela, o início da página sai de uma bissecção. Medido
  numa tabela de 200.000 linhas, pelo protocolo, pedindo 200 linhas:

  | `pular` | bissecção | passo |
  |---:|---:|---:|
  | 200 | 7 ms | 6 ms |
  | 20.000 | 7 ms | 18 ms |
  | 100.000 | 6 ms | 72 ms |
  | 199.800 | 6 ms | **131 ms** |

  A bissecção é **plana** — e os 6 ms dela são decodificar e serializar as 200
  linhas, não achar o começo. Dentro do motor, sem a rede e sem a serialização:
  **180 µs contra 55 ms** no meio de uma tabela de 200.000, e **164 µs contra
  246 ms** numa de 800.000. Os dois caminhos devolvem a mesma página — o
  exemplo `custo-da-pagina` afirma isso e falha se deixar de ser verdade.

- **`salto` na resposta do `varrer`**: `"bisseccao"` ou `"passo"`. A diferença
  entre os dois é de ordem de grandeza, e quem monta uma tela grande precisa
  saber qual está pagando — e o que fazer com a tabela para pagar o outro.

- **`visiveis` voltou a existir na resposta, e agora é barato.** Sai de dois
  contadores do cabeçalho: `registros − marcadas` são as ativas, `marcadas` são
  as excluídas. Era por essa conta não existir que o `total` tinha saído na
  0.14.0. Com ela, «página 3 de 40» voltou para a grade sem custar varredura.

- **Caixa «ir para a página» na grade**, com o botão `fim ⏭` ao lado. Salto
  para a página 500 de uma tabela de 200.000: **116 ms** medidos no navegador,
  incluindo o desenho da tela. O número da página sobrevive a navegar por
  cursor: `anterior` desconta um, `próxima` soma um.

- **`desde_rownum` no `varrer`**: a página que começa no número de ordem N,
  inclusive. É o cursor de quem guardou o número de ordem em vez do rowid.
  `rownum_inicio` e `rownum_fim` vêm na resposta.

- **`--pular` no `phxsql listar`**, e o rodapé diz por onde a página foi
  achada e qual o `--pular` da próxima.

### Mudado

- **`.log` v1 → v2**: o cabeçalho do evento passou de 36 para 44 bytes, e o
  evento deixou de ter largura fixa. Isso cobra um preço: até a v1 o evento N
  morava no offset `64 + N × 36` e pular era uma conta; agora chegar ao evento
  N é caminhar pelos anteriores. O que salva a leitura é o `qtd_eventos` do
  cabeçalho de cada volume — um volume inteiro se pula sem abrir.

- **O CRC do evento passou a cobrir a imagem**, e não só o cabeçalho. A imagem
  é o que a réplica grava **como dado**: um byte trocado ali entraria na
  réplica sem ninguém notar.

- **`.reg` v3 → v4**: o contador `marcadas` nos bytes 108..116 do volume 1.
  Arquivo da v3 não abre — e não abrir é o ponto: ele traria zero ali, zero
  quer dizer «nenhuma linha marcada», e o motor concluiria que a posição é o
  `rownum` numa tabela onde não é. A página sairia errada em silêncio.

- **O contador de marcadas vai ao disco na mesma operação que o muda**, e não
  no `sincronizar` — 128 bytes a mais por exclusão suave. Um contador que só é
  gravado depois volta atrás numa queda, e este não é número de vitrine: é ele
  que decide se o salto pode confiar no `rownum`.

- **`verificar` reconta as marcadas varrendo** em vez de acreditar no
  cabeçalho, e corrige de passagem. `Relatorio` ganhou o campo. É o mesmo
  caminho que o reparo chama.

### Sabido

- **A réplica aplica mais devagar do que o master escreve** — 4.273 eventos/s
  contra 18.773 linhas/s, com as três competindo pela mesma máquina. Sob carga
  sustentada elas ficam para trás. A razão está no caminho: aplicar decodifica
  a imagem para `Value` e **reencoda** o payload, em vez de gravar os bytes que
  vieram. Gravar o payload direto, remendando só os ponteiros dos anexos, é o
  próximo ganho grande.

- **O atraso da réplica é o intervalo do laço, não o trabalho.** Com
  `reconectar_em: 2` uma escrita leva de 1,3 s a 2,1 s para chegar. Baixar o
  intervalo baixa o atraso e sobe o tráfego de perguntas em vão; o `long-poll`
  — o source segurar a resposta até ter novidade — ainda não existe.

- **O JSON da replicação vai em claro**, e a imagem vai em hexadecimal, que
  dobra o tamanho. Não há TLS no transporte: por enquanto ele depende do túnel.

- **Não há transação, e o lote não muda isso.** Se a linha 700 de mil falhar,
  as 699 anteriores ficam gravadas: o `.reg` não reaproveita slot, então
  desfazer deixaria 699 buracos. Por isso o padrão é parar na primeira
  recusada; quem importa dado sujo de propósito passa `parar_no_erro: false` e
  recebe a lista do que ficou de fora, com o número da linha.

- **`1.500` continua ambíguo.** Mil e quinhentos ou um e meio? O motor
  converte `1.500,50` e `1,500.50` — o último separador é o decimal — e deixa
  `1.500` como está, em vez de escolher por conta própria.

- **Com buraco, o salto volta a andar.** Uma única linha excluída — de vez ou
  marcada — derruba a igualdade entre posição e `rownum` na tabela inteira, e
  o `pular` volta aos 131 ms. É correto: a posição realmente mudou. Mas é uma
  degradação em degrau, e não gradual: quem paginava a 6 ms passa a 131 com
  uma exclusão. Um índice de posição resolveria, ao preço de mantê-lo.

- **Por índice o salto continua sendo posição pura.** A ordem da chave não tem
  relação com a ordem de chegada, então não há `rownum` a bissetar ali.

---

## 0.14.0 — 2026-08-28

Paginação por **cursor**, a coluna de sistema **`rownum`**, e a partição
**alfanumérica** — `Clientes_A.reg` até `Clientes_Outros.reg` — com o descritor
`.pag` ao lado.

### Corrigido

- **O servidor nunca ligava `TCP_NODELAY` nas conexões que aceita** — só o
  cliente DbLink ligava. O Nagle segurava cada resposta por até 40 ms
  esperando mais bytes para encher um pacote, e nunca vinham: a resposta tinha
  acabado. Medido na porta de dados com 20.000 linhas: **1 ms de servidor e
  44 ms de relógio**. Depois: **1,3 ms**.

  Trinta e três vezes, numa opção de soquete de uma linha, e valia para **toda**
  operação do protocolo e para todo clique da tela. Achado medindo o relógio
  contra o `ms` que a própria resposta declara — ler o código não acharia, não
  há nada errado escrito.

- **O `varrer` lia a tabela inteira para devolver uma página.** `varrer_com`
  decodifica cada linha **com os anexos** do `.bin` e do `.memo`, monta tudo em
  memória, e só então o servidor jogava fora tudo menos as primeiras `max`.

  Medido com o exemplo `custo-da-pagina`, a mesma página de 200 linhas:

  | linhas na tabela | antes | pelo cursor |
  |---:|---:|---:|
  | 100.000 | 181 ms | não mensurável |
  | 400.000 | 749 ms | não mensurável |
  | 800.000 | **3.176 ms** | não mensurável |

  O custo crescia com a **tabela**, e não com a página — pior que o
  `LIMIT`/`OFFSET` de qualquer motor, porque o `OFFSET` ao menos não carrega o
  blob.

- **A grade da tela listava os baldes como tabelas separadas.** O catálogo só
  sabia tirar sufixo **numérico**, então `clientes_A.reg`, `clientes_B.reg` e
  companhia apareciam na árvore como se fossem 37 tabelas. Agora o sufixo de
  letra conta como volume — mas **só quando o `_A` está ao lado**, porque uma
  tabela que por acaso se chame `dados_X` continua sendo ela mesma.

- **Os arquivos externos saíam com sufixo de letra.** O `.log`, o `.bin`, o
  `.memo`, o `.trash` e o `.reason` não se partem por letra: rolam por tamanho.
  Um `clientes_B.log` se leria como «o diário do balde B», e o diário é da
  tabela inteira. Achado olhando o `ls` do diretório depois de criar a tabela
  pela tela.

### Adicionado

- **Paginação por cursor no protocolo e na grade.** `depois` e `antes` levam o
  rowid onde a página parou; a resposta devolve `cursor_inicio`, `cursor_fim`,
  `ha_mais` e `ha_antes`. `pular` continua como modo de compatibilidade, e a
  resposta declara qual dos dois foi usado em `modo`.

  `ha_mais` sai de **uma** leitura além do teto, e não de contar a tabela:
  contar para mostrar «página 3 de 40» é o item mais caro da tela numa tabela
  grande, e é o que ninguém lê.

  Dentro do navegador, 20 páginas encadeadas numa tabela de 20.000 linhas:
  **4,0 ms de média, 4,9 ms a pior**, sem crescer com a profundidade. Por
  posição no mesmo ponto: **16,1 ms**.

- **Coluna de sistema `rownum`** — o número de ordem de chegada da linha, em
  toda tabela. O motor preenche; não se escreve à mão e não se ajusta. **Nunca
  reaproveita número**: se reaproveitasse, uma linha nova apareceria *atrás* de
  um cursor parado e a paginação passaria a pular registro sem avisar. Alterar
  não renumera.

- **`rowid_do_rownum`: a bissecção.** O `rownum` cresce com o `rowid`, porque o
  `.reg` guarda as linhas na ordem de chegada — então achar a linha de número
  500.000 num milhão custa **vinte leituras**, sem índice nenhum a manter.

- **Partição alfanumérica.** 37 volumes fixos — `A`..`Z`, `0`..`9`, `Outros` —
  e a linha vai para o arquivo da letra dela. O rowid é atribuído como
  `(balde − 1) × registros_por_arquivo + slot`, que é a **inversa exata** da
  conta que `localizar` já fazia: nenhum caminho de leitura mudou, o `.ndx` não
  mudou, o espelho não mudou.

  Acento cai na letra sem acento; vazio e o que não for letra nem algarismo vão
  para `Outros`; o balde que nunca recebeu linha não ganha arquivo.

- **`.pag`, o descritor de partição**, em JSON indentado ao lado da tabela.
  Diz o modo, a coluna de referência, a conta do endereço por extenso, e o que
  cada balde tem. **Gerado, nunca lido pelo motor** — a verdade continua no
  bloco de esquema e nos cabeçalhos dos volumes. Apagar não quebra a tabela.

### Mudado

- **Esquema `PSCH` v4 → v5** (a coluna `rownum`) e **`.reg` v2 → v3** (o
  contador do `rownum` nos bytes 92..100, e os slots do balde em 100..108).

- **`total` saiu da resposta do `varrer`.** Produzi-lo exigia exatamente a
  varredura que esta versão removeu. No lugar entrou `registros`, que sai do
  cabeçalho e não custa nada. Cliente que lia `total` precisa trocar.

- **Junção e união não devolvem `rownum`**, pela mesma razão de não devolverem
  `softdeleted`: dois números de ordem, de tabelas diferentes, não paginam
  coisa nenhuma.

### Sabido

- **Alterar a coluna de referência de uma tabela alfanumérica é recusado.**
  Mudaria o arquivo em que a linha mora, e com ele o rowid — que é a identidade
  dela em todo índice. O caminho é excluir e inserir de novo, e a mensagem diz
  isso.

- **O teto passa a ser por letra.** Num cadastro brasileiro o `_S` enche muito
  antes do `_K`, e quem enche primeiro derruba a inserção daquela letra com as
  outras 36 ainda com espaço. É a conta a fazer ao dimensionar.

- **O cursor é o rowid, e por índice ele não vale.** O índice devolve rowid na
  ordem da *chave*, e «continuar depois do rowid X» não quer dizer nada ali —
  o próximo da chave pode ter rowid menor. Por índice a paginação é por
  posição, e a resposta declara isso.

- **Não há salto para «a página 500».** O cursor sabe ir e voltar uma página;
  ir direto para a milésima exigiria contar, que é justamente o que foi
  removido. Quem precisa de um ponto específico usa `rownum` com a bissecção.
  *(Resolvido na 0.15.0: o `pular` passou a bissetar, e a contagem voltou a
  partir do cabeçalho.)*

- **Uma tabela chamada `dados_X` e o balde X de uma tabela `dados` se escrevem
  igual.** A presença do `_A` separa os dois casos, mas criar as duas no mesmo
  diretório continua sendo uma colisão de nome que o motor não recusa.

---

## 0.13.0 — 2026-08-28

**Excluir deixou de ser uma coisa só.** Toda tabela ganhou a coluna de sistema
`softdeleted`, e dois arquivos novos entraram: o `.trash`, com a linha inteira
antes de ela sumir, e o `.reason`, com o porquê de cada exclusão. Os dois são
de quem administra.

### Corrigido

- **A grade de dados estava com os valores desalinhados do cabeçalho.** Cada
  célula era montada como `<td>${celulaValor(...)}</td>`, e `celulaValor` já
  devolve o `<td>` inteiro — o navegador fecha o primeiro e abre outro, então
  **cada valor ganhava uma célula vazia na frente**. A linha saía com o dobro
  de células do cabeçalho, e todo dado aparecia uma coluna à direita do nome
  dele. Achado abrindo a página no Chromium e contando as células do DOM, não
  lendo o código: o defeito estava em duas telas, a principal inclusive.

- **Um `atualizar` de rotina ressuscitava linha marcada como excluída.** O
  servidor monta a linha inteira a partir do JSON, e a coluna de sistema
  ausente virava `false` — sem erro, sem aviso, e a linha reaparecia na lista.
  Agora, quando o pedido não fala da coluna, ela **mantém o que a linha já
  tinha**. Achado escrevendo o teste, antes de existir na tela.

- **A lixeira dizia «0 anexos» para linha que tinha anexo.** A listagem não
  carrega os anexos de propósito — um memo de megabytes vezes trezentas linhas
  vira uma resposta que ninguém usa —, e o contador saía do vetor vazio em vez
  do cabeçalho do registro. Quem investigasse concluiria que a foto nunca
  existiu, que é o oposto do que o `.trash` serve para provar. Agora o contador
  vem do cabeçalho, o campo externo aparece como «anexo · não carregado» em vez
  de `NULL`, e há um botão que traz aquela linha inteira.

### Adicionado

- **Exclusão suave, e ela é o padrão.** `excluir` marca a linha: ela some das
  listas e continua inteira no `.reg`, com os anexos, e `restaurar` desfaz. A
  física acontece com `"fisico": true`.

  O padrão é o reversível porque **o irreversível não pode ser escolhido por
  omissão**: um cliente que manda `excluir` sem dizer mais nada está pedindo
  «tira isto da minha lista», e é isso que ele recebe.

- **O `.trash`: a linha inteira, antes de sumir.** Gravada e **sincronizada
  antes** de o slot do `.reg` ser liberado. Guardar depois de liberar teria uma
  janela em que a linha não existe em lugar nenhum, e uma queda dentro dela não
  tem conserto; guardar antes tem a janela oposta, que se resolve olhando.
  Entre perder e duplicar, o motor duplica. Há teste que fecha a tabela **sem
  sincronizar** e reabre, para provar que a garantia não depende de um
  `sincronizar` posterior.

  Guarda o *payload* byte a byte **mais o conteúdo dos anexos** — e não os
  ponteiros. Os blocos do `.bin` são liberados na exclusão e podem ser
  reaproveitados pela próxima inserção: com ponteiros, a foto voltaria sendo a
  de outra linha. Há teste que exclui, insere vinte linhas por cima e confere
  que a foto que volta ainda é a certa.

- **O `.reason`: quem, quando e por quê.** O `.log` diz que houve uma exclusão
  no rowid tal; o que ele não tem onde dizer — o evento dele tem 36 bytes
  fixos — é o motivo. Guarda a frase, a identidade da linha (a chave primária,
  em texto, porque «rowid 4173» não diz nada seis meses depois), o usuário e um
  UUID v7 do próprio evento. **Sobrevive à linha**: o expurgo da lixeira é
  registrado aqui antes de o dado sair.

- **Motivo obrigatório por tabela**, escolhido na criação. Marcado, o motor
  recusa qualquer exclusão sem frase escrita, antes de qualquer gravação.

- **Os três arquivos do administrador.** `lixeira` e `motivos` exigem
  `administrar`; o `.log` mantém a permissão `diario`, que já existe e que só
  um administrador concede. A razão está no conteúdo: quem só tem `ler` perdeu
  o direito àquela linha no instante em que ela foi excluída, e a lixeira
  devolveria o direito por outra porta.

- **Na tela:** o botão Excluir abre um diálogo com os dois modos e o campo do
  motivo — e não um `confirm()`, que só sabe perguntar sim ou não. A grade
  ganhou o par «ativas / excluídas», com botão de restaurar em cada linha
  marcada. Lixeira e Motivos têm tela própria, no menu Tabelas e no botão novo
  da barra. A coluna de sistema **não** vira campo de formulário: oferecer um
  `select` com «verdadeiro / falso» convidaria a excluir digitando, sem motivo
  registrado.

### Mudado

- **Esquema `PSCH` v3 → v4.** A v4 acrescenta a coluna de sistema e o byte do
  motivo obrigatório. Tabela gravada na v3 **continua abrindo e lendo
  exatamente como está** — ela só não tem exclusão suave, e a mensagem de erro
  diz isso em vez de ler lixo.

  A coluna entra em `Schema::new`, que é o caminho de criar; a leitura do disco
  usa outro caminho, que não acrescenta nada. Se acrescentasse, cada linha de
  uma tabela v3 passaria a ser lida com os *offsets* deslocados — e
  **silenciosamente**, porque o CRC do slot continuaria batendo: os bytes
  seriam os mesmos, só a interpretação mudaria. Há teste que trava isso.

- **A coluna entra no fim da lista**, para que os *offsets* das colunas do
  usuário não mudem de lugar. `inserir` com N−1 valores preenche `false`;
  `atualizar` com N−1 mantém o que a linha tinha.

- **`varrer` ganhou `visao`**: `ativas` (padrão), `excluidas`, `todas`. Sem o
  filtro por padrão, marcar não faria nada.

- **Junção e união não devolvem a coluna de sistema.** Uma junção traria duas —
  `c.softdeleted` e `p.softdeleted` —, e as duas seriam falso em toda linha,
  porque a junção só lê linha ativa.

### Sabido

- **A lixeira não devolve a linha para o `.reg`.** Ela guarda, mostra e deixa
  baixar; restaurar de lá exige reinserir, e a linha volta com **outro rowid** —
  o `.reg` não reaproveita slot, nem por restauração. Quem quer volta pelo mesmo
  rowid usa a exclusão suave, que é para isso.

- **O `.trash` e o `.reason` não são cifrados nem compactados.** Compactar
  arquivo append-only exige rotacionar e reescrever, e cifrar exige uma cifra
  de bloco que o projeto ainda não tem: há SHA-256, HMAC e PBKDF2 escritos aqui,
  mas nenhum AES. Enquanto isso, quem tem acesso ao disco lê os dois — a
  proteção é a permissão do sistema de arquivos, e não o formato.

- **A listagem da lixeira carrega o resultado inteiro na memória**, como a
  exportação. Serve para investigar; não serve para varrer uma lixeira de
  milhões de linhas.

- **Filtrar por visão num caminho de índice custa uma leitura por linha.** O
  índice devolve rowid e a marca está no registro. É o preço de pedir
  ordenado; a varredura direta não paga nada.

---

## 0.12.0 — 2026-08-28

A tabela sai em **sete formatos**, com o XLSX e o DOCX escritos aqui, e o
espelho `.bkp` entra no fluxograma de onde estava faltando.

### Corrigido

- **O cabeçalho da planilha saía com a cor da zebra e sem negrito.** Ele
  apontava para o estilo de índice 1, que é o «texto listrado». O Excel(R) não
  reclama de índice errado — ele obedece. Os índices do `cellXfs` agora têm
  nome (`estilo::CABECALHO`, `estilo::DATA_ZEBRA`, …) e há teste que confere a
  correspondência, porque número solto ali já custou caro uma vez.

- **A tabela do DOCX estava sem o `w:tblGrid`, que é obrigatório.** O Word(R)
  tolera a falta, então o defeito passaria despercebido até alguém abrir o
  arquivo noutro programa; o python-docx recusou o documento inteiro.

- **O `.bkp` não aparecia na seção 7 do dossiê** — justamente a que desenha o
  fluxo de gravação. Quem lia via cinco arquivos sendo escritos e concluía que
  o espelho era cópia feita depois. Não é: ele é escrito **no mesmo instante**
  que o principal, no mesmo offset. A figura ganhou a caixa do espelho e a da
  janela de durabilidade, que também faltava. Achado pelo Adriano lendo o
  dossiê.

### Adicionado

- **Exportar em CSV, TXT, JSON, XML, HTML, XLSX e DOCX.** Botão na barra e
  item no menu. Os dois formatos do Office são ZIP de XML, e o projeto já
  escreve ZIP com DEFLATE desde o backup: o que parecia exigir biblioteca são
  os mesmos tijolos que já estavam aqui. **Nenhuma crate entrou.**

- **A planilha sai formatada**, não crua: cabeçalho pintado, zebra nas linhas,
  painel congelado abaixo do cabeçalho, autofiltro em todas as colunas e
  largura medida das 500 primeiras linhas. O documento sai em paisagem, com o
  cabeçalho repetindo a cada página.

- **Data em planilha sai como número com formato**, e não como texto. Texto
  que parece data não ordena, não filtra por período e não entra em conta. A
  diferença entre a época do Excel(R) e a nossa é de 25.569 dias, e é só isso.

- **O HTML exportado leva filtro embutido** e não busca nada na rede: abre em
  máquina sem internet e continua funcionando.

- **`docs/MULTILINK.md`** — por que o pacote MULTILINK não dá para ligar por
  `.rlib` e qual é o caminho que funciona.

### Mudado

- **`FORMATO.md`, `MANUAL.txt` e `README.md`** passaram a dizer que a tabela é
  de cinco arquivos **mais um sexto opcional**, com a descrição de quando o
  `.bkp` é escrito, quando é lido e o que `reparar` faz nos dois sentidos.

### Sabido

- **O MULTILINK não entra por `.rlib`.** O pacote traz só binários — os fontes
  que o manifesto promete não estão nele —, e o `.rlib` foi compilado pelo
  rustc 1.98 contra o 1.94 daqui: **provado rodando o linkador** (E0514), não
  suposto. O formato do `.rlib` não é estável entre versões do compilador,
  então igualar resolveria hoje e quebraria na próxima atualização de qualquer
  um dos lados. Fora isso, um `.rlib` é dependência externa — a regra que
  sustenta o projeto —, não há fachada C que contorne, e o licenciamento é por
  máquina com prazo: linkar faria o servidor de dados inteiro passar a exigir
  licença válida para subir. O caminho é **falar por protocolo**, como o DbLink
  já faz.

- **A exportação carrega o resultado inteiro na memória** antes de escrever.
  Serve para o que uma pessoa abre no Excel(R); não serve para despejar uma
  tabela de dez milhões de linhas.

- **O DOCX não pagina coluna demais.** Em paisagem cabem umas doze colunas
  legíveis; acima disso a tabela aperta. Para tabela larga, XLSX.

---

## 0.11.0 — 2026-08-28

Os monitores da máquina no painel, o aviso de disco por e-mail, e o
**DbLink** — o banco de fora aparecendo na mesma grade que os daqui.

### Corrigido

- **O percentual de disco dividia pelo tamanho errado.** A conta era
  `usado / total`, e o certo é `usado / (usado + livre)`, como a do `df`.
  Reserva de sistema de arquivos e cota não estão à disposição de ninguém, e
  contá-las como livres faz um disco cheio parecer vazio. Na máquina onde isto
  foi medido o `df` dizia **55% usado** e a conta antiga dava **8%** — com 8%,
  um alerta de «menos de 10% livre» nunca dispararia e o disco encheria calado.
  Achado rodando o servidor, não lendo o código.

- **O e-mail do alerta não atravessava relé de sete bits.** O assunto levava o
  «ç» de «espaço» cru no cabeçalho, e cabeçalho de e-mail é ASCII por
  definição (RFC 5322); o corpo ia em UTF-8 cru declarado como 7 bits, e um
  relé sem `8BITMIME` tem licença para cortar o oitavo bit. Agora o assunto sai
  em palavra codificada da RFC 2047 e o corpo em base64. Conferido decodificando
  o que um relé de verdade recebeu, com um leitor independente.

- **`.botao.perigo` pintava vermelho sobre laranja.** A regra trocava a borda e
  a cor do texto mas não apagava o fundo do `.botao`, e o botão de excluir
  ficava ilegível — na tela de usuários, que já era assim, e na nova de DbLink.

### Adicionado

- **Monitores da máquina no painel:** CPU, memória, placas de rede, discos
  físicos e espaço livre de cada caminho que o servidor usa. Tudo do `/proc`,
  que o núcleo publica em texto; o espaço livre do `df`, porque exige
  `statvfs`, que não está na `std`. Nenhuma crate entrou. Os monitores renovam
  sozinhos a cada quatro segundos, e a primeira leitura **se declara primeira**:
  `/proc` traz contador desde o arranque, e taxa precisa de dois instantes.

- **Aviso de disco apertado, por e-mail.** Dois limites no OU — percentual e
  piso em MB —, porque cada um sozinho erra de um lado: 10% de 8 TB não são
  aperto, e 1 GB livre num disco de 20 GB são. O cliente SMTP é escrito aqui,
  com a `std`.

- **DbLink.** Botão na barra, definições no menu Configurações, e o protocolo
  do MySQL(R) escrito à mão. As tabelas do banco de fora na lista, o conteúdo
  na **mesma grade** das tabelas daqui — agrupar, buscar, totalizar e paginar
  valem igual. Testado contra um MySQL(R) 8.0.46 de verdade.

- **SHA-1**, conferido contra os vetores do FIPS 180-4. Entrou por causa do
  `mysql_native_password` e só por isso: não é usado em lugar nenhum do formato
  do PhxSql — senha continua em PBKDF2-HMAC-SHA256, integridade em CRC-32 e
  SHA-256. Quem define o protocolo é o outro lado.

- **`alertas` e `dblink` no `config.json`**, e o caminho do `base` **já
  resolvido** na tela de configuração: caminho relativo vale a partir de onde o
  servidor foi iniciado, e subir por outro caminho passa a ver outro banco.

- **As sete junções do diagrama**, mais `UNION` e `UNION ALL`. Na tela se
  escolhe **clicando no desenho de Venn**, com o SQL equivalente escrito
  embaixo de cada um. Chave composta, teto que se declara, e as três armadilhas
  do SQL respeitadas: nulo não casa com nulo, família errada é recusada na
  entrada em vez de devolver zero linhas parecendo resposta, e decimal casa por
  valor e não por escala.

- **`criar_tabela` com nome qualificado.** *(corrigido)* `filial.clientes`
  gravava cinco arquivos chamados `filial.clientes.reg` na **raiz** do banco.
  Toda leitura separa o ponto em schema e tabela desde sempre; só a criação não
  separava. A tabela nascia inalcançável e o servidor respondia «criada».

- **Erro com código estável.** A resposta traz `codigo`, `nome`, `classe` e
  `repetir` além do texto. Sem código, integrar exige comparar **texto** — e
  melhorar a redação de uma mensagem quebraria o cliente sem ninguém perceber.
  Número publicado não muda, e há teste que falha se mudar.

- **`sessoes` e `encerrar_sessao`** — quem está falando com o servidor agora, o
  que cada um executa e há quanto tempo, e como derrubar. Porta de dados e
  sessões do navegador na mesma lista.

- **`estatisticas`** — percentis, histograma de faixas que dobram, as mais
  demoradas, e uso por tabela, operação, usuário e código de erro. A média some
  de propósito: mil respostas de 1 ms e uma de 30 s dão média de 30 ms.

- **`checksum` de tabela** e **tempo no ar** no `ping`.

### Sabido

- **Não há TLS em lugar nenhum** — nem no SMTP nem no DbLink. A `std` não traz
  TLS e o projeto não aceita crate. O e-mail serve para relé interno na porta
  25; o DbLink, para rede interna ou túnel. A senha não viaja em texto nos dois
  casos, mas o **dado devolvido pelo DbLink viaja**.

- **Do `caching_sha2_password` só o caminho rápido**, que vale quando o
  servidor já tem a senha em cache. O completo exige TLS ou a chave RSA. Quando
  o servidor pede o completo, o erro diz isso e as duas saídas.

- **Não há compactação (`OPTIMIZE TABLE`).** O `.reg` nunca reaproveita slot
  excluído, e compactar significaria reescrever `rowid` — que é endereço. Uma
  tabela com muitas exclusões cresce e não encolhe: é consequência aceita da
  ordem de digitação ser garantida, não esquecimento. Detalhes em
  `docs/COMPARACAO.md`.

- **O código de erro é por variante, não por situação.** `ESQUEMA_INVALIDO`
  cobre desde config errado até chave de junção incompatível.

- **Junção é de duas tabelas por vez, e só por igualdade.** `ON a.x > b.y` não
  existe: o *hash join* casa por igualdade. `WHERE` sobre o resultado da junção
  também não — a tela filtra depois, na grade.

- **PostgreSQL(R) ainda não conecta.** A definição já pode ser guardada; o
  cliente não existe.

- **O monitor de CPU, memória e rede só existe no Linux.** Fora dele a tela diz
  que não sabe medir, em vez de mostrar zero. O espaço em disco continua
  valendo, porque vem do `df`.

---

## 0.10.0 — 2026-08-28

Uma correção de **perda silenciosa de dado** sob gravação concorrente, a
gravação **20× mais rápida** com durabilidade configurável, e a seção
`recursos` no `config.json`.

### Corrigido

- **Duas gravações simultâneas na mesma tabela sobrescreviam uma a outra.**
  Abrir uma tabela lê o cabeçalho, e o cabeçalho traz `slot_count` — o contador
  que decide onde a próxima linha vai. O servidor tomava a trava para abrir,
  **soltava**, e só então tomava de novo para gravar. Nessa fresta duas
  operações abriam a tabela, as duas guardavam `slot_count = N`, e as duas
  gravavam no rowid N+1: a segunda por cima da primeira, sem erro nenhum.

  Aparecia como «chave duplicada» quando havia índice único sobre a coluna
  — o índice pegava. **Sem índice único, a linha simplesmente sumia.**

  A trava passa a cobrir abrir *e* gravar, como um bloco só. Um teste em
  `tests/tabela.rs` deixa o contrato escrito: duas aberturas disputam o mesmo
  rowid, e por isso quem abre precisa serializar.

### Adicionado

- **Seção `recursos` no `config.json`**: durabilidade, tamanho do lote, cache
  de páginas, teto de memória, threads, percentual de CPU, conexões e usuários
  simultâneos. `conexoes_max` no topo continua valendo, para config antigo não
  parar de subir.

- **Durabilidade configurável**, e é o que acelera a gravação. Medido com
  20.000 linhas na mesma tabela:

  | quando sincroniza | linhas/s | ganho |
  |---|---:|---:|
  | a cada linha (o que o servidor fazia) | 1.289 | — |
  | a cada 100 | 18.264 | 14,2× |
  | a cada 1.000 | 24.858 | 19,3× |
  | só no fim | 26.301 | 20,4× |

  **95% do tempo de uma inserção era `fsync`.** Depois de tirá-lo, a inserção
  custa 37,5 µs, dos quais 65% são os dois índices — que é o gargalo seguinte,
  não este.

  Os bytes vão para o sistema operacional em toda gravação, sempre: um `write`
  direto, sem buffer nosso. Outro processo vê o dado na hora, sincronizado ou
  não. O `fsync` protege de uma coisa só: perder energia antes de o sistema
  descarregar a página.

- **Relógio de fundo** que fecha a janela de durabilidade quando ninguém grava.
  Sem ele, a última venda do dia às 18h ficaria sem `fsync` a noite inteira.

- **`sequencias`** e **`ajustar_sequencia`**: o contador de cada tabela do banco
  num lugar só, e o caminho do administrador para zerar ou pular uma faixa. O
  número continua morando no cabeçalho do `.reg` de cada tabela — a operação
  junta para mostrar, não cria uma segunda cópia.

- **`custo-do-sync`**, o medidor que produziu a tabela acima.

### Sabido

- `por_lote` é o padrão. Quem precisa de durabilidade por operação — um
  livro-razão, por exemplo — põe `"durabilidade": "por_operacao"` e paga os 20×.
- O `cpu_percentual` não é cota do sistema operacional: é quantos núcleos o
  trabalho dividido usa.
- `cache_paginas` e `memoria_max_mb` são lidos e mostrados, mas ainda **não são
  impostos**: o buffer pool do `.ndx` é o trabalho seguinte, e é ele quem vai
  usá-los.

---

## 0.9.0 — 2026-08-28

Duas peças de análise: o agrupamento da grade chega ao nível do Janus GridEX(R)
e do DevExpress(R), e a **tabela dinâmica** ganha assistente e um motor de
tabulação cruzada no servidor.

### Adicionado — tabela dinâmica

- **Operação `pivotar`**, que cruza uma tabela por dois eixos e resume as
  células. A agregação acontece **no servidor**, e é o ponto: um pivot resume —
  cem mil linhas viram uma grade de vinte por doze —, e trazer as cem mil para
  o navegador somar seria pagar o transporte do que vai ser jogado fora.

- **Junção por tabela de consulta.** Cruzar «vendas pela cidade do cliente»
  exige a cidade, que mora na outra tabela. A forma ingênua — uma busca no
  índice por linha de venda — custaria uma descida na árvore por linha. Aqui a
  tabela de consulta é lida **uma vez** para um mapa em memória e o cruzamento
  vira acesso direto: é o *hash join*, e para a forma de dado que um pivot cruza
  (muitos fatos, poucas dimensões) ele é a escolha certa. Teto de 500.000 linhas
  por tabela de consulta, dito no erro quando estoura.

- **Seis resumos**: soma, média, contagem, mínimo, máximo e valores distintos.
  Contagem é o único que dispensa campo de valor.

- **Granularidade de data**: cada valor, por dia, mês, trimestre ou ano. Cruzar
  venda por dia daria uma coluna por dia do ano; o que se quer é por mês ou
  trimestre, e isso é escolha de quem monta, não propriedade do dado. Os rótulos
  saem em ordem lexicográfica crescente (`2026-01`, `2026-T1`), então ordenar
  texto já ordena tempo.

- **Assistente de três passos** na interface (botão *Pivot*, `Alt+7`): quais
  tabelas entram — com as junções propostas a partir das chaves estrangeiras
  declaradas —, que campo vai em cada eixo (arrastando), e o resultado com total
  por linha, por coluna e geral. Mais «copiar como CSV» e «ver o pedido», que
  mostra o JSON equivalente pela porta 5000.

### Adicionado — agrupamento da grade

- **Ordem por nível**: a seta na pastilha inverte crescente/decrescente daquele
  nível. Agrupar por mês quase sempre quer o mais recente em cima. A direção é
  guardada por *campo* e não por posição, então arrastar a pastilha para outro
  lugar não vira a ordem de quem ficou no lugar dela.
- **Rodapé por grupo**, com o total alinhado **na coluna** e não numa tira de
  texto — é assim que se compara um total com os valores acima. Num grupo de
  trinta linhas o cabeçalho já rolou para fora da tela quando o total interessa.
- **Total geral** da grade, sobre o conjunto filtrado inteiro: ele não muda ao
  virar de página, porque um rodapé que muda ao virar de página não é total de
  nada.
- **Expandir tudo / recolher tudo**, e um botão que liga e desliga o rodapé por
  grupo.

### Corrigido

- **`Sequence` aparecia como campo de texto** na paleta do pivot. É um contador.

### Sabido

- O pivot lê até 5.000.000 de linhas por cruzamento. Acima disso o número
  devolvido seria de uma amostra, e amostra sem aviso é pior que recusa.
- A junção é por igualdade de uma coluna com a chave primária da tabela de
  consulta (ou a coluna nomeada em `chave`). Não há junção por faixa nem
  composta.
- Célula vazia quer dizer «nenhuma linha caiu ali», não zero — e os dois são
  informações diferentes.

---

## 0.8.0 — 2026-08-28

**Duas mudanças de formato**, e as duas entram agora porque não há dado em
produção: o campo ganhou identidade e metadados, e o volume aprendeu a cortar
pelo calendário. Junto vem a gestão do banco inteiro — catálogo, configurações,
diretivas e copiar/colar.

### Corrigido

- **Um `onclick` no `#painel` vazava para a tela seguinte.** A gestão do banco
  pendurou o clique no próprio painel, e o `folha()` troca o *conteúdo* do
  painel, não o *elemento* — o tratador sobrevivia à troca de tela e disparava
  na próxima. Clicar em «Configurações e diretivas» abria SysColumns. Corrigido
  em dois lugares: o tratador foi para o container das operações, e o `folha()`
  passou a limpar o `onclick` do painel por garantia.

- **O botão primário ocupava a linha inteira** numa barra de ações. O `.botao`
  nasceu com `width:100%` para o cartão de entrada, onde é o único da linha.

- **A tela de partições calculava por divisão**, que é a conta certa para a
  partição por faixa e errada para a por período: quatro meses apareciam como um
  volume só. Agora lê as fronteiras que o `esquema` devolve.

### Adicionado — formato

- **Esquema `PSCH` versão 3.** Cada coluna passa a carregar `id`, `caption`,
  `descricao` e `mascara`, e cada índice um bit de **primário**. A leitura ainda
  aceita a versão 2: tabela gravada antes abre, ganha um `id` v7 sorteado na
  hora e os textos vazios.

  O `id` é um UUID v7 **nunca reaproveitado**, e existe para que renomear a
  coluna não quebre nada: uma tela ou um relatório apontam para ele, e renomear
  troca só o `nome`. Os metadados moram no `.reg`, com o resto do esquema, pela
  mesma razão que o esquema mora ali — um dicionário externo se perde, se
  desatualiza, e obriga quem copia os cinco arquivos a copiar um sexto.

- **Chave primária de verdade.** Até aqui só havia «índice único», e chave
  primária é mais: é a identidade da linha. Só um índice pode ser primário, ele
  é sempre único, e nenhuma coluna dele aceita nulo — uma identidade nula não
  identifica. As três conferências acontecem no `Schema::new`.

  O papel de uma coluna — primária, estrangeira, composta — **não é gravado na
  coluna**: sai dos índices e das chaves estrangeiras, que são a verdade. Marcar
  no próprio campo criaria uma segunda verdade que divergiria no primeiro
  `ALTER`.

- **Partição por período: mensal, bimestral, semestral e anual.** O volume corta
  quando o período de uma coluna de data vira — ou quando enche, o que vier
  primeiro, porque `registros_por_arquivo` continua sendo teto.

  O endereço não pode sair de divisão quando o corte depende do calendário: dois
  meses rendem quantidades diferentes. Então **cada volume grava no próprio
  cabeçalho** o rowid em que começou e o período em que abriu, e a tabela de
  fronteiras se remonta lendo esses cabeçalhos na abertura. Achar o volume de um
  rowid vira uma busca binária num vetor de dezenas de posições, em vez de uma
  divisão. Sem arquivo extra e sem bloco que cresce.

  **A linha atrasada não volta**: um lançamento de janeiro digitado em março
  entra no volume de março. Voltar significaria escrever no meio de um arquivo
  já fechado, quebrando de uma vez a ordem de digitação e o endereço contíguo.
  Por isso o período de um volume é *o período em que ele abriu*.

- `Paginacao::com_max_arquivos` e `com_modo`; `Periodo` com `chave`,
  `primeiro_mes` e `rotulo`.

### Adicionado — protocolo

- **`copiar_tabela`**, que atravessa databases e schemas. A permissão de criar é
  conferida **no destino**, à parte: sem isso, quem pode ler um banco e não pode
  criar no outro conseguiria escrever onde não devia.
- **`sistabelas`** e **`siscolunas`** (também `systables` e `syscolumns`): o
  catálogo em forma de dado.
- `criar_tabela` aceita `caption`, `descricao`, `mascara` e `id` por coluna,
  `primario` por índice, e `particao` + `particao_coluna`.
- `esquema` devolve os metadados, o papel de cada coluna nas chaves, o modo de
  partição e a **tabela de fronteiras dos volumes**.

### Adicionado — interface

- **Gerir banco** (`Alt+6`), com 15 itens: tabelas, SysTables, SysColumns,
  copiar tabela, configurações, diretivas, editor de menu, conexões, arquivos
  bloqueados, transações, backup/restauração — e, apagados dizendo o que falta,
  triggers, procedures, jobs e modo exclusivo.
- **Configurações gerais do servidor, do banco e dos usuários**, cada uma com
  sua tela, mais **diretivas de acesso ao banco** com os seis portões na ordem
  em que fecham.
- **Copiar e colar tabela** entre bancos, com área de transferência.
- **Cadastro de campos** com id, nome, caption, tipo, tamanho, máscara,
  obrigatoriedade e descrição, e a chave primária escolhida por rádio.
- **Tabela particionada** com grade que mostra como o volume vai cortar, antes
  de gravar — porque depois não muda.
- **Configurações e diretivas da tabela**: a geometria decidida na criação, os
  índices e chaves, os volumes no disco, e o que a tabela herda do servidor.
- **Editor de menu**: troca o nome exibido de qualquer item. Fica no navegador
  de quem mexeu, não no servidor — é preferência de quem opera, não política do
  banco.

### Sabido

- **As telas de configuração leem, não gravam.** Gravar o `config.json` pela
  porta web significaria que uma sessão roubada abre o firewall, esvazia a lista
  de comandos proibidos e cria um supervisor. Criar e alterar usuário pela web
  tem o mesmo problema, com credencial no meio. As telas dizem qual campo mexer.
- **Triggers, procedures, jobs e modo exclusivo continuam não existindo.** As
  telas mostram o que falta e de que dependem; elas não os implementam.
- **Restaurar backup ainda não existe.** Copiar de volta é decidir o que fazer
  com o que está lá, e isso precisa de desenho.
- Mudar a partição de uma tabela existente continua sendo criar outra e copiar
  as linhas — o que refaz os rowids, que é exatamente o motivo de não ser
  automático.

---

## 0.7.0 — 2026-08-28

A tela ganha **gestão de tabelas**. Criar, duplicar, reparar e excluir tabela
passam a existir no protocolo — três operações que a interface pedia e o
servidor não tinha.

### Corrigido

- **Um servidor `somente_leitura` teria deixado apagar tabela.** As três
  operações novas entraram no despacho e ficaram fora de `OPS_ESCRITA`, a lista
  que o modo somente-leitura consulta. `criar_tabela` e `excluir_tabela`
  passariam num servidor marcado como só de leitura. Como a lista é escrita à
  mão, o conserto veio com um teste que a percorre.

- **`criar_schema` estava prometido em dois lugares e não existia.** Aparecia na
  tabela de permissões do `docs/USUARIOS.md` e em `OPS_ESCRITA`; pedir pela rede
  respondia «operacao desconhecida». A biblioteca já sabia criar a pasta —
  faltava a operação. Agora existe, e a tela de nova tabela tem o campo.

- **A largura do sufixo entrava depois do teto de volumes.** `Paginacao::nova`
  confere o teto contra os três dígitos do padrão, então pedir 9.999 volumes era
  recusado *antes* de o quarto dígito existir. Entrou `Paginacao::com_max_arquivos`,
  e a ordem passou a ser largura primeiro, teto depois.

- **«Sem teto» não existe, e o padrão fingia que sim.** O sufixo tem largura
  fixa: com três dígitos o volume 1000 não teria nome de arquivo. Teto omitido
  agora vira o maior que cabe no sufixo — 999 com três dígitos —, em vez de zero,
  que o validador recusava com uma mensagem que não ajudava quem preencheu a tela.

- **A árvore roubava a tela de quem pintasse depois dela.** `montarArvore`
  terminava sempre clicando no Painel; criar uma tabela redesenhava a árvore,
  voltava para a grade — e meio segundo depois o painel chegava por cima. Quem
  vai pintar a própria tela passa `montarArvore(false)`.

### Adicionado

- **Operação `criar_tabela`**, com colunas, índices, schema e paginação. O tipo
  da coluna aceita as três formas que aparecem na prática — `Int8`,
  `Decimal(15,2)` e a forma que o próprio `esquema` devolve —, e a razão é uma
  só: o que a leitura do esquema **devolve** tem de voltar como entrada, senão
  duplicar uma tabela exigiria traduzir cada tipo à mão. As colunas do índice
  vão por **nome**, não por posição: posição muda quando alguém reordena.

- **Operação `duplicar_tabela`**, que copia os cinco arquivos byte a byte. A
  cópia nasce com os **mesmos rowids e a mesma ordem de digitação** — o que uma
  reinserção linha a linha não daria.

- **Operação `excluir_tabela`**, que apaga os cinco arquivos e o espelho
  `.bkp`, todos os volumes de cada um. Exige a permissão `administrar`, não
  `excluir` — poder perder uma linha não é poder perder a tabela — e o nome da
  tabela repetido no campo `confirmar`. A conferência de qual arquivo pertence
  a qual tabela exige o sufixo todo em algarismos: sem isso, excluir `precos`
  levaria `precos_historico` junto.

- **Operação `criar_schema`**, a pasta dentro do database.

- **Botão e menu «Tabelas»**, com as oito operações sobre a tabela escolhida:
  estrutura, editar conteúdo, partições, duplicar, reparar tabela, reparar
  índice, nova tabela e excluir. `Alt+5` abre a grade.

- **Tela de partições**, que mostra em que volume cada faixa de rowid cai e com
  que nome de arquivo. As faixas são **conta, não busca** —
  `volume = (rowid−1) ÷ por_arquivo + 1` —, e a tela diz por que não dá para
  editá-las depois: mudar o divisor mudaria o endereço de cada registro já
  gravado.

- **Tela de nova tabela**, com colunas e índices montados linha a linha, os 21
  tipos com o que cada um custa em bytes, e schema opcional.

- **Gestão de transações no menu Ferramentas.** A tela mostra a **ausência**:
  não há `BEGIN`, `COMMIT` nem `ROLLBACK`, então ela não traz lista de
  transações abertas — uma lista vazia daria a entender que o mecanismo existe e
  está parado. Lista o que de fato existe e o que falta, na ordem.

- `digitos` e `bytes_por_arquivo` na resposta de `esquema`, sem os quais não dá
  para escrever o nome do volume: `_1` e `_001` são arquivos diferentes.

### Mudado

- O menu **Tabela** virou **Tabelas** e absorveu a gestão. Dois menus vizinhos
  com nomes quase iguais obrigariam a adivinhar em qual está cada operação.

- Novo menu **Ferramentas**, espelho da barra pelo teclado.

- A ferramenta *Transações* deixa de ser um botão apagado.

### Sabido

- **Continua sem transações.** A tela nova diz isso; ela não as implementa.
- A **CLI ainda não cria tabela** — só o protocolo e a interface.
- `buscar` e `desbloquear` continuam sem tela.

---

## 0.6.0 — 2026-08-28

A interface deixa de só navegar. **30 das 32 operações** têm tela agora — eram
14 há três versões.

### Adicionado

- **View Database, no padrão Browse → Form do Clarion(R)**, de onde este
  projeto vem. Ferramenta *View DB*, menu *Arquivo → View Database*, `Alt+4`,
  ou um clique no nome do database na árvore.

  A grade lista as tabelas com registros, slots, colunas e índices; um clique
  abre o conteúdo; um clique numa linha abre a **ficha**, com um campo por
  coluna e do tipo certo — caixa de texto para `Memo`, sim/não para `Bool`, e a
  dica do formato no lugar. **Salvar grava, Excluir apaga, Nova linha inclui.**

  Fecha as quatro operações que existiam no servidor e não tinham porta:
  `ler`, `inserir`, `atualizar` e `excluir`. Sobram `buscar` e `desbloquear`.

  Detalhes que a ficha respeita: campo em branco é **nulo**; coluna obrigatória
  tem asterisco; `Sequence` em branco faz o motor numerar; `Uuid` aceita a
  palavra `"novo"`. E o aviso da exclusão diz que **o slot não é
  reaproveitado** — é assim que a ordem de digitação se mantém.

- **`[+]` na árvore**, ao lado de *Bancos de dados*, para criar um database.

- **About no menu Ajuda**, abrindo a **tela de créditos** com a fênix do
  projeto Phoenix, quem fez o quê, e a lista honesta do que o motor se apoia:
  RFC 9562, FIPS 180-4, RFC 4231, RFC 8032, RFC 1951 e os demais — cada um
  escrito aqui e conferido contra o vetor oficial.

### Corrigido

- **A fênix vinha com um retângulo azulado no tema claro.** Os pixels de fora
  do símbolo têm alfa 1 a 3 em azul no arquivo de origem — quase-transparente
  não é transparente. 22.789 pixels zerados antes de embutir. É o mesmo defeito
  que a capa do dossiê já teve, e a mesma lição: alfa quase-zero se enxerga.

---

## 0.5.5 — 2026-08-28

### Adicionado

- **Barra de ferramentas** com as quinze pedidas, cada uma com ícone em SVG
  desenhado aqui e cor da paleta da marca. **Dez funcionam de verdade**; cinco
  aparecem apagadas, com um ponto âmbar, e clicar nelas diz o que falta e do
  que depende.

  Botão que parece funcionar e não funciona custa mais caro do que botão que
  falta: o primeiro só se descobre no meio do trabalho. Sumir com eles da lista
  seria esconder o roteiro; ligá-los a um aviso genérico seria fingir.

  | Ferramenta | Estado |
  |---|---|
  | Start/Stop | mostra o serviço; parar e subir pela tela ainda não |
  | **Query** | **novo** — `SelectMemory` com coluna, operador, valor e teto. Não é SQL, e a tela diz isso |
  | Usuários, Bancos, Repair, Backup, Ajuda | já existiam, agora com atalho |
  | **Diretivas** | **novo** — comandos proibidos, IPs permitidos, somente leitura, firewall, espelho |
  | **Conexões** | **novo** — conexões agora, sessões web, acessos e de onde vêm |
  | **Replicação** | **novo** — papel e portas, dizendo que são configuração e não serviço |
  | Duplicar, Transações, Importar, Server Mail, Blockchain | apagadas — não existem |

  A cor agrupa por família, não por gosto: quinze ferramentas para oito
  matizes, então a repetição é inevitável e precisa significar alguma coisa.

- A tela de consulta fecha a sexta das sete operações que não tinham porta:
  `selecionar_memoria`. **A interface passa de 25 para 26 das 32.** Continuam
  sem tela `inserir`, `atualizar`, `excluir`, `ler`, `buscar` e `desbloquear`
  — ou seja, a edição de dados.

### Corrigido

- **Duas cores da barra não existiam.** `--pend` e `--acento-2` são tokens do
  dossiê, não da interface: Repair e Blockchain saíam com a cor do texto. O
  teste de navegador leu a cor computada e mostrou. De quebra, `--vermelhao` e
  `--laranja` são a **mesma cor** no tema claro, por decisão da marca —
  escolher entre os dois seria escolher nada.

- **`folha()` apagava qual tabela estava aberta.** Carregar a tabela na RAM
  mostrava uma folha, a folha zerava `est.atual`, e a ferramenta de consulta
  abria com o database vazio — o erro saía com uma barra solta, `/naoexiste`.
  Quem escolhe uma tabela na árvore continua com ela escolhida; o que muda é o
  que está na tela.

---

## 0.5.4 — 2026-08-28

### Corrigido

- **O Centro de Controle estava marcado «pronto» e não edita dados.** Contando
  as operações que a tela realmente alcança: **25 de 32**. Faltam `inserir`,
  `atualizar`, `excluir`, `ler`, `buscar`, `selecionar_memoria` e
  `desbloquear` — ou seja, a interface **navega os dados mas não os altera**.

  Virou «parcial» no README e no dossiê. É o mesmo erro da chave estrangeira:
  marcar como pronto o que existe pela metade.

### Adicionado

- `docs/PENDENCIAS.md` refeito numa **tabela única** com os 64 pedidos na ordem
  em que foram feitos, com ☑️ feito, ◐ parcial e ☐ planejado. O saldo:
  **54 feitos, 4 parciais, 6 planejados**.

---

## 0.5.3 — 2026-08-28

### Adicionado

- **Barra de menu tradicional** no Centro de Controle: *Arquivo · Tabela ·
  Memória · Administração · Ver · Ajuda*, com ícone, atalho à direita e
  separadores, como manda o gênero.

  O motivo de existir está na conta: a interface usava **14 das 31 operações**
  do servidor. Backup, conferência de backup, reparo pelo espelho, reindex, a
  tabela em memória inteira e a configuração **não tinham porta de entrada
  nenhuma na tela** — existiam só para quem falasse o protocolo na mão.

  Teclado: a letra sublinhada abre o menu com **Alt**, as setas andam entre
  itens e entre menus, **Esc** fecha. Mais `F5` para atualizar, `Ctrl+B` para
  o backup e `Alt+1/2/3` para Painel, Estrutura e Conteúdo.

  Os itens que precisam de uma tabela ficam cinzas enquanto não houver uma, e
  o estado é recalculado na hora de abrir o menu — não na carga da página.
  As ações que mexem (reindexar, reparar) pedem confirmação.

- **Recado na barra** (`avisar`), que aparece e some sozinho. Um `alert()`
  interromperia quem está trabalhando para dizer "backup pronto"; erro fica
  mais tempo na tela, porque erro se lê.

### Corrigido

- **O menu fechava no mesmo clique que o abria** — defeito meu, achado no
  teste de navegador e invisível na leitura do código. `abrirMenu` refazia o
  `innerHTML` da barra para atualizar o cinza dos itens; isso destruía o
  elemento clicado, o `ev.target` virava um nó solto, e o
  `closest("#menubar")` do fechar-ao-clicar-fora devolvia `null`. Agora só o
  `disabled` dos botões existentes é atualizado.

---

## 0.5.2 — 2026-08-28

### Corrigido

- **Um byte trocado no cabeçalho do slot apagava o registro em silêncio.**
  Achado provando o espelho `.bkp` com um servidor de verdade e o `.reg`
  estragado à mão.

  O byte de status de um slot só pode valer 0 (livre) ou 1 (ativo). A leitura
  testava `slot[0] != ATIVO` e respondia `None` — que é a resposta certa para
  um registro excluído e a **errada** para um registro inteiro. Com o status
  virando lixo (254, no teste), o servidor respondia `{"ok": true,
  "resultado": null}`: nem erro, nem aviso, nem consulta ao espelho, que tinha
  a cópia boa ali do lado.

  O `reparar` errava pelo mesmo motivo, e pior: dava o slot por bom
  (`slot[0] != ATIVO ||` curto-circuitava o CRC), reportava
  `reparados: 1, integro: true` e deixava o registro perdido. Só o `verificar`
  percebia, e sem poder consertar: *"cabeçalho diz 11 registros, varredura
  achou 10"*.

  Agora status inválido é **corrupção**, não estado: cai na mesma segunda
  chance da falha de CRC, e o erro diz qual dos dois aconteceu. Depois do
  reparo o `.reg` volta a bastar sozinho.

  Dois testes de regressão, e o segundo é o contraponto: **excluir continua
  devolvendo `None` sem erro e sem acionar o espelho** — se o conserto tivesse
  passado do ponto, toda exclusão viraria corrupção.

### Sabido

- A segunda chance cobre payload corrompido e status inválido. **Não cobre o
  caso em que o bit trocado deixa o status exatamente em 0**: aí o slot fica
  indistinguível de uma exclusão legítima. Resolver isso exige usar o `.log`
  como desempate — ele registra toda exclusão com data e hora —, e é trabalho
  de outra rodada.

---

## 0.5.1 — 2026-08-28

Rodada de desempenho. Antes de repartir trabalho por nucleos, valia conferir se
o trabalho precisava existir — e nao precisava.

### Mudado

- **CRC-32 slice-by-8: a insercao ficou 3,1× mais rapida.** O medidor apontou o
  CRC da pagina inteira como o custo dominante: 10 µs por pagina de 4 KiB, e
  ~17 toques de pagina por linha inserida, porque toda leitura e toda gravacao
  do `.ndx` passa os 4096 bytes pelo laco byte a byte.

  O laco tem dependencia serial — cada volta precisa do CRC da anterior para
  indexar a tabela —, o que o prende a uma leitura de memoria por byte. Com
  oito tabelas, os oito bytes de uma palavra sao consultados em paralelo pelo
  processador. Mesmo polinomio, mesmo resultado, nenhuma mudanca de formato.

  | | antes | depois |
  |---|---:|---:|
  | só `.reg` | 6,5 µs | 6,3 µs |
  | +1 índice | 50,0 µs | 19,6 µs |
  | +1 único | 132,3 µs | 43,2 µs |
  | +2 índices | 177,1 µs | **56,5 µs** |
  | linhas/s | 5.645 | **17.700** |

  O CRC isolado sai de 10,00 µs por pagina (0,41 GB/s) para 2,34 µs (1,75 GB/s).

  Como um CRC diferente invalidaria todo arquivo ja gravado, o laco byte a byte
  ficou no codigo como definicao de referencia, e ha teste comparando os dois em
  todo tamanho de 0 a 300 bytes com quatro sementes, mais a pagina de 4096.

### Adicionado

- **`phxsql-core/src/paralelo.rs`** — divisao de faixa entre nucleos com
  `std::thread::scope`, sem dependencia externa. Nao e um `rayon`: e o pedaco
  de `rayon` que este projeto usa.

  A ordem do resultado e **sempre** a do laco sequencial — cada pedaco junta o
  seu num vetor proprio e os vetores sao concatenados na ordem dos pedacos. Uma
  consulta que mudasse de ordem conforme o numero de nucleos da maquina seria
  pior do que uma consulta lenta.

- **Varredura em memoria dividida entre nucleos.** A consulta sem atalho de
  mapa e o unico trecho do motor que divide bem: tudo em RAM, nada gravado, cada
  linha independente das outras. Um milhao de linhas, 4 nucleos: **36 ms → 20 ms**.

- `examples/paralelo.rs` e `examples/onde-doi.rs`, os dois medidores que
  sustentam os numeros acima.

### Sabido

- **O ganho da varredura paralela e 1,8×, nao 4×.** O filtro por linha e barato,
  entao a varredura e presa a banda de memoria e nao a conta: mais nucleo nao
  compra mais banda. O numero esta aqui para ninguem esperar escala linear.

- **A insercao continua monothread, e nenhuma thread a acelera.** Inserir uma
  linha e uma descida na B+tree em que cada passo depende do anterior. O que
  falta para varias conexoes gravarem ao mesmo tempo nao e thread — o servidor
  ja abre uma por conexao —, e **trava por tabela em vez da trava unica global**
  que hoje serializa todo acesso a dados.

---

## 0.5.0 — 2026-08-28

### Adicionado

- **Três tipos de identificador**, todos de largura fixa e inteiros dentro do
  slot — nenhum vai para o `.bin`, nenhum custa um ponteiro.

  | Tipo | Bytes | O que é |
  |---|---:|---|
  | `Uuid` | 16 | UUID de 128 bits do RFC 9562, v4 e v7 |
  | `Uuid256` | 32 | identificador de 256 bits — **não é um UUID**, o padrão só define 128. Existe porque um SHA-256 cabe exato |
  | `Sequence` | 8 | contador crescente da tabela, atribuído na inserção |

- **UUID v7, e o motivo é medido.** Os 48 bits altos de um v7 são o relógio em
  milissegundos, em big-endian; como a chave do `.ndx` guarda os bytes na ordem
  natural, comparar bytes é comparar tempo. Chave aleatória manda cada inserção
  para uma folha diferente da B+tree; chave crescente cai sempre na folha mais à
  direita, que já está na memória.

  É exatamente onde a bancada dói: a inserção cai de 5.089 linhas/s no primeiro
  milhão para 3.626/s no décimo, com o disco parado e a CPU em 99%. É a árvore
  sendo semeada, não o disco.

- **Monotonia de verdade.** Dois v7 no mesmo milissegundo sairiam fora de ordem
  se dependessem só do relógio, então os 12 bits de `rand_a` viram um contador
  (método 1 da seção 6.2 do RFC 9562): nasce sorteado a cada milissegundo novo e
  soma 1 a cada id seguinte; estourou, o relógio anda 1 ms para frente em vez de
  repetir. O gerador nunca devolve valor menor ou igual ao anterior, nem entre
  *threads* — há teste que pede vinte mil seguidos e exige que cada um cresça.

  O layout se confere contra o vetor do apêndice A.6 do próprio RFC.

- **A sequência, e a diferença para o rowid.** O rowid é a *posição física* do
  registro e não se escolhe; a sequência é dado — nasce onde se quiser, é
  gravada à mão e continua de onde parou. Valor escrito à mão **empurra o
  contador** para depois dele, senão a próxima numeração automática passaria por
  cima do que já existe. Excluir não devolve o número. Numa alteração, nulo
  mantém o número que a linha já tinha: a sequência identifica a linha, e
  renumerar trocaria a identidade dela.

- Pelo protocolo o id viaja em texto e **sai sempre na forma canônica
  minúscula** — um id que se escreve de dois jeitos vira dois ids no olho de
  quem lê. A palavra `"novo"` no lugar do valor pede ao servidor que gere um
  (`"v4"` força a versão sorteada); `Uuid256` aceita o prefixo `0x`.

- `crates/phxsql-store/examples/identificadores.rs` — monta uma tabela de
  blocos encadeados pelo hash, com a altura numerada pela sequência. Existe
  porque criar tabela ainda só se faz escrevendo Rust.

- Seção 4 do dossiê e seção 8 do `docs/FORMATO.md`.

### Mudado

- **Formato em disco**: os bytes 36..44 do cabeçalho do `.reg`, antes
  reservados, passam a guardar o próximo valor da sequência. Zero continua
  significando "nunca usada", então `.reg` antigo abre sem conversão.

- Uma sequência por tabela: duas dividiriam o mesmo contador do cabeçalho, o
  que só pareceria defeito. O esquema recusa na criação.

### Sabido

- **A sequência sozinha não é chave única.** O contador só vai ao disco no
  `sincronizar`; queda de energia antes disso o faz voltar atrás, e números já
  gravados podem repetir. Quem precisa de unicidade declara um índice `unico`
  sobre a coluna — aí é o índice que recusa.

- Um `.reg` gravado com estes tipos **não abre** numa versão anterior do
  binário: a tag do tipo é desconhecida lá, e o erro é claro.

---

## 0.4.1 — 2026-08-28

Rodada de revisão: nada de recurso novo, só o que a leitura do próprio projeto
achou de errado.

### Corrigido

- **A bancada media coisas diferentes dos dois lados.** Na varredura por faixa
  o MySQL(R) recebia `COUNT(*) + SUM(valor)` sobre 1.250.000 linhas enquanto o
  PhxSql lia 20.000 — mesma pergunta, 1,6% do trabalho. O «5× mais rápido» que
  saía dali não era o motor: era o serviço menor. A fase `varrer` de
  `examples/carga.rs` passou a ler a faixa inteira e somar o valor, e a
  medição de dez milhões foi **refeita do zero**.

  É o segundo erro deste tipo — o primeiro favorecia o MySQL(R), este
  favorecia o PhxSql. Por isso a bancada ganhou uma quarta regra: *mesma
  quantidade de trabalho*, não só mesma forma de pergunta.

  A prova de que agora está igual não é a promessa, é a soma: os dois motores
  devolvem 1.250.000 linhas e **5.576.201.000,00**, o mesmo total até o
  centavo, por dois códigos sem uma linha em comum.

  E o resultado sobreviveu ao conserto — a varredura continua a favor do
  PhxSql, por **3,3×** em vez dos 5× que a montagem errada prometia. A nova
  medição: inserção 20,7× mais devagar (4.039 linhas/s contra 83.492), busca
  pontual 2,6× mais devagar, exclusão 2,0×, atualização empatada, varredura
  3,3× mais rápida. Escreve 2,29 GiB onde o MySQL(R) escreve 32,03; ocupa
  2,27 GiB onde ele ocupa 0,88.

- **Campo com nome errado no `config.json` era silencioso.** Quem quisesse
  trocar a porta escreveria `"porta": 5001`, e o campo se chama `bind`: o
  servidor subia na 5000 sem uma palavra. O arranque agora lista os campos que
  não reconheceu e diz que o valor foi ignorado. Não vira erro — config antigo
  continua subindo.

- **Seis marcas de terceiros sem o `(R)`**: `MySQL` em `docs/REPLICACAO.md` e
  no dossiê, `HFSQL` em dois módulos, `SQLite` e `Clarion` no `docs/PLANO.md`.

- **O painel tem sete gráficos, não nove.** O README e o dossiê diziam nove.
  Contados: um de área, um de anel e cinco de barras.

- **A versão que o servidor anunciava estava errada.** O `Cargo.toml` do
  workspace ainda dizia `0.1.0` enquanto este changelog ia em 0.4.0 e os
  pacotes saíam com 0.4.0 no nome. Como `VERSAO` é `env!("CARGO_PKG_VERSION")`,
  o `ping`, o `quem_sou` e o rodapé do Centro de Controle respondiam `0.1.0` a
  quem perguntasse. Cliente que decide compatibilidade pela versão estava
  recebendo a resposta errada há três lançamentos.

- **Números velhos no dossiê.** A capa dizia 276 testes (são 280) e 3.184
  linhas de doc (são 3.261); o rodapé ainda dizia *PhxSql 0.3.0 · 19.242
  linhas · 69 KB de interface*, três números defasados de uma vez. Remedidos:
  20.224 linhas de Rust, 158 KiB de interface, 280 testes. A regra do projeto é
  medir, e ela vale para o documento que apresenta o projeto.

- **A bancada não estava no dossiê.** A comparação com o MySQL(R) em dez
  milhões de registros — a maior medição já feita aqui — existia só em
  `bancada/` e no roteiro, como uma linha marcada «pronto». Virou a seção 16,
  com a figura, a tabela dos oito números e o diagnóstico da inserção.

- **Três pedidos não estavam nem registrados.** Triggers, stored procedures e
  jobs foram pedidos e não constavam do roteiro do dossiê — nem como «a fazer».
  Ausência que não está escrita é ausência que se esquece.

### Adicionado

- **`docs/PENDENCIAS.md`.** A revisão do que falta, em um lugar só: o que foi
  pedido e não existe, o que depende de decisão do Adriano, o que está travado
  de fora, o checklist das perguntas já respondidas, e o único buraco que a
  medição apontou sem ninguém pedir.

- **`empacotar.sh`.** Monta os pacotes de Linux e Windows e o zip de fontes.
  Os das rodadas anteriores foram feitos à mão — pacote que ninguém consegue
  refazer é pacote em que não se deve confiar. O zip de fontes sai de
  `git archive`, que respeita o `.gitignore` de graça.

- **`docs/dossie/numeros-da-bancada.py`.** A figura, a tabela e o diagnóstico
  da seção 16 passam a ser **gerados** de `bancada/resultados.json`. Número
  digitado envelhece calado; número gerado não tem como divergir da medição.

- **`.gitignore`** para os 2,4 GB que a bancada cria em `bancada/phxsql/`.

---

## 0.4.0 — 2026-08-27

### Adicionado

- **Painel.** A primeira tela depois do login: o servidor inteiro em gráficos
  — bancos, registros, usuários, conexões, acessos, recusados, IPs bloqueados
  e tabelas em RAM nos números do topo; operações por hora nas últimas 24 h;
  operações mais pedidas; usuários por nível; maiores tabelas; de onde vêm os
  acessos; e quem mais usou.

  Tudo de **uma** chamada — a operação `painel` agrega no servidor. Dez
  chamadas deixariam a tela dez vezes mais lenta só pela ida e volta. E o
  painel conta **só o que quem está olhando poderia abrir**: base sem
  permissão de leitura não entra na conta.

  Os gráficos são SVG escrito à mão — barras, área e anel —, como o resto do
  projeto. Usam `currentColor` e os tokens do tema, então trocam de cor com o
  sol/lua sem uma linha a mais.

- **O phx-grid v0.8.0 na aba Conteúdo.** O grid do ecossistema Phoenix, ES5
  estrito e sem dependência. Arrastar um cabeçalho para a faixa de cima
  **agrupa**, com contagem e agregados por grupo; vários níveis empilham e as
  pastilhas reordenam arrastando. Vieram junto a busca global e a paginação.

  As colunas saem do **esquema** da tabela, não de uma lista escrita à mão —
  tabela nova aparece certa sem tocar na página. E o grid segue o tema do
  console.

- **Comparação medida com o MySQL(R)**, 10.000.000 de registros, em
  `bancada/`. Tudo para ser refeito: `python3 bancada/medir.py 10000000`.

- **Espelho `.bkp`** (`"espelho": true`): toda escrita no `.reg` vai também
  para um irmão, e a leitura tenta o espelho quando o CRC falha. `phxsql
  reparar` conserta nos dois sentidos e **conta** o que não teve salvação.

- **Três portas de replicação:** `envio` e `retorno` separadas, validadas
  contra a porta de dados, a da web e uma contra a outra.

- `phxsqld --pagina` escreve o Centro de Controle num arquivo — da **mesma**
  função que serve o navegador.

### Corrigido

- **Ligar o espelho apagava a cópia boa.** `espelhar()` copiava o `.reg` por
  cima do `.bkp` existente: estragar o principal e religar o espelho destruía
  a segunda chance. Um teste pegou. Agora só semeia o que ainda não existe.

- **Erro de medição na bancada.** A primeira versão mandava ao MySQL(R) um
  único `WHERE id IN (…)` e ao PhxSql vinte mil buscas separadas — 41× a
  favor do MySQL(R) pela *forma da pergunta*, não pelo motor. Corrigido para
  uma instrução por operação dos dois lados; o SELECT pontual passou de 41×
  perdendo para 3,4×, e o UPDATE de perdendo para empate.

- **Gráficos desproporcionais.** O `viewBox` de 620 dentro de cartões de
  ~370 px encolhia o desenho inteiro em 0,6 — texto de 12 px virava 7 px.
  Cada gráfico passa a nascer com a largura do cartão que o recebe.

- **Colisão de nome entre o relay e o backup**: o campo do servidor remoto se
  chamava `destino`, e `destino` já era o diretório do backup. Renomeado.

### Sabido

O que a medição diz e ninguém deve esconder: **a inserção é o nosso buraco**
— 3.685 linhas/s contra 95.301 do MySQL(R), e é CPU, não disco. Continuam
faltando triggers, stored procedures, jobs, transporte de replicação,
start/stop pela interface, transações e TLS.

**280 testes**, clippy limpo, zero dependências externas.

---

## 0.3.0 — 2026-08-27

### Corrigido

- **O nível de usuário quase afrouxou todo `config.json` existente.** O padrão
  do campo novo `nivel` era `leitor`, e isso mudava o comportamento de quem já
  tinha config: base sem regra explícita passava de *nega tudo* para *lê tudo*.
  Um teste antigo (`sem_curinga_e_sem_base_nega_tudo`) quebrou e apontou o
  problema. Existe agora `Nivel::Nenhum`, que é o padrão, e o teste antigo
  passa sem alteração — que é a prova de que nada mudou para quem já tem
  config.

- **`phxsqld --usuarios` mentia sobre quem podia o quê.** Escrevia
  `(nenhuma)` para usuário sem regra de base, mesmo quando o nível dava poder,
  e mostrava `supervisor` numa coluna em vez do nível. Agora mostra o nível e
  o que ele concede.

### Adicionado

- **Nível de usuário:** `nenhum`, `leitor`, `operador`, `dono`, `admin`. Cada
  um contém o anterior, e há teste que percorre as dez atividades para
  garantir. A regra de uma base específica ganha do nível, inclusive para
  **tirar** poder — dá para dar `admin` a alguém e ainda assim fechar uma base.

- **Backup em ZIP**, com o DEFLATE (RFC 1951) escrito neste projeto — Huffman
  fixo mais casamento LZ77. Nome
  `BancoNome_Admin_Data_HoraMin.zip`, com o manifesto dentro.

  A prova não é o teste de ida e volta com o próprio código; é o mundo abrir:
  `unzip -t` passa todos os CRC, e o `zipfile` do Python extrai e confere byte
  a byte contra o original. **18.311 → 2.406 bytes, 87% menor.**

- **Backup agendado**, seção `backup` no `config.json`, desligada por padrão.
  `hora` (uma vez por dia) ou `cada_horas`, com `manter` para a retenção. O
  relógio confere de minuto em minuto em vez de dormir até a hora — dormir
  horas seguidas é frágil. A faxina só apaga arquivo com a cara dos nossos.
  Todo backup agendado entra no `acessos.log`.

### Sabido

Continua tudo da 0.2.0: replicação sem transporte, sem start/stop pela
interface, sem transações, sem TLS, sem compactação, sem SQL, sem MCP, sem
ODBC.

**276 testes**, clippy limpo, zero dependências externas.

---

## 0.2.0 — 2026-08-27

### Corrigido

- **Sondagem de travessia de diretório não contava violação.** Nome de
  database, tabela ou schema com `..` ou barra já era recusado pelo motor,
  mas era recusado **calado**: não contava tentativa e não gerava bloqueio.
  Auditado com seis sondagens seguidas (`../../../etc`, `/etc`, `C:\dados`,
  byte nulo, quebra de linha): seis recusas, seis linhas no `acessos.log`,
  **zero bloqueios**. Quem sondasse podia tentar a noite inteira.

  Agora é violação grave, na mesma classe de comando proibido: **bloqueia na
  primeira tentativa** e cria a regra de firewall. Conferido contra servidor
  de verdade — uma sondagem, um bloqueio, uma regra.

  A separação está em `catalogo::nome_hostil`, deliberadamente distinta de
  `validar_nome`: `"minha tabela!"` é um nome ruim (alguém errou, recusar
  basta); `"../../etc/passwd"` não é nome nenhum.

- **Colisão de nome entre o relay e o backup.** O campo que escolhe o servidor
  remoto se chamava `destino` — e `destino` já era o diretório do backup.
  Resultado: todo pedido de backup ia parar no relay e voltava com "esta
  interface não fala com outro servidor". Renomeado para `servidor`. Achado
  ligando as duas peças, não lendo o código.

- **`fe_de_bytes` do Ed25519 lia sete bytes onde precisa de oito.** O pedaço
  do meio perdia o bit 152. Passou despercebido no teste do ponto base — que
  tem esse bit em zero — e só apareceu quando os vetores da RFC 8032 rodaram.
  É exatamente por isso que a regra "criptografia se confere contra vetor
  oficial" existe.

- **Duas cores presas ao tema escuro.** O gradiente da tela de entrada e a
  tinta do botão eram literais. No tema claro o botão ficava com tinta quase
  preta sobre vermelho escuro. Viraram token.

### Adicionado

- **Tabela em memória e `SelectMemory`.** A tabela inteira em RAM, com
  consulta que não toca em disco. Filtros (`=`, `!=`, `<`, `<=`, `>`, `>=`,
  `contem`, `comeca`, `termina`, `nulo`, `nao_nulo`), ordenação múltipla,
  projeção de colunas, `pular` e teto. Filtro de igualdade numa coluna
  mapeada evita a varredura, e a resposta diz qual mapa usou.

  **Medido** (`cargo run --release --example memoria`, 50.000 linhas, a mesma
  pergunta pelos dois caminhos):

  | caminho | tempo | linhas examinadas |
  |---|---:|---:|
  | varrendo o `.reg` | 55.878 µs | 50.000 |
  | `SelectMemory` | 641 µs | 8.333 |

  **87×.** Carga para a RAM: 53 ms, 2.205 KB de valores. O exemplo confere as
  duas respostas linha por linha antes de imprimir o número.

  Nada entra em memória sozinho, e toda escrita atualiza a cópia residente
  **dentro da mesma trava** do disco — não existe janela em que os dois
  discordem.

- **Chave assimétrica Ed25519 como segundo fator.** Escrito do zero, mais o
  SHA-512 que ele exige. Conferido contra os quatro vetores da RFC 8032
  seção 7.1, o vetor de 1023 bytes, e os quatro do FIPS 180-4 para o SHA-512.

  E a prova que vale mais: um cliente de teste que assina com a implementação
  **de referência** da RFC (Python puro, independente desta) gerou a mesma
  chave pública e teve a assinatura aceita pelo servidor.

  `phxsqld --gerar-chave` imprime o par uma vez. `"chave_publica"` no usuário
  do `config.json` passa a exigir assinatura no login, sobre o **mesmo**
  desafio da senha — então a assinatura também vale uma vez só.

- **Sistema de backup com manifesto conferível.** Cópia mais um `backup.json`
  com o SHA-256 de cada arquivo, e um comando que lê tudo de volta e confere.
  Acha arquivo que sumiu, arquivo que mudou (mesmo do mesmo tamanho) e
  arquivo que apareceu sem estar no manifesto.

  ```
  phxsql backup <base> <destino>        com o servidor parado
  phxsql conferir-backup <destino>      sai com erro se não bater
  {"op":"backup","destino":"..."}       com o servidor no ar, sob a trava
  ```

- **Alternador de tema, sol ☀️ e lua 🌙.** Paleta clara completa, começando no
  que o sistema pede e lembrando a escolha por navegador. O vermelhão
  escurece para `#c63c0a` no claro, por contraste — a mesma adaptação que o
  dossiê já fazia.

- **Campos de conexão no login:** servidor (IP ou DNS), porta, usuário, senha,
  chave privada e database. A porta que aparece é a que o servidor
  **realmente** escuta, lida do `/saude`.

- **Console para mais de um servidor.** Apontar o login para outro endereço
  abre uma conexão para ele, mantida viva pela sessão. `web.servidores`
  começa **vazio** — interface que fala com qualquer endereço é proxy aberto
  de saída.

- **`replicacao.escuta`:** o socket onde o *source* serve os eventos, separado
  da porta de dados. O config recusa colisão com a porta de dados e com a
  da web.

### Mudado

- Nomes de bancos de terceiros na documentação passam a levar **(R)**.
  Exceções deliberadas: nomes de pacote (`rusqlite` é identificador, não
  marca) e citações literais de texto alheio.
- `memoria_carregar`, `memoria` e `SelectMemory` pedem permissão de **ler**,
  não de administrar: é o mesmo dado do disco por outro caminho.
- O arranque avisa alto quando o papel de replicação não é `isolado`, porque
  o transporte de eventos ainda não existe.

### Sabido — o que ainda não funciona

- **Replicação não transporta evento.** A configuração entra e valida; o
  desenho está em `docs/REPLICACAO.md`; o `.log` v2 com imagem da linha é o
  próximo passo. Hoje o papel é só um rótulo.
- **Start/stop do serviço de dados pela interface** não existe. Parar a porta
  5000 sem derrubar o processo exige mexer no laço de aceitação, e prefiro
  fazer isso inteiro a fazer pela metade.
- **Sem transações**, logo sem o A nem o I do ACID.
- **Sem TLS.** O tráfego vai em claro; a credencial não, quando se usa
  desafio-resposta ou chave.
- **Sem compactação**, sem camada SQL, sem MCP, sem ODBC.
- `crypto.subtle` com Ed25519 é recente; navegador sem suporte não assina, e
  a página diz isso em vez de fingir.

**254 testes**, clippy limpo, zero dependências externas.

---

## 0.1.0 — 2026-08-27

Primeira versão que roda ponta a ponta.

### Adicionado

- **Os cinco arquivos:** `.reg` (registros na ordem de digitação, CRC por
  registro, esquema embutido), `.ndx` (B+tree com divisão de páginas, chave
  composta, ASC/DESC/NOCASE/único), `.bin` e `.memo` (blocos com CRC e
  contabilidade de espaço morto), `.log` (diário datado das três operações).
- **Ordem de digitação como garantia:** slot excluído nunca é reaproveitado.
- **Paginação em volumes** `_001`, `_002`, … com abertura preguiçosa. O
  volume sai da aritmética do rowid, então o índice não paga nada por ela.
- **Hierarquia** database → schema → tabela, em diretórios.
- **Chave estrangeira** no esquema, com CASCADE / RESTRICT / SET NULL.
- **Reindex:** recria o `.ndx` do zero a partir do `.reg`.
- **Servidor TCP na porta 5000**, protocolo JSON Lines, `config.json`.
- **Log de acessos** por IP, com data e hora ao milissegundo — inclusive das
  tentativas recusadas.
- **Cadastro de usuários** com senha em PBKDF2-HMAC-SHA256 de 210.000
  iterações, e permissão por base em dez atividades.
- **Login por desafio-resposta** (a senha não trafega) e por Base64.
- **Política, blacklist e gancho de firewall:** comando proibido bloqueia o
  IP na hora; token e senha errados contam tentativa.
- **Centro de Controle:** interface web embutida no binário, servida pelo
  próprio `phxsqld`.
- **Linha de comando** com nove comandos, e compilados para Linux e Windows.

### Sabido

Zero dependências externas — só a `std`. JSON, CRC-32, SHA-256, HMAC, PBKDF2
e Base64 escritos aqui, cada um conferido contra vetor oficial.
