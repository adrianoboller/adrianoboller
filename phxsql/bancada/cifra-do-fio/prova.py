#!/usr/bin/env python3
"""A CIFRA DO FIO pelo soquete, com um cliente escrito de novo.

    cargo build --release
    python3 bancada/cifra-do-fio/prova.py

# Por que este arquivo existe, se ja ha teste em Rust

Por duas coisas que o teste em Rust nao consegue dar:

1. **Independencia.** O X25519, o ChaCha20-Poly1305, o HKDF e a maquina do
   aperto estao escritos AQUI DE NOVO, em Python, so com a biblioteca padrao.
   Os dois lados fecharem o aperto deixa de ser "o mesmo codigo concordando
   consigo mesmo" e passa a ser duas implementacoes independentes chegando a
   mesma chave. Nao e interoperabilidade certificada com o Noise -- e evidencia
   boa, e o `docs/CIFRA-DO-FIO.md` secao 9 diz exatamente ate onde ela vale.
2. **O sistema operacional.** Cortar o fio de verdade, com o servidor num
   PROCESSO separado, e a unica forma de provar que o corte vira erro e nao
   fim de sessao.

# A armadilha que esta casa ja pagou, e que este arquivo NAO repete

`socket.makefile()` segura o descritor: fechar so o soquete deixa o fd aberto
e o servidor nunca ve o fim da conexao -- e o teste passa por engano. Aqui nao
ha `makefile` em lugar nenhum: a leitura de linha e feita a mao sobre `recv`,
e o corte fecha o soquete e mais nada existe para segurar.

# O que ela prova, na ordem, com o esperado escrito ANTES

 1. o cliente VELHO (sem aperto nenhum) grava e le como sempre;
 2. o aperto fecha entre o Python e o Rust, e o mesmo trabalho acontece dentro;
 3. a chave que o servidor apresenta e a que `phxsqld --chave-do-fio` imprime;
 4. o PINO errado derruba o aperto no cliente;
 5. registro repetido nao e atendido;
 6. **fio cortado vira erro no log; despedida nao** -- os dois vereditos;
 7. registro truncado (metade de uma linha, e o soquete morre) vira erro;
 8. com `exigir` ligado: claro e recusado com erro nomeado e a conexao fecha;
    o tunel continua trabalhando;
 9. DIAGNOSTICO (nao reprova): quanto o tunel engorda o fio. E daqui que sai a
    tabela da secao 6 do docs/CIFRA-DO-FIO.md.

Sobe um phxsqld PROPRIO nas portas 7210 (dados) e 7211 (web), e mata SO o
processo que ele mesmo criou, pelo PID. Nunca `pkill` -- pode haver phxsqld de
outra pessoa na maquina.
"""
import base64
import hashlib
import hmac
import json
import os
import shutil
import socket
import struct
import subprocess
import sys
import time

AQUI = os.path.dirname(os.path.abspath(__file__))
RAIZ = os.path.abspath(os.path.join(AQUI, "..", ".."))
PHXSQLD = os.path.join(RAIZ, "target", "release", "phxsqld")
BASE = "/tmp/phx-prova-cifra-do-fio"
PORTA, PORTA_WEB = 7210, 7211
TOKEN = "o token que o tunel esconde"

falhas = []


def confere(rotulo, visto, esperado):
    ok = visto == esperado
    print(f"  {'ok  ' if ok else 'ERRO'} {rotulo}: {visto!r}"
          + ("" if ok else f"   (esperava {esperado!r})"))
    if not ok:
        falhas.append(rotulo)


def confere_contem(rotulo, texto, pedaco):
    ok = pedaco.lower() in (texto or "").lower()
    print(f"  {'ok  ' if ok else 'ERRO'} {rotulo}: {(texto or '')[:110]!r}"
          + ("" if ok else f"   (esperava conter {pedaco!r})"))
    if not ok:
        falhas.append(rotulo)


# ===========================================================================
# X25519 -- implementacao de referencia da RFC 7748, secao 5
# ===========================================================================
P = 2**255 - 19
A24 = 121665


def _decode_u(u):
    u = bytearray(u)
    u[31] &= 0x7F
    return int.from_bytes(bytes(u), "little")


