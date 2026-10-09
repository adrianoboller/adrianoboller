#!/usr/bin/env python3
"""Leva os numeros ao `docs/MANUAL-PHXZIP.md` -- nenhum se digita la.

    python3 bancada/phxzip/manual.py             reescreve os blocos marcados
    python3 bancada/phxzip/manual.py --catraca   so confere: sai 1 se o manual
                                                 difere do que este gerador
                                                 escreveria (numero velho)

Dois blocos, cada um entre `<!-- phxzip:<nome>:inicio -->` e
`<!-- phxzip:<nome>:fim -->`:

* `bancada` -- do `bancada/phxzip/resultados.json` (o `medir.py` grava), com
  a data da medicao, mediana e faixa min--max de cada numero. O vencedor so
  e dito quando as faixas NAO se cruzam (a regra do pedido 155: esta casa ja
  declarou vencedor dentro do ruido uma vez).
* `tetos` -- das constantes do proprio fonte (`TETO_*` do PhxZipCmd, os
  limites da porta web e o `Limites` do motor). A receita do numero sai do
  codigo, e nao de uma lista copiada aqui: quando o teto mudar no fonte, o
  `--catraca` acusa o manual velho.

Bloco marcado que falta no manual reprova -- gerador que escreve menos do que
promete tem de dizer que escreveu menos.
"""
import json
import re
import sys
from pathlib import Path

AQUI = Path(__file__).resolve().parent
RAIZ = AQUI.parent.parent
MANUAL = RAIZ / "docs" / "MANUAL-PHXZIP.md"
RESULTADOS = AQUI / "resultados.json"
CMD = RAIZ / "crates" / "phxzip-cmd" / "src" / "lib.rs"
WEB = RAIZ / "crates" / "phxzip-web" / "src" / "lib.rs"
OPS = RAIZ / "crates" / "phxzip-web" / "src" / "ops.rs"
LEITOR = RAIZ / "crates" / "phxzip" / "src" / "leitor.rs"
CHAVE = RAIZ / "crates" / "phxzip" / "src" / "chave.rs"

SEM_TETO = None


# ------------------------------------------------------------- o fonte

def avaliar(expr: str, nomes: dict):
    """Avalia a expressao inteira de uma constante: numeros, `<<`, `*` e nomes
    ja conhecidos. Qualquer outra coisa reprova -- adivinhar seria publicar
    numero que o fonte nao diz."""
    # `_` so entre digitos (`65_536`); no nome (`CICLOS_PADRAO`) ele fica.
    expr = re.sub(r"(?<=\d)_(?=\d)", "", expr.strip())
    if expr in ("u64::MAX", "usize::MAX", "u32::MAX"):
        return SEM_TETO
    tokens = re.findall(r"\d+|<<|\*|[A-Za-z][A-Za-z0-9_:]*", expr)
    if "".join(tokens) != expr.replace(" ", ""):
        sys.exit(f"manual.py: expressao que nao sei avaliar: {expr!r}")

    def valor(t):
        if t.isdigit():
            return int(t)
        nome = t.split("::")[-1]
        if nome not in nomes:
            sys.exit(f"manual.py: nome desconhecido na expressao: {t!r}")
        return nomes[nome]

    # `<<` liga mais fraco que `*` no Rust; as constantes daqui so usam um ou
    # outro, mas a ordem certa custa uma linha.
    soma = None
    for parte in " ".join(tokens).split("<<"):
        fatores = [valor(t) for t in parte.split() if t != "*"]
        v = 1
        for f in fatores:
            v *= f
        soma = v if soma is None else soma << v
    return soma


def constante(arquivo: Path, nome: str, nomes=None):
    texto = arquivo.read_text()
    m = re.search(rf"const {nome}: [\w:]+ = ([^;]+);", texto)
    if not m:
        sys.exit(f"manual.py: {nome} nao achado em {arquivo.relative_to(RAIZ)}")
    return avaliar(m.group(1), nomes or {})


def campos_do_literal(texto: str, inicio: str):
    """Os `campo: expr,` do primeiro literal `Limites {` depois de `inicio`."""
    i = texto.find(inicio)
    if i < 0:
        sys.exit(f"manual.py: {inicio!r} nao achado em leitor.rs")
    j = texto.find("Limites {", i + len(inicio))
    k = texto.find("}", j)
    corpo = texto[j + len("Limites {"):k]
    return dict(re.findall(r"(\w+): ([^,\n]+),", corpo))


