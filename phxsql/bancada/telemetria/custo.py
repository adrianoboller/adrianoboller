#!/usr/bin/env python3
"""Quanto a TELEMETRIA custa -- desligada e ligada -- e quanto o log cresce.

    flock /tmp/phx-cargo.lock cargo build --release -p phxsql-server --bin phxsqld
    python3 bancada/telemetria/custo.py

Grava `bancada/telemetria/resultados.json`, no molde das outras bancadas: um
`quando` por numero, e nao so um no topo. A pagina de testes e a setima pagina
leem esse arquivo; bancada sem resultado aparece como NAO MEDIDA, com o comando
para rodar, e nunca some da tabela.

# Por que UM servidor so, e nao dois binarios

O `bancada/profiler/custo.py` sobe DOIS servidores porque la o que se mede e o
PORTAO: `if false` contra `if ligado.load(Relaxed)` sao dois binarios
diferentes, e nao ha como ter os dois no mesmo processo. Aqui a pergunta e
outra -- *o que os pontos de captura cobram quando estao ligados* -- e o
interruptor e um `AtomicBool` que as operacoes `telemetria_ligar` e
`telemetria_desligar` viram em tempo de execucao. Entao o mesmo processo, com
os mesmos arquivos e o mesmo cache, mede os dois lados: some do numero toda
diferenca que nao seja o interruptor.

O que esta corrida NAO mede, e esta dito na secao: o custo do portao em si (a
instrucao `load(Relaxed)` de cada ponto com a telemetria desligada). Isso pede
o metodo do Profiler -- recompilar com `if false` -- e e outra bancada.

# Por que EMPARELHADO, e a regra do pedido 155

O trabalho e picado em pedacos curtos que se alternam desligado/ligado,
trocando a ordem a cada volta: um pico de vizinho cai nos dois lados do par.
Reporta-se a mediana das razoes par a par, com o menor e o maior ao lado.

E o custo so e DECLARADO quando o TESTE DE SINAL passa: quantos pares dizem
que ligada custa, contra a moeda honesta, a 3 sigma. Quando nao passa, o
arquivo diz `resolvido: false` e a secao escreve «dentro do ruido» em vez de um
numero -- esta casa ja declarou vencedor dentro do ruido uma vez. Por que o
sinal e nao a faixa min-max do pedido 155 esta em `teste_de_sinal`.

O `--autoteste` prova a porta nos dois sentidos, em segundos e sem subir
servidor: um efeito real passa, ruido simetrico reprova.

Porta 6340, diretorio proprio, e mata SO o PID que subiu -- nunca `pkill`.
"""
import json
import os
import platform
import statistics
import subprocess
import sys
import time

AQUI = os.path.dirname(os.path.abspath(__file__))
RAIZ = os.path.abspath(os.path.join(AQUI, "..", ".."))
sys.path.insert(0, os.path.join(RAIZ, "bancada", "profiler"))

from comum import PHXSQLD, Conexao, baixar, subir  # noqa: E402

PORTA = 6340
BASE = os.path.join(AQUI, "srv-custo")
SAIDA = os.path.join(AQUI, "resultados.json")

# PAR CURTO, e MUITO par -- e a correcao que a primeira corrida obrigou.
#
# A primeira versao deste medidor usava nove pares de 2.000 `ping` cada. Medido
# numa maquina com carga 12 (outras frentes compilando ao lado), a mediana do
# ping DESLIGADO deu 101,5 us contra 63,5 us LIGADO: a telemetria «acelerando»
# o servidor em 37%. Nao acelera nada -- as amostras de um mesmo lado variaram
# de 54 a 128 us, e um par de 0,2 s nao ve a mesma maquina dos dois lados.
#
# O trabalho total e quase o mesmo; o que muda e o GRAO do par. Pares curtos
# poem os dois lados dentro da mesma janela de vizinho, e muitos pares dao ao
# teste de sinal amostra para decidir.
PARES = {"ping": 400, "inserir": 150, "checksum": 80}
PINGS = 200          # pedido puro: sem trava de dados e sem linha
INSERCOES = 100      # pedido + trava de dados, uma linha por pedido
LINHAS_CHECKSUM = 200_000  # o pior caso: uma chamada a `siga()` por linha

