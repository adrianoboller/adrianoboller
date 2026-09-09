# As diretivas do banco e do servidor — o HFSQL e o `ALTER … SET` do PhxSql

> **Todo número e todo comportamento deste documento saiu de uma corrida
> contra o motor vivo**, em 07/09/2026, commit `b463b1d` + esta frente. O
> script que refaz tudo é `bancada/diretivas/sql.py`; o mapa da §2 foi
> exercitado campo a campo, e o que diz «não existe» foi *perguntado ao
> servidor*, não deduzido do código.

O HFSQL espalha a configuração por oito portas — `HSetServer`,
`HSetTransaction`, `HSetLog`, `HSetIntegrity`, `HSetDuplicates`,
`HSetTrigger`, as propriedades da `Connection` e `HManageTask`. O pedido do
dono foi centralizar:

```sql
SHOW SERVER SETTINGS;
SHOW DATABASE erp SETTINGS;
SHOW TABLE clientes SETTINGS;
SHOW CONNECTION SETTINGS;

ALTER SERVER   SET max_linhas = 500 MOTIVO 'pico de exportacao';
ALTER DATABASE erp SET comandos_proibidos = (reindexar, excluir_tabela);
```

Um verbo para ver, um verbo para mudar, o escopo dito por extenso, e toda
alteração no **diário administrativo** com os nove campos que ele pediu.

---

## 1. A gramática, e onde cada comando desemboca

| Comando | Operação do protocolo | Portão |
|---|---|---|
| `SHOW SERVER SETTINGS` | `diretivas` (`escopo:"servidor"`) | `administrar` |
| `SHOW DATABASE <b> SETTINGS` | `diretivas` (`escopo:"database"`) | `administrar` |
| `SHOW TABLE <t> SETTINGS` | `diretivas` (`escopo:"tabela"`) | `administrar` |
| `SHOW CONNECTION SETTINGS` | `diretivas` (`escopo:"conexao"`) | `administrar` |
| `ALTER SERVER SET c = v` | `diretiva_gravar` → **`config_gravar`** | `administrar` |
| `ALTER DATABASE <b> SET c = v` | `diretiva_gravar` (`escopo:"database"`) | `administrar` |
| `ALTER TABLE <t> SET c = v` | recusa, nomeando o caminho que funciona | — |
| `ALTER CONNECTION SET c = v` | recusa, nomeando o que existe | — |

O `MOTIVO '…'` (ou `COMMENT '…'`) é opcional no fim do `ALTER … SET` e
alimenta o diário. É opcional de propósito: exigir motivo em toda mudança
ensina a escrever «x», e um diário cheio de «x» é pior que um com o campo
vazio, que ao menos não mente.

**`ALTER SERVER SET` não tem caminho próprio de gravação.** Ele monta
`{"op":"config_gravar","campos":{…}}` e chama a mesma função: mesmo portão,
mesma conferência de tipo, mesma gravação atômica, mesma aplicação a quente,
mesmo diário. Um segundo caminho ao lado daquele seria a porta dos fundos que
a lei desta casa manda procurar — *portão de permissão é UM só*.

Os valores aceitam `TRUE`/`FALSE`, `ON`/`OFF`, `YES`/`NO`, `SIM`/`NAO`,
número (inclusive negativo), texto entre aspas, palavra solta (é o
`recursos.durabilidade = por_lote`) e lista entre parênteses. Os escopos
também se escrevem em português: `SERVIDOR`, `BANCO`, `TABELA`, `CONEXAO`.

### O que a gramática NÃO rouba

`SHOW TRIGGERS`, `SHOW PROCEDURES` e `SHOW PROCEDURE STATUS` continuam do
`phxsql_sql::rotina`. O detector de diretiva só reclama a frase quando a
palavra depois do `SHOW` é `SERVER`, `DATABASE`, `TABLE` ou `CONNECTION` —
quatro palavras que o outro nunca atendeu. Há teste dos dois lados
(`nao_rouba_o_show_do_rotina`), e ele é o que impede a camada nova de deixar o
servidor sem listar gatilho nenhum.

