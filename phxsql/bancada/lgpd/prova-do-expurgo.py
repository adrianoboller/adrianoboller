#!/usr/bin/env python3
"""A PROVA do expurgo da trilha `.lgpd` contra o sistema operacional (pedido 368).

    cargo build --release -p phxsql-server --bin phxsqld   # binario velho mede o passado
    python3 bancada/lgpd/prova-do-expurgo.py

# Por que um script, e nao so os testes

Os testes do motor conferem o diretorio com `read_dir`, e isso ja e o sistema
de arquivos. O que so um servidor DE VERDADE prova e o caminho inteiro: o
`config.json` lido pelo binario (`lgpd.volume_mib` cortando o volume da
trilha, `lgpd.retencao_anos` subindo o relogio), a op pelo soquete com o
portao de permissao do cadastro, e o volume sumindo do diretorio que o `ls`
enxerga -- enquanto o processo continua vivo e atendendo.

A tabela e a PADRAO do protocolo, sem `registros_por_arquivo` -- a que, antes
do formato B (pedido 368), tinha a trilha num arquivo so que nunca fechava e
nunca se expurgava. O ativo e `clientes.lgpd`; os fechados, `clientes_NNN.lgpd`.

# O que ele confere

1. a trilha da tabela padrao vira volume de verdade (>= 3 arquivos `.lgpd`);
2. um usuario SEM `administrar` e recusado, e nada sai;
3. `ate` no futuro e recusado, e nada sai;
4. o expurgo do administrador deixa SO o volume ativo no diretorio, e a
   resposta diz o mesmo que o `ls`;
5. o rastro esta no `.reason` (op `motivos`) sem a chave de linha;
6. a op `trilha` continua lendo, e o total bate com o `restam`;
7. a trilha continua CRESCENDO depois, no volume ativo -- e nunca volta ao 1;
8. `fechar_ativo` fecha o ativo por pedido, e ele sai no mesmo expurgo;
9. o RELOGIO, sem administrador nenhum, expurga PELO PRAZO: com o servidor
   parado, os carimbos de um volume fechado vao para seis anos atras e os do
   ativo para quatro (CRC refeito; o expurgo decide pelo carimbo); o servidor
   sobe de novo e, na primeira passada, fecha o ativo velho por idade, derruba
   o volume de seis anos e guarda o de quatro -- com o rastro assinado pelo
   proprio servidor (usuario 0).

O passo 9 custa um minuto: a primeira passada do relogio e um minuto depois
do arranque.

Sobe um `phxsqld` proprio na porta `PHX_PORTA_EXPURGO` (7368). Derruba so o
PID que criou -- nunca `pkill`.
"""
import json
import os
import shutil
import struct
import sys
import time
import zlib

AQUI = os.path.dirname(os.path.abspath(__file__))
RAIZ = os.environ.get("PHX_RAIZ", os.path.abspath(os.path.join(AQUI, "..", "..")))
sys.path.insert(0, os.path.join(RAIZ, "bancada", "profiler"))

from comum import TOKEN, Conexao, baixar, hash_da_senha, subir  # noqa: E402

PORTA = int(os.environ.get("PHX_PORTA_EXPURGO", "7368"))
BASE = os.path.join(AQUI, ".base-da-prova")
DB = "loja"
TAB = "clientes"
CPF = "01234567890"


def config():
    return {
        # bancada de teste, NAO cliente do produto -- fala em claro para medir "A PROVA do expurgo da trilha .lgpd contra o sistema operacional" sem o aperto de mao no meio (servidor exige a cifra por padrao desde o pedido 370)
        "base": "base", "bind": "127.0.0.1:%d" % PORTA, "cifra_fio": {"exigir": False},
        "token": TOKEN,
        "web": {"ligado": False},
        # 1 MiB: o menor corte que o config aceita, para a trilha virar
        # volume em segundos e nao em anos. E o corte da TRILHA (formato B),
        # e nao o dos diarios: a tabela nem e paginada.
        "lgpd": {"retencao_anos": 5, "volume_mib": 1},
        "usuarios": [
            {"login": "adm", "nome": "Adriano", "id": 10, "nivel": "admin",
             "senha_hash": hash_da_senha("senha-do-adm"),
             "bases": {"*": {"ler": True, "inserir": True, "alterar": True,
                             "excluir": True, "criar": True,
                             "administrar": True, "verificar": True}}},
            {"login": "leitor", "nome": "Leitor", "id": 12, "nivel": "leitor",
             "senha_hash": hash_da_senha("senha-do-leitor"),
             "bases": {"*": {"ler": True}}},
        ],
    }