def _decode_k(k):
    k = bytearray(k)
    k[0] &= 248
    k[31] &= 127
    k[31] |= 64
    return int.from_bytes(bytes(k), "little")


def x25519(k_bytes, u_bytes):
    k, u = _decode_k(k_bytes), _decode_u(u_bytes)
    x1, x2, z2, x3, z3, swap = u, 1, 0, u, 1, 0
    for t in reversed(range(255)):
        kt = (k >> t) & 1
        swap ^= kt
        if swap:
            x2, x3 = x3, x2
            z2, z3 = z3, z2
        swap = kt
        a = (x2 + z2) % P
        aa = a * a % P
        b = (x2 - z2) % P
        bb = b * b % P
        e = (aa - bb) % P
        c = (x3 + z3) % P
        d = (x3 - z3) % P
        da = d * a % P
        cb = c * b % P
        x3 = (da + cb) % P
        x3 = x3 * x3 % P
        z3 = (da - cb) % P
        z3 = x1 * (z3 * z3 % P) % P
        x2 = aa * bb % P
        z2 = e * ((aa + A24 * e) % P) % P
    if swap:
        x2, x3 = x3, x2
        z2, z3 = z3, z2
    return ((x2 * pow(z2, P - 2, P)) % P).to_bytes(32, "little")


NOVE = b"\x09" + b"\x00" * 31


def vetores_do_rfc_7748():
    """Os vetores oficiais, conferidos ANTES de esta implementacao ser usada.

    Sem isto, um erro aqui apareceria como "o servidor Rust esta errado"."""
    h = bytes.fromhex
    confere("RFC 7748 5.2, vetor 1",
            x25519(h("a546e36bf0527c9d3b16154b82465edd62144c0ac1fc5a18506a2244ba449ac4"),
                   h("e6db6867583030db3594c1a424b15f7c726624ec26b3353b10a903a6d0ab1c4c")).hex(),
            "c3da55379de9c6908e94ea4df28d084f32eccf03491c71f754b4075577a28552")
    confere("RFC 7748 5.2, vetor 2",
            x25519(h("4b66e9d4d1b4673c5ad22691957d6af5c11b6421e0ea01d42ca4169e7918ba0d"),
                   h("e5210f12786811d3f4b7959d0538ae2c31dbe7106fc03c3efc4cd549c715a493")).hex(),
            "95cbde9476e8907d7aade45cb4b873f88b595a68799fa152e6f8f7647aac7957")
    a = h("77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a")
    b = h("5dab087e624a8a4b79e17f8b83800ee66f3bb1292618b6fd1c2f8b27ff88e0eb")
    confere("RFC 7748 6.1, o segredo comum",
            x25519(a, x25519(b, NOVE)).hex(),
            "4a5d9d5ba4ce2de1728e3bf480350f25e07e21c947d19e3376f09b3c1e161742")


# ===========================================================================
# ChaCha20-Poly1305 -- RFC 8439
# ===========================================================================
def _quarto(e, a, b, c, d):
    m = 0xFFFFFFFF
    e[a] = (e[a] + e[b]) & m
    e[d] ^= e[a]
    e[d] = ((e[d] << 16) | (e[d] >> 16)) & m
    e[c] = (e[c] + e[d]) & m
    e[b] ^= e[c]
    e[b] = ((e[b] << 12) | (e[b] >> 20)) & m
    e[a] = (e[a] + e[b]) & m
    e[d] ^= e[a]
    e[d] = ((e[d] << 8) | (e[d] >> 24)) & m
    e[c] = (e[c] + e[d]) & m
    e[b] ^= e[c]
    e[b] = ((e[b] << 7) | (e[b] >> 25)) & m


def chacha20_bloco(chave, contador, nonce):
    e = [0x61707865, 0x3320646E, 0x79622D32, 0x6B206574]
    e += list(struct.unpack("<8I", chave))
    e += [contador]
    e += list(struct.unpack("<3I", nonce))
    x = list(e)
    for _ in range(10):
        _quarto(x, 0, 4, 8, 12)
        _quarto(x, 1, 5, 9, 13)
        _quarto(x, 2, 6, 10, 14)
        _quarto(x, 3, 7, 11, 15)
        _quarto(x, 0, 5, 10, 15)
        _quarto(x, 1, 6, 11, 12)
        _quarto(x, 2, 7, 8, 13)
        _quarto(x, 3, 4, 9, 14)
    return struct.pack("<16I", *[(a + b) & 0xFFFFFFFF for a, b in zip(x, e)])


