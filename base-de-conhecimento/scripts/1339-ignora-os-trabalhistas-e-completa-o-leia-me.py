# Ignora os trabalhistas e completa o LEIA-ME
# 01/09 18:44

from pathlib import Path
p = Path(".gitignore")
s = p.read_text(encoding="utf-8")
alvo = "/bancada/resultados.parcial.json\n"
assert s.count(alvo) == 1
s = s.replace(alvo, alvo + """
# A bancada dos tres motores tem os mesmos dois trabalhistas: o arquivo por
# onde o comando vai ao mysql, e a medicao pela metade enquanto a corrida anda.
/bancada/comparacao/comando.sql
/bancada/comparacao/um-milhao.parcial.json
""")
p.write_text(s, encoding="utf-8")

# O LEIA-ME lista os arquivos da pasta, e o log da corrida faltava.
q = Path("bancada/comparacao/LEIA-ME.md")
s = q.read_text(encoding="utf-8")
s = s.replace(
    "| `comparacao-tres-motores.html` | a página de pé, para olhar sozinha |",
    "| `comparacao-tres-motores.html` | a página de pé, para olhar sozinha |\n"
    "| `corrida-um-milhao.log` | o registro da corrida que produziu o JSON |",
)
q.write_text(s, encoding="utf-8")
print("gitignore e LEIA-ME em dia")
