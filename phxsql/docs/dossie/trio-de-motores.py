#!/usr/bin/env python3
"""Escreve o bloco dos tres motores no dossie, da medicao de um milhao.

    python3 docs/dossie/trio-de-motores.py [caminho/do/dossie.html]

E o SEXTO gerador do dossie. Escreve o bloco `trio:`, dentro da secao da
bancada: o grafico, a tabela das medianas e o que os numeros nao dizem.

Duas decisoes que valem explicar
--------------------------------
**Nao redesenha nada.** O SVG sai do `bancada/comparacao/grafico.py`, que e o
dono do desenho, e este script so o INSERE. Duas receitas para a mesma figura
e a mesma armadilha da lista de arquivos copiada dentro do gerador do rodape:
elas divergem, e a divergencia aparece meses depois num numero publicado.

**Recusa desenho velho.** Se o SVG for mais antigo que o JSON da medicao, o
gerador PARA. E a licao do binario velho aplicada a uma figura: um grafico
gerado antes da ultima corrida publica o passado com data de hoje, e nada no
desenho denuncia isso.
"""

import json
import pathlib
import sys

RAIZ = pathlib.Path(__file__).resolve().parents[2]
# A regra de quem venceu a fase vem do dono do desenho, nao de uma copia:
# duas copias divergem, e esta ja divergiu -- a tabela marcava vencedor na
# busca enquanto o grafico, dois centimetros acima, dizia empate.
sys.path.insert(0, str(RAIZ / "bancada" / "comparacao"))
from grafico import vencedor  # noqa: E402
MEDICAO = RAIZ / "bancada" / "comparacao" / "um-milhao.json"
FIGURA = RAIZ / "bancada" / "comparacao" / "comparacao-tres-motores.svg"

ABRE = "<!-- trio:inicio (gerado por docs/dossie/trio-de-motores.py) -->"
FECHA = "<!-- trio:fim -->"

MOTORES = [("phxsql", "PhxSql"), ("sqlite", "SQLite(R)"), ("mysql", "MySQL(R)")]
FASES = [
    ("inserir", "Inserir {n}"),
    ("buscar", "Buscar {ops} pontuais"),
    ("atualizar", "Atualizar {ops}"),
    ("excluir", "Excluir {ops}"),
]


def _alvo():
    for a in sys.argv[1:]:
        if a.endswith(".html"):
            return pathlib.Path(a).resolve()
    return RAIZ / "docs" / "dossie" / "dossie-phxsql-0.18.html"


def mil(x):
    return f"{x:,}".replace(",", ".")


def dec(v, casas=2):
    """Numero com virgula decimal -- a pagina e em portugues."""
    bruto = f"{v:,.{casas}f}"
    return bruto.replace(",", "\x00").replace(".", ",").replace("\x00", ".")


def seg(v, casas=2):
    if v is None:
        return "—"
    if v < 1:
        return f"{v * 1000:.0f} ms"
    bruto = f"{v:,.{casas}f}"
    return bruto.replace(",", "\x00").replace(".", ",").replace("\x00", ".") + " s"


def tabela(d):
    n, ops = d["linhas"], d["operacoes_por_fase_pontual"]
    piso = (d.get("piso_do_mysql_s") or {}).get("mediana_s")
    linhas = [
        "<table class=\"tab\">",
        "<thead><tr><th>fase</th>"
        + "".join(f"<th class=\"num\">{r}</th>" for _, r in MOTORES)
        + "<th class=\"num\">MySQL(R) menos o piso</th></tr></thead>",
        "<tbody>",
    ]
    for chave, molde in FASES:
        por = d["fases"][chave]
        melhor = vencedor([
            (m, por[m]["mediana_s"], por[m].get("min_s"), por[m].get("max_s"))
            for m, _ in MOTORES if por[m]["mediana_s"] is not None
        ])
        celulas = ""
        for m, _ in MOTORES:
            v = por[m]["mediana_s"]
            forte = " class=\"num destaque\"" if m == melhor else " class=\"num\""
            celulas += f"<td{forte}>{seg(v)}</td>"
        # A carga inicial nao leva desconto: o piso foi medido para 20.000
        # instrucoes pontuais, e o `INSERT` sao 20 instrucoes grandes. Descontar
        # ali seria aplicar um numero fora da condicao em que ele foi medido.
        liquido = (
            seg(por["mysql"]["mediana_s"] - piso)
            if piso and chave != "inserir" and por["mysql"]["mediana_s"]
            else "—"
        )
        rotulo = molde.format(n=mil(n), ops=mil(ops))
        linhas.append(f"<tr><td>{rotulo}</td>{celulas}<td class=\"num\">{liquido}</td></tr>")
    linhas += ["</tbody></table>"]
    return "\n".join(linhas)


