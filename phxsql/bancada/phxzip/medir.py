#!/usr/bin/env python3
"""A bancada do PhxZip contra o 7-Zip do sistema (`/usr/bin/7z`) -- fatia Z11.

    cargo build --release --offline -p phxzip-cmd
    python3 bancada/phxzip/medir.py [--rodadas N]
    python3 bancada/phxzip/manual.py          # leva os numeros ao manual

Grava `bancada/phxzip/resultados.json`. O manual (`docs/MANUAL-PHXZIP.md`) nao
recebe numero digitado: o `manual.py` le este arquivo e reescreve o bloco
marcado.

Trabalho igual (as quatro regras do `bancada/LEIA-ME.md`)
----------------------------------------------------------
1. **Mesmos dados.** Os dois lados compactam a MESMA pasta e extraem o que
   eles mesmos gravaram. Dois conjuntos, previsiveis e sem sorteio: os fontes
   `crates/` do commit medido (texto, comprime) e um bloco pseudoaleatorio
   gerado por SHA-256 encadeado (nao comprime -- exercita o pedaco cru).
2. **Mesmo metodo.** O PhxZip grava LZMA2, um bloco por arquivo (nao solido),
   cabecalho sem compressao, uma thread, lc=3 lp=0 pb=2, cadeia de dispersao
   com 48 visitas (`PROFUNDIDADE`, `lzma_compressor.rs`) e codificador GULOSO.
   O 7z do lado igual recebe exatamente isso: `-m0=LZMA2:a=0:mf=hc4:mc=48`
   (`a=0` e o modo rapido, o guloso do 7-Zip), `-ms=off -mmt=1 -mhc=off`.
   O dicionario do 7z vai em 64 MiB, que e o teto do PhxZip: para arquivo
   menor que isso a compressao nao muda, so o numero declarado.
3. **Mesma forma de pergunta.** Um processo por operacao dos dois lados:
   o tempo inclui subir o executavel, para os dois.
4. **Mesma quantidade de trabalho.** A prova e o conteudo: cada extracao e
   conferida arquivo por arquivo (SHA-256) contra o conjunto original, nas
   duas direcoes cruzadas tambem (o 7z le o que o PhxZip gravou e o PhxZip le
   o que o 7z gravou). Extracao que nao bate reprova a corrida inteira.

O 7z no PADRAO dele (`-mx5`, solido, codificador otimo, varias threads) entra
como REFERENCIA, marcada `trabalho_igual: false`: e o que quem baixa o 7-Zip
obtem sem mexer em nada, e esconder isso seria publicar so a comparacao que
favorece. O manual a mostra separada, dizendo que nao e trabalho igual.

As rodadas sao INTERCALADAS (a ordem dos lados gira a cada rodada), para que
uma rajada de carga da maquina caia em todos os lados e nao num so. Cada numero
publicado e mediana com a faixa min--max.
"""
import hashlib
import json
import os
import resource
import shutil
import statistics
import subprocess
import sys
import tempfile
import time
from pathlib import Path

AQUI = Path(__file__).resolve().parent
RAIZ = AQUI.parent.parent
PHXZIPCMD = RAIZ / "target" / "release" / "phxzipcmd"
SETE_Z = "/usr/bin/7z"
RODADAS_PADRAO = 5
# Tamanho do conjunto que nao comprime. 8 MiB: grande o bastante para o tempo
# sair das dezenas de milissegundos, pequeno o bastante para a corrida inteira
# caber em poucos minutos.
ALEATORIO_BYTES = 8 << 20

# O metodo do PhxZip, escrito para o 7z. Ver a regra 2 do docstring.
SETE_Z_IGUAL = ["-t7z", "-m0=LZMA2:a=0:mf=hc4:mc=48:fb=273:lc=3:lp=0:pb=2:d=64m",
                "-ms=off", "-mmt=1", "-mhc=off"]
SETE_Z_PADRAO = ["-t7z"]


def lados():
    """Os tres lados: como compacta, como extrai, e se e trabalho igual."""
    z = str(PHXZIPCMD)
    return {
        "phxzip": {
            "rotulo": "phxzipcmd (LZMA2, nao solido, guloso, 1 thread)",
            "trabalho_igual": True,
            "compactar": lambda arq, pasta: [z, "compactar", arq, pasta],
            "extrair": lambda arq, dest: [z, "extrair", arq, "--destino", dest],
        },
        "7z-igual": {
            "rotulo": "7z " + " ".join(SETE_Z_IGUAL),
            "trabalho_igual": True,
            "compactar": lambda arq, pasta: [SETE_Z, "a", *SETE_Z_IGUAL, arq, pasta],
            "extrair": lambda arq, dest: [SETE_Z, "x", "-mmt=1", "-o" + dest, arq],
        },
        "7z-padrao": {
            "rotulo": "7z no padrao (-mx5, solido, otimo, varias threads)",
            "trabalho_igual": False,
            "compactar": lambda arq, pasta: [SETE_Z, "a", *SETE_Z_PADRAO, arq, pasta],
            "extrair": lambda arq, dest: [SETE_Z, "x", "-o" + dest, arq],
        },
    }


