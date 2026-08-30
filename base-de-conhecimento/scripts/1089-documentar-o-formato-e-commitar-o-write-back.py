# Documentar o formato e commitar o write-back
# 29/08 06:08

import io
p='docs/FORMATO.md'
s=io.open(p,encoding='utf-8').read()
velho='''| 44 | 8 | alterado em |
| 52 | 72 | reservado |
| 124 | 4 | CRC-32 dos bytes 0..124 |'''
novo='''| 44 | 8 | alterado em |
| 52 | 1 | **marca de sujo** (0 = fechado limpo) |
| 53 | 71 | reservado |
| 124 | 4 | CRC-32 dos bytes 0..124 |

#### A marca de sujo (byte 52), e por que ela existe

Desde a 0.18.0 o cache de páginas do `.ndx` é **write-back**: a página modificada
fica em RAM e o CRC-32 e a gravação acontecem no despejo, no fechamento ou no
`sincronizar` — não a cada chave. É o que o InnoDB faz com o buffer pool e o
Aria com `PCBLOCK_CHANGED`, e vale **16,4 → 7,5 µs por linha** com dois índices.

O preço é uma garantia que este documento afirmava até a 0.17.0: antes, uma
queda do **processo** não podia atrasar o `.ndx` em relação ao `.reg`, porque o
`write` já tinha entregue a página ao núcleo. Agora pode.

A marca é o que torna isso aceitável, e a **ordem** é a garantia:

1. antes da primeira página suja existir, o byte 52 vai a 1 **no arquivo**;
2. ela só volta a 0 depois de **todas** as páginas sujas terem ido ao disco.

Quem abre um `.ndx` com o byte 52 em 1 sabe que a árvore pode ter chave
faltando, e **toda operação recusa** com a mensagem que manda reconstruir. Um
índice atrasado se refaz do `.reg` — `reindexar` custa 0,31 s por milhão de
chaves desde a construção em lote. Um índice atrasado **em silêncio** não tem
conserto, porque ninguém sabe que ele está errado.

E fechar não limpa a marca de um arquivo que já foi **aberto** sujo: nada foi
reconstruído. Só o `reindexar`, que recria o arquivo, a tira — senão bastaria
abrir e fechar para o defeito virar invisível.

**Não há migração.** Arquivo escrito antes da 0.18.0 tem zero no byte 52, e zero
quer dizer «limpo» — que é a verdade para quem só escrevia através.'''
assert s.count(velho)==1
io.open(p,'w',encoding='utf-8').write(s.replace(velho,novo))
print('ok')
