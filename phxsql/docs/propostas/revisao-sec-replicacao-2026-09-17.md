# Revisão SEC adversária — replicação, cluster e quórum

*17/09/2026, 02:41 UTC. Papel SEC (revisor adversário), só leitura.* Nenhum
servidor foi subido, nenhuma bancada de tempo foi rodada — a bateria estava
rodando em paralelo (papel F). Toda prova aqui é de **leitura**, com
arquivo:linha e o trecho.

O que esta revisão procurou: o que quem tem **o token**, quem tem **a
credencial de réplica** (`replicar`) e quem tem **`administrar`** consegue pela
frente de replicação — e onde o portão que a casa já tem não alcança.

**Ordem de leitura para quem vai consertar:** A1 e A2 são as duas de
severidade alta e são independentes uma da outra. A3 é a que bate numa pétrea.

---

## Sumário

| # | severidade | uma frase | credencial exigida |
|---|---|---|---|
| A1 | **alta** | o pulso do cluster aceita **identidade auto-declarada** e época/posição/prioridade **sem teto**: um nó (ou quem tenha a credencial do cluster) rebaixa o master e paralisa a eleição **para sempre, inclusive depois de reiniciar** | `replicar` |
| A2 | **alta** | `replicar` com `"max":0` lê **o diário inteiro com as imagens** para a RAM **com a trava global de dados na mão**; o teto de 16 MiB corta **depois** | `replicar` |
| A3 | **média-alta** | `aplicar` é o caminho de gravação que desliga FK, CHECK, cascata e o `conferir_filhas` — e num `source`/`multi` **sem** `somente_leitura` ele está aberto: mata o pai que tem filhos, contra a pétrea | `administrar` na tabela |
| A4 | **média** | `cluster_no_remover` com `"propagar":false` cria **maiorias assimétricas**: dois masters graváveis; `cluster_no_acrescentar` com o mesmo campo tira a escrita do cluster inteiro | `administrar` |
| A5 | **média** | `replicacao_testar` com `host`/`porta` livres é sonda de rede interna (SSRF cego), **sem prazo de conexão** e sem teto de tentativas | `administrar` |
| A6 | **média** | `cluster_estado` entrega o **mapa da infraestrutura** (endereço:porta de cada nó, época, posição) a quem só tem `ler` — o contrário do critério que a casa aplicou ao `sistema` | `ler` |
| A7 | **média** | `replicas_autorizadas` é o **mesmo** portão da porta web: atrás de proxy reverso ou NAT a lista colapsa num IP só | — |
| A8 | **média** | `replicar` entrega o valor das colunas marcadas como dado pessoal e **não grava registro na trilha** (`.lgpd`) | `replicar` |
| A9 | **baixa-média** | modo B: o `carimbo_ms` vem do outro lado **sem teto**; um par hostil (não só um relógio adiantado) ganha **todo** conflito para sempre e sobrescreve calado | ser origem configurada |
| A10 | **baixa-média** | a op `config` publica a lista de nós do **arranque**, não a viva: o denominador da maioria mente depois de um escalonamento a quente | `administrar` |
| A11 | **baixa** | `cluster_pulso` é oráculo de ids de nó, com três respostas distintas e **sem contar violação leve** | `replicar` |

E uma seção separada, §Z: um achado **já documentado e ainda aberto no código**
(`SEGURANCA.md` §12.4), reconferido nesta rodada porque cai exatamente na
fronteira desta frente.

---

## A1 — alta — o pulso: identidade auto-declarada, época sem teto, estrago que sobrevive ao reinício

### A prova, de leitura

O pulso é lido inteiro do **pedido**, e nada nele é conferido contra quem está
do outro lado do soquete:

`crates/phxsql-server/src/cluster.rs:86-103`

```rust
pub fn de_json(j: &Json) -> Option<(String, PulsoDeNo)> {
    let id = j.texto_ou("id", "").trim().to_string();
    ...
    epoca: j.inteiro_ou("epoca", 0).max(0) as u64,
    posicao: j.inteiro_ou("posicao", 0).max(0) as u64,
    incompleta: j.booleano_ou("incompleta", false),
    prioridade: j.inteiro_ou("prioridade", 0),
    quando_ms: crate::agora_ms(),
```

O que a operação confere, e o que ela **não** confere
(`crates/phxsql-server/src/servidor.rs:3477-3486`):

```rust
if estado.no(&id).is_none() {
    return Err(PhxError::Autorizacao(format!(
        "o no {id:?} nao esta na lista de nos deste cluster"
    )));
}
if id == estado.config.id { ... }
```

Ela confere que o id **existe na lista**. Ela não confere que **quem conectou é
aquele nó** — e não tem como: `docs/CLUSTER.md:408-410` escreve a decisão com
todas as letras — *«o cluster autentica como a réplica, por usuário e permissão
`replicar`»*. A credencial é **uma só para o cluster inteiro**
(`cluster.token` / `cluster.usuario` / `cluster.senha_hash`), então todo nó
pode dizer que é qualquer outro nó.

O caminho do estrago, de `registrar` até o disco:

1. `cluster.rs:328-345` — `registrar` grava o pulso no mapa **como veio**, e
   ainda espelha a época quando ela é maior:
   `self.epoca.store(pulso.epoca, …); let _ = self.persistir();`
2. `cluster.rs:473-480` — `maior_epoca_vista()` é o **máximo sobre o mapa
   inteiro**, sem filtro de janela: um pulso forjado envenena o número para
   sempre (a entrada só sai se o nó for removido da lista);
3. `servidor.rs:3121-3131` — o master se rebaixa sozinho ao ver época maior:
   `if maior > estado.epoca() { … let _ = estado.rebaixar(maior); }`
4. `cluster.rs:500-505` — `rebaixar` chama `persistir()`, e `EstadoCluster::novo`
   (`cluster.rs:223-235`) faz o **arquivo ganhar do `config.json`**. Reiniciar
   não desfaz.

