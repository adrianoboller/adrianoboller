#!/usr/bin/env python3
"""PhxJev: o limiar, o formato e o registro saem daqui, nao do juiz.

Exercitado ao vivo, o juiz acertou a resposta e fugiu do formato (escreveu
`conf: alto` e omitiu a linha da calibracao). Por isso o Jev aplica limiar em
codigo: o juiz so devolve probabilidades em JSON, e este script valida, calcula
a confianca, decide o veredito, registra e mede a calibracao. So a biblioteca
padrao.

Uso:
  phxjev.py veredito  < julgamento.json
  phxjev.py desfecho  <id> <pergunta> <valor>   # 1/0 para noul, rotulo para choice, degrau para score
  phxjev.py colher    <PENDENCIAS.md>            # desfechos que o fechamento do pedido prova
  phxjev.py local     <modelo> < perguntas.json  # juiz local (Ollama), uma passada por pergunta
  phxjev.py auto      <modelo> < perguntas.json  # local; o incerto sobe ao Claude
  phxjev.py juiz      [claude|local|auto [modelo]]  # o que vale, e por que (a bancada decide)
  phxjev.py historico [juiz]                     # a calibracao do juiz, para ele ler antes
  phxjev.py calibrar
  phxjev.py mostrar   <selo>                     # a saida original, para conferir se foi editada
"""
import hashlib
import io
import json
import math
import os
import subprocess
import sys
import time

def raiz():
    """Raiz do git de onde se roda; sem git, o diretorio atual.

    Nao ha variavel de ambiente de proposito: exercitado ao vivo, o juiz
    apontou o registro para o proprio scratchpad, e os desfechos que a
    calibracao precisa se perderiam com a sessao.
    """
    try:
        r = subprocess.run(["git", "rev-parse", "--show-toplevel"],
                           capture_output=True, text=True, check=True)
        return r.stdout.strip()
    except (OSError, subprocess.CalledProcessError):
        return os.getcwd()


REGISTRO = os.path.join(raiz(), ".phxjev", "registro.jsonl")
MINIMO_PARA_CALIBRAR = 50

# A tabela da secao 4 da skill. Mudou aqui, muda la no mesmo commit.
LIMIAR_REAL = 0.50
LIMIAR_ALCANCAVEL = 0.30
LIMIAR_JA_TRATADO = 0.70
LIMIAR_DEFEITO_ATIVO = 0.50
LIMIAR_FERE_PETREA = 0.30
LIMIAR_CONF = 0.40
LIMIAR_MARGEM = 0.15
LIMIAR_SEVERIDADE = 2.0


class Invalido(Exception):
    pass


def entropia(ps):
    return -sum(p * math.log(p) for p in ps if p > 0)


def conf_de(ps):
    """1 - entropia normalizada: concentracao, nao acerto."""
    if len(ps) < 2:
        return 1.0
    return 1.0 - entropia(ps) / math.log(len(ps))


def conferir_p(nome, p):
    if not isinstance(p, (int, float)) or not 0.0 <= p <= 1.0:
        raise Invalido(f"{nome}: p fora de [0,1]: {p!r}")


def normalizar(item_id, nome, q):
    """Devolve (tipo, distribuicao dict rotulo->p, conf, valor resumido)."""
    tipo = q.get("tipo")
    evid = (q.get("evid") or "").strip()
    rotulo = f"{item_id}.{nome}"
    if tipo == "noul":
        p = q.get("p")
        conferir_p(rotulo, p)
        # Sem evidencia, a unica resposta honesta e nao sei.
        if not evid and abs(p - 0.5) > 1e-9:
            raise Invalido(f"{rotulo}: p={p} sem evidencia; sem evidencia e 0.5")
        dist = {"sim": p, "nao": 1 - p}
        return tipo, dist, conf_de(list(dist.values())), p
    if tipo in ("choice", "score"):
        dist = q.get("p")
        if not isinstance(dist, dict) or len(dist) < 2:
            raise Invalido(f"{rotulo}: {tipo} precisa de p por opcao (>=2)")
        for k, v in dist.items():
            conferir_p(f"{rotulo}.{k}", v)
        soma = sum(dist.values())
        if abs(soma - 1.0) > 0.01:
            raise Invalido(f"{rotulo}: probabilidades somam {soma:.3f}, nao 1")
        if not evid:
            raise Invalido(f"{rotulo}: {tipo} sem evidencia")
        c = conf_de(list(dist.values()))
        if tipo == "score":
            try:
                valor = sum(float(k) * v for k, v in dist.items())
            except ValueError:
                raise Invalido(f"{rotulo}: degraus de score tem de ser numeros")
        else:
            valor = max(dist, key=dist.get)
        return tipo, dist, c, valor
    raise Invalido(f"{rotulo}: tipo desconhecido {tipo!r}")


