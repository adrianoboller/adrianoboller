# Revisão SEC — rodada de 23/09/2026, noite

**Papel SEC (revisor adversário, não autor).** Alvo: `8f63d7a`, `af4a5a5`,
`f3abf54`, `8933f0f`, `8c3e77c` — pedidos **278** (prova de identidade do
pulso), **312** (`TETO_DO_APERTO`), **392** (escala do `Decimal`) e **339(a)**
(chave da API na aba).

**Método e limite, declarados.** Leitura do diff `9b8709e..8c3e77c` e do código
em volta; **nenhum `cargo build`/`test` foi rodado** (disco em 2,3 GB, ordem
explícita). Portanto: **nenhum número aqui é medido por mim.** Onde eu digo
«medido», o número é do autor e está citado como dele. Os cenários abaixo são
derivados do fonte, e cada um traz o **teste adverso** que o confirmaria ou o
mataria — que é o que falta para virar fato.

Veredito: **BLOQUEIO em A1 e A2** (memória reservada antes de autenticação, do
lado exposto; e oráculo que entrega o mapa de quem ainda é atacável). O resto
entra como dívida priorizada.

---

## ALTO

### A1 — O 312 consertou a metade de FORA; a metade EXPOSTA continua como estava

**Onde.**
- `crates/phxsql-server/src/servidor.rs:9543` — `canal.ler(&mut leitor)` no laço
  de conexão, ou seja `TETO_DO_REGISTRO` (**128 MiB**) para a **primeira** linha
  de uma conexão anônima, antes do `login` e antes do aperto.
- `crates/phxsql-server/src/http.rs:152` — a **linha de pedido** HTTP lida com
  `read_line` cru, **sem teto nenhum**.
- `crates/phxsql-server/src/http.rs:163` — cada linha de **cabeçalho**, idem: o
  `MAX_CABECALHO` (16 KiB, `http.rs:121`) é conferido **depois** de a linha
  inteira já estar na memória, então ele limita o *acumulado*, nunca a *linha*.

**Por que isto é o irmão, e não outro assunto.** O pedido 312 define o irmão
pela pergunta, não pela função: *«quanto eu reservo antes de saber quem é?»*. O
conserto respondeu-a em `replica::Cliente::cifrar` (`replica.rs:160`) e em
`servidor::Remoto::cifrar` (`servidor.rs:575`) — as duas **saídas**, falando com
um par configurado. A **entrada** — a porta que qualquer um da rede alcança — dá
a resposta antiga: 128 MiB na porta de dados, ilimitado na porta web. O teto
desceu 2.048× no lado que o atacante não usa.

**Cenário.** `recursos.conexoes_web_max` nasce **64** (`config.rs:2707`).
64 soquetes para a porta web, cada um mandando `GET /` seguido de bytes sem
`\n`: 64 `String` que crescem até o processo morrer, **sem uma linha de log e
sem um pedido HTTP válido**. Na porta de dados o mesmo com
`recursos.conexoes_max` = 64 (`config.rs:2703`), com o teto em 128 MiB → 8 GiB
de pico antes de qualquer credencial. Repare que ali o laço **sabe** tratar o
estouro (drena, responde, anota em `acessos.log`, conta violação leve,
`servidor.rs:9550-9600`) — o defeito não é a falta de tratamento, é o **tamanho**
que se deixa reservar a quem ainda não é ninguém.

**Não é falso positivo porque:** o que separa este caso do teto legítimo de
128 MiB é que o registro grande existe para o **lote** de um cliente já
autenticado; a **primeira** linha de uma conexão nunca é um lote. E na porta web
nem esse argumento cabe — `MAX_CORPO` já é 4 MiB e é conferido antes de alocar
(`http.rs:183-185`), o que prova que o autor da porta web sabia a diferença e
deixou a linha de fora.

**Teste adverso.** Dois, no molde do `tests/teto-do-aperto.rs` (que mede do lado
cliente) — mas apontando para o servidor:
1. `a_porta_web_recusa_a_linha_de_pedido_sem_fim`: subir o servidor, abrir o
   soquete web, empurrar `"GET /" + "x"*N` sem `\n`, e assertar que o processo
   para de ler antes de `MAX_CABECALHO + folga`. Com o `read_line` de hoje, o
   contador do lado que empurra passa de 128 MiB sem erro nenhum.
