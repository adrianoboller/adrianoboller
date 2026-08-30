# Record the measuring lesson
# 29/08 00:23

import pathlib
p = pathlib.Path("/home/user/adrianoboller/CLAUDE.md")
s = p.read_text()
alvo = '''**Número digitado à mão envelhece calado.**'''
novo = '''**A lista do que falta também é palpite até alguém medir.** O pedido 113 dizia
«ordene as chaves do lote antes do `.ndx`» e vinha com o alvo certo — os 83,5%
estavam mesmo lá. Só que o custo não era de **localidade**: era de reler do
arquivo e recalcular o CRC-32 da **mesma página** a cada descida da árvore. A
desordem custava 1,06×; ordenar teria comprado quase nada, e teria custado uma
garantia. Um cache de páginas de leitura comprou **2,40×**. *Medir a premissa do
item vem antes de implementar o item* — inclusive quando o item é nosso.

E o corolário: o mesmo medidor dizia «~20 toques de página por linha», citando
um `strace` de outro dia. Eram 10,86, e é por isso que a conta do CRC nunca
fechava naquele documento. **Número citado é número que não se mede** — hoje o
medidor conta os toques por dentro.

**Número digitado à mão envelhece calado.**'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)
p.write_text(s)
print("ok")
