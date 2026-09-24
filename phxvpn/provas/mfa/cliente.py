#!/usr/bin/env python3
"""Um membro tentando conectar, pelo openvpn DE VERDADE, com usuario, senha e
codigo -- do jeito que o OpenVPN GUI e o Connect fazem: pela interface de
gerencia, montando o SCRV1 do static-challenge.

O codigo TOTP e calculado AQUI, com o hmac/hashlib do Python: uma
implementacao independente da do phxvpn. Se as duas divergissem, nenhum
codigo certo passaria.

Uso (dentro do netns do cliente):
  cliente.py PERFIL LOG USUARIO SENHA CODIGO [IP_PARA_PINGAR]
  cliente.py --codigo SEGREDO_BASE32 DESLOCAMENTO_S   (so imprime o codigo)
Saida: uma linha JSON {"resultado": "conectou"|"recusado"|"prazo", ...}.
"""
import base64, hashlib, hmac, json, socket, struct, subprocess, sys, time


def totp(segredo_b32, instante):
    s = segredo_b32.upper() + "=" * (-len(segredo_b32) % 8)
    chave = base64.b32decode(s)
    h = hmac.new(chave, struct.pack(">Q", int(instante) // 30), hashlib.sha1).digest()
    o = h[19] & 15
    n = struct.unpack(">I", h[o:o + 4])[0] & 0x7FFFFFFF
    return "%06d" % (n % 1000000)


def main():
    if sys.argv[1] == "--codigo":
        print(totp(sys.argv[2], time.time() + int(sys.argv[3])))
        return
    perfil, log, usuario, senha, codigo = sys.argv[1:6]
    pingar = sys.argv[6] if len(sys.argv) > 6 else None
    porta = 7505
    ovpn = subprocess.Popen(
        ["openvpn", "--config", perfil, "--log", log, "--auth-retry", "none",
         "--management", "127.0.0.1", str(porta), "--management-query-passwords",
         "--management-hold"],
        stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    saida = {"resultado": "prazo", "desafio": None}
    inicio = time.time()
    try:
        g = None
        for _ in range(50):
            try:
                g = socket.create_connection(("127.0.0.1", porta), timeout=1)
                break
            except OSError:
                time.sleep(0.1)
        if g is None:
            saida["resultado"] = "sem-gerencia"
            return
        g.settimeout(0.5)
        g.sendall(b"state on\nhold release\n")
        buf = b""
        scrv1 = "SCRV1:%s:%s" % (base64.b64encode(senha.encode()).decode(),
                                 base64.b64encode(codigo.encode()).decode())
        while time.time() - inicio < 40:
            if ovpn.poll() is not None:
                saida["resultado"] = "recusado"
                break
            try:
                pedaco = g.recv(4096)
                if not pedaco:
                    saida["resultado"] = "recusado"
                    break
                buf += pedaco
            except socket.timeout:
                continue
            while b"\n" in buf:
                linha, buf = buf.split(b"\n", 1)
                l = linha.decode("utf-8", "replace").strip()
                if l.startswith(">PASSWORD:Need 'Auth'"):
                    # O perfil trouxe o static-challenge: o texto vem aqui.
                    if "SC:" in l:
                        saida["desafio"] = l.split("SC:", 1)[1]
                    g.sendall(('username "Auth" %s\npassword "Auth" "%s"\n'
                               % (usuario, scrv1)).encode())
                elif l.startswith(">PASSWORD:Verification Failed"):
                    saida["resultado"] = "recusado"
                elif ",CONNECTED,SUCCESS" in l:
                    saida["resultado"] = "conectou"
                    saida["segundos"] = round(time.time() - inicio, 2)
            if saida["resultado"] != "prazo":
                break
        if saida["resultado"] == "conectou" and pingar:
            p = subprocess.run(["ping", "-c", "3", "-W", "2", pingar],
                               capture_output=True, text=True)
            saida["ping"] = [l for l in p.stdout.splitlines() if "received" in l][:1]
    finally:
        ovpn.terminate()
        try:
            ovpn.wait(5)
        except subprocess.TimeoutExpired:
            ovpn.kill()
        print(json.dumps(saida, ensure_ascii=False))


main()
