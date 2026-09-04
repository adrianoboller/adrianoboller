# O roteiro até 1.0, e depois

As **55 sprints** que o dono definiu, na ordem em que ele as definiu. Este
documento é o roteiro **de registro**: as propostas tiradas de manuais de
concorrentes continuam em `docs/SPRINTS.md`, e o `docs/PLANO.md` continua sendo
o documento histórico da leitura do rusqlite e do contrato do FraseSQL.

> **Caminho crítico imediato: SP000001 até SP000016.** Sem contrato, build,
> transações, FK, durabilidade, concorrência, RID, armazenamento e MVCC, não
> faz sentido avançar para SQL ampliado ou recursos cognitivos.

**Toda mensagem de erro diz de qual sprint se trata**, por decisão do dono
(«prefixo em todas, e conserto os clientes»), no molde do MySQL(R) — o
identificador primeiro, entre colchetes, e depois a frase:
`[SP000008] integridade referencial: …`. A tabela mora no `sprint()` do
`crates/phxsql-core/src/error.rs`, com o motivo escrito em cada braço, e a
regra que a escolheu não é «de que área parece» e sim **qual sprint mudaria
este comportamento**. Há teste que lê este arquivo e reprova sprint citada que
não exista aqui — *número citado é número que não se mede*. Detalhes em
[MENSAGENS.md](MENSAGENS.md).


---

## A ordem de execução, reorganizada pelos gaps medidos

As 55 sprints continuam com **os números que sempre tiveram**, e isso não é
conservadorismo: **treze deles estão dentro das mensagens de erro do motor**
(`SP000001`, `006`, `008`, `010`, `012`, `016`, `018`, `020`, `021`, `025`,
`028`, `029`, `032`). Renumerar faria toda recusa apontar para a sprint errada,
e apontaria **calado**. O que se reorganiza é a **ordem**; o que falta entra
como sprint **nova**, a partir da SP000056.

### Primeiro: as três que nasceram de gaps medidos

| sprint | o que é | por que existe |
|---|---|---|
| ~~**SP000056**~~ | ~~**A bateria confiável**~~ **FEITA** (03/09) | O caso `telemetria` falhava em **4 de 40** execuções isoladas e **5 de 14** com a máquina carregada, trocando de tema entre elas. A causa não era o tema, nem o gestor de threads que a sprint mandava reescrever: `montarArvore()` disparava `abrirAdmin("painel")` num `Promise` que ninguém segurava, e o Painel voltava do `await` escrevendo **por cima** da tela pintada depois dele — janela de **32 ms de mediana**. A guarda para isso já existia e cobria só `abrirAdmin` contra `abrirAdmin`; as ~50 telas que pintam por `folha()` passavam por fora. Depois: **0 de 60**, **0 de 24**, bateria completa 36/36. `docs/TESTES.md` §11 |
| ~~**SP000057**~~ | ~~**`ao_alterar: cascata` executado**~~ **FEITA** (02/09) | Medido à época: o campo aparecia só em `schema.rs` (guardar/serializar) e no `phxsql-cli` (mostrar) — em ponto nenhum de escrita. Hoje a cascata executa, alcança a neta, e a **árvore inteira** é conferida antes da primeira escrita. |
| ~~**SP000058**~~ | ~~**Destravar o `push` e fazer a CI correr**~~ **FEITA (03/09)** | O acesso de escrita foi concedido, e os dois itens destravaram juntos como a entrada previa. **Medido no GitHub, não deduzido:** o workflow `Portoes` tem **11 corridas** na branch, a primeira às 11:53 e a última às 15:09 — 10 verdes e 1 vermelha. A descrição antiga («o push devolve 403, a sessão fala como `EnginePrint` com `push: false`») fica como **história**: era verdade quando foi medida e deixou de ser. E a vermelha achou algo que a máquina parada escondia por três rodadas: uma **colisão de porta entre testes do mesmo binário** — o `porta_livre()` soltava a porta e o comentário dele afirmava ser «o jeito de não brigar com outro teste», quando entre soltar e o servidor tomar corria a construção do `Config` inteiro. **CI que nunca correu não é CI**: a primeira corrida de verdade pagou por si mesma. |

### A ordem recomendada do caminho crítico

