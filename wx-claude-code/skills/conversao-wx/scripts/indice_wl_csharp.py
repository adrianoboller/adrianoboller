#!/usr/bin/env python3
"""Le o metadado .NET do WL.dll (WL_C#) e gera resources/wl-csharp/funcoes.json.

So std. Le as tabelas TypeDef e MethodDef do fluxo #~ (ECMA-335 II.22) e lista
o que o codigo C# convertido pode chamar: metodos publicos e estaticos das
classes estaticas (as funcoes), classes de instancia (os tipos avancados, com
propriedades e metodos), enumeracoes e constantes. Antes o indice saia de `strings`, que truncava nome
com acento e nao enxergava tudo; por isso o numero mudou de 261 para o que o
metadado tem de fato.

Uso: indice_wl_csharp.py <WL.dll> [--versao 1.2] [--origem URL] [--saida funcoes.json]
     indice_wl_csharp.py <WL.dll> --conferir   (compara com o funcoes.json gravado)
"""
import argparse
import hashlib
import json
import struct
import sys
from pathlib import Path

RAIZ = Path(__file__).resolve().parents[1]
SAIDA_PADRAO = RAIZ / "resources" / "wl-csharp" / "funcoes.json"

# ECMA-335 II.22: tabelas na ordem do #~; so precisamos do tamanho das linhas
# ate MethodDef, mas o tamanho depende da contagem de TODAS as tabelas.
TAG = {
    "TypeDefOrRef": (2, [0x02, 0x01, 0x1B]),
    "HasConstant": (2, [0x04, 0x08, 0x17]),
    "HasCustomAttribute": (5, [0x06, 0x04, 0x01, 0x02, 0x08, 0x09, 0x0A, 0x00, 0x0E, 0x17, 0x14, 0x11, 0x1A, 0x1B, 0x20, 0x23, 0x26, 0x27, 0x28, 0x2A, 0x2C]),
    "HasFieldMarshal": (1, [0x04, 0x08]),
    "HasDeclSecurity": (2, [0x02, 0x06, 0x20]),
    "MemberRefParent": (3, [0x02, 0x01, 0x1A, 0x06, 0x1B]),
    "HasSemantics": (1, [0x14, 0x17]),
    "MethodDefOrRef": (1, [0x06, 0x0A]),
    "MemberForwarded": (1, [0x04, 0x06]),
    "Implementation": (2, [0x26, 0x23, 0x27]),
    "CustomAttributeType": (3, [0x06, 0x0A]),
    "ResolutionScope": (2, [0x00, 0x1A, 0x23, 0x01]),
    "TypeOrMethodDef": (1, [0x02, 0x06]),
}


def rva_para_offset(secoes, rva):
    for va, tam, raw in secoes:
        if va <= rva < va + tam:
            return raw + (rva - va)
    raise ValueError(f"RVA {rva:#x} fora das secoes")


def ler_metadado(dados):
    pe = struct.unpack_from("<I", dados, 0x3C)[0]
    assert dados[pe:pe + 4] == b"PE\0\0", "nao e um PE"
    n_secoes = struct.unpack_from("<H", dados, pe + 6)[0]
    tam_opt = struct.unpack_from("<H", dados, pe + 20)[0]
    opt = pe + 24
    magic = struct.unpack_from("<H", dados, opt)[0]
    dir_off = opt + (96 if magic == 0x10B else 112)
    cli_rva = struct.unpack_from("<I", dados, dir_off + 14 * 8)[0]
    sec = opt + tam_opt
    secoes = []
    for i in range(n_secoes):
        s = sec + i * 40
        vsize, va, rawsize, raw = struct.unpack_from("<IIII", dados, s + 8)
        secoes.append((va, max(vsize, rawsize), raw))
    cli = rva_para_offset(secoes, cli_rva)
    md_rva, md_tam = struct.unpack_from("<II", dados, cli + 8)
    md = rva_para_offset(secoes, md_rva)
    assert dados[md:md + 4] == b"BSJB", "sem metadado CLI"
    tam_versao = struct.unpack_from("<I", dados, md + 12)[0]
    p = md + 16 + tam_versao
    n_fluxos = struct.unpack_from("<H", dados, p + 2)[0]
    p += 4
    fluxos = {}
    for _ in range(n_fluxos):
        off, tam = struct.unpack_from("<II", dados, p)
        fim = dados.index(b"\0", p + 8)
        nome = dados[p + 8:fim].decode()
        fluxos[nome] = (md + off, tam)
        p = (fim + 4) & ~3
    return fluxos


