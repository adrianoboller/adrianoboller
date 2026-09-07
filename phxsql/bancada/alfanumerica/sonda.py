#!/usr/bin/env python3
"""Sonda da PARTICAO ALFANUMERICA contra o motor vivo.

    flock /tmp/phx-cargo.lock cargo build --release -p phxsql-server
    python3 bancada/alfanumerica/sonda.py

Pergunta do dono, 07/09/2026: *«tabela particionada por campo texto chave, ex
nome, pega a primeira letra e cria uma paginacao de A a Z para ficar leve o
cadastro, e na hora de usar e transparente para select, insert, update,
softdelete e delete»*.

O desenho ja existia (pedido 11, `.pag`, `ModoParticao::PorLetra`). Esta sonda
nao le o codigo: **exercita as cinco operacoes** contra um `phxsqld` de pe e
mede o custo de ler UM balde. Ler o codigo diz o que deveria acontecer; a
sonda diz o que acontece.

# As tres coisas que a sonda aprendeu na primeira corrida, e que ficam aqui

1. **O instrumento antes do veredito.** A resposta do protocolo vem aninhada
   em `resultado`; lendo o nivel de cima, o `varrer` "devolvia 0 linhas" com
   seis gravadas. Uma sonda que le o campo errado publica um defeito que nao
   existe.
2. **O controle da tabela SEM particao.** O `atualizar` parcial e recusado
   TAMBEM na tabela normal (`coluna nome e obrigatoria e recebeu NULL`) --
   entao "o update parcial quebra na particionada" era erro da sonda, e nao
   do motor. Sem o controle, isso teria virado defeito relatado.
3. **Campo desconhecido: nao e buraco da particao.** `balde` e `letra` sao
   engolidos calados, mas `xyzzy` tambem -- e comportamento geral do
   protocolo. A sonda mede `xyzzy` de proposito, para nao acusar a particao
   por uma tolerancia que e de todo mundo.
"""

import json
import os
import shutil
import socket
import string
import subprocess
import sys
import time

PORTA = int(os.environ.get("PHX_SONDA_PORTA", "5479"))
BASE = f"/tmp/phx-sonda-alfa-{os.getpid()}"
BINARIO = "target/release/phxsqld"
POR_LETRA = int(os.environ.get("PHX_SONDA_POR_LETRA", "200"))


