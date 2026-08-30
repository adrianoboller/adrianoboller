# Transações no PhxSql: o desenho, antes do código

Documento de **decisão**, escrito antes da implementação e de propósito. Cada
escolha aqui está amarrada a uma linha de código que já existe, porque o que
mata um desenho de transação neste motor não é a teoria — é uma regra do
formato que ele não pode quebrar.

> **Estado:** desenho escolhido e **não implementado**. O que entrou nesta
> rodada foi o pré-requisito (§8) e este documento. A tela *Ferramentas →
> Gestão de transações* continua dizendo que não há transação, porque não há.

---

## 1. A regra que decide tudo, e o que ela mata

> **O `.reg` nunca reaproveita slot excluído.** (`store/src/reg.rs:15`)

Ela não é preferência. É o que faz percorrer o `.reg` do início ao fim devolver
os registros na ordem em que foram digitados, e é a garantia que o dono do
projeto comprou de propósito, sabendo o preço: espaço morto que só volta com
compactação explícita — que, por sua vez, está recusada com número, porque
compactar renumeraria rowid e **rowid é endereço**.

Daí sai a pergunta difícil de qualquer transação aqui:

> Se `BEGIN; INSERT; ROLLBACK` gravou o slot e consumiu o rowid, e o rowid não
> pode ser reusado, **o que é o rollback de um insert?**

Este documento responde: **não é nada, porque o insert ainda não aconteceu.**
A defesa está na §3.

---

## 2. O escopo: o que uma transação abrange

| | |
|---|---|
| **Pertence a** | uma **conexão** da porta de dados (`Sessao::ligacao`), não a um usuário e não a uma sessão HTTP |
| **Abrange** | várias operações, várias tabelas, **um** database |
| **Não abrange** | mais de um database; a porta web; o que a réplica aplica |

### 2.1 Por que a conexão, e não o usuário

Pelo motivo já escrito em `server/src/carga.rs`, sobre a reserva do
`BULKINSERT`: *«sem um id de CONEXÃO, a reserva só poderia ser identificada
pelo login — e aí duas janelas do mesmo usuário seriam o mesmo dono, o que é
exatamente o contrário de exclusivo.»* Uma transação tem o mesmo problema, com
o mesmo tamanho de estrago.

### 2.2 Por que a porta web fica de fora

Também já está decidido no mesmo arquivo: *«HTTP não tem conexão para cair —
cada pedido é um. Sem ligação a que amarrar, a primeira rede de proteção não
existe.»* Uma transação aberta por uma aba de navegador que o usuário fechou
ficaria pendurada segurando tabelas, e a única rede restante seria o prazo.

A web **não fica sem atomicidade** por isso: ela manda a transação inteira em
**um** pedido (`{"op":"transacao","operacoes":[…]}`), que roda inteiro dentro
de uma tomada da trava. É o mesmo caminho que o `inserir_lote` da tela já usa
hoje, e é por isso que a tela nunca precisou de `BEGIN`.

### 2.3 Por que um database só

Porque a marca de recuperação (§5) mora **dentro do diretório do database**, e
é isso que a faz viajar junto no backup e na restauração. Uma marca que
cobrisse dois databases deixaria de valer no instante em que alguém
restaurasse um deles sozinho — e restaurar um database sozinho é uma operação
que já existe (pedido 133).

Transação entre databases é *two-phase commit*, é outro projeto, e entra na
lista do que falta com esse nome. **Recusa fundamentada, não esquecimento.**

---

## 3. Como desfazer, sem reaproveitar slot

### 3.1 A escolha: nada vai a disco antes do `COMMIT`

Dentro de uma transação, `inserir`, `atualizar` e `excluir` **não tocam em
arquivo nenhum**. Eles entram num **conjunto de escrita em memória** — a lista
ordenada do que a transação quer fazer, na ordem em que foi pedido. O
`COMMIT` aplica a lista inteira numa passada só, com a trava de dados na mão.
O `ROLLBACK` joga a lista fora.

```text
BEGIN            → nasce o conjunto de escrita, vazio
INSERT           → entra na lista; nenhum slot, nenhum rowid, nenhum evento
UPDATE           → entra na lista
ROLLBACK         → a lista é descartada. O disco nunca soube que houve nada
COMMIT           → a lista é aplicada em ordem, numa tomada da trava
```