def ler_tabelas(dados, fluxos):
    tab_off, _ = fluxos["#~"] if "#~" in fluxos else fluxos["#-"]
    str_off, str_tam = fluxos["#Strings"]
    heap = dados[tab_off + 6]
    valido = struct.unpack_from("<Q", dados, tab_off + 8)[0]
    p = tab_off + 24
    linhas = {}
    for t in range(64):
        if valido >> t & 1:
            linhas[t] = struct.unpack_from("<I", dados, p)[0]
            p += 4
    tam_str = 4 if heap & 1 else 2
    tam_guid = 4 if heap & 2 else 2
    tam_blob = 4 if heap & 4 else 2

    def idx(t):
        return 4 if linhas.get(t, 0) >= 0x10000 else 2

    def codado(nome):
        bits, tabs = TAG[nome]
        maior = max(linhas.get(t, 0) for t in tabs)
        return 4 if maior >= 1 << (16 - bits) else 2

    esquema = {
        0x00: [2, tam_str, tam_guid, tam_guid, tam_guid],
        0x01: [codado("ResolutionScope"), tam_str, tam_str],
        0x02: [4, tam_str, tam_str, codado("TypeDefOrRef"), idx(0x04), idx(0x06)],
        0x03: [idx(0x04)],
        0x04: [2, tam_str, tam_blob],
        0x05: [idx(0x06)],
        0x06: [4, 2, 2, tam_str, tam_blob, idx(0x08)],
    }
    inicio = {}
    for t in sorted(linhas):
        if t > 0x06:
            break
        inicio[t] = p
        p += sum(esquema[t]) * linhas[t]

    def texto(i):
        a = str_off + i
        return dados[a:dados.index(b"\0", a)].decode("utf-8", "replace")

    def campos(t, n):
        base = inicio[t] + sum(esquema[t]) * (n - 1)
        out = []
        for tam in esquema[t]:
            out.append(struct.unpack_from("<I" if tam == 4 else "<H", dados, base)[0])
            base += tam
        return out

    tipos = [campos(0x02, i) for i in range(1, linhas.get(0x02, 0) + 1)]
    metodos = [campos(0x06, i) for i in range(1, linhas.get(0x06, 0) + 1)]
    tiporefs = [campos(0x01, i) for i in range(1, linhas.get(0x01, 0) + 1)]
    fields = [campos(0x04, i) for i in range(1, linhas.get(0x04, 0) + 1)]
    return Tabelas(tipos, metodos, tiporefs, fields, texto)


class Tabelas:
    def __init__(self, tipos, metodos, tiporefs, fields, texto):
        self.tipos, self.metodos, self.tiporefs, self.fields, self.texto = tipos, metodos, tiporefs, fields, texto

    def nome_base(self, extends):
        """Nome do tipo pai (coded index TypeDefOrRef): e o que separa enum e delegate de classe."""
        tag, idx = extends & 3, extends >> 2
        if idx == 0:
            return ""
        if tag == 0:
            return self.texto(self.tipos[idx - 1][1])
        if tag == 1:
            return self.texto(self.tiporefs[idx - 1][1])
        return ""

    def faixa(self, i, col, tabela):
        ini = self.tipos[i][col]
        fim = self.tipos[i + 1][col] if i + 1 < len(self.tipos) else len(tabela) + 1
        return range(ini, fim)


OCULTOS = (".", "<", "get_", "set_", "add_", "remove_", "op_")