---

## 2. O mapa: cada diretiva do HFSQL contra o motor de hoje

Legenda: **✔ existe** — já existe e foi exercitado; **➕ entrou** — entrou
nesta frente; **✖ dispensa** — não existe, com o motivo técnico.

### 2.1 Diretivas booleanas do servidor (`HSetServer`)

| HFSQL | PhxSql | Estado |
|---|---|---|
| `hActiveDirectory` | — | ✖ **dispensa.** Não há integração com diretório corporativo, e ela não é um campo: é um protocolo (LDAP/Kerberos) contra um servidor de fora, e este motor tem **zero dependências externas**. O que existe é o cadastro próprio com PBKDF2-SHA256 e desafio-resposta (`docs/SEGURANCA.md` §2). Ligar AD seria um cliente LDAP escrito à mão — decisão de arquitetura, não de diretiva. |
| `hAutoStatisticalCalc` | — | ✖ **dispensa.** Não há estatística de índice porque **não há otimizador que a leia**: o `traduzir.rs` escolhe entre `buscar` (índice) e `varrer` pela forma do `WHERE` e pelos índices declarados, não por cardinalidade. Estatística sem quem a consulte é arquivo que cresce. Entra no dia em que houver planejador de custo. |
| `hFindKey` | — | ✖ **dispensa** pelo mesmo motivo: a escolha da chave já é automática e determinística, sem interruptor a oferecer. |
| `hlbActive` e os sete pesos `hlb*` | — | ✖ **dispensa.** O PhxSql tem **cluster com eleição e promoção automática** (`docs/CLUSTER.md`), não balanceador: o cluster resolve *quem é o master* quando um nó cai; o balanceador do HFSQL reparte carga de leitura entre réplicas por pesos. São problemas diferentes, e o nosso não tem réplica de leitura servindo consulta a cliente — a réplica existe para durabilidade e promoção. Pesos de balanceamento sem tráfego repartido seriam sete campos que ninguém lê, que é exatamente a doença que `recursos.cache_paginas` já custou aqui. |
| `hMode2GB` | — | ✖ **dispensa.** É um limite do formato do HFSQL. Aqui o `.reg` é **paginado por volume** (`docs/FORMATO.md`), e a partição por quantidade e por período já passa de 2 GB sem interruptor nenhum — medido em 1.000.000 de linhas em dez volumes. Não há limite a destravar. |
| `hTelemetryEnable` | `telemetria_ligar` / `telemetria_desligar` | ✔ **existe**, com uma diferença que importa: a telemetria daqui é **local** — o painel de bolhas do Centro de Controle. Nada sai da máquina, e não há para quem enviar. `telemetria.*` (cores e limiares) são diretivas de servidor, editáveis, a quente. |
| `hConserveHistoryReindexing` | — | ✖ **dispensa.** O `reindexar` não guarda histórico próprio: o que ele fez aparece no `acessos.log` (op, tabela, duração, quem) como qualquer operação. Um segundo histórico só da reindexação seria uma trilha paralela ao log que já existe. |

### 2.2 Diretivas de valor do servidor

