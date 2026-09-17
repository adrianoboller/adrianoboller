# Revisão SEC — as saídas de segredo além do `Debug` (17/09/2026)

**Papel:** SEC (revisor adversário), separado do engenheiro e da QA. Não sou o
autor de `74de67e`.
**Pergunta:** o `Debug` derivado fechou em nove structs. **Quantos outros
caminhos de saída existem, e qual está aberto agora?** A pétrea nomeia três —
*arquivo, log, resposta do protocolo* — e o `Debug` era só o mais barato para o
segundo.
**Alcance:** as nove structs (`dblink::Definicao`, `config::{Config, Origem,
Cluster, Email, Rest}`, `usuarios::Usuario`, `sql::usuario::Comando`,
`odbc::Receita`) e **todo lugar por onde o valor delas sai**: `Display`, erro,
`acessos.log`, `perfil.txt`, `diretivas.log`, `.lgpd`, telemetria, `para_json`,
pânico, cópia, o fio (DbLink, réplica, cluster, SMTP, REST, web, ODBC, CLI) e os
arquivos em disco.
**Método:** só leitura, mais `git log -S` para datar. **Nada compilado, nada
subido**, nenhum arquivo além deste. Cada achado traz o trecho lido; achado sem
trecho não entrou.
**Este documento não conserta nada.** Conserto e commit têm outro dono.

---

## 1. Achados priorizados

Gravidade pelo critério do briefing: **alta** = sai em texto puro numa das três
saídas da pétrea; **média** = sai numa saída fora das três; **baixa** = teórico
ou de processo. Na dúvida, o lado mais conservador.

| # | Gravidade | Achado | Quem explora, com que acesso |
|---|---|---|---|
| **A1** | **Alta** | O profiler redige por **nome exato** contra a lista `SEGREDOS`, e **`token_remoto` não está nela**. `dblink_salvar` e `replicacao_testar` levam o token de serviço do OUTRO PhxSql — o portão 1 dele — para o `perfil.txt` (arquivo) e para o anel que a op `profiler` devolve (resposta). O campo nasceu em 03/09; a lista foi retocada em 05/09 e não o ganhou. | Quem tem o direito do profiler, ou lê o `perfil.txt` |
| **A2** | **Alta** | Um job grava o `pedido` **inteiro** em `jobs.json` (com permissão do `umask`) e o devolve inteiro na ficha. A guarda recusa `token` — *«seria senha em arquivo por outro nome»* — e **só** `token`: `senha`, `senha_hash`, `token_remoto`, `prova` passam. Mesma forma do achado do `Debug`: a guarda travou um nome, não a lei. | Administrador que agenda `usuario_alterar`/`dblink_salvar` com a credencial; depois, qualquer usuário local lê o arquivo |
| **A3** | **Média** | O DbLink para outro PhxSql **não tem como pedir o túnel**: a `Definicao` não tem `cifra`/`chave_do_fio`, e `Conexao::abrir` nunca chama `Cliente::cifrar`. O token vai em claro em todo pedido, sempre. A decisão registrada diz «pelo mesmo motivo — a `std` não traz TLS» e foi escrita em **03/09**; o túnel existe desde **30/08** e a réplica já o usa. | Escuta passiva no segmento; e um servidor com `cifra_fio.exigir: true` **não pode** ser alvo de DbLink |
| **A4** | **Média** | `config.json` e `dblink.json` são regravados com `std::fs::write` (permissão do `umask`) **e depois** `set_permissions` — a janela que `gravar_chave` nomeia e fecha para `chave-do-fio.hex`. E `config.json` **herda** a permissão que tinha: um `0644` de instalação fica `0644` com o token dentro, e o arranque não avisa. | Usuário local sem privilégio, no mesmo host |
| **A5** | **Média** | O driver ODBC é o **único** cliente desta casa que manda a senha em **texto puro** (`"senha"` no `login`, a forma 3 do `op_login`); sem `CIFRA=1` ela atravessa o fio em claro. Está registrado em `ODBC.md` como lacuna «quando o driver aprender o desafio-resposta» — sem prazo. A prova de senha já está na `phxsql-core`. | Escuta passiva no segmento |
| **A6** | **Baixa** | Nada desliga o **core dump**: um `SIGSEGV` com `ulimit -c` liberado grava a memória do `phxsqld` — `Config.token`, todas as `Definicao`, todos os `senha_hash` — num arquivo. É o único caminho pelo qual a memória vira uma das três saídas. | Quem provoca a queda e lê o `core` |
| **A7** | **Baixa** | O erro de sintaxe do SQL **ecoa o literal de texto** (`descrever()` devolve `'{t}'`): `… e veio '123.456.789-00'` vai para a resposta e para o `erro` do `acessos.log`. Não é credencial; é dado pessoal num log. | Quem lê o `acessos.log` |
| **A8** | **Baixa** | O teste genérico `nenhuma_credencial_do_config_sai_pela_op_config` só pega segredo que a **fixture** carrega; `rest.token` e `cifra_fio.chave_privada` não estão nela. Há prova específica para os dois — mas o genérico promete o que não cobre. | — (processo) |
| **A9** | **Baixa** | `Origem.senha` diz «existe só para quem ainda não trocou o `config.json`, e o arranque avisa em voz alta». **Não localizei o aviso.** Se existe, falta apontar a linha; se não existe, é o mesmo «comentário que se declara resolvido» da §16. | — (processo) |

