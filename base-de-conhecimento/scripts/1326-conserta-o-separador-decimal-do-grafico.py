# Conserta o separador decimal do grafico
# 01/09 18:30

from pathlib import Path
p = Path("bancada/comparacao/grafico.py")
s = p.read_text(encoding="utf-8")
velho = '''def fmt(v):
    if v is None:
        return "nao medido"
    if v >= 100:
        return f"{v:,.0f} s".replace(",", ".")
    if v >= 10:
        return f"{v:.1f} s"
    if v >= 1:
        return f"{v:.2f} s"
    return f"{v * 1000:.0f} ms"'''
novo = '''def numero(v, casas):
    """Virgula decimal e ponto de milhar -- a pagina e em portugues.

    Saia com ponto decimal («9.93 s») ate esta rodada. Nao muda o valor, mas
    muda a LEITURA: quem le em portugues ve «nove mil e noventa e tres».
    """
    bruto = f"{v:,.{casas}f}"           # 1,234.56
    return bruto.replace(",", "\\x00").replace(".", ",").replace("\\x00", ".")


def fmt(v):
    if v is None:
        return "nao medido"
    if v >= 100:
        return f"{numero(v, 0)} s"
    if v >= 10:
        return f"{numero(v, 1)} s"
    if v >= 1:
        return f"{numero(v, 2)} s"
    return f"{numero(v * 1000, 0)} ms"'''
assert s.count(velho) == 1
p.write_text(s.replace(velho, novo), encoding="utf-8")
print("fmt: virgula decimal")
