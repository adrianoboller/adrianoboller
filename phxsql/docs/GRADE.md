# A grade de dados do console

O console mostra dado em quatro lugares, e os quatro são a **mesma peça**: o
`phx-grid`, do ecossistema Phoenix — ES5 estrito, arquivo único, zero
dependência. Ele vive em `crates/phxsql-server/ui/grid/` e entra no binário por
`include_str!`, junto com a página.

Este documento responde três perguntas: **o que a grade sabe fazer**, **o que
cada tela liga**, e **o que ficou de fora e por quê**.

---

## 1. Onde ela aparece

| Tela | Onde no código | De onde vem o dado |
|---|---|---|
| **Conteúdo** da tabela | `ligarConteudo()` | `varrer` no PhxSql, até o teto de **Trazer** |
| **DbLink → abrir tabela** | `abrirTabelaDbl()` | `dblink_ler` num MySQL/Postgres de fora |
| **DbLink → consulta SQL** | o `#btRodar` de `telaSqlDbl()` | `dblink_consultar` |
| **Junção** e **União** | `desenharGradeDeLinhas()` | `juntar` / `unir` |

---

## 2. O que cada tela liga

Levantado lendo cada `PhxGrid.criar` e conferido no navegador.

| Recurso | Conteúdo | DbLink tabela | DbLink SQL | Junção/União |
|---|:--:|:--:|:--:|:--:|
| Ordenar por coluna | ✅ | ✅ | ✅ | ✅ |
| Paginar (50/100/200) | ✅ | ✅ | ✅ | ✅ |
| Funil por coluna (estilo Excel) | ✅ | ✅ | ✅ | ✅ |
| Chips do filtro ativo | ✅ | ✅ | ✅ | ✅ |
| Busca global | ✅ | ✅ | ✅ | ✅ |
| Arrastar para agrupar | ✅ | ✅ | ✅ | ✅ |
| Agregado por grupo, rodapé e total geral | ✅ | ✅ | ✅ | ✅ |
| Redimensionar coluna | ✅ | ✅ | ✅ | ✅ |
| Reordenar coluna | ✅ | ✅ | ✅ | ✅ |
| Esconder coluna | ✅ | ✅ | ✅ | ✅ |
| **Linha de filtro** no cabeçalho | ✅ | ✅ | ✅ | ✅ |
| **Congelar coluna** | ✅ `rowid` | ✅ 1ª | ✅ 1ª | ✅ 1ª |
| **Exportar a vista** | ✅ | ✅ | ✅ | ✅ |
| **Layout lembrado** | ✅ | ✅ | ❌ | ❌ |
| **Selecionar linha + ação em lote** | ✅ | ❌ | ❌ | ❌ |
| **Duplo clique abre a ficha** | ✅ | ❌ | ❌ | ❌ |
| Bandas (cabeçalho em dois níveis) | ❌ | ❌ | ❌ | ❌ |

Os três ❌ de baixo têm motivo, e não são esquecimento:

- **Seleção e ficha só no Conteúdo** porque marcar uma linha só serve se houver
  o que fazer com ela, e o que se faz é `excluir`/`atualizar` **por `rowid`**.
  Uma junção não tem `rowid` (as linhas são de duas tabelas), e uma tabela de
  fora tem a chave do outro motor, que este servidor não altera — a ligação
  DbLink nasce somente-leitura.
- **Layout lembrado só onde a forma se repete.** `conteudo:<db>.<tabela>` e
  `dblink:<ligação>.<base>.<tabela>` descrevem sempre o mesmo conjunto de
  colunas. Um `SELECT` e uma junção mudam de colunas a cada execução: guardar a
  largura de um seria guardar a largura de outro.
- **Bandas** agrupam colunas relacionadas sob um título comum (2024 → Q1…Q4).
  Nenhuma das quatro telas sabe quais colunas são irmãs — o esquema do PhxSql
  não tem esse conceito, e inventar por prefixo do nome erraria. Fica para
  quando a tabela dinâmica precisar.

---

## 3. A janela: o que a grade enxerga não é a tabela

**Este é o ponto mais importante deste documento.**

`varrer` não tem `WHERE`. Não há pushdown de filtro, de busca nem de
agrupamento — o servidor devolve uma página e a grade faz o resto **em memória,
sobre o que recebeu**. Então:

> filtro, busca, agrupamento, agregado, total geral e exportação respondem
> sobre as **N linhas que a tela trouxe**, e não sobre a tabela.

Medido na tabela de prova (20.000 pedidos, 2.500 em Blumenau, teto padrão de
200 linhas): filtrar `cidade = Blumenau` dava **25**. Nada na tela dizia que
25 era de 200, e não de 20.000. **Filtro que a tela aplica e o servidor ignora
é filtro que mente quando a página vira.**

Não dá para consertar isso na tela — o `WHERE` teria de existir no protocolo.
O que entrou foi **dizer**: sempre que `devolvidas < visíveis`, a aba Conteúdo
abre com uma tarja acima da grade contando a conta, e o seletor **Trazer**
ganhou a opção de 20.000 linhas. É a mesma regra do `recursos.cache_paginas`
que não era lido: campo que promete o que não faz é pior que campo ausente.

O caminho de verdade está descrito em §6.

### O que o contrato remoto já prevê e o servidor não atende

A grade envia, em toda carga:

```js
{ pagina, tamanho, ordem:{campo,dir,tipo}, filtros:[…],
  grupos:[…], dirsGrupo:[…], rodapeGrupo, recolhidos:{…},
  aggCols:[{campo,agregador}], tiposCampos:{…} }
```

e a busca global serializa `{campo:"*", termo, campos:[…]}` — o *pushdown* está
desenhado desde a 0.7.0. **Nenhuma das quatro telas usa fonte remota**: as
quatro passam `dados:` (vetor local). Então o servidor não *ignora* o contrato —
ele nunca o recebe. Ligar `fonte:` sem o `WHERE` do outro lado seria trocar uma
mentira silenciosa por outra, com uma ida à rede a mais por página.

---

## 4. Os recursos que entraram nesta rodada

### Congelar coluna

`api.congelar(campo, "esq"|null)`, e o alfinete **◧** no menu **Colunas** do
rodapé. A coluna congelada **vai para a ponta da ordem**: o `sticky` gruda no
lado do contêiner e não no lugar da coluna, então congelar a quinta sem movê-la
faria as quatro da esquerda passarem por baixo dela.

O alfinete mora no menu de Colunas, e não no cabeçalho, por um motivo prático:
a coluna que dá vontade de congelar já saiu da tela quando a vontade aparece.

Medido: rolando 1.200 px para a direita, o `rowid` fica em `left: 0` e o
`pedido` sai da tela.

### Linha de filtro no cabeçalho

`filterRow: true`. Texto contém, número com operador (`>` `>=` `<` `<=` `=`
`!=`), data exata, e `<select>` de valores para coluna de tipo *badge*. É o
filtro que um usuário de banco usa o dia inteiro, e estava escrito no
`phx-grid` desde a 0.6.0 — **desligado em todas as telas**.

### Seleção com ação em lote

`selecao: true, chave: "rowid"`. Com **chave**, e não por posição: sem `chave` a
grade marca "a terceira linha da página", e virar a página levaria a marca
junto. Clique marca, Shift marca a faixa, a caixa do cabeçalho marca a página.

A faixa de ações aparece **só quando há linha marcada**, com: copiar os rowids,
**excluir marcadas** e desmarcar. O excluir é o **reversível** (o `excluir` do
protocolo, que a Lixeira desfaz) — excluir de vez em lote, sem uma linha aberta
na frente, é estrago rápido demais para um botão.

### Exportar a vista

Botão **⤓ Exportar a vista** no rodapé; `api.vistaAtual(cb)` e
`api.csvDaVista(v)` para quem preferir a API.

Não é a mesma coisa que a tela **Exportar**: aquela leva a tabela como está
gravada; esta leva **estas colunas, nesta ordem, com este filtro e esta
ordenação**. Quem passou vinte minutos filtrando quer levar o resultado, não
recomeçar na planilha.

Três decisões:

- pede à fonte o **conjunto inteiro**, e não a página — exportar a página 1 de
  40 seria a mesma mentira do filtro truncado;
- CSV com `;` e BOM, que é o que o Excel em português abre sem perguntar nada
  (com vírgula ele joga a linha inteira numa célula);
- **valor cru, não formatado** — `R$ 1.234,56` volta como texto e ninguém soma
  a coluna.

Provado pelo caminho inteiro (clique → Blob → `<a download>` → arquivo): 44
linhas na vista, 44 no arquivo, só `status = pago`, ordem decrescente por
valor, `observacao` de fora porque estava escondida.