def decidir(perguntas):
    """Aplica a tabela na ordem; devolve (veredito, motivos, escalar)."""
    escalar = []
    g = perguntas.get
    for nome, lim, maior in (
        ("real", LIMIAR_REAL, False),
        ("alcancavel", LIMIAR_ALCANCAVEL, False),
        ("ja_tratado", LIMIAR_JA_TRATADO, True),
    ):
        if nome in perguntas:
            p = g(nome)["valor"]
            if (p > lim) if maior else (p < lim):
                sinal = ">" if maior else "<"
                return f"DESCARTAR ({nome}{sinal}{lim:.2f})", escalar

    partes = []
    for nome, q in perguntas.items():
        if q["tipo"] == "noul" and nome.startswith("fere_petrea") and q["valor"] >= LIMIAR_FERE_PETREA:
            escalar.append(f"{nome}: choque com petrea")
        if q["tipo"] == "choice":
            ordem = sorted(q["dist"].values(), reverse=True)
            if q["conf"] < LIMIAR_CONF or ordem[0] - ordem[1] < LIMIAR_MARGEM:
                escalar.append(f"{nome}: empate")
            else:
                partes.append(f"{nome}={q['valor']}")

    if "defeito_ativo" in perguntas:
        q = g("defeito_ativo")
        if q["valor"] >= LIMIAR_DEFEITO_ATIVO or q["conf"] < LIMIAR_CONF:
            partes.insert(0, "MANTER ☐")
        else:
            partes.insert(0, "MANTER ⏸")
    if "severidade" in perguntas:
        q = g("severidade")
        # Severidade espalhada pela regua nao e severidade: e «nao sei». Ela
        # nao some calada (as partes do 276 sairam com conf 0,16 a 0,28 sem
        # nenhum aviso) e, perto do limiar, bloqueia com interrogacao.
        incerta = q["conf"] < LIMIAR_CONF
        if incerta:
            escalar.append(f"severidade: incerta (conf {q['conf']:.2f})")
        if q["valor"] >= LIMIAR_SEVERIDADE:
            partes.append("bloqueia?" if incerta else "bloqueia")
    for nome, q in perguntas.items():
        if q["tipo"] == "noul" and nome not in ("real", "alcancavel", "ja_tratado") \
                and not nome.startswith("fere_petrea") and nome != "defeito_ativo":
            partes.append(f"{nome}={'sim' if q['valor'] >= 0.5 else 'nao'}")
    if not partes and not escalar and "real" in perguntas:
        partes.append("MANTER")
    return " ".join(partes) or "ESCALAR", escalar


def fmt(nome, q):
    if q["tipo"] == "noul":
        if "valor_cru" in q:
            return f"{nome} {q['valor_cru']:.2f}→{q['valor']:.2f}"
        return f"{nome} {q['valor']:.2f}"
    if q["tipo"] == "score":
        topo = max(float(k) for k in q["dist"])
        return f"{nome} {q['valor']:.2f}/{topo:g} (conf {q['conf']:.2f})"
    dist = " ".join(f"{k}:{v:.2f}" for k, v in sorted(q["dist"].items(), key=lambda kv: -kv[1]))
    return f"{nome} [{dist}] (conf {q['conf']:.2f})"