| HFSQL | PhxSql | Estado |
|---|---|---|
| `hServerPort` | `bind` | ✔ existe — exige reinício (o efeito a quente é da op `servico_subir`) |
| `hDatabasePath` | `base` | ✖ **dispensa como diretiva**: não está em `CAMPOS_EDITAVEIS`, e a ausência é decisão. Mudar a raiz dos dados pela rede é mover o banco inteiro por um comando; continua sendo edição do arquivo. |
| `hNdxCacheSize` | `recursos.cache_paginas` | ✔ existe — **a quente**. Em páginas, não em MB: é a unidade do `.ndx`. O cache comprou **2,40×** (`docs/DESEMPENHO.md`). |
| `hMaxNumberConnection` | `recursos.conexoes_max` | ✔ existe — exige reinício |
| `hkaInterval` / `hkaTimeout` | `timeout_s` | ✔ **parcial.** Há prazo de leitura por conexão; não há keep-alive com intervalo próprio. Uma conexão ociosa cai pelo `timeout_s`, e não por sonda. |
| `hLogPath` | `log_acessos` | ✔ existe no `config.json` — ✖ **dispensa como diretiva editável**: mover o log de acesso pela rede é o primeiro passo de quem quer apagar o rastro. Edição do arquivo. |
| `hLogLevel` (`"WL"`, `"WL,PARAM"`, `""`) | `profiler_ligar`/`profiler_desligar` + `acessos.log` | ✔ **existe, com dois níveis em vez de três**: o `acessos.log` é sempre ligado e grava op/usuário/IP/duração/erro (o `"WL"` deles); o **Profiler** grava o pedido inteiro com os parâmetros (o `"WL,PARAM"`), liga e desliga a quente, e **redige analisando, nunca recortando**. Desligar o log de acesso não existe, e isso é decisão. |
| `hMaxLogSize` | `profiler.arquivo_mib` + `profiler.arquivos` | ✔ existe — a quente, e vale para o arquivo corrente |
| `hBackupPath` | `backup.destino` | ✔ existe — exige reinício |
| `hJNLPath` / `hJNLBackupPath` | — | ✖ **dispensa.** O diário é **por tabela**, no `.log` ao lado do `.reg`; não há journal central para apontar em outro disco. Separá-lo seria mudança de formato, e formato muda cedo — hoje já há dado em produção. |
| `hTempDirectory` | — | ✖ **dispensa.** O motor não escreve temporário fora do diretório da tabela: a troca atômica é `.tmp` + `rename` **no mesmo sistema de arquivos**, que é o que torna o rename atômico. Um diretório temporário configurável em outro volume quebraria essa garantia. Há catraca que impede `std::env::temp_dir()` solto (`conferidor_temporarios.rs`). |
| `hActivityStatisticsPath` / `hActivityStatisticsPeriod` / `hMaxActivityStatisticsSize` | `telemetria` + `painel` | ✔ **parcial.** As contagens vivas existem e o painel as mostra; não há gravação periódica em arquivo com rodízio. |
| `hCacheNbUnusedFiles` | — | ✖ **dispensa.** Não há cache de tabelas abertas: a tabela **abre e fecha a cada operação**, de propósito — é o que permite a trava única de dados e o `.reg` append-only sem descritores presos. O cache que este motor tem é de **páginas do `.ndx`** (`recursos.cache_paginas`), que é onde os 83,5% do custo estavam. |
| `hDaemonUser` | — | ✖ **dispensa.** É do serviço, não do banco: quem roda o processo é o `systemd`/o contêiner. |
| `hDebuggingPort` | — | ✖ **dispensa.** Não há depurador de gatilho/procedimento. |
| `hServerLanguage` (`FR`/`US`/`ES`) | `idioma` | ✔ **existe, e mais largo** — seis idiomas (`Portugues`, `Frances`, `Ingles`, `Italiano`, `Alemao`, `Espanhol`), numa tabela de verdade (`phxsys.mensagens`, `docs/MENSAGENS.md`). ✖ **dispensa como diretiva editável pela rede**: não está em `CAMPOS_EDITAVEIS`. Trocar o idioma do servidor pela rede mudaria o texto que o filtro de log de outra pessoa casa. |
| `hWindowsDiskCacheSize` | — | ✖ **dispensa.** É do cache de disco do Windows, e este servidor não fala com ele: a durabilidade daqui se decide por `recursos.durabilidade` (`sempre` / `por_lote` / `nunca`) e pelo `fsync` da janela, que é portátil. |

### 2.3 Diretivas da `Connection`