2. `a_primeira_linha_da_porta_de_dados_nao_reserva_o_teto_do_registro`: idem na
   5000, medindo o RSS do servidor (`/proc/self/status`, que o
   `telemetria.rs:1660` já sabe ler) antes e depois.

---

### A2 — O erro do pulso entrega o mapa de quais nós ainda são atacáveis

**Onde.** `crates/phxsql-server/src/cluster.rs:506-513` contra
`crates/phxsql-server/src/pulso.rs:174-179`, com o texto inteiro do erro indo
para o fio em `servidor.rs:25678` (`("erro", texto_do_erro(e))`).

**O defeito.** Quem manda um `cluster_pulso` com `"prova"` qualquer recebe uma
de **duas frases distintas**:

- `"o pulso de \"X\" traz prova, mas cluster.nos[X].chave_do_fio esta vazio
  neste no…"` → **X não tem pino aqui**;
- `"a prova do pulso de \"X\" nao fecha…"` → **X tem pino aqui**.

E «X não tem pino» é exatamente «X nunca vai conseguir provar, logo X nunca
entra no `provaram`, logo **um pulso sem prova dizendo-se X continua passando**»
enquanto `exigir_prova_do_pulso` estiver no padrão. O erro do conserto do 278
publica a lista dos ids para os quais o 278 **não** foi consertado.

**Não é falso positivo porque** esta casa já pagou por este defeito exato,
uma camada acima e há seis dias: a nota do `op_cluster_pulso`
(`servidor.rs:4160-4172`) diz que «não está na lista» e «é ESTE servidor» viraram
**uma** resposta só por serem oráculo de ids (revisão SEC de 17/09, A11). E o
`config.rs:798-812` recusa publicar `cifra_do_no` por nó pelo mesmo argumento,
escrito: *«quem tem pino e quem nao, que e mapa para o atacante»*. O conserto de
hoje reabriu o mapa pela porta do erro.

**Cenário.** Atacante com a credencial de replicação (que é **uma só para o
cluster inteiro** — é a premissa do próprio 278): para cada id da lista, um pulso
com `"prova":"00"`; lê as duas frases; escolhe um id sem pino; manda o pulso do
278 original (sem `prova`, com `"papel":"master","epoca":N`). Zero tentativas
desperdiçadas contra ids que provariam.

**Não custa nada consertar** e por isso o bloqueio: as duas frases viram uma, e
o diagnóstico do pino vai para o `eprintln!` deste processo — o mesmo desenho
que o A11 já usou três telas acima, no mesmo arquivo.

**Teste adverso.** `o_pulso_nao_diz_quais_nos_tem_pino`: subir um nó com dois
pares, um com `chave_do_fio` e outro sem; mandar um pulso com prova inválida
para cada; assertar que as **duas respostas são byte a byte iguais** (`erro`,
`codigo`, `nome`). Hoje ele reprova.

---

## MÉDIO

### M1 — O teto de nonce é de CONTAGEM; a memória continua escolhida por quem manda

**Onde.** `pulso.rs:71-77` (`NONCES_POR_NO = 512`), `pulso.rs:204-228`
(`aceitar`), `cluster.rs:530` (`nonce` vem de `pedido.texto_ou("nonce","")`,
sem nenhuma conferência de tamanho ou alfabeto).

O comentário do teto diz, com todas as letras, que ele existe porque «sem ele a
memoria de quem confere seria escolhida por quem manda — **o mesmo defeito do
pedido 312**». Mas o teto conta **quantos**, e o que se guarda é
`nonce.to_string()` — o **tamanho** de cada um continua sendo do remetente, até
o teto da linha. 512 nonces × o que couber numa linha, por nó, retidos por 60 s.

**O que distingue de falso positivo:** é o primeiro lugar desta base em que um
nonce vindo do fio é **retido**. O `nonce_cliente` do desafio-resposta
(`servidor.rs:10584`) é usado e jogado fora dentro do mesmo pedido — ali o
tamanho não importa, e é por isso que a lição não tinha aparecido ainda. Custo
do ataque: precisa da chave privada do par (o HMAC vem antes, `cluster.rs:531`),
então é **insider / nó comprometido**, não anônimo. É o que o rebaixa de ALTO
para MÉDIO — e não o que o torna aceitável, porque o 278 nasceu justamente do
modelo «qualquer nó legítimo pode virar o atacante».