def cmd_veredito(entrada, saida, registro=REGISTRO, juiz="claude"):
    j = json.loads(entrada)
    estado = j.get("estado") or []
    if not estado:
        raise Invalido("estado vazio: julgamento sem nada lido nao se registra")
    preset = j.get("preset", "perguntar")
    carimbo = time.strftime("%Y%m%d-%H%M%S")
    linhas = [
        f"PhxJev · {preset} · juiz {juiz} · calibracao: {estado_calibracao(registro, juiz)}",
        f"estado: {len(estado)} trechos lidos ({', '.join(estado[:4])}{', ...' if len(estado) > 4 else ''})",
        "─" * 60,
    ]
    registros, escalar_tudo = [], []
    for n, item in enumerate(j.get("itens") or [], 1):
        iid = item.get("id") or f"i{n}"
        pergs = {}
        for nome, q in (item.get("perguntas") or {}).items():
            tipo, dist, c, valor = normalizar(iid, nome, q)
            pergs[nome] = {"tipo": tipo, "dist": dist, "conf": c, "valor": valor, "evid": q.get("evid", "")}
            a = ajuste(registro, juiz, nome) if tipo == "noul" else None
            if a:
                # O limiar decide pela p corrigida; o registro guarda a crua,
                # porque e a crua que o proximo ajuste precisa para se medir.
                corr = aplicar_ajuste(a, valor)
                pergs[nome].update(valor_cru=valor, valor=corr, conf=conf_de([corr, 1 - corr]))
        if not pergs:
            raise Invalido(f"{iid}: nenhuma pergunta")
        veredito, escalar = decidir(pergs)
        linhas.append(f"{iid}  " + "  ".join(fmt(k, v) for k, v in pergs.items()) + f"  → {veredito}")
        if item.get("motivo"):
            linhas.append(f"      motivo: {item['motivo']}")
        escalar_tudo += [f"{iid}.{e}" for e in escalar]
        registros.append({
            "id": f"{carimbo}-{iid}", "preset": preset, "juiz": juiz, "item": iid, "veredito": veredito,
            "perguntas": {k: {"tipo": v["tipo"], "dist": v["dist"], "conf": round(v["conf"], 4)} for k, v in pergs.items()},
            "desfecho": {},
        })
    if not registros:
        raise Invalido("nenhum item")
    linhas.append("─" * 60)
    linhas.append("escalar: " + ("; ".join(escalar_tudo) if escalar_tudo else "nada"))
    linhas.append(f"registro: {len(registros)} em {registro} (desfecho pendente)")
    # O juiz ja encurtou esta saida uma vez apresentando-a como intacta. O
    # selo nao impede editar; torna a edicao conferivel por `mostrar`.
    texto = "\n".join(linhas)
    selo = hashlib.sha256(texto.encode("utf-8")).hexdigest()[:12]
    for r in registros:
        r["selo"] = selo
    registros[0]["saida"] = texto
    gravar(registro, registros)
    saida.write(texto + f"\nselo: {selo} (phxjev.py mostrar {selo})\n")


def linhas_do_registro(registro):
    if not os.path.exists(registro):
        return []
    with open(registro, encoding="utf-8") as f:
        return [json.loads(l) for l in f if l.strip()]


def ler(registro):
    """Vereditos com os desfechos aplicados.

    O registro e so de acrescimo, como o `.reg`: um desfecho e uma linha nova
    que aponta para o veredito, nunca reescrita do arquivo. Reescrever
    conflitaria no git na primeira rodada com duas frentes. Desfecho gravado
    dentro do veredito (o formato de antes) continua valendo.
    """
    vereditos, ordem = {}, []
    for r in linhas_do_registro(registro):
        if r.get("tipo") == "desfecho":
            if r["ref"] in vereditos:
                vereditos[r["ref"]].setdefault("desfecho", {})[r["pergunta"]] = r["valor"]
            continue
        r.setdefault("desfecho", {})
        vereditos[r["id"]] = r
        ordem.append(r["id"])
    return [vereditos[i] for i in ordem]


def gravar(registro, novos):
    os.makedirs(os.path.dirname(registro) or ".", exist_ok=True)
    with open(registro, "a", encoding="utf-8") as f:
        for r in novos:
            f.write(json.dumps(r, ensure_ascii=False) + "\n")


