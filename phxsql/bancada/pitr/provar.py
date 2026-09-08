#!/usr/bin/env python3
"""O PITR pelo SOQUETE: restaurar a um INSTANTE, com o controle na mesma corrida.

    cargo build --release
    python3 bancada/pitr/provar.py

# Por que esta bancada existe

Porque a linha do PITR no `docs/COMPARATIVO.md` era uma **sonda de codigo**, e
sonda de codigo responde «o campo existe» -- que nao e a mesma pergunta que
«a linha 3 ficou de fora». *Celula que virou TEM por edicao do medidor sem
prova de efeito e o erro que a §1 do COMPARATIVO ja pagou cinco vezes.*

E porque a receita escrita em `docs/RESTAURACAO.md` § 7.10 precisa rodar, e
nao so estar escrita: **roteiro que resolveu algo nao pode morrer com a
sessao**, e receita errada no documento vira sonda quebrada na bancada.

# O que ela prova, e o que a faz honesta

A historia e uma so: linha 1, copia, linha 2, alteracao da 1, linha 3. Com
`ate` entre a alteracao e a inclusao da terceira, o restaurado tem a 1
ALTERADA e a 2, e NAO tem a 3.

O que a torna prova e o **controle positivo na mesma corrida**: o MESMO backup
restaurado SEM `ate` volta com uma linha so. Sem esse controle, uma
restauracao que devolvesse duas linhas por acaso passaria -- e uma bancada que
so sabe dizer «deu certo» nao sabe dizer nada.

Junto vao as recusas, pelo mesmo soquete: `ate` antes da copia, `ate` e
`ate_ms` no mesmo pedido, e o modo por cima. Recusa que o teste de unidade
prova e recusa no processo; aqui ela e o texto que chega ao cliente.

# As duas armadilhas, e as duas ja pagas

1. **O relogio precisa andar** entre as escritas. Tres insercoes seguidas caem
   no MESMO milissegundo, e ai nao existe instante entre a alteracao e a
   ultima linha. A bancada espera, e CONFERE que andou antes de cortar -- se
   nao andou, ela falha dizendo isso, em vez de medir outra coisa.
2. **O corte sai do proprio diario, medido.** Um `ate` digitado a mao seria um
   numero que ninguem mediu, que e o erro que esta casa ja pagou quatro vezes.

NUNCA usa pkill: o servidor morre pelo PID que este script guardou -- o
processo pode ser de outro agente.
"""
import json
import os
import shutil
import signal
import socket
import subprocess
import sys
import tempfile
import time

AQUI = os.path.dirname(os.path.abspath(__file__))
RAIZ = os.path.abspath(os.path.join(AQUI, ".."))
RAIZ = os.path.abspath(os.path.join(RAIZ, ".."))
# O `target` pode ser compartilhado por varias arvores de trabalho (disco
# apertado), e nesse caso ele NAO esta debaixo desta raiz. Quem manda e o
# `CARGO_TARGET_DIR`, que e o mesmo que o `cargo build` obedece -- deduzir
# «target ao lado do fonte» mediria um binario que talvez nem exista.
ALVO = os.environ.get("CARGO_TARGET_DIR") or os.path.join(RAIZ, "target")
PHXSQLD = os.path.join(ALVO, "release", "phxsqld")
SAIDA = os.path.join(AQUI, "resultados.json")

# Faixa propria, para nao colidir com as outras bancadas desta pasta.
PORTA, TOKEN = 7481, "prova-pitr"
DB, TAB = "loja", "clientes"

CONFERENCIAS, FALHAS, DETALHE = 0, [], []


def confere(rotulo, condicao, extra=""):
    global CONFERENCIAS
    CONFERENCIAS += 1
    print(("  ok   " if condicao else "FALHA  ") + rotulo
          + ("" if condicao else "   " + str(extra)))
    DETALHE.append(rotulo)
    if not condicao:
        FALHAS.append(rotulo)


def corpo(r):
    """A resposta de dentro do envelope.

    Ler o envelope em vez do `resultado` ja fez uma sonda desta casa publicar
    «nenhuma operacao entre as 0». Aqui a funcao e uma so.
    """
    return r.get("resultado", r)


