#!/usr/bin/env python3
"""PROVA QUE A PROVA PEGA: repoe cada defeito do catalogo e confere que o teste cai.

Esqueleto generalizado do `bancada/guardas/provar-guardas.py` do PhxSql.

USO
    provar-guardas.py --catalogo catalogo.py            roda todas as guardas
    provar-guardas.py --catalogo catalogo.py --so ID    so uma (repita --so)
    provar-guardas.py --catalogo catalogo.py --listar   lista e sai
    provar-guardas.py --autoteste                       prova o proprio juiz, em memoria

    --raiz DIR     arvore do projeto (padrao: diretorio atual)
    --copia DIR    onde montar a copia (padrao: ~/.cache/kit-guardas/<nome-da-raiz>)
    --json ARQ     grava o veredito de cada guarda com a data

O CATALOGO e um arquivo Python com tres nomes:

    COPIAR  = ["Cargo.toml", "Cargo.lock", "src", "tests"]   # o que a copia precisa
    COMANDO = "cargo test --offline --no-fail-fast {alvo}"   # {alvo} vem da entrada
    LINHA   = r"^test (\\S+) \\.\\.\\. (ok|FAILED|ignored)"     # nome e veredito por teste
    GUARDAS = [ {entrada}, ... ]

  cada entrada:
    id        nome curto
    titulo    o defeito em uma linha
    arquivo   caminho relativo a raiz
    trecho    o texto EXATO de hoje -- tem de aparecer UMA vez so no arquivo
    troca     o que entra no lugar: o defeito reposto
    trocas    opcional: lista de {arquivo, trecho, troca} para defeito em varios pontos
    alvo      o que substitui {alvo} no COMANDO (ex.: "-p meu_crate --lib")
    caem      testes que TEM de falhar com o defeito
    seguem    testes que tem de CONTINUAR passando (sem isto, quebrar o arquivo
              inteiro pareceria guarda provada)
    espera    "falha" (padrao) | "aborta" | "nada muda" (redundancia declarada)
    prazo     segundos ate matar a rodada (padrao 300)

O VEREDITO
    PROVADA     todos os `caem` cairam e todos os `seguem` ficaram de pe
    NAO PEGOU   um `caem` passou com o defeito reposto -- o achado mais valioso:
                teste que passa por engano e pior que teste que falta
    ESTRAGOU    um `seguem` caiu junto: a troca quebrou mais que o defeito
    QUEBRADA    trecho sumido/duplicado, nao compila, estourou o prazo, ou a
                BASE (arvore limpa) ja estava vermelha
    REDUNDANTE  `espera: "nada muda"` e nada mudou

OS CUIDADOS, e por que
  * Nunca na arvore de verdade: tudo numa copia; cada troca desfeita em
    `finally`, e um `atexit` como rede para Ctrl-C.
  * BASE VERDE PRIMEIRO: o mesmo comando, sem defeito, tem de passar os
    `caem` e os `seguem`. Medido: onze mutantes «vermelhos» numa rodada em
    que o proprio comando saia com erro de uso -- nenhum provava nada.
  * Prazo em toda rodada, matando o GRUPO de processos: defeito que pendura
    em vez de falhar travaria a bateria.
  * So os testes nomeados: a bateria inteira por mutacao custaria horas.
"""
import argparse
import atexit
import importlib.util
import json
import os
import re
import shutil
import signal
import subprocess
import sys
import time

PRAZO_PADRAO = 300


def carregar_catalogo(caminho):
    spec = importlib.util.spec_from_file_location("catalogo", caminho)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    for nome in ("COPIAR", "COMANDO", "LINHA", "GUARDAS"):
        if not hasattr(mod, nome):
            sys.exit("catalogo sem %s: %s" % (nome, caminho))
    return mod


def trocas_de(g):
    if "trocas" in g:
        return g["trocas"]
    return [{"arquivo": g["arquivo"], "trecho": g["trecho"], "troca": g["troca"]}]


def forma_ok(g):
    """Confere a FORMA da entrada antes do conteudo: uma regua que so olha o
    conteudo deu `ok` em entradas malformadas (cognicao a-regua-do-catalogo)."""
    faltam = [k for k in ("id", "alvo", "caem") if k not in g]
    if "trocas" not in g:
        faltam += [k for k in ("arquivo", "trecho", "troca") if k not in g]
    return faltam