1. ~~**SP000056** — a bateria confiável.~~ **FEITA** (03/09), e ela terminou
   contrariando a própria ordem de serviço — com número na mesa. A decisão do
   dono (02/09) era *«reescrever esse módulo com defeito no final»*, e o
   módulo apontado era o **gestor de threads**. **Ele não tinha defeito
   nenhum:** não aparece em nenhuma das reprovações medidas, e as duas
   promessas que só ele faz (grade preguiçosa por causa da largura zero, gesto
   sobrevivendo à volta do relógio) passam em 60 de 60. Reescrevê-lo teria
   custado uma frente e comprado zero.

   O defeito estava na **entrada do aplicativo**: `montarArvore()` terminava
   disparando o clique no nó Painel, e `Promise.resolve(abrirAdmin("painel"))`
   ficava sem dono. `abrirApp()` devolvia, a árvore aparecia — o sinal por
   onde o `entrar()` da bateria dizia «entrei» —, e o Painel voltava do
   `await vPainel()` **32 ms depois** escrevendo `p.innerHTML` por cima de
   quem tivesse pintado no meio-tempo, deixando **título de uma tela e corpo
   da outra**.

   O achado que dói: **a guarda para isso já existia**, com um comentário
   descrevendo o defeito por extenso e admitindo que não tinha prova real.
   Ela não tinha prova real porque não cobria o caso que descrevia — o
   contador era privado do `abrirAdmin`, e a vítima documentada
   (Configurações, `TESTES.md` §9.8) pinta por `folha()`. *Guarda sem prova
   real não é guarda, é intenção.*

   Entregue: a posse do painel promovida para `folha()`, `desenharAba()` e
   `abrirAdmin()`; `montarArvore()` esperando a primeira tela pintar;
   `#app[data-pronto="1"]` marcando o fim da entrada e o `entrar()` esperando
   por ela; e o caso `18-tela-atropelada`, que **segura a op `painel` no fio**
   e por isso prova a corrida sem virar ele próprio um intermitente — reprova
   nos dois temas com o defeito reposto. Continua valendo o que a sprint já
   tinha entregado: o `clicarOuExplicar` (foi ele que disse `achou: false` em
   vez de «timeout», e foi isso que virou o caso) e o portão de sintaxe da
   interface. **Medido depois:** 0 de 60, 0 de 24, e a bateria completa 36/36
   repetidas vezes. Detalhes em `docs/TESTES.md` §11.
2. ~~**SP000006** — read-your-own-writes.~~ **FEITA** (02/09). O endereço
   estava certo: o conserto é o caminho de **leitura** consultando a pilha
   pendente. Uma `Sobreposicao` presa ao *handle* da tabela, preenchida no
   `abrir_travada` — **num lugar só**, porque aplicada no `ler` e esquecida no
   `varrer` ela mostraria a ficha da transação e a lista do disco na mesma
   tela. Medido por soquete: 1→1→2→2 virou **1→2→2→3→2**. Custo para quem não
   usa transação: um teste de `None` antes de qualquer trabalho. Detalhes,
   com as duas imprecisões nomeadas, na §4.4.1 do `TRANSACOES.md`.
3. ~~**SP000057** — `ao_alterar`.~~ **FEITA** (02/09), e ela achou mais do que a
   sprint dizia: o campo não nascia `cascata` — nascia `Restringir` pela API
   Rust e `Cascata` pelo JSON, então **a mesma tabela nascia com integridade
   referencial diferente conforme quem a criasse.** As quatro ações acontecem,
   a cadeia alcança a neta, e o portão custa 64,9× menos que a varredura.
   Fica um item novo: a cascata **escreve em tabela que a transação não
   declarou** — sem corrida, porque a trava serializa, mas um `ROLLBACK` não
   alcança a filha.
4. **SP000016** — MVCC. Desbloqueada pela medição contra o MySQL(R): o que
   ancora a cadeia de versões é a identidade estável da linha, e o `rowid`
   daqui já é isso.
5. **SP000011 e SP000016 — a premissa da inversao CAIU em 04/09, e a ordem
   passou a depender de um numero que ainda nao existe.** A frase abaixo
   justificou a inversao dizendo que «o gap e leitor-com-leitor». Em 04/09 a
   bancada de concorrencia mediu o ESCRITOR pela primeira vez — o par que a
   premissa antiga nunca tinha rodado — e ele **regride: 0,51x com 2
   clientes**. Isso **nao devolve a ordem antiga** (o MVCC tambem nao conserta
   escritor-com-escritor), mas invalida a premissa, e **premissa invalida nao
   sustenta ordem, nem quando a ordem continua certa por outro motivo**.

   **DECISAO do dono, 04/09/2026: a bateria longa do `gravar` roda ANTES de a
   SP000011 comecar.** A SP000016 nao espera por ela — depois das respostas as
   sete perguntas fechadas (`docs/PESQUISA-MVCC-E-FORMATO.md` §8.0) a Sombra
   virou RAM + recusa, com **zero mudanca de formato**, e nao toca na trava.

   O que segue e o registro de 02/09, mantido como historia:

   **SP000011 e SP000016 — a ordem MUDOU, e mudou medida** (02/09). O que
   estava escrito aqui era: *«a SP000011 vem depois da SP000016, porque a
   SP000016 responde parte da escolha»*. **Falso, e o número diz por quê:** a
   medição que confirmou a premissa da SP000011 rodou **N leitores e nenhum
   escritor** — o gap é **leitor-com-leitor**, que é exatamente o par que o
   MVCC não conserta. E a refação que parecia barata está morta: o
   `Mutex<Instancia>` **não protege a `Instancia`** (um campo, todos os
   métodos `&self`), então `RwLock<Instancia>` **compila de primeira e está
   errado** — dois escritores com guarda de leitura abrem dois `Table` sobre
   os mesmos arquivos e o compilador não reclama.

   A ordem recomendada passa a ser, e as duas primeiras **não** precisam de
   máquina parada:
   1. ~~**encurtar as seções críticas**~~ — **o pedaço que dava para fazer está
      feito (03/09)**, e ele mudou de forma: as 5 que rodam código do dono não
      precisavam ser encurtadas, precisavam de **teto**, porque o comprimento
      delas é escrito por quem modela o banco e não aqui. O «teto de passos»
      que se citava não era teto da trava — medido, um gatilho `WHILE TRUE DO
      SET s = CONCAT(s, s)` **abortava o processo** depois de 10,2 s com a
      trava global na mão. Hoje há dois tetos e o pior caso é **500 ms**.
      O mapa de hoje diz **76 seções, 23** com `fsync` (era 24, e não fui eu:
      o gerador não vinha sendo rodado);
   2. escrever o invariante que o `RwLock` violaria, **antes** de qualquer
      refação;
   3. **SP000016** com a decisão de formato **cedo** (`.reg` v6 + área de
      undo fora do `.reg`);
   4. trava por tabela, se sobrar disputa.

   Detalhes e o arnês que **recusa publicar número sujo** em
   `docs/CONCORRENCIA.md`.