**Teste adverso.** `o_nonce_gigante_nao_fica_guardado`: com par legítimo, mandar
512 pulsos válidos com nonce de 1 MiB cada e medir o RSS do nó que confere.
Correção óbvia: recusar nonce fora de um tamanho/alfabeto (o próprio
`desafio::nonce()` produz hexadecimal de tamanho fixo — a régua já existe).

### M2 — Guarda que falha ABERTA e CALADA em duas travas

**Onde.** `pulso.rs:210-214`:

```rust
let Ok(mut v) = self.vistos.lock() else {
    return Ok(());   // trava envenenada -> "pulso fresco e inedito"
};
```

e `cluster.rs:449-454`: `self.provaram.lock().map(|p| p.contains(id)).unwrap_or(false)`
— trava envenenada → «este nó nunca provou» → o TOFU desliga.

Veneno de `Mutex` em Rust é **permanente**. Um pânico em qualquer ponto que
segure uma dessas travas desliga a anti-repetição (ou o TOFU) **para o resto da
vida do processo, sem uma linha de log**. O comentário justifica a escolha
(«recusar aqui derrubaria o cluster inteiro por um panico alheio») e o argumento
é legítimo — o que não é legítimo é ser **calado**: é a mesma lei do
`pagina-dos-pedidos.py`, «quem faz menos do que promete tem de dizer que fez
menos».

**Explorabilidade: baixa** — as duas travas são privadas e os corpos críticos
não têm caminho de pânico além de falha de alocação. É por isso que isto é MÉDIO
e não ALTO. O achado é de **postura**: para a segurança a saída conservadora é
falhar fechado, e quando se escolhe falhar aberto isso vira um `eprintln!` e um
contador, nunca um `return Ok(())`.

**Teste adverso.** `trava_envenenada_nao_passa_calada`: envenenar
`Antirrepeticao.vistos` (uma thread que entra em pânico segurando o lock),
repetir um nonce já usado e assertar que ou ele é recusado, ou o processo
registrou o rebaixamento. Hoje o pulso repetido passa e nada é dito.

### M3 — A inércia da guarda é invisível

**Onde.** `cluster.rs:478-500` — o caminho «sem prova, `exigir_prova_do_pulso`
desligado, nunca provou» é um `return Ok(())` **mudo**.

O `docs/CLUSTER.md` é honesto sobre o alcance («o que ele ainda não alcança,
dito aqui»). O código não tem nada: nenhum log, nenhum contador, nenhum campo. Um
cluster que rodou a versão nova, nunca preencheu `chave_do_fio` e deixou o
interruptor no padrão está **exatamente** tão exposto quanto antes do commit — e
não há uma única evidência em execução que permita a quem opera descobrir isso.
O `config.rs:798-812` recusa publicar o estado por nó no protocolo (decisão
defensável, é mapa para o atacante) — mas o argumento «não conte a quem pergunta»
não vale para o **stderr deste processo**, que é de quem opera.

**Teste adverso.** `aceitar_pulso_sem_prova_deixa_rastro`: subir o nó no padrão,
mandar um pulso sem prova, e assertar que o processo disse alguma coisa (log ou
contador na telemetria). Hoje reprova por silêncio.

### M4 — O TOFU é irreversível dentro da vida do processo, e um escorregão de config vira failover

**Onde.** `cluster.rs:456-460` (`marcar_provado`) × `cluster.rs:490-499` (a
recusa), com `cluster.rs:425` (`campos_da_prova` devolve `None` — em
silêncio — quando falta pino) e `config.rs:1950-1962` (a estática é **gerada
nova** quando o arquivo não existe).

**Cenário.** A provou a B; B marcou A. A é reiniciado sem o arquivo da estática
(volume novo, `base/` recriado, permissão) → A gera outra chave → ou A pára de
assinar, ou assina com chave que não fecha. Nos dois casos B passa a **recusar
todo pulso de A**, para sempre, até B reiniciar. B deixa de ver A vivo → janela
de inatividade → eleição. Um escorregão de configuração em **um** nó vira
failover do cluster, e o lado que errou (A) não tem como saber que parou de
provar: `campos_da_prova` devolvendo `None` não diz nada a ninguém.

**Não é falso positivo porque** o TOFU **não obedece ao interruptor**: com
`exigir_prova_do_pulso: false` ele morde do mesmo jeito. A pétrea «guarda nova
entra pedida, não imposta» foi cumprida no nome do campo e não no comportamento
— existe um caminho em que a guarda é obrigatória e não há como desligá-la.

