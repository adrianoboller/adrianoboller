# A bateria de frontend

O `cargo test` prova o motor. Esta bateria prova a **tela** — contra o
servidor de verdade, num navegador de verdade, sem maquete e sem mockup.

```bash
cargo build --release -p phxsql-server --bin phxsqld
node phxsql/testes-web/bateria.mjs
```

Ela sobe um `phxsqld` só dela (portas **6200** e **6201**), num diretório
temporário próprio, e o derruba **pelo PID** no fim — nunca por `pkill`.
Roda todos os casos de `casos/`, quase todos nos **dois temas** — os que medem
relógio rodam num tema só, porque cor não muda relógio. **Quantos são não fica
digitado aqui:** este texto já disse «treze» e «catorze» em rodadas em que o
diretório tinha mais, e número digitado à mão envelhece calado. O rodapé da
própria bateria conta as execuções que rodaram, e é ele que vale. ~5 min.

| chave | o que faz |
|---|---|
| `--tema claro` / `--tema escuro` | roda um tema só |
| `--caso <pedaço>` | roda os casos cujo nome contém o pedaço |
| `--capturas <dir>` | guarda os PNG de cada tela |
| `--ver` | abre o navegador na tela, devagar |
| `--porta <n>` | outra porta de dados (a web é ela + 1) |

## Atenção ao binário velho

A página está **embutida** no `phxsqld` (`include_str!`). Mexer em `ui/` e não
recompilar faz a bateria exercitar a página anterior — e passar verde numa
correção que ainda não existe. Esta casa já perdeu uma rodada inteira de
ganhos medindo com binário velho, então a bateria **recusa rodar**: ela
compara a data do binário com a do arquivo mais novo de `ui/` e diz qual.

## Os casos

Um por arquivo em `casos/`, na ordem do nome. A lista abaixo é o que cada um
**prova** — para saber quais existem hoje, `ls testes-web/casos/`.

| | o que prova |
|---|---|
| `entrada` | a tela de login, o desafio-resposta no navegador, e que a senha não sobra no documento. Falha se a página cair em modo demonstração — sem isso a bateria inteira passaria sem tocar no motor |
| `passeio` | clica **todos** os itens dos nove menus e **todos** os botões da barra — 112 telas — e reprova em qualquer erro. Vale mais que dez asserções bonitas |
| `ficha` | incluir e salvar pela tela. É o fluxo que quebrou inteiro quando o `rownum` entrou |
| `arvore` | a árvore remontando quando um banco novo aparece — e continuando **viva** depois de remontar |
| `grade` | nenhuma coluna de sistema vira coluna de dado na grade editável, e o phx-grid não perde coluna nenhuma |
| `css-global` | as três armadilhas do CSS global: controle esticado, dado em caixa alta, e caixa de marcar separada do próprio texto |
| `responsivo` | as **cinco** larguras — celular, tablet, desktop, ultrawide (3440) e dois monitores (5120). Nada rola de lado; e nas duas largas, nada estica: texto corrido tem teto, par rótulo→valor vira coluna, e texto de SVG não cresce com o monitor nem se sobrepõe. Planta um caminho de disco comprido antes de medir, senão a sobreposição não se reproduz |
| `lateral` | o painel retrátil e pinável, com volta |
| `cores` | a convenção das cinco cores (contorno, nunca fundo cheio) e o contraste **medido** de cada elemento pintado |
| `primeira-pintura` | a tela de entrada aparece mesmo quando a rede engole a fonte da marca |
| `lgpd` | a tela de Dado pessoal audita de verdade |
| `multitela` | abas vivas com estado próprio, regiões lado a lado com calha, janela solta dentro da página, e o pino. Mede os pedidos por minuto com a aba escondida, com ela fechada, e com as quatro telas nomeadas visíveis ao mesmo tempo |
| `acrescentar-coluna` | acrescentar coluna **pela tela**, numa tabela com dado: o botão na aba Estrutura, o cartão dizendo o preço antes dos campos, a coluna aparecendo na Estrutura e no Conteúdo com o dado antigo intacto, e a recusa da coluna obrigatória sem padrão chegando ao cartão. Achou, no primeiro minuto em que o cartão existiu, a caixa de marcar esticada a **834px** pelo `input{width:100%}` global |
| `monitores` | a emenda física entre dois monitores, o monitor pinado que sumiu, a janela destacada pegando a sessão pelo canal — e a sessão **não** aparecendo no `localStorage`. DPI de 2× num contexto próprio |
| `telemetria` | o único **painel vivo** em forma de grade: a grade nasce preguiçosa (dentro de um `<details>` fechado ela mediria largura zero), o gesto da pessoa sobrevive à volta do relógio, e a ordenação se confere pelo **efeito** — o dado sai ordenado — e não pelo estado |
| `tela-atropelada` | que uma tela lenta **não escreve por cima** da que a pessoa pediu depois dela. Segura a resposta da op `painel` no fio até a segunda tela estar pintada, então a corrida vira ordem fixa em vez de sorteio. Cobre as duas vítimas conhecidas (telemetria e Configurações) e a metade contrária, que impede a guarda de virar «nunca pinta nada». Ver `docs/TESTES.md` §11 |
| `botoes-da-grade` | os botões da **PhxGrid**, que é a grade de toda tela: paginação, agregador do cabeçalho, o popup de filtro (A-Z, Z-A, limpar, OK, cancelar), os chips, a barra de agrupamento (direção, desagrupar, expandir/recolher tudo, total por grupo), o seletor de colunas e o congelar. Cada passo confere o **efeito** — o dado que apareceu —, e não o estado. Achou o `.phx-th-agg` trocando a própria letra sem recalcular o total, com o cabeçalho dizendo AVG sobre uma soma |
| `botoes-do-conteudo` | os botões que mexem no **dado**: as quatro páginas por cursor, ativas/excluídas, editar e restaurar pela linha, o diálogo de excluir nos dois modos, a seleção em lote da aba Conteúdo e a lixeira (motivos, esvaziar, voltar). A prova é no **servidor**: quantas linhas ficaram ativas, e não o que a tela mostrou |
| `botoes-da-tira` | o cromo do modo multitela — abrir outra tela, pinar e fechar aba, dividir em regiões (com a calha), soltar numa janela flutuante e os botões dela. São os únicos botões da interface que não são `<button>`: o pino e o `×` da aba são `<span role="button">`, e foram eles que fizeram o conferidor aprender a segunda forma |

