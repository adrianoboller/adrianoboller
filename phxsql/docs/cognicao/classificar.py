#!/usr/bin/env python3
"""Estado de cada cognicao -- PENDENTE, FRUTIFERO ou INFRUTIFERO -- conferido.

Ordem do dono, 24/09/2026: *um aprendizado PENDENTE nao pode virar FRUTIFERO
automaticamente: precisa de evidencia validada. Falhas observadas entram como
INFRUTIFERO com causa/prevencao para alimentar avoid, enquanto sucessos
comprovados alimentam reuse.*

Este roteiro nunca ESCREVE o estado de uma cognicao -- quem escreve e quem tem
a evidencia na mao. Ele confere e recusa:

- sem secao `## Estado`, a cognicao e PENDENTE (e o padrao, nunca FRUTIFERO);
- FRUTIFERO exige `Evidência:` que CONFERE (arquivo que existe, `teste:nome`
  que existe no fonte, `commit:hash` que existe no git) e `Validado em:`;
- INFRUTIFERO exige a mesma evidencia da falha observada, `Causa:` e
  `Prevenção:`;
- qualquer evidencia citada que nao confere reprova, mesmo ao lado de outra
  que confere: evidencia falsa nao se dilui.

E gera, do que passou, as duas listas que se consultam antes de trabalhar:
`REUSAR.md` (sucessos comprovados) e `EVITAR.md` (falhas com causa e
prevencao). As duas sao geradas -- nao se editam.

    python3 phxsql/docs/cognicao/classificar.py            # confere e gera
    python3 phxsql/docs/cognicao/classificar.py --conferir # so confere
    python3 phxsql/docs/cognicao/classificar.py --rodar    # e roda os testes citados

Sai com 1 se alguma cognicao reprovar; as listas so se gravam sem reprovacao,
para nunca publicar um sucesso que nao se sustentou.
"""

import re
import subprocess
import sys
from pathlib import Path

AQUI = Path(__file__).resolve().parent
RAIZ = AQUI.parents[2]
ESTADOS = ("PENDENTE", "FRUTÍFERO", "INFRUTÍFERO")
# Aceita a grafia sem acento, que e a dos identificadores da casa.
SINONIMOS = {"FRUTIFERO": "FRUTÍFERO", "INFRUTIFERO": "INFRUTÍFERO"}
FORA = ("/target/", "/.claude/worktrees/", "/node_modules/")


def campos_do_estado(texto):
    """Le a secao `## Estado` (lista `- **Campo:** valor`). None se nao ha."""
    m = re.search(r"^## +Estado\s*$(.*?)(?=^## |\Z)", texto, re.M | re.S)
    if not m:
        return None
    campos = {}
    for linha in m.group(1).splitlines():
        c = re.match(r"^\s*[-*]\s*\*\*([^*:]+):\*\*\s*(.*)$", linha)
        if c:
            campos[c.group(1).strip().lower()] = c.group(2).strip()
    return campos


def secao(texto, palavra):
    """Primeiro paragrafo da secao cujo titulo contem `palavra`."""
    m = re.search(rf"^## [^\n]*{palavra}[^\n]*$(.*?)(?=^## |\Z)", texto, re.M | re.S | re.I)
    if not m:
        return ""
    for par in re.split(r"\n\s*\n", m.group(1).strip()):
        if par.strip():
            return " ".join(par.split())
    return ""


_fontes_rs = None


def fontes_rs():
    global _fontes_rs
    if _fontes_rs is None:
        _fontes_rs = [
            # O filtro olha o caminho RELATIVO a raiz: rodado de dentro de uma
            # copia em .claude/worktrees/, o absoluto casaria sempre e nenhum
            # teste seria achado.
            p for p in RAIZ.rglob("*.rs")
            if not any(f in "/" + str(p.relative_to(RAIZ)) for f in FORA)
        ]
    return _fontes_rs


def achar_teste(nome):
    """Arquivo .rs que declara `fn nome`, ou None."""
    padrao = re.compile(rf"\bfn\s+{re.escape(nome)}\s*\(")
    for p in fontes_rs():
        try:
            if padrao.search(p.read_text(errors="ignore")):
                return p
        except OSError:
            pass
    return None


def conferir_evidencia(item, rodar):
    """(ok, descricao). Uma evidencia so conta se se confere daqui."""
    if item.startswith("teste:"):
        nome = item[len("teste:"):]
        arq = achar_teste(nome)
        if not arq:
            return False, f"teste `{nome}` não existe no fonte"
        if rodar:
            caixa = next(
                (d for d in arq.parents if (d / "Cargo.toml").exists()), None
            )
            r = subprocess.run(
                ["cargo", "test", "-q", nome],
                cwd=caixa, capture_output=True, text=True,
            )
            passou = re.search(r"test result: ok\. [1-9]\d* passed", r.stdout)
            if r.returncode != 0 or not passou:
                return False, f"teste `{nome}` não passou (ou não rodou)"
        return True, f"teste `{nome}` ({arq.relative_to(RAIZ)})"
    if item.startswith("commit:"):
        h = item[len("commit:"):]
        r = subprocess.run(
            ["git", "cat-file", "-e", f"{h}^{{commit}}"], cwd=RAIZ,
            capture_output=True,
        )
        if r.returncode != 0:
            return False, f"commit `{h}` não existe"
        return True, f"commit `{h}`"
    p = RAIZ / item
    if not p.exists():
        return False, f"`{item}` não existe"
    return True, f"`{item}`"