def chacha20(chave, contador, nonce, dados):
    saida = bytearray()
    for i in range(0, len(dados), 64):
        bloco = chacha20_bloco(chave, contador + i // 64, nonce)
        saida += bytes(a ^ b for a, b in zip(dados[i:i + 64], bloco))
    return bytes(saida)


def poly1305(chave, mensagem):
    r = int.from_bytes(chave[:16], "little") & 0x0FFFFFFC0FFFFFFC0FFFFFFC0FFFFFFF
    s = int.from_bytes(chave[16:], "little")
    p = (1 << 130) - 5
    acc = 0
    for i in range(0, len(mensagem), 16):
        pedaco = mensagem[i:i + 16]
        n = int.from_bytes(pedaco + b"\x01", "little")
        acc = ((acc + n) * r) % p
    return ((acc + s) & ((1 << 128) - 1)).to_bytes(16, "little")


def _pad16(b):
    return b"\x00" * ((16 - len(b) % 16) % 16)


def selar(chave, nonce, aad, claro):
    cifrado = chacha20(chave, 1, nonce, claro)
    mac = poly1305(chacha20_bloco(chave, 0, nonce)[:32],
                   aad + _pad16(aad) + cifrado + _pad16(cifrado)
                   + struct.pack("<QQ", len(aad), len(cifrado)))
    return cifrado + mac


def abrir(chave, nonce, aad, pacote):
    cifrado, tag = pacote[:-16], pacote[-16:]
    mac = poly1305(chacha20_bloco(chave, 0, nonce)[:32],
                   aad + _pad16(aad) + cifrado + _pad16(cifrado)
                   + struct.pack("<QQ", len(aad), len(cifrado)))
    if not hmac.compare_digest(mac, tag):
        raise ValueError("etiqueta nao confere")
    return chacha20(chave, 1, nonce, cifrado)


# ===========================================================================
# HKDF -- RFC 5869
# ===========================================================================
def hkdf_duas(sal, material):
    prk = hmac.new(sal, material, hashlib.sha256).digest()
    a = hmac.new(prk, b"\x01", hashlib.sha256).digest()
    b = hmac.new(prk, a + b"\x02", hashlib.sha256).digest()
    return a, b


# ===========================================================================
# O aperto -- Noise_NX_25519_ChaChaPoly_SHA256
# ===========================================================================
NOME = b"Noise_NX_25519_ChaChaPoly_SHA256"
PROLOGO = b"phxsql-fio-v1"


def _nonce(n):
    return b"\x00" * 4 + struct.pack("<Q", n)


class Simetrico:
    def __init__(self):
        self.ck = NOME
        self.h = NOME
        self.k = None
        self.n = 0
        self.misturar_hash(PROLOGO)

    def misturar_hash(self, dado):
        self.h = hashlib.sha256(self.h + dado).digest()

    def misturar_chave(self, material):
        self.ck, self.k = hkdf_duas(self.ck, material)
        self.n = 0

    def cifrar_e_hash(self, claro):
        if self.k is None:
            self.misturar_hash(claro)
            return claro
        saida = selar(self.k, _nonce(self.n), self.h, claro)
        self.n += 1
        self.misturar_hash(saida)
        return saida

    def decifrar_e_hash(self, cifrado):
        if self.k is None:
            self.misturar_hash(cifrado)
            return cifrado
        claro = abrir(self.k, _nonce(self.n), self.h, cifrado)
        self.n += 1
        self.misturar_hash(cifrado)
        return claro

    def dividir(self):
        return hkdf_duas(self.ck, b"")


class Transporte:
    """As duas direcoes, cada uma com o proprio contador."""

    def __init__(self, envio, recepcao, transcricao):
        self.k_envio, self.n_envio = envio, 0
        self.k_recepcao, self.n_recepcao = recepcao, 0
        self.transcricao = transcricao

    def selar(self, tipo, conteudo=b""):
        registro = selar(self.k_envio, _nonce(self.n_envio), b"",
                         bytes([tipo]) + conteudo)
        self.n_envio += 1
        return base64.b64encode(registro).decode()

    def abrir(self, linha):
        claro = abrir(self.k_recepcao, _nonce(self.n_recepcao), b"",
                      base64.b64decode(linha))
        self.n_recepcao += 1
        return claro[0], claro[1:]


PEDIDO, FIM = 1, 2


# ===========================================================================
# O servidor e o cliente
# ===========================================================================
class Servidor:
    """Um phxsqld nosso. Morre pelo PID, e so ele."""

    def __init__(self, limpar=True, exigir=False):
        if limpar:
            shutil.rmtree(BASE, ignore_errors=True)
            os.makedirs(BASE, exist_ok=True)
        with open(os.path.join(BASE, "config.json"), "w") as f:
            json.dump({"base": "base", "bind": f"127.0.0.1:{PORTA}",
                       "token": TOKEN,
                       "cifra_fio": {"ligada": True, "exigir": exigir},
                       "web": {"ligado": False,
                               "bind": f"127.0.0.1:{PORTA_WEB}"}}, f, indent=2)
        log = open(os.path.join(BASE, "servidor.log"), "a")
        self.proc = subprocess.Popen([PHXSQLD], cwd=BASE, stdout=log,
                                     stderr=subprocess.STDOUT,
                                     stdin=subprocess.DEVNULL)
        self.esperar()

    def esperar(self):
        ate = time.time() + 20
        while time.time() < ate:
            try:
                socket.create_connection(("127.0.0.1", PORTA), 0.3).close()
                return
            except OSError:
                time.sleep(0.1)
        sys.exit("o servidor nao subiu na porta 7210")

    def parar(self):
        self.proc.terminate()
        self.proc.wait(timeout=10)


class Cliente:
    """Sem `makefile`: o buffer de linha e nosso, e nada segura o descritor."""

    def __init__(self, prazo=10):
        self.s = socket.create_connection(("127.0.0.1", PORTA), prazo)
        self.s.settimeout(prazo)
        self.buffer = b""
        self.tunel = None

    def _linha(self):
        while b"\n" not in self.buffer:
            pedaco = self.s.recv(65536)
            if not pedaco:
                return None
            self.buffer += pedaco
        linha, self.buffer = self.buffer.split(b"\n", 1)
        return linha.decode()

    def cru(self, texto):
        self.s.sendall((texto + "\n").encode())

    def fala(self, pedido):
        pedido.setdefault("token", TOKEN)
        linha = json.dumps(pedido)
        if self.tunel:
            self.cru(self.tunel.selar(PEDIDO, linha.encode()))
            volta = self._linha()
            if volta is None:
                return None
            tipo, conteudo = self.tunel.abrir(volta)
            if tipo == FIM:
                return None
            return json.loads(conteudo.decode())
        self.cru(linha)
        volta = self._linha()
        return json.loads(volta) if volta is not None else None

    def ok(self, pedido):
        r = self.fala(pedido)
        if not r or not r.get("ok"):
            sys.exit(f"FALHOU {pedido.get('op')}: {r}")
        return r["resultado"]

    def cifrar(self, pino=None):
        efemera = os.urandom(32)
        s = Simetrico()
        publica = x25519(efemera, NOVE)
        s.misturar_hash(publica)
        s.cifrar_e_hash(b"")
        self.cru(json.dumps({"op": "cifrar",
                             "e": base64.b64encode(publica).decode()}))
        resposta = json.loads(self._linha())
        if not resposta.get("ok"):
            raise ValueError(resposta.get("erro", "recusado"))
        m2 = base64.b64decode(resposta["resultado"]["m2"])
        if len(m2) != 96:
            raise ValueError(f"mensagem 2 com {len(m2)} bytes")

        s.misturar_hash(m2[:32])
        s.misturar_chave(x25519(efemera, m2[:32]))
        estatica = s.decifrar_e_hash(m2[32:80])
        if pino is not None and not hmac.compare_digest(estatica, pino):
            raise ValueError("a chave do servidor nao e a do pino")
        s.misturar_chave(x25519(efemera, estatica))
        s.decifrar_e_hash(m2[80:])
        envio, recepcao = s.dividir()
        self.tunel = Transporte(envio, recepcao, s.h)
        return estatica

    def despedir(self):
        self.cru(self.tunel.selar(FIM))

    def fechar(self):
        # SO o soquete, e nao ha mais nada segurando o descritor -- e por isso
        # que o servidor VE o fim da conexao.
        self.s.close()


def trabalhar(c, marca):
    """Um ciclo de vida de dado: criar, inserir, ler de volta."""
    base = f"fio_{marca}"
    c.ok({"op": "criar_database", "database": base})
    c.ok({"op": "criar_tabela", "database": base, "tabela": "cidades",
          "colunas": [{"nome": "n", "tipo": "Int8"},
                      {"nome": "nome", "tipo": "Str(40)"}]})
    c.ok({"op": "inserir", "database": base, "tabela": "cidades",
          "linha": {"n": 1, "nome": "Blumenau"}})
    linhas = c.ok({"op": "varrer", "database": base, "tabela": "cidades"})
    return [l.get("nome") for l in linhas.get("linhas", [])]


def linhas_do_log():
    caminho = os.path.join(BASE, "acessos.log")
    if not os.path.exists(caminho):
        return []
    with open(caminho, encoding="utf-8", errors="replace") as f:
        return [l for l in f.read().splitlines() if l.strip()]


def esperar_no_log(pedaco, quantas, prazo=10):
    """Espera por CONDICAO, e nao por tempo fixo -- o log e escrito noutra
    linha de execucao, e dormir um tempo fixo passa aqui e falha na proxima
    maquina."""
    ate = time.time() + prazo
    while time.time() < ate:
        achadas = [l for l in linhas_do_log() if pedaco in l]
        if len(achadas) >= quantas:
            return achadas
        time.sleep(0.1)
    return [l for l in linhas_do_log() if pedaco in l]


def medir_a_moldura():
    """Quanto o tunel engorda o fio, sobre registros selados de VERDADE.

    Existe porque o documento chegou a dizer «+33%», que e a expansao do
    Base64 no LIMITE -- e nao o que se paga num pedido curto, em que os 17
    bytes fixos (1 de tipo, 16 de etiqueta) pesam mais que a expansao. O
    numero do `docs/CIFRA-DO-FIO.md` secao 6 sai daqui.
    """
    t = Transporte(os.urandom(32), os.urandom(32), b"")
    casos = [
        ('{"op":"ping","token":"' + TOKEN + '"}', "um ping com token"),
        (json.dumps({"op": "inserir", "database": "loja", "tabela": "clientes",
                     "linha": {"id": 1, "nome": "Adriano Boller",
                               "cidade": "Blumenau"}, "token": TOKEN}),
         "uma insercao de uma linha"),
        ("x" * 5000, "um lote de ~5 KiB"),
        ("x" * 200000, "uma resposta de ~200 KiB"),
    ]
    print(f"  {'o que passa':28} {'em claro':>9} {'no fio':>9} {'a mais':>8}")
    for linha, nome in casos:
        claro = len(linha) + 1              # a linha e o \n
        fio = len(t.selar(PEDIDO, linha.encode())) + 1
        print(f"  {nome:28} {claro:9} {fio:9} {(fio / claro - 1) * 100:7.1f}%")
    print("\n  O pedido PEQUENO e o que paga caro -- e e o que o protocolo "
          "mais faz.")


def main():
    if not os.path.exists(PHXSQLD):
        sys.exit(f"binario ausente: {PHXSQLD}\n  rode antes: cargo build --release")

    print("\n=== 0. os vetores da RFC 7748, no cliente Python ===\n")
    vetores_do_rfc_7748()

    s = Servidor(limpar=True, exigir=False)
    try:
        print("\n=== 1. o cliente VELHO, que nunca ouviu falar do aperto ===\n")
        c = Cliente()
        confere("ping em claro", c.fala({"op": "ping"}).get("ok"), True)
        confere("grava e le em claro", trabalhar(c, "velho"), ["Blumenau"])
        c.fechar()
        confere("e o servidor NAO criou a chave do fio",
                os.path.exists(os.path.join(BASE, "chave-do-fio.hex")), False)

        print("\n=== 2. o aperto entre o Python e o Rust ===\n")
        c = Cliente()
        estatica = c.cifrar()
        confere("a estatica tem 32 bytes", len(estatica), 32)
        confere("ping por dentro do tunel", c.fala({"op": "ping"}).get("ok"), True)
        confere("grava e le por dentro do tunel",
                trabalhar(c, "tunel"), ["Blumenau"])

        print("\n=== 3. a chave apresentada e a que o phxsqld imprime ===\n")
        impressa = subprocess.run(
            [PHXSQLD, "--config", os.path.join(BASE, "config.json"),
             "--chave-do-fio"],
            capture_output=True, text=True, cwd=BASE).stdout.strip()
        confere("--chave-do-fio bate com a apresentada",
                impressa, estatica.hex())

        print("\n=== 4. o pino errado derruba o aperto ===\n")
        c2 = Cliente()
        try:
            c2.cifrar(pino=bytes(32))
            confere("pino errado", "passou", "tinha de cair")
        except ValueError as e:
            confere_contem("pino errado cai no cliente", str(e), "pino")
        c2.fechar()

        print("\n=== 5. registro repetido nao e atendido ===\n")
        c3 = Cliente()
        c3.cifrar()
        registro = c3.tunel.selar(PEDIDO, json.dumps(
            {"op": "ping", "token": TOKEN}).encode())
        c3.cru(registro)
        confere("o primeiro passa", c3._linha() is not None, True)
        c3.cru(registro)
        confere("o repetido nao", c3._linha(), None)
        c3.fechar()

        print("\n=== 6. fio cortado e despedida sao vereditos DIFERENTES ===\n")
        antes = len([l for l in linhas_do_log() if '"op":"fio"' in l])
        # (a) corta: pede, e o soquete morre sem despedida.
        corte = Cliente()
        corte.cifrar()
        corte.fala({"op": "ping"})
        corte.fechar()
        # (b) despede-se: pede, manda FIM, e fecha.
        limpo = Cliente()
        limpo.cifrar()
        limpo.fala({"op": "ping"})
        limpo.despedir()
        limpo.fechar()
        achadas = esperar_no_log('"op":"fio"', antes + 1)
        confere("o log ganhou UM corte (o (a)), e nao dois",
                len(achadas) - antes, 1)
        confere_contem("e o corte diz o que foi", achadas[-1], "cortado")

        print("\n=== 7. registro truncado tambem e erro ===\n")
        antes = len([l for l in linhas_do_log() if '"op":"fio"' in l])
        meio = Cliente()
        meio.cifrar()
        registro = meio.tunel.selar(PEDIDO, b'{"op":"ping"}')
        # Metade de um registro, SEM o \\n, e o soquete morre.
        meio.s.sendall(registro[: len(registro) // 2].encode())
        meio.fechar()
        achadas = esperar_no_log('"op":"fio"', antes + 1)
        confere("o truncado virou erro no log", len(achadas) - antes, 1)
        c.fechar()
        c3 = None
    finally:
        s.parar()

    print("\n=== 8. com `exigir` ligado ===\n")
    s = Servidor(limpar=False, exigir=True)
    try:
        velho = Cliente()
        r = velho.fala({"op": "ping"})
        confere("claro e recusado", r.get("ok"), False)
        confere_contem("e a recusa diz o que fazer", r.get("erro"), "cifrar")
        confere("e a conexao fecha", velho._linha(), None)
        velho.fechar()

        novo = Cliente()
        novo.cifrar()
        confere("o tunel continua trabalhando",
                trabalhar(novo, "exigir"), ["Blumenau"])
        novo.fechar()
    finally:
        s.parar()

    print("\n=== 9. o preco da moldura, MEDIDO (nao reprova: e diagnostico) ===\n")
    medir_a_moldura()

    print()
    if falhas:
        print(f"REPROVOU: {len(falhas)} — {', '.join(falhas)}")
        return 1
    print("PASSOU: o aperto fecha entre duas implementacoes independentes, "
          "o cliente velho nao sentiu nada, e o fio cortado nao virou sucesso.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
