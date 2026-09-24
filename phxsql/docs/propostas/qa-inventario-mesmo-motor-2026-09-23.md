# QA — inventário guarda × pétrea: «função e comando não se duplicam»

Papel G. Só leitura + grep/python. Nada consertado, nada afrouxado — só
nomeado. Comandos usados vão ao lado de cada número.

## 1. Família 1 — leitura de linha/bytes de soquete fora do `Canal`

Comando base (depois filtrado à mão por arquivo/módulo):

```
grep -rn '\.read_line(\|\.read_until(\|\.lines()\|\.read_to_end(\|\.read_to_string(' crates --include=*.rs
grep -rn 'BufReader::new\|BufRead\b' crates --include=*.rs
grep -rln 'TcpStream' crates/*/src
```

**Número medido: 1, não 0.**

O `TETO_DO_APERTO`/`TETO_DO_REGISTRO` do `fio.rs` fecharam os cinco sítios do
pedido 434 (aperto de mão dos três clientes + porta de dados anônima +
`http.rs`). Conferido um a um nesta rodada — `Remoto::abrir`/laço de conexão
(`servidor.rs:512-539,9540-9579`), `replica::Cliente` (`replica.rs:123`),
`Canal::abrir`/`cifrar`/`pedir` do ODBC (`phxsql-odbc/src/conexao.rs:310-593`)
e `http::ler_pedido` (`http.rs:171-208`, que hoje chama
`Canal::ler_ate(&mut leitor, MAX_CABECALHO as u64)`) — **todos os quatro
consumidores do fio de PhxSql passam pelo `Canal`. Zero ali.**

O `1` que sobrou é outro:

| arquivo:linha | o quê | soquete/arquivo | protocolo |
|---|---|---|---|
| `crates/phxsql-server/src/email.rs:173` | `self.leitor.read_line(&mut linha)` dentro de `Sessao::esperar` | soquete (`BufReader<TcpStream>`) | SMTP, para o relé de e-mail |

Não é o mesmo motor porque **não é a mesma pergunta**: `Canal` fala o fio do
PhxSql (Noise + moldura `Tipo::Pedido/Fim` + Base64), e um relé SMTP não faz
aperto de mão Noise nenhum — `esperar` lê `220`/`250`/`354` de RFC 5321.
Empurrar isso para dentro do `Canal` não uniria uma decisão duplicada; criaria
uma dependência sem sentido (`Canal::ler` devolveria `Recebido::Linha`/`Fim`
para um protocolo que não tem "Fim").

**O que não é a mesma pergunta ainda tem o mesmo risco que motivou o 434**: o
`read_line` do `email.rs:173` **não tem teto nenhum** — só timeout de tempo
(`set_read_timeout`, linhas 78-80), não de tamanho. Um relé comprometido (ou um
MITM na rede interna que a doc do módulo já admite não defender, por não haver
TLS) escolhe quanta memória este lado reserva numa linha de resposta SMTP,
exatamente a forma do defeito do 434 — só que num motor diferente do `Canal`,
porque este não é sequer candidato: não guarda o `Tipo::Fim`, não faz Noise,
não fecha com `fim_recebido`.

Achado à parte (não é duplicação, é ausência de teto num protocolo que nunca
teve um): nomeado, não corrigido — corrigir é do papel B/C, não do G.

**A armadilha que o próprio pedido já pagou, reencontrada:** quatro leituras
que PARECEM produção e não são, todas dentro de módulo `#[cfg(test)]` (não de
diretório `tests/`):

| arquivo:linha | módulo | o que finge ser |
|---|---|---|
| `phxsql-odbc/src/conexao.rs:829,909,987,1131` | `mod testes` (`#[cfg(test)]` na linha 596) | quatro "servidores de mentira" que são o OUTRO lado do fio no teste do aperto/login |
| `phxsql-odbc/src/lib.rs:1887` | `mod testes` (`#[cfg(test)]` na linha 1686) | `servidor_de_eco`, mesmo papel |
| `phxsql-server/src/apoio_teste.rs:83,90` | módulo inteiro `#[cfg(test)] mod apoio_teste;` (`lib.rs:8-9`) | `rele_falso`, o SMTP de mentira que prova o `email.rs` |
| `phxsql-server/src/servidor.rs:48487,48698,48970` | três `mod testes_*` (`#[cfg(test)]` nas linhas 48429/48429/48875) | clientes de teste conectando no servidor de verdade e lendo o que atravessou o fio |

