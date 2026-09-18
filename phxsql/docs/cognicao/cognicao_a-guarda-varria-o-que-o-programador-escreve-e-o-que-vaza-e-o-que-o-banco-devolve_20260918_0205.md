# A guarda varria o que o programador escreve, e o que vaza é o que o banco devolve

- **Quando:** 2026-09-18, 02:05
- **Onde:** pedido 347 — o texto de tela do `phxsys.mensagens` chegando cru ao
  `innerHTML`
- **Custo:** um XSS de bloqueio que atravessou **seis** revisões desta casa e
  só apareceu num parecer de fora, com a lei já escrita em dois lugares e
  **443** usos certos ao lado dos 8 errados

## O que aconteceu

O texto de tela não é constante do programa: vem da tabela `phxsys.mensagens`,
que é tabela **comum** do motor. Oito `txt()` do painel de rodízio do Profiler
iam para o `innerHTML` sem escape, e um título de coluna da grade também.

Havia guarda para isso — `nenhum_texto_da_fabrica_traz_etiqueta_crua`. Ela
varre a `FABRICA_TELA`: os `texto!` do **fonte Rust**, isto é, o que o
programador escreveu. E o que vaza é o que o **banco devolve**.

A guarda estava do lado errado do fluxo de dados. Não estava frouxa nem
desligada: estava olhando a fonte certa para a pergunta errada.

## O que eu concluí primeiro, e estava errado

**Duas vezes.**

Primeiro, achei que o achado do parecer externo estava incompleto e que eu
tinha encontrado um décimo sítio: `grep '${txt('` no `index.html` devolveu
**9**, e o pedido nomeava 8 no Profiler. Fui cantar a lei de que «lei que lista
menos casos do que existem não protege». Fui ao fonte antes de escrever: na
linha 9726 o `txt()` entra cru num pedaço chamado `rot`, e o `rot` é escapado
**um nível acima**, no `esc(rot)` que vai para o SVG. Não era furo. Os nove do
relatório eram nove — **o falso positivo era meu**, e o crivo que o produziu
foi um grep sem contexto.

Segundo, quis a catraca em **zero**. Zero é mais forte e não tem exceção que
apodreça. Só que zero exigia escapar dentro do `rot` e **tirar** o `esc(rot)`
de fora — criando uma string «já escapada» que o próximo a concatenar não
saberia que é. Trocaria um furo real por uma armadilha futura. A catraca nasceu
em **1**, com nome e endereço.

## O que a medição disse

- `${txt(` sem escape no `index.html`: **9 → 1**. Os `${esc(txt(` subiram de
  **443 → 451**.
- O **443** é o número que mais diz: a receita certa já estava escrita no
  `docs/MENSAGENS.md` e já era seguida 443 vezes. A maioria não é garantia.
- Prova real no navegador, nos dois sentidos e na mesma corrida: com o defeito
  reposto num clone do componente, o veneno `<img src=x onerror=…>`
  **disparou**; com o arquivo do repositório, **não disparou**, e o título
  aparece como texto literal. Sem servidor e sem `cargo` — o defeito vive no
  componente, e uma prova que subisse o `phxsqld` mediria o servidor junto.
- O privilégio necessário para o ataque é **`alterar`**, não `administrar`: não
  existe conceito de database de sistema, só `e_coluna_de_sistema`, que é de
  coluna. Quem altera uma tabela do `phxsys` executa script no navegador de
  quem tem mais poder que ele.

## A regra

**Pergunte de que LADO do fluxo a sua guarda está.** Guarda que varre o fonte
prova o que o programador escreveu; o que chega do banco, da rede ou do
arquivo precisa de guarda no **sumidouro**, e as duas não se substituem.

E o corolário do crivo: **escape à distância existe e é legítimo**, então um
conferidor de texto local vai produzir falso positivo nele. Ou o crivo entende
o nível acima, ou a catraca nasce com o número desses casos e cada um
**nomeado** — nunca com o número escondido num teto redondo.

## Como está guardado hoje

Guardado em dois pedaços, e cada um diz o que **não** cobre:

1. `crates/phxsql-server/src/conferidor_texto_cru.rs` —
   `TETO_TXT_CRU_EM_HTML = 1`, com três provas: a catraca, a régua contra o
   zero por engano (fonte sintética nos dois sentidos, mais o caso do escape um
   nível acima) e `o_teto_de_um_ainda_tem_dono`, que manda **baixar o teto** se
   a única ocorrência sumir.
2. `testes-web/prova-xss-do-texto-de-tela.mjs` — o lado que nenhuma varredura
   de texto alcança: o valor que **viajou** (saiu de `txt()`, foi guardado num
   objeto de configuração e só depois virou HTML). Ela se declara **INVÁLIDA**,
   e não verde, se o defeito reposto deixar de disparar.

**O buraco que fica nomeado:** as duas guardas juntas cobrem a forma
`${txt(…)}` e o caminho da grade. Não há guarda para um terceiro caminho ainda
não imaginado — e o que este pedido ensina é justamente que a lei achava que
havia «dois caminhos» quando havia quatro. A defesa contra o quinto não é uma
lista maior: é a pergunta do lado do fluxo, escrita aqui.
