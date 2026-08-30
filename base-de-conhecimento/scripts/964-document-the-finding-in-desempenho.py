# Document the finding in DESEMPENHO
# 29/08 00:56

import pathlib
p = pathlib.Path("docs/DESEMPENHO.md")
s = p.read_text()
alvo = '''**O que dá para fazer hoje, de graça:** quem importa um arquivo **já ordenado
pela chave primária** carrega 1,19× mais rápido. É uma linha de documentação,
não de código.'''
novo = '''**O que dá para fazer hoje, de graça:** quem importa um arquivo **já ordenado
pela chave primária** carrega 1,19× mais rápido. É uma linha de documentação,
não de código.

### 4.2 Parar o `.ndx` durante a carga e reconstruir no fim

A ideia é a mais tentadora da lista: durante uma carga, deixar o índice
parado — o `.reg` sozinho insere a **148 mil linhas/s** contra 54 mil com dois
índices — e reconstruir tudo de uma vez no fim.

```bash
cargo run --release --example indice-adiado -- 200000
```

Com a reconstrução **dentro da conta**, que é onde ela tem de estar:

| 200.000 linhas, chaves embaralhadas | inserir | reindexar | total | ganho |
|---|---:|---:|---:|---:|
| hoje (os dois índices na hora) | 3,93 s | — | 3,93 s | — |
| adiar **os dois** | 1,25 s | 2,54 s | 3,79 s | **1,02×** |
| adiar **só o não único** | 2,72 s | 1,19 s | 3,91 s | 1,01× |

**Um por cento.** E a razão está em três linhas de `Table::reindexar`:

```rust
while let Some((id, payload)) = self.reg.proximo_ativo(rowid)? {
    let chave = self.codificar_chave(i, &valores)?;
    self.ndx.inserir(i, &chave, id)?;   // ← uma descida na árvore por chave
}
```

Reconstruir hoje é **chave a chave** — o mesmo trabalho do caminho de dentro,
feito depois. Adiar não apaga trabalho: move de lugar. E como o trabalho movido
é idêntico, o total não muda.

### O ganho existe, e está no lote — não no adiar

Uma reconstrução **em lote** de verdade seria outra coisa: varrer o `.reg` uma
vez, codificar as chaves, **ordenar**, e encher as folhas em sequência montando
os níveis de cima por cima. Nenhuma descida. O piso disso, medido:

| | |
|---|---:|
| varrer o `.reg` e codificar 200.000 chaves (uma vez, serve aos dois índices) | 0,21 s |
| ordenar as chaves, por índice | 0,03 s |
| páginas a encher, em sequência | 4.047 |

Contra os **2,54 s** que o `reindexar` de hoje cobra pelos dois. Ou seja: com
uma reconstrução em lote, «adiar os dois» sairia por volta de 1,25 + 0,3 =
**~1,55 s contra 3,93 s — perto de 2,5×**.

**Então a ordem de trabalho é a inversa da intuição:** primeiro a construção em
lote da B+tree, depois o adiamento. Adiar sem ela compra 1%; a construção em
lote sozinha já acelera todo `reindexar` e todo reparo, sem mexer no caminho de
escrita.

E o teto tem uma trava que não é de desempenho: **o índice único não pode ser
adiado**. Ele é a própria decisão de aceitar ou recusar a linha, e a conferência
acontece antes de qualquer gravação porque o `.reg` nunca reaproveita slot —
descobrir a duplicata depois deixaria um buraco permanente por linha recusada.
Sobra adiar o não único, que na forma da bancada vale 1,19 s de 3,93.'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)

# a lista final de prioridades muda de ordem
alvo2 = '''2. **Índice não único adiado**, com fila e trabalhador de fundo. Não custa
   correção nenhuma, e o segundo índice ainda vale 5,9 µs de 18,5 (32%).'''
novo2 = '''2. **Construção em lote da B+tree** — varrer, ordenar, encher as folhas em
   sequência. É o que falta para o adiamento do índice valer alguma coisa
   (§4.2: hoje ele vale 1,02×, porque `reindexar` insere chave a chave), e
   sozinha já acelera todo `reindexar` e todo reparo. Piso medido: 0,24 s
   contra 2,54 s.'''
assert s.count(alvo2) == 1
s = s.replace(alvo2, novo2, 1)
p.write_text(s)
print("ok")
