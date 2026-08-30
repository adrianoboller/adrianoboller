# Rewrite DESEMPENHO sections 1 and 2
# 29/08 00:21

import pathlib
p = pathlib.Path("docs/DESEMPENHO.md")
s = p.read_text()

# --------------------------------------------------------------- secao 1
alvo = '''## 1. A resposta curta

> **83,5% do tempo de uma inserção está no `.ndx`.** O arquivo de dados — a
> parte que as propostas de WAL, MemTable e LSM querem substituir — já é
> *append-only* sequencial e custa 16,5%.

Isso muda o alvo. A receita clássica para acelerar escrita («tire o `fsync` do
caminho crítico») foi escrita para motores cujo gargalo é o `fsync`. O do PhxSql
não é, e há medida para isso: na bancada de 10 milhões de linhas, o processo
gastou **870 s de CPU para 884 s de relógio (98%) e leu 0,0 MiB**. Ele passou o
tempo inteiro *calculando*, não esperando disco.'''
novo = '''## 1. A resposta curta

> **O tempo estava no `.ndx`, e estava no CRC-32 de página inteira.** Cada
> inserção descia a B+tree relendo do arquivo as mesmas páginas — a raiz, a
> mesma para todas —, e cada leitura passava 4 KiB pelo CRC. Um cache de
> páginas de leitura tirou isso do caminho: **44,4 → 18,5 µs por linha, 2,4×**,
> sem mudar formato, sem mudar garantia e sem tocar na árvore.

A receita clássica para acelerar escrita («tire o `fsync` do caminho crítico»)
foi escrita para motores cujo gargalo é o `fsync`. O do PhxSql não era, e havia
medida para isso: na bancada de 10 milhões de linhas, o processo gastou **870 s
de CPU para 884 s de relógio (98%) e leu 0,0 MiB**. Ele passava o tempo inteiro
*calculando* — e agora se sabe calculando o quê.

Depois do cache a divisão mudou de lugar: o `.ndx` caiu de **83,5% para 63,6%**
do tempo de uma inserção, e o `.reg` + `.log` subiu de 16,5% para 36,4% — não
porque ficaram mais lentos, mas porque o outro lado encolheu.'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)

# --------------------------------------------------------------- secao 2
alvo = '''| Esquema | linhas/s | µs por linha |
|---|---:|---:|
| só `.reg` (sem índice nenhum) | **136.338** | 7,3 |
| + 1 índice comum | 46.433 | 21,5 |
| + o mesmo índice, agora único | 32.639 | 30,6 |
| + 2 índices (a forma da bancada) | **22.516** | 44,4 |

| Parcela | µs | % |
|---|---:|---:|
| `.reg` + `.log` | 7,3 | **16,5%** |
| primeiro índice | 14,2 | 32,0% |
| conferir a chave única | 9,1 | 20,5% |
| segundo índice | 13,8 | 31,0% |
| **total** | **44,4** | 100% |

**Seis vezes.** Uma tabela sem índice insere a 136 mil linhas/s; a mesma tabela
com dois índices, a 22,5 mil. O heap não é o problema.

E piora com o tamanho, o que confirma o diagnóstico: na carga de 10 milhões, o
primeiro milhão entrou a 16.051/s e o décimo a 9.311/s — **42% mais devagar no
fim**. Taxa que cai conforme a tabela cresce, com o disco parado, é assinatura
de estrutura de índice: a B+tree reescrita nó a nó, uma linha por vez.

### Uma conta que não fecha, e vale investigar

O mesmo medidor estima, pelo `strace`, ~41 chamadas de sistema e ~20 toques de
página por linha, e mede o CRC-32 de uma página de 4 KiB em 2,36 µs. Vinte
toques dariam ~47 µs só de CRC — **mais que os 44,4 µs medidos no total**.

A conta não fecha, e isso é informação: ou nem todo toque recalcula o CRC, ou
as páginas quentes não são revisitadas tantas vezes quanto o `strace` sugere.
Está registrado aqui como **pista aberta**, não como conclusão. É o próximo
lugar a instrumentar.'''
novo = '''| Esquema | antes do cache | depois do cache | |
|---|---:|---:|---:|
| só `.reg` (sem índice nenhum) | 7,3 µs | **6,7 µs** | 1,09× |
| + 1 índice comum | 21,5 µs | **12,2 µs** | 1,76× |
| + o mesmo índice, agora único | 30,6 µs | **12,6 µs** | 2,43× |
| + 2 índices (a forma da bancada) | 44,4 µs | **18,5 µs** | **2,40×** |

