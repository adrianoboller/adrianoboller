#!/usr/bin/env python3
"""PROVA QUE A PROVA PEGA: repoe cada defeito e confere que o teste cai.

    python3 bancada/guardas/provar-guardas.py
    python3 bancada/guardas/provar-guardas.py --so profiler-recorta
    python3 bancada/guardas/provar-guardas.py --listar

# O que ele responde

A casa exige que todo teste novo FALHE com o defeito reposto. Isso sempre foi
feito a mao, uma vez, por quem escreveu o teste -- e depois se perdia. Ninguem
conseguia dizer, hoje, quais das 1.242 asercoes ainda pegariam o defeito que as
motivou. Este executor responde essa pergunta a cada rodada.

Para cada entrada do `catalogo.py` ele:

  1. confere, na arvore LIMPA, que os testes nomeados PASSAM (a outra metade da
     prova real: passar com o conserto);
  2. repoe o defeito -- troca o trecho pelo texto de antes;
  3. roda SO o binario de teste nomeado e le, teste a teste, quem caiu;
  4. desfaz a troca;
  5. compara com o esperado.

O veredito de cada guarda:

    PROVADA     todos os `caem` cairam e todos os `seguem` continuaram de pe
    NAO PEGOU   um `caem` continuou passando -- E O ACHADO MAIS VALIOSO daqui:
                e um teste que passa por engano, e a casa considera isso pior
                que teste que falta
    REDUNDANTE  a entrada declarou `espera: "nada muda"` e nada mudou mesmo:
                a guarda existe DUAS vezes no codigo, e tirar uma so nao e
                sentido por teste nenhum. E resultado medido, e nao falha --
                mas fica escrito, porque a redundancia e o que faz o `caem`
                daquela outra entrada ser o que e
    ESTRAGOU    um `seguem` caiu junto: a troca quebrou mais do que o defeito
                de origem quebrava, entao ela nao prova a guarda
    QUEBRADA    o trecho nao esta mais no arquivo, ou aparece duas vezes, ou o
                codigo trocado nem compila -- a entrada do catalogo envelheceu

# Os tres cuidados que este executor tem, e por que

**Nunca na arvore de verdade.** Ele copia `crates/`, `Cargo.toml` e `Cargo.lock`
para um diretorio proprio (5 MB) e mexe so la. Mesmo assim, cada troca e
desfeita num `finally` e ha uma rede de seguranca no `atexit`: um Ctrl-C no meio
nao deixa defeito plantado em lugar nenhum.

**So os testes nomeados.** Rodar a bateria inteira a cada mutacao custaria
horas. Cada entrada diz o pacote e o binario de teste; o executor roda aquele
binario, que leva segundos.

**Prazo em toda rodada.** Defeito que PENDURA em vez de falhar travaria a
bateria -- e o `sujas-com-a-trava` e exatamente esse: um `Mutex` nao reentrante
pedido duas vezes pela mesma thread. O teste dele ja tem prazo proprio de 30 s;
o executor tem o dele por cima, e mais largo, senao mataria a rodada ANTES de o
teste conseguir reprovar.

Ele nao abre porta nenhuma e nao sobe servidor nenhum: e `cargo test` e mais
nada.
"""

import argparse
import atexit
import fcntl
import os
import re
import shutil
import signal
import subprocess
import sys
import time

AQUI = os.path.dirname(os.path.abspath(__file__))
RAIZ = os.path.abspath(os.path.join(AQUI, "..", ".."))
sys.path.insert(0, AQUI)

from catalogo import GUARDAS  # noqa: E402