def cmd_desfecho(rid, pergunta, valor, registro=REGISTRO, fonte="manual"):
    alvo = [r for r in ler(registro) if r["id"] == rid]
    if not alvo:
        raise Invalido(f"id {rid} nao esta no registro")
    q = alvo[0]["perguntas"].get(pergunta)
    if q is None:
        raise Invalido(f"{rid} nao tem a pergunta {pergunta}")
    if q["tipo"] == "noul":
        if valor not in ("0", "1"):
            raise Invalido("desfecho de noul e 1 (verdade) ou 0")
        valor = "sim" if valor == "1" else "nao"
    elif valor not in q["dist"]:
        raise Invalido(f"desfecho {valor!r} nao e opcao de {pergunta}: {sorted(q['dist'])}")
    gravar(registro, [{"tipo": "desfecho", "ref": rid, "pergunta": pergunta, "valor": valor,
                       "fonte": fonte, "quando": time.strftime("%Y%m%d-%H%M%S")}])
    return f"{rid}.{pergunta} = {valor}"


def fechamento_do_pedido(num, pendencias):
    """Unix time do commit que pos o pedido em ☑️, ou None se nao esta ☑️."""
    # Absoluto: com caminho relativo o git roda no diretorio do arquivo, nao
    # acha o pathspec, e o «nao comitado = agora» fazia TODO pedido ja ☑️
    # parecer fechado depois do veredito -- desfecho falso, achado na revisao.
    pendencias = os.path.abspath(pendencias)
    marca = f"| ☑️ | {num} |"
    with open(pendencias, encoding="utf-8") as f:
        if not any(l.startswith(marca) for l in f):
            return None
    r = subprocess.run(["git", "log", "--format=%ct %h", "-S", marca, "--", pendencias],
                       capture_output=True, text=True, cwd=os.path.dirname(pendencias) or ".")
    if r.returncode != 0:
        raise Invalido(f"git log falhou em {pendencias}: {r.stderr.strip()}")
    linhas = r.stdout.split()
    # O mais recente: pedido reaberto e fechado de novo conta pelo ultimo
    # fechamento. Data de COMMIT, nao de autor -- e quando entrou na branch.
    # A mudanca que ainda nao foi comitada vale como agora.
    return (int(linhas[0]), linhas[1]) if linhas else (int(time.time()), "nao-comitado")


def cmd_colher(pendencias, registro=REGISTRO):
    """Desfechos que o proprio PENDENCIAS.md prova, sem conferencia a mao.

    So colhe o caso sem ambiguidade: item cujo id comeca pelo numero do pedido
    SEM letra (276b nao diz qual parte o fechamento fechou), pedido que virou
    ☑️ DEPOIS do veredito (entao o defeito existia quando o juiz olhou e foi
    consertado: real=1, ja_tratado=0), e pergunta ainda sem desfecho.
    Pedido ja ☑️ antes do veredito nao prova nada e fica de fora.
    """
    import re
    feitos, pulados = [], 0
    for r in ler(registro):
        m = re.match(r"^(\d+)-", r.get("item", ""))
        if not m:
            continue
        fech = fechamento_do_pedido(m.group(1), pendencias)
        if fech is None:
            continue
        quando_veredito = time.mktime(time.strptime(r["id"][:15], "%Y%m%d-%H%M%S"))
        if fech[0] < quando_veredito:
            pulados += 1
            continue
        for pergunta, valor in (("real", "1"), ("ja_tratado", "0")):
            if pergunta in r["perguntas"] and pergunta not in r["desfecho"]:
                feitos.append(cmd_desfecho(r["id"], pergunta, valor, registro, f"colhido:{fech[1]}"))
    return feitos, pulados


def juiz_de(r):
    # Veredito de antes do campo e do Claude: o juiz local nasceu depois dele.
    return r.get("juiz", "claude")


