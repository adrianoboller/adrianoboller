# Cognição: o teto do FORMATO não é o teto do PROTOCOLO — 2⁶⁴−1 no disco, 2⁵³ no fio

**Descoberta:** 07/09/2026 17:30 UTC, medindo o topo da coluna `Sequence`
(bancada `bancada/sequencias/sonda.py`, bloco 19).

## 1. O que aconteceu

`docs/FORMATO.md` publica **2⁶⁴−1** como valor máximo do `rownum`, e a coluna
`Sequence` é um `u64` de 8 bytes (`types.rs:69`). Ajustando o contador para o
maior `i64` e inserindo duas linhas, as duas voltaram com **o mesmo número** na
tela.

## 2. O que eu concluí primeiro, e estava errado

**Duas vezes, e as duas com um diagnóstico plausível.**

Primeiro: *«é o `json` do Python no meio do caminho»*. É o palpite óbvio — o
cliente da sonda serializa e desserializa dos dois lados, e um cliente que
arredonda esconderia o motor. Medindo pelo **soquete cru**, sem `json` do Python
na leitura, o servidor mandou os inteiros exatos abaixo de 2⁵³ e arredondados
acima: o defeito é da casa, não do instrumento.

Segundo, e mais sutil: *«a perda é só no caminho do PEDIDO, porque o
`inteiro_ou` converte»*. Isso pareceu confirmado — mandei `9223372036854775807`
para o `ajustar_sequencia`, a resposta ecoou `9223372036854776000`, mas o
cabeçalho no disco ficou **exatamente** em `9223372036854775808`, o sucessor
certo. Conclusão errada e tentadora: *o valor sobrevive, só a resposta mente.*

Ele sobreviveu **por sorte**: `f64` de `i64::MAX` é 2⁶³, e o `as i64` da
conversão **satura de volta** para `i64::MAX`. Um único valor no universo faz
isso. Qualquer outro acima de 2⁵³ perde de verdade.

## 3. O que a medição disse

O `Json` desta casa tem **um único tipo numérico** — `Numero(f64)`,
`json.rs:20` —, e o formatador escreve como inteiro só abaixo de
`9.007199254740992e15`, que é 2⁵³. A prova pôs o valor **mandado** numa coluna
de texto ao lado, para a divergência não depender de fé:

```
"linhas":[{"rowid":1,"id":9007199254740992,"c":"9007199254740992",...},
          {"rowid":2,"id":9007199254740992,"c":"9007199254740993",...},
          {"rowid":3,"id":9007199254740996,"c":"9007199254740995",...}]
```

**2 de 3 divergiram**, na gravação, em silêncio, sem erro nenhum. O teto prático
de uma `Sequence` é **9.007.199.254.740.992** — 2.048 vezes menor que o do
formato.

## 4. A regra

**Todo teto publicado tem de dizer de QUE camada ele é.** Um número que o disco
guarda e o fio não carrega é um teto que só existe no papel — e o pior tipo de
erro, porque a documentação o confirma.

E o instrumento: **para provar que um valor sobreviveu, guarde ao lado dele o
valor MANDADO, em texto.** Comparar a resposta com o que se pediu não prova
nada quando os dois passam pelo mesmo conversor.

## 5. Como está guardado hoje

- Bloco 19 da sonda, com a resposta **crua** do soquete e a coluna de controle
  em texto; o `resultados.json` guarda o par mandado/gravado.
- `docs/AUTONUMBER.md` §A.3 (item 6) e §B.2.6, com as duas saídas: variante
  `Inteiro(i64)` no `Json` (correção de raiz, mexe num tipo que o servidor
  inteiro usa) ou **recusar cedo** acima de 2⁵³, com a mensagem que explica.
- **O buraco que fica:** o `docs/FORMATO.md` continua publicando 2⁶⁴−1 sem
  dizer que é o teto **do disco**. Corrigi-lo é do orquestrador — esta frente não
  edita o `FORMATO.md`.
