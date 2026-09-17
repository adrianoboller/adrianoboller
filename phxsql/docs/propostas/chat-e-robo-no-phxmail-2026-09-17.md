# Chat estilo WhatsApp e robo estilo bot do Telegram no PhxMail — pesquisa medida contra licenca e gargalo

**Papel:** J (pesquisa, duas frentes) redigido pelo papel H (documentacao).
**Data:** 17/09/2026. Todas as afirmacoes de licenca desta secao 2 foram
**reconferidas pelo integrador na fonte** — nao sao a palavra do pesquisador
repassada sem checagem. Numero sem fonte ao lado esta marcado **«nao
medido»**; nenhum numero deste documento saiu de memoria.

**Estado: pesquisa e parecer — nao e plano, nao ha codigo, nada foi gravado,
nada foi compilado.**

---

## 1. A pergunta do dono

O dono quer, dentro do **PhxMail**, um **chat estilo WhatsApp** e um **robo de
mensagens com push estilo bot do Telegram**, e pediu que a pesquisa saisse dos
fontes do GitHub dos dois produtos — nao da marca, da documentacao de
marketing ou da lembranca de como eles se comportam vistos de fora.

---

## 2. Licencas — a secao que vem primeiro porque pode matar o resto

Antes de ler uma linha de codigo alheio, a pergunta e se a leitura e legal e se
o motor apanhado do outro lado obriga a algo que esta casa nao pode cumprir.
Reconferido pelo integrador, lendo o `LICENSE` na fonte de cada repositorio:

| repositorio | licenca medida | veredito |
|---|---|---|
| `tdlib/td` (TDLib) | `Boost Software License - Version 1.0` | **PODE LER E INSPIRAR-SE** |
| `tdlib/telegram-bot-api` | `Boost Software License - Version 1.0` (byte a byte igual ao do TDLib) | **PODE LER E INSPIRAR-SE** |
| `telegramdesktop/tdesktop` | `GNU General Public License version 3` | **NAO LER** — e nao foi lido: so o `LICENSE` foi buscado |
| `signalapp/libsignal` | `GNU AFFERO GENERAL PUBLIC LICENSE, Version 3` | **NAO LER** |
| Especificacoes do Signal (X3DH, Double Ratchet) | **dominio publico** (*«This document is hereby placed in the public domain.»*) | **LER E REIMPLEMENTAR** |
| `tulir/whatsmeow` | MPL-2.0 no invólucro; a dependencia `go.mau.fi/libsignal` resolve para `tulir/libsignal-protocol-go`, **GPL-3.0** | ler vocabulario, nunca copiar |
| `WhiskeySockets/Baileys` | MIT no invólucro; `libsignal` de runtime e **GPL-3.0** no registro npm | idem |
| `jlucaso1/whatsapp-rust` | MIT **declarado sobre linhagem AGPL/GPL/MPL** | **NAO LER** — risco assimetrico |
| Cloud API da Meta | documentacao, nao fonte | pode consumir para integrar; termos **nao medidos** |

### Por que a BSL-1.0 e um caso MELHOR que o do Cassandra, nao so igual

A licenca precedente ja aceita nesta casa e a Apache-2.0 do Apache Cassandra®,
compativel com o nosso `MIT OR Apache-2.0`. A Boost Software License 1.0 do
TDLib e do `telegram-bot-api` e um caso **melhor**, nao apenas equivalente: e
**mais permissiva** que a Apache-2.0— sem clausula de patente para negociar e
sem obrigacao de `NOTICE` para carregar adiante — e o gatilho da unica
obrigacao que ela impoe (preservar o aviso de copyright) e **copiar codigo ou
partes substanciais dele**, nao ler nem se inspirar. A regra desta casa,
«inspiracao, nao copia», ja e mais estrita do que a propria licenca exige: o
TDLib e o `telegram-bot-api` podem ser lidos, entendidos e ter o desenho deles
medido contra o nosso gargalo, exatamente como se fez com o Cassandra.

### Por que a AGPL e a GPL NAO sao o caso do Cassandra

O precedente do Cassandra existe **porque** a Apache-2.0 e compativel com o
nosso `MIT OR Apache-2.0` — a leitura e a inspiracao nao criam obrigacao
nenhuma sobre o codigo desta casa. AGPL e GPL sao o oposto disso, nao uma
variacao mais dura da mesma coisa: sao licencas **copyleft**, e a AGPL em
particular estende a obrigacao de disponibilizar fonte a quem apenas **serve o
software por rede**, sem nunca distribuir um binario — e o PhxSql e
exatamente um software servido por rede. Ler o `tdesktop` (GPLv3) ou o
`libsignal` do Signal (AGPLv3) para «se inspirar» cria um risco que o
Cassandra nunca criou: o de um dia alguem alegar que o desenho do PhxSql
derivou de codigo copyleft e que o proprio PhxSql, servido pela rede, teria de
abrir o fonte. Por isso os dois **nao foram lidos** — so o arquivo `LICENSE`
foi buscado, para decidir se valia a pena ler o resto.