def volumes_no_disco():
    pasta = os.path.join(BASE, "base", DB)
    return sorted(n for n in os.listdir(pasta) if n.startswith(TAB) and n.endswith(".lgpd"))


def envelhecer(caminho, carimbo_ms):
    """Poe `carimbo_ms` em todo registro de um volume `.lgpd` em claro, e
    refaz o CRC de cada um. Confere o CRC de ANTES pela mesma regua, entao uma
    divergencia entre esta conta e a do motor para aqui, e nao vira um volume
    que o servidor recusa por outro motivo."""
    b = bytearray(open(caminho, "rb").read())
    cab_len = struct.unpack_from("<H", b, 10)[0]
    fim = struct.unpack_from("<Q", b, 24)[0]
    if cab_len >= 128 and b[40] & 1:
        raise SystemExit("%s esta cifrado: esta prova envelhece so em claro" % caminho)
    off, n = cab_len, 0
    while off + 56 <= fim:
        u16 = lambda o: struct.unpack_from("<H", b, off + o)[0]  # noqa: E731
        total = 56 + u16(10) + u16(40) + u16(42) + u16(48) + b[off + 50]

        def crc():
            return zlib.crc32(bytes(b[off + 56:off + total]), zlib.crc32(bytes(b[off:off + 52])))
        if crc() != struct.unpack_from("<I", b, off + 52)[0]:
            raise SystemExit("o CRC do registro em %d de %s nao bate com a regua do script"
                             % (off, caminho))
        struct.pack_into("<q", b, off, carimbo_ms)
        struct.pack_into("<I", b, off + 52, crc())
        off += total
        n += 1
    with open(caminho, "wb") as f:
        f.write(b)
    return n


def rajada(con, pedidos):
    """Os pedidos de uma vez, e so depois as respostas, na mesma ordem.

    Um a um, cada pedido paga a ida e volta do soquete. Em rajada o servidor
    atende na mesma ordem e o que a prova confere nao muda.
    """
    con.f.write(b"".join((json.dumps(dict(p, token=TOKEN)) + "\n").encode()
                         for p in pedidos))
    con.f.flush()
    for p in pedidos:
        r = json.loads(con.f.readline().decode())
        if not r.get("ok"):
            raise SystemExit("%s: %s" % (p["op"], r.get("erro")))


def alteracoes(n, desde):
    """`n` alteracoes da coluna marcada `obs`, cada uma com antes e depois de
    900 bytes: um registro de trilha de ~1,9 KiB, para um volume de 1 MiB
    fechar em ~550 alteracoes e nao em milhares de leituras."""
    return [{"op": "atualizar", "database": DB, "tabela": TAB, "rowid": 1,
             "valores": {"id": 1, "cpf": CPF,
                         "obs": ("%06d" % ((desde + i) % 1000000)) * 150}}
            for i in range(n)]


falhas = []


def conferir(condicao, frase):
    print(("   ok    " if condicao else "   FALHA ") + frase)
    if not condicao:
        falhas.append(frase)


