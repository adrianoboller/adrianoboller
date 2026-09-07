#!/usr/bin/env python3
"""Emissor de serial de ativação — a ferramenta de QUEM VENDE, fora do plugin.

Por que separado, e nao um subcomando do `licenca.py`: o `licenca.py` viaja
dentro do plugin, na maquina do cliente. Quem emite precisa da chave PRIVADA, e
chave privada nao pode morar na mesma pasta do produto entregue -- basta um `zip
-r` distraido. Aqui a pasta e outra, o `.gitignore` cobre as chaves e o livro, e
a ferramenta se empacota sozinha para levar num pendrive.

O que ela acrescenta ao `licenca.py`, alem de mudar de lugar: o LIVRO DE
EMISSOES. Quem vende precisa saber a quem vendeu, quando, ate quando e para
qual maquina -- e precisa reenviar o serial quando o cliente perde o e-mail.
O livro e append-only, um JSON por linha, nasce 0600 e mora em
`~/.wx-serial/` -- fora do repositorio, porque a primeira versao o criou ao
lado do script e esse e justamente o acidente a evitar. `WX_SERIAL_DIR` muda.

O que ela NAO faz, e esta escrito porque a diferenca importa:

  - nao revoga. Revogacao exige servidor que o cliente consulte, e nao ha.
    `marcar-revogado` anota no livro para VOCE saber; o serial continua valendo
    na maquina do cliente ate a validade.
  - nao conta instalacoes. O serial nao telefona para casa.
  - nao imprime a chave privada, nem em erro, nem em log.

Uso:
  emitir.py chaves --saida ./segredo
  emitir.py novo --cliente "Softhouse X" --validade 2027-12-31 --email a@b.c \\
      [--maquina ID] --chave-privada ./segredo/chave-privada.json
  emitir.py livro [--cliente X]
  emitir.py reenviar <id>
  emitir.py marcar-revogado <id> --porque "..."
  emitir.py empacotar --saida wx-serial.zip
"""
from __future__ import annotations

import argparse
import json
import os
import shutil
import sys
import zipfile
from datetime import date, datetime, timezone
from pathlib import Path

AQUI = Path(__file__).resolve().parent
# O livro NAO mora ao lado do script: rodando pela primeira vez ele nasceu
# dentro do repositorio, que e exatamente o acidente que esta ferramenta existe
# para evitar. Vai para a casa do usuario, e `WX_SERIAL_DIR` muda.
CASA = Path(os.environ.get("WX_SERIAL_DIR") or (Path.home() / ".wx-serial"))
LIVRO = CASA / "emissoes.jsonl"


def carregar_licenca():
    """A matematica vem do `licenca.py`, nunca de uma copia.

    Duas implementacoes da mesma assinatura divergem em silencio, e a regra do
    projeto e clara: criptografia se confere contra vetor oficial. Melhor um
    import com caminho explicito que um segundo RSA para manter.
    """
    candidatos = [
        Path(os.environ["WX_LICENCA_PY"]).parent if os.environ.get("WX_LICENCA_PY") else None,
        AQUI,  # copia posta pelo `empacotar`
        AQUI.parents[1] / "skills/conversao-wx/scripts",  # dentro do repositorio
    ]
    for c in candidatos:
        if c and (c / "licenca.py").is_file():
            sys.path.insert(0, str(c))
            import licenca  # noqa: PLC0415
            return licenca
    raise SystemExit("nao achei licenca.py. Rode dentro do repositório, use o pacote do "
                     "`empacotar`, ou aponte WX_LICENCA_PY=/caminho/licenca.py")


def escrever_livro(registro: dict) -> None:
    novo = not LIVRO.exists()
    LIVRO.parent.mkdir(parents=True, exist_ok=True)
    fd = os.open(LIVRO, os.O_WRONLY | os.O_CREAT | os.O_APPEND, 0o600)
    with os.fdopen(fd, "a", encoding="utf-8") as f:
        f.write(json.dumps(registro, ensure_ascii=False) + "\n")
    if novo:
        print(f"livro criado em {LIVRO} (0600; não entra no repositório)", file=sys.stderr)


