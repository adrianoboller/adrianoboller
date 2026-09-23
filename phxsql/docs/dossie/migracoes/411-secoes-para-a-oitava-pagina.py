#!/usr/bin/env python3
"""Pedido 411 -- a mudanca das secoes 18, 32 e 35 do dossie para a oitava pagina.

MIGRACAO DE UMA VEZ SO. Ela nao vira gerador porque nao se repete: o dossie e
um arquivo redigido a mao com blocos gerados dentro, e esta e a cirurgia que o
levou de 2.703.573 bytes para dentro do teto de republicacao. Guardada aqui
para a proxima sessao conseguir ler o que exatamente saiu, e para o relatorio
poder listar item a item onde cada coisa foi parar.

    python3 411-mudanca-do-dossie.py [--medir]

`--medir` mostra o tamanho que sairia sem gravar nada.

JA FOI APLICADA em 23/09/2026. Rodar de novo PARA com `ValueError` no primeiro
`index()`, porque as secoes que ela procura viraram ponteiros -- e parar e o
certo: uma migracao que roda duas vezes e uma migracao que ninguem sabe se ja
rodou. Ela vive aqui, e nao no diretorio temporario da sessao, porque script
que resolveu algo nao pode morrer com a sessao: e ele que diz, linha a linha,
o que exatamente saiu do dossie e quantos bytes cada corte valeu.

Ela NAO entra na conta dos geradores da pasta de cima: o `docs/dossie/*.py` que
o `LEIA-ME.md` conta nao alcanca subpasta, e ela nao escreve numero visivel
nenhum -- escreveu uma vez, e acabou.
"""
import pathlib
import re
import sys

# `docs/dossie/migracoes/` -> `docs/dossie` -> `docs` -> `phxsql`
RAIZ = pathlib.Path(__file__).resolve().parents[3]
DOSSIE = next((RAIZ / "docs" / "dossie").glob("dossie-phxsql-*.html"))

# O aceite do contrato desta frente.
TETO = 460800


def corta(t, ini, fim, rotulo):
    i = t.index(ini)
    j = t.index(fim, i) + len(fim)
    print(f"  - {rotulo}: {len(t[i:j].encode()):>9} bytes")
    return t[:i] + t[j:], len(t[i:j].encode())


def troca_secao(t, sid, novo, rotulo):
    i = t.index(f'<section id="{sid}">')
    j = t.index("</section>", i) + len("</section>")
    antes = len(t[i:j].encode())
    t = t[:i] + novo + t[j:]
    print(f"  ~ {rotulo}: {antes:>9} -> {len(novo.encode()):>6} bytes")
    return t


PONTEIRO_18 = """<section id="s18">
  <div class="rotulo"><span class="num">18</span><span class="traco"></span></div>
  <h2>O console em imagens, do login à replicação</h2>
  <p>As capturas do Centro de Controle — dez telas nos dois temas, do login à
  replicação, tiradas contra o servidor de verdade — <strong>mudaram de
  página</strong>, e nenhuma se perdeu. Elas eram a maior parte deste arquivo,
  e o guarda da republicação exige reler a página publicada inteira antes de
  aceitar a nova: a galeria sozinha já não cabia no teto.</p>
  <p>Estão em <!--console:a1:inicio--><a href="console-em-imagens.html">O console
  em imagens</a><!--console:a1:fim-->, com as legendas, os dois temas e a
  bancada que foi junto.</p>
</section>"""

PONTEIRO_32 = """<section id="s32">
  <div class="rotulo"><span class="num">32</span><span class="traco"></span></div>
  <h2>A bancada: dez milhões de linhas, e os três motores a um milhão</h2>
  <p>A medição inteira — dez milhões de registros contra o MySQL(R) na mesma
  máquina, o diagnóstico de onde a inserção dói, e os três motores a um milhão
  de linhas com a faixa mínimo–máximo em cada barra — <strong>mudou de
  página</strong> junto com a galeria, porque só as duas juntas faziam este
  arquivo caber no teto de republicação.</p>
  <p>Está em <!--console:a2:inicio--><a href="console-em-imagens.html">O console
  em imagens</a><!--console:a2:fim-->, seção 2, com a tabela, as três figuras e
  as notas de bancada.</p>
</section>"""


