#!/usr/bin/env python3
"""E2E do PhxClaw Desktop: o binario Tauri de verdade, dirigido por WebDriver.

Caminho oficial do Tauri no Linux: tauri-driver na frente do WebKitWebDriver, com a
janela num Xvfb. O cliente WebDriver aqui e urllib puro -- o protocolo e HTTP+JSON e
nao precisa de selenium para seis chamadas.

O que prova, e o que NAO prova:
- prova que o binario sobe, a WebView carrega a UI empacotada, a ponte IPC responde
  (host_status, verify_evidence, events_snapshot) e a UI mostra o que a ponte devolveu;
- prova que a politica padrao NEGA shell e input quando chamados pela propria WebView;
- NAO prova automacao de SO em desktop fisico (teclado/mouse/captura reais): isso e o
  gate desktop_os_automation_e2e do native_gate_catalog, e continua pendente.

Uso: python3 tests/desktop/desktop_e2e.py [caminho/do/binario]
Sai 0 so se todas as checagens passarem; grava a captura em tests/desktop/out/.
"""
import base64, json, os, re, shutil, signal, subprocess, sys, time, urllib.error, urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
BIN = Path(sys.argv[1]) if len(sys.argv) > 1 else ROOT / "target/debug/phxclaw-desktop"
OUT = ROOT / "tests/desktop/out"
PORT = 4444
UUID7 = re.compile(r"^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$")


def wd(method, path, body=None, timeout=60):
    req = urllib.request.Request(
        f"http://127.0.0.1:{PORT}{path}", method=method,
        data=None if body is None else json.dumps(body).encode(),
        headers={"Content-Type": "application/json"})
    try:
        with urllib.request.urlopen(req, timeout=timeout) as r:
            return json.loads(r.read() or b"{}").get("value")
    except urllib.error.HTTPError as e:  # o corpo diz o motivo; sem ele o 400 e mudo
        raise RuntimeError(f"{method} {path} -> {e.code}: {e.read()[:300]!r}") from None


