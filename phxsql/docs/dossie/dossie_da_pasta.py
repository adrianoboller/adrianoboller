"""Onde esta o dossie -- por VARREDURA, e nunca pelo nome escrito aqui.

O nome do dossie muda a cada refacao: era `dossie-phxsql.html`, virou `-0.15`,
hoje e `-0.18`. O `CLAUDE.md` promete que trocar o nome de novo nao exige
editar script nenhum -- e a promessa estava meio falsa: os scripts ACEITAM o
caminho por argumento, mas o PADRAO de sete deles era o nome digitado, em sete
lugares. Chamada nua depois da proxima refacao escreveria num arquivo que nao
existe mais.

E um deles nao tinha padrao nenhum. Medido em 07/09/2026: o
`pagina-dos-pedidos.py` chamado sem argumento gravava a pagina e a contagem e
pulava o painel do dossie CALADO -- o painel dizia 198 pedidos com o
PENDENCIAS.md em 203, cinco atras, sem ninguem ter digitado numero nenhum. O
`cobertura-por-area.py` e o irmao: mesmo laco, mesma falta.

A regra «so existe um dossie por vez» e o que torna a varredura segura, e ela
vira o portao: zero ou dois e parada com o motivo, nunca um palpite sobre qual
dos dois atualizar.
"""

import pathlib

PASTA = pathlib.Path(__file__).resolve().parent
PADRAO_DO_NOME = "dossie-phxsql-*.html"

# A OITAVA pagina (pedido 411, 23/09/2026). O dossie passou de 2,7 MB e o
# guarda da republicacao exige reler a versao publicada INTEIRA antes de
# aceitar a nova -- o mesmo teto de ~450 KiB que partiu a pagina dos pedidos
# em cinco (pedido 403). A secao 18 sozinha pesava 2.106.613 bytes (77,9% do
# arquivo), entao ela saiu do dossie e virou pagina propria, levando junto a
# bancada (secao 32) e os dois paineis medidos da secao 35.
#
# O nome dela NAO varre como o do dossie, e a diferenca e deliberada: o nome
# do dossie muda a cada refacao (`-0.15`, `-0.18`), o desta pagina nao muda
# com a versao. Mas ele mora AQUI, num dono so, pelo mesmo motivo da
# varredura -- seis geradores escrevem nela, e um nome digitado seis vezes e
# um nome que um dia diverge em cinco lugares.
NOME_DA_PAGINA_DO_CONSOLE = "console-em-imagens.html"


def pagina_do_console(exigir=True):
    """A oitava pagina -- onde as capturas, a bancada e os tetos passaram a viver.

    `exigir=False` devolve o caminho mesmo que o arquivo nao exista: e o que o
    proprio gerador dela usa, que precisa poder cria-la da primeira vez. Quem
    so ESCREVE num bloco dela para alto quando ela falta, porque escrever num
    arquivo que nao existe seria publicar numero em lugar nenhum.
    """
    p = PASTA / NOME_DA_PAGINA_DO_CONSOLE
    if exigir and not p.exists():
        raise SystemExit(
            f"A PAGINA DO CONSOLE NAO EXISTE: {p}\n"
            "  Ela e gerada -- rode `python3 docs/dossie/pagina-do-console.py`\n"
            "  antes dos geradores que escrevem blocos dentro dela."
        )
    return p


def achar_o_dossie():
    """O unico dossie da pasta. Para alto quando nao ha exatamente um."""
    achados = sorted(PASTA.glob(PADRAO_DO_NOME))
    if len(achados) == 1:
        return achados[0]
    if not achados:
        raise SystemExit(
            f"DOSSIE NAO ACHADO em {PASTA} (padrao `{PADRAO_DO_NOME}`).\n"
            "  Passe o caminho como argumento, ou conserte a pasta."
        )
    raise SystemExit(
        "DOIS DOSSIES na pasta, e so pode haver UM por vez:\n  "
        + "\n  ".join(a.name for a in achados)
        + "\nO anterior sai do repositorio no mesmo commit da refacao."
    )
