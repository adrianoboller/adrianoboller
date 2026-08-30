# Registrar a medicao e o veredito no DESEMPENHO
# 29/08 03:34

import io
p='docs/DESEMPENHO.md'
s=io.open(p,encoding='utf-8').read()

# --- 4.2: a secao do adiar, agora com o lote pronto e o ponto de virada ---
velho = """Contra os **2,54 s** que o `reindexar` de hoje cobra pelos dois. Ou seja: com
uma reconstrução em lote, «adiar os dois» sairia por volta de 1,25 + 0,3 =
**~1,55 s contra 3,93 s — perto de 2,5×**.

**Então a ordem de trabalho é a inversa da intuição:** primeiro a construção em
lote da B+tree, depois o adiamento. Adiar sem ela compra 1%; a construção em
lote sozinha já acelera todo `reindexar` e todo reparo, sem mexer no caminho de
escrita."""
novo = """Contra os **2,54 s** que o `reindexar` de hoje cobra pelos dois.

**Então a ordem de trabalho é a inversa da intuição:** primeiro a construção em
lote da B+tree, depois o adiamento. Adiar sem ela compra 1%; a construção em
lote sozinha já acelera todo `reindexar` e todo reparo, sem mexer no caminho de
escrita.

### 4.3 A construção em lote, feita — e o que ela mudou

`NdxFile::construir_em_lote` não desce a árvore nenhuma vez: ordena as chaves,
enche as folhas em sequência e monta os níveis de cima por cima dos de baixo.
Um milhão de chaves, `--example indice-em-lote`, duas corridas:

| | montar | páginas | varrer |
|---|---:|---:|---:|
| uma a uma (o `reindexar` de antes) | 7,72 s | 6.136 | 0,036 s |
| **em lote** | **0,31 s** | 5.271 | 0,028 s |

**23× a 25×.** E com o `reindexar` barato, o `--example indice-adiado` passou a
dizer outra coisa: adiar os dois índices vale **3,28×** (era 1,02×), e adiar só
o não único — o caminho que não abre mão da unicidade — vale **1,59×**.

O **enchimento das folhas** foi medido, e não herdado. 70% é a folga clássica e
não compra nada, porque inserção aleatória já assenta perto de 69% de ocupação
sozinha:

| enchimento | páginas | varrer | crescer 10% | páginas novas |
|---:|---:|---:|---:|---:|
| 70% | 6.028 | 0,035 s | 0,804 s | 0 |
| **80%** | **5.271** | **0,028 s** | **0,770 s** | **0** |
| 90% | 4.683 | 0,026 s | 0,901 s | 2.342 |
| 100% | 4.213 | 0,023 s | 0,984 s | 2.110 |

De 90% para cima a folha fica sem folga: crescer aloca milhares de páginas e
fica **mais lento** do que na árvore mais frouxa, e a varredura mais rápida não
paga isso. 80% é a ocupação mais densa que ainda absorve 10% de crescimento sem
alocar uma página.

> A primeira versão desse medidor deu 100% de graça, porque as chaves de
> crescimento entravam **acima** da faixa — e chave maior que todas vai sempre
> para a última folha, então a divisão que o enchimento deveria provocar nunca
> acontecia. Medidor com furo mede o furo.

### 4.4 E o adiamento em si: medido, e ele quase nunca compensa

Com o lote pronto, o adiamento virou item de implementar. **Medi antes**, e o
número o derrubou.

O 1,59× do `indice-adiado` é o caso da tabela **vazia**. Mas `reindexar`
reconstrói sobre a tabela **inteira**, e não sobre as linhas que acabaram de
entrar. Para uma tabela com 200.000 linhas, carregando M
(`--example adiar-vale-quando`):

| M | manter o índice | adiar (carga + refazer) | ganho |
|---:|---:|---:|---:|
| 200.000 (dobra a tabela) | 3,255 s | 2,149 + 0,512 = 2,662 s | **1,22×** |
| 100.000 | 1,620 s | 1,075 + 0,403 = 1,477 s | 1,10× |
| 40.000 | 0,653 s | 0,441 + 0,315 = 0,757 s | **0,86×** |
| 20.000 | 0,330 s | 0,219 + 0,286 = 0,505 s | 0,65× |
| 4.000 | 0,067 s | 0,044 + 0,264 = 0,308 s | 0,22× |

O ponto de virada fica perto de **M ≈ N/3**. Abaixo dele adiar **custa** tempo,
e o teto — dobrar a tabela de uma vez — vale 1,22×, e não os 1,59× que o caso
da tabela vazia sugeria.

E o preço não é só de tempo: adiar exigiria **marcar índice suspenso no
formato** do `.ndx`, porque uma queda no meio da carga deixaria uma árvore com
chaves faltando e nada dizendo isso — busca respondendo errado em silêncio, que
é o pior defeito que este projeto já teve três vezes. Formato novo, estado novo
que pode encalhar uma tabela, para ganhar 1,22× no melhor caso e perder na
maioria.

**Fica de fora, com o número na mesa.** O que o faria valer é outra coisa, e
maior: reconstruir só sobre as linhas novas e **fundir** a série ordenada na
árvore existente, em vez de refazê-la. Aí o custo passaria a depender de M, e
não de N+M."""
assert s.count(velho)==1
s=s.replace(velho,novo)

# --- 7: o roteiro ---
velho7 = """2. **Construção em lote da B+tree** — varrer, ordenar, encher as folhas em
   sequência. É o que falta para o adiamento do índice valer alguma coisa
   (§4.2: hoje ele vale 1,02×, porque `reindexar` insere chave a chave), e
   sozinha já acelera todo `reindexar` e todo reparo. Piso medido: 0,24 s
   contra 2,54 s."""
novo7 = """2. ~~**Construção em lote da B+tree**~~ — **feita** (§4.3): 23× a 25×, de
   7,72 s para 0,31 s num milhão de chaves. O adiamento que ela deveria
   destravar foi medido depois e **não compensa** (§4.4): ganha 1,22× no melhor
   caso, perde abaixo de M ≈ N/3, e cobraria um estado novo no formato."""
assert s.count(velho7)==1
s=s.replace(velho7,novo7)

velho8 = """cargo run --release --example custo-da-pagina -- 800000 200"""
novo8 = """cargo run --release --example custo-da-pagina -- 800000 200
cargo run --release --example indice-em-lote -- 1000000   # o lote do §4.3
cargo run --release --example adiar-vale-quando -- 200000 # o ponto de virada"""
assert s.count(velho8)==1
io.open(p,'w',encoding='utf-8').write(s.replace(velho8,novo8))
print('ok')