**Teste adverso.** `o_no_que_perdeu_a_estatica_nao_derruba_o_cluster`: par de nós
com prova válida trocada; matar A, apagar o arquivo da estática, subir A;
assertar o que se quer que aconteça (hoje: B rejeita para sempre e abre eleição).
Independentemente do veredito, `campos_da_prova` devolvendo `None` **depois** de
já ter provado tem de virar aviso no lado que deixou de provar.

### M5 — 339(a): o `endpoint` não tem portão no caminho de ENVIO

**Onde.** `ui/claude.js:321-326` — `perguntar()` faz
`fetch(c.endpoint || ENDPOINT_OFICIAL, {headers: cabecalhos(c.chave)})` e
**nunca consulta `oficial(c)`** (`claude.js:251`). O único aviso mora em
`telaConfig()` (`claude.js:860-862`), isto é, só aparece para quem **navega até
Configurações**. O caminho quente — botão da Claude na tela de Query — não passa
por lá.

**O que realmente segura hoje, e por que isso é frágil.** O CSP da página:
`connect-src 'self' https://api.anthropic.com` (`http.rs:315-320`). Ou seja, a
defesa que impede a chave de sair para um coletor **não está no 339(a)**: está
num cabeçalho de outro arquivo, mantido por outra frente, e nenhum teste do
339(a) a amarra. No dia em que alguém acrescentar uma origem ao `connect-src` —
uma CDN, um segundo provedor de IA, telemetria — o campo `endpoint` vira canal de
exfiltração vivo, sem aviso no caminho de envio, e **nenhum teste acusa**. É o
mesmo padrão da conferência própria do `juntar`/`unir`/`pivotar`: a proteção
existe longe de quem a precisa e parece supérflua para quem passar por ali.

**E o `'self'` não é inócuo.** Um `endpoint` apontando para a própria origem do
PhxSql passa o CSP e manda o `x-api-key` para o servidor — que é exatamente o
que o cabeçalho do arquivo promete que nunca acontece («o servidor **NUNCA** vê a
chave», `claude.js:9-11`). O servidor não registra cabeçalhos (só lê
`authorization` e `x-sessao`, `servidor.rs:8778` e `8911`), então não há
persistência — mas a invariante escrita deixa de valer, e qualquer proxy reverso
à frente passa a ver a chave.

**Segunda metade, respondendo à pergunta como foi feita.** Endereço trocado por
script **não** avisa antes de a chave sair: só avisa se a pessoa abrir
Configurações. E há um caminho de plantio que sobrevive à aba:
`migrarDoDisco()` (`claude.js:189-201`) promove um `endpoint` plantado no
`localStorage` para o cofre da aba **de toda aba nova** (`!cofre[k]` é verdade em
aba recém-aberta), apagando o rastro do disco em seguida. O comentário do desenho
diz que o endereço foi para o cofre justamente para não ser «tubulação
permanente» — a migração recria a tubulação uma vez por plantio.

**Teste adverso.** Em `testes-web/`: plantar
`localStorage["phxsql.ia"]={"endpoint":"http://127.0.0.1:PORTA/coletor"}`,
digitar uma chave, ir **direto** à tela de Query e pedir algo; assertar que ou o
`fetch` não sai, ou a pessoa viu um aviso antes. Hoje sai sem aviso (e o CSP
barra só porque o alvo não é `'self'` nem a Anthropic — troque o alvo para a
própria origem e ele passa).

---

## BAIXO

### B1 — `Assinado::mensagem` não é injetiva

`pulso.rs:100-124`: os campos entram separados por `\n`, o `nonce` é o **último**
campo livre e a transcrição do canal é concatenada **depois** dele. Logo
`(nonce = "N", canal = Some(C))` produz a mesma mensagem que
`(nonce = "N\n"+C, canal = None)`. Não é explorável hoje — a transcrição são 32
bytes crus (raramente UTF-8 válido, e o nonce viaja em campo de texto JSON) e
quem quisesse montar a colisão precisaria conhecer `C`. Mas a canonicalização não
prova o que a documentação diz que prova, e o conserto é o mesmo do M1: prender o
alfabeto e o tamanho do `nonce`, ou prefixar cada campo com o comprimento.

### B2 — A mensagem do teto do aperto diz «mais de 0 MiB»