## O que a bateria GRAVOU: `botoes-exercitados.txt`

A corrida **inteira** reescreve `testes-web/botoes-exercitados.txt`, que é a
lista dos ganchos de botão que receberam clique de verdade — anotados por um
ouvinte de captura dentro do navegador, e não lidos do fonte dos casos. É
dessa evidência que o `conferidor_botoes.rs` tira quantos botões da tela ainda
não têm prova, e é ela que a catraca `TETO_BOTAO_SEM_PROVA` cobra.

Três coisas que valem saber:

- **Corrida parcial não grava.** `--caso` ou `--tema` só imprimem quantos
  ganchos aquela corrida viu. Evidência parcial é pior que evidência faltando:
  ela daria por não-provado tudo o que a corrida inteira prova.
- **Corrida com falha não grava tampouco**, e a segunda metade desta frase
  custou uma corrida para aparecer: o `phxsqld` caiu no meio de uma corrida
  cheia, os 41 casos seguintes reprovaram com `ERR_CONNECTION_REFUSED`, e a
  gravação aconteceu do mesmo jeito — o arquivo perdeu 110 ganchos. *«Corrida
  inteira» não quer dizer «corrida que chegou ao fim»: quer dizer corrida que
  provou o que se propôs a provar.* Hoje a bateria **para no ato**, nomeando o
  caso depois do qual o servidor caiu e mostrando a saída dele, em vez de
  produzir 41 reprovações com a mesma frase e nenhuma delas dizendo por quê.
- **O arquivo não se edita.** Editar é a porta dos fundos da catraca, e
  `nenhuma_chave_morta_na_evidencia` a fecha: gancho que a tela não tem mais
  reprova.
- **O acumulador é do Node, não da página.** Ele já morou num `Set` dentro do
  `window`, e o caso `multitela` dá um `page.reload()` no meio — o `Set`
  nascia vazio de novo e os cliques anteriores sumiam. Hoje vai por
  `exposeBinding`, que sobrevive à navegação.

## A prova do multi-idioma, à parte

`node phxsql/testes-web/prova-idiomas.mjs --capturas <dir>` roda fora da
bateria, na faixa **6650/6651**, e prova o caminho do idioma de ponta a ponta:

1. sem escolher nada, a tela é a de sempre, **em português** — o teste do
   comportamento velho, que é o que mais importa numa guarda nova;
2. a bandeira da tela de **entrada** troca o texto na hora;
3. a escolha **atravessa o login**: o cromo entra no idioma escolhido;
4. a bandeira da tela de **configuração** troca o cromo sem recarregar e sem
   levar a pessoa para outra tela;
5. a escolha sobrevive a **sair e entrar** de novo;
6. o **alemão** (~30% mais longo) não corta rótulo da barra nem faz a página
   rolar de lado — o defeito que só aparece traduzindo;
7. a **frase que era picada**: a tela «Sobre o modo multitela» sai inteira e na
   ordem em português, com a ênfase virada `<b>`/`<code>` de verdade e sem
   marca crua à mostra; troca para alemão **sem sair da tela**, e nem o
   corpo, nem o título, nem o `title` da tira de abas ficam em português;
8. capturas da mesma tela em três idiomas × dois temas.

Foi ela que achou o `txt` declarado como `const` sendo pedido pelo
`aplicarTema` do arranque: a página morria na primeira pintura e o botão de
tema ficava sem `onclick`. Ler o código não acharia.