class Servidor:
    """Sobe um `phxsqld` de verdade e o derruba no fim, aconteca o que acontecer."""

    def __enter__(self):
        if not os.path.exists(BINARIO):
            raise SystemExit(
                f"{BINARIO} nao existe. Rode antes:\n"
                "  flock /tmp/phx-cargo.lock cargo build --release -p phxsql-server"
            )
        shutil.rmtree(BASE, ignore_errors=True)
        os.makedirs(BASE + "/dados", exist_ok=True)
        cfg = BASE + "/config.json"
        with open(cfg, "w") as f:
            json.dump(
                {
                    "bind": f"127.0.0.1:{PORTA}",
                    "base": BASE + "/dados",
                    "token": "t",
                    "web": {"ligado": False},
                    "max_linhas": 200000,
                },
                f,
            )
        self.p = subprocess.Popen(
            [BINARIO, "--config", cfg],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        for _ in range(100):
            try:
                socket.create_connection(("127.0.0.1", PORTA), 0.2).close()
                break
            except OSError:
                time.sleep(0.1)
        else:
            raise SystemExit(f"o servidor nao subiu na porta {PORTA}")
        self.s = socket.create_connection(("127.0.0.1", PORTA), 5)
        self.f = self.s.makefile("rwb")
        return self

    def __exit__(self, *_):
        try:
            self.f.close()
            self.s.close()
        except OSError:
            pass
        self.p.terminate()
        self.p.wait(timeout=10)
        shutil.rmtree(BASE, ignore_errors=True)

    def pedir(self, **kw):
        kw.setdefault("token", "t")
        self.f.write((json.dumps(kw) + "\n").encode())
        self.f.flush()
        r = json.loads(self.f.readline().decode())
        # A resposta util vem em `resultado`; o nivel de cima so tem `ok`/`op`.
        if isinstance(r.get("resultado"), dict):
            fora = {k: v for k, v in r.items() if k != "resultado"}
            r = {**r["resultado"], **fora}
        return r


def diz(rotulo, r, corte=150):
    marca = "OK  " if r.get("ok") else "ERRO"
    extra = "" if r.get("ok") else " -- " + str(r.get("erro") or r)[:corte]
    print(f"  [{marca}] {rotulo}{extra}")
    return r


COLUNAS = [
    {"nome": "id", "tipo": "Int8"},
    # A coluna de referencia e OBRIGATORIA por exigencia do motor: se aceitasse
    # nulo, toda linha sem valor cairia em `Outros` sem ninguem ter escolhido.
    {"nome": "nome", "tipo": "Str(60)", "obrigatoria": True},
    {"nome": "cidade", "tipo": "Str(40)"},
]
INDICES = [{"nome": "porId", "colunas": ["id"], "unico": True}]


def parte_1_as_cinco_operacoes(sv):
    print("\n=== 1. As cinco operacoes numa tabela particionada por letra\n")
    sv.pedir(op="criar_database", database="loja")
    diz(
        "criar_tabela particao=letra particao_coluna=nome",
        sv.pedir(
            op="criar_tabela",
            database="loja",
            tabela="clientes",
            colunas=COLUNAS,
            indices=INDICES,
            registros_por_arquivo=1000,
            particao="letra",
            particao_coluna="nome",
            softdelete=True,
        ),
    )

    nomes = [
        (1, "Alves", "Blumenau"),
        (2, "Silva", "Joinville"),
        (3, "Ávila", "Itajaí"),
        (4, "9 de Julho Ltda", "Recife"),
        (5, "@estranho", "Curitiba"),
        (6, "Andrade", "Blumenau"),
    ]
    print()
    for i, nome, cidade in nomes:
        r = sv.pedir(
            op="inserir",
            database="loja",
            tabela="clientes",
            linha={"id": i, "nome": nome, "cidade": cidade},
        )
        marca = "OK  " if r.get("ok") else "ERRO"
        print(f"  [{marca}] inserir {nome!r:20} rowid={r.get('rowid')}")

    print("\n  os arquivos que nasceram:")
    for a in sorted(os.listdir(BASE + "/dados/loja")):
        if a.endswith(".reg"):
            print(f"    {a:24} {os.path.getsize(BASE + '/dados/loja/' + a):>7} bytes")

    r = sv.pedir(op="varrer", database="loja", tabela="clientes", max=100)
    linhas = r.get("linhas") or []
    print(f"\n  varrer devolve as {len(linhas)} linhas como UMA tabela so:")
    for l in linhas:
        print(f"    rowid {l['rowid']:>6}  rownum {l.get('rownum'):>2}  {l['nome']}")

    alvo = {l["nome"]: l for l in linhas}
    print()
    diz(
        "atualizar linha INTEIRA, mesma letra",
        sv.pedir(
            op="atualizar",
            database="loja",
            tabela="clientes",
            rowid=alvo["Alves"]["rowid"],
            linha={"id": 1, "nome": "Alves", "cidade": "Gaspar"},
        ),
    )
    diz(
        "atualizar mudando a letra (A -> Z)",
        sv.pedir(
            op="atualizar",
            database="loja",
            tabela="clientes",
            rowid=alvo["Alves"]["rowid"],
            linha={"id": 1, "nome": "Zimmermann", "cidade": "Gaspar"},
        ),
    )
    diz(
        "excluir SUAVE",
        sv.pedir(
            op="excluir",
            database="loja",
            tabela="clientes",
            rowid=alvo["Silva"]["rowid"],
            motivo="sonda",
        ),
    )
    diz(
        "excluir DE VEZ",
        sv.pedir(
            op="excluir",
            database="loja",
            tabela="clientes",
            rowid=alvo["Ávila"]["rowid"],
            motivo="sonda",
            fisico=True,
        ),
    )
    r = sv.pedir(op="varrer", database="loja", tabela="clientes", max=100)
    print("  depois das duas exclusoes:", [l["nome"] for l in (r.get("linhas") or [])])

    print()
    diz("begin", sv.pedir(op="begin"))
    diz(
        "inserir DENTRO da transacao",
        sv.pedir(
            op="inserir",
            database="loja",
            tabela="clientes",
            linha={"id": 9, "nome": "Teste"},
        ),
        corte=220,
    )
    sv.pedir(op="rollback")


def parte_2_o_controle(sv):
    """O `atualizar` parcial e recusado na tabela NORMAL tambem?

    Sem esta pergunta, a recusa na particionada parece defeito da particao."""
    print("\n=== 2. CONTROLE -- o `atualizar` parcial na tabela SEM particao\n")
    sv.pedir(
        op="criar_tabela",
        database="loja",
        tabela="normal",
        colunas=COLUNAS,
        indices=INDICES,
    )
    sv.pedir(
        op="inserir",
        database="loja",
        tabela="normal",
        linha={"id": 1, "nome": "Alves", "cidade": "Blumenau"},
    )
    diz(
        "atualizar SO a cidade, tabela normal",
        sv.pedir(
            op="atualizar",
            database="loja",
            tabela="normal",
            rowid=1,
            linha={"cidade": "Gaspar"},
        ),
    )
    print(
        "    -> se isto tambem recusa, `atualizar` e linha INTEIRA em toda tabela,\n"
        "       e a recusa na particionada nao e defeito dela."
    )


def parte_3_a_paginacao_az(sv):
    print(f"\n=== 3. A paginacao A-Z: {POR_LETRA} linhas por letra, 26 letras\n")
    sv.pedir(
        op="criar_tabela",
        database="loja",
        tabela="agenda",
        colunas=COLUNAS,
        indices=INDICES,
        registros_por_arquivo=1000,
        particao="letra",
        particao_coluna="nome",
    )
    lote, n = [], 0
    for letra in string.ascii_uppercase:
        for k in range(POR_LETRA):
            n += 1
            lote.append({"id": n, "nome": f"{letra}{k:04d} Sobrenome", "cidade": "x"})
    t0 = time.time()
    for i in range(0, len(lote), 1000):
        r = sv.pedir(
            op="inserir_lote", database="loja", tabela="agenda", linhas=lote[i : i + 1000]
        )
        if not r.get("ok"):
            print("  lote falhou:", str(r.get("erro"))[:200])
            return
    print(f"  gravadas {n} linhas em {time.time() - t0:.2f}s")

    esq = sv.pedir(op="esquema", database="loja", tabela="agenda")
    pag = esq.get("paginacao") or {}
    baldes = {b["letra"]: b for b in pag.get("baldes", []) if b.get("registros")}
    print(f"  baldes com linha: {len(baldes)}  (modo={pag.get('modo')}, coluna={pag.get('coluna')})")

    print("\n  UM balde so, pela composicao que o protocolo JA permite:")
    print("    esquema -> primeiro_rowid do balde; varrer(depois=primeiro_rowid-1, max=registros)")
    for letra in ("A", "M", "Z"):
        b = baldes[letra]
        t0 = time.time()
        r = sv.pedir(
            op="varrer",
            database="loja",
            tabela="agenda",
            depois=b["primeiro_rowid"] - 1,
            max=b["registros"],
        )
        ls = r.get("linhas") or []
        so_desta = all(l["nome"].startswith(letra) for l in ls)
        print(
            f"    letra {letra}: devolvidas={len(ls):>4} examinadas={r.get('examinadas'):>4} "
            f"so_desta_letra={so_desta}  {(time.time() - t0) * 1000:6.1f} ms"
        )
    print(
        "    -> o custo da ULTIMA letra tem de ser igual ao da primeira. Se crescer,\n"
        "       a leitura esta varrendo o cadastro inteiro e a particao nao pagou nada."
    )

    print("\n  campo desconhecido -- recusa, ou engole calado?")
    for campo in ({"balde": "A"}, {"letra": "A"}, {"xyzzy": 1}):
        r = sv.pedir(op="varrer", database="loja", tabela="agenda", max=5, **campo)
        print(f"    varrer + {campo}: ok={r.get('ok')} devolvidas={r.get('devolvidas')}")
    print(
        "    -> `xyzzy` esta aqui como CONTROLE: se ele tambem passa, a tolerancia\n"
        "       e do protocolo inteiro e nao um buraco da particao."
    )


def main():
    with Servidor() as sv:
        parte_1_as_cinco_operacoes(sv)
        parte_2_o_controle(sv)
        parte_3_a_paginacao_az(sv)
    print("\nfim.")


if __name__ == "__main__":
    sys.exit(main())