`phxsql-core/src/fio.rs:562-568`: a frase formata `teto / (1024*1024)`, que com
`TETO_DO_APERTO` (65.536) dá **0**. O operador lê «o outro lado mandou mais de
0 MiB num registro so (65536 bytes de teto)», seguido do conselho «baixe o
tamanho do lote de quem serve ou parta a tabela» — que não quer dizer nada num
aperto de mão. O comentário logo acima explica que os bytes estão ali para quem
compara com o que mediu; os MiB agora atrapalham. Nenhum dos dois testes novos
confere o texto (`replica.rs:930` e `tests/teto-do-aperto.rs:60` conferem só a
classe do erro), então ninguém viu.

### B3 — O bloco `cluster` está fora do conferidor de campo desconhecido

`config.rs:3408+`, `SECOES_CONHECIDAS`: há 15 seções, e `cluster` não é uma
delas. Um `"exigir_prova_do_pulos": true` no `config.json` não gera aviso nenhum
e o interruptor fica desligado em silêncio — bem no passo final que o
`docs/CLUSTER.md` prescreve na migração. Atenuante honesto: o valor efetivo é
publicado no `config_ler` (`config.rs:806`), então dá para conferir de fora.

### B4 — «Some ao fechar a aba» não é verdade para as janelas destacadas

A tela afirma, em texto de segurança: *«ela some quando você fecha a aba»*
(`claude.js:818`). `window.open` de mesma origem (`ui/multitela.js:986`)
nasce com uma **cópia** do `sessionStorage`, e o próprio comentário do desenho
registra isso como ganho de usabilidade (`claude.js:141-143`). A consequência
não registrada é que a chave passa a existir em N cópias, e fechar a aba mãe não
limpa as filhas. Afirmação de segurança mostrada a quem usa tem de valer inteira.

### B5 — Login recusado deixa a senha e a chave privada no DOM

`ui/index.html:2659`: a limpeza de `#s`/`#t`/`#k` está no caminho de sucesso e
**não** no `catch` — decisão documentada e boa para a senha (corrigir uma letra
não pode virar redigitar tudo). Mas `#k` é a **chave privada Ed25519**, ela é
colada e não digitada, e uma entrada recusada deixa-a no documento por tempo
indeterminado. Vale separar os três: senha e token ficam, a chave privada some
(ou some por prazo).

### B6 — Réplay de 60 s depois de um reinício, em cluster sem cifra

A fila de nonces e o `provaram` vivem só na memória (`cluster.rs:213-222`). Com
`cluster.cifra` desligado não há transcrição na mensagem assinada
(`pulso.rs:117-122`), então um pulso capturado é reapresentável enquanto o
carimbo couber na janela de 60 s — e o reinício do alvo zera a fila que o
impediria. Efeito: renovar «vi o master agora» por até 60 s, atrasando uma
eleição legítima. Estreito, e o `docs/CLUSTER.md` já diz que o TOFU e a fila são
de memória; fica registrado porque o réplay em si não está dito.

---

## O que procurei e NÃO achei

Varredura sem achado é resultado. Cada item aqui é uma porta que eu abri e
encontrei fechada.

1. **Segredo em texto puro nos cinco commits.** Varri as linhas adicionadas por
   `println!`, `eprintln!`, `dbg!`, `console.log` e por serialização JSON de
   material sensível. O único `eprintln!` novo no caminho do pulso
   (`servidor.rs:3733`) leva o id do nó e o texto do erro — nenhuma chave,
   nenhum nonce, nenhum HMAC, nenhum material de DH. `prova`, `chave` e
   `assinatura` **já estavam** em `segredos.rs:SEGREDOS`, então o profiler
   redige e o agendador recusa; `nonce` entrou na lista de **falso positivo
   declarado** com o motivo escrito (`segredos.rs:261-269`), que é o
   procedimento certo — nonce é comparado para detectar repetição e escondê-lo
   tiraria justamente o que se compara.
2. **Crescimento sem teto do mapa anti-repetição.** Era a minha primeira
   hipótese (`HashMap` com chave vinda do fio) e ela **morreu**: `aceitar` só é
   chamada depois de o HMAC fechar (`cluster.rs:527-531`, com a ordem comentada
   e justificada) e depois de `estado.no(&id)` provar que o id está na lista do
   cluster (`servidor.rs:4155-4189`). As chaves do mapa ficam limitadas à lista
   configurada. A ordem está certa, e está certa **de propósito**.
