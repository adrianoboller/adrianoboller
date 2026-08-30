# Document the schema block v3
# 28/08 11:48

import pathlib
p = pathlib.Path('docs/FORMATO.md')
s = p.read_text()
v = '''Logo após o cabeçalho vem o **esquema serializado** (`schema_len` bytes), e
`data_offset` é o próximo múltiplo de 64. A tabela é auto-descritiva: o
conjunto de arquivos basta para reabrir os dados, sem dicionário externo.'''
n = '''Logo após o cabeçalho vem o **esquema serializado** (`schema_len` bytes), e
`data_offset` é o próximo múltiplo de 64. A tabela é auto-descritiva: o
conjunto de arquivos basta para reabrir os dados, sem dicionário externo.

### O bloco de esquema (`PSCH`, versão 3)

O bloco começa com `PSCH` e a versão. A versão **3** acrescentou os metadados
de coluna, o marcador de chave primária e o modo de partição. A leitura ainda
aceita a 2: tabela gravada antes abre normalmente, ganha um `id` v7 sorteado na
hora e os textos vazios. **Escrever, só na 3.**

Por coluna, nesta ordem:

| Campo | Tam | O que é |
|---|---:|---|
| `nome` | 2 + n | o nome no disco |
| tipo | 4 | tag + dois parâmetros (largura do `Str`, precisão/escala do `Decimal`) |
| `nullable` | 1 | aceita nulo |
| `id` | 16 | **UUID v7 da coluna**, sorteado na criação e nunca reaproveitado |
| `caption` | 2 + n | rótulo de tela; vazio significa "use o nome" |
| `descricao` | 2 + n | para que a coluna serve |
| `mascara` | 2 + n | PICTURE do Clarion(R): `@N-11.2`, `@D6`, `@P###-####P` |

O `id` existe para que **renomear a coluna não quebre nada**: uma tela, um
relatório ou um mapeamento apontam para ele, e renomear troca só o `nome`. É a
mesma razão de o esquema morar no `.reg` — um dicionário externo se perde, se
desatualiza, e obriga quem copia os cinco arquivos a copiar um sexto.

Por índice, os sinalizadores viraram um byte com dois bits: **único** no bit 0
e **primário** no bit 1.

### Chave primária, chave estrangeira, chave composta

Só um índice pode ser primário, ele é sempre único, e nenhuma coluna dele pode
aceitar nulo — uma identidade nula não identifica. As três conferências
acontecem no `Schema::new`.

O papel de uma coluna nas chaves **não é gravado na coluna**: sai dos índices e
das chaves estrangeiras, que são a verdade.

| Marca | De onde sai |
|---|---|
| primária | a coluna aparece no índice marcado como primário |
| estrangeira | a coluna aparece em alguma chave estrangeira |
| composta | a chave de que ela participa tem mais de uma coluna |

Guardar "é primária" no próprio campo criaria uma segunda verdade ao lado do
índice, e as duas divergiriam no primeiro `ALTER`.'''
assert s.count(v) == 1
p.write_text(s.replace(v, n))
print('ok')