| HFSQL | PhxSql | Estado |
|---|---|---|
| `Compression` | — | ✖ **dispensa registrada, e a premissa foi MEDIDA.** Não há compressão no fio. E ela **valeria**: uma resposta real de `varrer` com 5.000 linhas mede **535.870 bytes**, e o `deflate` (que já existe aqui, escrito à mão em `phxsql-core/src/zip.rs`, sem crate nenhuma) a leva a **55.284 — 9,69×**, custando **4,02 ms** para comprimir. Numa rede de 10 Mbit/s isso é 429 ms contra 44 ms; num soquete local, onde a mesma varredura leva 3,9 ms, seria puro custo. O que falta não é o compressor: é a **negociação** (o protocolo é uma linha JSON por pedido, e comprimir sem combinar quebra todo cliente antigo — *guarda nova entra pedida*) e o enquadramento, porque dado comprimido não tem `\n` para terminar a linha. Virou pedido, com o número na mão. |
| `Encryption` | a cifra do fio | ✔ **existe** — X25519 + HKDF-SHA256 + ChaCha20-Poly1305, aperto estilo Noise `NX` (`docs/CIFRA-DO-FIO.md`). ✖ **dispensa como `ALTER CONNECTION SET`**: ela se negocia no **aperto de mão** (op `cifrar`), antes de haver sessão para configurar. `SHOW CONNECTION SETTINGS` diz o estado (`encryption`, `encryption_exigida`). |
| `Access` (`hOReadWrite`) | `somente_leitura` | ✔ **existe, mas é do SERVIDOR** e não da conexão. ✖ dispensa por conexão: um cliente que se declarasse somente-leitura não ganharia garantia nenhuma — quem garante é o portão, e o portão já é por permissão de usuário, que é mais forte. |
| `CursorOptions` | — | ✖ **dispensa.** Não há cursor: a resposta é a lista de linhas, com teto (`max_linhas`) e paginação por `offset`. |

### 2.4 Diretivas do banco e das tabelas

| HFSQL | PhxSql | Estado |
|---|---|---|
| `HSetTransaction(t, True/False)` | `BEGIN`/`COMMIT`/`ROLLBACK` | ✔ **existe, e não se liga**: a transação é **por conexão**, sempre disponível, e quem não abre uma não paga nada (`docs/TRANSACOES.md`). ✖ dispensa do interruptor: um banco com transação *desligada* aceitaria `BEGIN` e não daria garantia — a pior das duas respostas. |
| `HSetLog(t, True/False)` | o `.log` da tabela | ✔ **existe, e é sempre ligado.** ✖ **dispensa do desligar**, e o motivo é a integridade: o `.log` é quem responde «quem mudou o quê», é o que a trilha de LGPD lê, é o que a **replicação** transporta e é o que a recuperação usa. Desligá-lo numa tabela replicada pararia a réplica em silêncio. |
| `HSetIntegrity(ligacao, True/False)` | o `verificar` da chave | ✔ **existe na declaração** (`declarar_fk`), e **a chave nasce conferida** — decisão do dono. ✖ **dispensa do `ALTER TABLE … SET referential_integrity`**: ver §4. |
| `HSetDuplicates(t.chave, True/False)` | o `unico` do índice | ✔ **existe na declaração** (`criar_tabela`). ✖ **dispensa do `ALTER TABLE … SET duplicate_check`**: ver §4. |
| `HSetTrigger(t, True/False)` | `CREATE TRIGGER` / `DROP TRIGGER` | ✔ **existe** como criar/excluir; ✖ dispensa do liga-desliga sem excluir: um gatilho desligado é um gatilho que alguém vai esquecer que existe. `SHOW TABLE <t> SETTINGS` conta quantos há. |
| `HSetREP(True/False)` | — | ✖ **dispensa.** O `.REP` é o arquivo de re-indexação do HFSQL; aqui o esquema mora **dentro do `.reg`**, e o índice se refaz por `reindexar`. |
| `HSetReplication(True/False)` | `replicacao.papel` | ✔ **existe** no `config.json` (`isolado`/`source`/`replica`/`spare`), com quatro servidores medidos. ✖ **dispensa como diretiva editável pela rede**: `replicacao.*` está fora do `CAMPOS_EDITAVEIS` de propósito — uma sessão roubada não vira este servidor para outro *source*. |
| `HSetRemoteAccess(True/False)` | `seguranca.ips_permitidos` + `web.servidores` | ✔ **parcial**, por IP e não por tempo; ✖ dispensa do «acesso remoto temporário». |
| `HManageTask(id, True/False)` | `job_ligar` | ✔ **existe** — a tarefa continua cadastrada e para de rodar, exatamente o comportamento do HFSQL (`docs/JOBS.md`). |
| `HActivateServerTrigger` / `HDeactivateServerTrigger` | — | ✖ **dispensa**, mesma razão do `HSetTrigger`. |
| `HNoDatabaseAccess(cnx, "ERP")` | `seguranca.bases_proibidas` | ✔ **parcial**: a base proibida é barrada para todo mundo, root inclusive — mas é **do arquivo**, e não se liga e desliga por comando. ➕ E agora existe o irmão mais fino: `comandos_proibidos` **por banco** (§3). |

