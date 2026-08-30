# Update DESEMPENHO with the new step
# 29/08 01:11

import pathlib
p = pathlib.Path("docs/DESEMPENHO.md")
s = p.read_text()

alvo = '''| Esquema | antes do cache | depois do cache | |
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
| **total** | **44,4** | 100% | **18,5** | 100% |'''
novo = '''| Esquema | antes | + cache de páginas | + cabeçalho enxuto | ganho |
|---|---:|---:|---:|---:|
| só `.reg` (sem índice nenhum) | 7,3 µs | 6,7 µs | **5,4 µs** | 1,35× |
| + 1 índice comum | 21,5 µs | 12,2 µs | **10,9 µs** | 1,97× |
| + o mesmo índice, agora único | 30,6 µs | 12,6 µs | **11,2 µs** | 2,73× |
| + 2 índices (a forma da bancada) | 44,4 µs | 18,5 µs | **17,0 µs** | **2,61×** |

| Parcela | antes | % | agora | % |
|---|---:|---:|---:|---:|
| `.reg` + `.log` | 7,3 | 16,5% | 5,4 | 31,8% |
| primeiro índice | 14,2 | 32,0% | 5,4 | 31,8% |
| conferir a chave única | 9,1 | 20,5% | 0,7 | **4,0%** |
| segundo índice | 13,8 | 31,0% | 5,5 | 32,4% |
| **total** | **44,4** | 100% | **17,0** | 100% |

(As três colunas de cima são três medições de três *builds*, cada uma com três
corridas — a primeira corrida depois de compilar sai contaminada pelo próprio
compilador e não conta.)

### 2.0 O cabeçalho que reserializava o esquema por linha

Achado respondendo «e se o `.ndx` parasse durante a carga?»: toda inserção
chamava `gravar_cabecalho`, e ele fazia **cinco coisas, das quais uma era
necessária**:

1. serializar o **esquema inteiro** — que não muda desde que a tabela foi criada;
2. calcular o **CRC-32 desse bloco**;
3. gravar os 128 bytes do cabeçalho, com os contadores — *esta* é a necessária;
4. gravar o **bloco de esquema outra vez**, byte a byte igual ao que já estava lá;
5. perguntar o **tamanho do arquivo** para ver se precisava esticar.

O esquema é imutável depois da criação: passou a ser serializado uma vez, no
construtor, com o CRC junto. E o caminho quente ganhou um irmão que grava **só o
cabeçalho** — o bloco de esquema e o teste de tamanho ficaram onde importam, na
criação do volume.

| | antes | depois | |
|---|---:|---:|---:|
| só `.reg` | 6,7–6,9 µs | **5,2–5,4 µs** | **1,27×** |
| com 2 índices | 18,3–19,0 µs | **17,0–17,1 µs** | 1,08× |'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)

s = s.replace('''> páginas de leitura tirou isso do caminho: **44,4 → 18,5 µs por linha, 2,4×**,
> sem mudar formato, sem mudar garantia e sem tocar na árvore.''',
'''> páginas de leitura tirou isso do caminho: **44,4 → 18,5 µs por linha, 2,4×**,
> sem mudar formato, sem mudar garantia e sem tocar na árvore. Depois dele, o
> cabeçalho que reserializava o esquema a cada linha (§2.0) levou a **17,0 µs**
> — **2,61× no total**.''', 1)
p.write_text(s)
print("ok")