O passo 7 tem prova real nos dois sentidos, e os dois defeitos foram
repostos: tirando o `est.repintar` que a tela do modo repõe, ela não troca de
idioma e o passo estoura no `waitForFunction`; tirando a conversão de marcas
do `marcado()`, a página mostra `**Multitela.**` com os asteriscos à mostra e
o passo diz qual frase saiu errada.

## A prova das QUATRO telas, também à parte

`node phxsql/testes-web/prova-idiomas-telas.mjs --porta 7550 --capturas <dir>`
é a irmã da de cima, e cobre o que ela não cobria: as telas que moram **fora**
do `index.html`. A de cima prova a máquina — login, cromo, texto que estica; a
desta prova as telas.

| passo | o que prova |
|---|---|
| a tela da **Claude** | português → **alemão**: o aviso de «leia antes de ligar», o rótulo da chave, e a explicação de custo do modelo, que vem do par `diz:`/`dizTxt:` |
| a **telemetria** | português → **francês**: a barra, o título da faixa, a gaveta das threads, e o rótulo do nível na legenda, que vem do par `rot:`/`txt:` do `NIVEIS` |
| a **grade** | português → **espanhol**: o rodapé, o seletor de colunas, e o painel de filtro de coluna, que **só existe depois do clique** |
| o **diagrama ER** | português → **italiano**: o `aria-label` do desenho |
| o comportamento **velho** | sem escolher nada, as telas são as de sempre |
| o console | nenhum `pageerror` nas quatro |

**Prova real, com o defeito reposto:** trocando o `txt` do `telemetria.js` por
`return padrao` — a delegação no global quebrada, que é o defeito mais
provável de quem move o módulo —, o passo da telemetria reprova nomeando o
rótulo que não trocou, **e os outros cinco continuam verdes**. É a delimitação
que importa: o passo acusa a tela dele, e não a bateria inteira.

Ela achou três defeitos que ler o código não acharia, e os três estão em
`docs/MENSAGENS.md`: `rot: "…"` com espaço não é par; quem escreve por último
manda (o `aplicarLegenda` reescrevia em português o botão que o `html()` já
tinha traduzido); e texto escrito com `\uXXXX` some das duas vias do
conferidor.

**Cada passo começa e termina em português, e o retorno está num `finally`.**
Sem isso, uma afirmação que falha no meio deixa a escolha de pé e o passo
seguinte reprova por um defeito que não é o dele — aconteceu na primeira
rodada, com o diagrama ER acusado de estar em espanhol por causa da grade.

## Os três canais de erro

`pageerror` não é o único. O `ligarMenu` manda **toda** exceção de item de
menu para `avisar(..., true)` — capturada, ela nunca vira `pageerror`. Por
isso o passeio olha três lugares:

1. `pageerror` — exceção que ninguém pegou (o runner cuida deste);
2. `#aviso.mal` — o recado vermelho da barra;
3. `#painel .aviso.mal` — o erro que uma aba deposita no painel.

Os dois últimos são limpos **antes** de cada clique: aviso deixado pela tela
anterior seria contado contra a próxima.

## O que ela deliberadamente NÃO faz

- **Não fala com a internet.** A fonte da marca vem do Google; a bateria
  recusa esses pedidos na origem. Deixá-los sair traria a rede de quem roda
  para dentro do resultado. O caso `primeira-pintura` é o dono desse assunto
  e instala a rota dele.
- **Não testa JavaScript por unidade.** A página não exporta módulo: é um
  `include_str!` de 11 mil linhas servido pelo binário. Todo caso é de ponta
  a ponta.
- **Não clica em diálogo nativo.** `confirm` e `prompt` são descartados pelo
  Playwright quando ninguém os escuta, e é por isso que o passeio pode clicar
  em «Excluir tabela» sem excluir nada. O caso `arvore` é a exceção: ele
  **responde** ao `prompt` do `[+]`, porque ali o diálogo é o caminho.
- **Não mede desempenho** do motor. Isso é a `bancada/`. O que ela mede é o
  custo da TELA em pedidos por minuto, que é outra coisa e mora no caso
  `multitela`.

  E há **duas bancadas** neste diretório que não são casos e não rodam com a
  bateria — elas medem, e por isso se chamam à mão:
  `grade/bancada-grade.mjs`, a prova de contrato da `phx-grid` isolada, e
  `grade/custo-da-ordem.mjs`, que mede **o que a grade ordenada custa na tela**
  em três escalas de tabela (pedido 188, `../docs/DESEMPENHO.md` §19). A segunda
  sobe um `phxsqld` só dela, nas portas **6300/6301** — fora da faixa da
  bateria, porque duas medições na mesma porta medem o servidor da outra.
- **Não exercita a Window Management API.** Ela existe no Chromium sem cabeça
  mas rejeita sem a permissão `window-management`, que o Playwright 1.56 não
  sabe conceder. O caso `monitores` a **dubla** e prova o caminho nosso —
  achar a emenda, alinhar as calhas, cair para o monitor principal. O que fica
  sem prova real é a resposta do navegador; ver `../docs/MULTITELA.md`.

O que cada caso cobre, o que ficou de fora e por quê está em
`../docs/TESTES.md`.
