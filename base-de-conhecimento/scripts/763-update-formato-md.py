# Update FORMATO.md
# 28/08 19:59

import pathlib
p = pathlib.Path("docs/FORMATO.md")
s = p.read_text()

s = s.replace("| 8 | 2 | versão do formato (3) |", "| 8 | 2 | versão do formato (4) |")
s = s.replace(
"""| 100 | 8 | `slots_no_balde` — slots já usados **neste** volume (só na partição alfanumérica) |
| 108 | 16 | reservado |""",
"""| 100 | 8 | `slots_no_balde` — slots já usados **neste** volume (só na partição alfanumérica) |
| 108 | 8 | `marcadas` — linhas vivas marcadas como excluídas (só o volume 1) |
| 116 | 8 | reservado |""")

antigo = """**Como ela pagina sem índice.** O `rownum` cresce com o `rowid`, porque o
`.reg` guarda as linhas na ordem de chegada. Uma sequência crescente num
arquivo de acesso aleatório se procura por **bissecção**: achar a linha de
número 500.000 num milhão custa vinte leituras, sem índice nenhum a manter. É
o mesmo motivo de o endereço sair de uma conta — a ordem lógica é a ordem
física."""
novo = """**Como ela pagina sem índice.** O `rownum` cresce com o `rowid`, porque o
`.reg` guarda as linhas na ordem de chegada. Uma sequência crescente num
arquivo de acesso aleatório se procura por **bissecção**: achar a linha de
número 500.000 num milhão custa vinte leituras, sem índice nenhum a manter. É
o mesmo motivo de o endereço sair de uma conta — a ordem lógica é a ordem
física.

**A exceção que a partição alfanumérica cria.** Ali o `rownum` **não** cresce
com o rowid: a Silva digitada primeiro mora no `_S`, com rowid alto, e a Alves
digitada depois mora no `_A`, com rowid 1 — número de ordem 1 num rowid maior
que o do número 2. Bissetar uma sequência que não está ordenada devolveria a
linha errada *em silêncio*, que é pior que devolver devagar; nesse modo o motor
varre. É a razão de `Table::posicao_e_rownum` recusar a partição por letra.

### `marcadas`, e a pergunta que ela responde em tempo constante

O cabeçalho do volume 1 guarda quantas linhas vivas estão **marcadas** como
excluídas. É um contador em cache, como o `live_count` ao lado dele, e existe
para duas contas que sem ele custariam a tabela inteira:

1. **Quantas linhas esta visão enxerga.** `registros − marcadas` são as ativas,
   `marcadas` são as excluídas, `registros` são todas. Era por não existir esse
   número que a resposta do `varrer` tinha deixado de trazer o total — mostrar
   «página 3 de 40» custava percorrer tudo.
2. **A posição de uma linha na lista é o `rownum` dela?** Se ninguém apagou de
   vez (`proximo_rownum − 1 == live_count`) e ninguém marcou (`marcadas == 0`),
   sim — e aí pular para a posição 500.000 é uma bissecção de vinte leituras em
   vez de meio milhão de passos.

Contador em cache diverge se algum caminho esquecer de mexer nele, e aqui a
divergência mandaria a tela para a linha errada sem avisar. Duas defesas: o
contador vai ao disco **na mesma operação** que o muda (128 bytes a mais, e não
no `sincronizar`, senão uma queda o faria voltar atrás), e `verificar` o
**reconta varrendo** em vez de acreditar nele — é o mesmo caminho que o reparo
chama."""
assert antigo in s
s = s.replace(antigo, novo)

antigo = """O contador vive nos bytes 92..100 do cabeçalho do volume 1 e vai ao disco no
`sincronizar`, como os outros."""
novo = """O contador vive nos bytes 92..100 do cabeçalho do volume 1 e vai ao disco no
`sincronizar`, como os outros."""
s = s.replace(antigo, novo)
p.write_text(s)
print("ok")