def preparar_conjuntos(base: Path, commit: str):
    """Monta os dois conjuntos em `base` e devolve a descricao de cada um."""
    conjuntos = {}

    fontes = base / "fontes"
    fontes.mkdir()
    arq = subprocess.run(["git", "-C", str(RAIZ), "archive", commit, "crates"],
                         check=True, capture_output=True).stdout
    subprocess.run(["tar", "-x", "-C", str(fontes)], input=arq, check=True)
    conjuntos["fontes"] = {
        "descricao": f"os fontes crates/ do commit {commit[:8]} (texto, comprime)",
        "pasta": fontes,
    }

    aleat = base / "aleatorio"
    (aleat / "dados").mkdir(parents=True)
    # SHA-256 encadeado: previsivel (a mesma corrida gera os mesmos bytes) e
    # sem padrao que um compressor ache.
    bloco, saida, h = b"phxzip-bancada", bytearray(), b""
    while len(saida) < ALEATORIO_BYTES:
        h = hashlib.sha256(h + bloco).digest()
        saida += h
    (aleat / "dados" / "aleatorio.bin").write_bytes(bytes(saida[:ALEATORIO_BYTES]))
    conjuntos["aleatorio"] = {
        "descricao": f"um arquivo de {ALEATORIO_BYTES >> 20} MiB por SHA-256 encadeado (nao comprime)",
        "pasta": aleat,
    }

    for c in conjuntos.values():
        c["impressao"] = impressao(c["pasta"])
        c["arquivos"] = len(c["impressao"])
        c["bytes"] = sum(n for n, _ in c["impressao"].values())
    return conjuntos


def impressao(pasta: Path):
    """{caminho relativo: (tamanho, sha256)} de cada arquivo comum sob `pasta`."""
    saida = {}
    for raiz, _, nomes in os.walk(pasta):
        for n in nomes:
            p = Path(raiz) / n
            if p.is_symlink() or not p.is_file():
                continue
            dados = p.read_bytes()
            saida[str(p.relative_to(pasta))] = (len(dados), hashlib.sha256(dados).hexdigest())
    return saida


def rodar(cmd, cwd):
    """Roda um processo e devolve (parede, cpu) em segundos. Falha reprova."""
    antes = resource.getrusage(resource.RUSAGE_CHILDREN)
    t0 = time.perf_counter()
    p = subprocess.run(cmd, cwd=cwd, capture_output=True)
    parede = time.perf_counter() - t0
    depois = resource.getrusage(resource.RUSAGE_CHILDREN)
    if p.returncode != 0:
        sys.exit(f"FALHOU ({p.returncode}): {' '.join(cmd)}\n"
                 f"{p.stdout.decode(errors='replace')[-800:]}\n{p.stderr.decode(errors='replace')[-800:]}")
    cpu = (depois.ru_utime - antes.ru_utime) + (depois.ru_stime - antes.ru_stime)
    return parede, cpu


def conferir_extracao(dest: Path, conjunto, quem):
    """A extracao tem de devolver o conjunto inteiro, byte a byte."""
    nome = conjunto["pasta"].name
    achado = impressao(dest / nome)
    if achado != conjunto["impressao"]:
        faltam = set(conjunto["impressao"]) - set(achado)
        sobram = set(achado) - set(conjunto["impressao"])
        diferem = [k for k in set(achado) & set(conjunto["impressao"])
                   if achado[k] != conjunto["impressao"][k]]
        sys.exit(f"EXTRACAO NAO CONFERE ({quem}): {len(faltam)} faltam, "
                 f"{len(sobram)} sobram, {len(diferem)} diferem")


def versao(cmd):
    p = subprocess.run(cmd, capture_output=True, text=True)
    for linha in (p.stdout + p.stderr).splitlines():
        if linha.strip():
            return linha.strip()
    return "?"


def resumo(xs):
    return {"mediana": statistics.median(xs), "min": min(xs), "max": max(xs)}