PRAZO_PADRAO = 300
# So o que a compilacao precisa. O `target/` NAO vem junto, e por dois motivos
# -- um que nao muda e outro que so cresce. O que nao muda: os caminhos ficam
# gravados DENTRO dele, entao a copia nao serviria sem recompilar do mesmo
# jeito. O que so cresce: este comentario dizia «2 GB», e quando alguem foi
# medir o `target/` da arvore ja estava em 7,1 GB -- 3,5x o numero escrito
# aqui. Numero em comentario envelhece calado igual a numero em documento, e a
# licao e a mesma: nao se cita, mede-se. Hoje: `du -sh target`.
#
# O diretorio novo compila uma vez e depois so incrementa; e por isso que o
# `alvo/` da copia fica em caminho FIXO e vale muito menos (1,2 GB, porque so
# tem o que estas guardas exercitam).
#
# `exemplos/` esta aqui porque o `lib.rs` do servidor faz
# `include_str!("../../../exemplos/Config_exemplo_01.json")` -- a copia so com
# `crates/` nem compila. Foi a primeira coisa que este executor descobriu, e o
# recado do compilador dizia exatamente qual arquivo faltava.
COPIAR = ["Cargo.toml", "Cargo.lock", "crates", "exemplos"]

CORES = {"ok": "\033[32m", "mal": "\033[31m", "fraco": "\033[90m",
         "aviso": "\033[33m", "fim": "\033[0m"}


def cor(nome, texto):
    if not sys.stdout.isatty():
        return texto
    return CORES[nome] + texto + CORES["fim"]


def trocas_de(g):
    """A lista de mudancas, venha ela na forma curta ou na longa."""
    if "trocas" in g:
        return g["trocas"]
    return [{"arquivo": g["arquivo"], "trecho": g["trecho"], "troca": g["troca"]}]