def pares(registro, so=None, juiz=None):
    """(pergunta, p de cada opcao, acerto 0/1) de tudo que tem desfecho."""
    for r in ler(registro):
        if juiz and juiz_de(r) != juiz:
            continue
        for nome, real in r.get("desfecho", {}).items():
            if so and nome != so:
                continue
            for opcao, p in r["perguntas"][nome]["dist"].items():
                yield nome, p, 1 if opcao == real else 0


def contar_desfechos(registro, juiz=None, pergunta=None):
    return sum(1 for r in ler(registro) if not juiz or juiz_de(r) == juiz
               for k in r.get("desfecho", {}) if not pergunta or k == pergunta)


def estado_calibracao(registro, juiz=None):
    n = contar_desfechos(registro, juiz)
    if n < MINIMO_PARA_CALIBRAR:
        return f"nao medida ({n}/{MINIMO_PARA_CALIBRAR} desfechos)"
    return f"medida em {n} desfechos (phxjev.py calibrar)"


def brier(ps):
    return sum((p - y) ** 2 for _, p, y in ps) / len(ps)


MINIMO_PARA_AJUSTAR = 50


def logit(p):
    p = min(max(p, 1e-4), 1 - 1e-4)
    return math.log(p / (1 - p))


def sigmoide(x):
    # Estavel nos dois lados: exp de numero grande estourava no ajuste.
    if x >= 0:
        return 1 / (1 + math.exp(-x))
    e = math.exp(x)
    return e / (1 + e)


def ajustar_platt(xs, ys, voltas=50):
    """a, b de p' = sigmoide(a*logit(p) + b), por Newton na verossimilhanca.

    Dois parametros, e so. Com o nosso volume, qualquer coisa mais flexivel
    (isotonica, por faixa) ajustaria o ruido. A penalidade puxa para a=1, b=0
    («nao corrigir»): quando o juiz deu sempre a mesma p, a e b nao se
    separam, e sem ela o Newton divergia ate estourar.
    """
    lam = 1.0

    def custo(a, b):
        t = 0.0
        for x, y in zip(xs, ys):
            p = min(max(sigmoide(a * x + b), 1e-12), 1 - 1e-12)
            t -= y * math.log(p) + (1 - y) * math.log(1 - p)
        return t + lam / 2 * ((a - 1) ** 2 + b ** 2)

    a, b = 1.0, 0.0
    for _ in range(voltas):
        ga = gb = haa = hab = hbb = 0.0
        for x, y in zip(xs, ys):
            p = sigmoide(a * x + b)
            e, w = p - y, p * (1 - p)
            ga += e * x
            gb += e
            haa += w * x * x
            hab += w * x
            hbb += w
        ga += lam * (a - 1)
        gb += lam * b
        haa += lam
        hbb += lam
        det = haa * hbb - hab * hab
        if abs(det) < 1e-12:
            break
        da = (hbb * ga - hab * gb) / det
        db = (haa * gb - hab * ga) / det
        # Passo amortecido: o Newton cheio pulava o minimo e divergia (a=40
        # quando o juiz dizia 0,9 e acertava 30%). So aceita o que desce.
        atual, passo = custo(a, b), 1.0
        while passo > 1e-6 and custo(a - passo * da, b - passo * db) > atual:
            passo /= 2
        a, b = a - passo * da, b - passo * db
        if passo * (abs(da) + abs(db)) < 1e-9:
            break
    return a, b


def ajuste(registro, juiz, pergunta):
    """Ajuste de uma pergunta noul deste juiz, ou None abaixo do minimo."""
    xs, ys = [], []
    for r in ler(registro):
        if juiz_de(r) != juiz or pergunta not in r.get("desfecho", {}):
            continue
        q = r["perguntas"][pergunta]
        if q["tipo"] != "noul":
            return None
        xs.append(logit(q["dist"]["sim"]))
        ys.append(1 if r["desfecho"][pergunta] == "sim" else 0)
    if len(xs) < MINIMO_PARA_AJUSTAR:
        return None
    return ajustar_platt(xs, ys)


def aplicar_ajuste(ab, p):
    return sigmoide(ab[0] * logit(p) + ab[1])


def excesso(ps):
    top = [(p, y) for _, p, y in ps if p >= 0.5]
    if not top:
        return 0.0
    return sum(p for p, _ in top) / len(top) - sum(y for _, y in top) / len(top)