def main():
    p = subir(BASE, PORTA, config())
    try:
        adm = Conexao(PORTA)
        adm.entrar("adm", "senha-do-adm")
        adm.ok({"op": "criar_database", "database": DB})
        adm.ok({"op": "criar_tabela", "database": DB, "tabela": TAB,
                "colunas": [{"nome": "id", "tipo": "Int8", "obrigatoria": True},
                            {"nome": "cpf", "tipo": "Str(14)"},
                            {"nome": "obs", "tipo": "Str(1000)"}],
                "indices": [{"nome": "porId", "colunas": ["id"],
                             "unico": True, "primario": True}]})
        adm.ok({"op": "marcar_lgpd", "database": DB, "tabela": TAB,
                "colunas": {"cpf": "pessoal", "obs": "pessoal"}})
        adm.ok({"op": "inserir", "database": DB, "tabela": TAB,
                "linha": {"id": 1, "cpf": CPF}})
        comeco = time.time()
        feitas = 0
        # O teto e de ALTERACOES, e nao so de tempo: tres volumes de 1 MiB sao
        # ~1.650 alteracoes, e uma trilha que nao fecha volume (a tabela
        # padrao antes do formato B) encheria o disco em minutos -- medido:
        # 1,87 GB num arquivo so antes de o teto de tempo chegar.
        while len(volumes_no_disco()) < 3:
            if feitas >= 20000 or time.time() - comeco > 300:
                raise SystemExit(
                    "a trilha da tabela padrao nao virou volume: %d alteracoes em "
                    "%.1f s, e no disco %s"
                    % (feitas, time.time() - comeco, volumes_no_disco()))
            rajada(adm, alteracoes(200, feitas))
            feitas += 200
        antes = volumes_no_disco()
        print("trilha: %d alteracoes em %.1f s gravaram %d volumes: %s"
              % (feitas, time.time() - comeco, len(antes), antes))
        conferir(len(antes) >= 3, "a trilha virou volume de verdade no diretorio")

        # +1: o ultimo registro pode ser do mesmo milissegundo, e o limite
        # derruba so o que e ANTERIOR a ele.
        agora_ms = int(time.time() * 1000) + 1
        leitor = Conexao(PORTA)
        leitor.entrar("leitor", "senha-do-leitor")
        r = leitor.fala({"op": "expurgar_trilha", "database": DB, "tabela": TAB,
                         "ate_ms": agora_ms, "motivo": "tentativa"})
        conferir(not r.get("ok"), "usuario sem administrar e recusado: %s" % r.get("erro"))
        conferir(volumes_no_disco() == antes, "a recusa nao apagou nada")

        r = adm.fala({"op": "expurgar_trilha", "database": DB, "tabela": TAB,
                      "ate": "2099-01-01", "motivo": "futuro"})
        conferir(not r.get("ok") and "futuro" in (r.get("erro") or ""),
                 "ate no futuro e recusado: %s" % r.get("erro"))
        conferir(volumes_no_disco() == antes, "a recusa do futuro nao apagou nada")

        res = adm.ok({"op": "expurgar_trilha", "database": DB, "tabela": TAB,
                      "ate_ms": agora_ms, "motivo": "prova do pedido 368"})
        depois = volumes_no_disco()
        ativo = TAB + ".lgpd"
        print("expurgo: %d volume(s), %d registro(s), parada=%s, restam=%d; no disco: %s"
              % (len(res["volumes"]), res["registros"], res["parada"], res["restam"], depois))
        conferir(depois == [ativo], "so o volume ativo ficou no diretorio (%s)" % ativo)
        conferir(len(res["volumes"]) == len(antes) - 1,
                 "a resposta diz o mesmo que o ls: %d volumes saíram" % len(res["volumes"]))
        conferir(res["parada"] == "volume_ativo", "parou no volume ativo")

        motivos = adm.ok({"op": "motivos", "database": DB, "tabela": TAB})["motivos"]
        rastro = [m for m in motivos if m.get("tipo") == "expurgo"]
        conferir(len(rastro) == 1, "um rastro de expurgo no .reason")
        if rastro:
            ident = rastro[0].get("identidade", "")
            print("rastro: %s | motivo: %s" % (ident, rastro[0].get("motivo")))
            conferir(ident.startswith(".lgpd volumes "), "o rastro nomeia os volumes")
            conferir(rastro[0].get("expurgo_da_trilha") is True,
                     "o rastro traz o bit do expurgo da trilha (C1), e nao so o texto")
            conferir(CPF not in ident, "o rastro NAO carrega a chave de linha")

        t = adm.ok({"op": "trilha", "database": DB, "tabela": TAB, "limite": 0})
        conferir(t["total"] == res["restam"] == len(t["registros"]),
                 "a op trilha le o que sobrou: total %d, restam %d, lidos %d"
                 % (t["total"], res["restam"], len(t["registros"])))

        for _ in range(20):
            rajada(adm, alteracoes(200, feitas))
            feitas += 200
            if len(volumes_no_disco()) > 1:
                break
        final = volumes_no_disco()
        print("depois de crescer: %s" % final)
        numero = len(antes)  # o ativo de antes era o volume len(antes)
        conferir(final == [ativo, "%s_%03d.lgpd" % (TAB, numero)],
                 "o ativo fechou como _%03d e o seguinte nasceu no nome fixo" % numero)

        # 8. O fechamento pedido: o ativo com registro fecha e sai junto.
        agora_ms = int(time.time() * 1000) + 1
        res = adm.ok({"op": "expurgar_trilha", "database": DB, "tabela": TAB,
                      "ate_ms": agora_ms, "motivo": "fechar e levar", "fechar_ativo": True})
        print("fechar_ativo: fechou=%s, %d volume(s) sairam; no disco: %s"
              % (res["fechou"], len(res["volumes"]), volumes_no_disco()))
        conferir(res["fechou"] == numero + 1, "fechar_ativo fechou o ativo %d" % (numero + 1))
        conferir(volumes_no_disco() == [ativo] and res["restam"] == 0,
                 "o que fechou a pedido saiu no mesmo expurgo; sobra o ativo vazio")

        # 9. O RELOGIO, pelo prazo. Enche ate um volume fechar por tamanho e o
        # ativo seguinte ter registro; para o servidor; envelhece os dois.
        while len(volumes_no_disco()) < 2:
            rajada(adm, alteracoes(200, feitas))
            feitas += 200
        cheio = [n for n in volumes_no_disco() if n != ativo][0]
        adm.fechar()
        leitor.fechar()
        baixar(p)
        pasta = os.path.join(BASE, "base", DB)
        agora = int(time.time() * 1000)
        ano = 366 * 86400000
        velhos = envelhecer(os.path.join(pasta, cheio), agora - 6 * ano)
        dentro = envelhecer(os.path.join(pasta, ativo), agora - 4 * ano)
        print("envelhecidos: %s com %d registro(s) de seis anos; %s com %d de quatro"
              % (cheio, velhos, ativo, dentro))
        numero_ativo = int(cheio[len(TAB) + 1:-len(".lgpd")]) + 1
        p = subir(BASE, PORTA, config(), limpar=False)
        comeco = time.time()
        while cheio in volumes_no_disco() and time.time() - comeco < 120:
            time.sleep(1)
        print("primeira passada do relogio em %.0f s; no disco: %s"
              % (time.time() - comeco, volumes_no_disco()))
        conferir(volumes_no_disco() == [ativo, "%s_%03d.lgpd" % (TAB, numero_ativo)],
                 "o relogio derrubou o volume de seis anos, fechou o ativo de quatro "
                 "por idade e o guardou")
        with open(os.path.join(BASE, "servidor.log")) as f:
            passadas = [l.strip() for l in f if l.startswith("retencao da trilha:")]
        print("log: %s" % (passadas[-1] if passadas else "(nenhuma passada)"))
        conferir(bool(passadas) and ": 1 volume(s) e %d registro(s)" % velhos in passadas[-1]
                 and "1 ativo(s) fechado(s) por idade" in passadas[-1],
                 "a linha do relogio no log diz o mesmo que o ls")
        adm = Conexao(PORTA)
        adm.entrar("adm", "senha-do-adm")
        motivos = adm.ok({"op": "motivos", "database": DB, "tabela": TAB})["motivos"]
        do_relogio = [m for m in motivos if m.get("tipo") == "expurgo"
                      and m.get("usuario") == 0]
        conferir(len(do_relogio) == 1 and do_relogio[0].get("expurgo_da_trilha") is True
                 and "lgpd.retencao_anos" in (do_relogio[0].get("motivo") or ""),
                 "o rastro do relogio esta no .reason, assinado pelo servidor (usuario 0)")
        t = adm.ok({"op": "trilha", "database": DB, "tabela": TAB, "limite": 0})
        conferir(t["total"] == dentro == len(t["registros"]),
                 "a trilha le os %d registro(s) de quatro anos que ficaram" % dentro)
        adm.fechar()
    finally:
        baixar(p)
    if falhas:
        # A base fica para quem for olhar o que reprovou.
        print("\nREPROVADA: %d falha(s); a base ficou em %s" % (len(falhas), BASE))
        sys.exit(1)
    shutil.rmtree(BASE, ignore_errors=True)
    print("\nA prova segura.")


if __name__ == "__main__":
    main()
