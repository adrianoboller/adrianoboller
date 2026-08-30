# Registrar as duas medicoes
# 29/08 04:45

import io
p='docs/DESEMPENHO.md'
s=io.open(p,encoding='utf-8').read()
anc = '''### O que continua sem explicação

A taxa de inserção ainda **cai com o tamanho** — 54.180 para 37.712 linhas/s —
e num processo só ela não cai. Sobrou, por lote de 50.000, cerca de **6,6 µs por
linha** que o `abrir` não explica mais. O suspeito seguinte é o **cache de
páginas do `.ndx` nascer vazio a cada processo**: `onde-doi` roda um processo e
mantém o cache quente do começo ao fim. Isso ainda não foi medido, e por isso
está escrito aqui como suspeita e não como causa.'''
assert s.count(anc)==1
novo = '''### O que sobrou, e o suspeito que a medição já derrubou

A taxa de inserção ainda **cai com o tamanho** — 54.180 para 37.712 linhas/s.

O suspeito era o **cache de páginas do `.ndx` nascer vazio a cada processo**.
`--example cache-frio` compara, no mesmo processo, um lote de 50.000 com o cache
herdado do lote anterior contra um lote logo depois de fechar e reabrir a
tabela:

| já tinha | cache | µs/linha | páginas lidas do arquivo |
|---:|---|---:|---:|
| 100.000 | quente | 16,20 | 0,00 |
| 150.000 | **FRIO** | 16,02 | 0,00 |
| 500.000 | quente | 16,23 | 0,00 |
| 550.000 | **FRIO** | 16,17 | 0,00 |

**±1,6%, e nenhuma página lida do arquivo nos dois casos.** O suspeito está
errado, e a razão é simples depois de vista: as chaves entram em ordem
crescente, então a inserção vai sempre para o caminho mais à direita — meia
dúzia de páginas, que o cache reaquece nas primeiras linhas do lote. **É o
quinto diagnóstico plausível que a medição derruba neste documento.**

### O tamanho do buraco, medido

Mesmo esquema, mesmo código, 6 milhões de linhas:

| | tempo | µs/linha |
|---|---:|---:|
| **um processo só** (`carga inserir 6000000`) | 104,4 s | **17,40** |
| a bancada, 120 processos de 50.000 | 138,3 s | 23,0 |

**33,9 s de diferença, ou ~283 ms por lote** — e nem a abertura do `.reg` (hoje
0,03 ms) nem o cache frio explicam isso. O que sobra por lote é o
`sincronizar()`, que a bancada faz uma vez a cada 50.000 linhas por definição —
e o `fsync` de um arquivo que cresce até 1,5 GiB. O relógio menos a CPU da
corrida inteira dá 13,8 s, então o `fsync` não pode ser a diferença toda.

**Está em aberto, e o número está aqui para quem for medir.** A corrida de um
processo foi feita com a máquina ocupada, o que a favorece menos, não mais.'''
io.open(p,'w',encoding='utf-8').write(s.replace(anc,novo))
print('ok')