**Nenhum dos nove achados é o `Debug` de volta.** Os quatro primeiros são a
mesma lei por outra porta: **lista por nome que envelhece** (A1, A2) e
**conserto que entrou no caminho que o motivou e não no irmão** (A3, A4).

---

## 2. A1 — `token_remoto` passa pelo profiler em claro (ALTA)

### A prova de leitura

A redação é por análise, como a pétrea manda — isso está certo:

```rust
// crates/phxsql-server/src/profiler.rs:875-882
fn analisar_pedido(linha: &str, database: &str) -> (String, Vec<(String, String)>) {
    …
    match Json::analisar(linha) {
        Ok(j @ Json::Objeto(_)) => { … (limpar(&j).escrever(), alvos) }
```

Mas o `limpar` decide **por nome exato** contra uma lista fixa:

```rust
// crates/phxsql-server/src/profiler.rs:131-155
const SEGREDOS: &[&str] = &[
    "senha", "senha_b64", "senha_hash", "nova_senha",
    "senha_banco", "senha_banco_b64", "prova", "token",
    "chave", "chave_privada", "assinatura",
];
// crates/phxsql-server/src/profiler.rs:986
if SEGREDOS.iter().any(|s| k.trim().eq_ignore_ascii_case(s)) {
```

`token_remoto` não está lá. E há **dois** leitores do campo no protocolo:

```rust
// crates/phxsql-server/src/dblink/mod.rs:246-248   (dblink_salvar)
let token_env = j.texto_ou("token_remoto_env", "").trim().to_string();
let token = if token_env.is_empty() { j.texto_ou("token_remoto", "").to_string() } …
// crates/phxsql-server/src/servidor.rs:21993        (replicacao_testar)
let remoto = p.texto_ou("token_remoto", "");
```

O campo se chama assim **de propósito** — o bloco de documentação em
`dblink/mod.rs:135-143` explica que `token` já é o portão 1 deste servidor, e o
`index.html:12777` repete o motivo. O nome novo foi decidido; a lista que redige
por nome não foi revisitada. Datas medidas no `git log -S`:

| o quê | commit | data |
|---|---|---|
| `token_remoto` nasce em `dblink/mod.rs` | `948e153` | 03/09/2026 |
| última alteração da lista `SEGREDOS` (entra `senha_banco`) | `70c5382` | 05/09/2026 |

A lista foi mexida **dois dias depois** de o campo existir, e ainda assim não o
ganhou — o comentário dela (`profiler.rs:136-147`) até diz que «campo que se
esquece de redigir no dia em que alguém passa a mandá-lo custa o segredo, e
custa em silêncio». Custou.

### Por onde sai

1. **Arquivo:** o pedido redigido vai para o anel e para o `perfil.txt`
   (`chegou`, `profiler.rs:686-710`, e `escrever_linha`, `profiler.rs:491-506`).
2. **Resposta do protocolo:** a op `profiler` devolve `e.pedido` com o
   comentário *«já vem redigido»* (`servidor.rs:21601-21603`).