6. **SP000005** — decompor o `servidor.rs`, hoje com 22.560 linhas.
7. ~~**SP000009**~~, ~~**SP000010**~~, **SP000012**, **SP000015** — fechar as
   parciais. A **SP000009** fechou em 03/09 junto com a SP000007 e a SP000008:
   eram um corpo só de trabalho no mesmo território, e as duas primeiras
   estavam **envelhecidas** — descreviam como pendentes duas metades que já
   tinham morrido. Remedir o estado antes de planejar valeu mais que o plano.
8. **SP000001** — o contrato, por último no bloco: congelar escopo antes de o
   escopo parar de se mexer seria congelar o errado.

### O que saiu do caminho, e por quê

* **SP000013** — rebaixada a melhoria. Deixou de ser pré-requisito do MVCC:
  medido contra um MySQL(R) real, o que ancora a cadeia de versões é a
  identidade estável da linha mais um ponteiro, e não um identificador novo.
* **SP000014** — **recusada pelo dono**. A pétrea vence: o `.reg` nunca
  reaproveita slot. Não é pendência, é escopo fora.
* **SP000024 (TLS)** — adiada por palavra do dono («pule o TLS, vemos depois»).
  Continua encostando na pétrea de zero dependências.
* **Impressão** e **FX SDK** — recusados com número (pedidos 161 e 160).

### Os blocos seguintes, na ordem que já tinham

SQL relacional (**SP000017–023**), segurança (**024–027**), alta
disponibilidade (**028–033**), 1.0 GA (**034–035**), Phx Contract
(**036–045**) e Cognitive Lab (**046–055**). Nenhum deles começou, e nenhum
muda de lugar: o que os antecede é o bloco crítico, e é ele que foi
reorganizado.

---

## Antes da lista: três decisões que o roteiro exige do dono

Não são objeções ao roteiro. São pontos em que ele **encosta em regra pétrea
desta casa**, e a própria regra manda discutir antes.

### 1. A SP000014 pede o que o `.reg` não faz — e não faz de propósito

> «**A ordem de digitação é sagrada.** O `.reg` nunca reaproveita slot
> excluído. Qualquer proposta que quebre isso precisa ser discutida antes.»
> — `CLAUDE.md`

A SP000014 é «Reuso de espaço, VACUUM e compactação». Reusar slot **é**
reaproveitar slot excluído. E compactar reescreve `rowid`, que aqui **é
endereço**: quem guardou um passa a apontar para outra linha.

Isso já está registrado como **consequência aceita**, não esquecimento
(`docs/COMPARACAO.md`, sobre o `OPTIMIZE TABLE`): uma tabela com muitas
exclusões cresce e não encolhe.

**A decisão é sua**, e ela tem um preço em cada direção. Manter a regra: o
espaço não volta. Relaxá-la: a garantia que o TopSpeed(R) não dava deixa de
existir aqui também.

### 2. A SP000013 é a chave que destrava a SP000016 — e isso é boa notícia

O MVCC está registrado nesta casa como **recusa medida**:

> «**MVCC** — Quebraria o rowid-como-endereço e a replicação por rowid.
> **Decisão pendente do dono**» — `base-de-conhecimento/05-TECNOLOGIAS.md`

A SP000013 é «**RID lógico estável** e formato físico v2». Um RID lógico é
exatamente o que separa «identidade da linha» de «posição da linha no arquivo»
— e é a peça que faz a objeção ao MVCC cair.