O rollback de um insert é **zero bytes de trabalho**, e a ordem de digitação
nunca correu risco.

E a ordem de digitação continua sendo a ordem de digitação: as linhas entram
no `.reg` na ordem em que foram pedidas dentro da transação, anexadas no fim.
Duas transações não podem embaralhar essa ordem numa mesma tabela porque não
podem tocar a mesma tabela ao mesmo tempo (§4.2).

### 3.2 A alternativa que foi recusada, e os quatro motivos

A resposta óbvia — **o slot que «nasceu e morreu»**: gravar o slot, e no
rollback marcá-lo com um terceiro status. Foi recusada por quatro motivos, em
ordem de gravidade:

1. **O terceiro status é corrupção para todo leitor que já existe.**
   `store/src/reg.rs:143` é literal: `b == STATUS_LIVRE || b == STATUS_ATIVO`,
   e qualquer outro valor cai no ramo *«status inválido»*. Um valor novo faria
   o `verificar`, o `reparar` e a comparação com o `.bkp` chamarem de
   corrompida uma linha que está exatamente como deveria — e o comentário logo
   acima dessa função conta por que a distinção foi cravada: **um único bit
   trocado apagava um registro em silêncio**, e o reparo nunca ia buscar a
   cópia boa no espelho. Reabrir isso para fazer transação seria trocar uma
   garantia paga por outra.

2. **O status novo é desnecessário: o `.reg` já sabe desfazer um insert, e é
   o que ele faz hoje.** Quando um índice único recusa a chave depois do slot
   gravado, o `inserir` chama `self.reg.excluir(rowid)`, que põe
   `STATUS_LIVRE` e não devolve o slot para a fila. Ou seja: a marca «nasceu
   e morreu» **já existe**, disfarçada de `LIVRE`. O que ela não resolve é o
   item 3.

3. **Na replicação, o slot queimado tem de ser queimado dos dois lados.**
   O `aplicar_evento` para a replicação quando o rowid que a réplica gerou não
   bate com o do evento: *«o source diz rowid N e aqui saiu M»*. Se o master
   queima o slot 700, a réplica precisa queimar o 700 também, senão o próximo
   insert sai 700 nela e 701 nele, e a replicação morre. Queimar o 700 na
   réplica exige **mandar para ela a inclusão e depois a exclusão** — quer
   dizer, **a transação revertida chega aplicada na réplica**, exatamente o
   que não pode acontecer (§6).

4. **O buraco é permanente.** A compactação está recusada com número
   (`DESEMPENHO.md` §4.7.3, e `COMPARACAO.md` sobre rowid ser endereço). Uma
   carga de 2.500 linhas que falha na linha 1 e é revertida deixaria 2.500
   slots mortos para sempre. A carga que motivou esta frente foi exatamente
   essa.

### 3.3 O que a escolha custa, dito antes de alguém descobrir

| Custo | Tamanho | O que se faz com ele |
|---|---|---|
| **Memória** | o conjunto de escrita inteiro fica em RAM | teto configurável (`recursos.transacao_max_linhas`). Estourou, a operação é **recusada com erro nomeado** — nunca engolida, nunca vazada para disco pelas costas |
| **Ler o que a própria transação escreveu** | não entrega nesta rodada | uma consulta dentro da transação **não vê** o que ela mesma inseriu. Está declarado na §4.3 e vai para a tela |
| **Falha no meio do `COMMIT`** | é a única janela que sobra | é a §5, e é por isso que ela é peça da transação e não detalhe |
| **Buraco na sequência** | o `AUTO_INCREMENT` é consumido ao empilhar, não ao confirmar | tem de ser: a coluna da sequência pode estar num índice único, e o `inserir` de hoje já numera **antes** das chaves por essa razão. Um `ROLLBACK` deixa o número queimado — que é o que todo banco faz, e é a única coisa que uma transação revertida deixa para trás aqui |

O terceiro item merece a única mitigação que este desenho tem: **o que pode
falhar é conferido na hora do `INSERT`, não na hora do `COMMIT`.** O `inserir`
de hoje já confere a unicidade **antes** de qualquer gravação, pela mesma razão
de formato — e o comentário lá está escrito com essas palavras. A transação faz
a mesma conferência ao empilhar, contra o índice **e** contra as chaves que ela
mesma já empilhou. Sobrando só falha de E/S no `COMMIT`, a recuperação da §5
tem uma resposta única e simples.

