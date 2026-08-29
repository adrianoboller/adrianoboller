# A integração com a Claude, no Centro de Controle

O console passa a conversar com a **API da Anthropic** — para escrever SQL a
partir de português, explicar uma consulta, sugerir índice, e **modelar tabelas
que nascem de verdade** no banco depois que a pessoa confirma.

Tudo mora em **`crates/phxsql-server/ui/claude.js`**, servido pelo `http.rs`
como o `diagrama-er.js` já era. No `index.html` só há duas linhas novas: o item
do menu *Configurações → Integração com a Claude…* e a chamada
`PhxIA.botaoDaConsulta(...)` na tela de Query.

---

## 1. Por que a chamada sai do NAVEGADOR, e não do servidor

A API da Anthropic é **HTTPS obrigatório**, e a **`std` do Rust não tem TLS**.
As saídas possíveis eram três:

| saída | o que custaria |
|---|---|
| acrescentar uma crate de TLS | quebra a primeira regra da casa — zero dependências, `cargo build --offline`, a compilação cruzada para Windows que funcionou de primeira |
| escrever TLS aqui dentro | TLS 1.3 escrito à mão, conferido contra vetor oficial, mantido para sempre — por um recurso de conveniência |
| **não passar pelo servidor** | a escolhida |

A chamada sai do `fetch` da própria tela. E isso não é um contorno: é a opção
**melhor**, porque o servidor deixa de ser um lugar onde uma chave de API
poderia vazar.

**Consequências, todas desejadas:**

- o servidor PhxSql **nunca vê a chave**, **nunca faz a chamada** e **não
  precisa de TLS**. Nenhuma linha de Rust fala com a Anthropic;
- a chave mora no `localStorage` do navegador de quem usa, e é dele. Cada
  pessoa usa a própria chave e paga a própria conta;
- quem abrir o console de outra máquina precisa configurar a chave lá. É o
  preço, e a tela diz isso antes de ligar.

### O que o servidor precisou ceder: uma linha de CSP

A página é servida com `Content-Security-Policy`, e `connect-src 'self'`
sozinho **barraria a chamada antes de ela sair** — sem erro visível para o
script, só uma tela parada. Então a política da **página** passou a listar uma
segunda origem:

```
connect-src 'self' https://api.anthropic.com
```

A folga é a menor possível e está travada por teste:

- vale **só na página** (`text/html`); as respostas de dados continuam com
  `connect-src 'self'` — `a_resposta_de_dados_continua_so_com_a_propria_origem`;
- vale **só para `connect-src`**. Não há `script-src` novo: nenhum script de
  fora entra nesta página, e o `zero dependências` continua valendo também na
  interface — `fetch` nativo, nenhum SDK, nenhuma biblioteca.

O teste `a_pagina_pode_chamar_a_api_da_anthropic` existe porque este é um
defeito que **não aparece lendo o código**: repondo `connect-src 'self'`
sozinho, tudo compila, tudo parece certo, e a chamada morre calada.

---

## 2. O contrato da API, conferido e não lembrado

```
POST https://api.anthropic.com/v1/messages
content-type: application/json
x-api-key: <a chave de quem usa>
anthropic-version: 2023-06-01
anthropic-dangerous-direct-browser-access: true
```

O cabeçalho de acesso direto do navegador é o que a API exige de quem chama de
dentro de uma página; sem ele a chamada volta com erro de CORS. **O nome foi
conferido no código do SDK oficial da Anthropic para TypeScript**
(`anthropics/anthropic-sdk-typescript`, `src/client.ts`, método `buildHeaders`),
que o manda exatamente assim quando `dangerouslyAllowBrowser` está ligado.
Não foi escrito de memória — e é por isso que ele está numa constante só,
`CABECALHO_NAVEGADOR`.

**Modelos oferecidos**, com o padrão primeiro; a escolha é de **custo**, e a
tela diz isso:

| modelo | quando |
|---|---|
| `claude-opus-5` | o padrão — o mais capaz |
| `claude-sonnet-5` | intermediário, custa menos |
| `claude-haiku-4-5` | o mais barato e o mais rápido |

**Streaming**, com `"stream": true`: a resposta aparece enquanto sai, em vez de
a tela ficar parada. São lidos `message_start` (tokens de entrada),
`content_block_delta` (`delta.text`), `message_delta` (tokens de saída) e
`message_stop`. O erro que chega **no meio** do fluxo — HTTP 200 e depois um
`event: error` — também é tratado; quem só olha o código de status não o vê.

**Duas coisas que NÃO se usam, e o motivo:**