| Parcela | antes | % | depois | % |
|---|---:|---:|---:|---:|
| `.reg` + `.log` | 7,3 | 16,5% | 6,7 | **36,4%** |
| primeiro índice | 14,2 | 32,0% | 5,4 | 29,2% |
| conferir a chave única | 9,1 | 20,5% | 0,4 | **2,3%** |
| segundo índice | 13,8 | 31,0% | 5,9 | 32,0% |
| **total** | **44,4** | 100% | **18,5** | 100% |

A linha que mais mudou diz o que aconteceu: **conferir a chave única caiu de
20,5% para 2,3%**. Essa conferência é uma descida na árvore que não escreve
nada — exatamente o trabalho que o cache serve de graça. Ela não ficou mais
esperta; ela parou de reler do arquivo o que já estava em RAM.

A tabela sem índice nenhum quase não mudou (1,09×), e é o controle da
experiência: o `.reg` não usa página de `.ndx`.

### A conta que não fechava, e agora fecha

A versão anterior deste documento registrava uma **pista aberta**: o medidor
estimava, por um `strace`, ~20 toques de página por linha, e o CRC-32 de uma
página de 4 KiB custa 2,34 µs. Vinte toques dariam ~47 µs só de CRC — mais que
os 44,4 µs medidos no total. A conta não fechava.

Ela não fechava porque o número de toques era **citado**, e não medido. O
medidor agora **conta** os toques dentro do `.ndx`, e o próprio cache é quem
conta:

```
paginas servidas pelo cache ....... 8,80 por linha
paginas lidas do arquivo .......... 0,00 por linha
paginas gravadas .................. 2,06 por linha
```

São **10,86** toques por linha, e não 20. Antes do cache, os 10,86 passavam
todos pelo CRC: 10,86 × 2,34 = **25,4 µs**, de 44,4 medidos — 57% do tempo de
uma inserção era CRC-32 de página. Depois, só as 2,06 gravações pagam: 4,8 µs
de 18,5 (26%).

O acerto de cache custa a **cópia** da página, não o CRC dela. É daí que veio o
2,4×.

E a piora com o tamanho, que confirmava o diagnóstico, continua confirmando: na
carga de 10 milhões, o primeiro milhão entrava a 16.051/s e o décimo a 9.311/s.
É a árvore crescendo para além do que cabe em RAM — o cache adia esse ponto, não
o elimina.

### O cache, em uma tela

- **É de leitura.** Toda gravação atravessa para o arquivo na hora. Segurar
  página suja daria mais e trocaria uma garantia por desempenho **sem avisar**:
  hoje uma queda do *processo* não atrasa o `.ndx` em relação ao `.reg`, porque
  o `write` já entregou a página ao núcleo. Só uma queda da *máquina* faz isso.
- **A página recém-gravada fica.** É o que mais rende numa carga: a folha que
  acabou de receber uma chave é quase sempre a que vai receber a próxima.
- **Despejo por segunda chance (CLOCK).** Fila simples não serviria — a raiz, a
  página mais visitada de todas, sairia junto com as outras assim que o teto
  enchesse.
- **Teto de 2.048 páginas** = 8 MiB por `.ndx` aberto. O número saiu de uma
  varredura, e não do chute (§2.1).

### 2.1 De quanto tem de ser o teto

```bash
cargo run --release --example ordem-da-chave -- 200000
```

Mesmas 200.000 linhas, dois índices, mudando só o teto do cache. A coluna da
direita é a mesma carga com as chaves **embaralhadas**, que é o caso comum de
quem importa de outro sistema:

| Teto | RAM | chaves crescentes | chaves embaralhadas |
|---|---:|---:|---:|
| sem cache | — | 43,5 µs | 46,0 µs |
| 512 páginas | 2 MiB | 18,0 µs | 25,3 µs |
| 1.024 páginas | 4 MiB | 18,2 µs | 23,2 µs |
| **2.048 páginas** | **8 MiB** | **17,9 µs** | **21,3 µs** |
| 4.096 páginas | 16 MiB | 17,8 µs | 20,5 µs |

2.048 é o joelho: dobrar de novo compra 0,8 µs e custa mais 8 MiB por tabela
aberta. O servidor abre e fecha a tabela a cada operação, então esse teto vale
enquanto a operação dura — e a operação que importa aqui, a carga em lote,
insere milhares de linhas dentro de uma única abertura.'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)
p.write_text(s)
print("ok")