---

## 4. O isolamento: o que se entrega e o que não

### 4.1 Não se pode segurar a trava global entre pedidos

A tentação é grande porque seria serialização de graça: `BEGIN` toma a trava,
`COMMIT` solta. **É a doença que este projeto já mediu.** Em `REPLICACAO.md`
§17, com a trava presa atravessando uma ida e volta de rede numa réplica
cortada em silêncio, `varrer` esperou **29.456 ms** enquanto o `ping` respondia
em 6 ms — o servidor no ar e a trava presa. Uma transação aberta por um cliente
que foi almoçar faria o mesmo, e não por engano de implementação: por desenho.

**Então a transação NÃO segura a trava global.** Ela a toma e solta operação a
operação, exatamente como hoje.

### 4.2 O que substitui: reserva de tabela, sem espera

Ao tocar uma tabela pela primeira vez, a transação a **reserva**, no mesmo
registro que o `BULKINSERT` já usa. Enquanto a transação viver, mais ninguém
**escreve** naquela tabela.

Quem esbarra recebe a recusa na hora, **sem esperar**: erro `EM_TRANSACAO`,
`repetir: true`, nomeando quem segura — o gêmeo do `EM_CARGA` 4002 que já
existe. Não esperar não é preguiça: **é o que torna impossível o abraço mortal
entre duas transações** que peguem duas tabelas em ordens opostas. Sem espera
não há ciclo.

E as duas redes de proteção contra reserva órfã são as mesmas do `BULKINSERT`,
pelo mesmo motivo: a queda da conexão solta, **e** o prazo solta. Uma só não
basta — soquete meio-morto existe, e é justamente o caso em que a primeira não
pega.

### 4.3 O nível, dito sem enfeite

| | |
|---|---|
| **Entre escritores** | **serializável** nas tabelas da transação — ninguém mais escreve nelas, e o efeito aparece de uma vez |
| **Para quem lê** | **read committed**, e sem bloquear: um leitor nunca vê dado não confirmado, porque **não há dado não confirmado em lugar nenhum** — ele ainda está em RAM |
| **Para a própria transação** | **nada.** Ela não tem *snapshot*: entre duas leituras dela, outra transação pode ter confirmado. E ela não vê as próprias escritas |

**Não é ANSI SERIALIZABLE**, e não vai ser chamado assim. O que ele é, com
precisão: *escrita serializável por tabela, leitura confirmada e não
bloqueante, sem leitura repetível.*

E fica registrado o que este desenho **não** compra: paralelismo. A trava única
continua serializando toda operação, uma de cada vez. Transação e concorrência
fina são frentes diferentes; confundi-las é o que faz uma prometer a outra.

---

## 5. Se o processo morrer no meio

### 5.1 Hoje não há marca nenhuma, e isso é verdade

O Aria tem 3 bytes para dizer «não fechei direito», o InnoDB tem o LSN do
checkpoint. Aqui não há nada — e por isso a recuperação não teria como saber o
que reverter. **A marca é peça da transação, e não um detalhe da entrega.**

### 5.2 A marca: `transacao_<id>.tx`, no diretório do database

Antes de a passada de `COMMIT` tocar em qualquer arquivo, o conjunto de
escrita inteiro é gravado num arquivo próprio e **sincronizado**:

```text
base/ → database → transacao_<id>.tx

cabeçalho  [magic PHXTX\0\0\0][versao u32][id u64][carimbo i64]
           [n_operacoes u32][crc32 u32]
operação   [tabela: u16 tam + bytes][op u8][rowid alvo u64]
           [tam payload u32][payload …][crc32 u32]
```

O `rowid alvo` é conhecido **antes** da passada: o `.reg` sempre anexa no fim,
e o próximo rowid é `slots() + 1`. Com a tabela reservada, ninguém pode mudar
isso no meio. É essa previsibilidade que torna a recuperação exata.

