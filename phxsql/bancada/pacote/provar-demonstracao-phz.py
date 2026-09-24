#!/usr/bin/env python3
"""O pacote de demonstracao tem de sair em `.phz`, nao em `config.json` claro.

# Por que ela existe

Pedido 478: a ordem do dono para o pedido 450 era que a instalacao NOVA
terminasse em `.phz`, e o `empacotar.sh` gravava o config de demonstracao (com
o token e o hash da senha "demo") direto em `config.json`, em claro
(`empacotar.sh:319` antes desta correcao) -- o contrario do que foi pedido.

# O que ela prova, nos dois sentidos

1. a receita de HOJE (`./empacotar.sh demonstracao <dir>`) fecha com
   `demonstracao/config.phz` -- 0600, um 7z de verdade -- e SEM
   `config.json` nem sobra de guarda (`.migrado-para-phz`); o servidor sobe
   dele; o `COMECE-AQUI.txt` ensina `--desempacotar-config`/`--empacotar-config`
   e nao chama o `.phz` de cifra;
2. a receita de ANTES do pedido 478, reposta aqui POR VALOR -- o
   `empacotar.sh` de HOJE ja nao a tem, entao depois que este pedido for
   integrado nao ha mais como le-la do arquivo -- fecha com `config.json`
   em CLARO, a assinatura do defeito, e sem `.phz` nenhum.

O ponto 2 e o que impede esta prova de passar por engano: sem ele, um
`demonstracao()` que sempre deixasse tudo em claro (ou que quebrasse por
outro motivo) passaria pelo item 1 do mesmo jeito.

# Como ela roda

    python3 bancada/pacote/provar-demonstracao-phz.py

Usa o `phxsqld` de `target/release/` -- compile antes se ele faltar:
`cargo build --release --offline -p phxsql-server --bin phxsqld`. Builda
`phxsql-cli` sozinha (e' o que `demonstracao()` ja faz, rapido com cache).
Sobe o servidor por ate 10 s para a prova de fogo e mata no fim; nao mexe em
`pacotes/`.
"""

import pathlib
import re
import shutil
import stat
import subprocess
import sys
import tempfile
import time

RAIZ = pathlib.Path(__file__).resolve().parents[2]
EMPACOTAR = RAIZ / "empacotar.sh"
PHXSQLD = RAIZ / "target" / "release" / "phxsqld"
ASSINATURA_7Z = b"7z\xbc\xaf\x27\x1c"

falhas = []


def confere(rotulo, cond, detalhe=""):
    print(("  ok    " if cond else "  FALHA ") + rotulo
          + (f"  -> {detalhe}" if not cond and detalhe else ""))
    if not cond:
        falhas.append(rotulo)


# A receita de ANTES do pedido 478 (empacotar.sh, demonstracao(), ate o
# commit que este pedido substitui): grava config.json e PARA -- sem
# empacotar. Reduzida ao que a prova precisa, a FORMA do arquivo final: o
# COMECE-AQUI completo e os outros campos do config nao mudam essa forma, e
# guardar so a forma (nao o arquivo inteiro do `git log`) e o que sobrevive
# depois que este pedido for integrado e o `empacotar.sh` de HEAD deixar de
# ter esta receita.
DEMONSTRACAO_ANTES_478 = r"""
set -euo pipefail
dir=$1; phxsqld=$2
mkdir -p "$dir/demonstracao"
hash=$(echo "demo" | "$phxsqld" --senha | sed 's/.*: "//;s/"//')
cat > "$dir/demonstracao/config.json" <<JSON
{
  "bind": "127.0.0.1:5000",
  "base": "dados",
  "token": "demo",
  "web": { "ligado": true, "bind": "127.0.0.1:8080", "sessao_minutos": 60 },
  "usuarios": [
    {"login": "adm", "nome": "Administrador da demonstracao", "id": 1,
     "senha_hash": "$hash", "supervisor": true}
  ]
}
JSON
"""


def demonstracao_do_defeito(dir_):
    subprocess.run(
        ["bash", "-c", DEMONSTRACAO_ANTES_478, "_", str(dir_), str(PHXSQLD)],
        check=True, capture_output=True, text=True)


def demonstracao_de_hoje(dir_):
    return subprocess.run(
        [str(EMPACOTAR), "demonstracao", str(dir_), "linux", ""],
        capture_output=True, text=True, cwd=RAIZ)