- **prefill de assistente** — recusado com 400 nos modelos atuais. O formato da
  resposta se controla pelo `system`;
- **`thinking` com `budget_tokens`** — o campo foi removido e devolve 400. Aqui
  o `thinking` é simplesmente omitido.

**`max_tokens`: 4000.** Chega para um SELECT, para uma explicação e para o
plano de modelagem. Como a leitura é em streaming, um teto maior não travaria a
tela — mas não há por que pagar por um que não se usa.

### Os erros, e o recado que cada um vira

A forma do erro (`{"type":"error","error":{"type":…,"message":…}}`) é
**analisada**, nunca recortada: o que não vira JSON vira o código HTTP e o
tamanho em bytes, porque não há como ler um campo de uma estrutura que não se
lê. Cada recado diz **o que fazer**, e não «erro»:

| código | o que a tela diz |
|---|---|
| 401 | a chave não foi aceita — confira em Configurações |
| 402 | cobrança pendente na conta da Anthropic |
| 403 | a chave não tem permissão para este recurso |
| 429 | limite de uso — espere, ou escolha um modelo mais barato |
| 400 | a API recusou o pedido, com a mensagem dela |
| 5xx / 529 | sobrecarga — não é a sua chave nem o seu pedido, tente de novo |
| rede | não deu para alcançar a API, com o endereço configurado |
| meio do fluxo | a resposta foi interrompida pela API |

**O custo é de quem usa, então a tela mostra o custo:** cada resposta exibe
`usage.input_tokens` e `usage.output_tokens`. Quem paga tem direito de ver.

---

## 3. A chave é segredo do naipe da senha

A regra da casa diz que senha nunca vai em texto puro para arquivo, log ou
resposta do protocolo. A chave da API é do mesmo naipe, e a regra aqui é mais
dura ainda, porque ela nem chega ao servidor:

- a chave **não entra em nenhum pedido ao PhxSql** — nem no corpo, nem em
  cabeçalho, nem em query;
- ela aparece **num lugar só**: o cabeçalho `x-api-key` do `fetch` que vai para
  a Anthropic;
- na tela o campo é `type=password`, e depois de salva só aparecem os **quatro
  últimos** caracteres. Há botão de **remover**.

**As provas.** Além do teste em Rust (`o_servidor_nao_carrega_chave_da_anthropic`,
que procura chave *de verdade* e não a mera menção do prefixo — a dica
`sk-ant-…` dentro do campo não é segredo), o exercício pelo navegador registra
**todo** pedido que a página faz e confere:

- a chave não aparece em nenhum dos pedidos ao PhxSql (53 e 93 pedidos
  conferidos, nas duas baterias);
- a chave aparece nos pedidos à Anthropic — senão o teste não mediria nada;
- `grep` no `servidor.log`, no `acessos.log`, no `blacklist.json` e em todo o
  diretório de dados: nada;
- e o **Profiler do servidor ligado**, que grava o pedido de cada chamada, viu
  **0 eventos** com a chave dentro. Com o defeito reposto, viu 13.

---

## 4. O que viaja para a Anthropic, e o que foi barrado

**Viaja o ESQUEMA** — nomes de tabela, de coluna, tipos, chaves, índices. É ele
que faz a resposta acertar, e sem ele o modelo chuta nomes de coluna.

O esquema é montado com as operações `tabelas` e `esquema` do protocolo, que
passam pelo **portão de permissão normal**. Daí sai uma garantia de graça:
**tabela que este usuário não pode ler não entra no contexto** — e a contagem
diz quantas ficaram de fora, em vez de o contexto fingir que o banco é menor do
que é. Não foi preciso operação nova, e por isso não há portão novo para
alguém esquecer.

**NÃO viaja LINHA de dado.** É o padrão, e ele é o de não vazar. Quem quiser
mandar linhas de exemplo marca uma caixa **por chamada**, e a marcação acende
um aviso vermelho dizendo que o dado sai da máquina.

**E mesmo marcada, dado pessoal sai redigido.** O esquema do PhxSql já carrega
a marcação de LGPD por coluna (`dado_pessoal`: `nao`, `pessoal`, `sensivel`).
A redação percorre as colunas e troca o valor das marcadas por `"***"` —
**por análise do objeto, coluna a coluna**, e não por recorte de texto, que é a
regra da casa: recortar depende de o dado estar escrito de um jeito; analisar e
reserializar, não. A tela informa quantos valores foram redigidos.