CIDADES = ["Blumenau", "Joinville", "Itajai", "Curitiba",
           "Chapeco", "Lages", "Florianopolis", "Criciuma"]


def agora_iso():
    return time.strftime("%Y-%m-%dT%H:%M:%S")


def linhas(base, n):
    return [{"id": base + i, "produto": "Produto %08d" % (base + i),
             "cidade": CIDADES[i % len(CIDADES)], "valor": i * 7}
            for i in range(n)]


def criar(c, tabela):
    c.ok({"op": "criar_tabela", "database": "loja", "tabela": tabela,
          "colunas": [{"nome": "id", "tipo": "Int8", "obrigatoria": True},
                      {"nome": "produto", "tipo": "Str(40)", "obrigatoria": True},
                      {"nome": "cidade", "tipo": "Str(20)"},
                      {"nome": "valor", "tipo": "Int8"}],
          "indices": [{"nome": "porId", "colunas": ["id"], "unico": True,
                       "primario": True}]})


def ligar(c, ligada):
    c.ok({"op": "telemetria_ligar" if ligada else "telemetria_desligar"})


# ------------------------------------------------------------------ as cargas
#
# Cada carga devolve MICROSSEGUNDOS POR PEDIDO -- nunca o tempo total da volta.
# Tempo total nao se compara entre cargas de tamanhos diferentes, e foi a razao
# de a primeira tabela do docs/TELEMETRIA.md ter medido o vizinho: 300
# insercoes davam 87 ms de relogio, e 87 ms nao resolvem 2 us.

def carga_ping(c, k, estado):
    fora = []
    for _ in range(PINGS):
        t = time.perf_counter()
        c.ok({"op": "ping"})
        fora.append((time.perf_counter() - t) * 1e6)
    return fora


def carga_inserir(c, k, estado):
    # Uma tabela por LADO, criada uma vez, e nao uma por par: criar tabela nao
    # e o que se mede, e as duas crescem no mesmo passo -- o `.reg` do lado
    # ligado nunca fica maior que o do desligado, que seria um vies a favor de
    # quem tem menos linha.
    tab = "ins_" + estado
    fora = []
    for l in linhas(1 + k * INSERCOES, INSERCOES):
        t = time.perf_counter()
        c.ok({"op": "inserir", "database": "loja", "tabela": tab, "linha": l})
        fora.append((time.perf_counter() - t) * 1e6)
    return fora


def carga_checksum(c, k, estado):
    t = time.perf_counter()
    c.ok({"op": "checksum", "database": "loja", "tabela": "grande"})
    return [(time.perf_counter() - t) * 1e6 / LINHAS_CHECKSUM]


# (chave, titulo, unidade, funcao, unidades_por_lado, PEDIDOS_por_lado)
#
# As duas ultimas sao coisas diferentes, e confundi-las ja custou um numero
# publicado: o `checksum` mede 200.000 LINHAS num unico PEDIDO, e usar as
# linhas como divisor do log deu «32 milhoes de operacoes» numa corrida que
# mandou 191 mil. Unidade e o que divide o tempo; pedido e o que o log conta.
CARGAS = [
    ("ping", "<code>ping</code> — só o pedido: sem trava de dados e sem linha",
     "µs/pedido", carga_ping, PINGS, PINGS),
    ("inserir", "<code>inserir</code> — pedido mais a trava de dados, uma "
     "linha por pedido", "µs/pedido", carga_inserir, INSERCOES, INSERCOES),
    ("checksum", "<code>checksum</code> — o pior caso: uma chamada a "
     "<code>siga()</code> por linha", "µs/linha", carga_checksum,
     LINHAS_CHECKSUM, 1),
]