E o número não tem teto útil: `phxsql-core/src/json.rs:86-91` faz `*n as i64`,
que **satura** em `i64::MAX` para qualquer número grande. A casa já tem o crivo
para isso — `Json::inteiro_impreciso` (`json.rs:101-106`, *«este numero cru pode
ter perdido precisao ao virar f64?»*) — e o pulso **não o usa**.

### O cenário de exploração, entrada → efeito

Entrada (uma linha, pela porta de dados de qualquer nó, autenticado com a
credencial do cluster — que é a credencial de `replicar`, `usuarios.rs:388`):

```json
{"op":"cluster_pulso","id":"no1","papel":"master","epoca":1e19,
 "posicao":1e15,"prioridade":9223372036854775807,"token":"<o do cluster>"}
```

Efeito, em três camadas:

- **no master de verdade**: `maior_epoca_vista()` = `i64::MAX`, `maior >
  epoca()` → **rebaixa a réplica** e persiste. O cluster fica sem master
  gravável;
- **em toda réplica que receber o mesmo pulso**: `master_visto_ms` é renovado a
  cada pulso forjado, então o silêncio nunca vence a janela e **a eleição nunca
  abre**. E, se abrir, `vencedor` (`cluster.rs:134-147`) elege o forjador, que
  se declarou `incompleta:false` e com a maior posição — e as réplicas só
  registram *«aguardando no1 assumir»* (`servidor.rs:3225-3229`);
- **a escrita dos clientes** vai para o endereço do forjador:
  `recusa_de_escrita` devolve `REDIRECIONA {}` com `no.alvo()` do id que o
  pulso declarou (`cluster.rs:588-592`).

Basta **um** pulso. E depois dele a época de todos está perto de `u64::MAX`, de
modo que um master legítimo eleito em seguida é ignorado por
*«master de epoca velha nao conta»* (`cluster.rs:330`, teste
`master_de_epoca_velha_nao_conta`, `cluster.rs:788`). O conserto operacional
passa a ser **apagar `cluster.estado.json` em cada nó à mão** — isso é
paralisia persistente, não indisponibilidade transitória.

### O que já mitiga (e o que a mitigação não alcança)

- **`ips_permitidos`** é conferido no `accept`, antes de tudo
  (`servidor.rs:8283` e `8295-8296`). É a tranca mais forte aqui — e ela **não
  alcança um nó do próprio cluster**, cujo IP tem de estar liberado para o
  pulso funcionar;
- **`replicas_autorizadas` NÃO tranca o pulso**, e esse é o achado de alcance:
  a lista tranca três operações e o pulso não é uma delas —
  `servidor.rs:309`:
  `pub(crate) const OPS_DE_REPLICACAO: &[&str] = &["posicao", "replicar", "aplicar"];`
  O comentário acima dela explica a escolha (*«`replicacao_estado`,
  `replicacao_testar` e `spare_promover` NAO entram … sao operacoes de
  administracao»*) e **não menciona o `cluster_pulso`**, que não é operação de
  administração: é a operação que decide quem manda, com a credencial de
  replicação. É a pétrea *«quando o portão passar a olhar um campo novo, procure
  quem não tem esse campo»* vista por outro eixo: o portão também tem uma
  **lista de operações**, e ela envelhece igual;
- a `cifra` do cluster com pino (`docs/CIFRA-DO-FIO.md` §12) protege de quem
  está **no meio**, não de quem está **na lista**;
- `cluster_no_acrescentar` já exige `administrar` exatamente por este medo
  (`usuarios.rs:390-396`: *«com `Replicar` aqui, qualquer replica poderia inflar
  o cluster com nos fantasmas e travar toda promocao»*). O raciocínio está
  certo e **não foi aplicado ao pulso**, que consegue o mesmo efeito sem mexer
  na lista.

### Conserto proposto, numa frase

Amarrar o pulso à identidade do nó — a chave estática do fio de cada nó já
existe no `config` (`NoCluster.chave_do_fio`) e serve de `known_hosts` nos dois
sentidos — e, até isso existir, recusar pulso com época/posição acima de um
teto sadio (`maior_epoca_vista + folga`, `posicao` conferida contra
`inteiro_impreciso`) e incluir `cluster_pulso` em `OPS_DE_REPLICACAO`.

### O teste adverso que demonstraria

Unitário, sem soquete, em `cluster.rs`: `um_pulso_de_epoca_absurda_nao_destrona`
— `EstadoCluster` promovido na época 3, `registrar("no1", PulsoDeNo{ papel:
Master, epoca: u64::MAX, .. })`, e afirmar que `maior_epoca_vista() <= 3 +
folga` e que o papel continua `Master`. Hoje ele falha. E pelo soquete (papel F,
`bancada/cluster/`): três nós de pé, um pulso forjado mandado por um quarto
processo com a credencial do cluster, e medir quantos segundos o cluster leva
para aceitar uma escrita depois disso — a expectativa adversa é **nunca**, e a
prova tem de conferir o `cluster.estado.json` dos três depois de um reinício.

---

## A2 — alta — `replicar` com `max: 0` lê o diário inteiro com a trava global na mão

### A prova, de leitura

`crates/phxsql-server/src/servidor.rs:21768-21805`:

```rust
fn op_replicar(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
    let desde = p.inteiro_ou("desde", 0).max(0) as u64;
    let max = p.inteiro_ou("max", 500).max(0) as u64;      // 21770 — sem clamp
    let _trava = self.travar_dados()?;                      // 21771 — trava GLOBAL
    ...
    let mut eventos = t.diario_com_imagem(desde, max)?;     // 21796 — le TUDO
    // O corte por BYTES, DEPOIS do corte por eventos
    let mut somados = 0usize;
    if let Some(corte) = eventos.iter().position(|(_, imagem)| {
        somados += imagem.len();
        somados > TETO_DO_LOTE_SERVIDO
    }) {
        eventos.truncate(corte.max(1));                     // 21804
    }
```

E `limite == 0` quer dizer **sem limite** no percurso do diário —
`crates/phxsql-store/src/log.rs:691-701`:

```rust
saida.push((evento, imagem));
if limite > 0 && saida.len() as u64 >= limite {
    ... return Ok(saida);
}
```