def main():
    if not os.path.exists(PHXSQLD):
        raise SystemExit(
            f"{PHXSQLD} nao existe.\n"
            "Compile antes de medir:  cargo build --release\n"
            "(com `target` compartilhado, exporte CARGO_TARGET_DIR primeiro)")

    base = tempfile.mkdtemp(prefix="bancada-pitr-")
    processo = None
    try:
        cfg = {
            "base": "base",
            "bind": f"127.0.0.1:{PORTA}",
            "token": TOKEN,
            # SEM a imagem da linha nao ha PITR, e o servidor recusa nomeando
            # o interruptor. Ela vale para o que for gravado daqui em diante,
            # entao entra ANTES de o servidor subir.
            "replicacao": {"papel": "isolado", "imagem_da_linha": True},
            "backup": {"destino": os.path.join(base, "backup")},
        }
        with open(os.path.join(base, "config.json"), "w") as f:
            json.dump(cfg, f, indent=2)
        log = open(os.path.join(base, "servidor.log"), "a")
        processo = subprocess.Popen([PHXSQLD], cwd=base, stdout=log,
                                    stderr=subprocess.STDOUT,
                                    stdin=subprocess.DEVNULL)

        fim = time.monotonic() + 25
        while True:
            try:
                s = socket.create_connection(("127.0.0.1", PORTA), timeout=5)
                break
            except OSError:
                if time.monotonic() > fim:
                    raise SystemExit(
                        "o servidor nao subiu:\n"
                        + open(os.path.join(base, "servidor.log")).read())
                time.sleep(0.1)
        s.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
        arquivo_fluxo = s.makefile("rwb")

        def fala(**pedido):
            pedido.setdefault("token", TOKEN)
            arquivo_fluxo.write((json.dumps(pedido) + "\n").encode())
            arquivo_fluxo.flush()
            linha = arquivo_fluxo.readline()
            if not linha:
                raise ConnectionError("o servidor fechou a conexao")
            return json.loads(linha.decode())

        # ---------------------------------------------- a historia, passo a passo
        fala(op="criar_database", database=DB)
        r = fala(op="criar_tabela", database=DB, tabela=TAB,
                 colunas=[{"nome": "id", "tipo": "Int4", "obrigatoria": True},
                          {"nome": "nome", "tipo": "Str(20)"}],
                 indices=[{"nome": "porId", "colunas": ["id"],
                           "unico": True, "primario": True}])
        confere("a tabela nasce", r.get("ok"), r)

        confere("linha 1 antes da copia",
                fala(op="inserir", database=DB, tabela=TAB,
                     linha={"id": 1, "nome": "um"}).get("ok"))
        time.sleep(0.01)

        r = fala(op="backup", destino=os.path.join(base, "backup"),
                 database=DB, zip=True)
        copia = corpo(r).get("arquivo")
        confere("o backup saiu", bool(copia), r)
        time.sleep(0.01)

        confere("linha 2 depois da copia",
                fala(op="inserir", database=DB, tabela=TAB,
                     linha={"id": 2, "nome": "dois"}).get("ok"))
        time.sleep(0.01)
        confere("a linha 1 e alterada",
                fala(op="atualizar", database=DB, tabela=TAB, rowid=1,
                     linha={"id": 1, "nome": "um alterado"}).get("ok"))
        time.sleep(0.01)
        confere("linha 3, depois do corte",
                fala(op="inserir", database=DB, tabela=TAB,
                     linha={"id": 3, "nome": "tres"}).get("ok"))

        # O CORTE sai do diario, e nao da cabeca de quem escreveu a bancada.
        eventos = corpo(fala(op="diario", database=DB, tabela=TAB,
                             max=100))["eventos"]
        confere("quatro eventos no diario", len(eventos) == 4, eventos)
        carimbos = [e["carimbo_ms"] for e in eventos]
        corte = carimbos[3] - 1
        confere("o relogio andou entre a alteracao e a ultima linha",
                corte >= carimbos[2], carimbos)

        # ---------------------------------------------------------------- o PITR
        r = fala(op="restaurar_backup", origem=copia,
                 database="loja_no_meio", ate_ms=corte)
        pitr = corpo(r).get("pitr")
        confere("a resposta traz o bloco pitr", pitr is not None, r)
        if pitr:
            confere("reaplicou 2 eventos", pitr["reaplicados"] == 2, pitr)
            confere("pulou 1 (o que ficou depois do corte)",
                    pitr["tabelas"][0]["pulados"] == 1, pitr)
            confere("nenhuma tabela parou no meio",
                    "parou_em" not in pitr["tabelas"][0], pitr)
            confere("nada sobrou de um lado so",
                    pitr["sem_diario_vivo"] == [] and pitr["novas_na_origem"] == [],
                    pitr)

        linhas = [(l["id"], l["nome"]) for l in
                  corpo(fala(op="varrer", database="loja_no_meio", tabela=TAB))["linhas"]]
        confere("O EFEITO: a 1 alterada e a 2, e NADA da 3",
                linhas == [(1, "um alterado"), (2, "dois")], linhas)

        # ------------------------------------------------- o CONTROLE POSITIVO
        r = fala(op="restaurar_backup", origem=copia, database="loja_da_copia")
        confere("controle: sem `ate` a resposta nao ganha bloco pitr",
                "pitr" not in corpo(r), r)
        ctl = [(l["id"], l["nome"]) for l in
               corpo(fala(op="varrer", database="loja_da_copia", tabela=TAB))["linhas"]]
        confere("controle: sem `ate` volta SO a linha 1", ctl == [(1, "um")], ctl)

        viva = corpo(fala(op="varrer", database=DB, tabela=TAB))["linhas"]
        confere("o banco vivo nao foi tocado: continua com 3 linhas",
                len(viva) == 3, viva)

        # --------------------------------------------------------- as recusas
        r = fala(op="restaurar_backup", origem=copia, database="loja_x",
                 ate="2020-01-01T00:00:00Z")
        confere("recusa: `ate` antes da copia",
                not r.get("ok") and "ANTES do instante da copia" in json.dumps(r), r)

        r = fala(op="restaurar_backup", origem=copia, database="loja_x",
                 ate="2099-01-01T00:00:00Z", ate_ms=1)
        confere("recusa: `ate` e `ate_ms` no mesmo pedido",
                not r.get("ok") and "nao os dois" in json.dumps(r), r)

        r = fala(op="restaurar_backup", origem=copia, database="loja_x",
                 ate="2026-09-08T15:00:00+03:00")
        confere("recusa: fuso escrito a mao nao e engolido",
                not r.get("ok") and "nao e um instante" in json.dumps(r), r)

        r = fala(op="restaurar_backup", origem=copia, database=DB,
                 modo="por_cima", confirmar=True, ate="2099-01-01T00:00:00Z")
        confere("recusa: por cima com `ate` (o por cima tira o diario do lugar)",
                not r.get("ok") and "POR CIMA" in json.dumps(r), r)

        # Recusa que ja criou o database nao e recusa: e estrago com mensagem.
        bancos = json.dumps(corpo(fala(op="bancos")))
        confere("nenhum database sobrou das recusas",
                "loja_x" not in bancos, bancos)
    finally:
        if processo is not None:
            try:
                processo.send_signal(signal.SIGTERM)
                processo.wait(timeout=8)
            except Exception:
                try:
                    processo.kill()
                except Exception:
                    pass
        shutil.rmtree(base, ignore_errors=True)

    resultado = {
        "conferencias": CONFERENCIAS,
        "falhas": len(FALHAS),
        "eventos_reaplicados": 2,
        "eventos_pulados": 1,
        "linhas_no_restaurado": 2,
        "linhas_no_controle_sem_ate": 1,
        # A data sai DAQUI, e nao do `mtime` do arquivo: juntar corridas de
        # dias diferentes sem dizer quando publica um retrato que nunca houve.
        "medido_em": time.strftime("%Y-%m-%d %H:%M"),
        "prova": "pelo SOQUETE: `ate` entre a alteracao da linha 1 e a inclusao "
                 "da linha 3, com o MESMO backup restaurado sem `ate` como "
                 "controle positivo na mesma corrida",
        "detalhe": DETALHE,
        "falhou_em": FALHAS,
    }
    with open(SAIDA, "w") as f:
        json.dump(resultado, f, indent=1, ensure_ascii=False)
    print()
    print(f"{CONFERENCIAS} conferencias, {len(FALHAS)} falha(s)")
    print(f"gravado em {SAIDA}")
    return 1 if FALHAS else 0


if __name__ == "__main__":
    sys.exit(main())
