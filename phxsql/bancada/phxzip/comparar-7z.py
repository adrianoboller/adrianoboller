#!/usr/bin/env python3
"""PhxZip x 7-Zip: tamanho, tempo de compactar e de descompactar.

Grava `bancada/phxzip/comparar-7z.json`; o grafico da pagina de graficos
(`docs/dossie/graficos-dos-testes.py`) sai dali, nunca da memoria.

Quatro lados por nivel: cada um com UM fio e com VARIOS (os nucleos da
maquina). Um fio contra um fio mede o codificador; varios contra varios mede
o que quem espera o arquivo sente. O PhxZip com varios fios corta em blocos
(docs/PHXZIP.md §3d) e perde alguns por cento de tamanho -- o grafico mostra
os dois custos lado a lado, em vez de escolher o que favorece.

Trabalho IGUAL, que e a regra da bancada (bancada/LEIA-ME.md):
- o 7-Zip com `-mf=off` (sem o filtro BCJ que ele poe sozinho em executavel;
  o PhxZip nao tem filtro, por decisao do dono no pedido 454) e `-mmt=1`
  (um fio: o PhxZip comprime num fio so, e comparar quatro nucleos contra um
  seria medir a maquina, nao o codificador);
- os dois descompactam para uma PASTA, gravando os arquivos -- nao um para
  /dev/null e o outro para o disco;
- os dois leem o MESMO corpus, copiado para uma pasta temporaria, e o
  sha256 de cada arquivo vai no resultado.

Cinco corridas por medida; grava mediana, minimo e maximo (o grafico mostra
a faixa e so declara vencedor quando as faixas nao se cruzam -- pedido 155).
Cada arquivo produzido pelo PhxZip e conferido pelo `7z t` e extraido pelo
7-Zip com o conteudo comparado: numero de arquivo que o outro nao abre nao
entra.

Uso:  python3 bancada/phxzip/comparar-7z.py [corridas]
Antes: cargo build --release -p phxzip-cmd
"""
import datetime
import glob
import hashlib
import json
import os
import shutil
import statistics
import subprocess
import sys
import tempfile
import time

RAIZ = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
PHX = os.path.join(RAIZ, "target", "release", "phxzipcmd")
SAIDA = os.path.join(RAIZ, "bancada", "phxzip", "comparar-7z.json")
NIVEIS = [1, 5, 9]

# Dois corpos: TEXTO (onde o codificador trabalha de verdade) e MISTO (texto
# + imagem ja comprimida + executavel -- o que alguem compacta no dia a dia).
# O executavel e o do proprio 7-Zip do sistema: fixo entre corridas, ao
# contrario do nosso binario, que muda a cada compilacao.
CORPOS = {
    "texto": ["docs/PENDENCIAS.md", "docs/FORMATO.md", "MANUAL.txt"],
    "misto": ["docs/PENDENCIAS.md", "docs/FORMATO.md", "MANUAL.txt",
              "marca/phxsql-abertura.png", "/usr/lib/7zip/7z.so"],
    # O GRANDE existe para o corte em blocos ter o que cortar: com 6 MB o
    # nivel 5 (blocos de 4 MiB) faz dois blocos e o paralelo quase nao
    # aparece. Todos os documentos + os binarios do 7-Zip do sistema.
    "grande": ["docs/*.md", "MANUAL.txt", "marca/phxsql-abertura.png",
               "/usr/lib/7zip/7z.so", "/usr/lib/7zip/7za", "/usr/lib/7zip/7zr",
               "/usr/lib/7zip/7z", "/usr/lib/7zip/7zCon.sfx"],
}
NUCLEOS = os.cpu_count() or 1
LADOS = [("phxzip", 1), ("phxzip", NUCLEOS), ("7zip", 1), ("7zip", NUCLEOS)]


def sha(caminho):
    h = hashlib.sha256()
    with open(caminho, "rb") as f:
        for bloco in iter(lambda: f.read(1 << 20), b""):
            h.update(bloco)
    return h.hexdigest()


def rodar(cmd, cwd=None):
    t = time.perf_counter()
    r = subprocess.run(cmd, cwd=cwd, stdout=subprocess.DEVNULL,
                       stderr=subprocess.PIPE)
    dt = time.perf_counter() - t
    if r.returncode != 0:
        raise SystemExit(f"falhou: {' '.join(cmd)}\n{r.stderr.decode()[-800:]}")
    return dt


def resumo(amostras):
    return {"mediana": statistics.median(amostras), "min": min(amostras),
            "max": max(amostras), "amostras": amostras}