def avaliar(arq, rodar):
    texto = arq.read_text()
    titulo = texto.splitlines()[0].lstrip("# ").strip() if texto else arq.stem
    campos = campos_do_estado(texto)
    r = {"arq": arq, "titulo": titulo, "estado": "PENDENTE", "erros": [],
         "evidencias": [], "regra": secao(texto, "regra"), "campos": {}}
    if campos is None:
        return r
    r["campos"] = campos
    bruto = campos.get("estado", "").split()[0].upper() if campos.get("estado") else ""
    estado = SINONIMOS.get(bruto, bruto)
    if estado not in ESTADOS:
        r["erros"].append(f"estado «{campos.get('estado', '')}» fora de {ESTADOS}")
        return r
    r["estado"] = estado
    itens = re.findall(r"`([^`]+)`", campos.get("evidência", campos.get("evidencia", "")))
    for item in itens:
        ok, desc = conferir_evidencia(item, rodar)
        (r["evidencias"] if ok else r["erros"]).append(desc)
    if estado == "PENDENTE":
        return r
    if not itens:
        r["erros"].append(f"{estado} sem evidência (`caminho`, `teste:nome` ou `commit:hash`)")
    if estado == "FRUTÍFERO" and not campos.get("validado em"):
        r["erros"].append("FRUTÍFERO sem «Validado em»")
    if estado == "INFRUTÍFERO":
        for c in ("causa", "prevenção"):
            if not campos.get(c) and not campos.get(c.replace("ç", "c").replace("ã", "a")):
                r["erros"].append(f"INFRUTÍFERO sem «{c.capitalize()}»")
    return r


CABECALHO = (
    "<!-- GERADO por classificar.py a partir das cognições. Não se edita: "
    "mude a seção «## Estado» da cognição e rode o gerador. -->\n\n"
)


def link(r):
    return f"[{r['titulo']}]({r['arq'].name})"


def gerar(avaliadas):
    fr = [r for r in avaliadas if r["estado"] == "FRUTÍFERO"]
    inf = [r for r in avaliadas if r["estado"] == "INFRUTÍFERO"]
    pend = len(avaliadas) - len(fr) - len(inf)
    placar = (f"{len(avaliadas)} cognições: **{len(fr)} frutíferas**, "
              f"**{len(inf)} infrutíferas**, **{pend} pendentes** "
              f"(sem evidência validada — não entram aqui).\n\n")
    reusar = [CABECALHO, "# Reusar — sucessos comprovados\n\n", placar,
              "Só entra o que tem evidência que o `classificar.py` conferiu. "
              "Antes de desenhar, procure aqui o que já se provou.\n\n"]
    for r in sorted(fr, key=lambda x: x["arq"].name, reverse=True):
        reusar.append(f"## {link(r)}\n\n")
        if r["regra"]:
            reusar.append(f"{r['regra']}\n\n")
        reusar.append(f"- Evidência: {'; '.join(r['evidencias'])}\n")
        reusar.append(f"- Validado em: {r['campos'].get('validado em', '')}\n\n")
    evitar = [CABECALHO, "# Evitar — falhas observadas, com causa e prevenção\n\n",
              placar, "Antes de repetir um caminho, procure aqui se ele já "
              "falhou e o que o previne.\n\n"]
    for r in sorted(inf, key=lambda x: x["arq"].name, reverse=True):
        c = r["campos"]
        evitar.append(f"## {link(r)}\n\n")
        evitar.append(f"- **Causa:** {c.get('causa', '')}\n")
        evitar.append(f"- **Prevenção:** {c.get('prevenção', c.get('prevencao', ''))}\n")
        evitar.append(f"- Evidência: {'; '.join(r['evidencias'])}\n\n")
    (AQUI / "REUSAR.md").write_text("".join(reusar))
    (AQUI / "EVITAR.md").write_text("".join(evitar))
    return len(fr), len(inf), pend


def main():
    rodar = "--rodar" in sys.argv
    avaliadas = [avaliar(a, rodar) for a in sorted(AQUI.glob("cognicao_*.md"))]
    ruins = [r for r in avaliadas if r["erros"]]
    for r in ruins:
        print(f"REPROVADA {r['arq'].name} ({r['estado']}):")
        for e in r["erros"]:
            print(f"  - {e}")
    if ruins:
        print(f"{len(ruins)} cognição(ões) reprovada(s); REUSAR.md e EVITAR.md "
              "não foram regravados.")
        sys.exit(1)
    if "--conferir" in sys.argv:
        n = {e: sum(r["estado"] == e for r in avaliadas) for e in ESTADOS}
        print(f"ok: {len(avaliadas)} cognições, {n}")
        return
    fr, inf, pend = gerar(avaliadas)
    print(f"ok: {len(avaliadas)} cognições -> {fr} frutíferas (REUSAR.md), "
          f"{inf} infrutíferas (EVITAR.md), {pend} pendentes")


if __name__ == "__main__":
    main()
