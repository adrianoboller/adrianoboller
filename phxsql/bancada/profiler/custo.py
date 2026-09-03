#!/usr/bin/env python3
"""Quanto custa o Profiler: DESLIGADO, ligado, e o portão que decide isso.

    cargo build --release -p phxsql-server --bin phxsqld
    python3 bancada/profiler/custo.py
    python3 bancada/profiler/custo.py --autoteste   # so o veredito, sem compilar

# TRAVADO, e não só medido -- achado do QA-PDCA

Até esta rodada este medidor só imprimia a razão "desligado custa zero?" e
sempre saía zero -- media a pétrea (*"instrumentação desligada tem de custar
zero"*) e não a impunha: uma regressão no ponto de captura não reprovava
bateria nenhuma, e `provar.py` nem chamava este arquivo. Agora
[`falhou_desligado_custa_zero`] julga a mediana medida contra
[`MINIMO_DESLIGADO`] e `main()` sai `1` quando ela reprova -- é o que faz este
arquivo virar um `parte()` de verdade em `provar.py`, e não mais invisível.

O veredito é só sobre o par "atual/sem" (desligado custa zero?): é a metade da
medição que É a garantia pétrea. As outras duas ("o portão vale quanto?" e
"ligado custa quanto?") continuam informativas -- não são regra do CLAUDE.md,
são contexto para quem for investigar uma reprovação.

`--autoteste` prova SÓ o veredito (`falhou_desligado_custa_zero` contra um par
saudável e um degradado, sem compilar nada) -- é o que se roda em segundos
para conferir a lógica do portão sem competir pelo `servidor.rs` com quem
estiver mexendo nele: `compilar()` reescreve o arquivo três vezes, mesmo que
devolva o original no `finally`, e isso é real demais para rodar de
acompanhamento numa árvore compartilhada.

# Por que a medição é EMPARELHADA, e não uma corrida atrás da outra

A primeira versão deste medidor rodava uma variante inteira, depois a outra, e
comparava as medianas. O número saiu **impossível**: o profiler LIGADO
apareceu 1,21× mais rápido que desligado. A causa não estava no servidor — era
um vizinho ocupando os mesmos quatro núcleos durante metade da corrida. Uma
corrida de cinco minutos não é uma condição; é cinco minutos de condições
diferentes.

Aqui as duas variantes ficam **no ar ao mesmo tempo**, em duas portas, e o
trabalho é picado em pedaços curtos que se alternam entre elas — trocando a
ordem a cada volta. Um pico de carga cai nos dois lados do par, e a razão de
cada par sobrevive a ele. O que se reporta é a **mediana das razões**, com o
menor e o maior ao lado: par a par é o que responde «uma é mais rápida que a
outra?», e a mediana de médias não é.

# As variantes

O portão do ponto de captura é UMA expressão, e é ela que se troca:

    atual   `if self.profiler_ligado.load(Relaxed)`   o portão barato
    sem     `if false`                                o profiler nem existe
    antigo  `if true`                                 o defeito da 0.17.0:
            analisa o corpo inteiro DUAS vezes e só então pergunta se está
            ligado -- e `chegou` devolve None, porque está desligado

Nunca usa pkill: mata só os PIDs que subiu. Portas 6270 e 6272.
"""
import io
import json
import os
import shutil
import statistics
import subprocess
import sys
import time

from comum import PHXSQLD, RAIZ, Conexao, baixar, subir

AQUI = os.path.dirname(os.path.abspath(__file__))
BIN = os.path.join(AQUI, "bin")
SERV = os.path.join(RAIZ, "crates", "phxsql-server", "src", "servidor.rs")
PORTAS = (6270, 6272)
PEDACOS = 15
LOTE = 5_000
UMA_A_UMA = 1_500

PORTAO = "self.profiler_ligado.load(Ordering::Relaxed)"
VARIANTES = [("atual", PORTAO), ("sem", "false"), ("antigo", "true")]

CIDADES = ["Blumenau", "Joinville", "Itajai", "Curitiba",
           "Chapeco", "Lages", "Florianopolis", "Criciuma"]

# Abaixo disto, o Profiler DESLIGADO deixou de custar perto de zero. 0,90
# tolera 10% de ruido de maquina compartilhada (a mesma folga que a nota do
# cabecalho documenta) e ainda pega uma regressao real -- o defeito historico
# da 0.17.0 (que este medidor tambem compara, no par "antigo") custava dois
# `Json::analisar` do corpo inteiro por pedido: bem abaixo de 0,90.
MINIMO_DESLIGADO = 0.90


