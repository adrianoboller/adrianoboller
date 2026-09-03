# Revisão de tela — 20260903

Papel E (designer gráfico). Interface só se prova exercitando — este documento
é o registro do que foi **clicado**, não do que foi lido.

Encerrado antes do previsto por ordem do orquestrador: oito frentes vivas na
máquina disputando 4 CPUs e a trava do `cargo`, e esta ficou parada demais
tempo com servidor e Chromium de pé. A seção **"O que não deu tempo"** nomeia
o corte.

## Como exercitei

Servidor `phxsqld` próprio (fora das portas 5770/6700/6701 já usadas por
outras frentes e pelo `capturar-dossie.mjs`), subido e derrubado só por mim,
pelo PID — nunca por `pkill -f`. Populei `Comercial.clientes` com 30-40 linhas
para a grade não ficar vazia. Dois roteiros Playwright, escritos para esta
revisão (não versionados — são script de sessão, não ferramenta do projeto) e
reaproveitando o padrão de subida/derrubada do
`docs/dossie/capturar-dossie.mjs`:

1. **Fluxo clicado**, nos dois temas (`localStorage` setado antes de entrar):
   login → `Alt+1` (Painel) → clique no botão "Tabelas" da barra → dois
   caminhos até a grade (ver Achado 1) → **Nova linha** com campos digitados
   de verdade → duplo clique numa linha para **alterar** um campo e Salvar →
   duplo clique noutra linha, **Excluir** (modo "marcar", o padrão do
   diálogo) → aba "excluídas" para ver o `restaurar` → **Consulta**
   (`memoria_carregar` antes, porque a tela é o `SelectMemory` e avisa disso
   sozinha quando a tabela não está em RAM).
2. **Larguras**: 1500 / 1024 / 800px, dois temas, medindo
   `document.documentElement.scrollWidth - clientWidth` no Painel e na grade.

Também rodei o roteiro sistemático que **já existe no projeto** para isto —
`docs/design/exercicio.mjs` (32 telas × 3 viewports × 2 temas) — copiado para
uma porta própria (sem editar o arquivo do repositório) e apontado para o meu
servidor. Cobri **84 das 192 combinações** (celular 390px completo nos dois
temas, tablet 820px escuro completo, tablet claro parcial) antes de encerrar
por contenção — ver "O que não deu tempo".

## Achado 1 — duas telas de "conteúdo da tabela", só uma tem "incluir"

**Onde:** `crates/phxsql-server/ui/index.html`.

- Caminho A — clicar a tabela na árvore lateral (`#arvore .no.tab`) → aba
  **"Conteúdo"** → `vConteudo()` / `ligarConteudo()` (por volta da L3001-3140).
  Grid com agrupamento, busca, seleção em lote e exclusão em lote
  (`#btExcluirSel`). **Não tem botão de incluir linha.** Duplo clique numa
  linha abre a ficha (`abrirFicha`), que permite Salvar e Excluir — só não dá
  para criar uma linha nova a partir daqui.
- Caminho B — barra "Tabelas" → clicar a tabela → "Editar conteúdo da
  tabela" (ou o atalho `Alt+3` com uma tabela aberta) → `verConteudoEditavel()`
  (por volta da L4038-4166). Grid com paginação por cursor, **com** o botão
  `#btNova` ("Nova linha") que chama `abrirFicha(db, tab, null)`.

Capturas (mesmos dados, mesma tabela, tema escuro):
`docs/revisao-de-tela-20260903/conteudo-pela-arvore-sem-incluir.png` (barra de
ações da grade não tem "Nova linha") e
`conteudo-por-gerir-tabelas-com-incluir.png` (tem).

**Por que é defeito:** o caminho mais natural para chegar aos dados de uma
tabela é clicar nela na árvore — é o "ponto de partida de toda sessão" (o
próprio comentário de `FERRAMENTAS` no código diz isso sobre o fluxo do dia a
dia). Quem chega por aí só descobre que dá para incluir uma linha se souber
que existe um segundo caminho ("Gerir as tabelas") ou o atalho de teclado
`Alt+3` — nenhum dos dois é sugerido a partir da tela que a árvore mostra. Não
é uma tela genérica de "detalhe read-only": ela já deixa **alterar e excluir**
pela mesma ficha que o outro caminho usa para incluir; só falta o botão que
abriria essa ficha em branco.

**Não sei se é intencional** (talvez a tela da árvore tenha nascido como
"navegação/browse rápido" e a de "Gerir as tabelas" como "operação completa",
de propósito) — por isso **descrevo e não conserto**: unificar as duas telas,
ou simplesmente acrescentar `#btNova` também em `vConteudo`, é uma decisão de
fluxo maior que uma regra de CSS, e outra frente pode estar mexendo nesta
mesma tela agora.

## Verificação — dado não muda de aparência (a pétrea do "Blumenau")

Tentei reproduzir especificamente o defeito já documentado
(`label{text-transform:uppercase}` fazendo "Blumenau" virar "BLUMENAU" na
grade). Incluí uma linha nova com `cidade = "blumenau"` (minúsculo,
deliberado) e comparei o valor **gravado** (lido de volta pela API, verdade em
disco) contra o texto que a **grade** mostra para a mesma linha — por
`rowid`, não por busca textual, porque a tabela de teste já tinha várias
"Blumenau" pré-existentes com maiúscula e uma busca por texto pegaria a linha
errada.