Ou seja: **o roteiro já resolve a objeção que esta casa tinha registrado**, e a
ordem SP000013 → SP000016 está certa. O que ele não diz é o custo: RID lógico
é **mudança de formato em disco**, e a regra é que mudança de formato entra
**cedo**, enquanto não há dado em produção.

### 3. A SP000024 troca a cifra do fio por TLS de verdade

Hoje o transporte usa cifra própria estilo Noise (`docs/CIFRA-DO-FIO.md`), e a
recusa de escrever TLS à mão está registrada com motivo: *«TLS mal escrito é
pior que TLS ausente, porque parece seguro»*.

A SP000024 pede TLS 1.3 e mTLS. Isso significa ou **escrever TLS** — o que a
recusa desaconselha — ou **admitir dependência externa**, o que quebra a regra
de zero dependências. Há um terceiro caminho: **proxy obrigatório** (a própria
auditoria externa o lista como alternativa aceitável). Os três têm preço, e
nenhum é gratuito.

---

## PhxSql 0.19 — Fundação

| # | Sprint | Estado medido |
|---|---|---|
| SP000001 | Contrato PhxSql 1.0 e congelamento do escopo | **não iniciado** |
| SP000002 | Build limpo, toolchain fixo e CI obrigatória | **FEITA (03/09)** — `rust-toolchain.toml` pina 1.94.1 (conferido: é o `rustc` que roda aqui, commit `e408947bf`) e `.github/workflows/portoes.yml` roda os três. **Correu, e correu 11 vezes**: a CI passou a existir de verdade em 03/09, quando o `push` destravou. A frase antiga — «nunca correu uma vez: depende do `push`, e o `push` é 403» — fica como história. |
| SP000003 | Proveniência Git, versão e release reproduzível | **feito** — `build.rs` embute commit e árvore suja, `--version` uniforme nos três binários |
| SP000004 | Fonte única de verdade e documentação gerada | **feito** — `CAPABILITIES.json` e os geradores escrevendo README, `TESTES.md` e `REST.md` |
| SP000005 | Decomposição de `servidor.rs` e fronteiras arquiteturais | **não iniciado** — **22.560** linhas (remedido; eram 22.396, e a conversão das telas não passa por aqui — o crescimento é das frentes de servidor) |

## PhxSql 0.20 — Transações e integridade

