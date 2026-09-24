#!/usr/bin/env python3
"""Servidor FALSO do PhxZip na web: responde o contrato de docs/PHXZIP-WEB.md.

Existe para a tela se provar no navegador ANTES de o servidor de verdade
existir (pedido 455: a interface comeca ja, contra um servidor falso). Segue o
contrato a letra -- rotas, envelope `PZW1`, nomes de erro, limites, cabecalhos
de seguranca --, e quando os dois discordarem o CONTRATO manda, nao este
arquivo.

O que ele NAO e: um compactador. O «pacote» que devolve comeca com a assinatura
do 7z e guarda os itens num formato so dele (JSON + zlib), para a volta
compactar -> abrir -> espiar -> baixar fechar no navegador. O 7-Zip nao o abre.

Os cenarios de erro saem do CONTEUDO do pacote enviado (como no servidor de
verdade), nunca do nome do arquivo: um pacote que comeca com a assinatura do 7z
seguida de `\\0CENARIO:<nome>\\n` dispara o cenario. O roteiro `exercitar.mjs`
escreve esses arquivos num diretorio temporario.

    python3 testes-web/phxzip/servidor_falso.py --porta 7799 --log /tmp/falso.log

So biblioteca padrao. Preso ao 127.0.0.1, como o de verdade.
"""
import argparse
import hashlib
import io
import json
import re
import sys
import tarfile
import threading
import time
import zlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import parse_qs, urlsplit

RAIZ = Path(__file__).resolve().parents[2]            # phxsql/
UI = RAIZ / "crates" / "phxzip-web" / "ui"
MENSAGENS = RAIZ / "crates" / "phxsql-server" / "src" / "mensagens.rs"

ASSINATURA = b"7z\xbc\xaf\x27\x1c"
MARCA_FALSO = b"\x00PHXZIP-FALSO\n"
MARCA_CENARIO = b"\x00CENARIO:"

# Os estaticos sao LISTA FECHADA (contrato secao 2): nao ha `../` para pedir.
ESTATICOS = {
    "/": ("index.html", "text/html; charset=utf-8"),
    "/phxzip.css": ("phxzip.css", "text/css; charset=utf-8"),
    "/phxzip.js": ("phxzip.js", "text/javascript; charset=utf-8"),
    "/fonte/exo2-latin.woff2": ("fonte/exo2-latin.woff2", "font/woff2"),
}

CSP = ("default-src 'none'; script-src 'self'; style-src 'self'; img-src 'self' data:; "
       "font-src 'self'; connect-src 'self'; base-uri 'none'; form-action 'none'; "
       "frame-ancestors 'none'")


def idiomas_do_motor():
    """A lista sai do `IDIOMAS` do motor, e nao de uma copia aqui: quando um
    gerador depende de uma lista, a lista sai do codigo."""
    fonte = MENSAGENS.read_text(encoding="utf-8")
    m = re.search(r"pub const IDIOMAS: \[&str; \d+\] = \[(.*?)\];", fonte, re.S)
    if not m:
        sys.exit("nao achei o IDIOMAS em mensagens.rs")
    return re.findall(r'"([^"]+)"', m.group(1))


IDIOMAS = idiomas_do_motor()


class Recusa(Exception):
    """Um erro nomeado do contrato (secao 6)."""

    def __init__(self, status, erro, **detalhe):
        super().__init__(erro)
        self.status, self.erro, self.detalhe = status, erro, detalhe


# ----------------------------------------------------------------- o motor falso
def conferir_nome(nome):
    """A regra do `phxzip::conferir_nome`, no minimo necessario para o falso:
    caminho absoluto, `..`, letra de unidade, barra invertida e NUL."""
    if (not nome or nome.startswith("/") or "\\" in nome or "\x00" in nome
            or re.match(r"^[A-Za-z]:", nome) or ".." in nome.split("/")):
        raise Recusa(422, "NOME_PERIGOSO", nome=nome)
    return nome


