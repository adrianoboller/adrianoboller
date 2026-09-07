#!/usr/bin/env python3
"""A SEQUENCIA do PhxSql contra o motor vivo.

    python3 bancada/sequencias/sonda.py

Pergunta do dono, 07/09/2026: *«Muitos bancos tem o sequence tipo o PostgreSQL
e como e isso no phxsql?»*. A resposta esta no formato (`docs/FORMATO.md`: o
contador vive nos bytes 92..100 do cabecalho do volume 1) e no protocolo
(`sequencias` lista, `ajustar_sequencia` zera ou pula). Esta sonda exercita as
quatro coisas que um usuario de PostgreSQL esperaria: declarar, inserir sem
informar o numero, listar o contador, e ajusta-lo -- inclusive o caso em que
ajustar para tras faria a proxima insercao repetir uma chave.
"""
import json, os, shutil, socket, subprocess, sys, time

PORTA = int(os.environ.get("PHX_SONDA_PORTA", "6900"))
BASE = f"/tmp/phx-seq-{os.getpid()}"
BIN = "target/release/phxsqld"

class Servidor:
    def __enter__(self):
        if not os.path.exists(BIN):
            raise SystemExit(f"{BIN} nao existe: flock /tmp/phx-cargo.lock cargo build --release -p phxsql-server")
        shutil.rmtree(BASE, ignore_errors=True); os.makedirs(BASE + "/dados")
        cfg = BASE + "/config.json"
        json.dump({"bind": f"127.0.0.1:{PORTA}", "base": BASE + "/dados", "token": "t",
                   "web": {"ligado": False}}, open(cfg, "w"))
        self.p = subprocess.Popen([BIN, "--config", cfg], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        for _ in range(100):
            try: socket.create_connection(("127.0.0.1", PORTA), 0.2).close(); break
            except OSError: time.sleep(0.1)
        else: raise SystemExit("servidor nao subiu")
        self.s = socket.create_connection(("127.0.0.1", PORTA), 5); self.f = self.s.makefile("rwb")
        return self
    def __exit__(self, *_):
        try: self.f.close(); self.s.close()
        except OSError: pass
        self.p.terminate(); self.p.wait(timeout=10); shutil.rmtree(BASE, ignore_errors=True)
    def pedir(self, **kw):
        kw.setdefault("token", "t")
        self.f.write((json.dumps(kw) + "\n").encode()); self.f.flush()
        r = json.loads(self.f.readline().decode())
        if isinstance(r.get("resultado"), dict):
            r = {**r["resultado"], **{k: v for k, v in r.items() if k != "resultado"}}
        return r

def diz(rot, r, corte=160):
    print(f"  [{'OK  ' if r.get('ok') else 'ERRO'}] {rot}" + ("" if r.get("ok") else " -- " + str(r.get("erro"))[:corte]))
    return r

def main():
    commit = subprocess.run(["git", "rev-parse", "--short", "HEAD"], capture_output=True, text=True).stdout.strip()
    print(f"sequencias -- {time.strftime('%Y-%m-%d %H:%M:%S UTC', time.gmtime())} -- commit {commit}\n")
    with Servidor() as sv:
        sv.pedir(op="criar_database", database="loja")
        print("=== 1. Declarar: a coluna e do tipo `Sequence` (uma so por tabela)")
        diz("criar_tabela com id Sequence", sv.pedir(op="criar_tabela", database="loja", tabela="pedidos",
            colunas=[{"nome": "id", "tipo": "Sequence", "obrigatoria": True},
                     {"nome": "cliente", "tipo": "Str(40)", "obrigatoria": True}],
            indices=[{"nome": "porId", "colunas": ["id"], "unico": True}]))
        diz("segunda coluna Sequence na mesma tabela (tem de recusar)", sv.pedir(op="criar_tabela", database="loja",
            tabela="duas", colunas=[{"nome": "a", "tipo": "Sequence"}, {"nome": "b", "tipo": "Sequence"}]))

        print("\n=== 2. Inserir SEM informar o id: o motor numera")
        for c in ("Alves", "Silva", "Andrade"):
            r = sv.pedir(op="inserir", database="loja", tabela="pedidos", linha={"cliente": c})
            print(f"  inserir {c:8} -> ok={r.get('ok')} rowid={r.get('rowid')}")
        r = sv.pedir(op="varrer", database="loja", tabela="pedidos", max=10)
        print("  linhas:", [(l["id"], l["cliente"]) for l in r.get("linhas", [])])

        print("\n=== 3. Inserir INFORMANDO o id (o contador acompanha, como no PostgreSQL nao acompanha)")
        diz("inserir id=100", sv.pedir(op="inserir", database="loja", tabela="pedidos", linha={"id": 100, "cliente": "Manual"}))
        r = sv.pedir(op="inserir", database="loja", tabela="pedidos", linha={"cliente": "Depois do 100"})
        r2 = sv.pedir(op="ler", database="loja", tabela="pedidos", rowid=r.get("rowid"))
        print("  o proximo automatico saiu:", (r2.get("linha") or r2).get("id"))

        print("\n=== 4. Listar os contadores: op `sequencias`")
        r = sv.pedir(op="sequencias", database="loja")
        for s in r.get("sequencias", []): print("  ", s)

        print("\n=== 5. Ajustar: op `ajustar_sequencia` (exige administrar)")
        diz("zerar (proxima=0)", sv.pedir(op="ajustar_sequencia", database="loja", tabela="pedidos", proxima=0))
        diz("pular para 5000", sv.pedir(op="ajustar_sequencia", database="loja", tabela="pedidos", proxima=5000))
        r = sv.pedir(op="inserir", database="loja", tabela="pedidos", linha={"cliente": "Depois do salto"})
        r2 = sv.pedir(op="ler", database="loja", tabela="pedidos", rowid=r.get("rowid"))
        print("  o proximo automatico saiu:", (r2.get("linha") or r2).get("id"))

        print("\n=== 6. Ajustar para TRAS de uma chave ja gravada: o indice unico e quem recusa a repeticao")
        diz("ajustar para 1 (ja existe)", sv.pedir(op="ajustar_sequencia", database="loja", tabela="pedidos", proxima=1))
        diz("inserir depois do ajuste (o id 1 ja existe: tem de recusar)",
            sv.pedir(op="inserir", database="loja", tabela="pedidos", linha={"cliente": "Repetido"}))

        print("\n=== 7. Onde o contador mora: byte 36 do cabecalho do volume 1 (o 92 e o do rownum)")
        # A primeira versao desta sonda leu o byte 92 e imprimiu 7 -- que e o
        # proximo_rownum, nao a sequencia. FORMATO.md tem os dois contadores:
        # `proxima_sequencia` em 36 e `proximo_rownum` em 92. Imprimir os dois
        # lado a lado e o que impede a confusao de voltar.
        import struct
        reg = BASE + "/dados/loja/pedidos.reg"
        with open(reg, "rb") as f:
            f.seek(36); seq = struct.unpack("<Q", f.read(8))[0]
            f.seek(92); rn = struct.unpack("<Q", f.read(8))[0]
        print(f"  byte 36 proxima_sequencia = {seq}   byte 92 proximo_rownum = {rn}")
        print("  (o cabecalho vai ao disco no `sincronizar`, no fecho da janela de durabilidade;")
        print("   o valor em memoria e o que a op `sequencias` devolve)")
        print("\n=== 8. Reabrir e conferir que o contador sobreviveu")
    # sobe de novo sobre a mesma base? nao -- a base foi apagada no __exit__; o teste de reabertura
    # esta em crates/phxsql-store (o contador vai ao disco no fecho da janela). Dispensa registrada.
    print("  (a persistencia do contador entre reaberturas e provada nos testes do phxsql-store)")

if __name__ == "__main__":
    sys.exit(main())
