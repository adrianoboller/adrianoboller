#!/usr/bin/env python3
"""Regera os numeros do par 11 de docs/propostas/aquario-707.md (pedido 707, fatia A1).

Compara a hipotese Ha1 (histograma 2 x p95) com a Ha4 (Welford em ln, z>=4, piso)
sobre os acessos.log reais e sobre series sinteticas de semente fixa (7 e 11).

  python3 regra.py            imprime a tabela
  python3 regra.py --gravar   reescreve o bloco entre os marcadores do par 11

So biblioteca padrao. Deterministico: nada de data na saida. Os logs reais
moram em /tmp (efemeros): se faltar algum, a saida diz NAO MEDIDA / BASE
DIFERENTE em vez de publicar numero de uma base menor como se fosse a mesma.
O custo em ns (bench.rs, Rust) nao e regerado aqui: roda-se
`rustc -O --edition 2021 bench.rs && ./bench` do Apendice B.
Sai com codigo 1 se algum numero regerado diverge do escrito no par 11.
"""
import json, math, glob, random, sys, os, collections

REPL = {'posicao', 'replicar', 'retrato_da_replica', 'aplicar', 'cluster_pulso', 'replicar_aguardar'}  # OPS_DE_REPLICACAO, servidor.rs:389
RAIZ = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', '..'))
DOC = os.path.join(RAIZ, 'docs', 'propostas', 'aquario-707.md')
INI = '<!-- aquario-regra:inicio (gerado por bancada/aquario/regra.py --gravar; nao editar) -->'
FIM = '<!-- aquario-regra:fim -->'

# logs reais do Apendice B: 1 + 1 + 22 = 24 esperados
FIXOS = ['/tmp/phx-207-quorum-real/no1/acessos.log', '/tmp/phx-odbc-sonda/acessos.log']
ESPERADOS = 24


def balde(x):  # 4 por oitava, x em unidade >=1
    if x < 1:
        return 0
    return int(math.floor(4 * math.log2(x))) + 1


def p95_hist(h):
    n = sum(h.values()); alvo = math.ceil(0.95 * n); a = 0
    for b in sorted(h):
        a += h[b]
        if a >= alvo:
            return 2 ** (b / 4) if b > 0 else 1  # topo do balde


class Hist:
    def __init__(s): s.h = collections.Counter(); s.n = 0
    def anormal(s, x, piso):
        if s.n < 20:
            return False
        return x >= max(2 * p95_hist(s.h), piso)
    def somar(s, x): s.h[balde(x)] += 1; s.n += 1


class Welf:  # a da IA: n>=30, z>=4 sobre ln, sem piso
    NM = 30
    def __init__(s): s.n = 0; s.m = 0.0; s.m2 = 0.0
    def anormal(s, x, _):
        if s.n < s.NM:
            return False
        sd = math.sqrt(s.m2 / (s.n - 1)) if s.n > 1 else 0
        if sd == 0:
            return False
        return (math.log(x) - s.m) / sd >= 4
    def somar(s, x):
        v = math.log(x); s.n += 1; d = v - s.m; s.m += d / s.n; s.m2 += d * (v - s.m)


class WelfPiso(Welf):  # a unificada (A0): n>=20, z>=4 sobre ln, piso
    NM = 20
    def anormal(s, x, piso): return x >= piso and Welf.anormal(s, x, piso)


def rodar(eventos, Cls, piso, excluir):
    base = {}; al = 0; av = 0; quais = []
    for op, tab, x in eventos:
        if excluir and op in REPL:
            continue
        k = (op, tab); b = base.setdefault(k, Cls()); av += 1
        if b.anormal(x, piso):
            al += 1; quais.append((op, x))
        b.somar(x)
    return al, av, quais


def logreal(f, minimo):
    for l in open(f):
        d = json.loads(l); yield d['op'], d.get('tabela', ''), max(d.get('ms', 0), minimo)


def achar_logs():
    fs = list(FIXOS) + sorted(glob.glob('/tmp/phx-custo-tx-4992-*/acessos.log'))
    return [f for f in fs if os.path.isfile(f)], [f for f in fs if not os.path.isfile(f)]