### Layout lembrado

`lembrar: "<chave>"`. Guarda em `localStorage` **largura, ordem, colunas
escondidas, colunas congeladas e itens por página**.

**Filtro e ordenação não**, de propósito: um filtro que volta sozinho ao
reabrir a tela é a mesma mentira do filtro truncado, com uma noite de
intervalo. Layout é gosto; filtro é pergunta, e pergunta se refaz.

Tudo em `try/catch`: em janela anônima o acesso ao `localStorage` **lança**, e
uma grade que não abre por causa da memória de largura de coluna seria péssima
troca. Coluna guardada que a tabela perdeu é ignorada, nunca ressuscitada.
`api.esquecerLayout()` apaga.

### Duplo clique abre a ficha

`aoAbrirLinha(linha, ix)`. **Duplo** e não simples: o clique simples já é da
seleção, e uma grade que navega ao primeiro toque atrapalha quem só queria
marcar.

A grade não sabe editar, e **não é ela que deve saber** — ver §5.

---

## 5. Edição na célula: recusa fundamentada

O `phx-grid` não tem edição na célula em nenhuma versão (0.6.0, 0.7.0, 0.8.0
nem a nossa), e ela **não entrou**. O motivo não é esforço:

O console já grava por um caminho só, a **ficha** (`abrirFicha`). Ela carrega a
**versão do slot** junto com a linha e manda essa versão de volta no
`atualizar`, e é isso que faz o servidor recusar escrita concorrente — a janela
de minutos entre abrir a ficha e clicar em salvar é exatamente onde existe
gente. Uma célula que grava direto ou repete essa guarda (e aí são duas
implementações da mesma coisa, e uma delas vai ficar para trás), ou não a
repete — e aí é um caminho de gravação **sem** a guarda, aberto para quem clicar
duas vezes rápido.

Além disso a ficha tem o *merge* por coluna que a §«Merge de conflito» do
`CLAUDE.md` descreve, tem validação por tipo e trata `Bin`/`Memo`, que moram
fora do slot e não cabem numa célula.

O que entrou no lugar: **duplo clique na linha abre a ficha**. Custa um evento,
reaproveita toda a guarda, e chega ao mesmo lugar em dois cliques.

Se um dia a edição na célula for pedida de verdade, o pré-requisito é a ficha e
a célula usarem **o mesmo** código de gravação — não dois.

---

## 6. O catálogo dos modelos, cruzado

Vieram duas coisas no pacote, e elas se leem de modos diferentes.

### 6.1 `phx-grid` 0.6.0 e 0.7.0 — nada a trazer

A nossa é mais nova que as duas (tem o motor de agrupamento da 0.8.0, que a
0.7.0 não tem, e mais três recursos que a 0.8.0 não documenta). As demos
(`nucleo`, `celulas`, `filtros`, `busca`, `bandas`, `excel`, `frow`) mostram os
recursos em uso — e o que a comparação com elas rendeu foi descobrir **quanto
do que já existia estava desligado nas nossas telas**, que é o §4 inteiro.

### 6.2 `phoenix_data_grid_x` v1–v38 — catálogo, não código

Produto diferente: Rust com **DataFusion**, PostgreSQL, Timescale,
OTLP/Prometheus/Grafana, Dioxus/WASM, detecção de anomalia por *z-score*,
cadeia de auditoria com rotação de chave, eleição de líder. Da v10 em diante
quase nada é grade: é infraestrutura de servidor.