### A honestidade de medicao: 404 relatado, 403 nao confirmado

O pesquisador reportou HTTP **404** ao consultar `facebook/WhatsApp` no
GitHub. O integrador, medindo daqui, obteve **403**, que e a assinatura tipica
de um proxy de rede barrando a chamada — nao do GitHub dizendo que o
repositorio nao existe. Os dois codigos contam historias diferentes: 404 diz
«nao ha nada aqui», 403 diz «alguem impediu voce de ver». Essa metade da
medicao **nao foi confirmada** pelo integrador, e este documento registra a
divergencia em vez de escolher em silencio qual das duas respostas acreditar.

---

## 3. O inventario do nosso lado, medido hoje (17/09/2026)

Busca em `crates/`, `--include=*.rs`:

| padrao | ocorrencias |
|---|---|
| `Sec-WebSocket` / `websocket` | **0** |
| `text/event-stream` / `EventSource` | **0** |
| `long_poll` / `longpoll` | **0** |
| `webhook` | **0** |
| `keep-alive` no `http.rs` e no `servidor.rs` | **0** |
| `grep -rl "correio_" crates/` | **0** — as tabelas do correio existem so como proposta em `docs/CORREIO-FORMATO.md` |
| `setInterval` em `ui/index.html` | **6** — o short-poll ja existe na tela |

Nada do vocabulario de push em tempo real existe hoje no servidor. Mas o que
existe e serve e maior do que o zero acima sugere:

- `rotinas.rs` — os gatilhos (o `ao_alterar`/`ao_excluir` da regra primordial
  da integridade correm por ali).
- `jobs.rs` — o agendador, que roda **como gente**: com o login e o poder de
  um usuario, nao com um poder proprio de sistema.
- `rest.rs` e `mcp.rs` — as duas portas de integracao ja abertas.
- `email.rs` — cliente SMTP inteiro, **392 linhas**, escrito em `std` pura,
  **sem TLS e com o motivo escrito no proprio arquivo**.
- `semaforo.rs`, linha 175 — `adquirir_ate(prazo)`. E exatamente a primitiva
  de espera com prazo que um long-poll precisa, e **ja esta escrita**.
- o `.log` com posicao — a estrutura que a secao 5 mostra ser mais forte que
  a fila do Telegram.

---

## 4. 🚨 O GARGALO — e e ele que decide, nao a elegancia

Medido em `config.rs`:2467-2473 e `servidor.rs`:1805-1875, 8079-8102:

- `conexoes_web_max = 64`, `fila_web_ms = 2000`;
- **uma thread por conexao**;
- `Connection: close` (`http.rs`:334, 773), **zero keep-alive**;
- `vaga_http()` toma a permissao **antes** do pedido e a segura ate o fim;
  depois de UMA espera que estourou, os pedidos seguintes levam **503 na
  hora** por mais 2 s.

| mecanismo | vaga ocupada por cliente | 64 vagas atendem |
|---|---|---|
| short-poll a 1 s | ~0,1% | milhares |
| long-poll de 50 s | **~100%** | **menos de 64 — e o 64º derruba a tela de administracao** |
| SSE / WebSocket | **100%, para sempre** | menos de 64, permanentemente |

**A frase que este documento precisa carregar**: long-poll e melhor que
short-poll **num servidor de laco de eventos**, que e o do Telegram. Num
servidor de thread-por-conexao com 64 vagas, **long-poll e uma negacao de
servico contra si mesmo**.

A aritmetica de ocupacao da tabela acima e **raciocinada, nao medida** — o
tempo de servico de uma conexao long-poll neste servidor nao foi cronometrado.
Ver secao 8, premissas P-A e P-B.

---

## 5. Onde estamos A FRENTE do Telegram, por consequencia de petrea

O `TQueue` do Telegram confirma o offset de forma **destrutiva e implicita**:
`do_get(forget_previous = true)` **apaga** tudo antes do offset confirmado. Um
robo com erro no offset perde dado em silencio, sem aviso e sem forma de
recuperar.

O `.log` desta casa e append-only e a ordem de digitacao e sagrada: **o offset
nunca esquece**. Um robo que volta depois de **uma semana** recebe tudo desde
a sua posicao — o Telegram nao consegue isso: a retencao la e de **24 h no
melhor caso**, e ha caso pior, medido a seguir.

