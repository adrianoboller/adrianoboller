#!/usr/bin/env python3
"""Compila o wx-modelos para Linux e Windows e carimba o que saiu.

Existe porque binario commitado envelhece calado: daqui a tres versoes o .exe
do repositorio mostra numeros de um codigo que ja mudou e ninguem percebe. Aqui
nada fica na arvore -- os binarios vao para `dist/`, que o git ignora, e cada
publicacao gera a ficha com versao, alvo, SHA-256, tamanho e o rustc que
compilou. Quem receber o arquivo confere o hash contra a ficha.

O hash identifica o arquivo entregue, e nao afirma build reprodutivel: medido,
recompilar o mesmo codigo muda o hash do `.exe`, porque o PE carrega carimbo de
tempo. Dizer "reprodutivel" aqui seria afirmar o que nao se mediu.

Tres recusas, todas de proposito:

  1. Teste vermelho nao vira binario. `cargo test` roda ANTES, e falhando aqui
     nao se publica nada -- publicar o que nao passa e como esqueleto de teste
     que passa: some do relatorio sem provar nada.
  2. Falta de ferramenta NAO e falha de compilacao: o alvo sem `rustup target`
     ou sem o mingw sai como PULADO, com o comando que resolve. Publicar um
     alvo a menos e dizer isso e melhor que quebrar a publicacao inteira.
  3. O .exe nao pode depender de DLL que nao seja do Windows. Isso e conferido
     LENDO o binario: se `libgcc_s` ou `libwinpthread` aparecer, a publicacao
     falha -- o cliente copiaria um arquivo que nao roda na maquina dele.

Uso:
  publicar.py                 # os dois alvos
  publicar.py --alvo linux    # so um
  publicar.py --sem-teste     # pula a bateria (nao use para entrega)
"""
from __future__ import annotations

import argparse
import hashlib
import json
import re
import shutil
import subprocess
import sys
from datetime import date
from pathlib import Path

AQUI = Path(__file__).resolve().parent
DIST = AQUI / "dist"

# (apelido, alvo do rustc, nome do arquivo, ferramenta de fora, como instalar)
ALVOS = [
    ("linux", "x86_64-unknown-linux-gnu", "wx-modelos", None, ""),
    ("windows", "x86_64-pc-windows-gnu", "wx-modelos.exe", "x86_64-w64-mingw32-gcc",
     "apt-get install mingw-w64  (ou o equivalente da sua distribuicao)"),
]
# DLL que so existe se o binario ficou preso ao compilador: o cliente nao a tem.
DLL_PROIBIDA = re.compile(rb"lib(gcc_s|winpthread|stdc\+\+)[^\x00]*\.dll", re.I)


def rodar(cmd: list[str], **kw) -> subprocess.CompletedProcess:
    """Ferramenta ausente vira codigo 127, nao traceback.

    Achado provando a recusa 2: num PATH sem `rustc` o script estourava
    FileNotFoundError -- justamente na maquina de quem ainda nao instalou o
    Rust, que e quem mais precisa da mensagem dizendo o que falta.
    """
    try:
        return subprocess.run(cmd, capture_output=True, text=True, cwd=AQUI, **kw)
    except (FileNotFoundError, PermissionError) as e:
        return subprocess.CompletedProcess(cmd, 127, "", f"{cmd[0]}: {e}")


def versao() -> str:
    m = re.search(r'^version\s*=\s*"([^"]+)"', (AQUI / "Cargo.toml").read_text(encoding="utf-8"), re.M)
    if not m:
        raise SystemExit("Cargo.toml sem version")
    return m.group(1)


def sha256(p: Path) -> str:
    h = hashlib.sha256()
    with p.open("rb") as f:
        for b in iter(lambda: f.read(1 << 20), b""):
            h.update(b)
    return h.hexdigest()


def alvo_instalado(alvo: str) -> bool:
    r = rodar(["rustup", "target", "list", "--installed"])
    return r.returncode == 0 and alvo in r.stdout.split()


def conferir_windows(exe: Path) -> list[str]:
    """As DLLs de fora do Windows que o binario carrega -- lidas do arquivo."""
    return sorted({m.group(0).decode(errors="replace") for m in DLL_PROIBIDA.finditer(exe.read_bytes())})


