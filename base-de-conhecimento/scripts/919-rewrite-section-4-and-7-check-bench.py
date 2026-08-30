# Rewrite section 4 and 7; check bench
# 29/08 00:21

import pathlib
p = pathlib.Path("docs/DESEMPENHO.md")
s = p.read_text()

alvo = '''Há um terceiro caminho, que ninguém propôs e que a medição favorece: **manter o
índice no caminho crítico, mas em lote**. A carga em lote já provou o princípio
no nível de cima — 2.715 → 25.985 linhas/s, 9,6× — porque tudo que acontecia
*por linha* passou a acontecer uma vez. A B+tree ainda é reescrita nó a nó
dentro do lote; ordenar as chaves do lote antes de inseri-las atacaria
justamente os 83,5%.'''
novo = '''Há um terceiro caminho, que ninguém propôs: **manter o índice no caminho
crítico, mas em lote** — ordenar as chaves antes de descer a árvore, para que
chaves vizinhas caiam na mesma folha. Era o item que este documento colocava em
primeiro lugar. **A medição mudou o veredito, e a ordem.**

### 4.1 Ordenar as chaves do lote: o que a medição disse

```bash
cargo run --release --example ordem-da-chave -- 200000
```

As mesmas linhas, os mesmos índices, mudando só a **ordem** em que as chaves
chegam. A diferença entre as duas é o **teto** do que ordenar o lote pode
recuperar:

| Forma | crescentes | embaralhadas | a desordem custa |
|---|---:|---:|---:|
| 1 índice único, chave inteira | 12,5 µs | 13,5 µs | 1,08× |
| 1 índice único, chave de texto | 12,6 µs | 13,9 µs | 1,10× |
| 2 índices (a forma da bancada) | 17,9 µs | 21,3 µs | **1,19×** |

**Antes do cache de páginas, a desordem custava 1,06×** — e ordenar teria
comprado praticamente nada. A hipótese que colocava este item em primeiro lugar
estava certa sobre o alvo (o `.ndx`) e errada sobre o mecanismo: o custo não era
de *localidade*, era de **reler e recalcular CRC da mesma página**. Com tudo em
RAM, mudar a ordem de chegada não mudava nada.

Depois do cache, a localidade finalmente importa — e vale 1,19× na forma da
bancada.

**Não implementado, e a razão está no formato.** Ordenar as chaves de um lote
exige conhecer os rowids antes de inserir no `.ndx`, o que exige gravar o `.reg`
antes — e aí uma falha no meio da fase do índice deixa linhas gravadas sem
chave, sem como desfazer (o `.reg` não reaproveita slot). Hoje uma falha de
índice desfaz a linha inteira. Trocar isso por «rode `reindexar`» é rebaixar uma
garantia rara, mas real, por 1,19%… por **1,19×**. Fica registrado com o número,
para a decisão ser tomada com ele na mão e não sem ele.

**O que dá para fazer hoje, de graça:** quem importa um arquivo **já ordenado
pela chave primária** carrega 1,19× mais rápido. É uma linha de documentação,
não de código.'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)

# --- secao 7, a ordem do que vem depois
alvo = '''1. **Ordenar as chaves do lote antes de inserir no `.ndx`.** Ataca os 83,5% sem
   mudar formato nem garantia. É onde está o dinheiro.
2. **Fechar a conta do CRC** (§2). Se o CRC realmente domina o caminho de
   página, a saída é CRC incremental por nó em vez de recalcular a página.
3. **Índice não único adiado**, com fila e trabalhador de fundo. 1,45× medido,
   e não custa correção nenhuma.
4. **Buffer de escrita maior**, para baixar as 41 chamadas de sistema por linha.
   Ganho incerto — o disco não é quem espera —, mas é barato de medir.
5. **Trava por tabela.** Não acelera uma inserção; acelera o servidor com muita
   gente. É outro eixo, e o roteiro já o previa.'''
novo = '''1. **CRC incremental por nó**, em vez de recalcular a página inteira. A conta do
   CRC agora fecha (§2): das 2,06 páginas gravadas por linha, cada uma paga
   2,34 µs de CRC — **4,8 µs de 18,5, ou 26%**. É o maior pedaço isolado que
   sobrou, e é o mesmo alvo do cache por outro lado: o cache tirou o CRC da
   leitura, isto tiraria o da gravação.
2. **Índice não único adiado**, com fila e trabalhador de fundo. Não custa
   correção nenhuma, e o segundo índice ainda vale 5,9 µs de 18,5 (32%).
3. **Ordenar as chaves do lote** (§4.1): 1,19× medido, e uma garantia a
   rebaixar. O número está na mesa; a decisão não é técnica sozinha.
4. **Buffer de escrita maior**, para baixar as chamadas de sistema por linha.
   Ganho pequeno pela medida: um `lseek` custa 0,10 µs, e mesmo 41 por linha
   dariam 4,1 µs — enquanto o CRC de gravação sozinho já custa 4,8.
5. **Trava por tabela.** Não acelera uma inserção; acelera o servidor com muita
   gente. É outro eixo, e o roteiro já o previa.'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)
p.write_text(s)
print("ok")
