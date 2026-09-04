# O teto que o medidor imprime não é o teto que o desenho compra

**04/09/2026, 06:15.** Descoberto lendo as duas baterias limpas de
concorrência, minutos depois de elas passarem.

## 1. O que aconteceu

Duas baterias de `escolher-o-desenho.py` passaram pelo `quieta.Vigia` (06:03 e
06:13). O relatório imprime quatro tetos, e um deles é o que decidiria a
Sombra:

```
MVCC, na espera    1.30x   (p99 do leitor COM escritor contra sozinho -- so o MVCC mexe)
```

O parêntese diz **«só o MVCC mexe»**, e está certo sobre o *par* que ele
compara. Mas o número não é o que o MVCC compra: o leitor com um escritor ao
lado paga **duas** coisas somadas — o custo de haver *qualquer* segundo cliente
na máquina, que o `RwLock` já recupera, e o custo de o segundo cliente ser um
**escritor**, que é a única parte que só o MVCC endereça.

A conta que separa as duas o relatório **não imprime**: é
`leitor-com-escritor ÷ dois-leitores`.

## 2. O que eu concluí primeiro, e estava errado

**Duas vezes, na mesma meia hora.**

**Primeiro:** li `MVCC, na espera 1.30x` e escrevi ao Adriano que *«o MVCC tem
1,30× para recuperar»*. Errado — 1,30× é o que **qualquer** desenho que
paralelize o segundo cliente recupera, e o `RwLock` é um deles. O teto próprio
do MVCC estava numa divisão que ninguém tinha feito.

**Depois:** ao escrever a §11.3 do `CONCORRENCIA.md`, sentenciei que o
`RwLock` *«custa muito menos — trocá-lo é uma decisão de uma linha»*. **É
exatamente a premissa que o pedido 164 já tinha matado**, e que a §2 **do mesmo
documento** desmente em letras maiúsculas: `RwLock<Instancia>` compila de
primeira e está errado, porque nenhum método pede `&mut` e o tipo não tem o que
proteger. Eu tinha lido a §1.3 vinte minutos antes, e ainda assim escrevi o
atalho — porque li «o `RwLock` não compila hoje (o marcador `!Sync`)» como
«falta uma linha», quando a guarda existe justamente para virar um erro
**silencioso** em erro de compilação. **Ler a guarda ao contrário.**

## 3. O que a medição disse

| corrida | sozinho | 2 leitores | c/ escritor | o teto IMPRESSO | **o teto EXCLUSIVO** |
|---|---:|---:|---:|---:|---:|
| A, `por_lote` | 6.583 µs | 8.546 | 8.586 | 1,30× | **1,00×** |
| B, `por_lote` | 7.317 µs | 9.580 | 8.704 | 1,19× | **0,91×** |
| A, `por_operacao` | 6.906 µs | 8.407 | 9.525 | 1,38× | **1,13×** |
| B, `por_operacao` | 7.071 µs | 8.638 | 8.780 | 1,24× | **1,02×** |

**O teto impresso: 1,19×–1,38×. O teto exclusivo: 0,91×–1,13×** — com uma
corrida em que o escritor ao lado saiu **mais barato** que outro leitor ao
lado.

E a conferência que impede chamar isso de «o medidor é ruidoso demais»: na
**mesma** bancada, o `RwLock` na espera deu **1,30 · 1,31 · 1,22 · 1,22**,
quatro medições dentro de 7% umas das outras, e a vazão dele deu
**2,48×–2,99×**. *O instrumento não é cego a diferenças reais; é cego a esta*,
porque esta não existe.

## 4. A regra

**O teto que o medidor imprime é do PAR que ele compara, não do desenho que o
paga. Antes de creditar um ganho a um desenho, desconte o que os outros
desenhos já recuperam.**

E a segunda, que é a do erro nº 2: **guarda que impede algo de compilar não é
uma linha que falta — é o aviso de que a coisa é mais cara do que parece.**

## 5. Como está guardado hoje

* **A conta exclusiva está escrita** na §11.2 do `docs/CONCORRENCIA.md`, com as
  quatro corridas, e o pedido 179 a carrega.
* **A §11.3 diz o que ela quase mentiu**, com o motivo — e não em silêncio:
  apagar a frase errada teria deixado a página certa e a lição perdida.
* **As corridas cruas estão versionadas** em
  `bancada/concorrencia/corridas/`, para que se confira depois se o número do
  documento saiu delas ou da memória de quem escreveu.
* **O buraco que eu ia deixar aberto fechou na mesma rodada.** Esta seção
  dizia «a coluna foi calculada à mão, fora do medidor — nomeado, e não
  feito», e isso deixaria a próxima pessoa lendo 1,30× e concluindo o que eu
  concluí. Hoje o `escolher-o-desenho.py` imprime **`MVCC, EXCLUSIVO`** ao
  lado de `MVCC, na espera`, e tem `--autoteste` com as quatro medições de
  04/09 gravadas. **Prova real nos dois sentidos:** com o denominador trocado
  de volta para o leitor sozinho, as quatro linhas falham e o autoteste sai
  **1**; com a conta certa, sai **0**. A segunda metade do autoteste é a que
  importa — ela exige que o denominador antigo dê um número **diferente**, e
  sem ela trocar o denominador de volta passaria despercebido.
* **E os dois IRMÃOS da receita errada foram atrás**, porque conserto entra no
  caminho que o motivou e o irmão fica: o *docstring* do próprio medidor e a
  §6.1 do `CONCORRENCIA.md` diziam ambos que *«este é o único par que o
  `RwLock` não mexe»*. Os dois foram corrigidos no mesmo commit. O terceiro
  lugar onde a frase aparecia — `PESQUISA-TRAVA-E-MVCC.md` §538 — **não** é
  irmão: ele diz *«o nosso gap medido é leitor-com-leitor»*, que é justamente
  o que esta medição confirmou.