def falhou_desligado_custa_zero(medido, minimo=MINIMO_DESLIGADO):
    """A TRAVA: julga o "desligado custa zero?" em vez de só imprimi-lo.

    `medido` e o dict que `comparar()` devolve para esse par -- {"lote":
    {"mediana": ..., ...}, "uma": {"mediana": ...}}. Devolve a lista de
    problemas (vazia = passou); quem chama decide o que fazer com ela.

    So este par entra no veredito: e o unico que mede a garantia pétrea
    ("instrumentação desligada tem de custar zero"). Os outros dois que
    `main()` calcula ("o portão vale quanto?", "ligado custa quanto?") sao
    contexto para investigar uma reprovação, não a regra em si.
    """
    problemas = []
    for tipo in ("lote", "uma"):
        mediana = medido[tipo]["mediana"]
        if mediana < minimo:
            problemas.append(
                "%s: o Profiler DESLIGADO rendeu %.3fx do binario sem ele -- "
                "abaixo do minimo %.2fx. Instrumentacao desligada tem de "
                "custar perto de zero (CLAUDE.md)." % (tipo, mediana, minimo))
    return problemas


def autoteste():
    """Prova a PROPRIA porta, sem compilar nada: um par saudavel passa, um
    par degradado reprova -- as duas metades da prova real, na função de
    veredito e não no medidor inteiro.

    Existe para se rodar sozinho (`--autoteste`) quando compilar as três
    variantes -- que reescreve `servidor.rs` três vezes -- competiria com
    quem estiver mexendo nesse arquivo na mesma árvore.
    """
    saudavel = {"lote": {"mediana": 0.98}, "uma": {"mediana": 1.02}}
    problemas = falhou_desligado_custa_zero(saudavel)
    assert problemas == [], "um par saudavel foi reprovado: %r" % problemas

    # 0,61x e o numero que a rodada anterior mediu para o "antigo" (o dobro de
    # `Json::analisar` por pedido) -- um degradado plausivel, nao inventado.
    degradado = {"lote": {"mediana": 0.61}, "uma": {"mediana": 0.99}}
    problemas = falhou_desligado_custa_zero(degradado)
    assert problemas, "um par degradado (0,61x no lote) passou sem reprovar"
    assert "lote" in problemas[0], problemas
    assert "uma" not in " ".join(p.split(":")[0] for p in problemas), problemas

    # A borda: exatamente no minimo passa (>=, nao >) -- e um tique abaixo
    # reprova. Sem isto, um erro de < por <= no meio ficaria invisivel.
    assert falhou_desligado_custa_zero({"lote": {"mediana": MINIMO_DESLIGADO},
                                        "uma": {"mediana": 1.0}}) == []
    assert falhou_desligado_custa_zero(
        {"lote": {"mediana": MINIMO_DESLIGADO - 0.001}, "uma": {"mediana": 1.0}})

    print("autoteste: ok -- par saudavel passa, par degradado reprova, "
          "borda do minimo (%.2fx) confere" % MINIMO_DESLIGADO)


def compilar():
    """Compila as três variantes A PARTIR DA ÁRVORE DE AGORA.

    Binário velho mede o passado, e a bancada já foi enganada por isso: uma
    rodada inteira de ganhos ficou invisível porque o executável era de antes.
    """
    os.makedirs(BIN, exist_ok=True)
    original = io.open(SERV, encoding="utf-8").read()
    assert original.count(PORTAO) == 2, original.count(PORTAO)
    try:
        for nome, portao in VARIANTES:
            io.open(SERV, "w", encoding="utf-8").write(
                original.replace(PORTAO, portao))
            print("  compilando %s ..." % nome, flush=True)
            r = subprocess.run(
                ["cargo", "build", "--release", "--offline",
                 "-p", "phxsql-server", "--bin", "phxsqld"],
                cwd=RAIZ, capture_output=True, text=True)
            if r.returncode:
                sys.exit(r.stdout + r.stderr)
            shutil.copy2(PHXSQLD, os.path.join(BIN, nome))
    finally:
        io.open(SERV, "w", encoding="utf-8").write(original)
    # Devolve o `target/` ao código de verdade: um defeito esquecido lá
    # dentro seria o próximo binário velho.
    subprocess.run(["cargo", "build", "--release", "--offline",
                    "-p", "phxsql-server", "--bin", "phxsqld"],
                   cwd=RAIZ, capture_output=True, text=True)


def linhas(base, n):
    return [{"id": base + i, "produto": "Produto %08d" % (base + i),
             "cidade": CIDADES[i % len(CIDADES)], "valor": i * 7}
            for i in range(n)]


def preparar(c, guardar):
    c.entrar("adm", "senha-do-adm")
    c.ok({"op": "criar_database", "database": "loja"})
    if guardar:
        c.ok({"op": "profiler_ligar", "guardar": guardar})