E a licao de medicao que este ponto carrega: **a documentacao do Telegram diz
«not kept longer than 24 hours» e o fonte mostra retencao POR TIPO** —
**86.400 s** para mensagem, **600 s** para `custom_event`, **150 s** para
`shipping_query`, **30 s** para `inline_query`. Quem planejasse pela
documentacao do fabricante erraria por um fator de ate **2.880×** (86.400 ÷
30) para o tipo mais curto. *Numero citado e numero que nao se mede — e aqui
o numero citado era o do proprio fabricante.*

---

## 6. As divergencias, com a restricao nossa nomeada em cada uma

Seis, e cada uma diz **qual petrea causa a divergencia**:

- **D1** — o prazo do long-poll e um **numero de clientes** aqui e um
  **numero de segundos** la. La a consulta segurada e um ponteiro num ator;
  aqui e uma thread e uma vaga. Restricao: **zero dependencias externas** (a
  `std` nao traz reator de eventos).
- **D2** — nao ha segunda fila: o `.log` ja e a fila, e `forget_previous`
  **nao existe e nao pode existir** aqui. Restricao: **`.log` append-only** e
  **ordem de digitacao sagrada**.
- **D3** — retencao por tipo la, retencao pelo **dono do dado** aqui.
  Restricao: **regra primordial da integridade** (a linha de `email` e mae de
  `anexo`, `ao_excluir` so aceita `restringir`) e a privacidade do correio.
- **D4** — confirmar leitura e **gravacao auditavel** aqui, e efeito colateral
  da leitura la. Custa uma gravacao que la custa zero, e se paga porque o
  defeito que ela previne e indetectavel no desenho deles.
- **D5** — 🚨 o `secret_token` do webhook do Telegram viaja **em claro** no
  cabecalho `X-Telegram-Bot-Api-Secret-Token`, e funciona porque la o webhook
  e HTTPS obrigatorio. **Aqui nao ha TLS**, e segredo portador em claro **e
  senha em texto puro** — petrea. Troca: **HMAC sobre o corpo**, com a
  implementacao que ja temos conferida contra **RFC 4231**.
- **D6** — a janela de coalescencia deles (1 ms de espera, 2 ms de teto) e boa
  e barata, **mas** aqui o despertar pode nascer dentro do `RwLock<Raiz>`
  (`servidor.rs`:761): copiar sem saber de que lado da trava ela cai troca
  2 ms de agrupamento por **2 ms de trava global segurada**. Ver premissa P-D.

---

## 7. A briga que trava sprint antes de comecar: ponta-a-ponta × robo

Dois pontos do PhxMail hoje exigem texto legivel no servidor:

- `fts.rs` indexa **no servidor** → sobre texto cifrado nao acha nada;
- `rotinas.rs` dispara na escrita **no servidor** → o gatilho receberia so o
  ciphertext.

**A saida, e ela ja esta construida**: o `jobs.rs` roda «como gente», com o
login e o poder de um usuario. Logo **o robo e uma PONTA, nunca o meio** —
entra na conversa com chave propria, recebe a copia cifrada dele, decifra e
responde. O servidor continua cego ao conteudo.

Registre que isso **nao e invencao nossa**: a politica de privacidade do
WhatsApp admite que o negocio da a um terceiro processador acesso as
comunicacoes quando o usuario usa recursos de negocio. La a ponta comercial e
da propria Meta; **aqui ela roda na maquina do dono da conta**. Restricao que
causa a divergencia: **nao existe terceiro nesta casa**.

**A decisao do dono, que vem ANTES de qualquer sprint porque muda o formato**:

- **(a)** chat ponta-a-ponta — sem busca no servidor, sem gatilho de
  conteudo; ou
- **(b)** chat cifrado so em repouso — `fts.rs` e `rotinas.rs` funcionam e o
  servidor le tudo.

**Nao existe (c)**: vender busca no servidor dentro de um chat anunciado como
ponta-a-ponta seria mentir sobre a garantia.

---

## 8. As quatro premissas a medir antes de qualquer item virar plano

- **P-A** — quantas das 64 vagas HTTP a interface ja come em repouso, com uma
  aba aberta? O numero ja e exposto por `vivo("http", &self.permissoes_http)`
  em `servidor.rs`:19799. **Decide o teto de usuarios do chat.**
- **P-B** — quanto custa uma rodada de short-poll **vazia**, ponta a ponta, em
  ms e bytes? A bancada `enxurrada-web.py` ja existe. Sem essa medida, a
  aritmetica da secao 4 e chute.
- **P-C** — o `.log` da `email` acorda quantos esperadores por mensagem?
  Decide entre sinal por tabela (O(N) por mensagem) e sinal por destinatario.
- **P-D** — o despertar cai **dentro ou fora** do `RwLock<Raiz>`?