Duas das três saídas da pétrea, para o token que abre a porta de dados do outro
servidor sem usuário nenhum (`dblink/mod.rs:130-133`).

### Cenário

Profiler ligado. Um administrador cadastra uma ligação `phxsql` pela API com
`token_remoto`, ou roda o assistente de replicação (que manda `token_remoto` —
`index.html:12786`, `13139`). Quem tem o direito do profiler pede `{"op":
"profiler"}` e lê o token do outro servidor no campo `pedido`; quem lê o
`perfil.txt` lê o mesmo.

### Teste adverso (unitário, no `profiler.rs`)

```rust
let s = redigir(r#"{"op":"dblink_salvar","nome":"erp","motor":"phxsql","token_remoto":"MARCA-TOKEN-REMOTO"}"#);
assert!(!s.contains("MARCA-TOKEN-REMOTO"), "{s}");
let s = redigir(r#"{"op":"replicacao_testar","host":"h","token_remoto":"MARCA-TOKEN-REMOTO"}"#);
assert!(!s.contains("MARCA-TOKEN-REMOTO"), "{s}");
```

Hoje os dois **falham**. É a prova nos dois sentidos: passa com o nome na lista,
volta a falhar se alguém o tirar.

### Conserto em uma linha (não feito)

Pôr `token_remoto` em `SEGREDOS` hoje — e, para o próximo nome não escapar, um
teste que **cruze os parâmetros do `catalogo.rs`** com a lista: todo parâmetro
cujo nome casa `senha|token|prova|chave|segredo` ou está em `SEGREDOS`, ou está
numa lista visível de falsos positivos (`senha_env`, `token_remoto_env`,
`chave_do_fio`). «Quando um gerador depende de uma lista, a lista sai do código.»

---

## 3. A2 — o job grava a credencial em arquivo, e a guarda fecha um nome só (ALTA)

### A prova de leitura

```rust
// crates/phxsql-server/src/jobs.rs:200-207
// Token no pedido seria senha em arquivo por outro nome -- e o job nao
// precisa dele: ele nao passa pela porta da rede.
if pedido.campo("token").is_some() {
    return Err(PhxError::Esquema(format!("job {nome:?}: o \"pedido\" nao leva \"token\". …")));
}
// crates/phxsql-server/src/jobs.rs:230           p.push(("pedido".to_string(), self.pedido.clone()));
// crates/phxsql-server/src/jobs.rs:240-245       pub fn ficha(&self) -> Json { let mut p = self.pares(); … }
// crates/phxsql-server/src/jobs.rs:516           std::fs::write(&self.caminho, j.escrever_identado())?;
```

A guarda está certa sobre o que diz — e diz um nome. `Job::de_json` aceita
**qualquer** `op` (`jobs.rs:187-216`; `op_job_salvar`, `servidor.rs:5626-5634`,
só confere que o login existe). `dblink_salvar` e `usuario_alterar` são ops
legítimas do protocolo com `senha`/`token_remoto` no corpo. O `jobs.json` é
escrito **sem** permissão (compare com `dblink/mod.rs:581-597`, que ao menos
tenta `0600`), e a ficha devolve o `pedido` inteiro à tela.

### Cenário

`{"op":"job_salvar","job":{"nome":"rotacao","agenda":"0 3 * * *","usuario":"root",
"pedido":{"op":"usuario_alterar","login":"ana","senha":"S3nh@"}}}`. O profiler
tapa `senha` no pedido que chegou (a lista cobre). Mas `jobs.json` fica com
`"senha":"S3nh@"` em claro, `0644` sob `umask 022`, e todo `{"op":"jobs"}` a
devolve. Exige que um administrador ponha credencial num job — e é exatamente
isso que o comentário da linha 200 já reconhece como caminho.

### Teste adverso (unitário, no `jobs.rs`, ao lado de `pedido_com_token_e_recusado`)

```rust
for campo in ["senha", "senha_hash", "token_remoto", "prova"] {
    let j = Json::analisar(&format!(r#"{{"nome":"x","pedido":{{"op":"usuario_alterar","login":"a","{campo}":"MARCA"}}}}"#)).unwrap();
    let r = Job::de_json(&j);
    assert!(r.is_err() || !r.unwrap().para_disco().escrever().contains("MARCA"), "{campo} foi para o disco");
}
```

