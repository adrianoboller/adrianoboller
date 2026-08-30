# Update FORMATO for the log v2
# 28/08 20:34

import pathlib
p = pathlib.Path("docs/FORMATO.md")
s = p.read_text()
antigo = """| 0 | 8 | assinatura `PHXLOG\\0\\0` |
| 8 | 2 | versão do formato (1) |"""
novo = """| 0 | 8 | assinatura `PHXLOG\\0\\0` |
| 8 | 2 | versão do formato (2) |"""
assert antigo in s
s = s.replace(antigo, novo)

antigo = """### Evento (36 bytes)

| Off | Tam | Campo |
|----:|----:|---|
| 0 | 8 | carimbo — milissegundos desde 1970-01-01T00:00:00Z |
| 8 | 1 | operação: 1 = inclusão, 2 = alteração, 3 = exclusão |
| 9 | 1 | flags |
| 10 | 2 | reservado |
| 12 | 8 | rowid afetado |
| 20 | 8 | versão do registro depois da operação |
| 28 | 4 | usuário (0 = não informado) |
| 32 | 4 | CRC-32 dos bytes 0..32 |

O carimbo é em **milissegundos**, não segundos, para que operações no mesmo
segundo continuem ordenáveis. Uma operação recusada — chave duplicada, tabela
cheia, coluna obrigatória em branco — **não** gera evento: o diário registra o
que aconteceu, não o que foi tentado."""
novo = """### Evento: 44 bytes de cabeçalho, e talvez um corpo

| Off | Tam | Campo |
|----:|----:|---|
| 0 | 8 | carimbo — milissegundos desde 1970-01-01T00:00:00Z |
| 8 | 1 | operação: 1 = inclusão, 2 = alteração, 3 = exclusão |
| 9 | 1 | flags — bit 0: tem imagem |
| 10 | 2 | reservado |
| 12 | 8 | rowid afetado |
| 20 | 8 | versão do registro depois da operação |
| 28 | 4 | usuário (0 = não informado) |
| 32 | 4 | tamanho da imagem (0 = sem imagem) |
| 36 | 4 | CRC-32 dos bytes 0..36 **e da imagem** |
| 40 | 4 | reservado |
| 44 | N | imagem da linha |

O carimbo é em **milissegundos**, não segundos, para que operações no mesmo
segundo continuem ordenáveis. Uma operação recusada — chave duplicada, tabela
cheia, coluna obrigatória em branco — **não** gera evento: o diário registra o
que aconteceu, não o que foi tentado.

### A imagem da linha

Sem ela o evento diz que o rowid 42 mudou; não diz **para quê**. Isso basta para
auditoria e não basta para replicar. Fica atrás de um interruptor no
`config.json` (`replicacao.imagem_da_linha`), e vem ligada num servidor com
`papel: source`.

```
imagem = [tam_payload u32][payload]
         [qtd_externos u16]
         [ (coluna u16, tamanho u32, conteúdo) ... ]
```

O payload vai **cru**, do jeito que está no `.reg` — sem reencodar, sem passar
por texto, sem perder precisão de decimal nem de data. E o **conteúdo** dos
externos vai junto, não os ponteiros: os offsets do `.bin` e do `.memo` são
desta máquina e apontariam para qualquer coisa na outra. É a mesma razão pela
qual o `.trash` guarda conteúdo.

Exclusão não leva imagem: o rowid basta.

O CRC cobrir a imagem, e não só o cabeçalho, é o detalhe que importa: a imagem é
o que a réplica grava **como dado**. Um byte trocado ali entraria na réplica sem
ninguém notar.

Medido, mesma tabela e mesmas 100.000 linhas:

| `imagem_da_linha` | linhas/s | bytes por evento |
|---|---:|---:|
| desligada | 21.740 | 44 |
| ligada | 19.531 | 223 |

### O que a largura variável custa

Até a versão 1 o evento N morava no offset `64 + N × 36`, e pular era uma conta.
Agora não é: chegar ao evento N é caminhar pelos anteriores lendo o tamanho de
cada um. O que salva a leitura é o `eventos neste volume` do cabeçalho — um
volume inteiro se pula sem abrir o arquivo."""
assert antigo in s
s = s.replace(antigo, novo)
p.write_text(s)
print("ok")
