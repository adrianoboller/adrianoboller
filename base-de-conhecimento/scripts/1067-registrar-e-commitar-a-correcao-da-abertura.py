# Registrar e commitar a correcao da abertura
# 29/08 04:36

import io
p='docs/DESEMPENHO.md'
s=io.open(p,encoding='utf-8').read()
anc = '## 5. Por que LSM não cabe dentro do motor atual'
assert s.count(anc)==1
novo = '''## 4.6 Abrir a tabela lia o arquivo INTEIRO

A pergunta era simples: continuamos perdendo do MySQL(R) no insert mesmo com o
`BULKINSERT`? A resposta é sim — e procurar o porquê achou outra coisa.

O primeiro número que não fechava: num processo só, inserir custa **16,0 µs por
linha com 200 mil e 16,4 com seis milhões** — não degrada. Mas a bancada mostra
a taxa caindo de 54.180 para 37.712 linhas/s, e ela carrega em lotes de 50.000
**abrindo e fechando a tabela em cada lote** — 200 processos para dez milhões.

Então: abrir custa mais conforme a tabela cresce? `--example abrir-cresce`:

| linhas | abrir | `.reg` | `.ndx` | `.log` |
|---:|---:|---:|---:|---:|
| 500.000 | 35,45 ms | 34,02 | 0,00 | 0,00 |
| 1.000.000 | 68,99 ms | 68,85 | 0,01 | 0,00 |
| 1.500.000 | 105,32 ms | 104,10 | 0,00 | 0,01 |
| 2.000.000 | 142,57 ms | 138,80 | 0,00 | 0,00 |

Linear, e tudo no `.reg`. A causa, em uma linha de código:

```rust
let bruto = std::fs::read(&primeiro)?;   // o volume INTEIRO
```

Ele trazia o volume inteiro para a RAM para tirar dele **128 bytes de cabeçalho
e o bloco de esquema**. Numa tabela sem paginação esse volume é a tabela toda:
**69 ms por milhão de linhas**, a cada abertura. Duas leituras curtas no lugar:

**138,80 ms → 0,03 ms, e agora é plano.**

### O que isso valeu, e o que não valeu

| na bancada de 10 milhões | antes | depois |
|---|---:|---:|
| inserir | 273,8 s | 265,2 s |
| **buscar** | 4,04 s | **1,21 s** |
| **varrer** | 5,04 s | **2,22 s** |
| **atualizar** | 3,38 s | **1,26 s** |
| excluir | 8,94 s | 7,34 s |

**No insert, 3%** — e a explicação é o cache do sistema: durante a carga o
arquivo está quente, e ler 400 MiB dele é um `memcpy`. Nas fases de leitura ele
está frio, e a mesma leitura vai ao disco: é lá que os segundos estavam.

Com isso o PhxSql passou a **ganhar em três das quatro** operações restantes,
`buscar` inclusive, que antes empatava.

### O que continua sem explicação

A taxa de inserção ainda **cai com o tamanho** — 54.180 para 37.712 linhas/s —
e num processo só ela não cai. Sobrou, por lote de 50.000, cerca de **6,6 µs por
linha** que o `abrir` não explica mais. O suspeito seguinte é o **cache de
páginas do `.ndx` nascer vazio a cada processo**: `onde-doi` roda um processo e
mantém o cache quente do começo ao fim. Isso ainda não foi medido, e por isso
está escrito aqui como suspeita e não como causa.

---

'''
io.open(p,'w',encoding='utf-8').write(s.replace(anc, novo+anc))
print('ok')