def cmd_historico(juiz="claude", registro=REGISTRO):
    """O que o juiz precisa saber de si antes de julgar, em poucas linhas.

    E a parte barata do que o Jev compra com treino de calibracao: o juiz ve
    onde errou de confianca, por pergunta, antes de dar o proximo numero.
    """
    por = {}
    for nome, p, y in pares(registro, juiz=juiz):
        por.setdefault(nome, []).append((nome, p, y))
    n = contar_desfechos(registro, juiz)
    out = [f"historico do juiz {juiz}: {n} desfechos"]
    if not por:
        return out[0] + " -- nada medido ainda; julgue pela evidencia"
    for nome in sorted(por):
        nd = contar_desfechos(registro, juiz, nome)
        e = excesso(por[nome])
        dica = ""
        if nd >= 10 and e > 0.10:
            dica = " -> REBAIXE: suas p altas acertaram menos do que diziam"
        elif nd >= 10 and e < -0.10:
            dica = " -> voce foi timido: acertou mais do que disse"
        elif nd < 10:
            dica = " (anedota: menos de 10)"
        a = ajuste(registro, juiz, nome)
        corr = f", ajuste ativo a={a[0]:.2f} b={a[1]:+.2f}" if a else ""
        out.append(f"  {nome}: {nd} desfechos, brier {brier(por[nome]):.3f}, excesso {e:+.2f}{corr}{dica}")
    return "\n".join(out)


def cmd_calibrar(registro=REGISTRO):
    ps = list(pares(registro))
    n = contar_desfechos(registro)
    out = [f"desfechos: {n} (minimo {MINIMO_PARA_CALIBRAR})"]
    if not ps:
        return "\n".join(out + ["sem desfecho registrado: nada a medir"])
    out.append(f"brier: {brier(ps):.4f} sobre {len(ps)} probabilidades (0 perfeito; 0,25 = chutar 0,5)")
    # Por pergunta: um Brier so deixa `real` otimista esconder `severidade` pessimista.
    por = {}
    for nome, p, y in ps:
        por.setdefault(nome, []).append((nome, p, y))
    out.append("pergunta            desfechos  brier   excesso de confianca")
    for nome in sorted(por):
        b = por[nome]
        nd = sum(1 for r in ler(registro) if nome in r.get("desfecho", {}))
        # excesso: media da p dada a opcao escolhida menos a taxa em que ela acertou
        exc = excesso(b)
        out.append(f"{nome[:18]:18s}  {nd:9d}  {brier(b):.4f}  {exc:+.2f}")
    out.append("faixa      n   prevista  ocorrida")
    for i in range(10):
        lo, hi = i / 10, (i + 1) / 10
        b = [(p, y) for _, p, y in ps if lo <= p < hi or (i == 9 and p == 1.0)]
        if b:
            out.append(f"{lo:.1f}-{hi:.1f}  {len(b):4d}   {sum(p for p, _ in b) / len(b):.2f}      {sum(y for _, y in b) / len(b):.2f}")
    if n < MINIMO_PARA_CALIBRAR:
        out.append("INSUFICIENTE: abaixo do minimo, a tabela e anedota")
    return "\n".join(out)


OLLAMA = "http://127.0.0.1:11434/api/generate"
LETRAS = "ABCDEFGHIJ"


def opcoes_de(q):
    if q["tipo"] == "noul":
        return ["sim", "nao"]
    if q["tipo"] == "score":
        return [str(k) for k in q.get("degraus", [0, 1, 2, 3])]
    return list(q["opcoes"])


