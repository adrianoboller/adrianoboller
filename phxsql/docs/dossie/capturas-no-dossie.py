#!/usr/bin/env python3
"""Poe as capturas do console dentro do dossie, como data URI.

Existe pela mesma lei dos outros geradores desta pasta: o que e visivel ou sai
de um gerador, ou esta errado e ninguem percebeu ainda. Captura de tela
envelhece igual a numero -- e uma colada a mao fica mostrando a interface de
tres versoes atras sem ninguem notar.

    node   docs/dossie/capturar-dossie.mjs . /tmp/brutas    fotografa
    python3 docs/dossie/capturas-no-dossie.py --preparar /tmp/brutas
    python3 docs/dossie/capturas-no-dossie.py [dossie.html]  embute

De onde vem cada imagem:

    docs/dossie/capturar-dossie.mjs   sobe um phxsqld na faixa 6700/6701,
                                      popula, e fotografa com o Playwright
    docs/dossie/capturas/*.png        o resultado, ja reduzido, versionado

DENTRO do HTML, e nao ao lado: a pagina publicada e um arquivo so, e a politica
de conteudo do visualizador bloqueia imagem de qualquer outra origem. Ao lado
ela ficaria com dezenove quadros quebrados e nenhum erro visivel.

O peso e o motivo de as imagens serem reduzidas e quantizadas antes de entrar
(ver `preparar()`): a pagina inteira nao pode ficar impossivel de abrir.
"""

import base64
import pathlib
import sys

RAIZ = pathlib.Path(__file__).resolve().parents[2]
CAPTURAS = RAIZ / "docs" / "dossie" / "capturas"
ABRE = "<!-- capturas:inicio (gerado por docs/dossie/capturas-no-dossie.py) -->"
FECHA = "<!-- capturas:fim -->"


def _alvo():
    for a in sys.argv[1:]:
        if a.endswith(".html"):
            return pathlib.Path(a).resolve()
    return RAIZ / "docs" / "dossie" / "dossie-phxsql-0.18.html"


# A ordem e a do caminho que o dono pediu: do login ate a replicacao. O
# `larga` marca as que ocupam a fileira inteira -- a multitela e um panorama de
# 2.800 px, e cortada ao meio de uma grade de duas colunas ela nao diz nada.
TELAS = [
    ("login", "A entrada", False,
     "O desafio-resposta acontece <b>no navegador</b>: a página pede um desafio, "
     "deriva a prova com PBKDF2 ali mesmo e manda só a prova. As seis bandeiras "
     "trocam o texto na hora, e a escolha atravessa o login."),
    ("painel", "O painel", False,
     "Oito números, sete gráficos e a máquina embaixo — CPU, memória, placas de "
     "rede, discos e espaço livre, todos do <code>/proc</code>. O caminho do "
     "diretório de dados aparece <b>já resolvido</b>, porque relativo vale a "
     "partir de onde o servidor subiu."),
    ("tabelas", "Gestão de tabelas", False,
     "As tabelas do banco com registros, slots, colunas, índices e volumes. "
     "Clicar numa linha abre as oito operações sobre ela — e cada uma mexe num "
     "conjunto diferente dos arquivos, que é o que decide o que é reversível."),
    ("grade", "A grade", False,
     "Linha de filtro no cabeçalho, faixa de agrupamento, coluna congelada, "
     "seleção e exportar a vista. E o recado do alto é o ponto: a grade diz "
     "que enxerga <b>200 das 240 linhas</b>, em vez de deixar quem olha "
     "concluir que filtrou a tabela inteira."),
    ("query", "A consulta", False,
     "<code>SelectMemory</code> sobre uma tabela residente: 220 achadas de 240 "
     "examinadas. A tela diz, com todas as letras, que <b>isto não é SQL</b> — "
     "o <code>SELECT</code> de verdade entra pela operação <code>sql</code>."),
    ("diagrama", "O diagrama ER", False,
     "As três tabelas e as duas chaves estrangeiras, lidas do próprio "
     "<code>.reg</code>. É também o editor: arrastar a caixa move a tabela, e "
     "arrastar de uma coluna até outra <b>declara</b> a chave — declarar, e não "
     "aplicar, que é o que o motor faz hoje."),
    ("telemetria", "A telemetria", False,
     "As bolhas no molde do SQL Check: o tamanho é o <b>peso</b> — tempo de "
     "servidor —, o raio sai da raiz quadrada dele, e as mais leves têm um piso "
     "que aparece desenhado na escala, porque proporção quebrada em silêncio "
     "seria mentira sobre o dado."),
    ("profiler", "O profiler", False,
     "O que chega pela porta 5000, uma linha antes do despacho. Repare no "
     "<code>\"token\":\"***\"</code> de cada linha: a redação <b>analisa e "
     "reserializa</b> o pedido, nunca recorta o texto."),
    ("replicacao", "A replicação", False,
     "Papel, imagem da linha, e a posição do diário tabela a tabela — "
     "<b>o evento N é a posição N</b>, e por isso não há GTID a inventar."),
    ("multitela", "As quatro telas lado a lado", True,
     "O modo multitela numa janela de 2.800&nbsp;px: Diagrama ER, Telemetria, "
     "Profiler e Consulta <b>vivos ao mesmo tempo</b>, cada região com a própria "
     "tira de abas e uma calha arrastável entre elas. Custa ≈ 90 pedidos por "
     "minuto, medido — e ninguém está escondido, então pausar seria mentir "
     "sobre o que a tela mostra."),
]