def tetos():
    chave = {n: constante(CHAVE, n) for n in ("CICLOS_PADRAO", "CICLOS_MAXIMO")}
    leitor = LEITOR.read_text()
    padrao = {k: avaliar(v, chave)
              for k, v in campos_do_literal(leitor, "impl Default for Limites").items()}
    conf = dict(padrao)
    conf.update({k: avaliar(v, chave)
                 for k, v in campos_do_literal(leitor, "pub fn confiavel()").items()})
    return {
        "senha": constante(CMD, "TETO_DA_SENHA"),
        "arquivo": constante(CMD, "TETO_DO_ARQUIVO"),
        "porta": constante(WEB, "PORTA_PADRAO"),
        "envio": constante(WEB, "ENVIO_MAX"),
        "conexoes": constante(WEB, "CONEXOES_MAX"),
        "cabeca": constante(OPS, "CABECA_MAX"),
        "espiar": constante(OPS, "ESPIAR_MAX"),
        "simultaneas": constante(OPS, "SIMULTANEAS"),
        "padrao": padrao,
        "confiavel": conf,
    }


def bytes_legiveis(n):
    if n is SEM_TETO:
        return "sem teto"
    for unidade, desloc in (("GiB", 30), ("MiB", 20), ("KiB", 10)):
        if n >= 1 << desloc and n % (1 << desloc) == 0:
            return f"{n >> desloc} {unidade}"
    return f"{n:,}".replace(",", ".") + " bytes"


def contagem(n):
    return "sem teto" if n is SEM_TETO else f"{n:,}".replace(",", ".")


def bloco_tetos():
    t = tetos()
    p, c = t["padrao"], t["confiavel"]
    linhas = [
        "| teto | padrão | com `--confiavel` | de onde sai |",
        "|---|---:|---:|---|",
        f"| arquivo lido pelo `phxzipcmd` | {bytes_legiveis(t['arquivo'])} | sem teto | `TETO_DO_ARQUIVO` |",
        f"| soma descompactada de um arquivo | {bytes_legiveis(p['total'])} | {bytes_legiveis(c['total'])} | `Limites::total` |",
        f"| uma entrada, descompactada | {bytes_legiveis(p['entrada'])} | {bytes_legiveis(c['entrada'])} | `Limites::entrada` |",
        f"| um bloco, descompactado | {bytes_legiveis(p['bloco'])} | {bytes_legiveis(c['bloco'])} | `Limites::bloco` |",
        f"| cabeçalho | {bytes_legiveis(p['cabecalho'])} | {bytes_legiveis(c['cabecalho'])} | `Limites::cabecalho` |",
        f"| entradas declaradas | {contagem(p['entradas'])} | {contagem(c['entradas'])} | `Limites::entradas` |",
        f"| ciclos do 7zAES (2^n rodadas de SHA-256) | {p['ciclos']} | {c['ciclos']} | `Limites::ciclos` |",
        f"| senha pela entrada padrão | {contagem(t['senha'])} bytes | igual | `TETO_DA_SENHA` |",
        "",
        "| teto do `phxzipweb` | valor | de onde sai |",
        "|---|---:|---|",
        f"| porta padrão (só `127.0.0.1`) | {t['porta']} | `PORTA_PADRAO` |",
        f"| envio (corpo de um pedido; `--envio` só baixa) | {bytes_legiveis(t['envio'])} | `ENVIO_MAX` |",
        f"| cabeça JSON do envelope | {bytes_legiveis(t['cabeca'])} | `CABECA_MAX` |",
        f"| espiar uma entrada | {bytes_legiveis(t['espiar'])} | `ESPIAR_MAX` |",
        f"| operações pesadas ao mesmo tempo | {t['simultaneas']} | `SIMULTANEAS` |",
        f"| conexões abertas | {t['conexoes']} | `CONEXOES_MAX` |",
        "",
        "A porta web abre todo arquivo com os limites **padrão**: o que chega pelo",
        "navegador é sempre «de fora».",
    ]
    return "\n".join(linhas)


# ------------------------------------------------------------- a bancada

def seg(x):
    return f"{x:.2f}".replace(".", ",")


def faixa(r):
    return f"{seg(r['mediana'])} s ({seg(r['min'])}–{seg(r['max'])})"


def razao(a, b):
    return f"{a / b:.2f}".replace(".", ",") + "×"


def comparar(rp, r7, oque):
    """Frase da comparacao de tempo PhxZip x 7z igual. Vencedor so sem
    cruzamento de faixas."""
    if rp["max"] < r7["min"]:
        return f"{oque}: o PhxZip é mais rápido ({razao(r7['mediana'], rp['mediana'])} pela mediana; as faixas não se cruzam)"
    if r7["max"] < rp["min"]:
        return f"{oque}: o 7z é mais rápido ({razao(rp['mediana'], r7['mediana'])} pela mediana; as faixas não se cruzam)"
    return f"{oque}: **empate dentro do ruído** — as faixas se cruzam (medianas {seg(rp['mediana'])} s × {seg(r7['mediana'])} s)"


