# Oráculo de tempo tem irmão que curto-circuita: o conserto pedido faria dele o único caminho rápido

**Estado:** PENDENTE

Pedidos 520 e 521. Papel B, em 24/09/2026, sobre o achado do SEC na terceira
revisão do 497 (`docs/propostas/parecer-sec-497-3a-2026-09-24.md`, «Fora do 497»).

## 1. O que aconteceu

O SEC achou que o login de quem **não existe** pagava 1.000 iterações de PBKDF2
contra 210.000 de quem existe (`usuarios.rs`, `Cadastro::autenticar`), com o
comentário logo acima afirmando que os dois «não se distinguem pelo relógio». O
conserto pedido era a fachada com `ITERACOES_PADRAO`. Procurando o irmão — quem
responde «credencial inválida» por outro caminho — apareceram mais três:

| caminho | antes (debug, porta de dados) |
|---|---|
| senha errada de quem existe | 2.671 ms |
| quem não existe (o do SEC) | 25,7 ms |
| **inativo** | **0,2 ms** — o `self.ativo && senha::conferir(...)` pulava o PBKDF2 inteiro |
| prova do desafio-resposta, não existe / inativo | 24–25 µs a menos que a de quem existe |
| o próprio `desafio`, não existe | +9 µs |

## 2. O que eu concluí primeiro, e estava errado

**Primeiro:** que consertar a fachada fechava o 520. Não fechava: o inativo
respondia sem conta nenhuma, e com a fachada certa ele viraria o **único** caminho
rápido — o relógio passaria de «não existe» para «existe, e está desligado», que
diz mais. O conserto do SEC, sozinho, teria piorado o oráculo.

**Segundo:** que a prova do desafio-resposta estava limpa. A primeira sonda media
cada caminho numa conexão própria, em sequência, e deu medianas de 0,161 / 0,134 /
0,167 ms — parecia ruído. Intercalando os três na mesma conexão, n = 1.000 em duas
rodadas, a diferença apareceu estável: **−24,4 a −25,3 µs** para quem não existe e
para o inativo, nas duas rodadas.

**Terceiro, no sentido oposto:** que o `desafio` tinha +11 µs para quem não existe.
Medido com login do **mesmo tamanho** (`ana` × `zzz`), eram +9 µs antes e 0,0 µs
depois; com `nao_existe` × `ana`, o conserto ainda mostrava +2 µs — e esses 2 µs
eram o tamanho do login (análise do JSON, linha do log), não o cadastro.

## 3. O que a medição disse

- Depois, pela porta de dados em debug: senha errada 1.367,8 ms, inativo 1.372,2 ms,
  não existe 1.368,8 ms. Prova: ±1 µs. Desafio: −0,2 a 0,0 µs.
- 521: login de 1 KiB 4,64× o de 8 B antes (12.271 × 2.643 ms), 0,98× depois;
  60.000 B 1,05×. O login inteiro ficou 1,95× mais rápido (2.671 → 1.368 ms): a
  iteração passou de quatro compressões de SHA-256 para duas.
- Os vetores não pegam o 521: com o defeito reposto, RFC, Wycheproof e a versão
  ingênua bit a bit ficam **todos verdes**. Só o contador de compressões cai
  (0 B, 10 iterações: 40 contra 22).

## 4. A regra

**Oráculo de tempo se conserta procurando TODO caminho que chega à mesma recusa, e
o que pula a conta antes dela é o primeiro suspeito. E se mede intercalado, com
entradas do mesmo tamanho; num teste, conta-se o trabalho por dentro, nunca o
relógio.**

## 5. Como está guardado hoje

- Uma chamada só a `senha::conferir` no `Cadastro::autenticar`, com o
  `senha::hash_de_fachada` para quem não existe e o `ativo` olhado **depois**; o
  mesmo desenho no ramo da prova do `op_login` e no `op_desafio`.
- Contadores por thread: `hash::iteracoes_pagas_nesta_thread` e
  `desafio::provas_conferidas_nesta_thread` (uma soma por derivação e por login), e
  as compressões só no binário de teste do core.
- Sete guardas no catálogo (`pbkdf2-normaliza-a-chave-a-cada-iteracao`,
  `conferir-sem-o-teto-da-senha`, `fachada-do-login-com-mil-iteracoes`,
  `inativo-pula-o-pbkdf2`, `prova-de-quem-nao-existe-sai-sem-conferir`,
  `login-sem-o-teto-da-senha`, `criar-usuario-sem-o-teto-da-senha`), vermelho
  medido repondo uma a uma.
- **Buraco que fica:** o irmão do `desafio` não tem guarda — só a sonda pelo
  soquete o mede, e um contador para ele seria um observador novo no caminho de
  toda conexão. E o hash carrega o próprio custo: usuário com hash feito à mão com
  outra contagem de iterações continua distinguível — o `desafio` já a publica.
- **E o buraco maior, achado procurando o irmão e NÃO consertado:** o sal falso do
  `desafio` é `HMAC(token, login)`, e quem tem o token — o mesmo alcance do 520 —
  o recalcula e sabe quem não existe em um pedido, sem relógio nenhum (6 de 6 na
  sonda). Igualar o relógio de um caminho cujo conteúdo já responde era consertar
  a porta com a janela aberta ao lado; o conserto pede um segredo persistente do
  servidor (o `mock_auth_nonce` do PostgreSQL), e vai como pedido novo.