def listar(dados):
    """Le do metadado o que o codigo C# convertido pode chamar: funcoes (metodos
    publicos e estaticos das classes estaticas), tipos avancados (classes de
    instancia, com propriedades e metodos), enumeracoes e constantes."""
    tb = ler_tabelas(dados, ler_metadado(dados))
    texto = tb.texto
    funcoes, tipos_av, enums, constantes = {}, {}, {}, {}
    for i, (flags, nome_i, ns_i, ext, _campo, _met) in enumerate(tb.tipos):
        if flags & 7 not in (1, 2):  # Public, NestedPublic
            continue
        classe = texto(nome_i)
        if classe.startswith("<"):
            continue
        base = tb.nome_base(ext)
        ns = texto(ns_i)
        chave = f"{ns}.{classe}" if ns else classe
        campos = [tb.fields[f - 1] for f in tb.faixa(i, 4, tb.fields)]
        if base == "Enum":
            enums[chave] = [texto(n) for fl, n, _ in campos if fl & 0x40]  # Literal
            continue
        if base == "MulticastDelegate":
            continue
        pub = [(tb.metodos[m - 1][2], texto(tb.metodos[m - 1][3])) for m in tb.faixa(i, 5, tb.metodos)]
        pub = [(fl, n) for fl, n in pub if fl & 7 == 6]
        estaticos = sorted({n for fl, n in pub if fl & 0x10 and not n.startswith(OCULTOS)}, key=str.casefold)
        # campos publicos, estaticos e literais: as constantes do WLanguage (Vrai, Faux, TAB, RC...)
        lits = sorted({texto(n) for fl, n, _ in campos if fl & 7 == 6 and fl & 0x10 and fl & 0x40}, key=str.casefold)
        if lits:
            constantes[chave] = lits
        # metodo estatico e funcao chamavel, esteja numa classe estatica (WL.Chaines)
        # ou num tipo avancado (WL.Image.ChargeImage): entra no indice de funcoes dos dois jeitos
        if estaticos:
            funcoes[chave] = estaticos
        if (flags & 0x180) == 0x180:  # Abstract|Sealed: classe estatica, sem instancia
            continue
        props = sorted({n[4:] for fl, n in pub if not fl & 0x10 and n.startswith("get_")}, key=str.casefold)
        mets = sorted({n for fl, n in pub if not fl & 0x10 and not n.startswith(OCULTOS)}, key=str.casefold)
        if props or mets or estaticos:
            tipos_av[chave.replace("`1", "<T>")] = {"propriedades": props, "metodos": mets, "estaticos": estaticos}
    return funcoes, tipos_av, enums, constantes


def listar_funcoes(dados):
    return listar(dados)[0]


def montar(dll, versao, origem):
    dados = dll.read_bytes()
    por_classe, tipos_av, enums, constantes = listar(dados)
    todas = sorted({f for fs in por_classe.values() for f in fs}, key=str.casefold)
    return {
        "origem": f"WL_C# {versao} ({origem}), funcoes, tipos avancados, enumeracoes e constantes lidos das tabelas TypeDef/MethodDef/Field do metadado .NET de WL.dll",
        "gerado_por": "skills/conversao-wx/scripts/indice_wl_csharp.py",
        "sha256_wl_dll": hashlib.sha256(dados).hexdigest(),
        "tamanho_wl_dll": len(dados),
        "aviso": "Indice de existencia, nao a documentacao: um nome aqui diz que a funcao existe na biblioteca, nao qual a assinatura nem a semantica. O codigo-fonte da WL_C# nao e publicado.",
        "quantidade": len(todas),
        "classes": {k: len(v) for k, v in sorted(por_classe.items())},
        "por_classe": dict(sorted(por_classe.items())),
        "funcoes": todas,
        "quantidade_tipos_avancados": len(tipos_av),
        "tipos_avancados": dict(sorted(tipos_av.items())),
        "enumeracoes": dict(sorted(enums.items())),
        "constantes": dict(sorted(constantes.items())),
    }


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("dll", type=Path)
    ap.add_argument("--versao", default="1.2")
    ap.add_argument("--origem", default="https://github.com/BernardSobra/WL-web/releases/tag/v1.2")
    ap.add_argument("--saida", type=Path, default=SAIDA_PADRAO)
    ap.add_argument("--conferir", action="store_true", help="nao grava; falha se o gravado difere")
    a = ap.parse_args()
    novo = montar(a.dll, a.versao, a.origem)
    if a.conferir:
        velho = json.loads(a.saida.read_text(encoding="utf-8"))
        chaves = ("sha256_wl_dll", "tamanho_wl_dll", "quantidade", "funcoes", "por_classe", "tipos_avancados", "enumeracoes", "constantes")
        dif = [k for k in chaves if velho.get(k) != novo[k]]
        if dif:
            print(f"funcoes.json difere do DLL em: {', '.join(dif)}", file=sys.stderr)
            return 1
        print(f"funcoes.json confere: {novo['quantidade']} funcoes, sha256 {novo['sha256_wl_dll'][:16]}…")
        return 0
    a.saida.write_text(json.dumps(novo, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    print(f"{a.saida}: {novo['quantidade']} funcoes em {len(novo['classes'])} classes, {novo['quantidade_tipos_avancados']} tipos avancados, "
          f"{len(novo['enumeracoes'])} enumeracoes, {sum(len(v) for v in novo['constantes'].values())} constantes; {novo['tamanho_wl_dll']} bytes, sha256 {novo['sha256_wl_dll']}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
