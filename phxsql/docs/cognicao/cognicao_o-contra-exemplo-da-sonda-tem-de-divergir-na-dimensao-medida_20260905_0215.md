# O contra-exemplo da sonda tem de divergir na dimensão que a sonda mede

**05/09/2026, 02:15** — descoberto medindo as premissas da onda 2, antes de
montar as frentes.

## 1. O que aconteceu

A onda 2 abriu com dois itens de durabilidade. O primeiro — «o volume do meio
nunca é sincronizado» — nasceu de ler `RegFile::sincronizar`
(`crates/phxsql-store/src/reg.rs:2025`), que abre exatamente **dois** volumes:

```rust
self.volumes.abrir_para_sincronizar(1)?;
let (fronteira, _) = self.localizar(self.slot_count.max(1));
if fronteira != 1 { self.volumes.abrir_para_sincronizar(fronteira)?; }
```

Numa tabela com cinco volumes, o 3 não aparece ali. A leitura é convincente e
está errada.

Escrevi `crates/phxsql-store/examples/sonda-do-volume-do-meio.rs` para medir, e
a sonda respondeu a pergunta do título na primeira corrida. Depois disso ela
foi usada para uma **segunda** pergunta — a da premissa escrita no comentário
do `ESCRITAS_PENDENTES` —, e foi aí que a armadilha apareceu: a primeira
versão dessa segunda metade mediu a coisa errada e **teria publicado a
conclusão certa pelo motivo errado**.

## 2. O que eu concluí primeiro, e estava errado

Duas conclusões, e as duas erradas por motivos diferentes.

**A primeira:** «`RegFile::sincronizar` só abre o 1 e a fronteira, logo o
volume do meio fica sem `fsync`». É a mesma forma do erro que o pedido 168 já
custou nesta casa — confundir *chama a função que faz X* com *faz X*. Quem
decide a lista final não é `RegFile::sincronizar`, é
`Volumes::sincronizar_listas`, que **une** os descritores abertos com o
registro de escritas pendentes do processo.

**A segunda, e é a que ensina mais:** para medir se duas grafias do mesmo
diretório dividem a família do registro, escrevi a grafia «torta» como
`/tmp/./phx-meio-N` — o mesmo caminho com um `.` no meio. A sonda respondeu
«a marca atravessa, o volume sujo vai ao disco», e eu quase escrevi que a
premissa do comentário estava errada com esse número na mão.

Estava errada, mas **não pelo que aquela corrida mediu**. `/tmp/./x` e `/tmp/x`
não dividem a família porque `PathBuf` compara por `components()`, e o
componente `CurDir` some ali: são a **mesma chave** do `BTreeMap`. A sonda
tinha medido «uma chave igual continua igual» e eu ia ler isso como «grafias
diferentes não separam».

## 3. O que a medição disse

Com `strace -f -y -e trace=fsync` e cerca por fase, corrida de 05/09/2026:

```text
fase 1, semeadura de 100 linhas em 5 volumes:            .reg -> 001,002,003,004,005
fase 2, altera o rowid 50 (volume 3) e morre:            .reg -> nenhum
fase 3, o fecho da janela reabre e sincroniza:           .reg -> 001, 003, 005
```

A fase 3 responde o título: **o volume do meio entra**, e o 2 e o 4 — que estão
limpos — ficam de fora. É a lista certa, e ela vem do registro da onda 1.

Trocando a grafia torta de `/tmp/./x` para um caminho **relativo**, que divide
a chave de verdade:

```text
fase 4, altera o rowid 30 (volume 2) pela grafia relativa e morre
fase 5, o fecho pela grafia absoluta:                    .reg -> 001, 005
```

O volume 2, sujo, **não vai ao disco**. Com `/tmp/./x` a mesma fase saía
`001, 002, 005` — a resposta oposta, e é por isso que a escolha do
contra-exemplo decidia o resultado.

E o número mata a segunda metade do comentário do
`crates/phxsql-store/src/volume.rs:132`, que afirma que a degradação por grafia
é «a mesma de acima, para o comportamento antigo, nunca para menos que ele»: o
comportamento antigo é o `abrir_para_sincronizar(1)` mais a fronteira, e a fase
3 acabou de provar que ele **não alcança o volume do meio**. A degradação perde
exatamente o volume que o registro salva. Não é benigna.

## 4. A regra

**O contra-exemplo de uma sonda tem de divergir na dimensão que a sonda mede —
e provar que diverge antes de valer como contra-exemplo.** Um «caso diferente»
que o código trata como igual não mede nada, e é pior que não medir: ele
devolve um número, e o número parece resposta.

O corolário prático: quando a chave é um tipo com igualdade **não estrutural**
— e `PathBuf` é um deles, porque compara por `components()` —, duas grafias
diferentes na tela podem ser a mesma chave no mapa. Confira a igualdade da
chave antes de montar o experimento em cima dela.

## 5. Como está guardado hoje

A sonda ficou no repositório com as duas perguntas e o motivo da escolha
escrito por dentro, no comentário da fase 4: *«a grafia torta é relativa de
propósito, e não `/tmp/./x`: medido nesta mesma sonda, o `.` redundante não
divide a família»*. Quem a reabrir daqui a seis meses lê a armadilha junto com
o experimento, em vez de refazê-la.

**O buraco, e ele fica nomeado:** a sonda é um `--example` e **não roda no
`cargo test`**. Uma sonda que ninguém chama envelhece sem avisar — é a mesma
forma do que a cognição
`cognicao_lei-so-vale-onde-alguem-a-pode-chamar_20260904_1133.md` registrou
para a lei do crivo. Fechar isso é da frente D2: o teste que trava o caso, e
que tem de falhar com o defeito reposto.

E o que a medição **não** respondeu, que é o que decide se há conserto de
código: se o `phxsqld` chega a produzir duas grafias do mesmo diretório de
tabela. Enquanto esse número não sair, o achado é um risco medido e não um
defeito medido — e a diferença entre os dois é exatamente o que esta casa cobra
de qualquer receita de fora.
