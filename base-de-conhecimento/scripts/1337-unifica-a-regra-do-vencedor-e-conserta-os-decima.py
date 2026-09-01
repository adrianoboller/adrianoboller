# Unifica a regra do vencedor e conserta os decimais
# 01/09 18:40

from pathlib import Path

# 1) A regra do vencedor vira funcao unica, no dono do desenho.
g = Path("bancada/comparacao/grafico.py")
s = g.read_text(encoding="utf-8")
s = s.replace('''    # Quem ganhou -- e so entre os que foram medidos. Comparar contra fase que
    # nao rodou daria vencedor por ausencia do outro.
    #
    # E so ha vencedor quando a faixa dele NAO cruza a do segundo colocado.
    # Contornar o mais rapido de 164 ms contra 166 ms, com as duas faixas
    # sobrepostas, e publicar como resultado o que e ruido da maquina.
    melhor = None
    if len(medidos) >= 2:
        ordem = sorted(medidos, key=lambda x: x[2])
        p1, p2 = ordem[0], ordem[1]
        teto_do_1o = p1[4] if p1[4] is not None else p1[2]
        piso_do_2o = p2[3] if p2[3] is not None else p2[2]
        if teto_do_1o < piso_do_2o:
            melhor = p1[0]
    elif len(medidos) == 1:
        melhor = medidos[0][0]''',
'''    melhor = vencedor([(x[0], x[2], x[3], x[4]) for x in medidos])''')

s = s.replace('''def painel(chave, titulo, nota, dados):''',
'''def vencedor(candidatos):
    """Quem ganhou a fase, ou None quando as faixas se cruzam.

    `candidatos` sao tuplas (nome, mediana, minimo, maximo), so dos motores
    MEDIDOS -- comparar contra fase que nao rodou daria vencedor por ausencia
    do outro.

    So ha vencedor quando a faixa do primeiro NAO cruza a do segundo. Marcar
    164 ms contra 166 ms, com as duas faixas sobrepostas (151-215 contra
    158-232), e publicar ruido da maquina como resultado.

    Mora AQUI, e o dossie a importa daqui, porque a primeira versao tinha duas
    copias da regra: consertei a do grafico e a da tabela continuou marcando
    vencedor na busca -- o documento se contradizia a dois centimetros de
    distancia.
    """
    if not candidatos:
        return None
    if len(candidatos) == 1:
        return candidatos[0][0]
    ordem = sorted(candidatos, key=lambda x: x[1])
    p1, p2 = ordem[0], ordem[1]
    teto_do_1o = p1[3] if p1[3] is not None else p1[1]
    piso_do_2o = p2[2] if p2[2] is not None else p2[1]
    return p1[0] if teto_do_1o < piso_do_2o else None


def painel(chave, titulo, nota, dados):''')
g.write_text(s, encoding="utf-8")

# 2) O dossie passa a IMPORTAR a regra, e a escrever numero em portugues.
t = Path("docs/dossie/trio-de-motores.py")
s = t.read_text(encoding="utf-8")
s = s.replace('''import json
import pathlib
import sys

RAIZ = pathlib.Path(__file__).resolve().parents[2]''',
'''import json
import pathlib
import sys

RAIZ = pathlib.Path(__file__).resolve().parents[2]
# A regra de quem venceu a fase vem do dono do desenho, nao de uma copia:
# duas copias divergem, e esta ja divergiu -- a tabela marcava vencedor na
# busca enquanto o grafico, dois centimetros acima, dizia empate.
sys.path.insert(0, str(RAIZ / "bancada" / "comparacao"))
from grafico import vencedor  # noqa: E402''')

s = s.replace('''def seg(v, casas=2):''',
'''def dec(v, casas=2):
    """Numero com virgula decimal -- a pagina e em portugues."""
    bruto = f"{v:,.{casas}f}"
    return bruto.replace(",", "\\x00").replace(".", ",").replace("\\x00", ".")


def seg(v, casas=2):''')

s = s.replace('''        medidos = {m: por[m]["mediana_s"] for m, _ in MOTORES if por[m]["mediana_s"]}
        melhor = min(medidos, key=medidos.get) if medidos else None''',
'''        melhor = vencedor([
            (m, por[m]["mediana_s"], por[m].get("min_s"), por[m].get("max_s"))
            for m, _ in MOTORES if por[m]["mediana_s"] is not None
        ])''')

s = s.replace('''            f" Para a busca isso é <strong>{piso / b * 100:.1f}% da barra dele</strong>:"
            f" sem medir o piso teríamos publicado"
            f" «{b / d['fases']['buscar']['phxsql']['mediana_s']:.2f}× mais rápido»"
            " quando entre motores são"
            f" <strong>{(b - piso) / d['fases']['buscar']['phxsql']['mediana_s']:.2f}×</strong>."''',
'''            f" Para a busca isso é <strong>{dec(piso / b * 100, 1)}% da barra"
            " dele</strong>: sem medir o piso teríamos publicado"
            f" «{dec(b / d['fases']['buscar']['phxsql']['mediana_s'])}× mais rápido»"
            " quando entre motores são <strong>"
            f"{dec((b - piso) / d['fases']['buscar']['phxsql']['mediana_s'])}×</strong>."''')

s = s.replace('''        mib = lambda b: f"{b / 1048576:.1f}".replace(".", ",")
        linha_disco = (
            f"<p><strong>E o disco:</strong> {mib(disco['phxsql'])} MiB contra"
            f" {mib(disco['sqlite'])} do SQLite(R) e {mib(disco['mysql'])} do"
            f" MySQL(R) — <strong>{disco['phxsql'] / disco['sqlite']:.2f}×</strong> e"
            f" <strong>{disco['phxsql'] / disco['mysql']:.2f}×</strong>. É o preço do"''',
'''        mib = lambda b: dec(b / 1048576, 1)
        linha_disco = (
            f"<p><strong>E o disco:</strong> {mib(disco['phxsql'])} MiB contra"
            f" {mib(disco['sqlite'])} do SQLite(R) e {mib(disco['mysql'])} do"
            f" MySQL(R) — <strong>{dec(disco['phxsql'] / disco['sqlite'])}×</strong> e"
            f" <strong>{dec(disco['phxsql'] / disco['mysql'])}×</strong>. É o preço do"''')
t.write_text(s, encoding="utf-8")
print("regra do vencedor unificada; numeros em portugues")
