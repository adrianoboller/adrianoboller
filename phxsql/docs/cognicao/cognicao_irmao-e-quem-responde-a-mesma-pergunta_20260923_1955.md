# Irmão não é quem chama a mesma função: é quem responde a mesma PERGUNTA

Descoberto em 23/09/2026, entre 19:20 e 19:55 UTC, consertando o pedido 392
(a escala do `Decimal` corrompendo o valor em 100×).

## 1. O que aconteceu

O pedido 392 dizia: no `unir`, a escala do primeiro braço corrompe o valor dos
outros em 100× — `10,5000` saindo `1050,00`. Fui reproduzir e **o `unir` já
estava consertado** no HEAD: a frente do `DISTINCT`/`UNION` (commit `0f7cb29`)
tinha entrado com `mais_largo` e `converter_para` no mesmo dia. A célula do
pedido continuava aberta porque ninguém a fechou, não porque o defeito vivesse.

Aí a pergunta virou outra: **quem mais lê a escala de um lado para interpretar
o valor do outro?** A lista que veio comigo era por nome (`op_unir`,
`op_diferencas`, `op_pivotar`, os agregados do 418). Medi as cinco, e o mapa
saiu torto em relação aos nomes:

- `consultar` + `agregados` (`soma` sobre `Decimal`) — o suspeito mais citado —
  **não tinha defeito**: cada coluna soma na própria escala.
- `op_juntar` — **não tinha**: a frente do 392 já o tinha corrigido.
- `diferencas`, `pivotar` e a junção da **composição** — os três quebravam, e
  nenhum dos três chama `juncao::unir`.

## 2. O que eu concluí primeiro, e estava errado

Concluí que o irmão do `unir` seria quem **chama as mesmas funções** — foi
assim que a lei do irmão está escrita, e foi assim que eu li a lista. Por esse
critério, `op_diferencas` e `op_pivotar` **não são irmãos do `unir`**: não
passam por `juncao::unir`, não montam cabeçalho de união, não compartilham uma
linha de código com ele.

E estava errado nos dois sentidos. Pelo critério de chamada, eu teria ido
procurar em `juncao.rs` (onde já estava consertado) e nos agregados do 418
(onde não havia defeito) — e teria passado ao largo dos três caminhos que
publicavam número errado naquele minuto.

## 3. O que a medição disse

Medido pelo protocolo, com `valor Decimal(12,2)` contra `Decimal(12,4)`:

| caminho | antes | depois |
|---|---|---|
| `unir` (`tabelas`, `partes`) | já certo no HEAD | igual |
| `juntar` (tabela) | já certo | igual |
| `diferencas`, chave | `10,5000` publicado como **`1050,00`** (100×) | `10,5000` |
| `diferencas`, linha | `7,25` e `0,0725` contavam **`iguais`** | `diferentes` |
| `pivotar`, junção | `7,25` casava `0,0725`; `10,50` não casava `10,5000` | casa por valor |
| `pivotar`, junção por `Date` | **nunca** casava | casa |
| `consultar` junção / `IN` / `EXISTS` | **0 linhas** onde o SQL devolve 1 | 1 linha |
| `consultar` + `soma` (418) | **sem defeito** | igual |

O que os três quebrados têm em comum não é chamada nenhuma: é **a pergunta**
— «estes dois valores são o mesmo?» — respondida por quatro funções
diferentes, cada uma com um jeito próprio de escrever a chave:
`juncao::pedaco_de_chave` (certa), `memoria::chave` (sem tipo),
`pivot::rotulo(v, 0)` + `pivot::rotulo_cru` (duas formas, e nem entre si
coincidiam) e `consultar::chave_de_juncao` (sobre o texto do JSON).

E o `pivotar` mostra o preço de ter duas: o mapa era montado com uma forma e
procurado com outra. No `Decimal` as duas escreviam o inteiro guardado, e por
isso o erro era simétrico e passava por «funciona». Na `Date` elas divergiam
(`2026-09-23` contra o número de dias) e a junção **nunca** casava — um defeito
que estava lá desde sempre e que nenhum teste de decimal acharia.

## 4. A regra

**Ao procurar o irmão, procure quem responde a MESMA PERGUNTA, e não quem
chama a mesma função.** Quando a pergunta for «estes dois valores são o
mesmo?», a resposta tem de sair de **uma** função, e ela tem de receber o
**tipo de cada lado**.

## 5. Como está guardado hoje

`juncao::pedaco_de_chave` virou `pub(crate)` com quatro clientes nomeados no
próprio comentário (junção, `UNION` distinto, `diferencas`, `pivotar`), e
`sem_zeros_a_direita` também, para a composição — que compara em texto — usar a
mesma forma. Sete provas de protocolo em `servidor.rs::testes_escala_decimal` e
três unitárias (`diferencas`, `consultar`), todas com a prova real feita nos
dois sentidos: cada uma falha com o defeito reposto, e o vermelho está citado
no comentário de cada teste.

**Onde o buraco ficou**, e é honesto dizer: não há conferidor que ache uma
quinta forma de escrever chave nascendo amanhã. O que existe é o comentário de
`pedaco_de_chave` listando os quatro clientes — e lista em comentário envelhece
calado, como toda lista digitada. O dia em que alguém escrever a quinta, nada
acusa.