`p.inteiro_ou("max", 500)` só devolve 500 quando o campo **está ausente**
(`json.rs:132-134`). Mandar `"max":0` — ou `"max":-1`, que o `.max(0)` converte
em 0 — passa `limite = 0`.

`travar_dados` é o `write()` do `RwLock` **único** do servidor
(`servidor.rs:1404`: `let guarda = self.dados.write()…`): não é por database
nem por tabela.

### O cenário de exploração, entrada → efeito

```json
{"op":"replicar","database":"erp","tabela":"pedidos","desde":0,"max":0,
 "token":"…"}
```

Efeito: o servidor caminha o diário inteiro, **decifrando cada imagem** no
caminho (`log.rs:680-686`, `cab.abrir(...)`), acumulando tudo num
`Vec<(Evento, Vec<u8>)>` — **com a trava global de escrita na mão**. Todo
cliente de todo database para. Só depois disso o teto de 16 MiB corta a
resposta, que é o único pedaço que ele protege.

O tamanho é o do diário, não o do lote: o próprio comentário do teto
(`servidor.rs:555-567`) faz a conta de *«500 linhas com um memo de 200 KiB são
100 MiB»*; com `max:0` o multiplicador é o número de eventos da tabela. Não há
teto de memória neste caminho — `recursos.memoria_max_mb` só é lido pelo
`memoria_carregar` (`servidor.rs:21234`, `23391`).

### O que já mitiga

- o direito `replicar` na base **e** na tabela (portão 3,
  `servidor.rs:9187-9208`), e `replicas_autorizadas` quando preenchida
  (`servidor.rs:9102-9110`). Num servidor de fábrica — cadastro de usuários
  vazio e lista vazia — sobra **só o token**: o portão do login só morde com
  cadastro preenchido (`servidor.rs:8942-8951`);
- o lado **réplica** tem teto de leitura de verdade: `TETO_DO_REGISTRO` = 128
  MiB por linha, aplicado **na leitura** (`phxsql-core/src/fio.rs:495,523`) —
  é o desenho certo, e é o que falta no lado source.

### A contradição com o que está escrito

`docs/REPLICACAO.md:1346-1357` diz, sobre este mesmo teto: *««quanto isso pode
crescer» virou pergunta com resposta obrigatória — e a resposta não podia ser
«o que o outro lado mandar»»*. Hoje a resposta **é** o que o outro lado manda:
o teto corta a **resposta**, não a **leitura**. É a lição do Profiler
(*«o portão que decide isso vem ANTES do trabalho»*) aplicada a teto em vez de a
interruptor.

### Os dois irmãos, no mesmo padrão

Conserto que entra aqui tem de olhar os dois caminhos que chamam o diário sem
limite:

- `servidor.rs:21426` — `op_diario`: `t.diario(0, 0)?` lê o diário inteiro (sem
  imagens) com a trava global, e só então pega os últimos `max`. Direito
  `diario`;
- `servidor.rs:4130` — `absorver_diario_local`: `tabela.diario_com_imagem(mapa.vistos, 0)?`
  — a primeira rodada do bidirecional carrega **todo** o diário local com
  imagens na RAM. Não é entrada de atacante, é crescimento: a tabela engordou.

### Conserto proposto, numa frase

Limitar `max` no `op_replicar` a um teto de servidor (e tratar `0` como o
padrão, nunca como «sem limite») e mover o teto de bytes para **dentro** de
`Log::percorrer`, que é quem sabe o tamanho antes de alocar.

### O teste adverso que demonstraria

Pelo soquete (é do sistema operacional, não de teste unitário): uma tabela com
N eventos com memo, um `{"op":"replicar","max":0}` numa conexão e um `varrer`
numa segunda conexão ao mesmo tempo, medindo **quanto o `varrer` esperou** e o
RSS do processo — a prova tem de medir *quanto* foi lido, e não *se* respondeu,
que é o erro que a casa já pagou. Unitário complementar em `log.rs`:
`percorrer_com_limite_zero_nao_le_tudo`.

---

## A3 — média-alta — `aplicar` é a porta de gravação que desliga a pétrea da integridade

### A prova, de leitura

`aplicar` liga `como_replica` e, com ela, **desliga o julgamento de
integridade** — `crates/phxsql-store/src/table.rs:3921-3934` e `1315-1317`:

```rust
pub fn aplicar_evento(&mut self, operacao: Operacao, rowid: RowId, imagem: &[u8]) -> Result<RowId> {
    self.como_replica = true;
    let r = self.aplicar_evento_interno(operacao, rowid, imagem);
    self.como_replica = false;
    r
}
...
fn julga_integridade(&self) -> bool { !self.como_replica }
```

O que isso apaga, sítio por sítio:

| o que deixa de rodar | arquivo:linha |
|---|---|
| conferência de FK no `inserir` | `table.rs:3065` |
| conferência de FK no `atualizar` | `table.rs:3356` |
| `conferir_filhas_com` no `excluir_de_vez` | `table.rs:3576-3585` |
| CHECK / coluna calculada / DEFAULT (`aplicar_regras`) | `table.rs:2852-2854` |
| planejamento da cascata do `ao_alterar` | `table.rs:1844-1846` |

**O que ele NÃO apaga, dito para ninguém supor a mais:** a unicidade continua
conferida (`table.rs:3107-3115`) — medido pelo papel C na mesma rodada
(`docs/propostas/parecer-dba-replicacao-2026-09-17.md` §2.5). Então o furo é de
**integridade referencial e de regra de esquema**, não de chave duplicada.

E o portão que decide se a operação passa não cobre o caso comum: `aplicar`
está **fora** de `OPS_ESCRITA` de propósito (`servidor.rs:161-166`) e o portão
2b-bis só morde quando `somente_leitura` está ligado
(`servidor.rs:9164-9175`):

```rust
if op == "aplicar" && self.cluster.is_none() && self.somente_leitura() {
```

Num **master/source em produção** — que por definição **não** roda
`somente_leitura` — nenhuma das duas condições vale, e `aplicar` passa com
`administrar` na tabela (`usuarios.rs:322`).

### O cenário de exploração, entrada → efeito