Hoje **falha** nos quatro.

### Conserto em uma linha (não feito)

A recusa do job usa a **mesma régua** do profiler (`SEGREDOS`, em qualquer
profundidade) em vez de um nome; e `jobs.json` nasce `0600` como a chave do fio.

---

## 4. A3 — o DbLink para PhxSql não alcança o túnel que a réplica já usa (MÉDIA)

### A prova de leitura

A `Definicao` não tem campo para pedir cifra (`dblink/mod.rs:115-154`: `nome,
motor, host, porta, usuario, senha, senha_env, token, token_env, database,
descricao, somente_leitura, timeout_s, max_linhas, sincronias`). A `Origem` tem
(`config.rs:178-184`: `cifra`, `chave_do_fio`). O cliente é o mesmo:

```rust
// crates/phxsql-server/src/dblink/phx.rs:68        (DbLink)
let mut cliente = Cliente::conectar(host, porta, token, espera)?;
// crates/phxsql-server/src/replica.rs:416-419      (réplica)
// O tunel ANTES do login, de proposito: e a prova do desafio-resposta e o
// token que ele existe para esconder, e depois do login ja seria tarde.
if origem.cifra { c.cifrar(origem.pino_do_fio()?)?; }
```

E o token vai em todo pedido: `replica.rs:159-160` (`campos.push(("token", …))`).
A decisão registrada:

```text
// crates/phxsql-server/src/dblink/phx.rs:35-39
// # O limite honesto: o fio vai em claro
// Igual aos outros dois motores, e pelo mesmo motivo (a `std` nao traz TLS).
// A SENHA nunca viaja -- o desafio-resposta cuida disso --, mas o DADO
// devolvido sim, e o TOKEN vai no pedido. Rede interna, VPN ou tunel.
```

(`docs/DBLINK.md:648` repete.) Datas: `Cliente::cifrar` nasce em `d3b7d62`
(**30/08**); o texto acima é de `948e153` (**03/09**). O motivo «a `std` não traz
TLS» já não valia quando foi escrito: a casa tinha o Noise `NK` próprio há
quatro dias.

### Por que é média e não alta

O fio em claro é decisão registrada da casa (`CIFRA-DO-FIO.md:152`, `480`:
`exigir: false` é o padrão pétreo). O que este achado aponta é outra coisa:
**a réplica pode escolher e o DbLink não**. E uma consequência funcional que é
de segurança: um servidor que ligou `cifra_fio.exigir: true` **recusa** todo
DbLink que aponte para ele — quem quiser a ligação vai desligar o `exigir`.

### Teste adverso (soquete)

Servidor B com `cifra_fio.exigir: true`. Em A, `dblink_salvar` para B com um
campo novo `"cifra": true` e o pino de B, depois `dblink_testar`. Hoje não há
campo, e sem ele B responde a recusa de texto claro (`servidor.rs:8770-8772`).

### Conserto em uma linha (não feito)

`Definicao` ganha `cifra` e `chave_do_fio` na forma da `Origem`, e
`Conexao::abrir` chama `cliente.cifrar(pino)` **antes** de `autenticar` — as
mesmas quatro linhas de `replica.rs:416-419`. E o parágrafo do `phx.rs` troca
«a `std` não traz TLS» pelo motivo verdadeiro, seja ele qual for.

---

## 5. A4 — a janela de permissão que a casa já nomeou, nos dois irmãos (MÉDIA)

### A prova de leitura

A regra está escrita na própria casa, com o motivo:

```rust
// crates/phxsql-server/src/config.rs:1812-1827
/// A permissao e posta na CRIACAO, e nao depois: entre criar aberto e apertar
/// ha uma janela em que qualquer um le a chave, e essa janela e a unica coisa
/// que este arquivo existe para nao ter.
fn gravar_chave(caminho: &Path, chave: &[u8; 32]) -> std::io::Result<()> {
    opcoes.write(true).create_new(true);  … opcoes.mode(0o600);
```

E os dois irmãos fazem o contrário — escrevem primeiro, apertam depois, e
engolem a falha do aperto:

```rust
// crates/phxsql-server/src/config.rs:4492-4499      (config.json: token, hashes)
std::fs::write(&temporario, corpo) …
if let Ok(meta) = std::fs::metadata(caminho) {
    let _ = std::fs::set_permissions(&temporario, meta.permissions());
}
// crates/phxsql-server/src/dblink/mod.rs:590-597    (dblink.json: senha, token_remoto)
std::fs::write(&temporario, j.escrever_identado()) …
let _ = std::fs::set_permissions(&temporario, std::fs::Permissions::from_mode(0o600));
```

Datas: o `gravar` do `dblink.json` é de `c11629b` (28/08); a lição do
`create_new + mode(0o600)` é de `d3b7d62` (30/08) e **não voltou** aos irmãos.
Dois efeitos:

1. **A janela.** Sob `umask 022`, `config.tmp` e `dblink.tmp` nascem `0644` com
   o segredo dentro, até o `set_permissions`. Curta — mas é a mesma janela que
   a casa disse que «é a única coisa que este arquivo existe para não ter».
2. **A herança.** `config.json` copia a permissão do original. Instalação que
   criou o arquivo `0644` e nunca apertou fica `0644` para sempre, com o token
   e os hashes; **não há aviso de arranque** (procurei `legivel por outros`,
   `umask`, `0o644` em `config.rs`: só o teste da chave do fio,
   `config.rs:4706-4708`). O `.lgpd` nasce `0600` (`trilha.rs:75-77`), a chave
   do fio nasce `0600` e tem teste; o arquivo com o portão 1 não tem nem um
   nem outro.

### Cenário

Usuário local sem privilégio: `cat config.json` quando ele é `0644` — sem
corrida nenhuma. Ou, com o arquivo `0600`, `inotifywait -e create` na pasta e
`cat config.tmp` no instante da regravação (todo `usuario_criar`, `ALTER
SERVER`, `dblink_salvar` regrava).

### Teste adverso (contra o sistema operacional, não unitário)

`umask 022`; `config.json` em `0600`; disparar `usuario_criar` e observar, com
`strace -e openat`, que o `openat` do `.tmp` sai com `O_CREAT` e modo `0666`
mascarado — hoje sai. Ou o mais simples: gravar num caminho **novo** (sem
original para herdar) e `stat -c %a`: hoje `644`, e devia ser `600`.

### Conserto em uma linha (não feito)

Extrair `gravar_chave` num `gravar_privado(caminho, bytes)` e usá-lo nos três
(`config.json`, `dblink.json`, `jobs.json`); e avisar no arranque quando
`config.json` for legível por grupo ou outros.

---

## 6. A5 — o ODBC é o único cliente que manda a senha em texto puro (MÉDIA, registrada como lacuna)

### A prova de leitura

```rust
// crates/phxsql-odbc/src/conexao.rs:289
("senha", Json::texto_de(&r.senha)),
// crates/phxsql-server/src/servidor.rs:9303-9309
/// 1. `prova` + `nonce_cliente` -- desafio-resposta. A senha nao sai da maquina do cliente.
/// 2. `senha_b64` -- Base64. … NAO e cifra.
/// 3. `senha` -- texto puro.
```

Todos os outros clientes desta casa fazem a forma 1: a interface
(`ui/index.html:2596-2601`), o `phxsqlcmd` (`phxsql-cmd/src/lib.rs:81-85`), a
réplica (`replica.rs:207-236`), o DbLink (`dblink/phx.rs:74`). O driver faz a
forma 3, e o túnel é opcional (`conexao.rs:39-42`: «Sem elas o driver fala em
claro»). `docs/ODBC.md:79-83` registra: *«O login do driver é a senha em claro
DENTRO do túnel, e não o desafio-resposta … quando o driver aprender o
desafio-resposta.»*

É decisão registrada — **como lacuna, sem prazo**. Responde à pergunta do
briefing («é achado ou é decisão registrada?») com as duas coisas: registrada,
e ainda assim a única senha de usuário que atravessa o fio em texto puro. A
`prova_de_senha` está em `phxsql_core::desafio`, que o driver já enlaça.

### Conserto em uma linha (não feito)

`conexao.rs` pede `desafio` e manda `prova` + `nonce_cliente`, como
`replica.rs:231-236` — sem dependência nova.

---

## 7. As baixas, em resumo

