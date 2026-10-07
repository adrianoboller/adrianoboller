#!/usr/bin/env python3
"""A guarda da regra «trabalho de frente nunca mora so numa pasta» -- pedido 655.

    python3 bancada/guardas/frente-so-numa-pasta.py --catraca
    python3 bancada/guardas/frente-so-numa-pasta.py --numeros
    python3 bancada/guardas/frente-so-numa-pasta.py --autoteste

# O defeito que motivou

Em 01/10/2026 uma limpeza apagou quatro frentes prontas: a conferencia dizia
«integrada» porque o ramo da frente era ancestral do HEAD -- e frente nao
comita, entao o ramo dela e sempre a base -- e a remocao usava `git worktree
remove -f`, que pula justamente a recusa do git a apagar copia suja. Ordem do
dono: *«Isso nao pode acontecer.»* Nasceram o `salvar-frentes.sh` e o
`limpar-frentes.sh` com tres provas; o que NAO nasceu foi quem falhe se alguem
desfizer isso. A auditoria G de 02/10 (pedido 655) contou: nada reprovava
reintroduzir o `-f` nem limpar com a arvore suja.

# Por que aqui, e nao no catalogo

O catalogo prova defeito reposto em Rust, rodando `cargo test`; um `.sh` nao
tem binario de teste. E o que depende do sistema operacional se prova contra o
sistema operacional: esta guarda monta um repositorio git de verdade numa
pasta temporaria, com tres copias de frente (suja, limpa-integrada, limpa com
commit proprio), roda o `limpar-frentes.sh` DE VERDADE e confere o que sobrou.

# As duas catracas, as duas em zero

* `TETO_REMOVE_FORCADO` -- `worktree remove` com `-f`/`--force` em qualquer
  `.sh`/`.py` de `phxsql/` (fora de comentario). Texto, porque o `-f` sozinho
  NAO aparece no comportamento enquanto a prova do `status --porcelain`
  estiver de pe: sao duas cercas, e cada uma precisa da sua guarda.
* `TETO_FRENTE_SUJA_APAGADA` -- copias que a corrida real do script apagou
  e nao podia: a suja, ou a que tem commit fora do HEAD. E mais o controle:
  a limpa e integrada TEM de sair, senao um script que nunca apaga nada
  passaria aqui por engano -- e isso conta como 1 tambem. E soma cada uma das
  cinco provas (`PROVAS`) que sair do texto antes do `worktree remove`: sem a
  do `status --porcelain` o git ainda segura a suja, e por isso so o texto ve.
"""
import os
import re
import shutil
import subprocess
import sys
import tempfile

AQUI = os.path.dirname(os.path.abspath(__file__))
RAIZ = os.path.abspath(os.path.join(AQUI, "..", ".."))
EU = os.path.relpath(os.path.abspath(__file__), RAIZ)

TETO_REMOVE_FORCADO = 0
TETO_FRENTE_SUJA_APAGADA = 0

FORCADO = re.compile(r"worktree\W+remove\b[^\n#]*?(?:\s|['\",])(-f|--force)\b")


def forcados(arquivos=None):
    """(arquivo, linha, texto) de cada `worktree remove -f` fora de comentario."""
    achados = []
    if arquivos is None:
        arquivos = {}
        for dirpath, dirs, nomes in os.walk(RAIZ):
            dirs[:] = [d for d in dirs if d not in ("target", ".git", "__pycache__",
                                                     "node_modules")]
            for n in nomes:
                if n.endswith((".sh", ".py")):
                    c = os.path.join(dirpath, n)
                    if os.path.abspath(c) == os.path.abspath(__file__):
                        continue  # o crivo escreve o padrao para descreve-lo
                    try:
                        with open(c, encoding="utf-8", errors="replace") as f:
                            arquivos[os.path.relpath(c, RAIZ)] = f.read()
                    except OSError:
                        pass
    for rel, texto in arquivos.items():
        for i, linha in enumerate(texto.splitlines(), 1):
            if linha.lstrip().startswith("#"):
                continue
            if FORCADO.search(linha):
                achados.append((rel, i, linha.strip()))
    return achados


# Identidade propria e configuracao global desligada: a prova nao pode
# depender do `~/.gitconfig` de quem roda (sem ele, o `commit-tree` do
# `salvar-frentes.sh` recusa, e o script -- certo -- nao apaga nada).
AMBIENTE_GIT = dict(os.environ, GIT_AUTHOR_NAME="guarda", GIT_AUTHOR_EMAIL="g@x",
                    GIT_COMMITTER_NAME="guarda", GIT_COMMITTER_EMAIL="g@x",
                    GIT_CONFIG_GLOBAL="/dev/null", GIT_CONFIG_SYSTEM="/dev/null")


def git(cwd, *args):
    r = subprocess.run(["git", "-C", cwd, *args], capture_output=True, text=True,
                       env=AMBIENTE_GIT)
    if r.returncode != 0:
        raise RuntimeError("git %s: %s" % (" ".join(args), r.stderr.strip()))
    return r.stdout