def bloco(d):
    n, ops = d["linhas"], d["operacoes_por_fase_pontual"]
    piso = (d.get("piso_do_mysql_s") or {}).get("mediana_s")
    fatia = ""
    if piso:
        b = d["fases"]["buscar"]["mysql"]["mediana_s"]
        fatia = (
            f" Para a busca isso é <strong>{dec(piso / b * 100, 1)}% da barra"
            " dele</strong>: sem medir o piso teríamos publicado"
            f" «{dec(b / d['fases']['buscar']['phxsql']['mediana_s'])}× mais rápido»"
            " quando entre motores são <strong>"
            f"{dec((b - piso) / d['fases']['buscar']['phxsql']['mediana_s'])}×</strong>."
        )
    disco = d.get("disco_bytes") or {}
    linha_disco = ""
    if disco.get("phxsql") and disco.get("sqlite"):
        mib = lambda b: dec(b / 1048576, 1)
        linha_disco = (
            f"<p><strong>E o disco:</strong> {mib(disco['phxsql'])} MiB contra"
            f" {mib(disco['sqlite'])} do SQLite(R) e {mib(disco['mysql'])} do"
            f" MySQL(R) — <strong>{dec(disco['phxsql'] / disco['sqlite'])}×</strong> e"
            f" <strong>{dec(disco['phxsql'] / disco['mysql'])}×</strong>. É o preço do"
            " modelo de arquivos separados, e no celular essa é a pergunta"
            " inteira.</p>"
        )
    ress = "".join(f"<li>{r}</li>" for r in (d.get("ressalvas") or []))
    dur = " · ".join(f"<strong>{k}</strong>: {v}" for k, v in (d.get("durabilidade") or {}).items())

    return f"""
  <h3>Os três motores, a um milhão de linhas</h3>
  <p>Tabela de {mil(n)} linhas, {mil(ops)} operações nas fases pontuais,
  {d['rodadas']} rodadas, os três <strong>intercalados na mesma rodada</strong> —
  somar medições de dias diferentes daria três colunas e nenhuma comparação.
  Medido em {d.get('medido_em', '—')}.</p>

  <div class="trio">
{FIGURA.read_text(encoding='utf-8')}
  </div>

  {tabela(d)}
  <p class="legenda">Mediana de {d['rodadas']} rodadas; o bigode do gráfico vai
  do mínimo ao máximo. O contorno só marca vencedor quando as faixas
  <em>não</em> se cruzam — na busca elas se cruzam, e é empate.</p>

  <p><strong>O piso que precisou ser medido.</strong> Os três não têm a mesma
  forma: o SQLite(R) é biblioteca em processo, o <code>carga</code> do PhxSql
  também, e o MySQL(R) é daemon que recebe <em>texto</em> por soquete. Não
  existe MySQL(R) embutido nesta máquina, então a barra dele carrega transporte
  e análise que as outras duas não pagam. Isso não se conserta — mede-se:
  {seg(piso, 3) if piso else '—'} para {mil(ops)} instruções que não fazem nada
  (<code>DO 1;</code>).{fatia}</p>

  {linha_disco}

  <p><strong>Onde perdemos, sem rodeio:</strong> a inserção, para o SQLite(R), e
  a exclusão também. Ganhamos o <code>UPDATE</code>, e a busca empata. Está tudo
  na tabela acima, com o vencedor de cada fase em destaque — inclusive quando
  não somos nós.</p>

  <p class="legenda">Durabilidade durante a medida — {dur}</p>
  <p><strong>O que estes números não dizem</strong></p>
  <ul>{ress}</ul>
"""