**A6 — core dump.** Zero ocorrências de `RLIMIT_CORE`, `PR_SET_DUMPABLE` ou
`dumpable` em `crates/` (grep). Teste, contra o SO: `ulimit -c unlimited`, subir
`phxsqld` com `token` marcado, `kill -SEGV`, `strings core | grep MARCA`.
Conserto: `setrlimit(RLIMIT_CORE, 0)` no arranque por `extern "C"` (não é
crate), ou `LimitCORE=0` na unit documentada em `EMPACOTAMENTO.md` — e a
decisão escrita.

**A7 — o literal ecoado.** `lexico.rs:97` (`Token::Texto(t) => format!("'{t}'")`),
usado em `sintaxe.rs:558` («… e veio {:?}») e `usuario.rs:257` («sobrou {:?}»).
O `exigir_senha` já **não** ecoa de propósito (`usuario.rs:235-241`) — a lição
existe e parou num lugar. O texto vai para `Acesso.erro` (`acesso.rs:78-81`).
Conserto: `descrever()` de `Token::Texto` devolve `'…'` nas mensagens; o
`normalizar` (`lexico.rs:120-128`) é outra função e não muda.

**A8 — a fixture do genérico.** `config.rs:4875-4907` lista dez marcas; não há
`rest` nem `cifra_fio.chave_privada`. As provas específicas existem
(`config.rs:5359-5374`; `a_privada_do_fio_nunca_sai`, citada em `SEGURANCA.md`
§16.4). Conserto: as duas marcas entram na fixture.

**A9 — o aviso prometido.** `config.rs:164-165`. Procurei `senha em claro`,
`avisos.push(…senha…)` em `config.rs`, `servidor.rs`, `replica.rs`: só
comentários e a marca do teste. Conserto: apontar a linha do aviso, ou tirar a
frase.

---

## 8. A cópia (`Clone`) — parecer: a pétrea NÃO deve alcançar a memória

As nove derivam `Clone` (`dblink/mod.rs:114`, `config.rs:147, 402, 1109, 2039,
2889`, `usuarios.rs:676`, `sql/usuario.rs:48`, `odbc/conexao.rs:43`). Os clones
vivos que li: `Sessao.usuario: Option<Usuario>` (`servidor.rs:318`, clonado em
`9399`) — um `senha_hash` por conexão, embora só o login precise dele; e
`Registro.achar().clone()` por operação de DbLink (`servidor.rs:20069-20073`).
Não há zeroização em `Drop` em lugar nenhum (grep `zeroize|write_volatile|impl
Drop for Segredo|Chave|Iniciador|Transporte`: zero).

**Recomendo não estender a pétrea à memória**, e registrar o motivo:

1. A pétrea nomeia **saídas**. Memória não é saída — vira saída por um caminho
   só, o core dump, e é esse caminho que se fecha (A6).
2. Com zero dependências e `String` da `std`, zeroizar em `Drop` seria garantia
   falsa: `move`, `realloc` e `clone` deixam cópias que o `Drop` não vê. Uma
   regra que não se consegue cumprir é regra que se ignora.
3. O que vale a pena, e não é pétrea: a `Sessao` guardar a ficha **sem** o
   hash depois do login. É higiene, não portão.

---

## 9. O que auditei e está fechado

Papel que cumpre aparece cumprindo. Cada linha tem a prova lida.

