# Renumber the figures by position
# 28/08 19:13

import io, re
p='docs/dossie/dossie-phxsql.html'
s=io.open(p,encoding='utf-8').read()

# A ordem no documento e 1..5, 9, 6, 7, 8, 10.. -- a minha figura entrou na
# posicao 6 e ficou com o numero 9. Renumera as quatro, por POSICAO.
alvos = ["<b>Figura 9.</b>", "<b>Figura 6.</b>", "<b>Figura 7.</b>", "<b>Figura 8.</b>"]
novos = ["<b>Figura 6.</b>", "<b>Figura 7.</b>", "<b>Figura 8.</b>", "<b>Figura 9.</b>"]
# Substitui da direita para a esquerda no documento, para nao colidir.
posicoes = []
for a in alvos:
    i = s.index(a)
    posicoes.append((i, a))
posicoes.sort()
assert [a for _, a in posicoes] == alvos, f"ordem inesperada: {[a for _,a in posicoes]}"
for (i, a), n in sorted(zip(posicoes, novos), reverse=True):
    s = s[:i] + n + s[i+len(a):]
io.open(p,'w',encoding='utf-8').write(s)
