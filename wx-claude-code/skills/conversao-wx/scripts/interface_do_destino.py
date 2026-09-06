#!/usr/bin/env python3
"""Qual a interface do programa Rust que sai da conversao -- e onde ela roda.

O questionario diz a LINGUAGEM de destino. Nao diz a FORMA: o mesmo nucleo em
Rust vira terminal, servico de rede, aplicacao de tela, pagina web, aplicativo
de celular ou firmware de placa -- e cada uma dessas exige uma ferramenta
diferente e roda num lugar diferente. Este script pergunta isso e responde com
o que a maquina AQUI consegue compilar, nao com o que se lembra.

Como o suporte e medido (nada vem de memoria):

  rustc --print target-list   todos os alvos que o compilador conhece
  rustup target list          os que tem `std` pre-compilada para baixar

Um alvo que aparece nos dois e tier 1/2: `rustup target add` e pronto. Um alvo
que so aparece no primeiro e tier 3: existe, compila, mas ninguem distribui a
`std` dele -- precisa de nightly e `-Z build-std`, e nao ha garantia de CI da
equipe do Rust. A diferenca muda o cronograma, entao ela e dita.

Sem `rustc` instalado nada disso se mede, e o suporte sai INDISPONIVEL com o
motivo -- em vez de afirmar por lembranca.

Uso:
  interface_do_destino.py listar
  interface_do_destino.py manual [opcao]
  interface_do_destino.py escolher --opcao terminal
"""
from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

# Cada opcao: os alvos que a forma exige, o que sai, onde roda, o que instalar.
# Os alvos sao consultados no rustc local; nada aqui afirma suporte por conta.
CATALOGO = [
    {
        "id": "terminal",
        "nome": "Terminal, modo texto (MS-DOS/console)",
        "alvos": ["x86_64-unknown-linux-gnu", "x86_64-pc-windows-msvc",
                  "x86_64-pc-windows-gnu", "aarch64-apple-darwin"],
        "gera": "um executavel unico, sem instalador e sem runtime",
        "roda": "prompt do Windows, terminal do Linux ou do macOS; tambem em .bat e agendador",
        "ferramenta": "so o cargo; para Windows a partir do Linux, o alvo -gnu mais o mingw-w64",
        "nucleo": "std pura",
    },
    {
        "id": "servico-tcp",
        "nome": "Servico em segundo plano, porta TCP",
        "alvos": ["x86_64-unknown-linux-gnu", "x86_64-pc-windows-msvc", "aarch64-unknown-linux-gnu"],
        "gera": "o mesmo executavel, escutando numa porta; sem tela",
        "roda": "servidor Linux (systemd), servico do Windows, contêiner",
        "ferramenta": "cargo; `std::net::TcpListener` da a escuta sem crate nenhuma",
        "nucleo": "std pura",
    },
    {
        "id": "desktop",
        "nome": "Aplicacao desktop, com janela",
        "alvos": ["x86_64-pc-windows-msvc", "x86_64-unknown-linux-gnu", "aarch64-apple-darwin"],
        "gera": "programa de janela; o desenho em si NAO vem da std",
        "roda": "Windows, Linux e macOS de mesa",
        "ferramenta": "o alvo e tier 1, mas a janela exige biblioteca de terceiros"
                      " (ou WebView do sistema); e a unica forma que fura o zero-dependencia",
        "nucleo": "std + biblioteca grafica",
    },
    {
        "id": "web",
        "nome": "Aplicacao web",
        "alvos": ["x86_64-unknown-linux-gnu", "wasm32-unknown-unknown"],
        "gera": "servidor HTTP proprio, ou o nucleo compilado para WebAssembly no navegador",
        "roda": "navegador (WASM) falando com o servidor, ou so o servidor servindo HTML",
        "ferramenta": "cargo; para o WASM, `rustup target add wasm32-unknown-unknown`",
        "nucleo": "std pura no servidor; no WASM a std existe, sem thread nem arquivo",
    },
    {
        "id": "mobile",
        "nome": "Aplicativo de celular (Android e iOS)",
        "alvos": ["aarch64-linux-android", "armv7-linux-androideabi", "aarch64-apple-ios"],
        "gera": "biblioteca nativa (.so/.a) chamada pela casca Kotlin ou Swift",
        "roda": "celular Android ou iPhone",
        "ferramenta": "Android NDK; para iOS, Xcode num Mac -- nao se compila iOS fora do macOS",
        "nucleo": "std pura na biblioteca; a tela e da plataforma",
    },
    {
        "id": "iot-esp32",
        "nome": "IoT, ESP32",
        "alvos": ["xtensa-esp32-none-elf", "xtensa-esp32-espidf", "riscv32imc-esp-espidf",
                  "riscv32imc-unknown-none-elf"],
        "gera": "firmware gravado na placa",
        "roda": "a propria placa ESP32, sem sistema operacional (ou com o ESP-IDF)",
        "ferramenta": "espup e espflash; o Xtensa exige um fork do compilador da Espressif",
        "nucleo": "no_std nos alvos -none-; std parcial nos -espidf",
    },
    {
        "id": "iot-arduino",
        "nome": "IoT, Arduino (AVR)",
        "alvos": ["avr-none", "avr-unknown-gnu-atmega328"],
        "gera": "firmware .hex para o microcontrolador",
        "roda": "Arduino Uno/Nano e parentes (ATmega)",
        "ferramenta": "nightly, avr-gcc e avrdude; `-Z build-std=core`",
        "nucleo": "no_std -- sem alocador, sem String, sem Vec por padrao",
    },
    {
        "id": "smart-tv",
        "nome": "Smart TV",
        "alvos": ["aarch64-linux-android", "aarch64-apple-tvos", "aarch64-unknown-linux-gnu"],
        "gera": "biblioteca nativa embutida no aplicativo da TV",
        "roda": "Android TV / Fire TV (Android), Apple TV (tvOS), TVs Linux",
        "ferramenta": "as mesmas do mobile; Tizen e webOS nao tem alvo proprio -- ali o"
                      " caminho e WASM ou servidor",
        "nucleo": "std nos alvos Android/Linux",
    },
    {
        "id": "carplay",
        "nome": "CarPlay",
        "alvos": ["aarch64-apple-ios"],
        "gera": "nao existe binario de CarPlay: CarPlay e uma forma de APRESENTAR um app iOS",
        "roda": "a tela do carro, espelhando um aplicativo que roda no iPhone",
        "ferramenta": "Xcode, com o entitlement da Apple para CarPlay; o Rust entra como a"
                      " mesma biblioteca do iOS",
        "nucleo": "std pura na biblioteca",
        "ressalva": "CarPlay NAO e um alvo de compilacao do Rust. O que se mede aqui e o"
                    " alvo iOS, que e o que de fato se compila.",
    },
]