CSS = """
<style>
/* Dois paineis por linha onde couber. Sem a grade eles empilham em largura
   cheia, e um viewBox de 460 esticado a 1.000 px vira uma figura do tamanho
   da tela para dizer tres numeros. */
.trio{margin:18px 0;display:grid;gap:16px;
  grid-template-columns:repeat(auto-fit,minmax(330px,1fr))}
.trio .fig{margin:0 0 14px;border:1px solid var(--linha);border-radius:10px;padding:12px}
.trio .fig svg{width:100%;height:auto;display:block}
.trio .tit{font:600 15px system-ui;fill:var(--tinta)}
.trio .nota{font:12px system-ui;fill:var(--tinta-3,var(--tinta))}
.trio .eixo{font:13px system-ui;fill:var(--tinta-2,var(--tinta))}
.trio .valor{font:600 13px system-ui;fill:var(--tinta);font-variant-numeric:tabular-nums}
.trio .valor.dentro{fill:#fff}
.trio .ausente{font:italic 13px system-ui;fill:var(--tinta-3,var(--tinta))}
.trio .bigode{stroke:var(--tinta-3,var(--tinta));stroke-width:2}
.trio .barra.vencedor{stroke:var(--tinta);stroke-width:2}
/* As tres cores sao as da marca, com passo proprio no tema escuro: tema
   escuro nao e tema claro invertido, e os tokens claros reprovam na faixa de
   luminosidade sobre fundo escuro. */
.trio{--m-phx:#c63c0a;--m-sql:#1f5c93;--m-lite:#37702e}
@media (prefers-color-scheme:dark){
  :root:not([data-theme="light"]) .trio{--m-phx:#d9741c;--m-sql:#4287cf;--m-lite:#54a84c}
}
:root[data-theme="dark"] .trio{--m-phx:#d9741c;--m-sql:#4287cf;--m-lite:#54a84c}
</style>
"""


def main():
    if not MEDICAO.exists():
        sys.exit(
            f"nao achei {MEDICAO}.\n"
            "O bloco sai da medicao -- rode `python3 bancada/comparacao/medir.py`."
        )
    if not FIGURA.exists():
        sys.exit(
            f"nao achei {FIGURA}.\n"
            "Rode `python3 bancada/comparacao/grafico.py` -- este script INSERE a\n"
            "figura, nao a desenha: duas receitas para o mesmo desenho divergem."
        )
    if FIGURA.stat().st_mtime < MEDICAO.stat().st_mtime:
        sys.exit(
            "a figura e MAIS VELHA que a medicao.\n"
            "Rode `python3 bancada/comparacao/grafico.py` antes: publicar um\n"
            "grafico desenhado da corrida anterior e publicar o passado com data\n"
            "de hoje, e nada no desenho denuncia isso."
        )

    d = json.loads(MEDICAO.read_text(encoding="utf-8"))
    dossie = _alvo()
    html = dossie.read_text(encoding="utf-8")
    i, j = html.find(ABRE), html.find(FECHA)
    if i < 0 or j < 0:
        sys.exit("as marcas trio:inicio/trio:fim nao estao no dossie")
    html = html[:i] + ABRE + CSS + bloco(d) + FECHA + html[j + len(FECHA):]
    dossie.write_text(html, encoding="utf-8")

    print(f"bloco do trio refeito em {dossie.name}, de {MEDICAO.name}")
    for chave, _ in FASES:
        por = d["fases"][chave]
        print(f"  {chave:<10} " + "  ".join(
            f"{r} {seg(por[m]['mediana_s'])}" for m, r in MOTORES
        ))


if __name__ == "__main__":
    main()