# ---------------------------------------------------------------- a copia
class Arvore:
    """A copia onde o defeito e reposto. A de verdade nunca e tocada."""

    def __init__(self, destino):
        self.dir = destino
        self.originais = {}
        self._tranca = None
        atexit.register(self.desfazer_tudo)

    def trancar(self):
        """UMA rodada de cada vez nesta copia -- a segunda espera a primeira.

        # O achado que obrigou a escrever isto

        A copia mora num caminho FIXO (`~/.cache/phx-guardas`), e de proposito:
        e o que guarda o `target/` quente entre rodadas. Ate aqui ninguem tinha
        rodado duas de uma vez.

        Duas rodadas ao mesmo tempo se estragam de tres jeitos, e os tres
        apareceram numa rodada so, com veredito de mentira em cada um:

          * a rodada A planta o defeito dela; a rodada B chama `repor` e nao
            acha mais o trecho -- e declara QUEBRADA («o codigo mudou e a
            entrada do catalogo envelheceu») uma entrada que esta perfeita;
          * a rodada B roda o binario com o defeito de A dentro, e um teste que
            nao tem nada com a guarda de B reprova -- vira NAO PEGOU, que e
            justamente o veredito que a casa trata como o achado mais valioso
            daqui;
          * o defeito de A e um que PENDURA (o `sujas-com-a-trava`), e com o
            `--limpar` de B apagando o `target/` embaixo dele a compilacao
            recomeca do zero e o prazo de 420 s estoura -- QUEBRADA de novo,
            por relogio.

        Nenhum dos tres tem cara de contaminacao: os tres tem cara de codigo
        que mudou. Foi preciso ir olhar a copia depois e achar o
        `// DEFEITO REPOSTO` ainda plantado nela.

        `flock` e o que resolve porque o nucleo a solta sozinho quando o
        processo morre -- inclusive num SIGKILL, que e o unico jeito de o
        `atexit` nao rodar. Tranca pendurada por rodada morta e impossivel.

        A tranca fica FORA do diretorio: o `--limpar` apaga o diretorio inteiro,
        e a tranca nao pode ir junto.
        """
        alvo = self.dir.rstrip(os.sep) + ".tranca"
        os.makedirs(os.path.dirname(alvo) or ".", exist_ok=True)
        self._tranca = open(alvo, "w")
        try:
            fcntl.flock(self._tranca, fcntl.LOCK_EX | fcntl.LOCK_NB)
            return 0.0
        except OSError:
            pass
        print(cor("aviso", "outra rodada esta usando %s -- esperando a vez"
                  % self.dir))
        inicio = time.time()
        fcntl.flock(self._tranca, fcntl.LOCK_EX)
        return time.time() - inicio

    def montar(self, reaproveitar):
        if reaproveitar and os.path.isdir(os.path.join(self.dir, "crates")):
            # Reaproveitar a copia guarda o `target/` dela, e com ele a
            # compilacao incremental: a diferenca e minutos por rodada.
            for item in COPIAR:
                self._sincronizar(item)
            return "reaproveitada"
        shutil.rmtree(self.dir, ignore_errors=True)
        os.makedirs(self.dir, exist_ok=True)
        for item in COPIAR:
            self._sincronizar(item)
        return "nova"

    def _sincronizar(self, item):
        """Copia POR CONTEUDO, e com a data de agora no que mudou.

        # A armadilha que este metodo existe para evitar

        A primeira versao usava `copytree`, que copia com `copy2` e PRESERVA a
        data do original. O efeito, medido: a rodada anterior compilou o
        `target/` da copia a partir do fonte MUTADO; a rodada seguinte
        devolveu o fonte limpo com a data velha; e o cargo, que decide por
        data, achou o artefato mais novo que o fonte e nao recompilou nada --
        entao a "arvore limpa" rodou o binario com o defeito ainda dentro.

        E a regra da casa aparecendo dentro da propria ferramenta que a prova:
        medidor com binario velho mede o passado. Quem pegou foi a conferencia
        da arvore limpa, que existe justamente para isso.

        Copiar tudo com a data de agora consertaria, e faria o workspace
        inteiro recompilar a cada chamada. Comparar o conteudo antes custa uma
        leitura de 5 MB e deixa a compilacao incremental de pe.
        """
        origem = os.path.join(RAIZ, item)
        destino = os.path.join(self.dir, item)
        if not os.path.isdir(origem):
            self._copiar_se_mudou(origem, destino)
            return
        vistos = set()
        for pasta, _, arquivos in os.walk(origem):
            relativo = os.path.relpath(pasta, origem)
            alvo = destino if relativo == "." else os.path.join(destino, relativo)
            os.makedirs(alvo, exist_ok=True)
            for a in arquivos:
                self._copiar_se_mudou(os.path.join(pasta, a), os.path.join(alvo, a))
                vistos.add(os.path.join(alvo, a))
        # O que sobrou de uma copia anterior sai: arquivo fantasma tambem
        # compila, e ninguem o encontraria lendo o fonte de verdade.
        for pasta, _, arquivos in os.walk(destino):
            for a in arquivos:
                caminho = os.path.join(pasta, a)
                if caminho not in vistos:
                    os.remove(caminho)

    @staticmethod
    def _copiar_se_mudou(origem, destino):
        try:
            if open(destino, "rb").read() == open(origem, "rb").read():
                return
        except OSError:
            pass
        # `shutil.copy` (e nao `copy2`): a data fica sendo a de AGORA, que e o
        # que faz o cargo recompilar o que mudou.
        shutil.copy(origem, destino)

    def caminho(self, relativo):
        return os.path.join(self.dir, relativo)

    def garantir_frescor(self, arquivos):
        """Poe a data de AGORA nos arquivos que o catalogo sabe mutar.

        Copiar por conteudo impede a contaminacao NOVA, e nao desfaz a velha: um
        `target/` que ficou de uma rodada anterior guarda o binario compilado a
        partir do fonte mutado, e o cargo -- que decide por data -- nao vai
        recompilar um fonte com data antiga. Nesse estado a "arvore limpa"
        rodaria o defeito.

        Custa uma recompilacao dos dois pacotes por invocacao. A ferramenta que
        existe para pegar binario velho nao pode ser enganada por um.
        """
        agora = time.time()
        for relativo in sorted(set(arquivos)):
            alvo = self.caminho(relativo)
            if os.path.exists(alvo):
                os.utime(alvo, (agora, agora))

    def repor(self, mudancas):
        """Aplica as trocas. Devolve o motivo quando alguma nao da para aplicar."""
        for m in mudancas:
            alvo = self.caminho(m["arquivo"])
            if not os.path.exists(alvo):
                return "o arquivo %s nao existe mais" % m["arquivo"]
            texto = open(alvo, encoding="utf-8").read()
            quantas = texto.count(m["trecho"])
            if quantas == 0:
                return ("o trecho nao esta mais em %s -- o codigo mudou e a "
                        "entrada do catalogo envelheceu" % m["arquivo"])
            if quantas > 1:
                return ("o trecho aparece %d vezes em %s: trocar a errada "
                        "provaria outra coisa" % (quantas, m["arquivo"]))
            self.originais.setdefault(alvo, texto)
            open(alvo, "w", encoding="utf-8").write(
                texto.replace(m["trecho"], m["troca"]))
        return None

    def desfazer_tudo(self):
        for alvo, texto in list(self.originais.items()):
            try:
                open(alvo, "w", encoding="utf-8").write(texto)
            except OSError:
                pass
            self.originais.pop(alvo, None)


