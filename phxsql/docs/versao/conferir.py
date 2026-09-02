#!/usr/bin/env python3
"""O que um incremento de versao tocaria -- e o que ele NAO pode tocar.

    python3 docs/versao/conferir.py            confere e lista
    python3 docs/versao/conferir.py 0.19.0     ENSAIA o bump (nao grava)
    python3 docs/versao/conferir.py 0.19.0 --gravar

Por que ele existe
------------------
Palavra do dono: «o contador do projeto parou na versao 18; importante
futuramente ser incrementado». Medido antes de propor: a versao aparece em mais
de VINTE arquivos, e o `confere_versoes` do `empacotar.sh` guarda quatro.

E o resto NAO e esquecimento -- e HISTORIA:

    «Desde a 0.18.0 o cache de paginas...»        docs/FORMATO.md
    «Ate a 0.18.0 o PhxSql sabia copiar e...»     docs/RESTAURACAO.md
    «A gravacao NAO atravessa mais (0.18.0)»      crates/.../ndx.rs

Um `sed` global transformaria essas frases em afirmacoes FALSAS sobre o
passado -- e falsas de um jeito que ninguem revisa, porque a frase continua bem
escrita. Este conferidor existe para que a diferenca entre «muda» e «fica»
apareca ANTES de alguem trocar o numero.

O que ele garante, e o que ele NAO garante
------------------------------------------
GARANTE que a lista e completa: nenhuma ocorrencia escapa, porque ele varre a
arvore em vez de consultar uma lista digitada. E garante que os lugares
CORRENTES concordam entre si -- se o manual diz uma versao e o `Cargo.toml`
diz outra, ele reprova.

NAO GARANTE a classificacao do resto. Ele nao tenta adivinhar historia por
padrao de frase: isso seria resolver por COMPARACAO DE FRASE, que esta casa ja
proibiu para texto de tela e que quebraria calado no dia em que alguem
reescrevesse um paragrafo. Ele poe as ocorrencias na mesa com arquivo, linha e
o texto ao redor, e quem decide e gente.
"""
import re
import subprocess
import sys
from pathlib import Path

RAIZ = Path(__file__).resolve().parents[2]

# Os lugares CORRENTES, cada um com o motivo de estar aqui. Sao poucos de
# proposito: lista curta que alguem consegue conferir vale mais que lista longa
# que ninguem le.
#
# O `index.html` NAO esta aqui, e isso e um achado: a tela tira a versao de
# `api("ping").versao` -- o servidor conta. As mencoes la dentro sao dado do
# modo demonstracao e prosa. A interface ja segue a doutrina.
CORRENTES = [
    ("Cargo.toml", r'^version = "(\d+\.\d+\.\d+)"',
     "a FONTE: os crates herdam com `version.workspace = true`"),
    ("MANUAL.txt", r'versao (\d+\.\d+\.\d+)',
     "o cabecalho do manual, que o `confere_versoes` ja compara"),
]
# Derivados: quem os atualiza e um programa, e nao um editor. Perguntar por eles
# aqui evita o pior dos dois mundos -- alguem editar a mao o que um gerador vai
# sobrescrever, ou esquecer de rodar o gerador e publicar o numero de ontem.
DERIVADOS = [
    ("Cargo.lock", "roda `cargo build` e ele se atualiza sozinho"),
    ("CAPABILITIES.json",
     "roda `python3 docs/dossie/numeros-do-projeto.py` -- e `escrever_capacidades()` "
     "que o grava, e NAO se acha essa linha grepando pelo nome do arquivo: ela diz "
     "`alvo.write_text(...)`, com o nome numa variavel"),
]

IGNORAR = ("target/", ".git/", "docs/dossie/pedidos.html", "pacotes/")


def versao_da_fonte():
    t = (RAIZ / "Cargo.toml").read_text(encoding="utf-8")
    m = re.search(r'^version = "(\d+\.\d+\.\d+)"', t, re.M)
    if not m:
        sys.exit("nao achei a versao no Cargo.toml")
    return m.group(1)