**Ninguém descobre depois.** O painel **«o que vai subir»** mostra, *antes* de
enviar, o corpo exato do POST e os cabeçalhos — com a chave mascarada, porque
mostrar o corpo e esconder os cabeçalhos esconderia justamente a parte que é
segredo.

---

## 5. Os cinco recursos

Todos ficam na tela de **Query**, atrás do botão «✦ Perguntar à Claude».

1. **Texto → SQL.** Descreve em português, recebe o SQL. **O SQL gerado nunca
   executa sozinho**: ele cai num editor, e quem aperta *Executar* é a pessoa.
   Está escrito na tela, e é o que o teste trava.
2. **Explicar o SQL.** Cola a consulta, recebe a explicação em português.
3. **Índice / desempenho.** A partir da consulta e do esquema, sugere índices
   ou reescritas — e o `system` **obriga** a resposta a terminar dizendo que é
   *sugestão a MEDIR, não verdade*, e proíbe afirmar ganho em número. A casa
   mede antes de aceitar receita de fora, inclusive quando a receita vem de um
   modelo de linguagem.
4. **Modelar tabelas.** A seção 6.
5. **Executar**, que é da pessoa: o editor manda o texto pela operação `sql` do
   protocolo, que passa pelo mesmo portão de permissão de qualquer pedido.

O `system` de todas ensina o **vocabulário real do motor** — os tipos que
existem (`Bool`, `Int1..Int8`, `UInt1..UInt8`, `Real4/8`, `Decimal(p,e)`,
`Date`, `Time`, `DateTime`, `Str(n)`, `Bin`, `Memo`, `Uuid`, `Uuid256`,
`Sequence`) e o **subconjunto de SQL que a camada realmente entende**, com a
lista do que ainda não existe (AND, OR, LIKE, IN, BETWEEN, IS NULL, DISTINCT,
GROUP BY, JOIN, subconsulta, INSERT/UPDATE/DELETE). Ensinar SQL que o motor não
tem seria pior que não ter o recurso: pareceria funcionar.

> **`Decimal(15,2)`, e não `Decimal{15,2}`.** O analisador de tipos do servidor
> (`valores.rs`) lê a forma com **parênteses**; a forma com chave é só como o
> `Debug` do Rust imprime o tipo quando o esquema é lido de volta. O primeiro
> plano escrito aqui usava a chave e o motor recusou com *«tipo desconhecido»* —
> por isso o `system` ensina a forma que o motor aceita, e a conferência da
> seção 6 pega o resto.

---

## 6. Modelar: a IA propõe, a pessoa confirma, o PhxSql cria

A modelagem deixou de ser texto para copiar à mão. O fluxo é:

```
descreve o negócio
   → a Claude devolve um PLANO em JSON (tabelas, colunas, índices, relacionamentos)
   → a tela CONFERE o plano contra o motor e contra o banco
   → a tela mostra a REVISÃO item por item, dizendo o que vai ser criado
   → a pessoa marca/desmarca e confirma
   → só então o PhxSql cria
   → o diagrama e o dicionário mostram o modelo que acabou de crescer
```

**A linha que não se cruza é a mesma do SQL: nada é gravado sem um clique
consciente.** A tela mostra a contagem — «vai criar N tabela(s) e M
relacionamento(s)» — **antes** de qualquer escrita, e o botão diz *só este
clique escreve no banco*.

### Reuso, e não um segundo caminho de criação

A criação usa as operações que já existem e já são provadas — as mesmas do
editor do diagrama ER:

- **`criar_tabela`** para a tabela, com colunas, tipos, `caption`,
  `dado_pessoal` e índices;
- **`declarar_fk`** para cada relacionamento;
- **`excluir_fk`** e **`excluir_tabela`** para desfazer.

Dois caminhos de criação divergiriam no primeiro campo que alguém
acrescentasse de um lado só. A IA é apenas mais uma **fonte de intenção** para
as mesmas operações.

Os relacionamentos entram **depois** de todas as tabelas, por `declarar_fk`, e
não dentro do `criar_tabela`. Assim a ordem de criação deixa de importar, e uma
FK entre duas tabelas do mesmo plano não depende de qual nasceu primeiro.

### A conferência vem ANTES de a primeira tabela nascer

Um plano com defeito não pode virar meia criação. `conferirPlano` recusa, com o
motivo, e a caixa daquele item fica **travada**:

- **tipo que não existe no motor** — `VARCHAR(80)`, `TEXT`, `SERIAL`. A
  conferência espelha o `valores.rs`;
- **`Str(n)` fora de 1..65535**, **`Decimal(p,e)`** com precisão fora de 1..38
  ou escala maior que a precisão;
