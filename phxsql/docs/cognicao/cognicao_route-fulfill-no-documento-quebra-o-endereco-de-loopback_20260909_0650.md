# `route.fetch()+fulfill()` no documento principal quebra o endereço de loopback para o Chromium

**09/09/2026, 06:50** — descoberto montando a prova dupla do pedido 231
("criar do plano sem confirmação"), não procurando defeito de segurança
nenhum.

## 1. O que aconteceu

Para repor o defeito "a tela criava a tabela sem esperar o clique" sem
recompilar o `phxsqld` (o binário embute `ui/claude.js` por `include_str!`, e
esta frente está proibida de rodar `cargo`), o caminho óbvio era interceptar a
resposta do documento principal com o Playwright — `context.route()` casando
`GET /`, `route.fetch()` para pegar a resposta de verdade, um `.replace()` no
corpo, `route.fulfill()` com o corpo patcheado. Funciona para reescrever
texto: a página carrega, o `claude.js` patcheado roda.

O que quebrou foi outra coisa, que a prova só usava incidentalmente: essa
página precisa chamar a Anthropic **falsa**, que mora noutra porta do mesmo
`127.0.0.1`. Toda chamada morria assim:

```
Access to fetch at 'http://127.0.0.1:6982/v1/messages' from origin
'http://127.0.0.1:6981' has been blocked by CORS policy: Permission was
denied for this request to access the `unknown` address space.
```

## 2. O que eu concluí primeiro, e estava errado

**Achei que era CSP.** A tela realmente tem uma política que restringe
`connect-src`, e essa é a peça central de OUTRA das três provas duplas desta
rodada (a Bateria 3) — então "CSP bloqueando fetch entre portas" era a leitura
óbvia. Não era: a mensagem cita CORS e "address space", não
`Content-Security-Policy`, e o teste em questão nem tocava o cabeçalho de CSP,
só o corpo do `claude.js`.

**Segundo palpite: faltava CORS no servidor falso.** Também não — o
`claude-falsa.mjs` já respondia `Access-Control-Allow-Origin` e tratava
`OPTIONS`, e a MESMA falsa funcionava perfeitamente para as Baterias 1 e 2 sem
patch nenhum no documento. A diferença não estava no servidor falso: estava em
como a PÁGINA CHAMADORA tinha chegado ao navegador.

## 3. O que a medição disse

Isolei a variável com um script mínimo: a mesma página, o mesmo `claude.js`
patcheado, testado dos dois jeitos.

| como a página chegou ao navegador | fetch para outro `127.0.0.1` |
|---|---|
| `page.goto()` direto no `phxsqld` de verdade | funciona (usado em Baterias 1 e 2 sem patch, e em toda a `bateria.mjs`) |
| `page.goto()` num servidor `http` puro que faz PROXY para o `phxsqld` (sem `route` do Playwright envolvido) | funciona |
| `page.goto()` interceptado por `context.route()` com `route.fetch()+fulfill()` no MESMO documento | **CORS "unknown address space"** |

A terceira linha é a única que muda: mesmo HTML final, byte a byte
equivalente (conferido), mesma origem `127.0.0.1`. O que muda é que o
Chromium aparentemente classifica o endereço de origem da navegação pelo
caminho de rede que a serviu — e uma resposta entregue via CDP
(`Fetch.fulfillRequest`, que é o que `route.fulfill()` faz por baixo) não
carrega a mesma informação de endereço IP que uma conexão TCP direta, caindo
em "unknown" para fins de Private Network Access. Dali em diante, QUALQUER
fetch para um endereço mais "privado" (loopback incluso) que o dela própria é
tratado como uma tentativa de "unknown space" alcançando "private space", e
recusado.

Isto não é documentado explicitamente pelo Playwright nem pela spec de PNA
como um efeito de `route.fulfill()` — é um efeito medido, não lido.

## 4. A regra

**Repor um defeito no CORPO de uma página não é neutro para a REDE dessa
página.** Quando a prova de um defeito precisa que a página patcheada ainda
fale com outro servidor de teste, a interceptação de rede do próprio
Playwright é arriscada — prefira servir a cópia por uma conexão de verdade
(um proxy reverso simples) e reservar `route.fulfill()` para os casos em que
o alvo do defeito é uma origem PÚBLICA (aí a classificação de endereço não
entra em jogo do mesmo jeito, e a Bateria 3 desta rodada confirma que
funciona sem problema).

## 5. Como está guardado hoje

`testes-web/claude-interceptar.mjs` tem os dois caminhos lado a lado:
`interceptarPaginaPrincipal` (via `route`, usado só onde o alvo é
`https://api.anthropic.com`, uma origem pública) e `subirCopiaComPatch` (o
proxy reverso em `http` puro, usado nas Baterias 1 e 2, cujo defeito precisa
alcançar a Anthropic falsa em loopback). O comentário de topo de
`subirCopiaComPatch` cita este arquivo. `docs/CLAUDE-IA.md` §8 também
resume o achado, para quem for reproduzir a prova dupla sem ler o código
primeiro.

**O que não foi investigado:** se existe uma flag do Chromium
(`--disable-features=...`) que reative a classificação correta para conteúdo
servido via CDP. Não foi preciso — o proxy resolve sem tocar em flag de
browser nenhuma, e mudar flag do navegador para TODAS as baterias (inclusive
a Bateria 3, que depende do comportamento de segurança real) seria trocar uma
correção cirúrgica por uma abrangente demais.