def main() -> int:
    if not BIN.exists():
        print(f"binario ausente: {BIN} (cargo build -p phxclaw-desktop)")
        return 2
    OUT.mkdir(parents=True, exist_ok=True)
    env = dict(os.environ)
    xvfb = None
    if "DISPLAY" not in env:
        xvfb = subprocess.Popen(["Xvfb", ":97", "-screen", "0", "1600x1000x24"],
                                stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        env["DISPLAY"] = ":97"
        # Xvfb nao tem DRI3; o renderizador dmabuf do WebKitGTK derrubava o processo web
        # no meio da sessao (5/9 verdes medidos). Desligado: 18/18. So vale neste Xvfb.
        env.setdefault("WEBKIT_DISABLE_DMABUF_RENDERER", "1")
        time.sleep(1)
    driver = subprocess.Popen([shutil.which("tauri-driver") or "tauri-driver", "--port", str(PORT)],
                              env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                              start_new_session=True)
    checagens, sessao = [], None

    def check(nome, ok, detalhe=""):
        checagens.append(ok)
        print(("ok   " if ok else "FALHA ") + nome + (f" :: {detalhe}" if detalhe else ""))

    try:
        for _ in range(50):
            try:
                wd("GET", "/status", timeout=2); break
            except Exception:
                time.sleep(0.2)
        sessao = wd("POST", "/session", {"capabilities": {"alwaysMatch": {
            "tauri:options": {"application": str(BIN)}}}}, timeout=120)["sessionId"]
        s = f"/session/{sessao}"

        def js(code, *args):
            return wd("POST", f"{s}/execute/sync", {"script": code, "args": list(args)})

        def async_js(code):
            return wd("POST", f"{s}/execute/async", {"script": code, "args": []}, timeout=60)

        def texto(elem_id, espera=15.0, diferente_de=None):
            fim = time.time() + espera
            v = None
            while time.time() < fim:
                v = js("const e=document.getElementById(arguments[0]);return e?e.textContent.trim():null;", elem_id)
                if v and v != diferente_de and v != "—":
                    return v
                time.sleep(0.25)
            return v

        check("janela carregou a UI empacotada", js("return document.title") is not None,
              js("return document.title"))
        # A splash termina (19 passos x 190 ms) e revela o Command Center sozinha. O botao
        # ENTRAR so fica clicavel 420 ms antes disso -- achado de UX, registrado, nao testado.
        visivel = False
        for _ in range(60):
            visivel = js("return document.getElementById('app').getAttribute('aria-hidden')==='false'"
                         " && document.getElementById('app').classList.contains('visible');")
            if visivel: break
            time.sleep(0.25)
        check("splash conclui e revela o Command Center", bool(visivel))
        # Pausa opcional para gravacao de demonstracao (padrao 0: o teste nao muda).
        pausa = float(os.environ.get("PHXCLAW_E2E_PAUSA", "0"))
        time.sleep(pausa)
        ponte = texto("nativeBridgeStatus", diferente_de="WEB PREVIEW")
        check("ponte IPC nativa (nao preview web)", ponte == "TAURI CONNECTED", ponte)
        sess = texto("hostSession")
        check("host_status devolveu sessao exibida", bool(sess) and sess != "—", sess)
        evid = texto("evidenceState", diferente_de="WAIT")
        check("cadeia de evidencia verificada pela ponte", bool(evid) and evid.startswith("VALID"), evid)
        pol = texto("hostPolicy")
        check("politica padrao exibida como DENY", pol == "DENY", pol)

        status = async_js("const d=arguments[arguments.length-1];"
                          "window.__TAURI__.core.invoke('host_status').then(d,e=>d({erro:String(e)}));")
        kver = texto("kernelVersion")
        check("versao na tela = versao do host", kver == f"v{status.get('version')}", f"{kver} / {status.get('version')}")
        check("host_status: session_uuid e UUIDv7", bool(UUID7.match(str(status.get("session_uuid", "")))),
              str(status.get("session_uuid")))
        # A requisicao tem de ser VALIDA: negacao por argumento malformado passaria por engano.
        pedido = {"uuid": status["session_uuid"], "shell": "direct", "program_or_script": "/bin/echo",
                  "args": ["pwned"], "cwd": None, "env": {}, "stdin": None, "timeout_ms": 2000}
        shell = async_js("const d=arguments[arguments.length-1];"
                         f"window.__TAURI__.core.invoke('execute_shell',{{request:{json.dumps(pedido)}}})"
                         ".then(r=>d({ok:r}),e=>d({erro:String(e)}));")
        # A negacao volta como resultado (status denied + evidencia), nao como excecao.
        res = shell.get("ok") or {}
        check("execute_shell NEGADO pela politica, com evidencia",
              res.get("status") == "denied" and "disabled by policy" in json.dumps(res)
              and bool(UUID7.match(str(res.get("evidence_uuid", "")))), json.dumps(shell)[:160])
        check("nada executou (sem saida 'pwned')", "pwned\n" not in json.dumps(res), "")
        verify = async_js("const d=arguments[arguments.length-1];"
                          "window.__TAURI__.core.invoke('verify_evidence').then(d,e=>d({erro:String(e)}));")
        check("verify_evidence continua valido depois da negacao",
              isinstance(verify, dict) and verify.get("valid") is True, json.dumps(verify)[:160])

        time.sleep(1.0)  # transicao CSS da splash, antes da captura
        time.sleep(pausa)
        png = wd("GET", f"{s}/screenshot")
        (OUT / "desktop_e2e.png").write_bytes(base64.b64decode(png))
        check("captura da janela gravada", (OUT / "desktop_e2e.png").stat().st_size > 10_000,
              str(OUT / "desktop_e2e.png"))
    except Exception as e:  # falha de infraestrutura tambem e falha, e aparece
        check("sessao WebDriver", False, repr(e)[:300])
    finally:
        if sessao:
            try: wd("DELETE", f"/session/{sessao}")
            except Exception: pass
        os.killpg(driver.pid, signal.SIGTERM)
        if xvfb: xvfb.terminate()
    print(f"placar: {sum(checagens)}/{len(checagens)}")
    return 0 if checagens and all(checagens) else 1


if __name__ == "__main__":
    raise SystemExit(main())
