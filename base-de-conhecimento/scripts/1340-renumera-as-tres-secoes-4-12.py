# Renumera as tres secoes 4.12
# 01/09 18:45

from pathlib import Path

d = Path("docs/DESEMPENHO.md")
s = d.read_text(encoding="utf-8")
# A primeira 4.12 fica: e a que treze das dezessete citacoes querem dizer.
s = s.replace("## 4.12 A trava de dados presa atrás de uma leitura de rede",
              "## 4.13 A trava de dados presa atrás de uma leitura de rede")
s = s.replace("## 4.12 `ALTER TABLE ADD COLUMN`: a inferência era «minutos», e são 5,5 s",
              "## 4.14 `ALTER TABLE ADD COLUMN`: a inferência era «minutos», e são 5,5 s")
d.write_text(s, encoding="utf-8")

# As citacoes que apontavam para as duas renumeradas.
for arq, velho, novo in [
    ("docs/REPLICACAO.md", "`DESEMPENHO.md` §4.12", "`DESEMPENHO.md` §4.13"),
    ("CHANGELOG.md", "a alteração custa 6,1% do que custou digitar o\n  dado. `docs/DESEMPENHO.md` §4.12.",
     "a alteração custa 6,1% do que custou digitar o\n  dado. `docs/DESEMPENHO.md` §4.14."),
]:
    p = Path(arq); t = p.read_text(encoding="utf-8")
    assert t.count(velho) == 1, f"{arq}: {t.count(velho)} ocorrencias"
    p.write_text(t.replace(velho, novo), encoding="utf-8")

# No PENDENCIAS sao tres, e cada uma vai para um lado: as duas da trava (147 e
# o item 2 da lista) para 4.13, e a do ALTER TABLE (148) para 4.14.
p = Path("docs/PENDENCIAS.md"); t = p.read_text(encoding="utf-8")
t = t.replace("`docs/FORMATO.md` §1.1, `docs/DESEMPENHO.md` §4.12",
              "`docs/FORMATO.md` §1.1, `docs/DESEMPENHO.md` §4.14")
t = t.replace("`docs/REPLICACAO.md` §18, `docs/DESEMPENHO.md` §4.12",
              "`docs/REPLICACAO.md` §18, `docs/DESEMPENHO.md` §4.13")
t = t.replace("(`docs/REPLICACAO.md` §18, `docs/DESEMPENHO.md` §4.13, guarda",
              "(`docs/REPLICACAO.md` §18, `docs/DESEMPENHO.md` §4.13, guarda")
p.write_text(t, encoding="utf-8")
print("renumeradas")