def piso(valores):
    """O DECIMO percentil de um lado do par -- o estimador do custo aditivo.

    # Por que o piso, e nao a mediana

    O que a telemetria acrescenta a um pedido e uma constante: dois
    `Instant::now()`, um `fetch_add`, um `lock`. Num tempo `t_i = base_i + c`,
    o `c` aparece inteiro no MENOR `t_i` -- e o menor e o unico que quase nao
    tem vizinho dentro. A mediana, numa maquina com carga 13, mede o vizinho:
    a primeira corrida desta bancada viu a mediana do `ping` andar de 59 us
    para 161 us entre duas corridas de dois minutos de diferenca, sem uma
    linha de codigo mudar.

    P10 e nao o minimo cru: o minimo de 200 amostras e uma amostra so, e uma
    amostra nao tem mediana nem faixa. O p10 e o piso com dez por cento de
    folga contra o outlier para baixo (relogio grosso, coalescencia de
    escrita).
    """
    v = sorted(valores)
    return v[len(v) // 10]


def resumo(valores):
    v = sorted(valores)
    return {"mediana": round(statistics.median(v), 4),
            "min": round(v[0], 4), "max": round(v[-1], 4),
            "p10": round(v[len(v) // 10], 4),
            "p90": round(v[(9 * len(v)) // 10], 4)}


def carga_da_maquina():
    """A carga do sistema, lida do proprio SO -- contexto, nao enfeite.

    Estas sondas medem diferencas de 1% a 3% num servidor local. Um vizinho
    ocupando os mesmos nucleos muda o numero mais que a mudanca medida, e
    quem le o resultado tem de saber em que condicao ele foi tirado sem ter
    de acreditar na palavra de ninguem.
    """
    try:
        with open("/proc/loadavg", encoding="utf-8") as f:
            return round(float(f.read().split()[0]), 2)
    except OSError:
        return None


def teste_de_sinal(diferencas):
    """Quantos pares dizem que LIGADA custa, e isso decide algo?

    # Por que o sinal, e nao «as faixas nao se cruzam»

    A regra do pedido 155 -- so contornar o vencedor quando as faixas min-max
    nao se cruzam -- foi escrita para barras de bancadas cujo efeito e de
    vezes, nao de por cento. Aqui o efeito medido e da ordem de 1%, e o ruido
    de uma maquina compartilhada chega a 3x: as faixas SEMPRE se cruzam, e a
    regra crua diria «nao sei» para sempre, inclusive se a telemetria
    dobrasse o custo.

    O que o par emparelhado da, e a faixa joga fora, e o SINAL: com ruido
    simetrico, «ligada nao custa nada» prediz metade dos pares para cada lado.
    Contar os pares a favor e um teste de sinal, e ele resiste a cauda pesada
    -- um par de 3x conta um voto, igual a um par de 1,01x.

    Reprova a 3 sigma da moeda honesta (sigma = 0,5*raiz(N)): e o mesmo rigor
    da regra das faixas, cobrado numa estatistica que o ruido desta maquina
    nao afoga.

    Recebe as DIFERENCAS por par (ligada menos desligada), e nao as razoes, de
    proposito: a estimativa publicada e a mediana dessas mesmas diferencas,
    entao o veredito e o numero nunca podem discordar. Discordaram numa
    corrida -- 231 de 400 pares a favor com a estimativa em -0,04% -- e
    veredito que aponta para um lado com o numero apontando para o outro e
    pior que veredito nenhum.
    """
    n = len(diferencas)
    a_favor = sum(1 for x in diferencas if x > 0)
    limite = 1.5 * (n ** 0.5)          # 3 sigma de uma binomial(n, 0,5)
    return {"pares": n, "a_favor_de_custar": a_favor,
            "limite_3_sigma": round(n / 2 + limite, 2),
            "resolvido": abs(a_favor - n / 2) > limite}


def autoteste():
    """Prova a PORTA nos dois sentidos, sem subir servidor nenhum.

    Teste que passa por engano e pior que teste que falta, e um veredito que
    so foi exercitado com o caso bom nunca provou que sabe dizer nao. Aqui os
    dois lados sao explicitos: um efeito real tem de RESOLVER, e ruido
    simetrico tem de ficar «dentro do ruido».
    """
    # Efeito real: 55 dos 60 pares dizem que ligada custa.
    real = [0.2] * 55 + [-0.1] * 5
    assert teste_de_sinal(real)["resolvido"], teste_de_sinal(real)

    # Ruido simetrico: metade para cada lado, e um deles com uma cauda enorme
    # -- o par gigante conta UM voto, que e a razao de o sinal existir.
    ruido = [60.0] + [0.01] * 29 + [-0.01] * 30
    assert not teste_de_sinal(ruido)["resolvido"], teste_de_sinal(ruido)

    # A borda: 3 sigma de 60 pares sao 11,6 votos de folga -- 42 resolvem, 41
    # nao. Sem esta linha, um `>` trocado por `>=` ficaria invisivel.
    assert teste_de_sinal([0.2] * 42 + [-0.2] * 18)["resolvido"]
    assert not teste_de_sinal([0.2] * 41 + [-0.2] * 19)["resolvido"]

    # O veredito e a estimativa saem da MESMA lista: sempre que o sinal diz
    # «custa», a mediana das diferencas e positiva. Foi o defeito de uma
    # corrida (231/400 a favor com -0,04% ao lado) e e o que esta linha trava.
    for amostra in (real, ruido, [0.2] * 42 + [-0.2] * 18):
        s = teste_de_sinal(amostra)
        med = statistics.median(amostra)
        if s["resolvido"] and s["a_favor_de_custar"] > len(amostra) / 2:
            assert med > 0, (s, med)

    print("autoteste: ok -- efeito real resolve, ruido simetrico nao, borda "
          "dos 3 sigma confere, e veredito e estimativa nunca discordam")


def medir(c):
    """Alterna desligada/ligada par a par e devolve uma carga por chave."""
    cargas = []
    for chave, titulo, unidade, faz, n, _pedidos in CARGAS:
        pisos = {"desligada": [], "ligada": []}
        medianas = {"desligada": [], "ligada": []}
        diferencas = []
        for k in range(PARES[chave]):
            # A ordem alterna: medir sempre o desligado primeiro daria a ele o
            # cache frio de todo par, e o numero sairia a favor do ligado.
            if k % 2 == 0:
                ligar(c, False)
                d = faz(c, k, "off")
                ligar(c, True)
                g = faz(c, k, "on")
            else:
                ligar(c, True)
                g = faz(c, k, "on")
                ligar(c, False)
                d = faz(c, k, "off")
            pisos["desligada"].append(piso(d))
            pisos["ligada"].append(piso(g))
            medianas["desligada"].append(statistics.median(d))
            medianas["ligada"].append(statistics.median(g))
            # A diferenca e' DENTRO do par: ela cancela o nivel de contencao
            # daquela janela, que e' o que varia de par para par.
            diferencas.append(piso(g) - piso(d))
        sinal = teste_de_sinal(diferencas)
        d, g = resumo(pisos["desligada"]), resumo(pisos["ligada"])
        custo = statistics.median(diferencas)
        cargas.append({
            "chave": chave,
            "titulo": titulo,
            "unidade": unidade,
            "unidades_por_lado": n,
            "desligada": d,
            "ligada": g,
            "sob_carga": {
                "desligada": round(statistics.median(medianas["desligada"]), 4),
                "ligada": round(statistics.median(medianas["ligada"]), 4),
            },
            "custo": round(custo, 4),
            "custo_faixa": resumo(diferencas),
            "custo_pct": round(100.0 * custo / d["mediana"], 2),
            "sinal": sinal,
            "resolvido": sinal["resolvido"],
            "carga_da_maquina": carga_da_maquina(),
            "quando": agora_iso(),
        })
        marca = "" if sinal["resolvido"] else "   (dentro do ruido)"
        print("  %-9s piso desligada %9.3f  custo %+8.4f (%+6.2f%%)  "
              "%d/%d pares a favor%s"
              % (chave, d["mediana"], custo, 100.0 * custo / d["mediana"],
                 sinal["a_favor_de_custar"], sinal["pares"], marca),
              flush=True)
    return cargas


def contar_operacoes():
    """Quantos pedidos esta corrida mandou -- contados da propria receita.

    Nao se digita: se PARES ou uma carga mudar, o divisor do log muda junto.
    Um numero copiado aqui envelheceria na primeira mexida em PINGS.
    """
    total = 0
    for chave, _titulo, _un, _faz, _n, pedidos in CARGAS:
        # dois lados por par, mais os dois liga/desliga de cada par
        total += PARES[chave] * (2 * pedidos + 2)
    # o `telemetria` do inventario das threads, que roda antes de o tamanho do
    # log ser lido. O preparo (criar tabela, semear o checksum) fica de FORA:
    # ele acontece antes de o tamanho de abertura ser tirado.
    return total + 1


def tamanho_do_log():
    p = os.path.join(BASE, "acessos.log")
    return os.path.getsize(p) if os.path.exists(p) else 0


def threads_e_pontos(c):
    """O inventario das threads, lido do proprio servidor.

    Nao vem de uma lista digitada aqui: `docs/TELEMETRIA.md` §5 ja teve a
    contagem a mao, e thread nova nao avisa documento nenhum.
    """
    r = c.ok({"op": "telemetria"})
    fios = r.get("threads") or []
    servico = sum(1 for f in fios if isinstance(f, dict)
                  and f.get("familia") == "servico")
    return {"registradas": len(fios), "de_servico": servico,
            "efemeras": len(fios) - servico, "quando": agora_iso()}


def versao():
    r = subprocess.run([PHXSQLD, "--version"], capture_output=True, text=True)
    return (r.stdout or r.stderr).strip()


def main():
    if not os.path.exists(PHXSQLD):
        sys.exit("falta %s -- rode: flock /tmp/phx-cargo.lock cargo build "
                 "--release -p phxsql-server --bin phxsqld" % PHXSQLD)
    print("=== o custo da telemetria, medido por soquete na porta %d ===" % PORTA)
    p = subir(BASE, PORTA)
    try:
        c = Conexao(PORTA)
        c.entrar("adm", "senha-do-adm")
        c.ok({"op": "criar_database", "database": "loja"})
        criar(c, "grande")
        criar(c, "ins_off")
        criar(c, "ins_on")
        print("  semeando %s linhas para o checksum ..." % LINHAS_CHECKSUM,
              flush=True)
        for i in range(0, LINHAS_CHECKSUM, 20_000):
            c.ok({"op": "inserir_lote", "database": "loja", "tabela": "grande",
                  "linhas": linhas(i + 1, 20_000)})
        log_antes = tamanho_do_log()
        cargas = medir(c)
        fios = threads_e_pontos(c)
        log_depois = tamanho_do_log()
        c.fechar()
    finally:
        baixar(p)

    ops = contar_operacoes()
    dados = {
        "quando": agora_iso(),
        "versao": versao(),
        "maquina": "%s %s, %d nucleos" % (platform.system(), platform.machine(),
                                          os.cpu_count() or 0),
        "metodo": "um servidor so, interruptor virado por `telemetria_ligar`/"
                  "`telemetria_desligar` a cada par; ordem alternada a cada "
                  "par; o custo e a mediana das DIFERENCAS por par, medidas "
                  "no piso (p10) de cada lado, com teste de sinal a 3 sigma",
        "pares": dict(PARES),
        "carga_da_maquina": carga_da_maquina(),
        "linhas_no_checksum": LINHAS_CHECKSUM,
        "cargas": cargas,
        "log": {
            "arquivo": "acessos.log",
            "bytes": log_depois - log_antes,
            "operacoes": ops,
            "bytes_por_operacao": round((log_depois - log_antes) / ops, 4),
            "quando": agora_iso(),
        },
        "threads": fios,
    }
    with open(SAIDA, "w", encoding="utf-8") as f:
        json.dump(dados, f, indent=1, ensure_ascii=False)
        f.write("\n")
    print("\nresultado gravado: %s" % os.path.relpath(SAIDA, RAIZ))
    resolvidas = sum(1 for c in cargas if c["resolvido"])
    print("  %d de %d cargas resolveram o custo; as outras ficam como «dentro "
          "do ruido», e e assim que a secao as publica"
          % (resolvidas, len(cargas)))
    return 0


if __name__ == "__main__":
    if "--autoteste" in sys.argv:
        autoteste()
        sys.exit(0)
    sys.exit(main())
