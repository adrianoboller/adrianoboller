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