Todos ficam em `src/`, nenhum em `tests/`. Confirmado pela busca do delimitador
de módulo (`grep -n '^#\[cfg(test)\]$'`) e checagem visual do trecho seguinte —
não pelo nome do arquivo, como o pedido avisou.

**`http.rs:961,979`** (`.lines()`) também é isca: são dois testes dentro de
`#[cfg(test)] mod testes_da_claude` conferindo o cabeçalho `Content-Security-
Policy` de uma `String` já montada em memória — nem soquete, nem produção.

**`mcp.rs:401`** (`entrada.lines()`, dentro de `pub fn servir`) é produção de
verdade, mas **não é soquete**: `main.rs:180` chama `servir(&ponte,
std::io::stdin().lock(), ...)` — é `stdin`, o transporte é um cano/pipe do
processo, não um `TcpStream`. O `/mcp` por HTTP (`servidor.rs:8650`) não lê
socket cru: reaproveita `pedido.corpo`, que já veio de `http::ler_pedido`
(Canal). Fora da família por definição do pedido («leitura de arquivo não é
desta família» — pipe também não é).

**Vizinho que quase entra e não entra:** `servidor.rs:25894`,
`descartar_ate_a_quebra<L: BufRead>`, opera sobre o mesmo
`BufReader<TcpStream>` do laço de conexão, fora do `Canal`, e por isso parece
candidato. Não é: usa `fill_buf`/`consume`, não uma das seis formas listadas, e
o comentário do próprio código (linhas 25888-25892) diz por quê — ele existe
para **drenar sem acumular** até o `TETO_DO_REGISTRO`, depois de o `Canal` já
ter recusado a linha por estourar o teto; a pergunta é «quanto descartar antes
de fechar sem gerar RST», não «qual é a próxima linha do protocolo». Nomeado
para quem for construir a catraca não tropeçar nele.

## 2. As outras famílias, com o limite aplicado

### Tetos de tamanho (`TETO_*`, `MAX_*`)

```
grep -rn '^pub const TETO_\|^const TETO_\|^pub const MAX_\|^const MAX_' crates --include=*.rs
```

34 constantes achadas, em domínios que não se tocam: teto de linha do fio
(`fio.rs`), fsync por fecho, profundidade de cascata, saída do HKDF, campos do
PIX, fila de saúde do disco, oito catracas de QA (`TETO_TABELA_NA_MAO`,
`TETO_SEGREDO_SOLTO`, etc. — cada uma com o defeito próprio no comentário),
truncamento de campo do profiler, cabeçalho/corpo HTTP, lote servido,
pivot/junção/aninhamento de consulta. Nenhum par responde à mesma pergunta.

O caso que mais parecia duplicação **não é**: `http.rs`'s `MAX_CABECALHO`
(16 KiB) não é um segundo teto de linha ao lado do `Canal` — é o **valor** que
alimenta o único teto que o `Canal` tem: `canal.ler_ate(&mut leitor,
MAX_CABECALHO as u64)` (`http.rs:171-186`). É exatamente o desenho que a pétrea
pede — reuso do mecanismo único, parametrizado por quem chama — e o comentário
acima da função (linhas 145-165) já registra que foi um conserto do 434.
`MAX_CORPO` é outra pergunta (tamanho do corpo já declarado por
`Content-Length`, lido por `read_exact`, não por linha) — teto irmão,
decisão diferente, comentado como tal.

**Veredito: nenhuma duplicação. Não se mecaniza catraca aqui** — não há regra
possível além de "toda constante nova documenta a pergunta que responde", que
já é a prática observada em cada um dos 34 sítios.

### Padrões de cifra (`CIFRA_DE_SAIDA_PADRAO` e irmãos)

```
grep -rn 'CIFRA_DE_SAIDA_PADRAO' crates --include=*.rs
```