A ordem é a mesma que a lixeira já usa e pelo mesmo motivo
(`store/src/lixeira.rs`): grava e **sincroniza** a intenção antes de mexer no
alvo, porque *«a ordem inversa tem uma janela em que o registro não existe em
lugar nenhum, e essa janela não tem conserto depois.»*

### 5.3 A recuperação anda para a frente, nunca para trás

Ao abrir um database, um `.tx` órfão significa: **alguém morreu no meio de um
commit**. A recuperação **completa o commit** — reaplica as operações que
faltam, e apaga o `.tx`.

Nunca desfaz. Não é escolha estética: desfazer exigiria devolver slots já
gravados, que é a regra da §1. Andar para a frente é a única direção que o
formato permite, e o `.tx` é o que torna isso possível — sem ele, não se sabe
para onde ir.

A reaplicação é **idempotente pelo rowid**: cada operação diz o slot que devia
ter escrito e o conteúdo. Slot já ativo com aquele conteúdo — passa adiante.
Slot livre — grava. É por isso que o `.tx` guarda o rowid alvo, e não só a
linha.

### 5.4 O que continua sem cobertura, dito

Uma queda **entre** a última operação da passada e o `unlink` do `.tx` faz a
recuperação reaplicar um commit que já estava inteiro — e ela vai encontrar
todos os slots já certos e não fazer nada. Custa uma varredura do `.tx` no
próximo arranque. É o único caso de trabalho repetido, e ele é seguro.

Uma queda **durante o `fsync` do próprio `.tx`** deixa um `.tx` truncado. Ele
tem CRC por operação justamente por isso: um `.tx` que não confere é um commit
que **nunca começou**, e é apagado. A transação se perde inteira, que é o
resultado correto.

---

## 6. A replicação: uma transação revertida não chega aplicada

### 6.1 O desenho não muda o `.log` — e essa é a resposta

A regra desta frente é dura: *«réplica que não conhece a versão nova continua
aplicando»*. Este desenho a cumpre da forma mais forte possível: **não existe
versão nova.** O `.log` não ganha campo, não ganha flag e não ganha operação.

Isso não é sorte: foi medido contra o formato, e a medição matou a alternativa.

* **Uma operação nova no `.log` quebraria toda réplica antiga.**
  `store/src/log.rs`, `Operacao::de_tag`, devolve `Corrompido` para qualquer
  tag que não seja 1, 2 ou 3. Uma tag `BEGIN` ou `COMMIT` não seria ignorada
  por uma réplica antiga — ela **pararia a replicação** com erro de corrupção.
* **Um identificador de transação não cabe no cabeçalho.** Os 44 bytes estão
  cheios: carimbo 0..8, operação 8, flags 9, origem 10..12, rowid 12..20,
  versão 20..28, usuário 28..32, tam_imagem 32..36, crc 36..40, tempero
  40..44. Os «reservados» do comentário do topo já foram gastos — a `origem`
  ficou nos 2 bytes que sobravam, e é ela que mata o laço do bidirecional.
* **No corpo também não cabe.** O corpo é a imagem da linha, os bytes que a
  réplica grava como dado. Um prefixo de transação ali seria lido como coluna
  por toda réplica antiga.

O único espaço realmente livre é o **byte de flags**, do qual só o bit 0 está
em uso (`FLAG_IMAGEM`), e `Evento::ler` nem o consulta — um bit novo seria
ignorado por réplica antiga, o que é ótimo para uma **dica** e inútil para uma
**garantia**, porque a réplica antiga o ignoraria justamente quando ele
importasse.

### 6.2 Por que não é preciso mexer nele

Porque **a transação aberta não produz evento nenhum.** Nada foi gravado, logo
nada foi journalizado, logo não há o que servir. O `replicar` do master entrega
o que sempre entregou: eventos de escritas que aconteceram.

E o `COMMIT` produz os eventos na ordem, de uma vez, dentro de uma tomada da
trava — indistinguíveis de um `inserir_lote` de hoje para quem os aplica. Uma
réplica de qualquer versão, inclusive uma anterior a esta rodada, aplica sem
saber que houve transação.

**Uma transação revertida não chega aplicada na réplica porque ela não chega,
ponto.** Não há supressão a implementar, nem janela em que a réplica segure
dado que o master vai desfazer.

### 6.3 A prova, e ela é obrigatória