**A conta, contada e não lembrada** — e ela já saiu errada uma vez neste
próprio documento, digitada de cabeça («12 existem, 22 dispensadas») quando o
mapa tinha 39 linhas:

```bash
python3 - <<'EOF'
s = open("docs/DIRETIVAS.md").read()
b = s.split("### 2.1")[1].split("**A conta")[0]
l = [x for x in b.split("\n")
     if x.startswith("| ") and "---" not in x and not x.startswith("| HFSQL")]
print("mapeadas      ", len(l))
print("existe (✔)    ", sum("✔" in x for x in l),
      "   dessas com ressalva:", sum("✔" in x and "✖" in x for x in l))
print("dispensa (✖)  ", sum("✖" in x and "✔" not in x for x in l))
print("entrou (➕)    ", sum("➕" in x for x in l))
print("parciais      ", sum("parcial" in x for x in l))
EOF
```

```text
mapeadas       39
existe (✔)     22    dessas com ressalva: 11
dispensa (✖)   17
entrou (➕)     1
parciais       4
```

Ou seja: **22 têm equivalente** — mas **11 delas com ressalva registrada**
(existem no `config.json` e **não** se mudam por diretiva pela rede, cada uma
com o motivo), e **4 são parciais**. **17 não existem** e são dispensa com o
motivo técnico. **1 entrou nesta frente**: `comandos_proibidos` por banco.

---

## 3. A diretiva por banco: `comandos_proibidos` (pedido 220)

O pedido do dono era *«comandos proibidos para um banco x»*. Até esta frente
`politica.comandos_proibidos` proibia **no servidor inteiro** e
`bases_proibidas` proibia **o banco inteiro** — «proibir `reindexar` só em
`financeiro`» não existia.

### Onde a diretiva por banco mora, e por quê

Foram consideradas duas casas:

1. um bloco `databases.<nome>` no `config.json`;
2. um arquivo `diretivas.json` dentro do diretório do banco.

**Ganhou nenhuma das duas: a entrada por banco entra na MESMA lista que já
existe**, `seguranca.comandos_proibidos`, como **objeto** ao lado das strings:

```json
"seguranca": {
  "comandos_proibidos": [
    "excluir_tabela",
    { "comando": "reindexar", "database": "financeiro" }
  ]
}
```

Os três motivos, em ordem de peso:

- **A política é lida no portão, antes de qualquer banco abrir.** O
  `despachar` confere a política antes do token, antes do login, antes de
  resolver nome de base. Um `diretivas.json` dentro do banco obrigaria a abrir
  um arquivo em disco a cada pedido — e a ler, do diretório de dados, a regra
  que protege o diretório de dados. **Quem pode escrever no banco levantaria a
  própria restrição.**
- **Duas listas para a mesma pergunta é a receita de alguém responder
  metade.** A pergunta é uma só: «o que ninguém pede aqui?».