| # | caminho | estado | prova |
|---|---|---|---|
| 1 | `Display` | fechado | 4 `impl Display` no repositório (`uuid.rs:356, 371`, `error.rs:342`, `integridade.rs:100`); nenhum nas nove. `Violacao` imprime `tabela.chave` = nome de coluna |
| 2 | `format!`/`write!` perto de segredo | fechado | grep em `crates/`: nenhuma interpolação de **valor** de segredo fora de teste. `ensinar_onde_vai_o_token` só testa vazio (`phx.rs:123-124`); `sem_a_senha` → `<comando invalido, N bytes>` (`usuario.rs:167`); léxico e JSON erram por posição (`lexico.rs:506-507`, `json.rs:553-554`); SMTP devolve só a resposta do servidor (`email.rs:95-97`); MySQL só `{usuario:?}` (`mysql.rs:195`); ODBC `Falha` nunca leva o pedido (grep `Falha::nova(…pedido`: zero) e `receita_mascarada` tapa `Token`/`PWD` (`conexao.rs:166-188`) |
| 3 | `acessos.log` | fechado | `Acesso` não tem corpo (`acesso.rs:25-51`); `servidor.rs:4670` |
| 4 | `diretivas.log` | fechado | máscara por nome (`diretivas.rs:121-126`), credenciais fora de `CAMPOS_EDITAVEIS` (`diretivas.rs:28-35`, `config.rs:4052`). É estrutural, não recorte |
| 5 | trilha `.lgpd` | fechado | nasce `0600`, mesma cifra e interruptor dos outros diários, e tem bit de redação (`trilha.rs:75-77, 115, 192`). Não vê credencial: usuário mora no `config.json`, não em tabela |
| 6 | telemetria | fechado | conta pedidos e fases; nenhum corpo (`telemetria.rs`, grep `pedido|corpo|redigir`) |
| 7 | `para_json`/fichas | fechado (ver A8) | `Definicao::para_json` (`dblink/mod.rs:318-356`) + teste; `Config::para_json` (`config.rs:3687`, origens sem senha/hash `3745-3747`) + genérico; `Cluster` (`631-672`, só `tem_pino`); `Rest` → `token_proprio` (`2154`) + teste `5359`; `CifraFio` nunca a privada (`1779-1788`); `Usuario::ficha` (`947-959`) + teste `1844`; `Alteracao` mascara; `Ligacao` sem segredo |
| 8 | pânico | fechado | zero `expect(&format!` em `src/`; `panic!("{…:?}")` só em `#[cfg(test)]`; zero `dbg!(` em `src/` |
| 9 | structs com segredo **sem** `Debug` | fechado | `Sessao` (`servidor.rs:316`), `replica::Cliente` (`60`), `dblink::phx::Conexao` (`51`), `fio::{Simetrico, Iniciador, Direcao}` (`74, 170, 321`), `cofre::Segredo` (`140`), `email::Sessao` (`135`): nenhuma deriva `Debug` (grep de `#[derive` na linha anterior) |
| 10 | MCP | fechado | token carimbado pela ponte e não sobrescrevível pelo argumento (`mcp.rs:98-124`, teste `728-741`); senha por `PHXSQL_SENHA`, nunca por argumento (`main.rs:40-48`) |
| 11 | interface web | fechado | token e senha só em memória (`index.html:2023` «NÃO são guardados», `2124`); login por desafio-resposta (`2596-2601`); `localStorage` só para rótulos, posições, idioma e a chave do console Claude (do visitante) |
| 12 | `phxsqlcmd` | fechado | desafio-resposta (`lib.rs:81-85`); `--senha` avisa do `ps` (`main.rs:64`) |
| 13 | réplica e cluster | fechado | desafio-resposta, hash nunca senha (`replica.rs:23-29, 225-229`); túnel **antes** do login (`416-419`); cluster idem por `Origem` |
| 14 | DbLink → MySQL / PostgreSQL | fechado (registrado) | senha nunca em texto: `mysql_native_password`/`caching_sha2` rápido, `mysql_clear_password` recusado (`mysql.rs:20-34, 449-453`, teste `684`); PG só `SCRAM-SHA-256`, `md5` recusado (`pg/mod.rs:42-47, 198-210`) |
| 15 | REST | fechado (registrado) | `Bearer` em claro dito na especificação e provado (`rest.rs:620-621`, teste `1128`); token nunca na especificação (teste `1145`); sem query string (grep `query|'?'`: zero) |
| 16 | `chave-do-fio.hex` | fechado | `0600` na criação + teste (`config.rs:1812-1829, 4706-4708`) |
| 17 | CLI `--senha` / `--gerar-chave` | fechado (decisão) | a privada Ed25519 sai **uma** vez no stdout com o aviso (`main.rs:57-64`); a senha vem do stdin (`74-99`) |
| 18 | resposta do `dblink_salvar` | fechado | devolve `d.para_json()` (`servidor.rs:20044-20049`) — `(oculto)`/`(do ambiente)` |
| 19 | `Comando::pedido()` emite `senha` | fechado (por desenho) | é o pedido interno para o `despachar` (`usuario.rs:82-88`); vira hash em `aplicar_na_arvore` antes de qualquer serialização (`servidor.rs:4667-4672`) |