def sobe_e_mata(config_dir):
    """Sobe o phxsqld em `config_dir` (o `--config config.json` padrao, que
    resolve para o `.phz` quando so ele existe) e confere que a porta de
    dados abriu, matando o processo em seguida."""
    erro = config_dir / "stderr-prova.txt"
    with open(erro, "w") as ferro:
        p = subprocess.Popen([str(PHXSQLD)], cwd=config_dir,
                              stdout=subprocess.DEVNULL, stderr=ferro)
    try:
        ate = time.time() + 10
        while time.time() < ate:
            texto = erro.read_text() if erro.exists() else ""
            if "porta de dados escutando em" in texto:
                return True, texto
            if p.poll() is not None:
                return False, texto
            time.sleep(0.1)
        return False, erro.read_text() if erro.exists() else ""
    finally:
        p.kill()
        p.wait()


def main():
    if not PHXSQLD.exists():
        print(f"binario ausente: {PHXSQLD}\n"
              "  cargo build --release --offline -p phxsql-server --bin phxsqld")
        return 2

    print("== o pacote de demonstracao sai em .phz? ==")
    tmp = pathlib.Path(tempfile.mkdtemp(prefix="phx-demo-phz-"))
    try:
        # 1. a receita de hoje
        hoje = tmp / "hoje"
        r = demonstracao_de_hoje(hoje)
        confere("./empacotar.sh demonstracao sai com sucesso",
                r.returncode == 0, r.stdout + r.stderr)
        demo = hoje / "demonstracao"
        phz = demo / "config.phz"
        confere("nasce demonstracao/config.phz", phz.exists())
        confere("NAO sobra demonstracao/config.json",
                not (demo / "config.json").exists())
        confere("NAO sobra a copia de guarda (.migrado-para-phz)",
                not any(demo.glob("*.migrado-para-phz")))
        if phz.exists():
            confere("config.phz e um 7z de verdade",
                    phz.read_bytes()[:6] == ASSINATURA_7Z)
            modo = stat.S_IMODE(phz.stat().st_mode)
            confere("config.phz nasce 0600", modo == 0o600, oct(modo))
            subiu, saida = sobe_e_mata(demo)
            confere("o servidor sobe do config.phz (mesmo --config de sempre)",
                    subiu, saida[-300:])
        comece = (hoje / "COMECE-AQUI.txt").read_text()
        confere("COMECE-AQUI ensina --desempacotar-config",
                "--desempacotar-config" in comece)
        confere("COMECE-AQUI ensina --empacotar-config (do config da demo E "
                "do roteiro de um servidor de verdade)",
                comece.count("--empacotar-config") >= 2)
        # A frase quebra linha no .txt ("...NAO E\nCIFRA:..."), entao o crivo
        # ignora espaco (inclusive a quebra) em vez de exigir contiguidade.
        confere("COMECE-AQUI nao chama o .phz de cifra",
                re.search(r"NAO\s+(E\s+)?CIFRA", comece) is not None)

        # 2. a receita de ANTES do 478 -- a assinatura do defeito
        antes = tmp / "antes"
        demonstracao_do_defeito(antes)
        claro = antes / "demonstracao" / "config.json"
        confere("a receita de ANTES do 478 deixa config.json em CLARO",
                claro.exists() and "\"token\": \"demo\"" in claro.read_text())
        confere("e a receita de antes NAO produz .phz nenhum",
                not (antes / "demonstracao" / "config.phz").exists())

        # 3. o MANUAL e o README tambem terminam o roteiro em .phz (item 1
        # do pedido 478) -- as duas cartas que um administrador realmente le
        # antes de instalar um servidor de verdade, e nao so' o COMECE-AQUI
        # do pacote de demonstracao.
        manual = (RAIZ / "MANUAL.txt").read_text()
        m73 = manual.split("7.3 ", 1)[1].split("\n7.4 ", 1)[0]
        confere("MANUAL.txt 7.3 manda empacotar antes de subir",
                "--empacotar-config" in m73)
        leiame = (RAIZ / "README.md").read_text()
        confere("README.md manda empacotar no roteiro do servidor",
                "--empacotar-config" in leiame)
    finally:
        shutil.rmtree(tmp, ignore_errors=True)

    print()
    if falhas:
        print("REPROVOU em %d: %s" % (len(falhas), "; ".join(falhas)))
        return 1
    print("o pacote de demonstracao sai em .phz, e a receita de antes reprova")
    return 0


if __name__ == "__main__":
    sys.exit(main())