| # | Sprint | Estado medido |
|---|---|---|
| SP000006 | Visão transacional única e read-your-own-writes | **FEITA (02/09)** — a `Sobreposicao` do `store::table`, presa ao handle e preenchida pelo servidor a partir do conjunto de escrita; cobre `ler`, `varrer`, as cinco paginações, `contar`, `filtrar` e `buscar`. Medido por soquete: 1→**2**→2→3→2. Duas imprecisões nomeadas (ordem do índice para a linha pendente; `Sequence`/`rownum` só nascem no commit) e uma dispensa registrada (o caminho que empilha desliga a sobreposição). Estado anterior: **não iniciado, e MEDIDO** — `bancada/transacoes/visibilidade.py`, por soquete: dentro da transação a própria escrita **não** aparece (1→1), o commit aplica (→2) e o rollback descarta (2→2). Os dois últimos são o que separa *modelo de empilhamento coerente* de *transação com defeito*: o empilhamento entrega o **A** do ACID, falta o **I**. E falta por construção — a escrita fica fora da tabela até o commit e a leitura vai na tabela —, então o conserto é no caminho de **leitura**, consultando a pilha pendente |
| SP000007 | Definição e validação estrutural de chaves estrangeiras | **FEITA (03/09)** — a declaração valida a **estrutura** e agora também o **dado que já está gravado**. `ao_excluir` aceita **só** `restringir`, a chave **nasce conferida**, a chave conferida exige índice dos dois lados, e `redeclarar_chaves_estrangeiras` (`table.rs:549`) **recusa** declarar conferida sobre tabela que já tem órfã, nomeando a linha — a promessa falsa (`verificar: true` que nunca valeu para as linhas antigas) é pior que a ausência dela, porque quem lê o esquema para de perguntar. Confere só a chave que **passa a ser** conferida agora, para que um `ALTER TABLE` que nem toca nela não pague a varredura. **Dois limites, nomeados e não escondidos:** a exigência de índice dos dois lados continua imposta na **gravação**, não na declaração (declarar a chave e criar o índice depois é ordem legítima); e referência para tabela em **outro schema** não é vista por nenhum dos caminhos que perguntam pelo nome. `docs/INTEGRIDADE.md` §2.4 e §6 |
| SP000008 | Execução completa de FK e ações referenciais | **FEITA (03/09)** — as duas metades que esta entrada nomeava **morreram as duas**, e a entrada estava envelhecida nas duas. (1) A imposição não é mais *pedida*: a chave **nasce conferida**, e o `verificar` só serve para o lado contrário. (2) `ao_alterar: cascata` **executa**, alcança a neta, e a **árvore inteira** é conferida antes da primeira escrita (`Table::conferir_a_arvore`). Hoje `conferir_fks` roda em **3** pontos de escrita — `inserir` (`table.rs:2022`), `atualizar` (`:2243`) e `restaurar` (`:2525`) — e `conferir_filhas` em **2** de exclusão, de vez (`:2430`) e **suave** (`:2489`). E a conferência passou a perguntar «esta mãe está **viva**?», e não só «existe?»: a mãe excluída de forma suave continua no `.reg` com a chave no índice, e a filha nascia apontando para ela. NULL satisfaz (MATCH SIMPLE). Custo medido no laço quente: **+7,03 µs/linha (+11,2%)** na chave conferida, **zero** em quem não pediu. `DESEMPENHO.md` §15 |
| SP000009 | FK em todos os caminhos e verificador de consistência | **FEITA (03/09)** — o levantamento dos caminhos de escrita saiu do **código**, não de lista, e está em `docs/INTEGRIDADE.md` §1 com arquivo e linha. Quatro buracos fechados, os quatro medidos por sonda: filha nascendo de mãe morta, `excluir_tabela` matando o pai (`catalogo.rs:414`), declarar conferida sobre órfã, e a réplica divergindo. **A réplica APLICA, ela não JULGA** (`table.rs:787`) — conferindo, ela recusava a filha que a origem já aceitara e `pedidos` ficava com **0 de 2** eventos em duas das três ordens: a guarda causava a perda de dado que existe para impedir. O **bidirecional** caía no mesmo buraco por outra porta (casa por chave, não por rowid) e ali a consequência era pior: o erro subia pelo `?`, a posição nunca andava, e o par de servidores ficava **parado**. E a cascata do source passou a gravar a imagem no diário da filha — sem ela a réplica recusava o evento com «veio sem imagem», nas três ordens. **O verificador** é `crates/phxsql-store/src/integridade.rs` com `--example conferir-integridade`: ele faz as três perguntas (tabela mãe existe, há índice dos dois lados, cada filha tem mãe viva) e **RELATA, não conserta** — consertar dado do dono sem ele pedir é pior que o defeito. `docs/INTEGRIDADE.md` |
| SP000010 | Protocolo de commit e matriz real de durabilidade | **FEITA (03/09)** — a matriz cruza os cinco pontos de morte (antes do `fsync` da marca, depois dele e antes da 1ª tabela, entre operações/tabelas, entre a última operação e o `unlink`, no meio da cascata do `ao_alterar`) com os três regimes, provada por `SIGKILL` de processo real, nunca teste unitário (`bancada/durabilidade/prova.py`). **Achado central, medido e não suposto**: os quatro primeiros pontos têm a mesma garantia nos três regimes — `gravar_marca` sincroniza sempre, incondicional ao regime, e uma queda de PROCESSO nunca perde um `write` que o kernel já recebeu. O regime só decide **quanto tempo a marca fica pendurada** depois de um commit que não caiu: `por_operacao` nunca deixa (0/0/0 numa checagem a 1,25 s), `por_lote` fecha com o relógio de fundo (1/1/0), `sistema` nunca fecha sozinho (1/1/1). **A matriz achou um quinto caso, não previsto pelo documento**: a cascata pode deixar o **índice da filha** sujo (write-back do `.ndx`, mecanismo geral — `DESEMPENHO.md` §4.8), e a recuperação recusa cascatear para ela em vez de arriscar; medido em 21 corridas (1.200 filhas), 9 caíram nesse caso e as 9 saíram **denunciadas** em `operacoes IMPOSSIVEIS` — zero cascatas parciais em silêncio. `docs/TRANSACOES.md` §5.5.3 e §5.7 |

## PhxSql 0.21 — Concorrência e armazenamento