Resultado, nos dois temas: gravado `"blumenau"`, grade mostra `"blumenau"` —
sem mudança de aparência. **Registro do processo, porque errei primeiro:** a
minha primeira tentativa de checar isso comparava por busca textual
(`/blumenau/i` em qualquer célula da grade) e deu **falso positivo** — achou
uma "Blumenau" pré-existente e reportou como se a tela tivesse forçado
maiúscula na linha nova. Só depois de comparar pela API e pelo `rowid` exato
o resultado ficou confiável. Fica registrado para quem for repetir esta
checagem: comparar por busca de texto solto é o jeito errado de fazer.

## Cores de ação

Nas telas de incluir/alterar/excluir (ambos os caminhos do Achado 1), nos dois
temas: medi o `background-color` computado dos botões `.incluir` `.alterar`
`.marcar` `.excluir` `.consultar` fora do `:hover` — todos com fundo
transparente e contorno, nenhum com fundo cheio. Confirma visualmente também:
"Nova linha"/"Incluir" verde-contorno, "Salvar" âmbar-contorno, "Marcar como
excluído" rosa-contorno, "Excluir de vez" vermelho-contorno (só aparece no
diálogo, ao trocar de modo), botão "clientes" (que abre a tabela) azul-contorno
na grade de "Gerir as tabelas". Nenhuma violação da convenção.

## Contraste (tema claro)

Medi a razão de contraste (fórmula WCAG: luminância relativa + razão) em
`.botao, label, .leg, th, .rot` nas telas do fluxo clicado — 0 pares abaixo de
4,5:1. Cobertura parcial: não cheguei a rodar a bateria completa de seletores
que o `CONTRASTE()` de `docs/design/exercicio.mjs` mede (que compõe fundos
translúcidos até a primeira superfície opaca, e cobre mais pares — barra,
árvore, KPI, ficha, pino, aviso). Ver "O que não deu tempo".

## Rolagem lateral

Corpo da página: **0 excesso** de `scrollWidth` sobre `clientWidth` em
1500/1024/800px, dois temas, no Painel e na grade.

A barra de ferramentas **tem** rolagem própria abaixo de 1024px
(`#ferramentas{overflow-x:auto}`, comentário no CSS: "flex-wrap volta a ser
nowrap abaixo de 1025px, onde envolver significaria sete fileiras") — isso é
decisão já tomada e documentada, não achado novo. Confirmei que a rolagem
funciona de fato (forcei `scrollLeft=999` em 800px e o contêiner respondeu:
`scrollWidth` 1447 contra `clientWidth` 800, batendo exatamente com o máximo
de rolagem). Não é a página que rola — é um contêiner com rolo próprio, que é
exatamente o padrão que `docs/design/exercicio.mjs` trata como correto (ver o
`LEIA-ME.md` da pasta: "um botão da barra de ferramentas em 1806px não é
defeito, porque a barra rola").

Revisei visualmente uma amostra das capturas do roteiro sistemático em 390px e
820px (painel, tabela-conteúdo, diagrama-er, query, ambos os temas): nenhuma
rolagem lateral perceptível, nenhum componente deformado.

## O que não achei (varredura limpa, com cobertura)

- Nenhuma coluna de sistema nova quebrando `find()` em vez de `filter()` — o
  fluxo de incluir/salvar pela ficha funcionou nas duas rodadas (dois temas),
  sem o erro "a lista tem N valores" que já aconteceu antes.
- Nenhum componente visivelmente deformado pelo CSS global (`input`, `label`,
  checkbox de seleção da grade, radio do diálogo de exclusão) nas telas
  exercitadas.
- Nenhuma mudança de aparência do dado digitado (ver seção específica acima).
- Nenhuma cor de ação fora da convenção contorno/hover.
- Nenhum contraste abaixo do mínimo nos pares medidos.
- Nenhuma rolagem lateral do corpo da página em nenhuma das larguras testadas.

## O que não deu tempo (impedimento nomeado)

- **`docs/design/exercicio.mjs` não terminou.** Cobri 84 de 192 combinações
  (celular 390px completo nos dois temas — 52 telas, tablet 820px escuro
  completo — 32 telas, tablet claro com 4) antes de encerrar por ordem do
  orquestrador. O script só escreve `relatorio.json` (com os números de
  "conteúdo passando da borda" e "corte sem rolo" por tela) **depois** do laço
  inteiro — como o processo foi interrompido no meio, esse relatório agregado
  nunca foi gravado. Sobraram só as 84 capturas de tela, que revisei numa
  amostra (~6 telas) e não integralmente. Rodar o roteiro até o fim (contra um
  `phxsqld` próprio, em porta livre, sem editar o arquivo do repositório — só
  as três constantes do topo) é o próximo passo natural para fechar esse
  número.
- **Não exercitei "excluir de vez"** (exclusão física) — só "marcar como
  excluído" (o padrão do diálogo). Vi a troca de cor/texto do botão ao
  selecionar o outro modo, mas não completei o fluxo com o `window.confirm`
  nativo que ele pede.
- **Não exercitei formulários dentro de**: usuários, jobs, replicação,
  telemetria, profiler, diagrama ER (além de abrir a tela). Vi essas telas de
  relance nas capturas do roteiro sistemático em 390/820px, sem clicar dentro
  delas.

## Consertos feitos nesta rodada

Nenhum. O único achado (duas telas de conteúdo, uma sem "incluir") é uma
decisão de fluxo, não uma regra de CSS — fica descrito para quem tratar
disso, com o comentário do próprio código já citado como pista de onde
"outra frente pode estar mexendo nesta tela".
