#!/usr/bin/env python3
"""O SEXTO gerador: os tetos dos desenhos de concorrencia, das corridas cruas.

    python3 docs/dossie/tetos-da-trava.py docs/dossie/dossie-phxsql-0.18.html

Por que ele existe
------------------
A §35 do dossie dizia «a trava global contra o MVCC, medida e ainda sem plano
aprovado». Em 04/09 ela foi medida de verdade, e os quatro numeros que decidem
o desenho passaram a existir. Escreve-los a mao no HTML quebraria a lei que o
proprio dossie carrega -- *todo numero visivel sai de um gerador, ou esta
errado e ninguem percebeu ainda* --, e nesta mesma rodada essa lei cobrou caro:
uma bancada mediu quatro vezes a mesma coisa por um campo que o servidor nao
le, e o numero publicado ficou errado por uma hora.

Entao os tetos saem das CORRIDAS GUARDADAS, que sao a saida crua do medidor,
versionadas em `bancada/concorrencia/corridas/`. Se elas nao estiverem la, ele
reprova em vez de inventar: gerador que devolve vazio quando a fonte sumiu e a
mesma doenca do conferidor que diz «limpo» sem ter conferido nada.

So le arquivo com CERTO no nome. As corridas invalidadas ficam guardadas ao
lado -- apaga-las perderia a serie e a licao --, e e justamente por isso que o
nome, e nao a data, e quem decide o que entra.
"""
import glob
import os
import re
import sys

AQUI = os.path.dirname(os.path.abspath(__file__))
RAIZ = os.path.dirname(os.path.dirname(AQUI))
CORRIDAS = os.path.join(RAIZ, "bancada", "concorrencia", "corridas")

ABRE = "<!-- tetos:inicio (gerado por docs/dossie/tetos-da-trava.py) -->"
FECHA = "<!-- tetos:fim -->"


def corridas(padrao):
    achadas = sorted(glob.glob(os.path.join(CORRIDAS, padrao)))
    if not achadas:
        sys.exit(f"nao achei corrida nenhuma para {padrao} em {CORRIDAS} -- "
                 "sem a fonte crua este gerador NAO inventa numero")
    return achadas


def tetos_do_desenho():
    """Os tetos por durabilidade, de cada corrida do `escolher-o-desenho.py`."""
    saida = {}
    for arq in corridas("desenho-CERTO-50-*.txt"):
        dur = None
        for linha in open(arq, encoding="utf-8"):
            m = re.match(r"== durabilidade: (\S+) ==", linha.strip())
            if m:
                dur = m.group(1)
                continue
            m = re.match(r"\s+(trava por tabela|RwLock|MVCC, EXCLUSIVO)\s+"
                         r"([\d.]+)x", linha)
            if m and dur:
                saida.setdefault((m.group(1), dur), []).append(float(m.group(2)))
    if not saida:
        sys.exit("as corridas do desenho existem mas nao trazem teto nenhum")
    return saida


def teto_do_comboio():
    """O p99 do escritor e do leitor, K=4 contra K=1."""
    saida = {}
    for arq in corridas("comboio-CERTO-*.txt"):
        papel = None
        for linha in open(arq, encoding="utf-8"):
            if "O ESCRITOR" in linha:
                papel = "escritor"
            elif "O LEITOR" in linha:
                papel = "leitor"
            m = re.search(r"K=4 contra K=1:\s+p99 ([\d.]+)x", linha)
            if m and papel:
                saida.setdefault(papel, []).append(float(m.group(1)))
    if not saida:
        sys.exit("as corridas do comboio existem mas nao trazem p99 nenhum")
    return saida


def faixa(vs):
    """Uma medicao vira «1,21x»; duas ou mais viram «1,00x-1,21x».

    A faixa e a media: com duas baterias limpas, dizer a media esconderia que
    uma deu 1,00 e a outra 1,21 -- e a dispersao entre baterias limpas e
    exatamente o que separa um achado de um ruido nesta casa.
    """
    a, b = min(vs), max(vs)
    f = lambda x: f"{x:.2f}".replace(".", ",")
    return f"{f(a)}x" if abs(a - b) < 0.005 else f"{f(a)}x&ndash;{f(b)}x"


def bloco():
    d = tetos_do_desenho()
    c = teto_do_comboio()
    n = len(corridas("desenho-CERTO-50-*.txt"))
    linhas = [
        '<div class="rolo"><table>',
        "<thead><tr><th>candidato</th>"
        '<th class="dado">por_lote (padrão)</th>'
        '<th class="dado">por_operacao</th>'
        "<th>o que ele compra</th></tr></thead><tbody>",
    ]
    for rotulo, chave, compra in (
        ("trava por tabela", "trava por tabela",
         "nada — não é a tabela que serializa"),
        ("<code>RwLock</code>", "RwLock",
         "vazão de leitura; não mexe no escritor"),
        ("Sombra (MVCC), exclusivo", "MVCC, EXCLUSIVO",
         "o <code>fsync</code> atrás da trava, que o "
         "<code>RwLock</code> não toca — e leitura repetível"),
    ):
        lote = d.get((chave, "por_lote"), [])
        oper = d.get((chave, "por_operacao"), [])
        linhas.append(
            f'<tr><td>{rotulo}</td><td class="dado">{faixa(lote)}</td>'
            f'<td class="dado">{faixa(oper)}</td><td>{compra}</td></tr>')
    linhas.append(
        '<tr><td>comboio do fecho de janela</td>'
        f'<td class="dado">{faixa(c["escritor"])}</td>'
        '<td class="dado">—</td>'
        '<td>nada compra: é código de hoje, e nem o <code>RwLock</code> nem o '
        'MVCC o consertam</td></tr>')
    linhas.append("</tbody></table></div>")
    linhas.append(
        f"<p>Medido em máquina parada, com o vigia da bancada aprovando cada "
        f"corrida, e a leitura no tamanho de uma página de grade "
        f"(<code>max: 50</code>). {n} baterias limpas do medidor de desenho; o "
        f"comboio, {len(corridas('comboio-CERTO-*.txt'))}. O p99 do "
        f"<b>leitor</b> no comboio — que lê uma tabela que ninguém escreve — é "
        f"{faixa(c['leitor'])}. As corridas cruas estão versionadas em "
        f"<code>bancada/concorrencia/corridas/</code>, e o parecer inteiro em "
        f"<code>docs/CONCORRENCIA.md</code>.</p>")
    linhas.append(
        "<p><b>A resposta depende de uma configuração, e ela é do dono do "
        "banco:</b> em <code>recursos.durabilidade: por_operacao</code> o "
        "<code>fsync</code> acontece em toda gravação e a Sombra é o maior "
        "ganho da tabela; em <code>por_lote</code>, o padrão, ele acontece uma "
        "vez por janela e quem paga é o <code>RwLock</code> — com o comboio "
        "acima dos dois.</p>")
    return "\n".join(linhas)


def main():
    if len(sys.argv) < 2:
        sys.exit("uso: tetos-da-trava.py <dossie.html>")
    alvo = sys.argv[1]
    s = open(alvo, encoding="utf-8").read()
    i, j = s.find(ABRE), s.find(FECHA)
    if i < 0 or j < 0:
        sys.exit(f"{alvo} nao tem as marcas tetos:inicio/fim")
    novo = s[:i + len(ABRE)] + "\n" + bloco() + "\n" + s[j:]
    open(alvo, "w", encoding="utf-8").write(novo)
    print(f"{os.path.basename(alvo)}: painel dos tetos regravado")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