A bancada de Docker (`bancada/replicacao/docker/provar.py`) roda com o daemon
no ar. O roteiro da prova, quando a implementação existir:

| # | O que se faz | O que tem de acontecer |
|---|---|---|
| 1 | `BEGIN`, 2.500 `INSERT`, `ROLLBACK` no master | a `posicao` do diário **não anda**; a réplica não recebe evento; o `slots()` do master não muda |
| 2 | `BEGIN`, 2.500 `INSERT`, `COMMIT` | as 2.500 chegam; retrato SHA-256 de cada linha idêntico dos dois lados; rowid idêntico |
| 3 | Réplica compilada **antes** desta rodada | aplica o commit sem nenhuma mudança — é o teste que mais importa |
| 4 | `kill -9` no master no meio do `COMMIT` | ao subir, o `.tx` completa o commit; a réplica alcança e bate |

O item 3 é o teste do comportamento **velho**, e é a razão de o desenho ter
chegado até aqui sem tocar no `.log`.

---

## 7. O custo para quem NÃO usa transação

**A regra:** se acrescentar algo mensurável ao caminho de quem nunca abre uma
transação, o desenho está errado — e volta para a mesa.

O único acréscimo previsto no caminho comum é **um portão, e ele vem antes de
qualquer trabalho**: um `AtomicUsize` com o número de transações abertas, lido
com `load(Relaxed)`. Zero transações abertas — que é o servidor inteiro hoje —
e nenhuma estrutura de transação é consultada, nenhum `Mutex` é tomado, nenhuma
`String` é montada.

Isto é literalmente a lição que o Profiler cobrou: *«o portão que decide isso
vem ANTES do trabalho»*. O ponto de captura do Profiler desligado fazia dois
`Json::analisar` do corpo inteiro **antes** de perguntar se estava ligado, e
cobrava 7% da carga pela rede. Nenhuma consulta ao mapa de transações pode
acontecer antes do `load`.

E o teste que mais importa não é o do recurso novo: é o
**`sem_transacao_nada_muda`** — quem nunca manda `BEGIN` vê exatamente o
comportamento de hoje, inclusive a mensagem literal do `inserir_lote` sobre as
linhas gravadas antes do erro.

**Como medir, quando existir:**

```bash
cargo build --release --examples -p phxsql-store   # medidor com binário velho mede o passado
cargo run --release -p phxsql-server --example custo-do-portao
```

O molde é o `custo-do-portao`, que já resolve o problema difícil desta medição:
**rodadas intercaladas** (1,2,3, 1,2,3…) em servidores limpos, e a conclusão
dada como *comparação* — enquanto a diferença entre cenários couber dentro do
espalhamento de um cenário sozinho, o resultado honesto é «o portão não
aparece», e não um número.

---

## 8. O pré-requisito, que entrou nesta rodada

A transação vai morar em cima da trava de dados, então a trava precisava ter
**um** dono antes de a transação existir. Ela não tinha.

### 8.1 As 13 tomadas fora do ponto único

`travar_dados()` afirmava, em comentário, ser *«o único lugar que a toma»*.
Era mentira medida: havia **13** `self.dados.lock()` fora dele. Todas as 13
entraram, e nenhuma foi convertida no automático — cada uma respondeu à
pergunta «quem chama isto já tem a trava?»:

| Onde | Quem chama | Já tem a trava? |
|---|---|---|
| `mensagens_atualizar` | `msg`, `texto_do_erro`, `op_mensagens` | não — a «regra de ouro» escrita no próprio arquivo é exatamente essa, e os pontos de uso são portões e montagem de resposta |
| `semear_mensagens` | o arranque e `op_mensagens_semear` | não |
| `posicao_do_diario` | o árbitro do cluster, no ritmo do pulso | não |
| `alcancar_tabela_bidi` | `rodada_bidirecional` | não |
| `atender_http` (`/idiomas`) | o despacho de rota | não |
| `op_mensagens`, `op_idiomas`, `_carga`, `_padrao`, `_exportar`, `_importar` | o despachar | não |
| `descarregar_sujas` | o relógio de gravação e a saída da conexão | não — e a variante `_com` existe justamente para quem tem |
| `executar_rotina` (`CREATE TRIGGER`) | o despachar, via `sql` | não |