def main():
    t = DOSSIE.read_text(encoding="utf-8")
    antes = len(t.encode())
    print(f"dossie antes: {antes:,} bytes".replace(",", "."))

    # 1) a secao 18 -- a galeria inteira
    t = troca_secao(t, "s18", PONTEIRO_18, "secao 18 (capturas)")

    # 2) a secao 32 -- a bancada
    t = troca_secao(t, "s32", PONTEIRO_32, "secao 32 (bancada)")

    # 3) a secao 35 -- so os dois paineis medidos e o roteiro saem; o painel
    #    dos pedidos FICA, porque ele e o resumo de capa do estado do projeto
    #    e quem o escreve (`pagina-dos-pedidos.py`) continua escrevendo aqui.
    i = t.index('<section id="s35">')
    j = t.index("</section>", i)
    s35 = t[i:j]
    k = s35.index("<h3>A trava global contra o MVCC")
    cabeca = s35[:k]
    #    Os dois paragrafos abaixo eram PROSA DIGITADA que ja contradizia o
    #    painel gerado logo acima dela: o texto dizia «as quatro parciais» com
    #    o painel mostrando quatorze. Sai, em vez de ser corrigido a mao para
    #    envelhecer de novo na rodada seguinte.
    for marca in ("<p><strong>Um pedido está na coluna «planejado»</strong>",
                  "<p>As <strong>quatro parciais</strong> são:"):
        a = cabeca.index(marca)
        b = cabeca.index("</p>", a) + len("</p>")
        print(f"  - secao 35, prosa digitada que contradizia o painel: "
              f"{len(cabeca[a:b].encode()):>6} bytes")
        cabeca = cabeca[:a] + cabeca[b:]
    novo35 = cabeca.rstrip() + """

  <p>Os dois painéis <strong>medidos</strong> desta seção — os quatro tetos da
  trava global e os testes por área — e o roteiro que ia com eles
  <strong>mudaram de página</strong>, para este arquivo caber no teto de
  republicação. Estão em <!--console:a3:inicio--><a href="console-em-imagens.html">O
  console em imagens</a><!--console:a3:fim-->, seção 3.</p>
</section>"""
    print(f"  ~ secao 35: {len(s35.encode()) + 10:>9} -> {len(novo35.encode()):>6} bytes")
    t = t[:i] + novo35 + t[j + len("</section>"):]

    # 4) a marca da capa. Ela NAO sai por preferencia: com o teto em 460.800
    #    bytes, os 75.394 bytes dela eram a diferenca entre caber e nao caber,
    #    e nenhuma resolucao menor cabia (medido: o menor derivado oficial, o
    #    icone de 40x34 px, ainda custa 1.990 bytes em base64 -- e um simbolo
    #    de 40 px numa placa de 440 e a marca mostrada mal). Ela MUDOU DE CASA,
    #    como as secoes: vive inteira, em 440 px, na pagina do console.
    t, _ = corta(t, '  <div class="placa">', "</div>\n", "marca da capa (placa)")

    # 5) o CSS que so servia ao que saiu -- a galeria e a placa da marca.
    #    Receita de numero envelhece; folha de estilo de secao que mudou de
    #    pagina tambem. Ela foi junto, para `pagina-do-console.py`.
    t, _ = corta(t, "/* ------------------------------------------------------------ capturas */",
                 ".tela.larga img{max-width:100%}\n\n", "CSS da galeria")
    t, _ = corta(t, """/* A marca vive sobre #010418 -- e o fundo oficial, medido dos originais.
   Sobre papel claro ela precisa levar esse fundo junto, senao o cilindro,
   que e escuro por dentro, vira um fantasma branco. */
.placa{""", ".marca{display:block;width:min(300px,62vw);height:auto}\n",
                 "CSS da placa/marca")
    t = t.replace("  figure,.rolo,pre,.nota,.tela{break-inside:avoid}\n",
                  "  figure,.rolo,pre,.nota{break-inside:avoid}\n", 1)
    t = t.replace("  .telas{grid-template-columns:1fr 1fr;gap:14pt}\n"
                  "  .tela img{box-shadow:none}\n", "", 1)
    t = t.replace("  .placa{box-shadow:none;padding:8px 14px}\n"
                  "  .marca{width:190px}\n", "", 1)

    depois = len(t.encode())
    print(f"dossie depois: {depois:,} bytes".replace(",", ".")
          + f"   (teto {TETO:,})".replace(",", "."))
    folga = TETO - depois
    print(f"folga: {folga:,} bytes ({100 * folga / TETO:.2f}%)".replace(",", "."))
    if "--medir" in sys.argv:
        return 0
    if depois > TETO:
        print("ACIMA DO TETO -- nao gravei.")
        return 1
    DOSSIE.write_text(t, encoding="utf-8")
    print(f"gravado: {DOSSIE}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