- **mais de uma coluna `Sequence`** na mesma tabela — o motor aceita uma só;
- **índice que cita coluna que a tabela não tem** (o sufixo `desc`/`nocase` é
  entendido);
- **coluna repetida**, **tabela repetida no plano**, tabela sem coluna;
- **relacionamento cujo destino não existe** nem no plano nem no banco;
- e o aviso, que não trava: **tabela sem índice primário**.

`Str` e `Decimal` **sem parâmetro** passam com aviso, e não com recusa: o motor
os aceita, caindo em `Str(60)` e `Decimal(15,2)`. Recusar seria mentir sobre o
motor — mas uma decisão implícita de tamanho é coisa que se lê, não que se
descobre.

### As verdades que a tela é obrigada a dizer

- **A chave estrangeira do PhxSql é DECLARADA, e não imposta.** O motor guarda
  a declaração e as telas a usam, mas ele **não confere** a referência na hora
  de gravar. Está escrito na revisão, em cima da lista de relacionamentos.
  Prometer integridade que não existe entrega um estrago com nome bonito.
- **Não existe ALTER de coluna.** Se o plano propõe uma tabela que **já
  existe**, o item é recusado dizendo isso e oferecendo o caminho real: criar
  com outro nome, ou duplicar e recriar. **Nada é sobrescrito em silêncio** — e
  o `system` já instrui a Claude a nunca propor alteração de tabela existente.

### «Exibir conforme for desenvolvendo»

Depois de cada criação confirmada:

- o **diagrama** do estado real do banco é desenhado **ali mesmo**, no painel,
  reusando o `PhxER` do editor de diagrama — nada de uma segunda visualização;
- o desenho é montado **relendo o servidor**, e não desenhando o plano: o que
  interessa mostrar é o que **existe**, não o que foi pedido;
- há atalho para o **dicionário de dados** (`SysColumns`) e para o **diagrama em
  tela cheia**, que veem o mesmo estado — não há dois estados do mesmo banco;
- a **árvore da esquerda** é atualizada junto, pelo mesmo motivo.

### Desfazer

O que **esta rodada** criou fica registrado, e o botão *Desfazer esta rodada*
remove os relacionamentos e as tabelas por `excluir_fk` e `excluir_tabela`.

Tabela recém-criada e vazia é o caso fácil. **Tabela que já ganhou linha segue
a regra normal**: a tela conta quantas linhas há, avisa em vermelho que o dado
vai junto e que não há volta, e **exige um segundo clique**. Não há atalho para
apagar dado por causa de um desfazer.

---

## 7. Guarda nova entra pedida, não imposta

**Sem chave configurada, nada muda.** A tela de Query, o diagrama ER e o
dicionário de dados funcionam exatamente como antes desta rodada: o botão da IA
**não é desenhado**. O item do menu *Configurações* está sempre lá, e é por ele
que a integração se descobre e se liga.

Os testes que mais importam aqui são os do comportamento **velho** — «sem
chave, a tela de Query continua inteira (Consultar + 6 campos)», «sem chave, o
diagrama ER funciona exatamente como hoje», «sem chave, o dicionário de dados
funciona exatamente como hoje».

---

## 8. As provas

Sem chave de verdade não dá para chamar a Anthropic — e inventar uma seria
inventar o resultado. Então o caminho inteiro foi exercitado contra um
**servidor falso** que fala o formato da API, **inclusive o SSE pedaço a
pedaço**, e que encena os erros.

**Bateria 1 — o caminho inteiro (43 provas).** Comportamento velho;
configuração; «Testar a chave»; os quatro recursos; o painel do que sobe; o
streaming aparecendo aos pedaços (4 tamanhos parciais vistos); o SQL caindo no
editor sem executar; o *Executar* da pessoa trazendo linhas; a recusa do motor
com o motivo quando o SQL não tem substrato; as linhas de exemplo com o dado
pessoal redigido; os erros 401/429/500/529/400, o erro no meio do fluxo e a
rede caída; os dois temas; e a chave que não vaza.

**Bateria 2 — a modelagem que cria (31 provas).** O diagrama e o dicionário sem
chave; o plano com tipo inexistente recusado antes de tocar o banco; o plano que
colide com tabela existente; a resposta que não é plano; a revisão com a
contagem antes de escrever; a criação confirmada; **as tabelas existindo de
verdade, provadas pela operação `esquema` e não pela tela**; os índices e a
marcação de dado pessoal gravados; o diagrama e o dicionário mostrando o que
nasceu; a segunda rodada já colidindo com o que nasceu; e o botão *Desfazer*
removendo.