- **Zero migração.** Um `config.json` escrito antes desta rodada continua
  significando exatamente o que significava — string solta é global. É o teste
  que mais importa (`sem_regra_por_banco_nada_muda`), e a bandeira
  `ha_proibidos_por_base` nasce apagada, de modo que quem nunca pediu a guarda
  não paga nem uma trava no caminho quente.

### O global continua valendo; o do banco só APERTA

```sql
ALTER DATABASE erp SET comandos_proibidos = (reindexar) MOTIVO 'auditoria';
```

- **acrescenta**; nunca remove, e nunca toca nas entradas globais;
- **aplica a quente** — aperto que só valesse no próximo arranque deixaria
  aberta justamente a janela em que alguém está fechando a porta;
- **recusa o nome que não existe**: proibir `voar` deixaria a lista com uma
  guarda que nunca fecha e quem escreveu achando que fechou a porta. O
  catálogo inteiro de operações (`catalogo.rs`) é quem sabe os nomes;
- **recusa a lista vazia**, dizendo por onde se retira. Retirar continua sendo
  edição do arquivo, e isso é a guarda e não a falta dela: `seguranca.*` está
  fora do `CAMPOS_EDITAVEIS` porque *uma sessão roubada não esvazia a lista de
  comandos proibidos*. O pior que uma sessão roubada consegue por esta porta é
  deixar o servidor **mais** restrito num banco.

A recusa é `SP000025`, com a mensagem na fábrica de idiomas
(`erro.comando_proibido_na_base`, seis idiomas), e conta como **violação
grave** — o mesmo tratamento do comando proibido global: blacklist, firewall
opcional e e-mail ao administrador quando `alertas.email.avisar_seguranca`
estiver ligado.

---

## 4. O que NÃO entrou no escopo de TABELA, e por quê

`ALTER TABLE <t> SET <campo> = <valor>` **não grava nada**, e a recusa nomeia
o caminho que funciona. Não é esquecimento; são duas decisões:

**`duplicate_check`** é o `unico` do índice, declarado no `criar_tabela`.
Ligá-lo depois não é mudar um campo: é **provar que não há duplicata** varrendo
a tabela inteira, e — se houver — descobrir que o dado gravado contradiz a
declaração nova. Desligá-lo é mais barato, mas assimétrico: um interruptor que
liga fácil num sentido e caro no outro convida a ligá-lo sem entender o preço.
Isso é **migração**, não diretiva, e migração se pede pelo nome.

**`referential_integrity`** é o `verificar` da chave, e aqui a pétrea manda:
*chave declarada NASCE conferida*. `SET referential_integrity = FALSE`
desligaria a conferência de uma chave que **já aceitou linhas sob a garantia** —
e nada no arquivo diria quais linhas entraram com ela e quais sem. O caminho
que existe continua sendo o mesmo de sempre e é uma escolha *escrita*:
`declarar_fk` com `"verificar": false`, na declaração, quando ainda não há dado
confiando nela. E `= TRUE` também não entra: ligar exige provar as linhas que
já estão lá **e** índice dos dois lados, que é o que o `declarar_fk` já faz e
já sabe recusar dizendo qual índice falta.

O que `SHOW TABLE <t> SETTINGS` devolve é a leitura da **declaração** —
`duplicate_check` por índice, `referential_integrity` por chave (com
`ao_excluir` e `ao_alterar`), o journal, os gatilhos, o motivo obrigatório.
Guardar uma cópia disso num bloco de configuração paralelo criaria uma segunda
verdade ao lado do esquema, e a segunda é sempre a que diverge.

---

## 5. O diário administrativo

Toda alteração de diretiva grava uma linha com os **nove campos do dono**, em
JSON Lines, no `diretivas.log`, **ao lado do `acessos.log`**:

```json
{"data_hora":"2026-09-07 18:40:12,345","quando_ms":1788802812345,
 "servidor":"127.0.0.1:5000","banco":"","recurso":"max_linhas",
 "valor_anterior":1000,"valor_novo":500,"usuario":"ana",
 "ip_origem":"192.168.50.20","motivo":"pico de exportacao"}
```