| # | Sprint | Estado medido |
|---|---|---|
| SP000011 | Remoção do `Mutex<Instancia>` global | **parcial, e a premissa agora está MEDIDA** — a trava virou **ponto único** (catraca `so_um_lugar_toma_a_trava`), mas continua **global**, e isso custa: `bancada/concorrencia/a-trava-serializa.py`, com **controle**. Com 2 clientes e metade da máquina ociosa, o mesmo caminho entrega **1,99×** no `ping` (que não toma a trava) e **1,51–1,59×** no `varrer` — a trava come ~20% do paralelismo disponível na leitura e ~25% na escrita já com dois clientes. O controle é o que separa este resultado de um palpite: esta casa já culpou um mutex sem medir e errou por 262.000×. O que a medição **não** diz é qual desenho substitui a trava — trava por tabela, `RwLock` e MVCC (SP000016) são três respostas, e escolher entre elas é outra medição. `docs/DESEMPENHO.md` §14.<br><br>**Etapa 1 fechada em 03/09**, e ela mudou de forma: as 5 seções que rodam código do dono ganharam **teto** em vez de serem encurtadas — o comprimento delas não é escrito aqui. Medido, o «teto de passos» que se citava não era teto da trava: um `BEFORE` com `CONCAT(s, s)` **abortava o processo** após 10,2 s com a trava global na mão, e o pior caso hoje é **500 ms** (57× menos).<br><br>E o `fsync` sob a trava, que a §8 do `CONCORRENCIA.md` listava como não medido, está medido: **1.267–1.371 µs por gravação, 10,3×–12,3×**. Com ele veio o número que reordena o resto — **uma leitura segura a trava 23× mais tempo que uma gravação** no padrão `por_lote` (3.122 contra 137 µs). O «favorece o `RwLock`» da §14 deixou de ser inferência. `docs/CONCORRENCIA.md` §1.4 e §7.1 |
| SP000012 | Deadlock, cancelamento e governança de recursos | **parcial, e agora CONTADO** (03/09). Abraço mortal: três guardas, e o mapa confirma **0 de 76** seções na classe `rede-ou-espera`. Cancelamento: **4 de 76** seções têm ponto de cancelamento (`Atividade::siga`) — nas outras 72, mandar parar não para. Governança: os nove tetos de `Recursos` existem e são lidos, e o buraco que nenhum cobria era a **memória que o código do dono aloca** — um gatilho `BEFORE` podia alocar até o alocador falhar e ABORTAR o processo. Fechado com `TEXTO_MAX` + prazo de parede. Fica **nomeado e não feito**: levar o `Atividade::siga` até o interpretador, que exigiria o `phxsql-sql` conhecer a telemetria do servidor. `docs/CONCORRENCIA.md` §7.2 |
| SP000013 | RID lógico estável e formato físico v2 | **não iniciado, e REBAIXADO a melhoria** — deixou de ser pré-requisito do MVCC. A premissa foi medida contra um MySQL(R) 8.0.46 de verdade (`bancada/mvcc/premissa-innodb.sh`): o que ancora uma cadeia de versões é a **identidade estável da linha** mais um ponteiro para a versão anterior, e não um identificador novo. O `rowid` daqui já é isso, por construção — o `.reg` nunca reaproveita slot. *A pétrea que parecia atrapalhar é o que torna o MVCC mais fácil aqui* |
| SP000014 | Reuso de espaço, VACUUM e compactação | **RECUSADO POR DECISÃO DO DONO**, e não «hoje» — a pétrea vence: «a ordem de digitação é sagrada, o `.reg` nunca reaproveita slot excluído». O que ela compra é o que se perderia: `rowid` como identidade estável para sempre, que é o que a replicação usa para identificar linha e o que o `.trash`/`.reason` apontam. Espaço de linha excluída volta só em reconstrução explícita. **Não é pendência: é escopo fora**, e a 0.21 fecha sem ela |
| SP000015 | Buffer pool, checkpoints e group commit | **parcial** — cache de páginas (2,40×) e group commit (2,63×) medidos |
| SP000016 | MVCC e níveis de isolamento | **DESBLOQUEADO pela medição** — não depende mais da SP000013. Medido no oráculo: leitura aberta antes da escrita alheia continua vendo a versão velha sem bloquear (100 → 100 → 200, o 200 só após o próprio commit), as versões velhas acumulam enquanto a leitura está aberta (*history list* 7 → 207) e são recolhidas quando ela fecha (26 s → 0, demonstrado uma vez; uma segunda tentativa ficou inconclusiva e isso fica dito). O caminho aqui é cadeia de versões ancorada no `rowid` + área de undo, no molde do InnoDB |

## PhxSql 0.22 — SQL relacional

**A coluna abaixo estava vazia — nunca tinha sido remedida.** Remedida em
03/09: das sete, **cinco têm trabalho real e testado** (`crates/phxsql-sql`,
89 testes unitários verdes — `flock /tmp/phx-cargo.lock cargo test -p
phxsql-sql`), e só duas continuam do zero. «Vazio» era mentira por omissão.