Nenhuma das quatro foi medida nesta rodada.

---

## 9. Recomendacao, e ela INVERTE a intuicao

- **v1 — short-poll nos dois**, chat e robo. **Zero codigo novo de servidor**:
  a tela ja faz `setInterval`. Latencia 0–1.000 ms, mediana ~500 ms — contra
  os **2.012 ms** que a replicacao entrega hoje (medido em
  `docs/propostas/p2p-transporte-2026-09.md`:409, com rede pura de
  **0,47 ms**: 0,023% do atraso e transporte, o resto e sono do laco).
- **v1.5 — webhook de saida** para o robo que prefere ser chamado. Custo
  **zero** no gargalo (o servidor liga, nao segura vaga). Tres divergencias
  obrigatorias:
  - o limite de TLS declarado o mesmo que o do `email.rs` (rede que voce
    controla, nao URL aleatoria de terceiro);
  - **HMAC no lugar do `secret_token`** (ver D5);
  - o recuo do `Ritmo` do `replica.rs` **com o sorteio anti-rebanho** que o
    `WebhookActor.cpp`:500 ensina (`Random::fast(60, 120)`) — essa e boa,
    barata, e nao passaria pela nossa cabeca sem ler o fonte deles.
- **v2 — long-poll, em PORTA PROPRIA**, so depois de P-A e P-B medidas. E **o
  primeiro consumidor e a replicacao (pedido 19), nao o chat**: e a mesma
  primitiva, mas o numero de conexoes seguras da replicacao e escolhido pelo
  **administrador**, e o do chat e escolhido pelos **usuarios**. A replicacao
  ja tem o ganho medido (secao acima); o chat ainda nao tem nem a pergunta
  respondida.

---

## 10. As recusas, com o motivo

- **Ler o `tdesktop`** — GPLv3, nao compativel com `MIT OR Apache-2.0`.
- **Usar o `libsignal` do Signal** — AGPLv3, alcanca servico em rede.
- **Portar `whatsmeow` / `Baileys` / `whatsapp-rust`** — miolo GPL-3.0 de
  runtime nos dois primeiros, linhagem AGPL/GPL/MPL nao rastreada no
  terceiro.
- **Cliente nao oficial do protocolo do WhatsApp** — questao de termo de
  servico, nao de licenca; fora do escopo desta pesquisa de codigo-fonte.
- **A ponte Cloud API da Meta, nesta rodada** — exige porta de **entrada**
  com TLS valido, que esta casa nao tem.
- **WebSocket na v1** — o aperto de mao do RFC 6455 exige **SHA-1**, que esta
  casa nao tem e nao vale escrever so para isso, mais 100% de vaga ocupada
  permanentemente (secao 4).
- **SSE na v1** — **mais barato de escrever** que o long-poll; o que o mata
  nao e a complexidade, e a vaga (secao 4).
- **Copiar o desenho do `TQueue`** — o `.log` ja e a fila, e e melhor (secao
  5): nao apaga nada por engano.
- **Copiar o `secret_token`** — segredo em claro sobre canal sem TLS e senha
  em texto puro (D5).
- **Push de celular por FCM/APNs** — impossivel em `std` pura: exige TLS,
  HTTP/2, JWT ES256 ou OAuth2, e conta cadastrada na Apple ou no Google. **E
  gate externo, da mesma familia do TLS na conexao.**

Registre tambem que o **Web Push** (RFC 8291/8188 + VAPID) e o unico parente
cujas primitivas criptograficas esta casa alcanca — mas **ainda precisa de
TLS**. Fica escrito aqui para nao voltar como «mas o Web Push e aberto».

---

## 11. O que NAO foi medido, declarado

Nenhuma bancada foi rodada nesta pesquisa: todo custo de implementacao em
`std` pura para os itens propostos e **raciocinado, nao medido**. Alem disso,
ficam **nao medidos**:

- os termos completos da documentacao da Cloud API da Meta;
- a documentacao da Apple sobre APNs;
- quantas quebras historicas do `Baileys` foram causadas por mudanca do lado
  da Meta;
- qual das paginas da Meta vale para a retentativa de webhook (ha divergencia
  entre 7 dias e 36 h nas fontes consultadas, e nenhuma das duas foi
  reconferida);
- a proveniencia linha a linha do `whatsapp-rust`;
- o conteudo do WhatsApp Security Whitepaper — o PDF baixou (**1.879.555
  bytes**), mas a extracao de texto saiu ilegivel, e **nenhuma linha dele e
  citada** neste documento.

Onde este documento nao diz «medido em <arquivo>:<linha>» ou nao traz URL da
fonte primaria, a afirmacao correspondente e raciocinio, nao medicao.