def perguntar_local(modelo, estado, texto, opcoes, url=OLLAMA):
    """Distribuicao sobre as opcoes numa passada so, pelos logprobs da letra.

    E o que mais se aproxima do Jev sem os pesos dele: nada de texto gerado,
    um token lido, e a probabilidade vem da conta do modelo e nao de um numero
    que ele escreveu. Letra fora do top_logprobs recebe o piso, para que opcao
    que o modelo nem considerou nao saia com zero absoluto.
    """
    import urllib.request
    linhas = "\n".join(f"{LETRAS[i]}) {o}" for i, o in enumerate(opcoes))
    prompt = (f"Contexto:\n{estado}\n\nPergunta: {texto}\n{linhas}\n"
              f"Responda so com a letra.\nResposta:")
    corpo = json.dumps({"model": modelo, "prompt": prompt, "stream": False,
                        "logprobs": True, "top_logprobs": 20,
                        "options": {"num_predict": 1, "temperature": 0,
                                    # janela explicita: o Ollama corta calado o que passa do padrao
                                    "num_ctx": 8192}}).encode()
    t0 = time.time()
    req = urllib.request.Request(url, corpo, {"Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=300) as r:
        d = json.load(r)
    ms = (time.time() - t0) * 1000
    if d.get("error"):
        raise Invalido(f"ollama: {d['error']}")
    tops = (d.get("logprobs") or [{}])[0].get("top_logprobs") or []
    massa = {}
    for t in tops:
        k = t.get("token", "").strip().rstrip(")").upper()
        if len(k) == 1 and k in LETRAS[:len(opcoes)]:
            massa[k] = massa.get(k, 0.0) + math.exp(t["logprob"])
    piso = 1e-4
    bruto = [massa.get(LETRAS[i], piso) for i in range(len(opcoes))]
    soma = sum(bruto)
    return {o: b / soma for o, b in zip(opcoes, bruto)}, ms, sum(massa.values())


def cmd_local(modelo, entrada, saida, registro=REGISTRO, url=OLLAMA):
    """Julga com o modelo local e passa pelo MESMO veredito (limiar, registro, selo)."""
    j = json.loads(entrada)
    estado = "\n".join(j.get("contexto") or j.get("estado") or [])
    itens, tempos = [], []
    for item in j.get("itens") or []:
        pergs = {}
        for nome, q in item["perguntas"].items():
            ops = opcoes_de(q)
            dist, ms, cobertura = perguntar_local(modelo, estado, q["pergunta"], ops, url)
            tempos.append(ms)
            ev = f"logprob:{modelo} cobertura {cobertura:.2f}"
            if q["tipo"] == "noul":
                pergs[nome] = {"tipo": "noul", "p": round(dist["sim"], 6), "evid": ev}
            else:
                pergs[nome] = {"tipo": q["tipo"], "p": dist, "evid": ev}
        itens.append({"id": item["id"], "perguntas": pergs})
    julg = {"preset": f"local:{modelo}", "estado": j.get("estado") or ["contexto local"], "itens": itens}
    cmd_veredito(json.dumps(julg), saida, registro, juiz=f"local:{modelo}")
    if tempos:
        saida.write(f"latencia: {len(tempos)} perguntas, mediana {sorted(tempos)[len(tempos)//2]:.0f} ms\n")


CONFIG = os.path.join(raiz(), ".phxjev", "config.json")
BANCADA = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "bancada", "resultados.json")
MODOS = ("claude", "local", "auto")
# A regra da cognicao de 24/09 (juiz local pequeno nao substitui o Claude),
# agora em codigo: sem ela, «auto» seria so um nome.
QUALIFICA_N, QUALIFICA_BRIER, QUALIFICA_PIOR_ERRO = 50, 0.10, 0.90


def ler_config(config=CONFIG):
    try:
        with open(config, encoding="utf-8") as f:
            c = json.load(f)
    except (OSError, ValueError):
        c = {}
    return {"juiz": c.get("juiz", "claude"), "modelo": c.get("modelo", "qwen2.5:3b")}


def qualificado(modelo, bancada=BANCADA):
    """(passou?, motivo) do modelo local pela ultima bancada gravada."""
    try:
        with open(bancada, encoding="utf-8") as f:
            r = json.load(f)["juizes"].get(f"local {modelo}")
    except (OSError, ValueError, KeyError):
        r = None
    if not r:
        return False, f"{modelo} nao foi medido na bancada"
    falhas = []
    if r["n"] < QUALIFICA_N:
        falhas.append(f"n {r['n']} < {QUALIFICA_N}")
    if r["brier"] >= QUALIFICA_BRIER:
        falhas.append(f"brier {r['brier']} >= {QUALIFICA_BRIER}")
    if r.get("pior_erro", 1.0) > QUALIFICA_PIOR_ERRO:
        falhas.append(f"pior erro {r.get('pior_erro', 'nao medido')} > {QUALIFICA_PIOR_ERRO}")
    return (not falhas), ("; ".join(falhas) or f"brier {r['brier']} em {r['n']}")


def juiz_efetivo(config=CONFIG, bancada=BANCADA):
    """(modo que vale, modelo, frase que diz por que) -- o pedido nao manda sozinho."""
    c = ler_config(config)
    if c["juiz"] == "claude":
        return "claude", None, "juiz claude (configurado)"
    ok, motivo = qualificado(c["modelo"], bancada)
    if not ok:
        return "claude", None, f"juiz claude: {c['juiz']} pedido, mas {motivo} (bancada)"
    return c["juiz"], c["modelo"], f"juiz {c['juiz']} com {c['modelo']} ({motivo})"


def cmd_juiz(args, config=CONFIG, bancada=BANCADA):
    if args:
        if args[0] not in MODOS:
            raise Invalido(f"modo {args[0]!r}: use {'|'.join(MODOS)}")
        c = ler_config(config)
        c["juiz"] = args[0]
        if len(args) > 1:
            c["modelo"] = args[1]
        os.makedirs(os.path.dirname(config), exist_ok=True)
        with open(config, "w", encoding="utf-8") as f:
            json.dump(c, f, ensure_ascii=False, indent=1)
            f.write("\n")
    return juiz_efetivo(config, bancada)[2]


def cmd_auto(modelo, entrada, saida, registro=REGISTRO, url=OLLAMA):
    """Local primeiro; o que sair incerto vai para o Claude, nunca some."""
    buf = io.StringIO()
    cmd_local(modelo, entrada, buf, registro, url)
    texto = buf.getvalue()
    saida.write(texto)
    incertas = [l for l in texto.splitlines() if l.startswith("escalar:") and l != "escalar: nada"]
    saida.write("ESCALAR AO CLAUDE: " + (incertas[0][9:] if incertas else "nada") + "\n")


def cmd_mostrar(selo, registro=REGISTRO):
    for r in ler(registro):
        if r.get("selo") == selo and "saida" in r:
            texto = r["saida"]
            if hashlib.sha256(texto.encode("utf-8")).hexdigest()[:12] != selo:
                raise Invalido(f"selo {selo}: o registro foi alterado")
            return texto
    raise Invalido(f"selo {selo} nao esta no registro")


def main(argv):
    try:
        if argv[1:2] == ["veredito"]:
            cmd_veredito(sys.stdin.read(), sys.stdout)
        elif argv[1:2] == ["desfecho"] and len(argv) == 5:
            print(cmd_desfecho(*argv[2:5]))
        elif argv[1:2] == ["mostrar"] and len(argv) == 3:
            print(cmd_mostrar(argv[2]))
        elif argv[1:2] == ["colher"] and len(argv) == 3:
            feitos, pulados = cmd_colher(argv[2])
            print("\n".join(feitos) or "nada novo a colher")
            print(f"colhidos: {len(feitos)} · pulados (pedido ja fechado antes do veredito): {pulados}")
        elif argv[1:2] == ["local"] and len(argv) == 3:
            cmd_local(argv[2], sys.stdin.read(), sys.stdout)
        elif argv[1:2] == ["historico"]:
            print(cmd_historico(argv[2] if len(argv) > 2 else "claude"))
        elif argv[1:2] == ["juiz"]:
            print(cmd_juiz(argv[2:]))
        elif argv[1:2] == ["auto"] and len(argv) == 3:
            cmd_auto(argv[2], sys.stdin.read(), sys.stdout)
        elif argv[1:2] == ["calibrar"]:
            print(cmd_calibrar())
        else:
            print(__doc__.strip(), file=sys.stderr)
            return 2
    except (Invalido, json.JSONDecodeError) as e:
        print(f"PhxJev RECUSOU: {e}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