def empacotar(cabeca, carga):
    """O «pacote» falso: assinatura, marca, JSON com os itens, e o conteudo
    concatenado (comprimido com zlib quando o nivel nao e armazenar)."""
    senha = cabeca.get("senha") or ""
    armazenar = cabeca.get("nivel") == "armazenar"
    itens, p = [], 0
    for it in cabeca["itens"]:
        it = dict(it)
        if not it.get("pasta"):
            dado = carga[p:p + it["tamanho"]]
            p += it["tamanho"]
            it["comp"] = len(dado) if armazenar else len(zlib.compress(dado, 9))
        itens.append(it)
    guardado = carga if armazenar else zlib.compress(carga, 9)
    meta = {
        "itens": itens,
        "cifrado": bool(senha),
        "nomes_cifrados": bool(senha) and bool(cabeca.get("cifrar_nomes", True)),
        "senha_sha256": hashlib.sha256(senha.encode()).hexdigest() if senha else None,
        "zlib": cabeca.get("nivel") != "armazenar",
        "metodo": "Copy" if cabeca.get("nivel") == "armazenar" else "LZMA2",
    }
    m = json.dumps(meta).encode()
    return ASSINATURA + MARCA_FALSO + len(m).to_bytes(4, "little") + m + guardado


def amostra(nome_cenario):
    """Os pacotes de exemplo, um por cenario. Cada um e o que o servidor de
    verdade devolveria para um pacote com aquela forma."""
    agora = 1790000000
    if nome_cenario == "xss":
        # Nomes que atacam a TELA, nao o disco: marcacao, direcao invertida,
        # aspas, um nome imenso -- e o «Blumenau» da licao da caixa alta.
        itens = [
            ("<img src=x onerror=alert(1)>.txt", b"se isto rodou, a tela interpretou o nome\n"),
            ("fatura‮txt.exe", b"MZ nao sou um texto\n"),
            ('nome "com" aspas & <tags>.txt', b"aspas\n"),
            ("Blumenau.txt", b"Blumenau, SC\n"),
            ("pasta-com-um-nome-muito-comprido-que-nao-cabe-em-celular-nenhum/"
             "e-continua-descendo-por-mais-uma-pasta/arquivo-final-tambem-comprido.json", b'{"ok": true}\n'),
            ("الملف.txt", b"nome em arabe, legitimo\n"),
        ]
        return pacote_de_itens(itens, agora)
    if nome_cenario in ("config", "config-quebrado"):
        cfg = CONFIG_VALIDO if nome_cenario == "config" else CONFIG_QUEBRADO
        return pacote_de_itens([("config.json", cfg)], agora, senha="phx-config",
                               nomes_cifrados=True)
    if nome_cenario == "cifrado7zip":
        return pacote_de_itens([("segredo.txt", b"texto secreto\n")], agora, senha="certa",
                               nomes_cifrados=True, sem_crc_cifrado=True)
    # "misto": pastas, texto, JSON, binario, vazio -- um bloco solido e um so.
    png = bytes.fromhex("89504e470d0a1a0a0000000d49484452") + bytes(range(256)) * 3
    itens = [
        ("docs", None),
        ("docs/leia-me.txt", "Relatório de 2026 — ação, coração, Blumenau.\n".encode() * 40),
        ("docs/dados.json", json.dumps({"cidade": "Blumenau", "itens": list(range(40))}, indent=2).encode()),
        ("docs/vazio.txt", b""),
        ("imagens", None),
        ("imagens/marca.png", png),
        # Maior que o teto de espiar (256 KiB): o trecho mostrado e cortado.
        ("grande.log", b"".join(("linha de registro %05d: tudo certo por aqui\n" % i).encode() for i in range(9000))),
    ]
    return pacote_de_itens(itens, agora, blocos_solidos=True)


CONFIG_VALIDO = json.dumps({
    "_comentario": "configuracao de exemplo do PhxSql",
    "porta": 5000,
    "idioma": "Portugues",
    "recursos": {"cache_paginas": 4096, "max_linhas": 1000},
    "usuarios": ["adm", "leitura"],
}, indent=2, ensure_ascii=False).encode() + b"\n"
CONFIG_QUEBRADO = CONFIG_VALIDO.replace(b'"max_linhas": 1000', b'"max_linhas": 1000,,')