Uma declaração (`config.rs:209`), sete referências, todas
`crate::config::CIFRA_DE_SAIDA_PADRAO` — nenhuma reescreve `true`/`false` a
mão. `servidor.rs:25778` documenta o motivo em voz alta: *"o padrão é um só
porque padrão que mora em dois lugares diverge nos dois"* — ou seja, esta
frente **já pagou** a lição que a pétrea de hoje generaliza. **Já unificado,
sem achado.**

### Leitura de variável de ambiente para segredo (`*_env`)

```
grep -rn 'Segredo::ler' crates/phxsql-server/src --include=*.rs
grep -rn 'std::env::var' crates --include=*.rs
```

`Segredo::ler` (`config.rs:1310`) é, no próprio comentário, *"a ÚNICA função
que decide de onde o segredo vem"*. Cinco chamadas — `alertas.email.senha`,
`cifra.senha`, `cifra_fio.chave_privada`, `dblink.senha`,
`dblink.token_remoto` — batendo com os *"cinco donos do pedido 372"* já
documentados em `config.rs:1681,2083`. **Não achei um sexto.** A frente 372
está corrigindo esses quatro-cinco sítios agora; não conto nenhum como achado
meu, e não há um adicional fora da lista dela.

Achei dois `std::env::var("PHXSQL_SENHA")` **fora** desse mecanismo —
`phxsql-server/src/main.rs:144` (`phxsqld --mcp --usuario`) e
`phxsql-cmd/src/main.rs:65` (`phxsqlcmd`). **Não são a mesma pergunta que o
`Segredo::ler` responde** (aquele lê o NOME da variável a partir de um campo
`campo_env` do `config.json`; estes leem uma variável de nome FIXO para
credencial de operador na linha de comando, análogo ao `--senha` que aparece
no `ps`). E as duas implementações **já divergem por necessidade**: o
`phxsqld --mcp` é modo não-interativo e falha se a variável faltar; o
`phxsqlcmd` é console interativo e cai para `--senha` (com aviso) e depois para
prompt de terminal. Forçá-las a convergir quebraria o modo não-interativo (que
não pode pedir senha no terminal) ou tiraria o prompt do console. **Pergunta
diferente. Não é duplicação — revisado e descartado.**

### Mecanismos de aviso de arranque

```
grep -n 'avisos: &mut Vec<String>\|\.avisos\.\(push\|extend\)\|config\.avisos\|c\.avisos\|cadastro\.avisos' crates/phxsql-server/src/config.rs crates/phxsql-server/src/main.rs
```

Um único acumulador: `Config.avisos: Vec<String>`, alimentado por
`Painel::de_json`, `usuarios::de_json`/`extrair_hash` e
`dblink::Cadastro::avisos()` via `.extend(...)` dentro de `Config::ler`
(`config.rs:3818-4196`). Três pontos em `main.rs` (linhas 277, 410, 470)
imprimem esse mesmo vetor com `for aviso in &config.avisos { eprintln!("AVISO:
{aviso}"); }` — repetido porque são três ramos de saída antecipada da MESMA
`fn main` (`--chave-do-fio`, `--usuarios`, arranque normal), não três decisões
diferentes. É repetição de uma linha de formatação, não de uma lógica que possa
divergir — o `--mcp` (linha 306-307) roda DEPOIS da impressão de `config.avisos`
na linha 277, então o caminho MCP também vê os avisos de config antes de
desviar. **Já unificado onde importa (a decisão de o que é aviso); a impressão
repetida é estilística e de baixo risco — não proponho catraca aqui.**