def arquivos():
    r = subprocess.run(["git", "ls-files"], cwd=RAIZ, capture_output=True, text=True)
    for linha in r.stdout.splitlines():
        if any(linha.startswith(i) or f"/{i}" in linha for i in IGNORAR):
            continue
        p = RAIZ / linha
        if p.is_file():
            yield linha, p


def ocorrencias(versao):
    alvo = re.escape(versao)
    achadas = []
    for rel, p in arquivos():
        try:
            texto = p.read_text(encoding="utf-8")
        except (UnicodeDecodeError, OSError):
            continue
        if versao not in texto:
            continue
        for n, linha in enumerate(texto.splitlines(), 1):
            if re.search(alvo, linha):
                achadas.append((rel, n, linha.strip()))
    return achadas


def conferir_correntes(versao):
    problemas = []
    for arq, padrao, _ in CORRENTES:
        p = RAIZ / arq
        if not p.exists():
            problemas.append(f"{arq} nao existe")
            continue
        m = re.search(padrao, p.read_text(encoding="utf-8"), re.M)
        if not m:
            problemas.append(f"{arq}: nao achei o padrao {padrao!r}")
        elif m.group(1) != versao:
            problemas.append(f"{arq} diz {m.group(1)}, e a fonte diz {versao}")
    return problemas


def bump(versao, nova, gravar):
    print(f"\n== ENSAIO do bump {versao} -> {nova} ==" if not gravar
          else f"\n== BUMP {versao} -> {nova} ==")
    for arq, padrao, porque in CORRENTES:
        p = RAIZ / arq
        t = p.read_text(encoding="utf-8")
        novo = re.sub(padrao, lambda m: m.group(0).replace(versao, nova), t, count=1, flags=re.M)
        mudou = novo != t
        print(f"  {'grava' if gravar and mudou else 'trocaria'}  {arq:<16} {porque}")
        if gravar and mudou:
            p.write_text(novo, encoding="utf-8")
    for arq, porque in DERIVADOS:
        print(f"  derivado  {arq:<16} {porque}")
    dossie = sorted((RAIZ / "docs/dossie").glob("dossie-phxsql-*.html"))
    for d in dossie:
        curto = ".".join(nova.split(".")[:2])
        print(f"  a mao     {d.name:<16} vira dossie-phxsql-{curto}.html, "
              "e o anterior sai no MESMO commit")
    print(f"  a mao     {'CHANGELOG.md':<16} secao NOVA; as mencoes antigas sao historia")
    if not gravar:
        print("\n  (nada foi gravado -- acrescente --gravar)")


def principal():
    versao = versao_da_fonte()
    print(f"versao na fonte (Cargo.toml): {versao}\n")

    problemas = conferir_correntes(versao)
    if problemas:
        print("== OS LUGARES CORRENTES NAO CONCORDAM ==")
        for p in problemas:
            print("  ", p)
    else:
        print(f"== os {len(CORRENTES)} lugares correntes concordam ==")
    for arq, _, porque in CORRENTES:
        print(f"   {arq:<16} {porque}")

    todas = ocorrencias(versao)
    correntes = {a for a, _, _ in CORRENTES}
    revisar = [(r, n, l) for r, n, l in todas if r not in correntes]
    print(f"\n== outras {len(revisar)} ocorrencias, em {len({r for r,_,_ in revisar})} arquivos ==")
    print("   Elas NAO se classificam sozinhas. A maioria e historia -- «desde a»,")
    print("   «ate a», «nasceu na» -- e historia reescrita vira mentira. Leia antes")
    print("   de trocar o numero:\n")
    atual = None
    for rel, n, linha in revisar:
        if rel != atual:
            print(f"   {rel}")
            atual = rel
        print(f"      {n:>6}  {linha[:96]}")

    if len(sys.argv) > 1 and re.fullmatch(r"\d+\.\d+\.\d+", sys.argv[1]):
        bump(versao, sys.argv[1], "--gravar" in sys.argv)
    return 1 if problemas else 0


if __name__ == "__main__":
    sys.exit(principal())