# ---------------------------------------------------------------- o cargo
LINHA_DE_TESTE = re.compile(r"^test (\S+) \.\.\. (ok|FAILED|ignored)", re.M)


def rodar(arvore, pacote, alvo, prazo):
    """Roda um binario de teste e devolve (mapa nome->veredito, desfecho, saida).

    `desfecho` e um de: "rodou", "aborta", "prazo", "nao compilou".
    """
    cmd = ["cargo", "test", "--offline", "--no-fail-fast", "-p", pacote] + alvo
    amb = dict(os.environ)
    amb["CARGO_TARGET_DIR"] = os.path.join(arvore.dir, "alvo")
    amb["CARGO_NET_OFFLINE"] = "true"
    amb.pop("RUSTFLAGS", None)
    inicio = time.time()
    p = subprocess.Popen(cmd, cwd=arvore.dir, env=amb, text=True,
                         stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                         start_new_session=True)
    try:
        saida = p.communicate(timeout=prazo)[0]
        desfecho = "rodou"
    except subprocess.TimeoutExpired:
        # Pelo grupo, e nao pelo PID: o `cargo` fica de fora do caminho e quem
        # esta pendurado e o binario de teste que ele criou.
        try:
            os.killpg(os.getpgid(p.pid), signal.SIGKILL)
        except OSError:
            pass
        saida = p.communicate()[0] or ""
        desfecho = "prazo"
    gasto = time.time() - inicio
    vereditos = {n: v for n, v in LINHA_DE_TESTE.findall(saida)}
    if desfecho == "rodou":
        if not vereditos and re.search(r"^error(\[E\d+\])?: ", saida, re.M):
            desfecho = "nao compilou"
        elif p.returncode not in (0, 101):
            # 134 = SIGABRT. Um "stack overflow" derruba o binario inteiro e
            # nao sobra veredito de teste nenhum -- e o tamanho do estrago E a
            # prova, no unico defeito do catalogo que aborta.
            desfecho = "aborta"
        elif "stack overflow" in saida or "process didn't exit successfully" in saida:
            desfecho = "aborta"
    return vereditos, desfecho, saida, gasto


# ------------------------------------------------------------- o veredito
def julgar(g, vereditos, desfecho):
    """Devolve (veredito, [notas])."""
    espera = g.get("espera", "falha")
    notas = []
    if desfecho == "nao compilou":
        return "QUEBRADA", ["o codigo com o defeito reposto nao compila"]
    if desfecho == "aborta":
        if espera == "aborta":
            return "PROVADA", ["o binario abortou, que e como esta guarda pega"]
        return "ESTRAGOU", ["o binario abortou, e esta guarda devia so reprovar"]
    if espera == "aborta":
        return "NAO PEGOU", ["esperava o binario abortar, e ele terminou"]
    if desfecho == "prazo":
        return "QUEBRADA", ["a rodada estourou o prazo do executor"]

    if espera == "nada muda":
        # A entrada AFIRMA que tirar so esta metade nao muda nada, porque a
        # outra metade cobre sozinha. Se algum teste cair, a afirmacao morreu:
        # a redundancia acabou, e alguem precisa saber.
        cairam = [n for n, r in vereditos.items() if r == "FAILED"]
        if cairam:
            return "NAO PEGOU", [
                "a entrada dizia que nada mudaria, e caiu: %s" % ", ".join(cairam[:5])]
        return "REDUNDANTE", [g.get("nota_da_redundancia", "nada mudou, como declarado")]

    de_pe = [n for n in g["caem"] if vereditos.get(n) == "ok"]
    sumidos = [n for n in g["caem"] if n not in vereditos]
    cairam_demais = [n for n in g.get("seguem", []) if vereditos.get(n) != "ok"]
    if sumidos:
        return "QUEBRADA", ["o teste %s nao existe mais neste binario" % n
                            for n in sumidos]
    if de_pe:
        notas += ["PASSOU COM O DEFEITO REPOSTO: %s" % n for n in de_pe]
        return "NAO PEGOU", notas
    if cairam_demais:
        notas += ["caiu junto, e nao devia: %s" % n for n in cairam_demais]
        return "ESTRAGOU", notas
    return "PROVADA", notas


