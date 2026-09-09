# Cognição: o campo `padrao` do cartão de "acrescentar coluna" é uma EXPRESSÃO — texto cru precisa de aspas, e ninguém tinha achado porque o caso quebrava sempre no mesmo lugar

## 1. O que aconteceu

`node testes-web/bateria.mjs` (corrida inteira, as duas obrigatórias para
gravar `botoes-exercitados.txt` e fechar o achado 4 da G5-TELA) reprovava o
caso `acrescentar-coluna` nos dois temas, sempre com o mesmo timeout:
`page.waitForSelector('#acNome', { state: 'detached' })` esgotando 20 s —
o cartão de "Acrescentar coluna" nunca fechava depois do clique em
`#acFazer`. Confirmado com `git stash` (as fontes de `ui/index.html` e
`idiomas.rs` voltadas para `e27775b`, o commit-base desta frente): o mesmo
defeito, byte a byte — **pré-existente**, sem relação com o direito por
coluna que motivou esta rodada.

## 2. O que eu concluí primeiro, e estava errado

A primeira hipótese, olhando só o timeout, foi "flake" — algum problema de
tempo ou de recurso na bateria (que já tem casos sensíveis a isso, como o
`telemetria` documentado no `apoio.mjs`). Rodei o caso duas vezes seguidas,
isolado, e ele falhou as duas, do mesmo jeito, com a mesma mensagem — não é
flake, é determinístico.

## 3. O que a medição disse

Um script Playwright avulso (fora da bateria, com `page.on('response', …)`
escutando a API) reproduziu o clique em `#acFazer` e capturou a resposta
`400` que a bateria não estava olhando: `[SP000018] esquema invalido: o
padrao de situacao usa a coluna "ativo", que a tabela nao tem`. O caso
preenchia `#acPadrao` com `'ativo'` (o texto cru, sem aspas) — e o campo
`padrao` do `acrescentar_coluna`, como o `padrao` do `criar_tabela`, é uma
EXPRESSÃO (`Expressao::analisar`, `phxsql-core`), não um valor literal.
Sem aspas, `ativo` analisa como referência a uma COLUNA chamada `ativo`,
que a tabela `clientes` deste caso não tem — daí a recusa. Reescrevendo o
campo para `"'ativo'"` (aspas simples como parte do TEXTO digitado, para
virar um literal de string na expressão), o cartão fecha e o resto do caso
passa — inclusive a asserção de que a grade mostra `ativo` preenchido nas
linhas antigas (a string, sem as aspas, que é o que a expressão avalia).

## 4. A regra

**Todo campo de tela que alimenta uma EXPRESSÃO do motor (`padrao`,
`calculada`, `check`) espera sintaxe de expressão, não o valor final — um
literal de texto leva aspas, do mesmo jeito que levaria dentro de um SQL. Um
teste que preenche esse campo com o valor cru está testando um pedido
inválido, e a recusa do servidor (certa) aparece como o teste quebrando
(errado).**

## 5. Como está guardado hoje

`testes-web/casos/14-acrescentar-coluna.mjs`, linha do `page.fill('#acPadrao', …)`,
com o comentário nomeando o erro exato e por que ele apareceu. Fora
dele, o alcance NÃO está escrito em lugar nenhum: `docs/SQL.md` (ou
`docs/FORMATO.md`, onde `padrao`/`calculada`/`check` são descritos) não diz,
para quem só lê a tela, que o campo do cartão espera expressão — quem
digitar um texto cru pela INTERFACE (não pelo teste) recebe a mesma recusa
`[SP000018]`, que nomeia a coluna inventada mas não diz "use aspas para um
texto". Isto não é achado desta rodada (a mensagem do motor está certa, só
não é óbvia para quem nunca viu o padrão-como-expressão) — fica nomeado
para quem cuidar da tela de Estrutura depois.
