# Registrar o achado e empurrar
# 29/08 06:22

import io
p='docs/DESEMPENHO.md'
s=io.open(p,encoding='utf-8').read()
anc = '## 5. Por que LSM não cabe dentro do motor atual'
assert s.count(anc)==1
novo = '''## 4.8 O write-back entrou — e o gargalo mudou de lugar

`gravar_pagina` passou a deixar a página **suja em RAM**; o CRC-32 e o `write`
acontecem no despejo, no fechamento ou no `sincronizar`. É o que o InnoDB faz
(`mtr0mtr.cc:338` marca, `buf0flu.cc:1243` sela) e o Aria também
(`PCBLOCK_CHANGED`, `PAGECACHE_WRITE_DELAY`).

`--example onde-doi`, dois índices, o empilhamento da rodada:

| | µs/linha |
|---|---:|
| 0.17.0 | 16,4 |
| + cabeçalho do `.ndx` fora do caminho da chave (§4.6b) | 14,5 |
| + CRC slice-by-16 | 13,1 |
| + **cache write-back** | **7,5** |

**2,19×**, e a forma mudou: `.reg`+`.log` virou **60,8%** e os dois índices
29,4%. O `.ndx` deixou de ser o dono do tempo. E o ganho **se mantém a 3
milhões de linhas** — 7,5 µs lá também.

### Só que a bancada mal se mexeu: 265,2 → 261,8 s

O motivo não é escala, e não é disco: a corrida de 10 milhões é **95% CPU** e
escreve **2,4 GiB contra 32,0 GiB do MySQL(R)** — treze vezes menos.

É o **esquema**. Mesmo tamanho, mesmo código, um processo só:

| esquema | µs/linha |
|---|---:|
| 3 colunas — `Int8`, `Str(40)`, `Str(20)` | **7,50** |
| 5 colunas da bancada — as três acima mais `Decimal(15,2)` e `Date` | **16,61** |

**2,2× de diferença por causa de duas colunas.** O `onde-doi` mede um esquema
mais simples do que o da bancada, e por isso viu um ganho que a bancada não vê.

Isso não invalida o write-back — ele é real e está medido —, mas **realoca a
fila**: o custo dominante agora é a **codificação da linha** (`montar_payload`,
`codificar_chave`), e não a árvore. As duas colunas suspeitas são o `Decimal`,
que é `i128`, e o `Date`.

**Não está medido** qual das duas custa, nem quanto disso é encode contra
tamanho de slot. É a próxima medição, e ela vem antes de qualquer conserto — a
regra que este documento já aplicou seis vezes.

> E fica a lição sobre o próprio medidor: o `onde-doi` e a bancada usam esquemas
> diferentes, e essa diferença esteve escondida em todos os números desta
> sessão. **Medidor que não mede a mesma coisa que a bancada mede outra coisa.**

---

'''
io.open(p,'w',encoding='utf-8').write(s.replace(anc, novo+anc))
print('ok')