Fora do escopo desta pergunta (é outra família, "aviso de arranque" ≠ "alerta
de operação"): `saude_do_disco::anotar_aviso`,
`servidor.rs::avisos_do_cluster/texto_do_aviso_de_falha/texto_do_aviso_de_parado/texto_do_aviso_de_saude`
e `cluster::tomar_aviso_de_promocao` constroem texto de ALERTA de runtime (para
e-mail e para a tela), não aviso de leitura de `config.json` no boot. Pergunta
diferente da que foi pedida; não julguei duplicação interna entre eles porque
não foi isso que se pediu, e nomear sem medir seria o mesmo erro que a lei
proíbe.

## 3. A proposta de catraca

### Para a família 1: proponha, não implemente

**Nome:** `TETO_LEITURA_FORA_DO_CANAL`
**Valor de hoje:** **1** (não 0 — medido, não esperado).
**Onde morar:** `crates/phxsql-server/src/conferidor_canal.rs`, novo arquivo,
no molde de `conferidor_segredos.rs`/`conferidor_temporarios.rs` (que já
varrem código-fonte do repositório com um `pub fn conferir() -> Vec<Achado>` +
`pub const TETO_...` + `mod testes` no fim do próprio arquivo, rodando dentro
de `cargo test --workspace`) — não em `bancada/guardas/`, porque aquela pasta
prova guardas por MUTAÇÃO (repor o defeito e rodar `cargo test`), e o que se
quer aqui é uma varredura estática de padrão de texto, que é o gênero dos
`conferidor_*` do crate, não do `provar-guardas.py`.

**O obstáculo que nenhum `conferidor_*` de hoje resolveu ainda:** todos os
oito `conferidor_*` existentes varrem `FONTES` — HTML/JS embutido por
`include_str!` — ou arquivos de texto simples (`.md`, `.json`). **Nenhum varre
`.rs` distinguindo módulo de produção de `#[cfg(test)]`.** Um contador de
chaves para achar o limite do `mod testes { ... }` é frágil justamente NESTE
repositório: o próprio `fio.rs` tem testes com string bruta carregando `{` e
`}` dentro de JSON (`br#"{"op":"ping"}"#`, linha 928/935), e um contador ingênuo
de chaves conta as de dentro da string também — pode fechar o módulo de teste
cedo demais (marcando produção real como teste, escondendo um achado de
verdade) ou tarde demais (marcando um mock de teste como produção, reprovando
código que sempre esteve certo — o exato estrago que "guarda pedida não
imposta" veda).

**Proposta de implementação que não tropeça nisso:** não reimplementar um
parser de módulo. Em vez disso, casar a lição de `conferidor_segredos.rs`
(`ISENTOS`, motivo escrito) com uma marca textual que ESTE código-base já usa
por convenção em todo sítio de teste-com-soquete-cru que encontrei — cada um
já tem um comentário nomeando por que lê cru (`"servidores de MENTIRA"`,
`"Teste unitario nao prova o que viaja no fio"`, `"teste unitario NAO prova
entrega de e-mail"`). O conferidor soma **toda** ocorrência de
`read_line`/`read_until`/`.lines()`/`read_to_end`/`read_to_string` num raio
textual curto de um identificador que contenha `soquete`/`fluxo`/`leitor` +
`TcpStream`, e subtrai as que carregam, na função-mãe, uma marca allow-listada
por arquivo:linha (uma tabela `ISENTOS` com o motivo, exatamente como
`conferidor_segredos.rs`), e não por "está depois de um `#[cfg(test)]` que eu
contei sozinho". Cada entrada do `ISENTOS` é uma linha nomeada, revisável a
olho — os 9 sítios que hoje são teste (4+1+2+3, listados acima) entram nele
no dia em que o conferidor nascer.

**O que essa catraca NÃO pegaria** (dizer é parte da guarda):

- **Reescritas manuais do laço de linha** que não chamam nenhum dos seis
  métodos — um `for byte in fluxo.bytes() { if byte == b'\n' { ... } }` monta a
  mesma coisa sem usar `read_line`. Busquei essa forma nesta rodada (`grep
  '\.bytes()'`) e não achei nenhuma sobre `TcpStream`; mas a catraca proposta,
  por casar só os seis nomes, não a pegaria se alguém a escrever amanhã.
- **`fill_buf`/`consume` direto** (a forma de `descartar_ate_a_quebra`) — de
  propósito, porque é outra pergunta (drenar, não parsear), mas isso quer dizer
  que um SEGUNDO uso de `fill_buf` que na verdade reimplemente um parser de
  linha (a pergunta do `Canal`, escondida atrás de um nome que foge da lista)
  passaria batido. A guarda mede a FORMA, não a INTENÇÃO.
- **Protocolos que o `Canal` não serve e nunca deveria** — SMTP (`email.rs`),
  MySQL/PostgreSQL (`dblink/mysql.rs`, `pg/mod.rs`) continuam fora do
  denominador certo se a catraca não os isentar por nome: sem o `ISENTOS`,
  `email.rs:173` reprova pela letra da lei mesmo sendo pergunta diferente — e
  É pergunta diferente, então a isenção tem de vir escrita, com o motivo deste
  documento, não calada.
- **Um `Canal` de fato usado ERRADO** — chamar `ler_ate` com um teto absurdo
  (`u64::MAX`) não aparece nesta catraca: ela conta AUSÊNCIA de `Canal`, não
  MAU USO dele. Esse caso já tem prova própria em `fio.rs`
  (`a_leitura_padrao_para_no_teto_do_registro_e_nao_no_que_o_outro_lado_mandar`).

**A prova real que a catraca exige de si mesma** (o que falta para ela nascer
provada, não só proposta): um teste que planta um `read_line` cru fora do
`ISENTOS`, num arquivo de exemplo dentro da árvore copiada do
`bancada/guardas/`-style, e confere que o conferidor sobe de 1 para 2 —
espelhando a prova de `conferidor_temporarios.rs`/`conferidor_segredos.rs`, que
já fazem isso com um arquivo fabricado em `DirTemp`. Sem essa prova, a catraca
seria só um número no comentário — a mesma crítica que a lei faz a todo número
não medido.

### Para as outras famílias

**Tetos, cifra, avisos de arranque:** nenhuma catraca proposta — a régua
verificou que cada par de sítios responde a uma pergunta diferente, e mecanizar
"todo `TETO_*` tem de ter irmão" ou "toda constante booleana pública é
suspeita de duplicação" reproduziria o erro que o `CLAUDE.md` já nomeou para o
conferidor de erro cru: 8 interpolações, 2 defeitos, um casador reprovaria as
outras 6 legítimas. Aqui: 34 tetos, 0 duplicação; 7 referências de cifra, 0
duplicação; 3 impressões de aviso, 0 decisão duplicada (só formatação
repetida, que não é o alvo da pétrea).

**`*_env`:** nenhuma catraca proposta pelo mesmo motivo — os únicos dois sítios
fora do `Segredo::ler` (`PHXSQL_SENHA` em dois binários) respondem pergunta
diferente da que o `campo_env` resolve, e uma catraca que tratasse "duas
strings `std::env::var("PHXSQL_SENHA")` iguais" como duplicação estaria
medindo forma de novo, não decisão.

## Resumo para quem vai decidir

| pétrea | guarda hoje | número medido | catraca proposta |
|---|---|---|---|
| leitura de linha de soquete fora do `Canal` (a de hoje) | **nenhuma** | **1** (`email.rs:173`, SMTP — pergunta diferente, mesmo risco de memória do 434, sem teto) | `TETO_LEITURA_FORA_DO_CANAL = 1`, `conferidor_canal.rs`, com `ISENTOS` nomeado (9 sítios de teste catalogados nesta rodada) |
| tetos de tamanho | — | 34 constantes, 0 duplicação | nenhuma — não mecaniza |
| padrão de cifra | já unificado (`CIFRA_DE_SAIDA_PADRAO`) | 1 declaração, 7 usos | nenhuma — já correto |
| segredo por variável de ambiente | já unificado (`Segredo::ler`) | 5 donos, 0 sexto | nenhuma — pedido 372 cobre |
| aviso de arranque | já unificado (`Config.avisos`) | 1 acumulador, 3 impressões (repetição, não decisão) | nenhuma — não mecaniza |

**A catraca que subiria indevidamente com a mudança em revisão:** nenhuma das
existentes muda de teto por causa desta pétrea — ela é nova, não altera régua.
O risco nomeado é o oposto: nascer com um número **maior** que o medido (por
exemplo, `TETO_LEITURA_FORA_DO_CANAL = 5`, copiando o texto do 434 sem
remedir) seria subir a catraca no dia em que ela nasce, e a lei manda nascer no
número do dia — que é **1**, não o histórico do pedido 434.