```json
{"op":"aplicar","database":"erp","tabela":"clientes",
 "eventos":[{"operacao":"exclusao","rowid":7}],"token":"…"}
```

Efeito: a linha 7 de `clientes` é **apagada fisicamente** com `pedidos`
apontando para ela. A pétrea da casa — *«nunca se mata o pai que tem filhos»*,
e o `CLAUDE.md` diz explicitamente *«E ela é imposta na gravação, não só na
declaração: `excluir` — de vez e suave — recusa a linha que tem filha»* — é
contornada por uma operação de gravação que a declaração da tabela não viu.

A variante suave é a mesma porta: a exclusão suave chega como `alteracao`
(`table.rs:3996-3998`), e `atualizar` com `como_replica` não chama
`conferir_filhas` em lugar nenhum — então aplicar uma imagem com o bit de
exclusão ligado deixa filha órfã **sem nem o erro de integridade**. E uma
imagem forjada passa por cima de qualquer `CHECK` declarado.

### O que já mitiga

- exige `administrar` **na base e na tabela** — não é furo de anônimo nem de
  leitor;
- num servidor trancado por administração (`source`/`isolado` +
  `somente_leitura`) o portão 2b-bis recusa, e a bateria
  `bancada/seguranca/porta.py` caso 4d-ii mediu esse conserto;
- a decisão de desligar o julgamento **está medida e escrita**
  (`table.rs:1835-1859`): conferir na réplica causava perda de dado nos três
  ordenamentos, e a garantia é da origem. O desenho está certo **para o laço da
  réplica**; o que falta é o crivo de **quem chamou**.

### Conserto proposto, numa frase

Separar as duas portas: manter `aplicar_evento` sem julgamento para o laço
interno da réplica e exigir, para o `aplicar` **pela rede**, que o papel do
servidor seja um dos que existem para receber replicação (o mesmo crivo do
portão 2b-bis) **independentemente** do `somente_leitura`.

### O teste adverso que demonstraria

`aplicar_pela_rede_num_source_nao_mata_o_pai_com_filhos`: source sem
`somente_leitura`, `clientes` mãe com `pedidos` filha e FK conferida, um
`excluir` normal recusando (o comportamento velho, que também tem de ser
afirmado) e o `aplicar` com `{"operacao":"exclusao"}` — hoje o segundo grava.
Com o conserto, os dois recusam e a réplica de verdade continua aplicando.

---

## A4 — média — remover nó sem propagar cria maiorias assimétricas (dois masters graváveis)

### A prova, de leitura

A propagação é **opcional e vem do pedido** (`servidor.rs:3733`):

```rust
if !pedido.booleano_ou("propagar", true) {
```

A lista viva é o denominador da maioria (`cluster.rs:362-367`, `383-385`):

```rust
pub fn e_maioria(&self, vivos: usize) -> bool { vivos * 2 > self.total() }
```

E `remover` também tira o pulso do mapa (`cluster.rs:411-427`), enquanto o
pulso de um nó **fora da lista** é recusado na hora (`servidor.rs:3477-3481`)
— então o nó removido nunca consegue contar a época nova para quem o removeu.

### O cenário de exploração, entrada → efeito

Num cluster de três, contra `no1` (o master):

```json
{"op":"cluster_no_remover","id":"no2","propagar":false,"token":"…"}
{"op":"cluster_no_remover","id":"no3","propagar":false,"token":"…"}
```

Efeito, passo a passo:

1. lista viva de `no1` = `[no1]`, `total()` = 1, `vivos_qtd` = 1 →
   `e_maioria(1)` é verdadeiro → `liberar_escrita(true)`
   (`servidor.rs:3172-3175`). `no1` continua **gravando**;
2. `no2` e `no3` continuam com três na lista, param de ouvir `no1` (o pulso
   deles é recusado) → master calado além da janela → `vencedor` sobre 2 de 3 é
   maioria → **promovem** (`servidor.rs:3213-3224`). O novo master também
   **grava**;
3. `no1` não recebe pulso de ninguém, então `maior_epoca_vista()` não cresce e o
   rebaixamento automático (`servidor.rs:3121`) nunca dispara.

Dois masters graváveis, permanentes, sem partição de rede nenhuma. E o espelho
do mesmo campo é a negação de serviço: `cluster_no_acrescentar` com
`"propagar":false` e cinco ids fantasmas leva `total()` a 8 com 3 vivos →
`3*2 > 8` é falso → **toda escrita do cluster é recusada**.

### O que já mitiga

- exige `administrar` (`usuarios.rs:390-396`, `cluster_para_escalonar` em
  `servidor.rs:3693-3707`), e o portão está em **um** lugar;
- a resposta traz veredito por nó quando a propagação falha
  (`docs/CLUSTER.md:287`) — mas `"propagar":false` **não é falha**, é pedido;
- o `docs/CLUSTER.md` §2.4 já diz que o cluster não é Raft. O que ele **não**
  diz é que uma operação suportada, com um campo documentado como
  antitempestade, produz o split-brain que a eleição por maioria existe para
  impedir.

### Conserto proposto, numa frase

Tratar `"propagar":false` como **ordem interna do próprio cluster** (aceita só
quando o pedido chega autenticado como `cluster.usuario` e de um IP da lista
viva) e, para a ordem que vem de um cliente, recusar a remoção que deixa o nó
sem maioria confirmada nos demais.

### O teste adverso que demonstraria

Pelo soquete, em `bancada/cluster/`: três nós, dois `cluster_no_remover` com
`propagar:false` no master, e depois **uma escrita em cada nó** com retrato
SHA-256 dos três — a prova pega quando os retratos divergem e quando duas
escritas em nós diferentes são as duas aceitas.

---

## A5 — média — `replicacao_testar` como sonda de rede interna, sem prazo de conexão

### A prova, de leitura

`servidor.rs:21976-22011` monta uma `Origem` com **host e porta do pedido**,
sem lista de destinos permitidos:

```rust
let host = p.texto_ou("host", "").trim().to_string();
...
porta: p.inteiro_ou("porta", crate::config::PORTA_PADRAO as i64).clamp(1, 65_535) as u16,
```