3. **Campo do pulso fora da assinatura que o receptor use para decidir.**
   Nenhum. Tudo o que `PulsoDeNo::de_json` lê — `id`, `papel`, `epoca`,
   `posicao`, `incompleta`, `prioridade` — está em `Assinado`; `quando`, `nonce`
   e `para` também; e `quando_ms` (o que a eleição usa como «visto agora») vem
   do relógio **local**, não do fio (`cluster.rs:97`). Fora da assinatura sobram
   `op` e o envelope de credencial, e trocar qualquer um deles não dá nada ao
   forjador que a credencial já não dê.
4. **Segunda porta para `registrar`.** Só existem duas
   (`servidor.rs:3733` e `servidor.rs:4203`) e as duas passam por
   `conferir_identidade` antes. O autor foi atrás do irmão sozinho — a resposta
   do pulso entra no mapa tanto quanto o pedido, e está guardada.
5. **Porta dos fundos do portão de tabela nas operações tocadas hoje.**
   `diferencas` nomeia as duas tabelas em `"a"` e `"b"` — fora do campo
   `"tabela"` que o portão lê — e **paga conferência própria**
   (`servidor.rs:21509-21527`, permissão e sonda de travessia). O inventário de
   `direito_coluna::tabelas_do_pedido` (`direito_coluna.rs:457-535`) já conhece
   `juntar`, `diferencas`, `unir` (inclusive `partes`), `pivotar`, os `destino`
   das cópias de tabela e a descida recursiva do `consultar` por
   `de`/`juntar`/`escalar`/`em`/`existe`. Nenhuma operação nova entrou hoje;
   nenhum campo novo de nome de tabela entrou hoje.
6. **Vazamento de coluna negada pela composição do 392.** O sub-pedido do
   `em`/`escalar`/`juntar` passa por `executar_derivado`
   (`servidor.rs:6966-6976`), que roda o portão **e** o direito por coluna, e
   `varrer`/`buscar` são `PorColuna::Le` — a peneira tira a coluna das linhas
   antes de a composição vê-las. As linhas que chegam ao casamento decimal não
   têm a coluna negada, então `cs::campo` devolve `None` e o `retain` derruba a
   linha: fail-closed, não oráculo. **Ressalva honesta:** isto eu li, não
   executei. O teste que fecharia — e que eu recomendo — é
   `a_composicao_nao_responde_sobre_coluna_negada`: usuário sem leitura em
   `salario`, `{"op":"consultar","de":{"op":"varrer","tabela":"folha"},
   "em":[{"coluna":"salario",…}]}`, assertar recusa ou zero linhas **constante**,
   nunca uma contagem que varie com o valor.
7. **As vinte perguntas sobre coluna negada.** Existem e estão cobertas:
   `recusar_pergunta_sobre_coluna_negada` (`servidor.rs:7139-7248`) recusa
   `onde`, `ordenar`, `colunas`, `expressao`, `tendo`, a chave do índice **e o
   filtro do índice parcial** — este último é o oráculo mais fino do conjunto e
   está fechado com o motivo escrito.
8. **Cripto nova.** Não há. O 278 usa X25519 (RFC 7748), SHA-256 (FIPS 180-4) e
   HMAC-SHA256 (RFC 2104/4231) que já existem nesta casa e já têm vetor oficial;
   o segredo cru do DH passa por `sha256(ROTULO || segredo)` antes de virar
   chave de HMAC (`pulso.rs:132-140`), a separação de domínio está lá, e a
   comparação é em tempo constante (`pulso.rs:173`). Nada a conferir contra
   vetor que já não esteja conferido.
9. **A tabela de segredos do `Debug` do `Cluster`.** O campo novo entrou na
   desestruturação exaustiva (`config.rs:623`, `639`), então o `Debug` continua
   a não compilar se alguém acrescentar um campo e esquecer de decidir o que
   fazer com ele. A `senha_hash` continua `"(oculto)"`.

---

## O que eu bloqueio, e o que não bloqueio

- **Bloqueio A1** — memória reservada antes de autenticação, no lado exposto.
- **Bloqueio A2** — oráculo que entrega quais nós ainda são atacáveis.
- **Não bloqueio** o resto. M1–M5 são dívida priorizada com teste nomeado;
  B1–B6 são registro.
- **Não bloqueio o padrão desligado do `exigir_prova_do_pulso`**, e digo por
  quê: «proteção que quebra todo cliente antigo não é proteção, é estrago» é
  pétrea, e a decisão está certa. O que eu cobro é o M3 — que a guarda diga,
  **em execução**, quando está inerte. Hoje só o documento diz, e documento não
  é evidência de instalação.
