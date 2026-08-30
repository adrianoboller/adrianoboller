# Rewrite section 2.2
# 29/08 01:57

import pathlib
p = pathlib.Path("docs/DESEMPENHO.md")
s = p.read_text()
i = s.index("### 2.2 O que o `.log` custa")
j = s.index("### 2.1 De quanto tem de ser o teto")
novo = '''### 2.2 O `.log` não atrasa o `.reg`

A tabela acima media `.reg` e `.log` **juntos**, e este documento registrava
isso como um bloco não decomposto. Ele foi decomposto, e a decomposição virou
uma mudança.

```bash
cargo run --release --example custo-do-log -- 200000
```

O `.log` já era enxuto — não reserializa nada, e o cabeçalho fica em cache na
leitura. O que ele fazia de sobra eram **duas escritas por evento**: os 44 bytes
do evento, e os 64 bytes do cabeçalho com `fim` e `qtd_eventos`.

O evento **tem** de ir na hora. O cabeçalho é um contador — e a leitura sabe
recalculá-lo varrendo os próprios eventos. Ele passou a ir no `sincronizar`:

| | antes | depois | |
|---|---:|---:|---:|
| `.log` por evento, sem imagem | 1,22 µs | **0,67 µs** | 1,82× |
| `.log` por evento, com a imagem da linha | 2,24 µs | **1,61 µs** | 1,39× |
| inserção completa, 2 índices | 17,0 µs | **15,9 µs** | 1,06× |

### O que isso custou, e o que não custou

**Não custou o evento.** Ele continua indo para o arquivo dentro da inserção,
antes de a operação terminar. O que ficou para depois foi o *contador*.

**Custou um caminho de reparo**, e ele é a parte que valia escrever com cuidado.
Uma queda antes do `sincronizar` deixa o cabeçalho atrasado em relação aos
eventos que já estão no arquivo — e, sem cura, a próxima gravação escreveria
**por cima** deles. Não seria evento invisível: seria evento destruído.

Então `abrir` varre para a frente a partir do `fim` gravado, validando cada
evento pelo **CRC que ele já carrega**, e para no primeiro que não confere ou no
fim do arquivo. A varredura é limitada ao que entrou desde o último
`sincronizar` — uma janela de centenas de eventos. Região zerada não passa: o
CRC-32 de 36 bytes zerados não é zero.

Quatro testes travam isso, e o que mais importa é
`depois_da_cura_o_novo_evento_nao_sobrescreve`.

### O que continua fora, e por quê

Guardar os **eventos** em RAM compraria os 0,67 µs restantes — 4,2%. Não foi
feito, e a razão não é de tamanho, é de natureza:

> Índice perdido se reconstrói do `.reg` com `reindexar`. **Evento perdido não
> se reconstrói.** Ele é a história, com carimbo de hora e autor, e é a posição
> de que a replicação depende — uma réplica pularia a linha em silêncio.

'''
s = s[:i] + novo + s[j:]
p.write_text(s)
print("ok")
