# A catraca subiu e o código ficou mais barato — o terceiro caso da lei da régua

**Descoberto em 17/09/2026, 14:10 UTC**, ao medir por que a `alcancam-fsync` do
`mapa-da-trava.py` estava em 25 com teto 22, tendo sido 23 em 16/09.

## 1. O que aconteceu

O aviso de rodada (`comunicacao.sh`) imprimiu `SUBIU alcancam-fsync 25 (teto 22)`.
O registro do pedido 252 dizia **23**, com o denominador em **86** seções
críticas. Hoje: **25**, denominador **86**.

Varredura **linear** dos 13 commits que tocam `crates/phxsql-server/src` ou
`crates/phxsql-store/src` desde `595e3f2` — apontando o `mapa-da-trava.py` a
cada `servidor.rs` histórico, sem tocar o script versionado. O salto está em
`20d2c59` (17/09 06:06), e as duas seções são **`empilhar`** e
**`empilhar_atualizar_com_cascata`**.

E aí o achado: **nenhuma das duas passou a alcançar `fsync`.**

## 2. O que eu concluí primeiro, e estava errado

Escrevi no pedido 252, com o número na mão e vinte minutos antes desta medição:

> «Denominador igual com numerador subindo **não é régua que passou a medir
> mais**: são duas seções novas que alcançam `fsync` com a trava na mão.»

O raciocínio é bom e a conclusão é falsa. Ele supõe que só há **duas** causas
para uma catraca andar — seção nova, ou régua nova —, porque são as duas que a
pétrea nomeia. Existe uma terceira, e o denominador não a detecta: **o código
reorganizou uma porta compartilhada em porta privada**, e a mesma régua mudou de
veredito sem que o comportamento mudasse.

Errei duas vezes hoje pelo mesmo mecanismo: de manhã, dividindo um custo
proporcional a M por um proporcional a N; agora, deduzindo a causa de um número
em vez de medi-la. *O errado sobrevive melhor quando é plausível.*

## 3. O que a medição disse

O commit trocou, em `empilhar` e no irmão, `abrir_travada(...)` seguido de
`t.ver_so_o_disco()` pela porta nova `abrir_travada_sem_sobrepor(...)`. As duas
portas são invólucros de **uma linha** sobre a mesma função
(`servidor.rs:10524-10561`), diferindo num booleano:

```rust
fn abrir_travada(...)              -> self.abrir_travada_com(..., true)
fn abrir_travada_sem_sobrepor(...) -> self.abrir_travada_com(..., false)
```

E o comentário entre elas já dizia: *«O corpo das duas portas. UMA
implementacao»*. O caminho de durabilidade que o mapa imprime é **idêntico** nos
dois casos:

```
durabilidade   via abrir_travada_sem_sobrepor -> abrir_travada_com
               -> espelhar(2/2) -> sincronizar(10/10)
```

O que mudou foi o **corte da porta comum**, que é regra do próprio mapa
(`PORTA_COMUM`, 17 seções): uma porta usada por muitas seções vira `HERDADO` e
sai de `proprias` — «é a melhor prova em tanta seção que não distingue nenhuma».
A catraca conta `proprias`.

| | chamadas | seções | acima do corte? | durabilidade conta? |
|---|---:|---:|---|---|
| `abrir_travada` | 34 | 23–27 | sim (`HERDADO 18/86`) | **não** |
| `abrir_travada_sem_sobrepor` | 3 | 2 | não | **sim** |

**Duas seções atravessaram o corte, e é isso que os +2 medem.**

E a direção é perversa: o commit que levantou a catraca deixou o motor **mais
barato**. A porta nova existe para não montar a sobreposição da transação sob a
trava e apagá-la na linha seguinte — `O(pendentes)` por operação, `O(n²)` por
transação, escrito no `servidor.rs:10528-10536`. **A frente que economizou
trabalho sob a trava global foi a que a catraca acusou.**

## 4. A regra

**Antes de dizer por que uma catraca andou, meça a causa — o denominador não
distingue seção nova de reclassificação.** Quando o numerador sobe, o primeiro
passo não é o `git log`: é **listar** o conjunto medido antes e depois e diffar
os nomes. E há três causas, não duas: seção nova (desfaz-se), régua nova
(aposenta a catraca) e **porta que deixou de ser comum** (não se desfaz, porque
não há defeito — a régua é que mudou de lado sozinha).

## 5. Como está guardado hoje

**Mal guardado, e os buracos são três.**

- **O medidor conta e não lista.** O `--catraca` imprime o número, o relatório
  longo imprime `25/86`, e nenhum dos dois nomeia as seções. Esta medição só foi
  possível importando o script e chamando `mapear()` de fora. Um `--lista` no
  próprio medidor torna a próxima investigação trivial, e não existe.
- **A catraca não sabe distinguir o seu terceiro caso.** Ela compara um inteiro
  com um teto; a diferença entre «alguém acrescentou `fsync`» e «alguém tornou
  uma porta privada» não cabe num inteiro. Se guardasse o **conjunto de nomes**
  em vez do número, a própria saída diria qual dos três casos é — e o custo é um
  arquivo de nomes versionado ao lado do teto.
- **O aviso de rodada continua acusando sem diagnosticar**, e agora com um caso
  a mais: ele disse «SUBIU» para uma mudança que não tem o que desfazer. O pedido
  252 já registrava essa limitação para o 23 parado; ela agora tem precedente
  medido.

A correção está no pedido 252, no `PENDENCIAS.md`, ao lado da conclusão errada —
que fica, porque uma correção sem o erro ensina só a resposta.
