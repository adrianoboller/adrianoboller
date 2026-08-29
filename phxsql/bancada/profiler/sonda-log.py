#!/usr/bin/env python3
"""O ARQUIVO .txt do Profiler, provado contra o SISTEMA OPERACIONAL.

    cargo build --release -p phxsql-server --bin phxsqld
    python3 bancada/profiler/sonda-log.py

  (a) disco cheio DE VERDADE -- tmpfs de 64 KB montado so para isto
  (b) sistema de arquivos somente-leitura
  (c) o caminho e um diretorio
  (d) o caminho nao existe
  (e) reinicio: o profiler volta ligado? o arquivo sobrevive? o append segue?
  (f) rotacao: quanto o arquivo cresce por pedido

Os itens (a) e (b) precisam montar tmpfs, o que pede root. Sem root eles sao
PULADOS com recado -- pular e honesto; fingir com um diretorio 0500 nao e,
porque o bit de permissao nao vale para o uid 0 e o teste passaria por engano.

Nao usa pkill: mata so o PID que ela mesma subiu.
"""
import os
import shutil
import subprocess

from comum import AQUI, Conexao, baixar, subir

BASE = os.path.join(AQUI, "srv-log")
CHEIO = os.path.join(AQUI, "cheio")
SO_LEITURA = os.path.join(AQUI, "somenteleitura")
PORTA = 6253


def montar(ponto, opcoes):
    """Monta um tmpfs minusculo. Devolve False quando nao da (sem root)."""
    os.makedirs(ponto, exist_ok=True)
    if os.path.ismount(ponto):
        return True
    r = subprocess.run(["mount", "-t", "tmpfs", "-o", opcoes, "tmpfs", ponto],
                       capture_output=True, text=True)
    return r.returncode == 0


def desmontar(ponto):
    if os.path.ismount(ponto):
        subprocess.run(["umount", ponto], capture_output=True)


def trafego(c, n, marca):
    for i in range(n):
        c.fala({"op": "inserir", "database": "loja", "tabela": "clientes",
                "linha": {"id": i + 1, "nome": "%s-%04d" % (marca, i),
                          "obs": "x" * 60}})


def main():
    tem_cheio = montar(CHEIO, "size=64k")
    tem_ro = montar(SO_LEITURA, "size=64k,ro")
    proc = subir(BASE, PORTA)
    try:
        c = Conexao(PORTA)
        c.entrar("adm", "senha-do-adm")
        c.ok({"op": "criar_database", "database": "loja"})
        c.ok({"op": "criar_tabela", "database": "loja", "tabela": "clientes",
              "colunas": [{"nome": "id", "tipo": "Int8", "obrigatoria": True},
                          {"nome": "nome", "tipo": "Str(40)"},
                          {"nome": "obs", "tipo": "Str(80)"}],
              "indices": [{"nome": "porId", "colunas": ["id"], "unico": True,
                           "primario": True}]})

        print("=== (d) caminho cujo diretorio nao existe ===")
        r = c.fala({"op": "profiler_ligar", "arquivo": "/nao/existe/x.txt"})
        print("   %s" % (r.get("erro") or "ACEITOU -- devia ter recusado"))

        print("\n=== (c) o caminho E um diretorio ===")
        r = c.fala({"op": "profiler_ligar", "arquivo": BASE})
        print("   %s" % (r.get("erro") or "ACEITOU -- devia ter recusado"))

        print("\n=== (b) sistema de arquivos somente-leitura ===")
        if tem_ro:
            r = c.fala({"op": "profiler_ligar",
                        "arquivo": os.path.join(SO_LEITURA, "p.txt")})
            print("   %s" % (r.get("erro") or "ACEITOU -- devia ter recusado"))
        else:
            print("   PULADO: nao consegui montar tmpfs (precisa de root)")

        print("\n=== (a) DISCO CHEIO: tmpfs de 64 KB ===")
        if tem_cheio:
            alvo = os.path.join(CHEIO, "profiler.txt")
            if os.path.exists(alvo):
                os.remove(alvo)
            c.ok({"op": "profiler_ligar", "arquivo": alvo})
            trafego(c, 400, "cheio")
            e = c.ok({"op": "profiler", "max": 5})
            print("   observados no anel ........ %s" % e["observados"])
            print("   bytes no arquivo .......... %s" % os.path.getsize(alvo))
            print("   linhas no arquivo ......... %s"
                  % sum(1 for _ in open(alvo, errors="replace")))
            print("   livre no tmpfs ............ %s B"
                  % shutil.disk_usage(CHEIO).free)
            print("   gravados_bytes ............ %s"
                  % e.get("gravados_bytes", "(campo ausente)"))
            print("   falhas_de_escrita ......... %s"
                  % e.get("falhas_de_escrita", "(campo ausente)"))
            print("   >>> o servidor AVISA que parou de gravar? %s"
                  % ("sim" if e.get("falhas_de_escrita") else "NAO"))
            c.ok({"op": "profiler_desligar"})
        else:
            print("   PULADO: nao consegui montar tmpfs (precisa de root)")

        print("\n=== (e) REINICIO ===")
        alvo2 = os.path.join(BASE, "vivo.txt")
        c.ok({"op": "profiler_ligar", "arquivo": alvo2})
        trafego(c, 5, "antes")
        antes = open(alvo2, errors="replace").read()
        print("   antes: %d B, %d linha(s)" % (len(antes), len(antes.splitlines())))
        c.fechar()
        baixar(proc)
        proc = subir(BASE, PORTA, limpar=False)
        c = Conexao(PORTA)
        c.entrar("adm", "senha-do-adm")
        e = c.ok({"op": "profiler", "max": 5})
        print("   depois: ligado=%s arquivo=%r observados=%s"
              % (e["ligado"], e["arquivo"], e["observados"]))
        depois = open(alvo2, errors="replace").read()
        print("   o arquivo sobreviveu: %d B (era %d)" % (len(depois), len(antes)))
        c.ok({"op": "profiler_ligar", "arquivo": alvo2})
        trafego(c, 3, "depois")
        final = open(alvo2, errors="replace").read()
        print("   religado no MESMO arquivo: %d B, %d linha(s) -- append: %s"
              % (len(final), len(final.splitlines()),
                 "sim" if final.startswith(antes[:60]) else "NAO"))

        print("\n=== (f) ROTACAO ===")
        tam0 = os.path.getsize(alvo2)
        trafego(c, 500, "cresce")
        tam1 = os.path.getsize(alvo2)
        print("   %d B -> %d B em 500 pedidos (%.0f B/pedido)"
              % (tam0, tam1, (tam1 - tam0) / 500))
        print("   ha rotacao ou teto? %s"
              % ("sim" if tam1 <= tam0 else "NAO -- cresce sem fim"))
        e = c.ok({"op": "profiler", "max": 1})
        print("   a tela mostra o tamanho? gravados_bytes = %s"
              % e.get("gravados_bytes", "(campo ausente)"))
        c.ok({"op": "profiler_desligar"})
        c.fechar()
    finally:
        baixar(proc)
        desmontar(CHEIO)
        desmontar(SO_LEITURA)


if __name__ == "__main__":
    main()