e `crate::replica::ligar(&origem)?` (`servidor.rs:22014`) cai em
`replica.rs:70-75`:

```rust
pub fn conectar(host: &str, porta: u16, token: &str, espera: Duration) -> Result<Cliente> {
    let alvo = format!("{host}:{porta}");
    let fluxo = TcpStream::connect(&alvo)
        .map_err(|e| PhxError::Io(std::io::Error::other(format!("{alvo}: {e}"))))?;
```

`TcpStream::connect` **sem prazo** — a variante com prazo existe ao lado
(`conectar_com_prazo`, `replica.rs:80`) e é usada só pelo pulso, com o motivo
escrito: *«o `connect` sem prazo pode ficar minutos pendurado num host que
caiu»*. É o padrão «conserto entrou no caminho que o motivou e o irmão ficou».

E o erro **volta para quem pediu**, com o alvo e a causa do sistema
operacional dentro (`{alvo}: {e}`): «Connection refused» distingue porta
fechada, «Connection timed out» distingue filtrada, e um serviço que aceita e
fala outra coisa cai no erro do aperto. É um scanner com três respostas.

### O cenário de exploração, entrada → efeito

`{"op":"replicacao_testar","host":"10.0.0.37","porta":6379,"token":"…"}` →
a resposta diz se aquela porta interna está aberta. Repetido, mapeia a rede de
dentro. Com `host` sendo um nome, o servidor também **resolve DNS** por ordem
de quem pediu (`Cliente::conectar` resolve dentro do `connect`), que é um canal
de saída às cegas.

Amplificação: cada sonda contra um IP que engole SYN fica pendurada o tempo de
SYN do sistema (na configuração usual do Linux, ~127 s) **ocupando uma vaga de
`conexoes_max`** (padrão 64). E `recusar_se_estacionada` — o único freio —
só vale para **origem nomeada** (`servidor.rs:21963`), não para host solto.

### O que já mitiga

Exige `administrar`; num servidor de fábrica, só o token. A resposta nunca
carrega credencial, e isso está conferido (`servidor.rs:22097-22099`).

### Conserto proposto, numa frase

Usar `conectar_com_prazo` também aqui (o irmão que já existe), classificar o
erro de rede antes de devolvê-lo (uma frase só, sem o texto do sistema
operacional) e, para host solto, contar a tentativa como violação leve pelo
mesmo caminho do resto.

### O teste adverso que demonstraria

Por soquete e contra o sistema operacional: `ss -tn` durante N
`replicacao_testar` para um IP de buraco negro (`192.0.2.1`), medindo quantos
descritores ficam presos e por quanto tempo, e um teste de protocolo que
afirme que a resposta de erro **não** diferencia «refused» de «timed out».

---

## A6 — média — `cluster_estado` dá o mapa da infraestrutura a quem só tem `ler`

### A prova, de leitura

`usuarios.rs:389`: `"cluster_estado" => Atividade::Ler`. A resposta
(`servidor.rs:3810-3856`) traz, **por nó**: `id`, `endereco` (host:porta),
`papel`, `epoca`, `posicao`, `posicao_incompleta`, `ultimo_pulso_ms`, `vivo`, e
o bloco `master` com `id` e `endereco`.

O critério contrário está escrito na mesma função da casa, quinze linhas acima
(`usuarios.rs:179-182`), sobre o `sistema`:

> *«Ja o monitor da MAQUINA pede administrar. Nome de placa de rede, nome de
> disco e ponto de montagem descrevem a infraestrutura, e nao o dado — quem so
> le uma tabela nao ganha nada com isso e o atacante ganha o mapa.»*

Endereço e porta de cada nó do cluster **são** infraestrutura pelo mesmo
critério. A justificativa escrita (`usuarios.rs:386-387`) é que *«qualquer
cliente precisa dele para achar o master»* — e isso justifica o **bloco
`master`**, não a lista inteira com época, posição e idade de pulso de cada nó,
que é exatamente o que um atacante precisa para escolher o alvo de A1 e A4.

### Conserto proposto, numa frase

Partir a resposta: `ler` recebe o bloco `master` e `escrita_liberada`
(o que o cliente precisa para achar quem manda); a lista de nós com época,
posição e idade de pulso exige `administrar`, como o `sistema`.

### O teste adverso

`cluster_estado_de_leitor_nao_lista_os_nos`: usuário nível leitor, e afirmar
que a resposta tem `master.endereco` e **não** tem `nos[].endereco` — e o
irmão, `cluster_estado_de_administrador_continua_completo`, que é o teste do
comportamento velho.

---

## A7 — média — o IP da sessão: não é falsificável por cabeçalho, mas colapsa atrás de proxy

A pergunta do briefing tem duas metades, e as respostas são opostas.

**Não é falsificável.** Não existe leitura de `X-Forwarded-For` nem de
`X-Real-IP` em lugar nenhum de `crates/` (varredura sem nenhuma ocorrência). O
`sessao.ip` sempre sai do par do soquete: na porta de dados
(`servidor.rs:8328-8331`) e na porta web (`servidor.rs:8039`, `8048`, e
`sessao_do_cabecalho` em `servidor.rs:7964-7969`). Isso está **certo** e é o
que fecha a falsificação trivial do portão 2a-bis.

**Mas o portão é o mesmo nas duas portas.** `/api` é *«o mesmo protocolo da
porta 5000»* (`servidor.rs:8032`) e cai no **mesmo** `despachar`
(`servidor.rs:8145`), então `posicao`/`replicar`/`aplicar` são alcançáveis pela
porta web e a única tranca por endereço é a mesma `replicas_autorizadas`
(`servidor.rs:9102-9110`), avaliada contra o IP da conexão HTTP.

Consequência concreta: numa montagem comum — réplica local e um proxy reverso
na frente da interface —, `replicas_autorizadas` contendo `127.0.0.1` autoriza
**todo** cliente que chegar pelo proxy, porque para o servidor todos são
`127.0.0.1`. Atrás de NAT o efeito é o mesmo com o IP da borda. O portão
confia num salto que ele não vê, e o `docs/REPLICACAO.md` §7 descreve a lista
como *«IPs que podem pedir o fluxo de replicação»* sem essa ressalva.