def correr(limpar_texto, salvar_texto):
    """Monta o repositorio, roda o script, devolve o que aconteceu.

    Devolve dict com: `sobrou` (quais copias continuam), `salvas` (refs em
    `refs/salvas/`), `saida` do script. Tudo numa pasta temporaria: nada do
    repositorio de verdade e tocado."""
    tmp = tempfile.mkdtemp(prefix="guarda-frentes-")
    try:
        repo = os.path.join(tmp, "repo")
        os.makedirs(repo)
        git(repo, "init", "-q", "-b", "base")
        with open(os.path.join(repo, "a.txt"), "w") as f:
            f.write("base\n")
        git(repo, "add", "a.txt")
        git(repo, "commit", "-q", "-m", "base")
        for nome, texto in (("limpar-frentes.sh", limpar_texto),
                            ("salvar-frentes.sh", salvar_texto)):
            c = os.path.join(repo, nome)
            with open(c, "w") as f:
                f.write(texto)
            os.chmod(c, 0o755)
        for w in ("suja", "limpa", "propria"):
            git(repo, "worktree", "add", "-q", "-b", "worktree-agent-" + w,
                ".claude/worktrees/agent-" + w, "HEAD")
        suja = os.path.join(repo, ".claude/worktrees/agent-suja")
        with open(os.path.join(suja, "a.txt"), "w") as f:
            f.write("trabalho da frente que ninguem comitou\n")
        propria = os.path.join(repo, ".claude/worktrees/agent-propria")
        with open(os.path.join(propria, "b.txt"), "w") as f:
            f.write("x\n")
        git(propria, "add", "b.txt")
        git(propria, "commit", "-q", "-m", "commit da frente")
        r = subprocess.run(["sh", os.path.join(repo, "limpar-frentes.sh")],
                           cwd=repo, capture_output=True, text=True, timeout=120,
                           env=AMBIENTE_GIT)
        sobrou = {w for w in ("suja", "limpa", "propria")
                  if os.path.isdir(os.path.join(repo, ".claude/worktrees/agent-" + w))}
        salvas = git(repo, "for-each-ref", "--format=%(refname)", "refs/salvas/")
        return {"sobrou": sobrou, "salvas": salvas.split(), "saida": r.stdout + r.stderr}
    finally:
        shutil.rmtree(tmp, ignore_errors=True)


def julgar_corrida(c):
    """Quantas copias foram tratadas errado (o numero da catraca), e por que."""
    erros = []
    if "suja" not in c["sobrou"]:
        erros.append("a copia SUJA foi apagada")
    if "propria" not in c["sobrou"]:
        erros.append("a copia com commit fora do HEAD foi apagada")
    if "limpa" in c["sobrou"]:
        erros.append("controle: a limpa e integrada NAO saiu -- um script que "
                     "nunca apaga passaria aqui por engano")
    if not any("agent-suja" in r for r in c["salvas"]):
        erros.append("o trabalho da suja nao foi salvo em refs/salvas/")
    return erros


def ler(rel):
    with open(os.path.join(RAIZ, rel), encoding="utf-8") as f:
        return f.read()


# As provas que o script tem de fazer ANTES do `worktree remove`, pelo texto.
# A corrida real nao ve a falta de uma delas enquanto outra cerca segurar: sem
# a do `status --porcelain`, o proprio git ainda recusa a copia suja -- ate o
# dia em que alguem por o `-f`. Cerca dupla so e dupla se as duas forem
# guardadas.
PROVAS = [
    ("status --porcelain", "a prova da arvore suja"),
    ("merge-base --is-ancestor", "a prova de a ponta estar no HEAD"),
    ("/proc/", "a prova de nenhum processo usar a copia"),
    ("locked", "a prova da tranca do agente"),
    ("salvar-frentes.sh", "o salvar antes de tudo"),
]


def provas_ausentes(texto):
    """As provas que nao aparecem fora de comentario ANTES do remove."""
    util = "\n".join(l for l in texto.splitlines() if not l.lstrip().startswith("#"))
    corte = util.find("worktree remove")
    antes = util if corte < 0 else util[:corte]
    return ["%s saiu do script (ou foi para depois do remove)" % porque
            for marca, porque in PROVAS if marca not in antes]


def medir():
    limpar = ler("limpar-frentes.sh")
    c = correr(limpar, ler("salvar-frentes.sh"))
    return forcados(), julgar_corrida(c) + provas_ausentes(limpar), c