def medir():
    fs, faltam = achar_logs()
    out = []; num = {}
    out.append(f'logs reais lidos: {len(fs)} (esperados {ESPERADOS})'
               + ('' if len(fs) == ESPERADOS else ' -> BASE DIFERENTE da escrita no par 11; numeros nao comparaveis'))
    for f in faltam:
        out.append(f'NAO MEDIDA: {f} (ausente)')
    if not fs:
        out.append('NAO MEDIDA: nenhum acessos.log real; rode a bancada que os gera (Apendice B).')
    evs = {}
    configs = [('A hist 2xp95 piso250 excl', Hist, 250, 1, 1), ('A sem excl', Hist, 250, 0, 1),
               ('A sem piso excl', Hist, 0, 1, 1), ('B welford ln z4 sem excl', Welf, 0, 0, 0.5),
               ('B welford excl', Welf, 0, 1, 0.5), ('C unificada n20 z4 piso250 excl', WelfPiso, 250, 1, 0.5),
               ('C sem piso', WelfPiso, 0, 1, 0.5)]
    out.append('')
    out.append('| regra sobre os logs reais | alarmes | de | % | ops mais frequentes |')
    out.append('|---|---|---|---|---|')
    for nome, Cls, piso, exc, minimo in configs:
        tot = [0, 0]; q = collections.Counter()
        for f in fs:
            ev = evs.setdefault((f, minimo), list(logreal(f, minimo)))
            a, v, qq = rodar(ev, Cls, piso, exc); tot[0] += a; tot[1] += v; q.update(o for o, _ in qq)
        num[nome] = tot
        pct = 100 * tot[0] / max(tot[1], 1)
        out.append(f'| {nome} | {tot[0]} | {tot[1]} | {pct:.2f}% | {q.most_common(4)} |')
    # casos sinteticos do F4 da IA (em us)
    out.append('')
    out.append('| caso do F4 (30 amostras ~1 ms) | 200 ms alarma? | 1,3 ms alarma? |')
    out.append('|---|---|---|')
    for Cls, piso in [(Hist, 250_000), (Hist, 0), (Welf, 0)]:
        r = []
        for ult in (200_000, 1_300):
            b = Cls()
            for i in range(30):
                b.somar(1000 * (1 + 0.05 * ((i % 5) - 2)))
            r.append(b.anormal(ult, piso))
        out.append(f'| {Cls.__name__} piso {piso} | {r[0]} | {r[1]} |')
    # sinteticos, semente 7: lognormal e bimodal, 200k cada
    random.seed(7)
    out.append('')
    out.append('| sintetico (semente 7, 200.000) | estatistica | falso alarme |')
    out.append('|---|---|---|')
    for nome, gen in [('lognormal s=1', lambda: math.exp(random.gauss(math.log(800), 1.0))),
                      ('lognormal s=0,3', lambda: math.exp(random.gauss(math.log(800), 0.3))),
                      ('bimodal 90/10 400us/6ms', lambda: random.gauss(400, 40) if random.random() < .9 else random.gauss(6000, 600)),
                      ('bimodal 99/1 400us/6ms', lambda: random.gauss(400, 40) if random.random() < .99 else random.gauss(6000, 600))]:
        ev = [('x', 't', max(gen(), 1)) for _ in range(200_000)]
        for cn, Cls in (('hist 2xp95', Hist), ('welford z4', Welf)):
            a, v, _ = rodar(ev, Cls, 0, 0)
            out.append(f'| {nome} | {cn} | {100 * a / v:.3f}% |')
            num[(nome, cn)] = 100 * a / v
    # nmin.py: aquecimento com n>=20 e n>=30, semente 11
    random.seed(11)
    out.append('')
    out.append('| aquecimento (semente 11, 4.000 chaves x 60) | falso alarme |')
    out.append('|---|---|')
    for nmin in (20, 30):
        for sig in (0.3, 1.0):
            al = 0; av = 0
            for k in range(4000):
                b = Welf()
                for i in range(60):
                    x = math.exp(random.gauss(math.log(800), sig))
                    if b.n >= nmin:
                        sd = math.sqrt(b.m2 / (b.n - 1)); av += 1
                        if sd > 0 and (math.log(x) - b.m) / sd >= 4:
                            al += 1
                    b.somar(x)
            out.append(f'| welford z4 n>={nmin} sigma={sig} | {100 * al / av:.3f}% ({al}/{av}) |')
    return out, num


def conferir(num):
    """Compara com o que o par 11 escreve. Nao ajeita: so relata."""
    def g(k, i):
        return num[k][i] if k in num else None
    esc = [
        ('Ha1 alarmes (hist 2xp95 piso250 excl)', 1, g('A hist 2xp95 piso250 excl', 0)),
        ('Ha1 avaliacoes', 9317, g('A hist 2xp95 piso250 excl', 1)),
        ('Ha4 alarmes (unificada n20 z4 piso250 excl)', 1, g('C unificada n20 z4 piso250 excl', 0)),
        ('Ha4 avaliacoes', 9317, g('C unificada n20 z4 piso250 excl', 1)),
        ('sem piso, Ha4 (C sem piso)', 40, g('C sem piso', 0)),
        ('sem piso, Ha1 (A sem piso excl)', 39, g('A sem piso excl', 0)),
        ('Welford sem exclusao', 51, g('B welford ln z4 sem excl', 0)),
        ('Welford com exclusao', 37, g('B welford excl', 0)),
        ('Hist sem exclusao', 15, g('A sem excl', 0)),
    ]
    div = []
    out = ['| conferencia contra o par 11 | escrito | regerado | |', '|---|---|---|---|']
    for n, e, l in esc:
        ok = e == l
        out.append(f'| {n} | {e} | {l} | {"ok" if ok else "DIVERGE"} |')
        if not ok:
            div.append((n, e, l))
    for nome, cn, esc_pct in (('lognormal s=1', 'hist 2xp95', '0.612'), ('lognormal s=1', 'welford z4', '0.004')):
        v = num.get((nome, cn))
        lido = f'{v:.3f}' if v is not None else None
        ok = lido == esc_pct
        out.append(f'| {nome} {cn} (%) | {esc_pct} | {lido} | {"ok" if ok else "DIVERGE"} |')
        if not ok:
            div.append((f'{nome} {cn}', esc_pct, lido))
    return out, div


def main():
    corpo, num = medir()
    conf, div = conferir(num)
    texto = '\n'.join(corpo + [''] + conf)
    print(texto)
    if div:
        print('\nDIVERGENCIAS:')
        for n, e, l in div:
            print(f'  {n}: escrito {e}, regerado {l}')
    if '--gravar' in sys.argv:
        md = open(DOC, encoding='utf-8').read()
        novo = f'{INI}\n\n{texto}\n\n{FIM}\n'
        if INI in md and FIM in md:
            a = md.index(INI); b = md.index(FIM) + len(FIM) + 1
            md = md[:a] + novo + md[b:]
        else:
            ancora = '### 11.2 '
            if ancora not in md:
                sys.exit('ancora "### 11.2" nao achada em ' + DOC)
            md = md.replace(ancora, novo + '\n' + ancora, 1)
        open(DOC, 'w', encoding='utf-8').write(md)
        print(f'\ngravado em {DOC}')
    return 1 if div else 0


if __name__ == '__main__':
    sys.exit(main())