TIER12, TIER3, SEM_ALVO = "tier-1-2", "tier-3", "sem-alvo"
ROTULOS = {
    TIER12: "std pré-compilada (rustup target add)",
    TIER3: "tier 3 — existe, mas sem std pronta (nightly + build-std)",
    SEM_ALVO: "o rustc local não conhece este alvo",
}


def medir_rustc() -> dict:
    """O que a maquina AQUI compila. Sem rustc, INDISPONIVEL com o motivo."""
    if not shutil.which("rustc"):
        return {"disponivel": False, "motivo": "rustc nao encontrado no PATH",
                "conhecidos": set(), "com_std": set(), "versao": ""}
    def rodar(cmd):
        try:
            r = subprocess.run(cmd, capture_output=True, text=True, timeout=60)
        except (OSError, subprocess.SubprocessError):
            return None
        return r.stdout.splitlines() if r.returncode == 0 else None

    conhecidos = rodar(["rustc", "--print", "target-list"])
    if conhecidos is None:
        return {"disponivel": False, "motivo": "rustc nao respondeu a --print target-list",
                "conhecidos": set(), "com_std": set(), "versao": ""}
    # rustup pode faltar mesmo com rustc instalado (pacote da distribuicao):
    # ai nao da para separar tier 1/2 de tier 3, e isso e dito, nao chutado.
    com_std = rodar(["rustup", "target", "list"]) if shutil.which("rustup") else None
    versao = (rodar(["rustc", "--version"]) or [""])[0]
    return {
        "disponivel": True,
        "motivo": "",
        "conhecidos": {l.strip() for l in conhecidos if l.strip()},
        "com_std": {l.split()[0] for l in com_std if l.strip()} if com_std else None,
        "versao": versao,
    }