def bloco_bancada():
    if not RESULTADOS.is_file():
        return ("**NÃO MEDIDA** — não há `bancada/phxzip/resultados.json`. Para medir:\n\n"
                "```bash\ncargo build --release --offline -p phxzip-cmd\n"
                "python3 bancada/phxzip/medir.py\npython3 bancada/phxzip/manual.py\n```")
    d = json.loads(RESULTADOS.read_text())
    lados = d["lados"]
    out = [
        f"Medido em **{d['medido_em']}**, commit `{d['commit'][:8]}`, {d['rodadas']} rodadas "
        f"intercaladas, {d['nucleos']} núcleos, carga média no início "
        f"{' / '.join(str(x).replace('.', ',') for x in d['carga_no_inicio'])} e no fim "
        f"{' / '.join(str(x).replace('.', ',') for x in d['carga_no_fim'])}.",
        f"`{d['versao_phxzipcmd']}` contra `{d['versao_7z']}`.",
        "",
        "Os lados:",
        "",
    ]
    for ln, l in lados.items():
        marca = "trabalho igual" if l["trabalho_igual"] else "**referência — NÃO é trabalho igual**"
        out.append(f"- `{ln}`: {l['rotulo']} — {marca}")
    for cn, c in d["conjuntos"].items():
        out += [
            "",
            f"**Conjunto `{cn}`** — {c['descricao']}: {contagem(c['arquivos'])} "
            f"{'arquivo' if c['arquivos'] == 1 else 'arquivos'}, "
            f"{contagem(c['bytes'])} bytes.",
            "",
            "| lado | compactar (mediana, faixa) | extrair (mediana, faixa) | tamanho do `.7z` | do original |",
            "|---|---:|---:|---:|---:|",
        ]
        for ln, m in c["medidas"].items():
            r = m["resumo"]
            tam = r["tamanho"]["mediana"]
            pct = f"{100 * tam / c['bytes']:.1f}".replace(".", ",") + "%"
            tam_txt = contagem(int(tam))
            if r["tamanho"]["min"] != r["tamanho"]["max"]:
                tam_txt += " (variou!)"
            nome = f"`{ln}`" if lados[ln]["trabalho_igual"] else f"`{ln}` (referência)"
            out.append(f"| {nome} | {faixa(r['compactar_s'])} | {faixa(r['extrair_s'])} | {tam_txt} | {pct} |")
        rp, r7 = c["medidas"]["phxzip"]["resumo"], c["medidas"]["7z-igual"]["resumo"]
        tp, t7 = rp["tamanho"]["mediana"], r7["tamanho"]["mediana"]
        if tp == t7:
            dif = "o mesmo do 7z, byte a byte"
        else:
            lado = "maior" if tp > t7 else "menor"
            pct = f"{100 * abs(tp - t7) / t7:.2f}".replace(".", ",")
            dif = f"{contagem(int(abs(tp - t7)))} bytes {lado} que o do 7z ({pct}%)"
        out += [
            "",
            f"PhxZip × 7z com trabalho igual, no `{cn}`:",
            "",
            f"- {comparar(rp['compactar_s'], r7['compactar_s'], 'compactar')};",
            f"- {comparar(rp['extrair_s'], r7['extrair_s'], 'extrair')};",
            f"- tamanho: o do PhxZip é {dif}.",
        ]
    return "\n".join(out)


# ------------------------------------------------------------- o manual

BLOCOS = {"bancada": bloco_bancada, "tetos": bloco_tetos}


def montar(texto: str) -> str:
    for nome, gerar in BLOCOS.items():
        ini, fim = f"<!-- phxzip:{nome}:inicio -->", f"<!-- phxzip:{nome}:fim -->"
        if texto.count(ini) != 1 or texto.count(fim) != 1:
            sys.exit(f"manual.py: o manual nao tem o bloco {nome} marcado uma vez so ({ini} ... {fim})")
        a = texto.index(ini) + len(ini)
        b = texto.index(fim)
        texto = texto[:a] + "\n" + gerar() + "\n" + texto[b:]
    return texto


def main():
    atual = MANUAL.read_text()
    novo = montar(atual)
    if "--catraca" in sys.argv:
        if novo != atual:
            print("REPROVADA: o docs/MANUAL-PHXZIP.md difere do que o gerador escreve "
                  "(resultados.json ou teto do fonte mudou). Rode: python3 bancada/phxzip/manual.py")
            sys.exit(1)
        print("segura: os numeros do docs/MANUAL-PHXZIP.md batem com o resultados.json e o fonte")
        return
    if novo != atual:
        MANUAL.write_text(novo)
        print(f"reescrito {MANUAL.relative_to(RAIZ)} ({len(BLOCOS)} blocos)")
    else:
        print(f"{MANUAL.relative_to(RAIZ)} ja estava em dia ({len(BLOCOS)} blocos)")


if __name__ == "__main__":
    main()