def versao_7z():
    r = subprocess.run(["7z"], stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    for linha in r.stdout.decode(errors="replace").splitlines():
        if linha.startswith("7-Zip"):
            return linha.split(":")[0].strip()
    return "7-Zip (versao nao lida)"


def conferir_iguais(a, b):
    for nome in os.listdir(a):
        if sha(os.path.join(a, nome)) != sha(os.path.join(b, nome)):
            raise SystemExit(f"conteudo diferente depois da ida e volta: {nome}")


def medir(corridas):
    if not os.path.exists(PHX):
        raise SystemExit(f"falta {PHX}: rode cargo build --release -p phxzip-cmd")
    resultado = {
        "medido_em": datetime.datetime.now(datetime.timezone.utc)
        .strftime("%Y-%m-%dT%H:%M:%SZ"),
        "corridas": corridas,
        "sete_zip": versao_7z(),
        "comando_7z": "7z a -t7z -m0=lzma2 -mx=N -mf=off -mmt=F",
        "comando_phxzip": "phxzipcmd a -mx=N -mmt=F",
        "fios_mt": NUCLEOS,
        "maquina": os.uname().machine + ", " + str(os.cpu_count()) + " nucleos",
        "corpos": {},
    }
    for corpo, arquivos in CORPOS.items():
        with tempfile.TemporaryDirectory(prefix="phx-7z-") as tmp:
            orig = os.path.join(tmp, "orig")
            os.mkdir(orig)
            info = []
            fontes = []
            for a in arquivos:
                src = a if os.path.isabs(a) else os.path.join(RAIZ, a)
                fontes += sorted(glob.glob(src)) if "*" in src else [src]
            for src in fontes:
                dst = os.path.join(orig, os.path.basename(src))
                shutil.copyfile(src, dst)
                info.append({"nome": os.path.basename(src),
                             "bytes": os.path.getsize(dst), "sha256": sha(dst)})
            total = sum(i["bytes"] for i in info)
            nomes = [i["nome"] for i in info]
            por_nivel = {}
            for n in NIVEIS:
                med = {}
                for quem, fios in LADOS:
                    lado = quem if fios == 1 else f"{quem}_mt"
                    arq = os.path.join(tmp, f"{lado}-{n}.7z")
                    if quem == "phxzip":
                        cmd_a = [PHX, "a", arq, *nomes, f"-mx={n}",
                                 f"-mmt={fios}", "-y"]
                    else:
                        cmd_a = ["7z", "a", "-t7z", "-m0=lzma2", f"-mx={n}",
                                 "-mf=off", f"-mmt={fios}", arq, *nomes]
                    comp, desc = [], []
                    for _ in range(corridas):
                        if os.path.exists(arq):
                            os.remove(arq)
                        comp.append(rodar(cmd_a, cwd=orig))
                        out = os.path.join(tmp, f"x-{lado}-{n}")
                        shutil.rmtree(out, ignore_errors=True)
                        if quem == "phxzip":
                            cmd_x = [PHX, "x", arq, f"-o{out}", "-y"]
                        else:
                            cmd_x = ["7z", "x", f"-mmt={fios}", f"-o{out}", "-y", arq]
                        desc.append(rodar(cmd_x))
                        conferir_iguais(orig, out)
                    tamanho = os.path.getsize(arq)
                    if quem == "phxzip":
                        # O arquivo nosso tem de abrir no 7-Zip, com o mesmo
                        # conteudo -- senao o tamanho nao vale nada.
                        rodar(["7z", "t", arq])
                        cruz = os.path.join(tmp, f"cruz-{lado}-{n}")
                        rodar(["7z", "x", f"-o{cruz}", "-y", arq])
                        conferir_iguais(orig, cruz)
                    med[lado] = {"bytes": tamanho, "fios": fios,
                                 "compactar_s": resumo(comp),
                                 "descompactar_s": resumo(desc),
                                 "compactar_mb_s": total / 1e6 /
                                 statistics.median(comp),
                                 "descompactar_mb_s": total / 1e6 /
                                 statistics.median(desc)}
                    print(f"{corpo:6} nivel {n} {lado:9} {tamanho:>10} bytes  "
                          f"compacta {statistics.median(comp):.3f} s  "
                          f"descompacta {statistics.median(desc):.3f} s",
                          flush=True)
                por_nivel[str(n)] = med
            resultado["corpos"][corpo] = {"bytes": total, "arquivos": info,
                                          "niveis": por_nivel}
    with open(SAIDA, "w") as f:
        json.dump(resultado, f, ensure_ascii=False, indent=1)
        f.write("\n")
    print(f"gravado {os.path.relpath(SAIDA, RAIZ)}")


if __name__ == "__main__":
    medir(int(sys.argv[1]) if len(sys.argv) > 1 else 5)