**Bateria 3 — a política real (5 provas).** Sem `bypassCSP`, com o endereço
**oficial** `https://api.anthropic.com/v1/messages`: a chamada **sai** da página
(a política não a barra), leva a chave, o cabeçalho de navegador e a versão, e a
resposta chega à tela.

### Os defeitos repostos, e o que cada um derrubou

Teste que não falha com o defeito de volta é teste que passa por engano — e
esse é pior que teste que falta.

| defeito reposto | o que caiu |
|---|---|
| `connect-src 'self'` sozinho | `a_pagina_pode_chamar_a_api_da_anthropic` (Rust) e a Bateria 3 inteira |
| chave embutida no `claude.js` | `o_servidor_nao_carrega_chave_da_anthropic` |
| chave no corpo de um pedido ao PhxSql (`_chave` no `tabelas`) | «a chave NUNCA aparece em pedido ao servidor PhxSql» — e o Profiler passou a ver 13 eventos com ela |
| criar do plano sem confirmação | «antes de confirmar, as tabelas do plano ainda NÃO existem», e mais duas |

---

## 9. O que exercitar achou, e ler o código não acharia

**`montarArvore()` sem argumento ABRE O PAINEL.** O padrão do parâmetro é
`abrirPainel = true`. Chamado para atualizar a árvore depois de criar as
tabelas, ele jogava a pessoa para fora da tela — a revisão inteira, a lista do
que nasceu e o diagrama sumiam no instante da criação, e a tela voltava para o
Painel. É `montarArvore(false)`. Nada disso aparece lendo a chamada.

**Relógio fixo lê texto cru como se fosse tela pronta.** O plano em JSON leva
vários segundos para chegar pelo streaming. Com `waitForTimeout(3500)` o teste
lia o **texto ainda cru** da resposta e encontrava nele as palavras que
procurava — três provas passavam **por engano**, inclusive a do tipo
inexistente. A espera agora é pelo elemento da revisão, e não pelo relógio.
É a mesma lição do soquete, por outro caminho: *o que depende do tempo se prova
esperando o efeito, não contando os segundos.*

**Teste bom demais acusa o inocente.** A primeira versão de
`o_servidor_nao_carrega_chave_da_anthropic` procurava o prefixo `sk-ant-` na
página — e achava, na **dica** dentro do campo de senha. Uma dica não é um
segredo. O teste passou a procurar chave *com corpo* (vinte ou mais caracteres
do alfabeto dela logo após o prefixo), e continua caindo com uma chave de
verdade embutida. Foi a reposição do defeito que mostrou a diferença.

**`excluir_tabela` deixa restos, e o nome não pode ser reusado.** Ele apaga
`.reg`, `.ndx`, `.bin`, `.memo` e `.log`, mas **deixa `.pag`, `.reason` e
`.trash`**. Com o `.trash` no lugar, criar de novo uma tabela com o mesmo nome
falha com *«dados/<base>/<tabela>.trash ja existe; use Table::abrir»*. Isso
atinge qualquer caminho de criação, não só este — mas o *Desfazer* desta tela
torna o caso muito mais fácil de encontrar (criar → desfazer → criar de novo).
A tela mostra a mensagem do servidor inteira, sem traduzir para «erro».
**Não foi consertado aqui de propósito:** apagar o `.trash` é apagar a lixeira
de linhas excluídas, e isso é decisão de dado — do naipe das que a casa manda
discutir antes.

---

## 10. Limites, ditos com todas as letras

- **Não há chave de verdade neste exercício.** Tudo foi provado contra um
  servidor falso que fala o formato. O que uma chave real acrescentaria é a
  qualidade das respostas — não o caminho, que está exercitado ponta a ponta.
- **A qualidade do plano é do modelo.** A tela confere o que dá para conferir
  contra o motor (tipos, índices, colisão, referência) — ela não julga se o
  modelo de dados é *bom*. Quem confirma é quem responde.
- **O `endpoint` é configurável**, e existe para poder apontar a tela ao
  servidor falso. Como endereço trocado em silêncio seria a forma mais fácil de
  desviar uma chave, a tela de configuração **mostra o endereço** e o marca com
  um pino vermelho *«NÃO é o oficial»* quando ele não é.
- **A integração é por navegador.** Não há como um administrador ligá-la para
  todo mundo de uma vez — e isso é consequência direta de a chave ser de quem
  usa, não um esquecimento.
- **Nada disto é ACID nem replicação.** A folha de marca afirma as duas coisas;
  nenhuma é verdade hoje, e esta integração não muda isso.