### Arquivo, e não tabela do `phxsys`

A tabela seria mais bonita — a grade já a editaria, o backup já a levaria. Está
errada aqui por três motivos:

1. **O que se audita não pode desligar a auditoria.** A tabela viveria num
   database, e database obedece a `bases_proibidas`, à permissão por base e ao
   `somente_leitura` — todos configuráveis pelo mesmo `ALTER SERVER` que o
   diário existe para registrar.
2. **A configuração muda antes de haver banco.** Um servidor recém-subido tem
   zero databases; gravar a primeira diretiva criaria o `phxsys` como efeito
   colateral de mexer num campo.
3. **O molde da casa já existe** e é o `acessos.log`: legível a olho nu,
   `grep`ável, e sobrevive quando o motor de dados não está utilizável — que é
   exatamente a hora em que alguém vai querer saber quem mexeu na configuração.

### O que ele grava, e o que ele esconde

- **As duas portas.** O diário entra no `config_gravar`, que é onde
  `ALTER SERVER SET` e a tela de Configurações desembocam. Registrar só no
  `ALTER` deixaria de fora a maioria das mudanças de hoje.
- **`valor_anterior` sai do ARQUIVO, antes da gravação.** Ler depois devolveria
  o valor novo; ler da memória viva devolveria o do arranque para os campos que
  só valem no próximo.
- **Segredo nunca aparece.** A regra é por **nome** de campo — qualquer um que
  contenha `token`, `senha`, `password`, `secret` ou `chave_privada` sai como
  `(oculto)`. É por nome, e não por lista de campos, porque lista de segredos
  envelhece calada e o primeiro que faltar nela vaza em texto puro num arquivo
  que ninguém trata como sigiloso. Há teste nos dois sentidos: o sigiloso some,
  o comum sai inteiro.
- **Ele não derruba a gravação.** Um disco cheio tiraria do administrador
  justamente o poder de consertar o servidor. A falha vai para o erro padrão.

`SHOW … SETTINGS` traz as últimas 20 linhas junto, e isso é decisão: a pergunta
que uma pessoa faz ao ver uma diretiva estranha é «quem mexeu nisso?», e diário
que só se lê por outro comando é diário que ninguém lê.

O formato está registrado em [`FORMATO.md`](FORMATO.md) §diretivas.log.

---

## 6. Duas coisas que só apareceram exercitando

**O `SHOW` respondia o valor VIVO, calado.** `ALTER SERVER SET timeout_s = 45`
gravava 45 e o `SHOW SERVER SETTINGS` seguinte devolvia 30 — porque
`configuracao_json` devolve o que está valendo. É o mesmo defeito que a tela de
Configurações já tinha pago uma vez («quem acabou de digitar 90 via 45 de novo
e não tinha como saber que o 90 estava gravado»), **reaparecido pela porta
nova**. Hoje o campo traz `no_arquivo` e `esperando_reinicio` ao lado do valor
vivo, e há teste com controle positivo — o campo que aplica a quente **não**
ganha o par, senão «esperando reinício» perderia o sentido.

**A mensagem `«o campo … nao se grava pela tela»` ficou como está**, embora
agora haja duas portas e não uma. Mudá-la envelheceria três saídas já coladas
(`docs/pdf/respostas/M.md`, o `PENDENCIAS.md` e a página dos pedidos), e o
texto continua verdadeiro: o caminho é o `config.json`. Decisão registrada, não
esquecimento.

---

## 7. Como se refaz

```bash
cargo build --release -p phxsql-server --bin phxsqld
python3 bancada/diretivas/sql.py      # portas 7300-7302
```

O script sobe dois `phxsqld` de verdade, exercita os quatro `SHOW`, os quatro
`ALTER`, confere o `config.json` em disco depois de cada gravação, lê o
`diretivas.log` linha a linha, mede a compressão que não existe e prova que
quem não tem `administrar` é recusado. Os números vão para `resultados-sql.json`.