def main() -> int:
    p = argparse.ArgumentParser(description="publica o wx-modelos")
    p.add_argument("--alvo", choices=[a[0] for a in ALVOS], action="append")
    p.add_argument("--sem-teste", action="store_true")
    args = p.parse_args()
    escolhidos = [a for a in ALVOS if not args.alvo or a[0] in args.alvo]
    v = versao()

    if not shutil.which("cargo"):
        print("cargo não está no PATH. Instale o Rust (https://rustup.rs) e rode de novo.",
              file=sys.stderr)
        return 2
    if args.sem_teste:
        print("AVISO: bateria pulada; isto não serve para entrega.\n")
    else:
        print("cargo test …", end=" ", flush=True)
        t = rodar(["cargo", "test", "--offline", "--quiet"])
        if t.returncode != 0:
            print("FALHOU\n")
            print(t.stdout[-2000:] or t.stderr[-2000:])
            print("teste vermelho não vira binário; nada foi publicado.")
            return 1
        print("ok")

    rustc = (rodar(["rustc", "--version"]).stdout or "").strip() or "INDISPONÍVEL"
    DIST.mkdir(exist_ok=True)
    ficha = {"programa": "wx-modelos", "versao": v, "publicado_em": date.today().isoformat(),
             "rustc": rustc, "binarios": [], "pulados": []}

    for apelido, alvo, nome, ferramenta, como in escolhidos:
        if not alvo_instalado(alvo):
            ficha["pulados"].append({"alvo": apelido, "porque": f"alvo {alvo} não instalado",
                                     "resolve": f"rustup target add {alvo}"})
            print(f"{apelido:<8} PULADO — rustup target add {alvo}")
            continue
        if ferramenta and not shutil.which(ferramenta):
            ficha["pulados"].append({"alvo": apelido, "porque": f"{ferramenta} não está no PATH",
                                     "resolve": como})
            print(f"{apelido:<8} PULADO — falta {ferramenta}: {como}")
            continue
        b = rodar(["cargo", "build", "--release", "--target", alvo])
        if b.returncode != 0:
            print(f"{apelido:<8} FALHOU na compilação\n{b.stderr[-1500:]}")
            return 1
        origem = AQUI / "target" / alvo / "release" / nome
        if apelido == "windows":
            presas = conferir_windows(origem)
            if presas:
                print(f"{apelido:<8} FALHOU: o .exe depende de {', '.join(presas)} — "
                      "o cliente não tem essas DLLs")
                return 1
        destino = DIST / f"wx-modelos-{v}-{apelido}-x86_64{'.exe' if nome.endswith('.exe') else ''}"
        shutil.copy2(origem, destino)
        item = {"alvo": apelido, "rustc_target": alvo, "arquivo": destino.name,
                "bytes": destino.stat().st_size, "sha256": sha256(destino)}
        ficha["binarios"].append(item)
        print(f"{apelido:<8} {destino.name}  {item['bytes']:>9,} bytes  {item['sha256'][:16]}…")

    (DIST / "entrega.json").write_text(json.dumps(ficha, ensure_ascii=False, indent=2) + "\n",
                                       encoding="utf-8")
    linhas = [f"# wx-modelos {v}", "", f"Publicado em {ficha['publicado_em']} com {rustc}.", "",
              "| alvo | arquivo | bytes | SHA-256 |", "| --- | --- | --- | --- |"]
    linhas += [f"| {b['alvo']} | `{b['arquivo']}` | {b['bytes']:,} | `{b['sha256']}` |"
               for b in ficha["binarios"]]
    if ficha["pulados"]:
        linhas += ["", "## Alvos pulados", ""]
        linhas += [f"- **{x['alvo']}**: {x['porque']} — `{x['resolve']}`" for x in ficha["pulados"]]
    linhas += ["", "Confira o arquivo recebido contra o hash acima antes de rodar.", "",
               "O hash identifica **este** arquivo, e não é prova de compilação",
               "reprodutível: medido aqui, recompilar o mesmo código dá outro hash no",
               "`.exe`, porque o formato PE carrega carimbo de tempo. Para conferir uma",
               "entrega, compare com a ficha que veio junto dela — não com uma compilação",
               "sua.", ""]
    (DIST / "ENTREGA.md").write_text("\n".join(linhas), encoding="utf-8")
    print(f"\nficha em {DIST / 'ENTREGA.md'}")
    return 0 if ficha["binarios"] else 1


if __name__ == "__main__":
    sys.exit(main())