def main():
    rodadas = RODADAS_PADRAO
    if "--rodadas" in sys.argv:
        rodadas = int(sys.argv[sys.argv.index("--rodadas") + 1])
    if rodadas < 3:
        sys.exit("--rodadas pede pelo menos 3: com menos, a faixa nao diz nada")
    if not PHXZIPCMD.is_file():
        sys.exit(f"falta {PHXZIPCMD}: cargo build --release --offline -p phxzip-cmd")
    if not os.access(SETE_Z, os.X_OK):
        sys.exit(f"falta {SETE_Z}: apt install 7zip (ou p7zip-full)")

    commit = subprocess.run(["git", "-C", str(RAIZ), "rev-parse", "HEAD"],
                            check=True, capture_output=True, text=True).stdout.strip()
    carga_inicio = os.getloadavg()
    todos = lados()

    # A base de trabalho mora FORA do repositorio e e de quem roda (0700): o
    # extrator do PhxZip recusa pasta em que outro usuario escreve, e e assim
    # que tem de ser.
    with tempfile.TemporaryDirectory(prefix="phxzip-bancada-") as tmp:
        base = Path(tmp)
        os.chmod(base, 0o700)
        (base / "c").mkdir()
        conjuntos = preparar_conjuntos(base / "c", commit)
        medidas = {c: {l: {"compactar_s": [], "compactar_cpu_s": [], "extrair_s": [],
                           "extrair_cpu_s": [], "tamanho": []} for l in todos}
                   for c in conjuntos}
        ordem = list(todos)
        for r in range(rodadas):
            # Gira a ordem dos lados a cada rodada (intercalado).
            ordem_r = ordem[r % len(ordem):] + ordem[:r % len(ordem)]
            for cn, c in conjuntos.items():
                for ln in ordem_r:
                    lado = todos[ln]
                    trab = base / f"r-{cn}-{ln}"
                    shutil.rmtree(trab, ignore_errors=True)
                    trab.mkdir()
                    arq = str(trab / "pacote.7z")
                    parede, cpu = rodar(lado["compactar"](arq, c["pasta"].name), cwd=c["pasta"].parent)
                    m = medidas[cn][ln]
                    m["compactar_s"].append(parede)
                    m["compactar_cpu_s"].append(cpu)
                    m["tamanho"].append(os.path.getsize(arq))
                    dest = trab / "saida"
                    dest.mkdir()
                    parede, cpu = rodar(lado["extrair"](arq, str(dest)), cwd=trab)
                    m["extrair_s"].append(parede)
                    m["extrair_cpu_s"].append(cpu)
                    conferir_extracao(dest, c, ln)
                    if r == 0 and ln != "7z-padrao":
                        # Cruzado, uma vez por conjunto: o outro lado le o que
                        # este gravou. Prova de formato, nao entra no tempo.
                        outro = "7z-igual" if ln == "phxzip" else "phxzip"
                        cruz = trab / "cruzado"
                        cruz.mkdir()
                        rodar(todos[outro]["extrair"](arq, str(cruz)), cwd=trab)
                        conferir_extracao(cruz, c, f"{outro} lendo {ln}")
                    shutil.rmtree(trab)
                print(f"rodada {r + 1}/{rodadas}: {cn} ok", flush=True)

    resultado = {
        "medido_em": time.strftime("%Y-%m-%d %H:%M:%S"),
        "commit": commit,
        "rodadas": rodadas,
        "nucleos": os.cpu_count(),
        "carga_no_inicio": [round(x, 2) for x in carga_inicio],
        "carga_no_fim": [round(x, 2) for x in os.getloadavg()],
        "versao_phxzipcmd": versao([str(PHXZIPCMD), "--version"]),
        "versao_7z": versao([SETE_Z]),
        "lados": {ln: {"rotulo": l["rotulo"], "trabalho_igual": l["trabalho_igual"]}
                  for ln, l in todos.items()},
        "conjuntos": {},
    }
    for cn, c in conjuntos.items():
        resultado["conjuntos"][cn] = {
            "descricao": c["descricao"],
            "arquivos": c["arquivos"],
            "bytes": c["bytes"],
            "medidas": {ln: {**{k: [round(v, 4) if isinstance(v, float) else v for v in xs]
                                for k, xs in m.items()},
                             "resumo": {k: resumo(xs) for k, xs in m.items()}}
                        for ln, m in medidas[cn].items()},
        }

    # Escreve o parcial e troca de uma vez: ou o resultados.json e o antigo
    # inteiro, ou o novo inteiro (`bancada/LEIA-ME.md`).
    parcial = AQUI / "resultados.parcial.json"
    parcial.write_text(json.dumps(resultado, indent=1, ensure_ascii=False) + "\n")
    os.replace(parcial, AQUI / "resultados.json")
    print(f"gravado {AQUI / 'resultados.json'}")


if __name__ == "__main__":
    main()
