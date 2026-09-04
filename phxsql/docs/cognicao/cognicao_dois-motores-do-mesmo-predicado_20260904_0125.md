# Dois motores do mesmo predicado, e o pushdown que some com linha

**Assunto:** empurrar o filtro da grade para o servidor quando os dois lados
têm implementações próprias do mesmo operador.
**Descoberto em:** 04/09/2026, 01h25, montando o `WHERE` do `varrer`.

## 1. O que aconteceu

O `varrer` ganhou predicado (`"onde"`), e o passo seguinte era óbvio: a grade
já sabe montar filtro desde a 0.7.0, então bastava traduzir o que ela serializa
para o contrato do servidor. Os tipos batiam quase um a um — `texto/contem`
para `contem`, `faixa` para `>=`/`<=`, `expr` para o operador, `valores` para
`=`.

Só que há **dois motores de filtro** aqui, escritos por gente diferente em
linguagens diferentes:

- `passaCondicao`, em `crates/phxsql-server/ui/grid/phx-grid.js:114`;
- `casa`, em `crates/phxsql-store/src/memoria.rs:547`.

E eles **não concordam**. Lendo os dois lado a lado:

| filtro | a grade faz | o servidor faz | concordam? |
|---|---|---|---|
| `contem` | `semAcento(v).indexOf(...)` — sem acento **e** sem caixa | `to_lowercase().contains()` — sem caixa, **com** acento | **não** |
| `faixa`/`expr` em coluna de TEXTO | `String(v).toLowerCase()` | `x.cmp(y)` sobre os bytes | **não** |
| `faixa`/`expr` em coluna INTEIRA | `Number(v)` | comparação de inteiro | sim |
| `valores` com um valor | `String(v) === String(alvo)` | `Igual` sobre a cadeia | sim |

Buscar «sao» acha «São Paulo» na grade e **não acha** no servidor. Traduzido
sem conferir, esse filtro faria a linha sumir — e sumir calado, porque quem
olha a tela não sabe o que não veio.

## 2. O que eu concluí primeiro, e estava errado

Que a pergunta era **«dá para traduzir?»**. Ela não é. O `contem` da grade
traduz perfeitamente para o `contem` do servidor: mesmo nome, mesma aridade,
mesma família. A tradução é limpa e a resposta é diferente.

A pergunta é **«as duas peneiras concordam?»** — e ela não se responde olhando
o contrato, só lendo as duas implementações.

Errei uma segunda vez no contrato: desenhei o `max` do `varrer` como «linhas
DEVOLVIDAS», com um `examinar` novo para limitar a varredura — o formato de um
`LIMIT` de SQL. Isso faz um filtro pouco seletivo varrer a tabela inteira com a
trava global na mão, e **esse custo eu não tinha medido**. O `max` ficou como
sempre foi: linhas EXAMINADAS.

## 3. O que a medição disse

Medido com `cargo run --release -p phxsql-server --example onde-doi-no-varrer`,
tabela de 100.000 linhas, `max=2500`, mediana de 7 rodadas intercaladas:

| camada | 1 em 100 casando | 1 em 2 casando |
|---|---|---|
| varredura (`pagina` + `ler`) | 12.846 µs — **48,0%** | 13.199 µs — **46,3%** |
| montar o JSON | +1.223 µs — 4,6% | +1.526 µs — 5,4% |
| serializar em texto | +2.643 µs — 9,9% | +2.871 µs — 10,1% |
| fio + análise no cliente | +10.043 µs — 37,5% | +10.895 µs — 38,2% |
| **total de antes** | **26.755 µs** | **28.491 µs** |
| **medido com `onde`** | **12.902 µs = 2,07×** | **21.098 µs = 1,35×** |
| bytes no fio | 532.777 → 5.638 (**94,5×**) | 533.050 → 266.843 (2,0×) |

O teto previsto pelo modelo (varredura inteira + transporte proporcional) era
**2,06×** e **1,37×**. O medido bateu nos dois casos, e é por isso que o modelo
vale para prever o terceiro.

A parte que o `WHERE` **não** compra está no número da primeira linha: quase
metade do custo é ler as linhas, e nenhum predicado sem índice remove isso.

## 4. A regra

**Antes de empurrar um filtro para o outro lado, prove que as duas peneiras
concordam — e desça só a interseção provada. E deixe o cliente REAPLICAR
tudo: assim o pushdown só pode diminuir o que atravessa o fio, nunca mudar a
resposta.**

A reaplicação é o que torna a interseção barata de escolher. Sem ela, cada
tradução seria uma aposta; com ela, o pior caso de um filtro que ficou de fora
é gastar banda — e o de um traduzido errado seria perder linha, que é o motivo
de a lista do que desce ser curta e escrita.

## 5. Como está guardado hoje

- A lista do que desce está em `ondeDoServidor`, em
  `crates/phxsql-server/ui/index.html`, com a tabela da divergência no
  comentário — quem for acrescentar um tipo lê por que os outros não estão lá.
- A reaplicação é da própria grade: `fonteVarrer` delega a
  `PhxGrid._fonteLocal`, que roda **todos** os filtros sobre o que voltou. O
  `_fonteLocal` passou a ser exportado por causa disto (phx-grid 0.9.3).
- A divergência entre `casa` e `passaCondicao` **continua existindo** — este
  aprendizado não a conserta, contorna. O buraco fica nomeado: unificar exigiria
  ou o servidor tirar acento (e aí `contem` deixa de casar byte a byte, que é
  outra decisão de DBA), ou a grade parar de tirar (e aí a busca da tela piora).
  Nenhuma das duas se decide sem alguém pedir.
- Do lado do motor, `casa` é agora **uma só** para `varrer` e `SelectMemory`
  (ficou `pub` em `phxsql-store::memoria`), e
  `o_filtro_do_varrer_e_o_do_selectmemory` é o teste que acusa se alguém
  escrever a segunda cópia.