def catraca():
    print("=== a guarda das copias de frente (pedido 655) ===")
    f, erros, c = medir()
    ruim = 0
    for rel, i, linha in f:
        print(f"      {rel}:{i}  {linha}")
    for nome, n, valor in (("TETO_REMOVE_FORCADO", len(f), TETO_REMOVE_FORCADO),
                           ("TETO_FRENTE_SUJA_APAGADA", len(erros),
                            TETO_FRENTE_SUJA_APAGADA)):
        if n != valor:
            print(f"   {'SUBIU' if n > valor else 'DESCEU'}  {nome}: {n} (teto {valor})")
            ruim = 1
        else:
            print(f"   ok  {nome}: {n} (teto {valor})")
    for e in erros:
        print(f"      {e}")
    if ruim:
        print("   Reprovado: trabalho de frente nunca mora so numa pasta. Sem "
              "`-f`, e com as tres provas antes do `worktree remove` "
              "(docs/cognicao/cognicao_ancestral-do-head-nao-prova-que-o-"
              "trabalho-foi-integrado_20261001_0500.md).")
        print("   saida do script na corrida de prova:")
        for linha in c["saida"].splitlines()[-8:]:
            print("      " + linha)
    return ruim


def numeros():
    f, erros, _c = medir()
    print(f"catraca:nome=TETO_REMOVE_FORCADO;onde={EU};valor={TETO_REMOVE_FORCADO};"
          f"medido={len(f)};tipo=teto;mede=worktree remove com -f fora de comentario")
    print(f"catraca:nome=TETO_FRENTE_SUJA_APAGADA;onde={EU};"
          f"valor={TETO_FRENTE_SUJA_APAGADA};medido={len(erros)};tipo=teto;"
          f"mede=copias de frente tratadas errado pela corrida real do limpar-frentes.sh, mais provas ausentes do texto dele")
    return 0


def autoteste():
    """O defeito de 01/10 reposto no script de verdade, rodado de verdade."""
    falhas = []

    def conferir(nome, cond, detalhe=""):
        print("   %s  %s%s" % ("ok  " if cond else "FALHOU", nome,
                               "" if cond else "  -- " + detalhe))
        if not cond:
            falhas.append(nome)

    limpar, salvar = ler("limpar-frentes.sh"), ler("salvar-frentes.sh")
    c = correr(limpar, salvar)
    conferir("o script de hoje: a suja e a propria ficam, a limpa sai, a suja e salva",
             julgar_corrida(c) == [], str(julgar_corrida(c)) + c["saida"])

    linha_suja = next(l for l in limpar.splitlines() if "status --porcelain" in l
                      and not l.lstrip().startswith("#"))
    defeito = (limpar.replace(linha_suja + "\n", "", 1)
               .replace('git worktree remove "$W"', 'git worktree remove -f "$W"', 1))
    conferir("o recorte achou as duas cercas", defeito != limpar
             and "remove -f" in defeito and linha_suja not in defeito)
    c = correr(defeito, salvar)
    e = julgar_corrida(c)
    conferir("defeito de 01/10 reposto (sem a prova da arvore suja E com -f): "
             "a suja e APAGADA e a guarda acusa",
             "a copia SUJA foi apagada" in e, str(e))
    conferir("o -f sozinho a regua de texto acusa",
             len(forcados({"x.sh": defeito})) == 1)
    conferir("o -f em comentario nao conta",
             forcados({"x.sh": "# nunca `git worktree remove -f`\n"}) == [])
    conferir("o -f numa lista de argv do Python conta",
             len(forcados({"x.py": 'run(["git", "worktree", "remove", "--force", w])'})) == 1)
    nunca = limpar.replace('git worktree remove "$W"', 'true', 1)
    c = correr(nunca, salvar)
    conferir("controle: script que nunca apaga e acusado (a limpa nao saiu)",
             any("controle" in x for x in julgar_corrida(c)), str(julgar_corrida(c)))
    conferir("o texto de hoje tem as cinco provas antes do remove",
             provas_ausentes(limpar) == [], str(provas_ausentes(limpar)))
    so_porcelain = limpar.replace(linha_suja + "\n", "", 1)
    conferir("so a prova da arvore suja removida: o git ainda segura a suja...",
             "suja" in correr(so_porcelain, salvar)["sobrou"])
    conferir("...e por isso a regua de texto a acusa sozinha",
             provas_ausentes(so_porcelain) == ["a prova da arvore suja saiu do "
                                               "script (ou foi para depois do remove)"],
             str(provas_ausentes(so_porcelain)))
    sem_salvar = limpar.replace('"$AQUI/salvar-frentes.sh" >/dev/null ||', 'true ||', 1)
    c = correr(sem_salvar, salvar)
    conferir("sem chamar o salvar-frentes antes: acusa o trabalho nao salvo",
             any("salvo" in x for x in julgar_corrida(c)), str(julgar_corrida(c)))
    print("   %s" % ("todos passaram" if not falhas
                     else "FALHOU: " + ", ".join(falhas)))
    return 1 if falhas else 0


def principal():
    if "--autoteste" in sys.argv:
        print("=== autoteste da guarda das copias de frente (pedido 655) ===")
        return autoteste()
    if "--catraca" in sys.argv:
        return catraca()
    if "--numeros" in sys.argv:
        return numeros()
    return catraca()


if __name__ == "__main__":
    sys.exit(principal())
