# Nome de sistema isento dentro de um template literal deixa de ser isento

*Descoberto em 05/09/2026, ~00:43 — papel H (tradutor), leva de idiomas.*

## 1. O que aconteceu

Ao converter os títulos de `folha()` das telas de SysTables e SysColumns
(`ui/index.html`), a forma original era

```js
folha(`SysTables · ${db}`, ...)
```

`SysTables` está na lista `ISENTOS` do `conferidor.rs` — «nome de tabela de
sistema», mesmo em toda língua. A primeira tentativa de tirar isto da conta do
`TETO_ROTULOS_E_CRASE` foi

```js
folha("SysTables · " + db, ...)
```

trocando o template literal por concatenação, mantendo o separador ` · `
dentro do mesmo literal que abre a chamada. Rodado o conferidor: **continuava
cravado**, agora relatado como `SysTables ·` em vez de `SysTables · ${db}`.

## 2. O que eu concluí primeiro, e estava errado

Concluí que trocar backtick por concatenação bastava — que o `literal()` do
conferidor pegaria só o primeiro literal da expressão (`"SysTables · "`), e que
esse literal, começando pela palavra isenta, herdaria a isenção. Parecia
razoável: a via 2 (rótulo) já lê só o **primeiro** literal que encontra depois
de `folha(`.

Errado. O `literal()` lê exatamente os caracteres entre as aspas que abrem e
fecham — `"SysTables · "` inteiro, com o separador dentro —, e o `isento()`
julga essa string **inteira**. `"SysTables · "` tem espaço e letra minúscula
misturada com maiúscula: não bate em nenhuma regra de isenção (não é sigla, não
é identificador, não está na lista literal). O texto continuava "fora da
fábrica" com outra cara.

## 3. O que a medição disse

O conferidor, rodado depois de cada tentativa:

| forma | o que o `literal()` extrai | isento? |
|---|---|---|
| `` `SysTables · ${db}` `` | `SysTables ·` (miolo, após `${db}` virar buraco) | não — conta como cravado |
| `"SysTables · " + db` | `SysTables ·` (o separador ficou dentro da aspa) | não — conta como cravado |
| `"SysTables" + " · " + db` | `SysTables` (só o primeiro literal, aspa fecha antes do espaço) | **sim** — é o item exato da lista `ISENTOS` |

Só a terceira forma zerou a contagem para aquela linha. A prova real: reverti
para a segunda forma e o `cargo run --example textos-fora-da-fabrica` voltou a
listar `SysTables ·` como cravado; com a terceira, sumiu.

## 4. A regra

**Para um nome isento seguido de dado dinâmico, o nome tem de ser o literal
INTEIRO e sozinho — nunca compartilhar aspas com pontuação ou separador.**
`"Nome" + " · " + variavel`, nunca `"Nome · " + variavel` nem
`` `Nome · ${variavel}` ``. O separador entra no literal seguinte (ou fica solto
na concatenação), porque o crivo de isenção julga o literal pela igualdade
exata com a lista — um caractere a mais já tira o item da lista.

E o corolário que quase virou um segundo defeito: se em vez disto eu tivesse
criado uma chave nova na fábrica com o texto `"Nome · {var}"` idêntico nos
seis idiomas (porque `"Nome"` não se traduz e `·`/`{var}` também não mudam),
a guarda `nenhuma_chave_com_os_seis_idiomas_colados` teria reprovado — a menos
que o miolo (o texto sem os marcadores) tivesse três caracteres ou menos. Duas
guardas diferentes, duas armadilhas vizinhas; a saída que resolve as duas ao
mesmo tempo é a mesma: manter o nome isento **fora** de qualquer chave nova,
como literal isolado.

## 5. Como está guardado hoje

- `ui/index.html`, funções `verSysTables` e `verSysColumns`: os quatro
  `folha(...)` que citam o nome da tabela de sistema usam
  `"SysTables" + " · " + db` (e o mesmo para `SysColumns`, com o sufixo de
  tabela opcional concatenado à parte).
- Nenhuma chave nova entrou na `FABRICA_TELA` para esses títulos — de
  propósito, porque não há palavra ali que se traduza.
- `crates/phxsql-server/src/conferidor.rs`, `TETO_ROTULOS_E_CRASE`: desceu de
  1.706 para 1.051 na mesma leva que fechou este caso, junto com outras 654
  traduções — não é uma catraca dedicada a este achado, mas ele fazia parte da
  diferença antes do conserto.
- Não há teste automatizado dedicado a este padrão específico (nome isento +
  separador); a prova real ficou no antes/depois do placar do conferidor,
  citado na seção 3. Quem mexer de novo em `folha(` com um nome de sistema na
  frente deve repetir essa checagem — rodar o exemplo antes e depois da
  mudança de forma.