# ------------------------------------------------------------------ saida
def main():
    ap = argparse.ArgumentParser(add_help=True)
    ap.add_argument("--so", action="append", default=[],
                    help="roda so as guardas cujo id contem este pedaco")
    ap.add_argument("--arvore", default=None,
                    help="onde fica a copia (padrao: ~/.cache/phx-guardas)")
    ap.add_argument("--limpar", action="store_true",
                    help="refaz a copia do zero, sem reaproveitar o target/")
    ap.add_argument("--listar", action="store_true",
                    help="mostra o catalogo e sai")
    ap.add_argument("--json", default=None, help="grava o resultado neste arquivo")
    opc = ap.parse_args()

    escolhidas = [g for g in GUARDAS
                  if not opc.so or any(p in g["id"] for p in opc.so)]
    if opc.listar:
        print("%d guardas no catalogo:\n" % len(GUARDAS))
        for g in GUARDAS:
            print("  %-28s %s" % (g["id"], g["titulo"]))
            print("      %s  %s   %d teste(s) tem de cair"
                  % (g["pacote"], " ".join(g["alvo"]), len(g["caem"])))
        return 0
    if not escolhidas:
        print("nenhuma guarda casa com %s" % opc.so)
        return 2

    # FORA do /tmp, e de proposito. O `restaurar.rs` tem um teste que exige que
    # o palco da restauracao NAO caia em `std::env::temp_dir()`, e ele o mede
    # contra o diretorio de trabalho: com a copia em `/tmp/...`, o proprio
    # diretorio de trabalho e temporario e o teste reprova sem haver defeito
    # nenhum. Achado rodando -- ler o teste nao mostraria isso.
    destino = opc.arvore or os.path.join(
        os.path.expanduser("~"), ".cache", "phx-guardas")
    arvore = Arvore(destino)
    print("=" * 72)
    print("PROVANDO AS GUARDAS -- repor o defeito e conferir que o teste cai")
    print("=" * 72)
    print("copia da arvore: %s" % destino)
    # A TRANCA VEM ANTES DO `montar`, porque o `--limpar` apaga o diretorio.
    esperou = arvore.trancar()
    if esperou:
        print("                 esperou %.0f s pela vez" % esperou)
    estado = arvore.montar(reaproveitar=not opc.limpar)
    arvore.garantir_frescor(
        m["arquivo"] for g in GUARDAS for m in trocas_de(g))
    print("                 %s\n" % estado)

    # A arvore LIMPA primeiro: sem isto, uma guarda cujo teste ja estivesse
    # vermelho apareceria como provada -- o defeito reposto nao teria feito
    # nada e o teste cairia do mesmo jeito.
    base = {}
    alvos = []
    for g in escolhidas:
        chave = (g["pacote"], tuple(g["alvo"]))
        if chave not in alvos:
            alvos.append(chave)
    print("--- a arvore limpa, antes de qualquer defeito ---")
    for pacote, alvo in alvos:
        v, desfecho, saida, gasto = rodar(arvore, pacote, list(alvo), 900)
        base[(pacote, alvo)] = v
        maus = [n for n, r in v.items() if r == "FAILED"]
        limpa = desfecho == "rodou" and not maus
        marca = cor("ok", "verde") if limpa else cor(
            "mal", "VERMELHA" if desfecho == "rodou" else desfecho.upper())
        print("  %-40s %-8s %5.1f s  %d testes"
              % (pacote + " " + " ".join(alvo), marca, gasto, len(v)))
        if desfecho != "rodou" or maus:
            print(cor("mal", "  a arvore limpa nao esta verde; nada aqui prova nada."))
            for l in saida.splitlines()[-25:]:
                print("   | " + l)
            return 2
    print()

    resultados = []
    print("--- com o defeito reposto, um de cada vez ---")
    for g in escolhidas:
        chave = (g["pacote"], tuple(g["alvo"]))
        limpa = base[chave]
        faltando = [n for n in g.get("caem", []) + g.get("seguem", [])
                    if n not in limpa]
        if faltando:
            resultados.append((g, "QUEBRADA", 0.0,
                               ["teste que o catalogo nomeia e o binario nao "
                                "tem: %s" % ", ".join(faltando)]))
            print("  %-28s %s" % (g["id"], cor("mal", "QUEBRADA")))
            continue

        motivo = arvore.repor(trocas_de(g))
        if motivo:
            arvore.desfazer_tudo()
            resultados.append((g, "QUEBRADA", 0.0, [motivo]))
            print("  %-28s %s  %s" % (g["id"], cor("mal", "QUEBRADA"), motivo))
            continue
        try:
            v, desfecho, saida, gasto = rodar(
                arvore, g["pacote"], list(g["alvo"]), g.get("prazo", PRAZO_PADRAO))
        finally:
            arvore.desfazer_tudo()
        veredito, notas = julgar(g, v, desfecho)
        if veredito == "QUEBRADA" and desfecho == "nao compilou":
            notas += [l for l in saida.splitlines() if l.startswith("error")][:3]
        resultados.append((g, veredito, gasto, notas))
        pintura = {"PROVADA": "ok", "REDUNDANTE": "fraco", "NAO PEGOU": "mal",
                   "ESTRAGOU": "aviso", "QUEBRADA": "mal"}[veredito]
        caidos = sum(1 for n in g.get("caem", []) if v.get(n) == "FAILED")
        print("  %-28s %-22s %5.1f s  %d/%d cairam"
              % (g["id"], cor(pintura, veredito), gasto, caidos,
                 len(g.get("caem", []))))
        for n in notas:
            print("      %s" % cor(pintura, n))

    # ------------------------------------------------------------ o placar
    conta = {}
    for _, v, _, _ in resultados:
        conta[v] = conta.get(v, 0) + 1
    print("\n" + "=" * 72)
    print("%d guardas: %d provadas, %d redundantes, %d nao pegaram, "
          "%d estragaram, %d quebradas"
          % (len(resultados), conta.get("PROVADA", 0), conta.get("REDUNDANTE", 0),
             conta.get("NAO PEGOU", 0), conta.get("ESTRAGOU", 0),
             conta.get("QUEBRADA", 0)))
    ruins = [(g, v, n) for g, v, _, n in resultados
             if v not in ("PROVADA", "REDUNDANTE")]
    if ruins:
        print("\nO que NAO ficou provado:")
        for g, v, notas in ruins:
            print("  %-28s %s" % (g["id"], v))
            for n in notas:
                print("      %s" % n)
    print("=" * 72)

    if opc.json:
        import json
        with open(opc.json, "w", encoding="utf-8") as f:
            json.dump({
                "quando": time.strftime("%Y-%m-%d %H:%M"),
                "guardas": [
                    {"id": g["id"], "titulo": g["titulo"], "veredito": v,
                     "segundos": round(s, 2), "notas": n}
                    for g, v, s, n in resultados],
            }, f, ensure_ascii=False, indent=2)

    # Codigo de saida honesto: 1 quando alguma guarda nao ficou provada.
    return 0 if not ruins else 1


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except BrokenPipeError:
        # `--listar | head` fecha o cano no meio: sair calado e a unica resposta
        # honesta -- o erro seria do `head`, e nao do executor.
        os._exit(0)