def avaliar(opcao: dict, m: dict) -> dict:
    if not m["disponivel"]:
        alvos = [{"alvo": a, "suporte": "INDISPONIVEL", "motivo": m["motivo"]} for a in opcao["alvos"]]
        return {**{k: v for k, v in opcao.items() if k != "alvos"},
                "alvos": alvos, "veredito": "INDISPONIVEL"}
    alvos = []
    for a in opcao["alvos"]:
        if a not in m["conhecidos"]:
            alvos.append({"alvo": a, "suporte": SEM_ALVO})
        elif m["com_std"] is None:
            alvos.append({"alvo": a, "suporte": "indefinido",
                          "motivo": "rustup ausente: nao da para saber se a std vem pronta"})
        else:
            alvos.append({"alvo": a, "suporte": TIER12 if a in m["com_std"] else TIER3})
    suportes = {x["suporte"] for x in alvos}
    # Misto e o caso do ESP32: o RISC-V bare-metal e tier 2 e o Xtensa e tier 3.
    # Dizer so "PRONTO" ali esconderia justamente a placa que da trabalho.
    if TIER12 in suportes and TIER3 in suportes:
        veredito = "PRONTO EM PARTE"
    elif TIER12 in suportes:
        veredito = "PRONTO"
    elif TIER3 in suportes:
        veredito = "TIER 3"
    elif "indefinido" in suportes:
        veredito = "INDEFINIDO"
    else:
        veredito = "SEM ALVO"
    return {**{k: v for k, v in opcao.items() if k != "alvos"},
            "alvos": alvos, "veredito": veredito}


def relatorio(m: dict) -> dict:
    return {
        "medido_em": datetime.now(timezone.utc).isoformat(timespec="seconds"),
        "rustc": {"disponivel": m["disponivel"], "versao": m["versao"], "motivo": m["motivo"],
                  "alvos_conhecidos": len(m["conhecidos"]),
                  "alvos_com_std": None if m["com_std"] is None else len(m["com_std"])},
        "opcoes": [avaliar(o, m) for o in CATALOGO],
    }


def listar(args, raiz: Path) -> int:
    m = medir_rustc()
    r = relatorio(m)
    if args.json:
        print(json.dumps(r, ensure_ascii=False, indent=2))
        return 0
    print("Interface do programa Rust — o que esta maquina compila\n")
    if not m["disponivel"]:
        print(f"  rustc: INDISPONÍVEL ({m['motivo']}) — o suporte abaixo não foi medido.\n")
    else:
        print(f"  {m['versao']} · {len(m['conhecidos'])} alvos conhecidos · "
              f"{'rustup ausente' if m['com_std'] is None else str(len(m['com_std'])) + ' com std pronta'}\n")
    for o in r["opcoes"]:
        print(f"  [{o['id']}] {o['nome']}")
        print(f"      veredito: {o['veredito']}")
        for a in o["alvos"]:
            print(f"        · {a['alvo']}: {ROTULOS.get(a['suporte'], a['suporte'])}"
                  + (f" — {a['motivo']}" if a.get("motivo") else ""))
        if o.get("ressalva"):
            print(f"      ressalva: {o['ressalva']}")
        print()
    print("  Escolha com: interface_do_destino.py escolher --opcao <id>")
    return 0


def manual(args, raiz: Path) -> int:
    m = medir_rustc()
    alvo = args.opcao
    escolhidas = [o for o in CATALOGO if alvo in (None, o["id"])]
    if not escolhidas:
        print(f"opção desconhecida: {alvo}", file=sys.stderr)
        return 2
    for o in escolhidas:
        a = avaliar(o, m)
        print(f"## {o['nome']}  ({o['id']})\n")
        print(f"O que gera .... {o['gera']}")
        print(f"Onde roda ..... {o['roda']}")
        print(f"Ferramenta .... {o['ferramenta']}")
        print(f"Núcleo ........ {o['nucleo']}")
        print(f"Suporte ....... {a['veredito']} (medido no rustc local)")
        for x in a["alvos"]:
            print(f"   · {x['alvo']}: {ROTULOS.get(x['suporte'], x['suporte'])}")
        if o.get("ressalva"):
            print(f"Ressalva ...... {o['ressalva']}")
        print()
    return 0