| # | Sprint | Estado medido |
|---|---|---|
| SP000017 | Contrato SQL, lexer, parser e AST | **PARCIAL (03/09)** — lexer (`lexico.rs`, 495 linhas, 11 testes), parser e AST do `SELECT` (`sintaxe.rs`, 771 linhas, 16 testes: `Selecao`, `Alvo`, `Condicao`, `Literal`, `Ordenacao`, `Projecao`), mais o parser próprio de `CREATE TRIGGER`/`CREATE PROCEDURE` (`rotina.rs`, 3203 linhas, 34 testes) e o dos comandos de transação (`transacao.rs`, 540 linhas, 11 testes) — total 89 testes, todos verdes. Ligado ao servidor pela op `{"op":"sql"}` (`servidor.rs:9644`), com erro citando a coluna. **Falta:** o contrato não cobre DDL de tabela nem DML como texto SQL — ambos recusam nomeando o motivo (ver SP000020/021), então «contrato» aqui é o de SELECT + transação + rotina, não o de SQL inteiro |
| SP000018 | Binder, catálogo, tipos e coerção | **PARCIAL, pequeno (03/09)** — `traduzir()` (`traduzir.rs:99`) recebe um catálogo mínimo (`IndiceInfo`: nome, colunas, único, primário) buscado do `esquema` do servidor a cada consulta (`servidor.rs:9704`), e resolve `WHERE`/`ORDER BY` contra ele, recusando e listando os candidatos quando falta índice (`traduzir.rs:162-178`). Coerção: literal numérico do lexer é sempre texto (decimal exato), e o motor foi alargado para aceitar inteiro-como-texto em coluna `Int4` (`crates/phxsql-server/src/valores.rs`, teste `numero_continua_valendo_exatamente_como_antes`), achado ao ligar a op `sql` (`docs/SQL.md` §5). **Falta:** nenhum catálogo/tabela de símbolos mora no crate (é buscado ad hoc por consulta), colunas do `SELECT` (fora do filtro/ordem) nunca são validadas contra o esquema, e não há verificação de tipo além da coerção do literal do `WHERE` |
| SP000019 | Expressões, NULL e funções | **PARCIAL, em dois territórios (03/09)** — no corpo de gatilho/procedimento existe um avaliador completo: `Expr` com literais, `NULL`, chamadas de função (`rotina.rs:501-524`: `CONCAT`, `UPPER`/`UCASE`, `LOWER`/`LCASE`, `TRIM`, `LENGTH`/`CHAR_LENGTH`, `ROUND`, `ABS`, `COALESCE`/`IFNULL`), aritmética decimal exata (mantissa `i128`, sem `f64`) e `IS NULL` funcionando (`rotina.rs:2320`, comentário «se achou compara com IS NULL»), coberto por parte dos 34 testes de `rotina`. **No SELECT/WHERE geral, nada**: `WHERE preco * 1.1 > 100` não tem quem avalie (doc do `lib.rs`), e `IS NULL` é explicitamente recusado ali (`sintaxe.rs:532`) — o avaliador não alcança consulta nenhuma |
| SP000020 | DDL SQL e catálogo transacional | **PARCIAL (03/09)** — `CREATE TRIGGER`/`CREATE PROCEDURE`/`DROP` existem, testados e ligados ao servidor (`crates/phxsql-server/src/rotinas.rs`, 664 linhas, 8 testes; `docs/TRIGGERS.md`, 530 linhas, «17 recusas nomeadas e testadas» — pedido 49 do `PENDENCIAS.md`). **Falta:** `CREATE`/`ALTER`/`DROP TABLE` via SQL não existe — recusado nomeando a saída («Tabela se cria pela operação `criar_tabela` do protocolo», `rotina.rs:846-848`); e o catálogo (`information_schema`) só se lê pela op JSON própria (`sistabelas`/`siscolunas`, `servidor.rs:6181-6182`), nunca por `SELECT … FROM information_schema`. Nenhuma evidência de que o DDL de rotina participe de transação SQL |
| SP000021 | DML SQL, prepared statements e transações SQL | **PARCIAL — metade feita, metade não (03/09)**. Transações: **completo e testado** — `BEGIN`/`START TRANSACTION` com `SCOPE`/`SCOPE MODE`/`TIMEOUT`/`LOCK TIMEOUT`/`STATEMENT TIMEOUT`/`LOCK MODE`, `COMMIT`, `ROLLBACK`, `SAVEPOINT` (`transacao.rs`, 540 linhas, 11 testes), ligado ao servidor (`servidor.rs:9669`) com teste de integração pelo próprio `op_sql` (`servidor.rs:22886`). DML de topo: **recusado pelo nome** — `INSERT`/`UPDATE`/`DELETE` como comando solto não existem (`sintaxe.rs:269`); só há `INSERT`/`SELECT…INTO` dentro de corpo de rotina. Prepared statements: **não existe** (nenhum `PREPARE`/`EXECUTE`; `Preparada::preparar` de `servidor.rs:11881` é de restauração de backup, sem relação com SQL) |
| SP000022 | Operadores relacionais avançados | **NÃO INICIADO no nível SQL (03/09)** — `JOIN`/`INNER`/`LEFT`/`RIGHT`, `DISTINCT`, `GROUP BY` geral, `AND`/`OR`, `LIKE`, `IN`, `BETWEEN` são todos reconhecidos pelo léxico só para serem recusados, cada um com motivo próprio (`sintaxe.rs:306,317-326,334,520-533,539`). As operações embaixo (`juntar` com sete formas, `unir`, `pivotar`) já existem no protocolo e até têm UI própria (pedido 91, clicar no diagrama de Venn) — mas nenhuma é alcançável escrevendo SQL |
| SP000023 | Estatísticas, otimizador, `EXPLAIN` e conformidade SQL | **NÃO INICIADO (03/09)** — nenhuma referência a `EXPLAIN`, `ANALYZE`, estatística ou custo em `crates/phxsql-sql/`. O tradutor documenta a ausência do lado de dentro: «não há planejador: se houvesse dois candidatos, o primeiro declarado venceria» (`traduzir.rs:224`). Nenhum teste do crate cobre otimização ou conformidade |

