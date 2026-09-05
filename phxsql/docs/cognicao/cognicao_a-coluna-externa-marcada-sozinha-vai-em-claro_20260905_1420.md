# A coluna externa marcada SOZINHA vai para o disco em claro

**05/09/2026, 14:20** — descoberto montando a bancada «com e sem senha» que o
dono pediu, e não procurando defeito nenhum.

## 1. O que aconteceu

O pedido era medir uma tabela complexa de 1.000.000 de linhas com e sem senha.
A primeira corrida de prova, com 2.000 linhas, devolveu um número bonito e
falso:

```text
disco 1.000x        insert 1.006x   select 1.000x   update 1.068x
```

Disco idêntico — 4.897.786 contra 4.897.978 bytes, e os 192 de diferença eram
`.log`, `.trash` e `.reason`. Se houvesse cifra, cada valor externo levaria
nonce e etiqueta, e o `.memo` teria crescido.

Eu ia publicar «a cifra não custa disco nenhum».

## 2. O que eu concluí primeiro, e estava errado

**Três vezes, em escada.**

**A primeira:** «a cifra não deve estar ligada». Medido pelo protocolo:
`{"op":"config"}` devolvia `cifra.ligada: true`, `iteracoes: 210000`,
`modo: "aead"`. Estava ligada, e o `Cifra::aplicar` (`config.rs:1109`) é
chamado de verdade em `config.rs:2581`.

**A segunda:** «então a cifra é por tabela e a tabela não foi declarada».
Também não: a cifra em repouso desta casa é **por coluna marcada**, e o
cabeçalho do `reg.rs` diz isso na linha 44 — *«cifra-se só a coluna marcada
como dado pessoal»*. Marquei as duas colunas pesadas com `dado_pessoal:
"sensivel"`, conferi pelo `siscolunas` que a marcação chegou ao esquema, e o
`.memo` **continuou em claro, byte a byte idêntico**.

**A terceira, e é a que quase virou número publicado:** o `select` saiu
**3,349×** mais lento no lado com senha, e eu li isso como «o custo de decifrar
por linha». Não é: 0,4 s contra 0,125 s são ~275 ms, que é o PBKDF2 de 210.000
voltas na **abertura** — um custo por arquivo, não por linha. Ele apareceu
justamente porque nada estava sendo cifrado: a chave era derivada e não usada.

## 3. O que a medição disse

A cadeia, lida no fonte depois que o número não fechou:

1. `faixas_pessoais` (`reg.rs:2117`) faz `continue` quando `col.ty.externo()`
   — coluna `Memo`/`Bin` marcada **não gera faixa**;
2. sem faixa, `faixas.is_empty()` e o volume nasce `Material::EM_CLARO`
   (`reg.rs:266`);
3. `selar_externo` (`reg.rs:1591`) devolve o dado intacto, porque
   `!self.material.cifrado()`.

A prova nos dois sentidos, dez linhas pelo soquete, mesma senha e mesma
marcação:

| colunas marcadas | `.memo` | texto em claro no disco? |
|---|---|---|
| só externas (`Memo` + `Bin`) | 6.264 B | **SIM** |
| externas **+ uma inline** (`nome`) | 6.664 B | não |

Os 400 bytes são os 40 por valor (nonce de 24 mais etiqueta de 16) das dez
linhas. **Uma coluna inline marcada muda o destino das outras duas.**

E o motivo de nenhum teste pegar: o `esquema()` do `cifra-dos-dados.rs` marca
`nome` (`Str`, inline) **além** de `obs` (`Memo`). Com uma inline marcada,
`faixas` não fica vazio e tudo funciona — a bateria inteira prova o caminho que
funciona, e o irmão nunca foi exercitado.

## 4. A regra

**Quando uma condição LIGA um mecanismo, confira se ela enxerga todos os casos
que o mecanismo trata.** Aqui o código de selar o externo está escrito e está
certo; o que não alcança é a condição que o liga, derivada só das colunas
inline. Guarda que existe e não é chamada protege tanto quanto guarda que não
existe.

E o corolário para bancada: **número que não muda é achado, não é resultado.**
`1.000x` no disco de uma cifra que deveria acrescentar 40 bytes por valor era a
denúncia, e eu quase a publiquei como conclusão.

## 5. Como está guardado hoje

A guarda está no repositório e está **VERMELHA de propósito**:
`coluna_externa_marcada_sozinha_nao_pode_ir_em_claro`, em
`crates/phxsql-store/tests/cifra-dos-dados.rs`, com `#[ignore]` e o motivo no
atributo. Roda com:

```bash
cargo test -p phxsql-store --test cifra-dos-dados -- --ignored coluna_externa
```

O `#[ignore]` é deliberado e não é conforto: sem ele a suíte inteira ficaria
vermelha e a catraca deixaria de segurar qualquer outra coisa. O que ele não
faz é esconder — o nome, o atributo e este documento dizem o que ela prova.

**O que NÃO foi feito, e por quê:** o conserto. Ele muda o que uma tabela
**nasce** sendo em disco, e formato em disco é palavra do papel C e decisão do
dono. O conserto aparente é uma linha — `faixas_pessoais` vazio não deveria
implicar `EM_CLARO` quando há coluna externa marcada —, mas ele tem
consequência de formato: o cabeçalho passa a ser o cifrado (`CAB_LEN_CIFRADO`)
e o slot ganha o rabo. Está proposto, medido e à espera.

**E o buraco maior, nomeado:** ninguém sabe quantas tabelas por aí estão nesse
estado. Não há conferidor que responda «esta tabela tem coluna marcada e nasceu
em claro?», e enquanto não houver, o alcance deste achado é uma suposição.