def ler_livro() -> list[dict]:
    if not LIVRO.is_file():
        return []
    saida = []
    for linha in LIVRO.read_text(encoding="utf-8").splitlines():
        if linha.strip():
            try:
                saida.append(json.loads(linha))
            except ValueError:
                continue
    return saida


def chaves(args) -> int:
    lic = carregar_licenca()
    args.saida.mkdir(parents=True, exist_ok=True)
    priv, pub = lic.gerar_chaves()
    p = args.saida / "chave-privada.json"
    fd = os.open(p, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    with os.fdopen(fd, "w", encoding="utf-8") as f:
        json.dump(priv, f, indent=2)
    (args.saida / "chave-publica.json").write_text(json.dumps(pub, indent=2) + "\n", encoding="utf-8")
    print(f"privada: {p} (0600) — nunca entra no repositório nem no pacote do cliente")
    print(f"pública: {args.saida / 'chave-publica.json'} — copie para licenca/chave-publica.json do plugin")
    return 0


def novo(args) -> int:
    lic = carregar_licenca()
    try:
        d = date.fromisoformat(args.validade)
    except ValueError:
        print("validade deve ser AAAA-MM-DD", file=sys.stderr)
        return 2
    if d <= date.today():
        # emitir vencido nao e erro do cliente descobrir na hora de usar
        print(f"validade {args.validade} já passou; não emito serial nascido vencido.", file=sys.stderr)
        return 2
    priv = json.loads(args.chave_privada.read_text(encoding="utf-8"))
    serial = lic.gerar_serial(args.cliente, args.validade, priv, args.maquina, args.email, args.aviso)
    # A conferencia tem de usar a publica PAR da privada usada, nao a do plugin:
    # medido aqui, sem isto todo serial recem-emitido saia "assinatura-invalida"
    # -- e o emissor recusaria o proprio trabalho correto.
    par = args.chave_privada.parent / "chave-publica.json"
    pub = json.loads(par.read_text(encoding="utf-8")) if par.is_file() else None
    if pub is None:
        print(f"não achei a chave pública par em {par}; sem ela não confiro o que emiti.",
              file=sys.stderr)
        return 2
    conferido = lic.verificar_serial(serial, pub)
    if conferido["status"] not in {"valida", "maquina-diferente"}:
        # serial que nao passa no proprio verificador nao vai para o livro nem
        # para o cliente: seria descobrir o defeito na maquina de quem pagou
        print(f"o serial gerado não passou no verificador ({conferido['status']}); nada foi gravado.",
              file=sys.stderr)
        return 1
    escrever_livro({"id": conferido.get("id", ""), "cliente": args.cliente, "email": args.email,
                    "validade": args.validade, "maquina": args.maquina or "",
                    "aviso": args.aviso or "",
                    "emitido_em": datetime.now(timezone.utc).isoformat(timespec="seconds"),
                    "serial": serial, "revogado": False})
    print(serial)
    return 0


def livro(args) -> int:
    itens = [r for r in ler_livro()
             if not args.cliente or args.cliente.lower() in (r.get("cliente", "").lower())]
    if args.json:
        print(json.dumps(itens, ensure_ascii=False, indent=2))
        return 0
    if not itens:
        print("nenhuma emissão registrada.")
        return 0
    hoje = date.today().isoformat()
    print(f"{'id':<14} {'cliente':<26} {'validade':<12} {'máquina':<10} estado")
    for r in itens:
        est = "REVOGADO" if r.get("revogado") else ("vencido" if r["validade"] < hoje else "válido")
        maq = (r.get("maquina") or "—")[:10]
        print(f"{r.get('id', '')[:14]:<14} {r['cliente'][:26]:<26} {r['validade']:<12} {maq:<10} {est}")
    print(f"\n{len(itens)} emissões. O serial completo sai com `reenviar <id>`.")
    return 0


def reenviar(args) -> int:
    for r in ler_livro():
        if r.get("id", "").startswith(args.id):
            if r.get("revogado"):
                print(f"atenção: marcado como revogado em {r.get('revogado_em', '?')}", file=sys.stderr)
            print(r["serial"])
            return 0
    print(f"id {args.id} não está no livro.", file=sys.stderr)
    return 2


def marcar_revogado(args) -> int:
    itens = ler_livro()
    achou = False
    for r in itens:
        if r.get("id", "").startswith(args.id):
            r["revogado"] = True
            r["revogado_em"] = datetime.now(timezone.utc).isoformat(timespec="seconds")
            r["porque"] = args.porque
            achou = True
    if not achou:
        print(f"id {args.id} não está no livro.", file=sys.stderr)
        return 2
    fd = os.open(LIVRO, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    with os.fdopen(fd, "w", encoding="utf-8") as f:
        for r in itens:
            f.write(json.dumps(r, ensure_ascii=False) + "\n")
    print(f"{args.id} marcado como revogado NO SEU LIVRO.")
    print("Isto não invalida o serial na máquina do cliente: revogação de verdade")
    print("exige um servidor que ele consulte, e não há. Ele vale até a validade.")
    return 0


def empacotar(args) -> int:
    """Zip autonomo: este script, o licenca.py e o que ELE importa.

    A primeira versao levava so os dois, e o pacote nao rodava: `licenca.py`
    importa `registro`. Descompactar e rodar num diretorio vazio e a unica
    prova de que um pacote e autonomo -- ler a lista de arquivos nao e.
    """
    lic = carregar_licenca()
    origem = Path(lic.__file__)
    with zipfile.ZipFile(args.saida, "w", zipfile.ZIP_DEFLATED) as z:
        z.write(__file__, "emitir.py")
        z.write(origem, "licenca.py")
        for irmao in ("registro.py",):
            junto = origem.parent / irmao
            if junto.is_file():
                z.write(junto, irmao)
        z.writestr("LEIA-ME.txt",
                   "wx-serial — emissor de serial de ativação.\n\n"
                   "  python3 emitir.py chaves --saida ./segredo\n"
                   "  python3 emitir.py novo --cliente \"...\" --validade AAAA-MM-DD "
                   "--chave-privada ./segredo/chave-privada.json\n\n"
                   "A chave privada e o livro de emissões NÃO estão neste pacote, de propósito.\n"
                   "Guarde-os fora dele e fora de qualquer repositório.\n")
    print(f"{args.saida} — leva emitir.py e licenca.py; sem chave, sem livro.")
    return 0


def main() -> int:
    p = argparse.ArgumentParser(description="emissor de serial — ferramenta de quem vende",
                                formatter_class=argparse.RawDescriptionHelpFormatter, epilog=__doc__)
    sub = p.add_subparsers(dest="cmd", required=True)
    c = sub.add_parser("chaves"); c.add_argument("--saida", type=Path, required=True)
    n = sub.add_parser("novo")
    n.add_argument("--cliente", required=True); n.add_argument("--validade", required=True)
    n.add_argument("--email", default=""); n.add_argument("--maquina", default="")
    n.add_argument("--chave-privada", type=Path, required=True)
    n.add_argument("--aviso", default="", help="URL do receber.py que recebe o aviso de instalacao")
    l = sub.add_parser("livro"); l.add_argument("--cliente"); l.add_argument("--json", action="store_true")
    r = sub.add_parser("reenviar"); r.add_argument("id")
    v = sub.add_parser("marcar-revogado"); v.add_argument("id"); v.add_argument("--porque", default="")
    e = sub.add_parser("empacotar"); e.add_argument("--saida", type=Path, default=AQUI / "wx-serial.zip")
    a = p.parse_args()
    return {"chaves": chaves, "novo": novo, "livro": livro, "reenviar": reenviar,
            "marcar-revogado": marcar_revogado, "empacotar": empacotar}[a.cmd](a)


if __name__ == "__main__":
    sys.exit(main())