TEMAS = [("escuro", "tema escuro"), ("claro", "tema claro")]


def data_uri(caminho: pathlib.Path) -> str:
    return "data:image/png;base64," + base64.b64encode(caminho.read_bytes()).decode("ascii")


# Quanto cada captura mede DEPOIS de reduzida, e por que estes numeros.
#
# 1.200 px e o dobro da largura em que ela aparece na pagina, entao ela ainda
# tem o que mostrar num monitor de duas vezes a densidade. A multitela vai a
# 2.000 porque o original tem 2.800 e ela e um panorama de quatro telas: a
# 1.200 o texto de dentro vira borrao.
#
# PNG com paleta de 160 cores, e nao JPEG: a captura e quase toda texto e
# linha fina, e o JPEG poe halo em volta de cada letra. Medido nas vinte:
# 1.505 KiB em PNG quantizado contra 1.501 em JPEG q82 -- mesmo peso, e um
# deles com o texto limpo.
LARGURA = {"multitela": 2000}
LARGURA_PADRAO = 1200
CORES = 160


def preparar(origem: str) -> None:
    """Reduz e quantiza as capturas brutas para `docs/dossie/capturas/`."""
    try:
        from PIL import Image
    except ImportError:
        sys.exit("--preparar precisa do Pillow; as capturas ja prontas nao precisam")
    bruto = pathlib.Path(origem).resolve()
    CAPTURAS.mkdir(parents=True, exist_ok=True)
    total = 0
    for f in sorted(bruto.glob("*.png")):
        im = Image.open(f).convert("RGB")
        larg = next((v for k, v in LARGURA.items() if k in f.name), LARGURA_PADRAO)
        alt = round(im.height * larg / im.width)
        p = CAPTURAS / f.name
        (im.resize((larg, alt), Image.LANCZOS)
           .convert("P", palette=Image.ADAPTIVE, colors=CORES)
           .save(p, optimize=True))
        total += p.stat().st_size
        print(f"  {f.name:26} {p.stat().st_size // 1024:4} KiB")
    print(f"{total // 1024} KiB em {CAPTURAS.relative_to(RAIZ)}")


def main() -> None:
    if "--preparar" in sys.argv:
        preparar(sys.argv[sys.argv.index("--preparar") + 1])
        return
    alvo = _alvo()
    if not CAPTURAS.is_dir():
        sys.exit(f"{CAPTURAS} nao existe -- rode antes o capturar-dossie.mjs")

    fig = []
    bytes_ = 0
    for nome, titulo, larga, legenda in TELAS:
        for tema, rotulo in TEMAS:
            p = CAPTURAS / f"{nome}-{tema}.png"
            if not p.exists():
                sys.exit(f"falta {p.name} -- rode o capturar-dossie.mjs")
            bytes_ += p.stat().st_size
            classe = "tela larga" if larga else "tela"
            alt = f"{titulo} do Centro de Controle do PhxSql, no {rotulo}"
            fig.append(f'  <figure class="{classe}">')
            fig.append(f'    <img src="{data_uri(p)}" alt="{alt}" loading="lazy">')
            fig.append(f'    <figcaption><span class="qual">{titulo} · {rotulo}</span>'
                       f"{legenda}</figcaption>")
            fig.append("  </figure>")

    bloco = '<div class="telas">\n' + "\n".join(fig) + "\n</div>"

    html = alvo.read_text(encoding="utf-8")
    i, j = html.find(ABRE), html.find(FECHA)
    if i < 0 or j < 0:
        sys.exit(f"{alvo.name} nao tem as marcas capturas:inicio/fim")
    html = html[:i] + ABRE + "\n" + bloco + "\n" + FECHA + html[j + len(FECHA):]
    alvo.write_text(html, encoding="utf-8")

    print(f"{len(TELAS) * len(TEMAS)} capturas embutidas em {alvo.name}")
    print(f"  {bytes_ // 1024} KiB de PNG  ->  {int(bytes_ * 4 / 3) // 1024} KiB em base64")
    print(f"  dossie: {alvo.stat().st_size // 1024} KiB")


if __name__ == "__main__":
    main()