def pacote_de_itens(itens, agora, senha="", nomes_cifrados=False, blocos_solidos=False,
                    sem_crc_cifrado=False):
    lista, carga = [], b""
    for nome, conteudo in itens:
        if conteudo is None:
            lista.append({"nome": nome, "pasta": True, "modificado": agora})
        else:
            lista.append({"nome": nome, "pasta": False, "tamanho": len(conteudo), "modificado": agora,
                          "comp": len(zlib.compress(conteudo, 9))})
            carga += conteudo
    meta = {"itens": lista, "cifrado": bool(senha), "nomes_cifrados": nomes_cifrados,
            "senha_sha256": hashlib.sha256(senha.encode()).hexdigest() if senha else None,
            "zlib": True, "metodo": "LZMA2", "solido": blocos_solidos,
            "sem_crc_cifrado": sem_crc_cifrado}
    m = json.dumps(meta).encode()
    return ASSINATURA + MARCA_FALSO + len(m).to_bytes(4, "little") + m + zlib.compress(carga, 9)


def abrir(pacote, senha):
    """Devolve (meta, [(entrada, conteudo)]) ou recusa com o nome do contrato."""
    if not pacote.startswith(ASSINATURA):
        raise Recusa(422, "NAO_E_7Z")
    resto = pacote[len(ASSINATURA):]
    if resto.startswith(MARCA_CENARIO):
        cen = resto[len(MARCA_CENARIO):].split(b"\n", 1)[0].decode()
        return abrir(cenario(cen), senha)
    if not resto.startswith(MARCA_FALSO):
        # Um 7z de verdade: o falso nao sabe le-lo, e responde com a amostra.
        return abrir(amostra("misto"), senha)
    resto = resto[len(MARCA_FALSO):]
    n = int.from_bytes(resto[:4], "little")
    meta = json.loads(resto[4:4 + n])
    # O tamanho do pacote que se LISTA: num cenario, o da amostra gerada, e
    # nao o do arquivo-marcador de 21 bytes que o disparou.
    meta["_tamanho"] = len(pacote)
    guardado = resto[4 + n:]
    senha_ok = (not meta["cifrado"]) or (
        senha and hashlib.sha256(senha.encode()).hexdigest() == meta["senha_sha256"])

    def conteudo():
        if meta["cifrado"] and not senha:
            raise Recusa(422, "SENHA_AUSENTE")
        if not senha_ok:
            raise Recusa(422, "SENHA_ERRADA_OU_CORROMPIDO" if meta.get("sem_crc_cifrado") else "SENHA_ERRADA")
        try:
            carga = zlib.decompress(guardado) if meta.get("zlib") else guardado
        except zlib.error:
            raise Recusa(422, "CORROMPIDO", onde="CRC do bloco 0 nao confere")
        saida, p = [], 0
        for it in meta["itens"]:
            if it.get("pasta"):
                saida.append((it, b""))
            else:
                saida.append((it, carga[p:p + it["tamanho"]]))
                p += it["tamanho"]
        return saida

    if meta.get("nomes_cifrados"):
        if not senha:
            raise Recusa(422, "SENHA_AUSENTE")
        if not senha_ok:
            raise Recusa(422, "SENHA_ERRADA_OU_CORROMPIDO" if meta.get("sem_crc_cifrado") else "SENHA_ERRADA")
    for it in meta["itens"]:
        conferir_nome(it["nome"])
    return meta, conteudo


def cenario(nome):
    if nome == "corrompido":
        raise Recusa(422, "CORROMPIDO", onde="CRC do cabecalho nao confere (esperado 9ae0daaf, lido 1c291ca3)")
    if nome == "legado":
        raise Recusa(422, "METODO_LEGADO", metodo="BZip2")
    if nome == "zipslip":
        raise Recusa(422, "NOME_PERIGOSO", nome="../../etc/cron.d/phx")
    if nome == "grande":
        raise Recusa(413, "GRANDE_DEMAIS", oque="bloco", declarado=8 << 30, teto=256 << 20)
    if nome == "estrutura":
        raise Recusa(422, "ESTRUTURA", onde="tamanho empacotado aponta para fora do arquivo")
    if nome == "desconhecido":
        raise Recusa(422, "ERRO_QUE_A_TELA_NAO_CONHECE")
    return amostra(nome)