def escolher(args, raiz: Path) -> int:
    o = next((x for x in CATALOGO if x["id"] == args.opcao), None)
    if o is None:
        print(f"opção desconhecida: {args.opcao}. Veja `listar`.", file=sys.stderr)
        return 2
    m = medir_rustc()
    a = avaliar(o, m)
    destino = raiz / ".wx-migration"
    destino.mkdir(parents=True, exist_ok=True)
    ficha = {
        "escolhida": o["id"],
        "nome": o["nome"],
        "veredito": a["veredito"],
        "alvos": a["alvos"],
        "gera": o["gera"], "roda": o["roda"], "ferramenta": o["ferramenta"], "nucleo": o["nucleo"],
        "ressalva": o.get("ressalva", ""),
        "medido_em": datetime.now(timezone.utc).isoformat(timespec="seconds"),
        "rustc": m["versao"] or "INDISPONIVEL",
    }
    (destino / "interface.json").write_text(
        json.dumps(ficha, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    linhas = [f"# Interface do executavel: {o['nome']}", "",
              "Gerado por `interface_do_destino.py escolher`; nao edite a mao.", "",
              f"- Gera: {o['gera']}",
              f"- Roda em: {o['roda']}",
              f"- Ferramenta: {o['ferramenta']}",
              f"- Nucleo: {o['nucleo']}",
              f"- Veredito medido em {ficha['rustc'] or 'INDISPONIVEL'}: **{a['veredito']}**", ""]
    if o.get("ressalva"):
        linhas += [f"> {o['ressalva']}", ""]
    linhas += ["| alvo | suporte |", "| --- | --- |"]
    linhas += [f"| `{x['alvo']}` | {ROTULOS.get(x['suporte'], x['suporte'])} |" for x in a["alvos"]]
    linhas += ["", "Tier 3 quer dizer: o alvo existe e compila, mas ninguem distribui a `std`"
               " dele -- exige nightly e `-Z build-std`, e nao ha CI da equipe do Rust por tras.", ""]
    (destino / "interface-do-destino.md").write_text("\n".join(linhas), encoding="utf-8")
    # O questionario ganha o campo, nao uma pergunta nova: a contagem de perguntas
    # e verificada por teste, e acrescentar uma aqui a quebraria sem necessidade.
    q = raiz / "questionario.json"
    if q.is_file():
        try:
            dados = json.loads(q.read_text(encoding="utf-8"))
        except (OSError, ValueError):
            dados = None
        if isinstance(dados, dict) and isinstance(dados.get("H_backend"), dict):
            dados["H_backend"]["interface"] = o["id"]
            q.write_text(json.dumps(dados, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
            print(f"questionario.json: H_backend.interface = {o['id']}")
    print(f"escolhida: {o['nome']} — {a['veredito']}")
    print(f"gravado em {destino / 'interface.json'}")
    if a["veredito"] == "TIER 3":
        print("atenção: nenhum alvo desta forma tem std pronta; exige nightly e -Z build-std.")
    elif a["veredito"] == "PRONTO EM PARTE":
        tier3 = [x["alvo"] for x in a["alvos"] if x["suporte"] == TIER3]
        print("atenção: parte dos alvos é tier 3 (nightly + build-std): " + ", ".join(tier3))
    if o.get("ressalva"):
        print(f"ressalva: {o['ressalva']}")
    return 0


def main() -> int:
    p = argparse.ArgumentParser(description="a interface do Rust de destino, medida no rustc local")
    p.add_argument("--project-root", default=".")
    sub = p.add_subparsers(dest="cmd", required=True)
    l = sub.add_parser("listar", help="as nove formas, com o suporte medido")
    l.add_argument("--json", action="store_true")
    ma = sub.add_parser("manual", help="o manual de uma forma (ou de todas)")
    ma.add_argument("opcao", nargs="?")
    e = sub.add_parser("escolher", help="grava a forma escolhida")
    e.add_argument("--opcao", required=True)
    args = p.parse_args()
    raiz = Path(args.project_root).resolve()
    return {"listar": listar, "manual": manual, "escolher": escolher}[args.cmd](args, raiz)


try:
    import registro
except ImportError:
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import registro


if __name__ == "__main__":
    sys.exit(registro.envolver(__file__, main))