**Linkar é impossível aqui**, e é recusa fundamentada, não pendência: a
DataFusion sozinha traz centenas de crates e seria o fim do `cargo build
--offline` e da compilação cruzada para Windows que funcionou de primeira. É a
mesma parede do MULTILINK (`PENDENCIAS.md` #95): o caminho é ler e portar a
ideia.

Lido o `ROADMAP.md` da v1 (a única versão que ainda é uma grade) e as notas de
v9 a v38, o cruzamento fica:

| O que o modelo tem | Aqui |
|---|---|
| Busca full-text local | ✅ existe e ligada (busca global, com ranking por relevância) |
| Filtro por valores estilo Excel | ✅ existe e ligada — e estava **ilegível**; ver §7 |
| Ordenação | ✅ |
| Colunas visíveis/ocultas | ✅ — e o menu **não montava**; ver §7 |
| Largura por coluna | ✅ ligada, e agora lembrada |
| Reordenação de colunas | ✅ ligada, e agora lembrada |
| Agrupamento encadeado | ✅ multi-nível, com ordem por nível e agregados |
| Pivot Engine | ✅ **no servidor** (`op_pivotar`), com assistente na tela — a decisão de fazer o pivot no motor está em `pivot.rs`: um pivot resume, e mandar cem mil linhas ao navegador para somar seria pagar o transporte do que vai ser jogado fora |
| Fonte e altura da linha / densidade | ❌ não entrou. Cabe (é CSS), mas ninguém pediu e a página tem tema próprio |
| Blue/Green/Red Light (temas) | ❌ **recusado**: a marca manda, e são dois temas (`#010418` e papel), não seis |
| Vertical View (uma linha na vertical) | ❌ não entrou. A **ficha** já é isso, com edição por cima |
| Resumo analítico local | ◐ o total geral e o agregado por grupo cobrem o número; o texto em prosa não |
| Drag-and-drop de cabeçalho | ✅ arrasta para reordenar e para agrupar |
| Zonas Pivot (Filters/Columns/Rows/Values) | ◐ o assistente da tabela dinâmica faz o mesmo por seleção, sem arrastar |
| Filtro Excel avançado: operadores, intervalo, busca dentro do filtro | ✅ os três existem (Filtros de Número com E/OU, faixa, e o campo Pesquisar dentro do funil) |
| Filtro Excel avançado: calendário, top N, árvore hierárquica | ❌ não entrou |
| Layouts salvos | ✅ entrou nesta rodada (§4) |
| `DataProvider` assíncrono / pushdown | ◐ o **contrato** existe (§3); o `WHERE` no servidor não |
| PostgreSQL, ClickHouse, DuckDB/Parquet, Kafka | ◐ **DbLink** já lê MySQL e Postgres — e é uma implementação nossa do protocolo de fio, sem crate. Os outros três seriam três protocolos novos, e cada um é um projeto |
| Embeddings, busca híbrida, Ask the Grid, AI Pivot, forecasting | ❌ **recusa fundamentada**: exigem modelo de linguagem ou índice vetorial. A grade já tem a tomada (`cfg.buscaSemantica`), e o console já fala com a Claude por `ui/claude.js`, mas isso é chamada a serviço de fora — não vira recurso de grade |
| Detecção de anomalia por *z-score* | ❌ não entrou. Este **caberia** sem dependência (é média e desvio-padrão sobre a coluna, `std` pura), e é a proposta com melhor relação custo/valor da lista. Fica anotado |
| RBAC, auditoria com rotação de chave, eleição de líder, OTLP | fora do escopo de grade — e os dois primeiros já existem aqui de outra forma (`usuarios.rs`, trilha LGPD) |

---

## 7. O que o exercício achou — e que ler o código não acharia

Três defeitos **já em produção**, todos encontrados abrindo a tela e medindo.

### 7.1 O funil da coluna estava ilegível

Medido no Chrome, antes: a caixinha de marcar da lista de valores tinha
**204 px de largura** — o `input{width:100%}` da página — e empurrava o nome da
cidade para fora do popover. A lista de oito cidades aparecia como **oito
quadrados sem texto nenhum**. O rádio **E/OU** dos Filtros de Número media
**33,6 px**: a bolinha do tamanho da célula, de novo. E o
`label{text-transform:uppercase}` mostrava «Blumenau» como «BLUMENAU» — mentira
sobre o dado, porque quem lê não sabe se está gravado assim.

Depois da cerca: 13 × 13 px nos dois, `text-transform: none`, «Blumenau»
escrito como está.

A cerca fica no `index.html`, ao lado do bloco que já reapontava os tokens de
cor do grid, **e não no `phx-grid.css`**: o estrago vem daqui, e a folha da
grade é do ecossistema Phoenix — não tem por que conhecer o CSS do nosso
console. O teste `a_cerca_do_css_global_continua_de_pe` derruba quem apagar
qualquer uma das regras.

Esta é a **quarta** vez que o CSS global morde um componente novo. O padrão não
muda: `input{width:100%}` e `label{text-transform:uppercase}` são certos num
formulário e errados dentro de uma tabela, e nenhum dos dois aparece lendo o
código.

### 7.2 O menu de Colunas nunca montava

`montaColSel()` só era chamado por `moverColunaAntes` e por `mostrarColuna` —
isto é, **depois** de alguém já ter mexido nele. Aberto, vinha vazio, e o botão
vinha sem texto: um quadradinho em branco no rodapé. Esconder coluna nunca
funcionou por aqui.

O defeito é do `phx-grid` de origem: está igual na 0.6.0, na 0.7.0 e na 0.8.0.
Ler o código não mostra — as duas chamadas existem e parecem bastar.

### 7.3 Seleção com agrupamento contava cabeçalho de grupo como linha

Só aparece com **os dois** ligados, que é o que esta rodada fez. `atualizaMestre`,
o "marcar todas" do cabeçalho e a faixa com Shift percorriam
`ultimaCarga.linhas` inteiro, e com grupos ligados `__grupo` e `__rodape` caem
no meio: o "marcar todas" nunca fechava (sempre menos marcadas que linhas) e a
faixa punha no conjunto chaves que não existem no dado.

Os três passaram a usar `eMarcador()`, e a lista de marcadores virou **uma só**
(`MARCADORES`), ao lado de quem os cria. É a lição de sempre: **peça nova no
fim de uma lista quebra quem filtra pela primeira**.

Medido depois: 100 linhas na página, 7 cabeçalhos de grupo, 93 de dado, **93
marcadas**, caixa do cabeçalho fechada, nenhuma chave `undefined`.

### 7.4 A linha de filtro engordava toda coluna (achado ao ligar)

Ligar `filterRow` levou o `rowid` de 68 para **237 px** — mais largo que o
`pedido`, e ele pede 90. Numa tabela de layout automático é a largura
**intrínseca** do controle que decide a largura da coluna, e um `<input>` traz
o velho `size=20` (~170 px).

Duas medições até acertar: `size` pequeno resolve a caixa de **texto** e só
ela — `size` **não vale** para `input[type=number]`, e a caixa de número
continuava nos mesmos 169 px. E `flex:1` também não resolvia: a caixa enchia a
célula, mas a célula só era larga porque a contribuição de *max-content* da
caixa continuava sendo 169. Com largura fixa e pequena: **118 px** (68 sem a
linha de filtro nenhuma).

---

## 8. A versão, que dizia três coisas ao mesmo tempo

O cabeçalho do `phx-grid.js` e o do `.css` diziam **`v0.1.0 — Núcleo (S01)`**
desde a S01. O `PhxGrid.versao` dizia `0.8.0`. E o código já tinha **ordem por
nível de grupo, rodapé de grupo e total geral**, que a 0.8.0 nem documenta.
Quatro lugares, três respostas.

Hoje os quatro são conferidos entre si pelo teste **`grade_versao_nao_mente`**
(`http.rs`), que toma o topo do `CHANGELOG-phx-grid.md` como verdade — é o
único dos quatro que vem com a lista do que mudou ao lado. Repondo o defeito
(`v0.1.0` no cabeçalho), o teste falha dizendo qual dos quatro discorda.

**Número digitado à mão envelhece calado.**

---

## 9. O que ficou de fora, com motivo

| Recurso | Por que não |
|---|---|
| Edição na célula | §5 — duplicaria a guarda de escrita concorrente da ficha, ou não a teria |
| Pushdown de filtro/ordem/grupo | §3 — precisa de `WHERE` em `varrer`, que é mudança de protocolo e de motor, não de tela |
| Bandas no cabeçalho | o esquema não diz quais colunas são irmãs; adivinhar por prefixo erraria |
| Temas Blue/Green/Red | a marca manda: são dois temas |
| Calendário, top N e árvore no funil | cabem, mas ninguém pediu; os três filtros que existem cobrem o uso diário |
| Detecção de anomalia | **caberia sem dependência** (média e desvio-padrão em `std` pura) e é a melhor da lista dos modelos. Anotado, não feito |
| DataFusion, Timescale, OTLP, WASM/Dioxus | dependência — o fim do `cargo build --offline` e da compilação cruzada |

---

## 10. Como conferir

```bash
cargo test -p phxsql-server grade_versao_nao_mente
cargo test -p phxsql-server a_cerca_do_css_global_continua_de_pe
```

E pelo navegador, que é onde os quatro defeitos da §7 apareceram — nenhum deles
sai da leitura do código.