def listar(meta, entradas_fn, tamanho_pacote):
    itens = meta["itens"]
    entradas, blocos = [], []
    arquivos = [i for i, it in enumerate(itens) if not it.get("pasta")]
    nao_vazios = [i for i in arquivos if itens[i]["tamanho"] > 0]
    comp = {i: itens[i].get("comp", itens[i]["tamanho"]) for i in nao_vazios}
    metodos = [meta.get("metodo", "LZMA2")] + (["7zAES"] if meta["cifrado"] else [])
    if meta.get("solido") and len(nao_vazios) > 1:
        # Um bloco solido com todos menos o ultimo, e um bloco so para o ultimo:
        # os dois desenhos da coluna «compactado» aparecem na mesma lista.
        ult = nao_vazios[-1]
        blocos = [
            {"indice": 0, "compactado": sum(comp[i] for i in nao_vazios[:-1]),
             "tamanho": sum(itens[i]["tamanho"] for i in nao_vazios[:-1]),
             "entradas": len(nao_vazios) - 1, "metodos": metodos},
            {"indice": 1, "compactado": comp[ult], "tamanho": itens[ult]["tamanho"],
             "entradas": 1, "metodos": metodos},
        ]
        bloco_de = {i: 0 for i in nao_vazios[:-1]}
        bloco_de[ult] = 1
    else:
        blocos, bloco_de = [], {}
        for k, i in enumerate(nao_vazios):
            blocos.append({"indice": k, "compactado": comp[i],
                           "tamanho": itens[i]["tamanho"], "entradas": 1, "metodos": metodos})
            bloco_de[i] = k
    for i, it in enumerate(itens):
        entradas.append({
            "indice": i, "nome": it["nome"], "pasta": bool(it.get("pasta")),
            "tamanho": 0 if it.get("pasta") else it["tamanho"],
            "modificado": it.get("modificado"),
            "cifrada": bool(meta["cifrado"]) and not it.get("pasta"),
            "crc": None if it.get("pasta") else "%08x" % (zlib.crc32(it["nome"].encode()) & 0xffffffff),
            "bloco": bloco_de.get(i),
        })
    return {"ok": True, "tamanho_do_pacote": tamanho_pacote,
            "cabecalho_cifrado": bool(meta.get("nomes_cifrados")),
            "entradas": entradas, "blocos": blocos}


