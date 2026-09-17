# A recusa também se mede: o LLM local custava uma linha de CSP, não um cliente HTTP

**Descoberto em 17/09/2026, 02:44 UTC**, papel J, ao medir o material «IA ·
Query Designer · Excel Studio» que o dono mandou de um projeto irmão
(`docs/propostas/phoenix-query-designer-2026-09-17.md`).

## 1. O que aconteceu

O briefing da tarefa nomeava, item por item, o que o PhxSql tem e o que não tem.
Sobre o «modelo local Ollama × nuvem» do mockup, ele dizia:

> «"LLM plugável" exigiria cliente HTTP de saída, que NÃO existe —
> `docs/SAUDE-DO-DISCO.md` diz que não há cliente HTTP de saída.»

A citação está **certa**: `docs/SAUDE-DO-DISCO.md:23` mede que os únicos
`GET`/`POST` escritos num `TcpStream` nesta casa são de **testes** contra o
próprio servidor (`servidor.rs:42072`, `:42198`). A conclusão está **errada** —
e ela custaria o item inteiro, que sairia do parecer marcado como barrado por
pétrea.

O que o fonte diz: a chamada de IA daqui **não sai do servidor**. Sai do `fetch`
da própria tela — `docs/CLAUDE-IA.md` §1 («Por que a chamada sai do NAVEGADOR, e
não do servidor»), `ui/claude.js:92`. A decisão está registrada com as três
saídas possíveis e o custo de cada uma, e a escolhida foi «não passar pelo
servidor», justamente porque a `std` do Rust não tem TLS e crate de TLS quebra a
primeira regra da casa.

O Ollama local atende em `http://localhost:11434`, **sem TLS**, e a nossa página
também é servida sem TLS. Então o item custa **uma origem a mais no `connect-src`
da página** — `src/http.rs:317`, hoje
`connect-src 'self' {ORIGEM_ANTHROPIC}` —, pelo mesmo mecanismo, com a mesma
folga mínima, travável pelo mesmo par de testes que já existe
(`a_pagina_pode_chamar_a_api_da_anthropic` e o irmão
`a_resposta_de_dados_continua_so_com_a_propria_origem`).

## 2. O que eu concluí primeiro, e estava errado

Concluí o mesmo que o briefing, e pela mesma razão: **eu conheço a pétrea de
zero dependências, e LLM local soa como «o servidor vai falar HTTP com alguém»**.
Cheguei a escrever a linha da matriz com «barrado por pétrea» antes de abrir o
`CLAUDE-IA.md`.

O erro não foi de fato, foi de **camada**: medi a ausência na camada onde ela
existe (o servidor) e apliquei o resultado a um recurso que **não usa aquela
camada** (a tela). O diagnóstico era plausível, a citação era verdadeira, e o
veredito era falso — a pior combinação, porque ela não convida ninguém a
conferir.

## 3. O que a medição disse

| pergunta | medido | onde |
|---|---|---|
| existe cliente HTTP de saída no servidor? | **não** | `docs/SAUDE-DO-DISCO.md:23` |
| a chamada de IA passa pelo servidor? | **não**, sai do `fetch` da tela | `docs/CLAUDE-IA.md` §1; `ui/claude.js:92` |
| o que o servidor cedeu para isso funcionar? | **uma linha de CSP**, só na página e só em `connect-src` | `src/http.rs:317` |
| quantas linhas custaria uma segunda origem? | **1** | idem |
| a página web manda a op `sql` de quantos lugares? | **1**, e é dentro do painel de IA | `ui/claude.js:1317` (`grep -c` em todos os `.js` e no `index.html`) |

O último número é o que fecha o raciocínio: a interface inteira só alcança a
camada SQL **pelo painel de IA**. Ou seja, o «LLM plugável» não é um enfeite
periférico — é o único caminho da tela até o SQL, e por isso medir a premissa
dele errado teria fechado a porta mais usada.

## 4. A regra

**Recusa citada é recusa que não se mede.** Quando um item chega barrado por uma
restrição nossa, confira em que CAMADA a restrição vive e em que camada o item
roda — fato verdadeiro na camada errada é veredito falso que ninguém confere.

## 5. Como está guardado hoje

- O parecer registra a correção com o número, e **nomeia a premissa do briefing
  como errada em vez de corrigi-la em silêncio**:
  `docs/propostas/phoenix-query-designer-2026-09-17.md` §4.2.
- O que **sobra** de decisão está lá também, e é do dono, não desta lei:
  alargar o `connect-src` é política de superfície da página, e o `OLLAMA_ORIGINS`
  do lado do Ollama é configuração da máquina de quem usa.
- **Onde o buraco ficou:** não há guarda que pegue este naipe de erro, e eu não
  proponho uma — um conferidor que casasse «pétrea citada × camada do item»
  julgaria texto de parecer, e portão que interpreta prosa é portão que erra. O
  que existe é o precedente escrito: **três** casos nesta casa já foram «citação
  certa, veredito errado» — o `~20 toques de página por linha` que eram 10,86, o
  «mutex serializa» que custava 13,2 ns contra 3.456 µs do parse, e agora este. A
  família tem nome, e nome é o que faz a próxima ser reconhecida.
- E o alcance de uma lei que já existe, que é o que este arquivo acrescenta: «**a
  lista do que falta também é palpite até alguém medir**» (pedido 113) fala do que
  está **ausente**. Este caso mostra que ela vale igual para o que está declarado
  **impossível** — e a segunda metade é mais perigosa, porque ausência convida a
  medir e impossibilidade convida a passar adiante.