**Contagem:** 19 caminhos auditados; **9 achados** (2 altas, 3 médias, 4
baixas); os outros 10 fechados com prova, 4 deles por decisão registrada.

---

## 10. Decisões registradas que NÃO entram como achado

- **Fio em claro por padrão** (`exigir: false`) — `CIFRA-DO-FIO.md:152, 480`;
  «guarda nova entra pedida, não imposta».
- **`config.json` e `dblink.json` com segredo em claro**, com a alternativa
  `_env` para cada um — `dblink/mod.rs:283-307`, `config.rs:1777`.
- **SMTP `AUTH LOGIN` em base64, sem `STARTTLS`** — `email.rs:11-23`, dito com
  todas as letras («codificação, não cifra, e qualquer um no caminho lê»).
- **Porta web em HTTP** — `servidor.rs:352-353`; o proxy põe o TLS
  (`SEGURANCA.md:715`).
- **`--gerar-chave` imprime a privada** — uma vez, por desenho (`main.rs:57`).

---

## 11. O que o briefing dizia errado

Briefing de orquestrador também é número citado. Conferido no fonte:

1. **«nove structs em quatro crates».** São **três**: `phxsql-odbc`,
   `phxsql-server`, `phxsql-sql` — cinco arquivos (`git show 74de67e --stat`).
2. **«uma ligação DbLink sem [a cifra] manda a senha em claro».** Não manda: a
   senha **nunca** viaja para outro PhxSql (`phx.rs:18, 74` — desafio-resposta),
   nem para MySQL (embaralhado), nem para PostgreSQL (SCRAM). O que viaja em
   claro é o **token** — e a pergunta certa era por que a `Definicao` não tem o
   campo `cifra` que a `Origem` tem (A3).
3. **«a réplica manda imagem da linha».** A réplica **puxa** (`config.rs:146`
   «De onde a replica puxa os eventos»); quem manda é o source. Detalhe de
   sentido, mas é o sentido que diz de que lado se liga o túnel.
4. **«O profiler já redige analisando».** Verdadeiro no **método** (`analisar_
   pedido`), falso na **cobertura**: a lista é por nome exato e `token_remoto`
   não está nela (A1). «Redige analisando» e «redige tudo» não são a mesma
   frase.
5. **«Há o teste genérico … que pega campo novo no `Config`».** Pega
   serialização nova de um segredo que a **fixture** já carrega. Segredo novo
   fora da fixture (`rest.token`, `cifra_fio.chave_privada`) passa (A8).
6. **«as outras oito têm equivalente?»** — pergunta errada: das nove, só
   `Definicao`, `Config` (e as quatro seções por composição) e `Usuario`
   serializam para fora. `Comando::pedido()` emite a senha **de propósito**
   (interno) e `Receita` não tem `para_json` — tem `receita_mascarada`, que é o
   equivalente e está certo.
7. **«O log de acessos, o profiler e o diário LGPD — o que cada um serializa
   quando o pedido contém segredo».** Só o profiler serializa pedido. O
   `acessos.log` não tem corpo; o `.lgpd` vê valor de coluna, não pedido. O
   terceiro que **deveria** estar na lista é o `diretivas.log` — e ele está
   certo.
8. **«`expect("... {:?}") com argumento formatado antes ainda vaza»** — o
   risco é real, a contagem é **zero** em `src/`.

---

## 12. Os números

- **19** caminhos auditados; **9** achados: **2 altas** (A1, A2), **3 médias**
  (A3, A4, A5), **4 baixas** (A6–A9).
- **0** achados no `Debug`: o conserto de `74de67e` está inteiro.
- **4** dos 9 são a mesma lei da §16 por outra porta: lista por nome que
  envelhece (A1, A2) e conserto que não voltou ao irmão (A3, A4).
- **O pior, em uma linha:** com o profiler ligado, cadastrar uma ligação
  `phxsql` ou testar uma origem de replicação escreve o **token de serviço do
  outro servidor** em texto puro no `perfil.txt` e o devolve pela op
  `profiler`, porque `token_remoto` — nome escolhido de propósito — nunca
  entrou na lista que redige por nome.