# ----------------------------------------------------------------- o HTTP
class Manipulador(BaseHTTPRequestHandler):
    server_version = "PhxZipFalso"
    sys_version = ""
    protocol_version = "HTTP/1.1"

    # ---------------------------------------------------------- o que se registra
    def log_message(self, fmt, *args):
        pass

    def registrar(self, status):
        """Registra a linha do pedido e TODOS os cabecalhos -- de proposito: o
        roteiro procura a senha neste arquivo, e ela nao pode estar em lugar
        nenhum que nao seja o corpo. O corpo NAO se registra (contrato secao 3)."""
        if not self.server.log:
            return
        with self.server.trava_log:
            with open(self.server.log, "a", encoding="utf-8") as f:
                f.write(f"{status} {self.requestline}\n")
                for k, v in self.headers.items():
                    f.write(f"    {k}: {v}\n")

    # ---------------------------------------------------------- respostas
    def cabecalhos_comuns(self):
        self.send_header("Content-Security-Policy", CSP)
        self.send_header("X-Content-Type-Options", "nosniff")
        self.send_header("Referrer-Policy", "no-referrer")
        self.send_header("Cache-Control", "no-store")

    def responder(self, status, corpo, tipo, extra=None):
        self.send_response(status)
        self.cabecalhos_comuns()
        self.send_header("Content-Type", tipo)
        self.send_header("Content-Length", str(len(corpo)))
        for k, v in (extra or {}).items():
            self.send_header(k, v)
        self.end_headers()
        self.wfile.write(corpo)
        self.registrar(status)

    def json(self, status, obj, extra=None):
        self.responder(status, json.dumps(obj, ensure_ascii=False).encode(), "application/json; charset=utf-8", extra)

    def recusar(self, r):
        self.json(r.status, {"ok": False, "erro": r.erro, **({"detalhe": r.detalhe} if r.detalhe else {})})

    def download(self, corpo, nome, tipo="application/octet-stream", extra=None):
        ascii_ = re.sub(r'[^A-Za-z0-9._-]', "_", nome) or "arquivo"
        from urllib.parse import quote
        cab = {"Content-Disposition": f"attachment; filename=\"{ascii_}\"; filename*=UTF-8''{quote(nome, safe='')}"}
        cab.update(extra or {})
        self.responder(200, corpo, tipo, cab)

    # ---------------------------------------------------------- as guardas
    def conferir_host_e_origem(self, post):
        porta = self.server.server_address[1]
        validos = {f"127.0.0.1:{porta}", f"localhost:{porta}"}
        if self.headers.get("Host") not in validos:
            raise Recusa(403, "HOST_RECUSADO")
        if post:
            origem = self.headers.get("Origin")
            if origem is not None and origem not in {f"http://{v}" for v in validos}:
                raise Recusa(403, "ORIGEM_RECUSADA")
            sfs = self.headers.get("Sec-Fetch-Site")
            if sfs is not None and sfs != "same-origin":
                raise Recusa(403, "ORIGEM_RECUSADA")

    def ler_envelope(self):
        tipo = (self.headers.get("Content-Type") or "").split(";")[0].strip()
        if tipo != "application/octet-stream":
            raise Recusa(415, "TIPO_DE_CONTEUDO")
        if self.headers.get("Content-Length") is None:
            raise Recusa(411, "TAMANHO_AUSENTE")
        n = int(self.headers["Content-Length"])
        lim = self.server.limites
        if n > lim["envio"]:
            # O teto se confere ANTES de ler. E depois de responder, o de
            # verdade drena o corpo ate um teto de descarte (contrato secao 4);
            # o falso tem a chave --nao-drenar para medir o que o navegador ve
            # sem isso.
            self.drenar_depois = n if not self.server.nao_drenar else 0
            raise Recusa(413, "GRANDE_DEMAIS", oque="envio", declarado=n, teto=lim["envio"])
        corpo = self.ler(n)
        if len(corpo) < 8 or corpo[:4] != b"PZW1":
            raise Recusa(400, "PEDIDO_MALFORMADO", campo="assinatura")
        nc = int.from_bytes(corpo[4:8], "little")
        if nc > lim["cabeca"]:
            raise Recusa(413, "GRANDE_DEMAIS", oque="cabeca", declarado=nc, teto=lim["cabeca"])
        try:
            cabeca = json.loads(corpo[8:8 + nc].decode("utf-8"))
        except (UnicodeDecodeError, json.JSONDecodeError):
            raise Recusa(400, "PEDIDO_MALFORMADO", campo="cabeca")
        return cabeca, corpo[8 + nc:]

    def ler(self, n):
        """Le o corpo. Acima de 4 MiB, le devagar -- a unica forma de a barra
        de progresso do ENVIO aparecer num teste local, onde o soquete engole
        megabytes num piscar."""
        partes, falta = [], n
        devagar = n > 4 << 20
        while falta:
            bloco = self.rfile.read(min(falta, 256 << 10))
            if not bloco:
                break
            partes.append(bloco)
            falta -= len(bloco)
            if devagar:
                time.sleep(0.04)
        return b"".join(partes)

    # ---------------------------------------------------------- GET
    def do_GET(self):
        try:
            self.conferir_host_e_origem(False)
            url = urlsplit(self.path)
            if url.path in ESTATICOS:
                arq, tipo = ESTATICOS[url.path]
                self.responder(200, (UI / arq).read_bytes(), tipo)
            elif url.path == "/api/estado":
                self.json(200, {"ok": True, "produto": "PhxZip", "versao": "0.0.0-falso",
                                "limites": self.server.limites, "niveis": ["armazenar", "lzma2"],
                                "formatos": ["7z", "phz"], "idiomas": IDIOMAS})
            elif url.path == "/api/idiomas":
                self.json(200, textos_resolvidos(parse_qs(url.query).get("idioma", [""])[0]))
            elif url.path.startswith("/api/"):
                raise Recusa(405 if url.path in ROTAS_POST else 404,
                             "METODO_HTTP" if url.path in ROTAS_POST else "ROTA_INEXISTENTE")
            else:
                raise Recusa(404, "ROTA_INEXISTENTE")
        except Recusa as r:
            self.recusar(r)

    # ---------------------------------------------------------- POST
    def do_POST(self):
        self.drenar_depois = 0
        try:
            self.conferir_host_e_origem(True)
            rota = urlsplit(self.path).path
            if rota not in ROTAS_POST:
                raise Recusa(404, "ROTA_INEXISTENTE")
            if not self.server.vagas.acquire(blocking=False):
                raise Recusa(503, "OCUPADO")
            try:
                cabeca, carga = self.ler_envelope()
                ROTAS_POST[rota](self, cabeca, carga)
            finally:
                self.server.vagas.release()
        except Recusa as r:
            self.close_connection = True
            self.recusar(r)
            if self.drenar_depois:
                self.drenar(self.drenar_depois)

    def drenar(self, n):
        falta = n
        try:
            while falta:
                b = self.rfile.read(min(falta, 1 << 20))
                if not b:
                    break
                falta -= len(b)
        except OSError:
            pass

    def rota_compactar(self, cabeca, carga):
        for campo in ("formato", "nivel", "itens"):
            if campo not in cabeca:
                raise Recusa(400, "PEDIDO_MALFORMADO", campo=campo)
        if cabeca["formato"] not in ("7z", "phz"):
            raise Recusa(400, "PEDIDO_MALFORMADO", campo="formato")
        if cabeca["nivel"] not in ("armazenar", "lzma2"):
            raise Recusa(400, "PEDIDO_MALFORMADO", campo="nivel")
        vistos = set()
        for it in cabeca["itens"]:
            conferir_nome(it["nome"])
            if it["nome"] in vistos:
                raise Recusa(422, "NOME_REPETIDO", nome=it["nome"])
            vistos.add(it["nome"])
        soma = sum(it.get("tamanho", 0) for it in cabeca["itens"] if not it.get("pasta"))
        if soma != len(carga):
            raise Recusa(400, "PEDIDO_MALFORMADO", campo="itens.tamanho")
        if cabeca["formato"] == "phz":
            # O que o `phxzip::empacotar` recusaria: o .phz e UMA entrada, com senha.
            if not cabeca.get("senha"):
                raise Recusa(422, "SENHA_AUSENTE")
            if not cabeca["itens"]:
                raise Recusa(422, "SEM_ENTRADA")
            if len(cabeca["itens"]) > 1:
                raise Recusa(422, "MAIS_DE_UMA_ENTRADA", quantas=len(cabeca["itens"]))
            if cabeca["itens"][0].get("pasta"):
                raise Recusa(422, "ENTRADA_E_PASTA", nome=cabeca["itens"][0]["nome"])
            cabeca = dict(cabeca, nivel="lzma2", cifrar_nomes=True)
        if len(carga) > 4 << 20:
            time.sleep(1.2)  # o «trabalho» do servidor, para a fase aparecer
        pacote = empacotar(cabeca, carga)
        nome = cabeca.get("nome") or "pacote." + cabeca["formato"]
        self.download(pacote, nome, "application/x-7z-compressed", {"X-PhxZip-Tamanho-Original": str(soma)})

    def rota_listar(self, cabeca, carga):
        meta, _ = abrir(carga, cabeca.get("senha"))
        self.json(200, listar(meta, None, meta.get("_tamanho", len(carga))))

    def rota_testar(self, cabeca, carga):
        meta, conteudo = abrir(carga, cabeca.get("senha"))
        tudo = conteudo()
        self.json(200, {"ok": True, "entradas": sum(1 for e, _ in tudo if not e.get("pasta")),
                        "pastas": sum(1 for e, _ in tudo if e.get("pasta")),
                        "bytes": sum(len(c) for _, c in tudo),
                        "blocos": 1})

    def rota_extrair(self, cabeca, carga):
        meta, conteudo = abrir(carga, cabeca.get("senha"))
        tudo = conteudo()
        if cabeca.get("todas"):
            indices = list(range(len(tudo)))
        else:
            indices = cabeca.get("indices") or []
            for i in indices:
                if not isinstance(i, int) or i < 0 or i >= len(tudo):
                    raise Recusa(422, "ENTRADA_INEXISTENTE", indice=i)
        if len(indices) == 1 and not cabeca.get("todas"):
            ent, dado = tudo[indices[0]]
            extra = {"X-PhxZip-Tamanho": str(len(dado))}
            ate = cabeca.get("ate")
            if isinstance(ate, int):
                ate = min(ate, self.server.limites["espiar"])
                if len(dado) > ate:
                    extra["X-PhxZip-Cortado"] = "1"
                parte = dado[:ate]
            else:
                parte = dado
            if cabeca.get("avaliar_json"):
                extra.update(avaliar_json(dado, self.server.limites["avaliar_json"]))
            nome = ent["nome"].rsplit("/", 1)[-1]
            self.download(parte, nome, extra=extra)
            return
        buf = io.BytesIO()
        with tarfile.open(fileobj=buf, mode="w", format=tarfile.PAX_FORMAT) as tar:
            for i in indices:
                ent, dado = tudo[i]
                ti = tarfile.TarInfo(ent["nome"])
                ti.mtime = ent.get("modificado") or 0
                if ent.get("pasta"):
                    ti.type = tarfile.DIRTYPE
                    ti.mode = 0o755
                    tar.addfile(ti)
                else:
                    ti.size = len(dado)
                    ti.mode = 0o644
                    tar.addfile(ti, io.BytesIO(dado))
        self.download(buf.getvalue(), "pacote.tar", "application/x-tar")