Uma frase resumia isto como «o começo da camada: `SELECT` simples traduzido
para as operações que já existem no protocolo» — e ela envelheceu junto com a
coluna vazia que a remedição acabou de encher. O resumo certo é outro, e as
cinco parciais acima o dizem juntas: **o SQL não alcança o que o motor já sabe
fazer.** `juntar`, `unir` e `pivotar` existem, têm tela e têm teste, e nenhuma
é escrevível como `JOIN`; `INSERT` funciona dentro do corpo de uma rotina e é
recusado como comando solto; o avaliador de expressão que atende `CONCAT` e
`COALESCE` num gatilho não atende um `WHERE`. Não falta motor: falta o caminho
do texto SQL até ele.

Os números de cada sprint ficam **só na tabela acima**, e é lá que se
atualizam. Repeti-los aqui seria a segunda contagem da mesma coisa — o jeito
clássico de dois pedaços do mesmo documento discordarem, que já custou ao
dossiê um painel dizendo 28.914 enquanto a seção ao lado dizia 34.048.

## PhxSql 0.23 — Segurança

| # | Sprint |
|---|---|
| SP000024 | TLS 1.3 e mTLS em todos os canais — *ver decisão 3 acima* |
| SP000025 | Secure-by-default, identidade e autorização |
| SP000026 | Criptografia em repouso, gestão de chaves e auditoria inviolável |
| SP000027 | Fuzzing, revisão criptográfica e correção de segurança |

A criptografia em repouso **já existe** por coluna marcada (0,10 µs/linha,
escolha (c) do dono entre quatro medidas); o que a SP000026 acrescenta é gestão
de chaves e auditoria inviolável.

## PhxSql 0.24 — Alta disponibilidade e ecossistema

| # | Sprint |
|---|---|
| SP000028 | Replicação de produção em WAN e entre versões |
| SP000029 | Consenso, quorum commit e cluster sem split-brain |
| SP000030 | ODBC completo e drivers oficiais |
| SP000031 | Portabilidade real: Windows, Linux, macOS, ARM, Android e iOS |
| SP000032 | Backup incremental, PITR e upgrades N/N-1 |
| SP000033 | Observabilidade, capacidade e benchmark certificado |

A SP000030 tem uma dívida já nomeada: o driver responde `SQL_TC_NONE` —
«sem transações» — e o servidor **tem** transações desde a frente 37, o que faz
disso lacuna do driver e não limite do motor (`docs/ODBC.md`).

## PhxSql 0.99 e 1.0

| # | Sprint |
|---|---|
| SP000034 | Pilotos reais, UX, documentação e congelamento funcional |
| SP000035 | Auditoria independente e liberação PhxSql 1.0.0 GA |

## PhxSql 2.x — Banco declarativo e adaptativo

| # | Sprint |
|---|---|
| SP000036 | Esquema semântico, LGPD, retenção, residência e acesso |
| SP000037 | Linguagem Phx Contract |
| SP000038 | Compilador de garantias |
| SP000039 | Nó local-first |
| SP000040 | Conflito por tipo de dado |
| SP000041 | Consistência adaptativa |
| SP000042 | Branch, diff, merge e time travel |
| SP000043 | Reactive SQL |
| SP000044 | PhxSql Guardian |
| SP000045 | IA governada |

## PhxSql 2.x — Cognitive Lab

| # | Sprint |
|---|---|
| SP000046 | Arquitetura do Cognitive Lab |
| SP000047 | Telemetria e Evidence Store |
| SP000048 | Anomalias e causa-raiz |
| SP000049 | Soluções candidatas |
| SP000050 | Cognitive Sandbox Copy-on-Write |
| SP000051 | Captura e replay de carga |
| SP000052 | Benchmark comparativo reproduzível |
| SP000053 | Guarantee Gate |
| SP000054 | Cognitive Advisor |
| SP000055 | Change Manager e rollback |

---

## Como este documento não envelhece

A coluna «estado medido» é a que mente primeiro. Ela sai de conferência contra
o código, e cada linha diz **onde** conferir. Quando um sprint fechar, o estado
muda aqui **e** no `docs/PENDENCIAS.md`, que continua sendo a fonte da página
dos pedidos.

E vale a regra que esta casa já pagou duas vezes: *a lista do que falta também
é palpite até alguém medir*. O item da trava ficou dizendo «12 tomadas fora do
ponto único» depois de a frente 29 tê-las zerado — com a receita da medição
escrita ao lado, por refazer.