class Arvore:
    """A copia isolada onde os defeitos sao repostos."""

    def __init__(self, raiz, destino, copiar):
        self.raiz, self.dir, self.copiar = raiz, destino, copiar
        self.repostas = []  # (caminho, texto_original)

    def montar(self):
        os.makedirs(self.dir, exist_ok=True)
        for item in self.copiar:
            o, d = os.path.join(self.raiz, item), os.path.join(self.dir, item)
            if not os.path.exists(o):
                sys.exit("COPIAR cita o que nao existe: %s" % item)
            if os.path.isdir(o):
                shutil.copytree(o, d, dirs_exist_ok=True)
            else:
                os.makedirs(os.path.dirname(d), exist_ok=True)
                shutil.copy2(o, d)

    def repor(self, trocas):
        for t in trocas:
            p = os.path.join(self.dir, t["arquivo"])
            if not os.path.exists(p):
                raise ValueError("arquivo nao esta na copia: %s" % t["arquivo"])
            texto = open(p, encoding="utf-8").read()
            n = texto.count(t["trecho"])
            if n != 1:
                raise ValueError("trecho aparece %d vezes em %s (tem de ser 1)" % (n, t["arquivo"]))
            self.repostas.append((p, texto))
            with open(p, "w", encoding="utf-8") as fh:
                fh.write(texto.replace(t["trecho"], t["troca"]))

    def desfazer_tudo(self):
        while self.repostas:
            p, texto = self.repostas.pop()
            with open(p, "w", encoding="utf-8") as fh:
                fh.write(texto)


def rodar(arvore, comando, linha_re, prazo):
    """Devolve (mapa nome->veredito, desfecho). desfecho: rodou|aborta|prazo|nao compilou."""
    p = subprocess.Popen(comando, shell=True, cwd=arvore.dir, text=True,
                         stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                         start_new_session=True)
    try:
        saida = p.communicate(timeout=prazo)[0]
        desfecho = "rodou"
    except subprocess.TimeoutExpired:
        try:
            os.killpg(os.getpgid(p.pid), signal.SIGKILL)  # pelo grupo: o pendurado e o neto
        except OSError:
            pass
        saida = p.communicate()[0] or ""
        desfecho = "prazo"
    vereditos = dict(re.findall(linha_re, saida, re.M))
    if desfecho == "rodou" and not vereditos and p.returncode != 0:
        desfecho = "nao compilou"
    elif desfecho == "rodou" and p.returncode < 0:
        desfecho = "aborta"
    return vereditos, desfecho


def julgar(g, vereditos, desfecho):
    """Devolve (veredito, [notas]). Logica identica a do executor de origem."""
    espera = g.get("espera", "falha")
    if desfecho == "nao compilou":
        return "QUEBRADA", ["o codigo com o defeito reposto nao compila"]
    if desfecho == "aborta":
        return ("PROVADA", ["abortou, como esperado"]) if espera == "aborta" \
            else ("ESTRAGOU", ["abortou, e devia so reprovar"])
    if espera == "aborta":
        return "NAO PEGOU", ["esperava abortar, e terminou"]
    if desfecho == "prazo":
        return "QUEBRADA", ["estourou o prazo do executor"]
    if espera == "nada muda":
        cairam = [n for n, r in vereditos.items() if r == "FAILED"]
        if cairam:
            return "NAO PEGOU", ["dizia que nada mudaria, e caiu: %s" % ", ".join(cairam[:5])]
        return "REDUNDANTE", [g.get("nota_da_redundancia", "nada mudou, como declarado")]
    sumidos = [n for n in g["caem"] if n not in vereditos]
    if sumidos:
        return "QUEBRADA", ["o teste %s nao existe mais" % n for n in sumidos]
    de_pe = [n for n in g["caem"] if vereditos.get(n) == "ok"]
    if de_pe:
        return "NAO PEGOU", ["PASSOU COM O DEFEITO REPOSTO: %s" % n for n in de_pe]
    demais = [n for n in g.get("seguem", []) if vereditos.get(n) != "ok"]
    if demais:
        return "ESTRAGOU", ["caiu junto, e nao devia: %s" % n for n in demais]
    return "PROVADA", []