def avaliar_json(dado, teto):
    """O veredito do contrato secao 3.4. No servidor de verdade quem julga e o
    `Json::analisar` da casa; aqui, o `json` do Python -- e a posicao vai em
    BYTES, como a do motor."""
    if len(dado) > teto:
        return {"X-PhxZip-Json": "nao_avaliado"}
    try:
        texto = dado.decode("utf-8")
    except UnicodeDecodeError:
        return {"X-PhxZip-Json": "nao_e_texto"}
    try:
        json.loads(texto)
        return {"X-PhxZip-Json": "valido"}
    except json.JSONDecodeError as e:
        return {"X-PhxZip-Json": "invalido", "X-PhxZip-Json-Posicao": str(len(texto[:e.pos].encode()))}


ROTAS_POST = {
    "/api/compactar": Manipulador.rota_compactar,
    "/api/listar": Manipulador.rota_listar,
    "/api/testar": Manipulador.rota_testar,
    "/api/extrair": Manipulador.rota_extrair,
}


def textos_resolvidos(idioma):
    """Os degraus do `idiomas.rs`: idioma desconhecido cai no Portugues, e
    celula vazia cai na coluna Portugues. Relido a cada pedido: editar o
    `textos.json` vale no proximo recarregar."""
    col = idioma if idioma in IDIOMAS else "Portugues"
    doc = json.loads((UI / "textos.json").read_text(encoding="utf-8"))
    textos = {}
    for chave, cel in doc["textos"].items():
        desconhecidas = set(cel) - set(IDIOMAS)
        if desconhecidas:
            raise SystemExit(f"{chave}: coluna que o motor nao tem: {sorted(desconhecidas)}")
        textos[chave] = cel.get(col) or cel["Portugues"]
    return {"ok": True, "idioma": col, "idiomas": IDIOMAS, "textos": textos}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--porta", type=int, default=7799)
    ap.add_argument("--log", default=None, help="registra linha de pedido e cabecalhos (nunca o corpo)")
    ap.add_argument("--teto-envio", type=int, default=256 << 20)
    ap.add_argument("--ui", default=None,
                    help="serve a tela de outro diretorio (a prova das guardas serve uma copia com o defeito reposto)")
    ap.add_argument("--nao-drenar", action="store_true",
                    help="responde 413 e fecha sem drenar o corpo (para medir o que o navegador ve)")
    a = ap.parse_args()
    global UI
    if a.ui:
        UI = Path(a.ui).resolve()
    srv = ThreadingHTTPServer(("127.0.0.1", a.porta), Manipulador)
    srv.daemon_threads = True
    srv.log = a.log
    srv.trava_log = threading.Lock()
    srv.nao_drenar = a.nao_drenar
    srv.limites = {"envio": a.teto_envio, "cabeca": 1 << 20, "entrada": 256 << 20,
                   "bloco": 256 << 20, "cabecalho": 16 << 20, "espiar": 256 << 10,
                   "avaliar_json": 16 << 20, "simultaneas": 2}
    srv.vagas = threading.Semaphore(srv.limites["simultaneas"])
    print(f"PhxZip falso em http://127.0.0.1:{a.porta}/", flush=True)
    try:
        srv.serve_forever()
    except KeyboardInterrupt:
        pass


if __name__ == "__main__":
    main()