**Conserto proposto, numa frase:** dizer no documento e no aviso de arranque
que `replicas_autorizadas` só vale contra o **par do soquete** e portanto não
vale atrás de proxy ou NAT, e avaliar se a lista deve trancar por
credencial/usuário (que atravessa o proxy) em vez de por endereço.

**Teste adverso:** de leitura/integração — um pedido `replicar` pelo `/api` a
partir de `127.0.0.1` num servidor com `replicas_autorizadas:["127.0.0.1"]`
tem de passar hoje; a prova é o registro dele no `acessos.log` mostrando `ip
127.0.0.1` para um cliente que veio de fora.

---

## A8 — média — `replicar` entrega dado pessoal e não deixa registro na trilha

### A prova, de leitura

A faixa **inline** da imagem viaja decifrada — está medido em
`docs/SEGURANCA.md:1962-1969`: *«o nome marcado aparece literalmente dentro dos
156 bytes da imagem»*. E o registro na trilha tem exatamente **cinco** pontos
de chamada, nenhum deles na replicação:

| ponto | operação | arquivo:linha |
|---|---|---|
| `trilhar_acesso` | `agrupar` | `servidor.rs:11857` |
| `trilhar_acesso` | `ler` | `servidor.rs:16498` |
| `registrar_acesso` | `varrer` | `servidor.rs:16545` |
| `trilhar_acesso` | `buscar` | `servidor.rs:16914` |
| `trilhar_acesso` | `procurar_texto` | `servidor.rs:16956` |

`op_replicar` (`servidor.rs:21768`) e `op_posicao` (`servidor.rs:21690`) não
chamam nenhum dos dois. Então a pergunta que a trilha existe para responder —
*«quem viu o prontuário do fulano?»*, `docs/LGPD.md:155` — não tem resposta
para o caminho que entrega **todas as linhas de uma vez, com o valor dentro**.

### O que já mitiga, dito com honestidade

O `acessos.log` registra a chamada (data, hora, IP, login, operação, base,
tabela) — e é o mesmo argumento que o `docs/LGPD.md:266-273` usa para a trilha
não se registrar a si mesma. A diferença que faz este achado valer: a leitura
da trilha exige `administrar` e **não devolve mais dado do que o auditor já
podia ver**, enquanto `replicar` exige só `replicar` e **entrega o valor**. O
relatório de trilha por tabela, que é o artefato que a lei cobra, fica com um
buraco do tamanho da replicação.

### Conserto proposto, numa frase

Registrar um acesso por chamada de `replicar` quando a tabela tem coluna
marcada, com o critério sendo `replicar desde=N ate=M` e `linhas` = eventos
servidos — o mesmo desenho por operação que o `varrer` já usa, e que
`docs/LGPD.md` §4 mediu como o barato.

### O teste adverso

`replicar_numa_tabela_marcada_deixa_rastro_na_trilha`: marcar uma coluna, um
`replicar` que devolva k eventos, e afirmar que a trilha cresceu em 1 com o
critério dentro — hoje ela não cresce. O irmão obrigatório é
`sem_marca_nada_muda` para o `replicar`, para o conserto não passar a custar
numa tabela sem coluna marcada.

---

## A9 — baixa-média — modo B: o carimbo vem do outro lado, sem teto

`replica.rs:397` lê `carimbo_ms: e.inteiro_ou("carimbo_ms", 0)` — sem teto — e
`bidirecional.rs:97-99` decide o conflito com ele:

```rust
pub fn remoto_vence(carimbo: i64, origem: u16, local: &Toque) -> bool {
    carimbo > local.carimbo || (carimbo == local.carimbo && origem > local.origem)
}
```

A casa já escreveu a metade **acidental** disso — `docs/REPLICACAO.md:636-637`:
*«essa regra exige relógios sincronizados entre os servidores (NTP). Sem isso, o
lado com o relógio adiantado vence sempre»*. A metade **adversária** não está
escrita: um par que **mente** o carimbo (não um relógio que deriva) fixa o
`Toque` local num valor que nenhuma escrita local jamais alcança, e passa a
sobrescrever **calado** toda alteração deste lado, para sempre. É o mesmo campo,
com um dono diferente.

O crivo para isso também já existe na casa e não é usado aqui:
`Json::inteiro_impreciso` (`json.rs:101-106`).

**Conserto proposto, numa frase:** recusar (ou marcar como colisão, no mesmo
contador que `replicacao_estado` já publica) o evento cujo `carimbo_ms` esteja
mais que uma folga configurada **no futuro** do relógio local, e escrever no
`REPLICACAO.md` §12 que a regra confia no par e não só no NTP dele.

**Teste adverso:** unitário em `bidirecional.rs`,
`carimbo_no_futuro_nao_ganha_para_sempre` — um `Toque` local de agora contra um
evento remoto com `i64::MAX`, afirmando que o remoto **não** vence ou que a
colisão é contada.

---

## A10 — baixa-média — a op `config` publica a lista de nós do arranque

`Cluster::para_json` lê `self.nos` — o retrato do arquivo no arranque
(`config.rs:653-670`) —, e `configuracao_json` corrige **três** valores vivos
por cima e não a lista (`servidor.rs:4407-4412`):

```rust
let mut j = self.config.para_json();
j.definir("max_linhas", …);
j.definir("somente_leitura", …);
j.definir("espelho", …);
```

Mas `cluster_no_acrescentar` grava o **arquivo** (`servidor.rs:3714-3720`) e a
**lista viva**, e o `Servidor` guarda `config: Config` **imutável**
(`servidor.rs:633`) — nada recarrega. Então, depois de um escalonamento a
quente, `config` mostra a lista de ontem e `cluster_estado` a de hoje, e a
diferença entre as duas é **o denominador da maioria**. O comentário da própria
função promete o contrário: *«depois de uma gravacao a quente, o arquivo e a
memoria concordam»*.