Três delas doíam, e é o que a lista de pendências já dizia: o despejo do cache
segurava a trava por uma passada inteira, o corpo de um gatilho pelo tempo que
quisesse, e o laço da replicação **através de uma ida e volta de rede**. Agora
as três aparecem no `espera_ms_s` da telemetria, e o `TELEMETRIA.md` §2.1
voltou a ser verdade.

E a que mais preocupava foi provada **por soquete**, e não por leitura: o
`alcancar_tabela_bidi` é a única das 13 que nenhum teste unitário exercita de
ponta a ponta, porque ela segura a trava atravessando a rede entre dois
servidores. Dois `phxsqld` em modo multi-master (portas 7010 e 7012), 20 linhas
escritas em cada lado ao mesmo tempo: **40 e 40, conteúdo idêntico nos dois**.
É a lição do `BULKINSERT` aplicada de novo — teste unitário não prova o que
depende da rede.

### 8.2 A catraca, porque comentário não conta

O teste `so_um_lugar_toma_a_trava` conta as tomadas **no próprio fonte** — pelo
mesmo `include_str!` do conferidor de textos, e pela mesma razão dele: assim
não há como o teste contar um arquivo e o binário ser compilado de outro. Ele
reprova a décima-quarta, e confere que a que sobrou está mesmo dentro do
`travar_dados`.

*(A primeira versão do teste acusou 4 onde havia 1: o literal escrito no
próprio teste entrava na conta, e as citações nos comentários também. A agulha
é montada, não escrita, e linha de comentário fica de fora.)*

### 8.3 A reentrância deixou de pendurar o servidor

`std::sync::Mutex` não é reentrante, e pedir a trava que a própria thread já
tem parava o servidor **inteiro**, para sempre, sem log e sem pilha. Aconteceu
três vezes neste projeto — a última em configuração padrão, com escrita comum
em duas tabelas.

Agora `travar_dados` pergunta antes, numa `Cell` de thread, e devolve erro
nomeado em vez de parar: o pedido culpado falha dizendo o que houve, e as
outras conexões continuam sendo atendidas. O teste
`a_trava_pedida_duas_vezes_pela_mesma_thread_vira_erro` roda **com prazo**, e
com a guarda removida ele acusa em 30 s em vez de pendurar o `cargo test`
inteiro — foi assim que a prova foi feita.

Ele também confere o outro lado, que é onde uma guarda dessas erra: depois do
`drop`, a mesma thread **volta a conseguir**. Sem isso, a guarda trocaria um
abraço mortal por uma thread aleijada para o resto da vida dela.

O custo dessa guarda está medido em `DESEMPENHO.md` §9. Ela está no caminho de
**toda** leitura e **toda** escrita do servidor, então medi-la não era
opcional.

---

## 9. O que fica para as próximas rodadas, e por quê

| | O que falta | Por que não entrou agora |
|---|---|---|
| 1 | O `BEGIN`/`COMMIT`/`ROLLBACK` em si | o desenho vem antes do código, e é esta rodada. O pré-requisito era o terreno, e ele consumiu a rodada por inteiro |
| 2 | Ler o que a própria transação escreveu | exige sobrepor o conjunto de escrita em **todo** caminho de leitura — que é o antipadrão do «portão espalhado por quarenta operações» que já produziu a porta dos fundos do `juntar` e do `unir`. Precisa de um ponto único de leitura antes |
| 3 | Transação entre databases | *two-phase commit*. Recusa fundamentada, §2.3 |
| 4 | Concorrência fina (trava por tabela) | frente diferente. A transação não a promete, e não depende dela |
| 5 | A tela de Gestão de transações | **não foi tocada de propósito**: nada passou a existir, e ela continua dizendo a verdade. Quando o `COMMIT` existir, é ela que muda — e o texto novo entra pela fábrica de idiomas, com a catraca do `conferidor.rs` baixada no mesmo commit (ela está em **1.999 de 1.999** hoje: qualquer texto cru novo reprova o `cargo test`) |

E a frase que continua valendo até o item 1 existir:

> ***ACID compliant* é falso.** Sem transação não há o **A** nem o **I**. Não
> se repete isso em documento técnico enquanto não houver `COMMIT`.
