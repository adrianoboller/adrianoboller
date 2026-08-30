# Update the load numbers everywhere
# 29/08 01:13

import pathlib, json
p = pathlib.Path("bancada/carga/resultados.json")
d = json.loads(p.read_text())
d.update({"versao":"0.17.0","uma_a_uma_s":7.521,"uma_a_uma_por_s":2659,
          "em_lote_s":0.509,"em_lote_por_s":39287,"ganho":14.77,
          "nota":"duas corridas: o lote deu 39.038 e 39.287 linhas/s; publicada a segunda"})
p.write_text(json.dumps(d, indent=2, ensure_ascii=False) + "\n")

p = pathlib.Path("bancada/carga/LEIA-ME.md")
s = p.read_text()
s = s.replace('''| uma a uma | 2.609 | uma viagem, uma abertura e um `fsync` por linha |
| lotes de 5.000 | **37.021** | **14,2×** |

O ganho **não é do disco**: é de tudo que acontecia *por linha* passar a
acontecer uma vez por lote. Na 0.16.0 o lote dava 25.985/s; o que mudou entre
uma e outra foi o cache de páginas do `.ndx` (`docs/DESEMPENHO.md` §2).''',
'''| uma a uma | 2.659 | uma viagem, uma abertura e um `fsync` por linha |
| lotes de 5.000 | **39.287** | **14,8×** |

O ganho **não é do disco**: é de tudo que acontecia *por linha* passar a
acontecer uma vez por lote. Na 0.16.0 o lote dava 25.985/s; o que mudou entre
uma e outra foram o cache de páginas do `.ndx` e o cabeçalho que parou de
reserializar o esquema por linha (`docs/DESEMPENHO.md` §2 e §2.0).

O lado de uma a uma é o mais instável dos dois — cada linha paga uma viagem de
rede e um `fsync` —, e é por isso que ele é o **controle** e não o resultado:
duas corridas seguidas deram 2.400 e 2.659 linhas/s, enquanto o lote deu 39.038
e 39.287.''')
p.write_text(s)
print("ok")