**Conserto proposto, numa frase:** `configuracao_json` passar a injetar a lista
**viva** do `EstadoCluster` sobre o bloco `cluster`, pelo mesmo mecanismo dos
três campos que já são corrigidos ali.

**Teste adverso:** `config_mostra_a_lista_viva_do_cluster` — acrescentar um nó
a quente e afirmar que `config.cluster.nos` tem o mesmo tamanho que
`cluster_estado.nos`.

---

## A11 — baixa — `cluster_pulso` é oráculo de id, e não conta violação

`servidor.rs:3477-3486` responde três coisas diferentes: erro
*«nao esta na lista»*, erro *«e ESTE servidor»*, ou o pulso. E nenhuma delas
passa por `violacao_leve` — as chamadas existentes são nove e nenhuma está ali
(`servidor.rs:1861,7022,7402,7619,8296,8414,8908,8924,8980,9105`). Quem tem a
credencial de `replicar` enumera os ids do cluster sem gastar tolerância
nenhuma, e é dessa lista que A1 precisa.

**Conserto proposto, numa frase:** contar a recusa de id desconhecido como
violação leve, pelo mesmo caminho da recusa de `replicas_autorizadas`
(`servidor.rs:9105`), que já faz exatamente isso.

---

## Z — já documentado e **ainda aberto no código**: a coluna externa marcada

Não é achado novo — está em `docs/SEGURANCA.md` §12.4 — e é reconferido aqui
porque cai na fronteira desta frente e o briefing pede confirmação.

`crates/phxsql-store/src/reg.rs:1827-1830`:

```rust
pub fn abrir_externo(&self, coluna: u16, guardado: &[u8]) -> Result<Vec<u8>> {
    if !self.material.cifrado() || !self.externa_marcada(coluna) || guardado.is_empty() {
        return Ok(guardado.to_vec());
    }
```

Numa réplica **sem** cifra, o `!self.material.cifrado()` devolve os bytes como
vieram — ou seja, grava o **texto cifrado** como se fosse o conteúdo. A §12.4 já
mediu os 63 bytes e chamou isso de *«o pior dos três casos, porque é o único que
não dá erro»*, e nomeou o que falta: *«uma conferência que falta»*. **Continua
faltando** — o `decodificar_com_externos` (`table.rs:4051-4082`) confia no
retorno.

Não abro achado novo; registro que o buraco está no mesmo lugar e que, para a
conclusão desta rodada, *replicar tabela com coluna externa marcada continua
recusado na prática*.

---

## O que está bem, e por quê — com arquivo:linha

Para a conclusão do papel H não elogiar de memória.

1. **A credencial não sai por resposta de protocolo, e está conferido nos três
   blocos.** `config.rs:3734` (`("token", Json::texto_de("(oculto)"))`),
   `config.rs:3774-3795` (as origens saem com `nome`/`host`/`porta`/`usuario` e
   o comentário *«A senha NAO sai daqui, nem o hash»*), `config.rs:629-631` e
   `652-670` (o `Cluster::para_json` sem token, sem hash e **sem o pino** — só
   `tem_pino`, com o motivo escrito: *«"cifra_do_no" por no diria quem tem pino
   e quem nao, que e mapa para o atacante»*). `EstadoOrigem::para_json`
   (`bidirecional.rs:204-260`) também não carrega credencial nenhuma.
2. **`replicacao.*` está FORA de `CAMPOS_EDITAVEIS`, por decisão escrita.**
   `config.rs:4043-4049`: *«uma sessao roubada nao abre o firewall, nao cria
   supervisor, nao vira a replica para outro source e nao mexe em campo que
   carrega credencial»*. É a razão pela qual `replicas_autorizadas` não se muda
   pela porta web — e é a resposta certa para «quem administra pode virar a
   réplica?».
3. **O quórum não mente em tela nenhuma, e quem diz isso é o servidor.**
   `config.rs:646` grava `("quorum_imposto", Json::Bool(false))` com o
   comentário *«O que o campo acima NAO faz, dito pelo servidor e nao pela
   tela»*; a tela lê esse campo (`idiomas.rs:2008`, `ui/index.html:12423`), o
   `docs/CLUSTER.md:155-161` e o `docs/RISCOS.md` R12 dizem o mesmo. Conferido:
   **nenhuma tela afirma efeito**. É o contrário do `recursos.cache_paginas`.
4. **O arranque avisa o que a lista vazia significa, e diz o pior pedaço.**
   `servidor.rs:1555-1568`: *«`posicao`, `replicar` e `aplicar` atendem QUALQUER
   endereco que tenha o token … e com replicacao.imagem_da_linha ligada o
   `replicar` entrega a LINHA INTEIRA»* — e está **fora** do `if papel !=
   Isolado` de propósito, com o motivo escrito.
5. **O portão 2a-bis fez a pergunta certa quando nasceu.**
   `servidor.rs:9096-9101`: *«O campo que este portao passou a olhar e o IP DA
   SESSAO, entao a pergunta obrigatoria e quem NAO tem esse campo: job agendado,
   rotina interna e a replicacao chamada de dentro chegam com `ip` vazio»*. A
   pergunta foi feita pelo eixo **campo**; A1 mostra que faltou fazê-la pelo
   eixo **lista de operações**.
6. **`op_posicao` confere `replicar` tabela por tabela DENTRO da operação.**
   `servidor.rs:21700-21715` — é a lição do `juntar`/`unir` aplicada antes de
   doer, e o comentário nomeia a família.
7. **O IP da sessão nunca vem de cabeçalho.** Nenhuma ocorrência de
   `forwarded`/`x-real-ip` em `crates/`; o IP é sempre o par do soquete
   (`servidor.rs:8328-8331`, `7964-7969`). O portão 2a-bis não é falsificável
   por pedido.
8. **`ips_permitidos` é conferido no `accept`, antes de qualquer trabalho.**
   `servidor.rs:8283` e `8295-8296` — e a recusa conta violação leve. É a
   mitigação mais forte para A1, A2 e A5.
