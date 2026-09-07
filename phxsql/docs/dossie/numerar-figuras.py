#!/usr/bin/env python3
"""Renumera TODAS as legendas de figura do dossie, na ordem do documento.

    python3 docs/dossie/numerar-figuras.py [dossie.html]   # sem argumento, acha o da pasta

# Por que ele existe, e por que ele nasceu tarde

O dossie tinha 26 legendas `<b>Figura N.</b>` DIGITADAS. Enquanto as figuras
so entravam no fim, o numero digitado batia por sorte. Em 07/09/2026 entraram
duas no MEIO -- na secao 9 e na 31 --, e as 17 seguintes viraram mentira de
uma vez, sem ninguem tocar nelas.

E o pior tipo de mentira: legenda errada **nao quebra nada**. A pagina abre, o
desenho aparece, e so quem for conferir uma referencia cruzada descobre. E
ninguem confere referencia cruzada.

Entao a numeracao passa a sair daqui, e este script roda **por ultimo**, depois
dos geradores de bloco -- eles podem chutar o proprio numero, que este acerta
todos. E idempotente: rodar duas vezes da o mesmo resultado.

Legenda SEM numero (as 27 capturas de tela) fica como esta: elas nao sao
referenciadas por numero em lugar nenhum, e numera-las mudaria o significado
das referencias que ja existem.
"""

import pathlib
import re
import sys

# O nome do dossie muda a cada refacao: quem o acha e a varredura da pasta,
# num dono so. Padrao digitado aqui envelhece calado na proxima refacao.
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from dossie_da_pasta import achar_o_dossie  # noqa: E402

AQUI = pathlib.Path(__file__).resolve().parent
LEGENDA = re.compile(r"<b>Figura (\d+)\.</b>")


def main():
    alvo = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else achar_o_dossie()
    texto = alvo.read_text(encoding="utf-8")
    antes = [int(m.group(1)) for m in LEGENDA.finditer(texto)]
    if not antes:
        sys.exit("nenhuma legenda `<b>Figura N.</b>` -- a forma mudou e este "
                 "script passou a medir nada")

    conta = iter(range(1, len(antes) + 1))
    novo = LEGENDA.sub(lambda _: f"<b>Figura {next(conta)}.</b>", texto)
    depois = [int(m.group(1)) for m in LEGENDA.finditer(novo)]

    trocadas = sum(1 for a, d in zip(antes, depois) if a != d)
    alvo.write_text(novo, encoding="utf-8")
    print(f"{alvo.name}: {len(depois)} figuras numeradas, {trocadas} corrigidas")
    if trocadas:
        fora = [f"{a}->{d}" for a, d in zip(antes, depois) if a != d]
        print("  " + " ".join(fora[:12]) + (" ..." if len(fora) > 12 else ""))


if __name__ == "__main__":
    main()
