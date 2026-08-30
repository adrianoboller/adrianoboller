# Update the CPU claim; regenerate dossier bench numbers
# 29/08 00:43

import pathlib
p = pathlib.Path("docs/DESEMPENHO.md")
s = p.read_text()
s = s.replace('''foi escrita para motores cujo gargalo é o `fsync`. O do PhxSql não era, e havia
medida para isso: na bancada de 10 milhões de linhas, o processo gastou **870 s
de CPU para 884 s de relógio (98%) e leu 0,0 MiB**. Ele passava o tempo inteiro
*calculando* — e agora se sabe calculando o quê.''',
'''foi escrita para motores cujo gargalo é o `fsync`. O do PhxSql não era, e havia
medida para isso: na bancada de 10 milhões de linhas, o processo gastou **870 s
de CPU para 884 s de relógio (98%) e leu 0,0 MiB**. Ele passava o tempo inteiro
*calculando* — e agora se sabe calculando o quê.

Na mesma bancada depois do cache: **289 s de CPU para 303 s de relógio (95%),
0,0 MiB lidos**. Continua sendo CPU, e continua não sendo disco — só que agora
é três vezes menos CPU.''', 1)
p.write_text(s)
print("ok")