9. **O túnel vem ANTES do login, nos dois clientes, com o motivo escrito.**
   `replica.rs:417-420` (*«e a prova do desafio-resposta e o token que ele
   existe para esconder, e depois do login ja seria tarde»*) e o pulso do
   cluster igual (`servidor.rs:3017-3027`, com a guarda
   `pulso-do-cluster-em-claro` nomeada). E a amarração ao canal
   (`replica.rs:217-229`) fecha o homem-no-meio que termina o túnel.
10. **O teto de leitura do lado réplica está no lugar certo — na leitura.**
    `phxsql-core/src/fio.rs:495` e `523`, e `replica.rs:54-57` explica por que
    ele **desceu** para o `Canal`: *«Um teto nesta camada voltaria a deixar o
    caminho cifrado sem nenhum»*. É o desenho que falta no lado source (A2).
11. **Pino torto é recusado na DECLARAÇÃO, não no primeiro pulso.**
    `config.rs:617-625`, inclusive com a cifra desligada: *«um pino guardado
    para ligar depois nao pode estar torto esperando o dia em que alguem
    ligue»*.
12. **O escalonamento a quente preserva o pino ao regravar a lista.**
    `config.rs:4356-4365` — sem essa linha, um `cluster_no_acrescentar`
    rebaixaria **todo** o cluster a escuta passiva em silêncio. O comentário diz
    exatamente isso.
13. **As três respostas do pedido 203 estão certas e provadas nos dois
    sentidos.** `replica.rs:468-482` (`Falha::ao_ligar` separa credencial de
    rede e o `na_rodada` **não** trata `Autorizacao` como credencial recusada),
    `replica.rs:536-549` (estaciona na primeira), e
    `credencial_recusada_estaciona_na_primeira` (`replica.rs:588`). O recuo
    limita o deslocamento **antes** de multiplicar (`replica.rs:554-558`), e há
    teste de 100.000 falhas seguidas.
14. **`vencedor` é função pura e a proteção mais importante tem teste que
    falha.** `cluster.rs:134-147`, `sem_maioria_visivel_nao_promove`
    (`cluster.rs:634-642`) e o do pedido 211
    (`eleicao_prefere_completa_a_incompleta`, `cluster.rs:677-695`), este com a
    explicação de **por que a prova pega** escrita no doc-comment.
15. **`posicao_do_diario` deixou de engolir a falha em silêncio.**
    `servidor.rs:3343-3376` devolve `(u64, bool)` e o `incompleta` sobe por
    cinco caminhos distintos — é o conserto do achado do `SEGURANCA.md`
    §«posicao_do_diario nao para a replica».
16. **`cluster_no_*` exige `administrar` com o raciocínio adversário escrito.**
    `usuarios.rs:390-396`: *«com `Replicar` aqui, qualquer replica poderia
    inflar o cluster com nos fantasmas e travar toda promocao»*. O raciocínio
    está certo; A1 é ele não aplicado ao pulso.
17. **`abrir_travada` define usuário e origem num lugar só.**
    `servidor.rs:9747-9753`, com o motivo: *«e o unico lugar em que a tabela e
    aberta para o cliente»*.
18. **A réplica não redireciona para cadáver.** `cluster.rs:572-587` olha a
    idade do último pulso do master antes de mandar o cliente para lá, e diz
    «eleição em curso» em vez de um endereço morto.

---

## O que esta revisão NÃO cobriu

**Por decisão de escopo — nada com servidor de pé.** Não subi servidor, não
rodei bancada de tempo, não rodei os testes por soquete. Tudo acima é leitura,
e cada achado traz o teste que o demonstraria.

O que **exige servidor de pé** e fica para o papel F:

1. **A1 pelo soquete** — um pulso forjado contra os três nós de
   `bancada/cluster/`, mandado por um processo que não é nó, com a credencial do
   cluster. A prova tem de ir até o fim: conferir o `cluster.estado.json` dos
   três **depois de reiniciar**, porque o que eu li diz que o estrago persiste.
2. **A2 pelo soquete** — `{"op":"replicar","max":0}` numa tabela grande com
   memo, medindo RSS do processo e **quanto tempo** um `varrer` concorrente
   esperou. É a métrica que a casa já aprendeu a exigir: medir *quanto* foi
   lido, não *se* respondeu.
3. **A5 contra o sistema operacional** — `ss -tn`/`strace` durante sondas de
   `replicacao_testar` para um IP de buraco negro, contando descritores presos e
   vagas de `conexoes_max` consumidas.
4. **A4 pelo soquete** — dois `cluster_no_remover` com `propagar:false` e
   retratos SHA-256 dos três nós depois de uma escrita em cada.
5. **Queda de conexão, reserva presa, descritor aberto** — o
   `crates/phxsql-server/tests/trava-atras-da-rede.rs` e o estágio `queda` da
   bancada de contêiner já cobrem a janela do §18; **não** os reexecutei, e a
   página de testes tem de dizer a data da última corrida, não a minha palavra.
6. **A cifra do fio ponta a ponta** — só li o desenho. Não provei o
   rebaixamento com `exigir:false`, nem o pino recusando chave trocada.
7. **Os vetores oficiais de cripto** — não refiz nenhum. A auditoria está em
   `docs/propostas/auditoria-sec-cripto-2026-09.md` e é dela que a conclusão
   deve citar FIPS 180-4, RFC 4231, PBKDF2 e RFC 8032, com a data de lá.
8. **O quórum de escrita** — não há código de imposição para revisar
   (`config.rs:646` declara `quorum_imposto: false`). Revisei só a honestidade
   da declaração, e ela está certa.
9. **A porta REST e o MCP** — `rest.rs` e `mcp.rs` não entraram: o briefing
   delimitou a frente na replicação/cluster. Se `replicar` for alcançável por
   elas, A2 e A7 têm um terceiro caminho, e isso precisa de uma varredura
   própria.

E o que eu **não** repeti por já estar fechado: as saídas de segredo por
`Debug` e por profiler (`docs/SEGURANCA.md` §16 e §17, e
`docs/propostas/revisao-sec-saidas-de-segredo.md`), e o
`posicao_do_diario` engolindo falha, que o pedido 211 consertou.