def criar(c, tabela):
    c.ok({"op": "criar_tabela", "database": "loja", "tabela": tabela,
          "colunas": [{"nome": "id", "tipo": "Int8", "obrigatoria": True},
                      {"nome": "produto", "tipo": "Str(40)", "obrigatoria": True},
                      {"nome": "cidade", "tipo": "Str(20)"},
                      {"nome": "valor", "tipo": "Int8"}],
          "indices": [{"nome": "porId", "colunas": ["id"], "unico": True,
                       "primario": True},
                      {"nome": "porCidade", "colunas": ["cidade"]}]})


def pedaco_lote(c, k):
    tab = "lote%d" % k
    criar(c, tab)
    ls = linhas(1, LOTE)
    t = time.perf_counter()
    c.ok({"op": "inserir_lote", "database": "loja", "tabela": tab,
          "linhas": ls})
    s = time.perf_counter() - t
    assert c.ok({"op": "verificar", "database": "loja",
                 "tabela": tab})["registros"] == LOTE
    return LOTE / s


def pedaco_uma(c, k):
    tab = "uma%d" % k
    criar(c, tab)
    ls = linhas(1, UMA_A_UMA)
    t = time.perf_counter()
    for l in ls:
        c.ok({"op": "inserir", "database": "loja", "tabela": tab, "linha": l})
    s = time.perf_counter() - t
    assert c.ok({"op": "verificar", "database": "loja",
                 "tabela": tab})["registros"] == UMA_A_UMA
    return UMA_A_UMA / s


def comparar(rotulo, a, b, guardar_a=0, guardar_b=0):
    """`a` e `b` são (nome, binário). Devolve as razões a/b, par a par."""
    (nome_a, bin_a), (nome_b, bin_b) = a, b
    pa = subir(os.path.join(AQUI, "srv-a"), PORTAS[0], binario=bin_a)
    pb = subir(os.path.join(AQUI, "srv-b"), PORTAS[1], binario=bin_b)
    try:
        ca, cb = Conexao(PORTAS[0]), Conexao(PORTAS[1])
        preparar(ca, guardar_a)
        preparar(cb, guardar_b)
        razoes = {"lote": [], "uma": []}
        for k in range(PEDACOS):
            for tipo, faz in (("lote", pedaco_lote), ("uma", pedaco_uma)):
                # A ordem alterna: sempre medir A primeiro daria a ele o
                # cache frio de toda volta.
                if k % 2 == 0:
                    va, vb = faz(ca, k), faz(cb, k)
                else:
                    vb, va = faz(cb, k), faz(ca, k)
                razoes[tipo].append(va / vb)
        ca.fechar()
        cb.fechar()
    finally:
        baixar(pa)
        baixar(pb)
    saida = {}
    print("\n=== %s ===" % rotulo)
    print("   %s / %s, %d pares" % (nome_a, nome_b, PEDACOS))
    for tipo in ("lote", "uma"):
        r = sorted(razoes[tipo])
        med = statistics.median(r)
        saida[tipo] = {"mediana": round(med, 4),
                       "min": round(r[0], 4), "max": round(r[-1], 4),
                       "pares": [round(x, 4) for x in r]}
        print("   %-5s  mediana %6.3fx   (menor %5.3f, maior %5.3f)"
              % (tipo, med, r[0], r[-1]))
    return saida


def main():
    if "--sem-compilar" not in sys.argv:
        print("=== compilando as três variantes da árvore de agora ===")
        compilar()
    atual = ("atual", os.path.join(BIN, "atual"))
    sem = ("sem", os.path.join(BIN, "sem"))
    antigo = ("antigo", os.path.join(BIN, "antigo"))

    tudo = {}
    tudo["desligado custa zero?"] = comparar(
        "o Profiler DESLIGADO custa alguma coisa? (1,00 = custo zero)",
        atual, sem)
    tudo["o portao vale quanto?"] = comparar(
        "o portão barato contra o defeito da 0.17.0 (>1,00 = o portão ganha)",
        atual, antigo)
    tudo["ligado custa quanto?"] = comparar(
        "ligado contra desligado, mesmo binário (<1,00 = ligar custa)",
        ("atual ligado", os.path.join(BIN, "atual")),
        ("atual desligado", os.path.join(BIN, "atual")),
        guardar_a=500)
    json.dump(tudo, open(os.path.join(AQUI, "custo.json"), "w"), indent=1)
    print("\nRESULTADO " + json.dumps(tudo))

    # A TRAVA. Ate aqui era so o que "RESULTADO" imprime -- nenhuma linha
    # decidia nada, e uma regressao no ponto de captura nao reprovava bateria
    # nenhuma. Ver `falhou_desligado_custa_zero`.
    problemas = falhou_desligado_custa_zero(tudo["desligado custa zero?"])
    if problemas:
        print("\nREPROVADO -- instrumentacao desligada nao custa perto de "
              "zero:")
        for p in problemas:
            print("  " + p)
        sys.exit(1)


if __name__ == "__main__":
    if "--autoteste" in sys.argv:
        autoteste()
        sys.exit(0)
    main()