def provar(g, arvore, cat):
    faltam = forma_ok(g)
    if faltam:
        return "QUEBRADA", ["entrada sem %s" % ", ".join(faltam)]
    cmd = cat.COMANDO.format(alvo=g["alvo"])
    prazo = g.get("prazo", PRAZO_PADRAO)
    # 1. base: sem defeito, os nomeados passam
    base, desf = rodar(arvore, cmd, cat.LINHA, prazo)
    vermelhos = [n for n in g["caem"] + g.get("seguem", []) if base.get(n) != "ok"]
    if desf != "rodou" or vermelhos:
        return "QUEBRADA", ["BASE nao saiu verde (%s): %s" % (desf, ", ".join(vermelhos[:5]))]
    # 2..4. repor, rodar, desfazer
    try:
        arvore.repor(trocas_de(g))
        vereditos, desf = rodar(arvore, cmd, cat.LINHA, prazo)
    except ValueError as e:
        return "QUEBRADA", [str(e)]
    finally:
        arvore.desfazer_tudo()
    # 5. comparar
    return julgar(g, vereditos, desf)


def autoteste():
    """Prova o juiz nos dois sentidos, sem rodar nada de fora."""
    g = {"id": "x", "alvo": "", "caem": ["a"], "seguem": ["b"]}
    casos = [
        ({"a": "FAILED", "b": "ok"}, "rodou", "PROVADA"),
        ({"a": "ok", "b": "ok"}, "rodou", "NAO PEGOU"),
        ({"a": "FAILED", "b": "FAILED"}, "rodou", "ESTRAGOU"),
        ({"b": "ok"}, "rodou", "QUEBRADA"),
        ({}, "prazo", "QUEBRADA"),
        ({}, "nao compilou", "QUEBRADA"),
    ]
    ruins = 0
    for v, d, esperado in casos:
        obtido = julgar(g, v, d)[0]
        if obtido != esperado:
            ruins += 1
            print("AUTOTESTE FALHOU: %r/%s deu %s, esperado %s" % (v, d, obtido, esperado))
    if forma_ok({"id": "y"}) == []:
        ruins += 1
        print("AUTOTESTE FALHOU: entrada malformada passou na forma")
    print("autoteste: %d casos, %d falhas" % (len(casos) + 1, ruins))
    return ruins == 0


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--catalogo")
    ap.add_argument("--raiz", default=os.getcwd())
    ap.add_argument("--copia")
    ap.add_argument("--so", action="append", default=[])
    ap.add_argument("--listar", action="store_true")
    ap.add_argument("--json")
    ap.add_argument("--autoteste", action="store_true")
    a = ap.parse_args()

    if a.autoteste:
        sys.exit(0 if autoteste() else 1)
    if not a.catalogo:
        ap.error("--catalogo e obrigatorio")
    if not autoteste():
        sys.exit("o juiz nao passou no proprio autoteste -- nenhum veredito vale")

    cat = carregar_catalogo(a.catalogo)
    guardas = [g for g in cat.GUARDAS if not a.so or g.get("id") in a.so]
    if a.listar:
        for g in guardas:
            print("%-32s %s" % (g.get("id"), g.get("titulo", "")))
        print("%d guardas" % len(guardas))
        return
    raiz = os.path.abspath(a.raiz)
    destino = a.copia or os.path.join(os.path.expanduser("~/.cache/kit-guardas"), os.path.basename(raiz))
    arvore = Arvore(raiz, destino, cat.COPIAR)
    atexit.register(arvore.desfazer_tudo)
    arvore.montar()

    resultados, contagem = [], {}
    for g in guardas:
        inicio = time.time()
        veredito, notas = provar(g, arvore, cat)
        contagem[veredito] = contagem.get(veredito, 0) + 1
        print("%-11s %-32s %5.1fs %s" % (veredito, g.get("id"), time.time() - inicio, "; ".join(notas)))
        resultados.append({"id": g.get("id"), "veredito": veredito, "notas": notas,
                           "quando": time.strftime("%Y-%m-%dT%H:%M:%S")})
    print("\n" + "  ".join("%s %d" % kv for kv in sorted(contagem.items())))
    if a.json:
        with open(a.json, "w", encoding="utf-8") as fh:
            json.dump(resultados, fh, ensure_ascii=False, indent=1)
    ruins = sum(contagem.get(k, 0) for k in ("NAO PEGOU", "ESTRAGOU", "QUEBRADA"))
    sys.exit(1 if ruins else 0)


if __name__ == "__main__":
    main()
