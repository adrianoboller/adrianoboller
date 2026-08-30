# Document the decomposition
# 29/08 01:47

import pathlib
p = pathlib.Path("docs/DESEMPENHO.md")
s = p.read_text()
alvo = '''### 2.1 De quanto tem de ser o teto'''
novo = '''### 2.2 O que o `.log` custa, e o que um buffer compraria

A tabela acima media `.reg` e `.log` **juntos** — 5,4 µs —, e este documento
registrava isso como um bloco não decomposto. Ele foi decomposto:

```bash
cargo run --release --example custo-do-log -- 200000
```

| | µs por evento | de uma inserção de 17,0 µs |
|---|---:|---:|
| o `.log` inteiro, sem imagem | **1,22** | 7,2% |
| o `.log` inteiro, com a imagem da linha (replicação ligada) | 2,24 | 13,2% |
| **só** a reescrita do cabeçalho, por evento | 0,41 | 2,4% |

Ou seja: dos 5,4 µs, o diário são 1,22 e o **`.reg` sozinho é ~4,2 µs**.

O `.log` já é enxuto — não reserializa nada, e o cabeçalho fica em cache na
leitura. O que ele faz de sobra é **duas escritas por evento**: os 44 bytes do
evento, e os 64 bytes do cabeçalho com `fim` e `qtd_eventos`.

### A pergunta que isso responde

«Dá para guardar o diário em memória e gravar quando a inserção terminar?»
Separando as duas coisas que a pergunta junta:

- **Segurar os EVENTOS em RAM** compraria, no teto, os 1,22 µs do diário
  inteiro — **7,2%**. E trocaria a garantia de que linha gravada tem evento
  gravado.
- **Parar de reescrever o CABEÇALHO a cada evento** compraria 0,41 µs —
  **2,4%** — e **não precisa de buffer nenhum**: o evento continua indo para o
  arquivo na hora.

A diferença entre as duas é de natureza, não de tamanho. Um índice perdido se
reconstrói do `.reg` com `reindexar`. **Um evento perdido não se reconstrói** —
ele é a história, com carimbo de hora e autor, e é a posição de que a replicação
depende. Uma réplica pularia a linha em silêncio.

Por isso a primeira está registrada e parada, e a segunda é que vale considerar:
ela pede um caminho de reparo (varrer do `fim` gravado para a frente, achando
eventos válidos pelo CRC) que hoje não existe, mas não pede que nada fique
retido em RAM.

### 2.1 De quanto tem de ser o teto'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)

s = s.replace('''| `.reg` + `.log` | 7,3 | 16,5% | 5,4 | 31,8% |''',
              '''| `.reg` + `.log` | 7,3 | 16,5% | 5,4 | 31,8% |
| <span>↳ só o `.log` (§2.2)</span> | — | — | 1,22 | 7,2% |''')
p.write_text(s)
print("ok")
