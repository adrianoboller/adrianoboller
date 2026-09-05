# Mencionar um botão não é clicá-lo — e o `Set` da página some no `reload`

**05/09/2026, 14h20** (hora da descoberta, não a do commit)

## 1. O que aconteceu

A ordem do dono era «bateria de testes de todos os botões». Para cumpri-la é
preciso primeiro saber quantos são, e depois saber quais a bateria já
exercita. Escrevi o conferidor
(`crates/phxsql-server/src/conferidor_botoes.rs`) e, para o cruzamento,
comecei pelo caminho barato: **varrer o fonte dos casos** em `testes-web/`
atrás dos seletores escritos ali, e casar com a chave de cada botão.

Deu 48 botões «exercitados» de 298.

Depois troquei o método: um ouvinte de captura dentro do navegador, que anota
os ganchos do botão que **recebeu clique de verdade**, gravados em
`testes-web/botoes-exercitados.txt` pela corrida inteira da bateria.

Deu **28**.

## 2. O que eu concluí primeiro, e estava errado

Concluí que a leitura estática bastava, com um argumento que parecia bom: *o
caso precisa escrever o seletor para clicar nele, então o seletor escrito é a
prova do clique.* E o argumento é falso nos dois sentidos, e eu não tinha
medido nenhum dos dois:

- **Falso para mais**: um `page.waitForSelector('#btSalvar')` escreve o
  seletor e não clica. Um comentário citando `.phx-fbtn` também. Vinte dos 48
  eram menções — `.phx-fbtn` e `.phx-exp-btn` entre elas, seletores que
  aparecem no fonte da bateria há rodadas sem nunca ter recebido um clique.
- **Falso para menos**: o caso `passeio` clica ~112 botões que **nenhum**
  seletor do fonte nomeia, porque ele varre o menu pelo DOM
  (`querySelectorAll('.item')`) e clica pelo índice.

Errei uma segunda vez logo em seguida, e essa custou mais: pus o acumulador
num `Set` dentro do `window` da página, e o caso `multitela` dá um
`page.reload()` no meio. O `Set` nascia vazio de novo, e todo clique
**anterior** ao reload sumia — entre eles o `[data-jan="acoplar"]`, que aquele
caso clica há rodadas. A evidência dizia «nunca clicado» de um botão provado,
e a catraca teria mandado escrever um caso que já existe.

## 3. O que a medição disse

| leitura | botões dados por exercitados |
|---|---|
| seletores escritos no fonte dos casos | 48 |
| clique gravado no navegador, `Set` na página | 28 |
| clique gravado no navegador, acumulador no Node (`exposeBinding`) | **28 → 85 depois dos três lotes novos** |

O `Set` na página perdia, medido no caso `multitela` sozinho, **22 ganchos**
numa corrida de um caso só.

E o número cru da varredura ingênua também estava errado, pela terceira causa
independente: `grep '<button' ui/*.js` diz **277**, o conferidor diz **298**.
A diferença é **+19** do subdiretório `ui/grid/` (que um `*.js` não alcança) e
**+2** de `<span role="button">`, que uma varredura por `<button` não vê.

## 4. A regra

**Prova de clique se grava no clique.** Seletor escrito no fonte de um teste é
menção, não prova — e o acumulador da gravação mora fora da página, porque a
página morre no `reload`.

## 5. Como está guardado hoje

- O gravador é o `GRAVADOR` + `exposeBinding('__phxGravaBotao')` de
  `testes-web/bateria.mjs`, e só a corrida **inteira** reescreve
  `testes-web/botoes-exercitados.txt` — corrida parcial não grava, porque
  evidência parcial é pior que evidência faltando.
- O cruzamento e a catraca `TETO_BOTAO_SEM_PROVA` (211 hoje) estão em
  `crates/phxsql-server/src/conferidor_botoes.rs`, com
  `nenhuma_chave_morta_na_evidencia` fechando a porta dos fundos de editar o
  arquivo à mão.
- O relatório é `cargo run --example botoes-sem-prova -p phxsql-server`.

**Onde o buraco ficou:** a gravação é por corrida inteira, e nada obriga
alguém a rodá-la. Quem apagar um caso e não rodar a bateria deixa a evidência
mais generosa do que a verdade até a próxima corrida cheia. A guarda de chave
morta só pega o lado contrário — o botão que sumiu da tela.
