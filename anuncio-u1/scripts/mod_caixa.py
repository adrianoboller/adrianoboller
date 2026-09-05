# Modulo CAIXA do anuncio do Snapmaker U1 - versao 2: a caixa de papelao do
# cliente (Meshy), remodelada ao maximo (Revisao 2, item 1 da ESPECIFICACAO).
#
# O que e: corpo de 5 faces (parede de 8 mm real, oca) + 4 abas no topo com
# dobradica na aresta superior - duas grandes ao longo de X que se encontram
# no meio (a fita fica dividida entre elas) e duas pequenas ao longo de Y por
# baixo. Chanfro de 2,5 mm em toda aresta. A aparencia vem de um BAKE da
# Meshy (3 M tris) para esta geometria limpa: assets/caixa_cor[_2k].png,
# caixa_normal[_2k].png (espaco tangente) e caixa_rugosidade[_2k].png, feitos
# uma vez por scripts/bake_caixa.py (Cycles, selected-to-active) em 4096^2 e
# 2048^2 - 'resolucao_texturas' escolhe; o padrao e '2k' (5,1 MB) porque os
# 15,7 MB do 4k viram ~21 MB em base64 no arquivo unico. Este modulo so
# CARREGA os PNGs e os empacota no .blend. A etiqueta pendurada e a propria
# malha da Meshy (decimada a 6 k tris e desdobrada de novo), guardada em
# assets/caixa_etiqueta_malha.png como bytes, com as texturas originais dela
# baked em 1024^2 (caixa_etiqueta_{cor,normal,rugosidade}.png) - ver
# _decodificar_malha.
# A logo EnginePrint NAO vai mais na caixa (mudanca do cliente depois da
# Revisao 2): 'com_logo' fica False por padrao e o decal continua no codigo,
# desligado, caso ele volte atras. 'centro_logo', 'normal_logo' e
# 'topo_tampa_z' continuam na API e significam o CENTRO DO TOPO da caixa
# fechada (a emenda das abas grandes) - e para la que a camera final mergulha.
#
# So definicoes aqui - nada roda no import. Quem monta a cena e chama as
# animacoes e mod_coreografia.py; quem prova este modulo sozinho e
# teste_caixa.py. A versao anterior (caixa branca de tampa solta) esta em
# mod_caixa_v1_branca.py.bak, so como referencia.
#
# Eixos e medidas seguem docs/ESPECIFICACAO.md: metros, Z para cima, frente
# em -Y (a face dos icones), origem no centro da base da caixa.
#
# Decisoes desta versao (e o porque):
# - UV: uma ilha por face, sem sobreposicao, e o MESMO layout no modulo e no
#   bake (_layout_uv e deterministico a partir das medidas). As duas abas
#   grandes ficam ADJACENTES no atlas formando o topo inteiro: e o que deixa a
#   fita continua na emenda quando fechada e o que permite projetar a logo em
#   UV - uma so imagem, um so material, e a logo gruda em cada aba quando ela
#   gira. Projecao por espaco de mundo escorregaria pela aba em movimento;
#   por espaco de objeto exigiria um material por aba.
# - Faces nao impressas (interior, bordas de dobradica, fundo das abas, topo
#   das abas pequenas) NAO vem da Meshy: o raio do bake atravessaria a parede
#   e traria os icones espelhados para dentro. O bake_caixa.py preenche essas
#   ilhas com papelao liso copiado do fundo da propria Meshy.
# - Ordem de abertura: GRANDES primeiro, pequenas depois. As pequenas ficam
#   por baixo; se abrissem antes atravessariam as grandes (a ponta da pequena
#   sobe 31 cm e a grande cobre a largura inteira). Fechar e o inverso:
#   pequenas primeiro, grandes por cima. O parametro 'ordem' existe para quem
#   quiser o contrario, sabendo do atravessamento.
# - Dobradica: a origem de cada aba fica UMA ESPESSURA para fora da aresta
#   superior da parede. Com o pivo exatamente no canto, a espessura da aba
#   entrava 2-4 mm na parede a 120 graus; um pivo para fora e o que o papelao
#   faz na pratica (o vinco abaulado).
# - Sem chao (Revisao 2, item 2): os flocos de espuma sobem, passam POR CIMA
#   das abas abertas (funil de ~1,08 m de altura e ~0,48 m de alcance) e caem
#   ate sair do quadro por baixo, sumindo em fade de escala; ao voltar, entram
#   de fora pelo mesmo caminho.
# - 'tampa' virou um Empty no centro do topo, hide_render=True: a coreografia
#   le tampa.matrix_world (beat 7) e grava location/rotation nela no beat 1
#   (por isso NAO e filha do corpo), e o checador de colisoes pula objetos
#   com hide_render. As abas sao filhas do corpo e giram com ele.
# - A escala da Meshy e nao uniforme: o corpo dela mede 1,597 x 1,449 x 1,453
#   (o 1,90 x 1,52 do arquivo inclui a etiqueta), e o alvo e 0,72 x 0,62 x
#   0,80 - fatores 0,451 / 0,428 / 0,551. Os icones esticam ~1,25x na
#   vertical, aceito e anotado.
# - Logo: os valores medidos na versao branca (gamma 2,0 antes do AgX,
#   specular 0 e sheen 0 na tinta) valem; a tinta sobre papelao fica um pouco
#   mais fosca que o papelao (rugosidade do bake + 0,12).
# - Espuma (Revisao 4, item 2): packing peanuts classicos "tipo cheetos" -
#   tubo extrudado de secao trilobada, dobrado em S ou em 8, 3 a 5 cm, creme-
#   amarelo (#F0E2B4 a #E8D59A, sorteado por floco), poroso, sem brilho. So a
#   malha e o material mudaram: contagem (48), arrumacao em repouso, arco de
#   saida e volta sao os mesmos, porque 'caixa_raio' continua sendo o raio da
#   esfera envolvente e 'caixa_extensoes' e medido da malha nova - o mecanismo
#   de folga nao sabe a forma do floco. As pontas do S nao podem furar parede
#   nem U1: teste_caixa.py mede.

import math
import random

import bmesh
import bpy
from mathutils import Euler, Vector, noise

NOME = "caixa"

PARAMS_PADRAO = {
    # Externo da caixa fechada (abas incluidas). Interior = externo - 2 paredes.
    "exterior": (0.72, 0.62, 0.80),
    "parede": 0.008,
    "chanfro": 0.0025,
    "segmentos_chanfro": 2,
    "folga_aba": 0.001,               # aba pequena x parede lateral, por lado
    # Grade nominal do atlas em pixels: o layout e calculado nela e convertido
    # em fracoes de UV, entao a imagem real pode ter qualquer tamanho.
    "grade_atlas": 4096,
    "gutter_px": 16,
    "densidade_nao_impressa": 0.5,    # faces internas ocupam 1/4 da area
    # '2k' (padrao) usa caixa_*_2k.png (2048^2, ~4 MB no total: e o que cabe
    # no arquivo unico colado na aba Scripting); '4k' usa caixa_*.png
    # (4096^2, 15,7 MB). 'texturas' explicito (dict cor/normal/rugosidade)
    # manda sobre os dois.
    "resolucao_texturas": "2k",
    "texturas": None,
    "etiqueta": {
        "malha": "caixa_etiqueta_malha.png",
        "cor": "caixa_etiqueta_cor.png",
        "normal": "caixa_etiqueta_normal.png",
        "rugosidade": "caixa_etiqueta_rugosidade.png",
    },
    "cor": "clara",                   # so por compatibilidade: a cor vem do bake
    # Logo impressa no topo: DESLIGADA a pedido do cliente. True religa o
    # decal (dividido entre as duas abas grandes), com os valores medidos.
    "com_logo": False,
    "logo": "logo_engineprint.png",   # relativo a assets/; ou caminho absoluto
    "largura_logo": 0.45,             # fracao da largura externa (X) do topo
    "saturacao_logo": 1.0,
    # Gamma > 1 escurece a tinta antes do AgX (medido na versao branca: 2,0
    # leva o cinza da engrenagem de 164 para 58 sRGB, fonte 56). Depende da
    # exposicao da cena; quem iluminar diferente mede de novo no teste.
    "gamma_logo": 2.0,
    "abertura_grande": 120.0,         # graus, abas ao longo de X
    "abertura_pequena": 110.0,        # graus, abas ao longo de Y
    "sobrepasso": 0.05,               # fracao do angulo, overshoot ao abrir
    "ordem": "grandes_primeiro",      # ou "pequenas_primeiro" (atravessa!)
    "n_espumas": 48,
    # Packing peanut: 'raio_espuma' e o raio da esfera envolvente (meio eixo
    # maior), entao 3 a 5 cm de comprimento; 'secao_espuma' e o diametro da
    # secao extrudada; 'dobra_espuma' e a amplitude do S, fracao do
    # comprimento (Revisao 4, item 2).
    "raio_espuma": (0.015, 0.025),
    "secao_espuma": (0.012, 0.016),
    "dobra_espuma": (0.12, 0.28),
    "semente": 7,
    # Onde o U1 vai ficar dentro da caixa: as espumas se arrumam em volta
    # desse volume, mesmo sem o U1 existir na cena de teste.
    "u1": (0.584, 0.499, 0.730),
    # Onde os flocos "somem": abaixo disto (m) ja estao fora do 9:16 na
    # distancia de camera do beat 2 (medido na previa: a borda de baixo do
    # quadro fica em z ~ -0,65 a 2,1 m).
    "z_fora_do_quadro": -1.3,
}

SUFIXO_TEXTURAS = {"4k": "", "2k": "_2k"}


def nomes_texturas(p):
    """Arquivos de textura do corpo conforme 'resolucao_texturas' (ou o dict
    'texturas' explicito)."""
    if p.get("texturas"):
        return dict(p["texturas"])
    suf = SUFIXO_TEXTURAS.get(str(p.get("resolucao_texturas", "2k")).lower())
    if suf is None:
        print("[caixa] AVISO: resolucao_texturas %r desconhecida; usando '2k'" % (p.get("resolucao_texturas"),))
        suf = "_2k"
    return {k: "caixa_%s%s.png" % (k, suf) for k in ("cor", "normal", "rugosidade")}


# Papelao liso para quando as texturas nao existirem (kraft medio da Meshy,
# medido no atlas dela). Serve ao bake_caixa.py antes de existir o bake.
COR_PAPELAO = (0xD6, 0xA0, 0x66)
# Espuma creme-amarela (Revisao 4, item 2): cada floco sorteia entre as duas
# pelo Random do Object Info. Nada de branco puro. Sao a cor QUE SE VE; o
# albedo passa por _cor_espuma (abaixo), porque o AgX dessatura o claro.
COR_ESPUMA_CLARA = (0xF0, 0xE2, 0xB4)
COR_ESPUMA_ESCURA = (0xE8, 0xD5, 0x9A)
COR_ESPUMA = COR_ESPUMA_CLARA     # compatibilidade com quem lia o nome antigo
# Medido com o AgX (Medium High Contrast) aplicado por Image.save_render a
# valores lineares conhecidos, sem render: o albedo #F0E2B4 cru vira #CBC3B0
# na tela a radiancia 1,0 e #E0D9CA (B/R 0,90 - quase branco) na luz do
# teste, onde o lado da key recebe ~2x. A cor pedida como PIXEL exige albedo
# mais saturado que ela: cinza + k*(cor - cinza) em linear com k = 1,8 e
# teto 0,85 no canal maior (para o floco nao sair mais claro que um produto
# branco) da B/R de ~0,70-0,78 na tela entre radiancia 1,5 e 2,0, e mais
# amarelo na sombra - como packing peanut de verdade. Com k = 2,4 o escuro
# ja saia mostarda (B/R 0,31 a radiancia 1,0).
SATURACAO_ESPUMA_PRE_AGX = 1.8
TETO_ALBEDO_ESPUMA = 0.85
CORES = {"clara": COR_PAPELAO, "escura": COR_PAPELAO}   # compatibilidade

FPS = 30.0


# ---------------------------------------------------------------- utilidades

def _srgb_para_linear(c):
    c = c / 255.0
    return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4


def cor_linear(hex3):
    return tuple(_srgb_para_linear(v) for v in hex3) + (1.0,)


def _cor_espuma(hex3, k=None, teto=None):
    """Albedo linear do floco: a cor pedida com a saturacao reforcada em
    linear (ver SATURACAO_ESPUMA_PRE_AGX) e o canal maior limitado ao teto."""
    k = SATURACAO_ESPUMA_PRE_AGX if k is None else k
    teto = TETO_ALBEDO_ESPUMA if teto is None else teto
    r, g, b, _ = cor_linear(hex3)
    y = 0.2126 * r + 0.7152 * g + 0.0722 * b
    c = [max(0.0, y + k * (v - y)) for v in (r, g, b)]
    esc = min(1.0, teto / max(c))
    return tuple(v * esc for v in c) + (1.0,)


def limpar_colecao(nome):
    """Remove a sub-colecao <nome> e tudo o que ela contem (idempotencia)."""
    col = bpy.data.collections.get(nome)
    if col is None:
        return
    for obj in list(col.all_objects):
        dados = obj.data
        acao = obj.animation_data.action if obj.animation_data else None
        bpy.data.objects.remove(obj, do_unlink=True)
        if dados is not None and dados.users == 0:
            if isinstance(dados, bpy.types.Mesh):
                bpy.data.meshes.remove(dados)
        # A action orfa acumulava lixo no .blend a cada rodada (medido).
        if acao is not None and acao.users == 0:
            bpy.data.actions.remove(acao)
    for filha in list(col.children):
        limpar_colecao(filha.name)
    bpy.data.collections.remove(col)


def _colecao(cena, colecao_pai, nome):
    if colecao_pai is None:
        colecao_pai = bpy.data.collections.get("ANUNCIO")
        if colecao_pai is None:
            colecao_pai = bpy.data.collections.new("ANUNCIO")
        if colecao_pai.name not in cena.collection.children:
            cena.collection.children.link(colecao_pai)
    col = bpy.data.collections.new(nome)
    colecao_pai.children.link(col)
    return col


def _caminho_asset(nome_arquivo):
    import os
    if os.path.isabs(nome_arquivo):
        return nome_arquivo
    try:
        raiz = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    except NameError:
        # Aba Scripting sem arquivo (colado direto): vale a pasta do .blend.
        raiz = os.path.dirname(bpy.data.filepath) if bpy.data.filepath else os.getcwd()
    return os.path.join(raiz, "assets", nome_arquivo)


def _sombrear_suave(malha, suave=True):
    malha.polygons.foreach_set("use_smooth", [suave] * len(malha.polygons))
    malha.update()


def _medidas(p):
    """Derivadas das medidas externas. Tudo o mais sai daqui."""
    ex, ey, ez = p["exterior"]
    t = p["parede"]
    return {
        "ex": ex, "ey": ey, "ez": ez, "t": t,
        "hx": ex / 2.0, "hy": ey / 2.0,
        # Paredes +-Y sao uma espessura mais altas que as +-X: as abas grandes
        # apoiam nelas e nas pequenas ao mesmo tempo, sem fresta.
        "zx": ez - 2 * t,             # topo das paredes +-X e base das abas pequenas
        "zy": ez - t,                 # topo das paredes +-Y e base das abas grandes
        "L": ey / 2.0,                # comprimento de toda aba (as grandes se encontram)
        "wp": ey - 2 * t - 2 * p["folga_aba"],   # largura da aba pequena
    }


# ---------------------------------------------------------------- layout UV

def _majores(p):
    """As faces 'maiores' da caixa fechada, em coordenadas de MUNDO: para cada
    uma, origem, eixos u/v (m), tamanho (m), normal, se e impressa (vem da
    Meshy) e a que parte pertence. E a tabela que define as ilhas de UV."""
    m = _medidas(p)
    hx, hy, t, zx, zy, ez, L, wp = m["hx"], m["hy"], m["t"], m["zx"], m["zy"], m["ez"], m["L"], m["wp"]
    ex, ey = m["ex"], m["ey"]
    X, Y, Z = Vector((1, 0, 0)), Vector((0, 1, 0)), Vector((0, 0, 1))
    d = p["densidade_nao_impressa"]

    def maj(nome, origem, u, v, tam, normal, impressa, partes, dens=None):
        return {
            "nome": nome, "origem": Vector(origem), "u": u, "v": v, "tam": tam,
            "normal": normal, "impressa": impressa, "partes": set(partes),
            "dens": 1.0 if impressa else (d if dens is None else dens),
        }

    lista = [
        # corpo, faces externas (impressas)
        maj("fundo", (-hx, -hy, 0), X, Y, (ex, ey), -Z, True, ["corpo"]),
        maj("frente", (-hx, -hy, 0), X, Z, (ex, zy), -Y, True, ["corpo"]),
        maj("tras", (hx, hy, 0), -X, Z, (ex, zy), Y, True, ["corpo"]),
        maj("esquerda", (-hx, hy, 0), -Y, Z, (ey, zy), -X, True, ["corpo"]),
        maj("direita", (hx, -hy, 0), Y, Z, (ey, zy), X, True, ["corpo"]),
        # corpo, faces internas e bordas (papelao liso)
        maj("fundo_int", (-hx + t, -hy + t, t), X, Y, (ex - 2 * t, ey - 2 * t), Z, False, ["corpo"]),
        maj("frente_int", (-hx, -hy + t, t), X, Z, (ex, zy - t), Y, False, ["corpo"]),
        maj("tras_int", (hx, hy - t, t), -X, Z, (ex, zy - t), -Y, False, ["corpo"]),
        maj("esquerda_int", (-hx + t, hy - t, t), -Y, Z, (ey - 2 * t, zx - t), X, False, ["corpo"]),
        maj("direita_int", (hx - t, -hy + t, t), Y, Z, (ey - 2 * t, zx - t), -X, False, ["corpo"]),
        maj("borda_frente", (-hx, -hy, zy), X, Y, (ex, t), Z, False, ["corpo"], 1.0),
        maj("borda_tras", (-hx, hy - t, zy), X, Y, (ex, t), Z, False, ["corpo"], 1.0),
        maj("borda_esq", (-hx, -hy + t, zx), Y, X, (ey - 2 * t, t), Z, False, ["corpo"], 1.0),
        maj("borda_dir", (hx - t, -hy + t, zx), Y, X, (ey - 2 * t, t), Z, False, ["corpo"], 1.0),
        # topo: UMA ilha para as duas abas grandes (adjacentes na emenda y=0)
        maj("topo", (-hx, -hy, ez), X, Y, (ex, ey), Z, True, ["aba_frente", "aba_tras"]),
        # aba grande da frente (y de -hy a 0)
        maj("aba_frente.baixo", (-hx, -hy, zy), X, Y, (ex, L), -Z, False, ["aba_frente"]),
        maj("aba_frente.borda_fora", (-hx, -hy, zy), X, Z, (ex, t), -Y, True, ["aba_frente"]),
        maj("aba_frente.borda_ponta", (-hx, -hy + L, zy), X, Z, (ex, t), Y, False, ["aba_frente"], 1.0),
        maj("aba_frente.borda_esq", (-hx, -hy, zy), Y, Z, (L, t), -X, True, ["aba_frente"]),
        maj("aba_frente.borda_dir", (hx, -hy, zy), Y, Z, (L, t), X, True, ["aba_frente"]),
        # aba grande de tras (y de 0 a hy)
        maj("aba_tras.baixo", (-hx, 0.0, zy), X, Y, (ex, L), -Z, False, ["aba_tras"]),
        maj("aba_tras.borda_fora", (hx, hy, zy), -X, Z, (ex, t), Y, True, ["aba_tras"]),
        maj("aba_tras.borda_ponta", (-hx, 0.0, zy), X, Z, (ex, t), -Y, False, ["aba_tras"], 1.0),
        maj("aba_tras.borda_esq", (-hx, 0.0, zy), Y, Z, (L, t), -X, True, ["aba_tras"]),
        maj("aba_tras.borda_dir", (hx, 0.0, zy), Y, Z, (L, t), X, True, ["aba_tras"]),
        # aba pequena da esquerda (x de -hx a -hx+L), por baixo das grandes
        maj("aba_esq.topo", (-hx, -wp / 2, zy), X, Y, (L, wp), Z, False, ["aba_esq"]),
        maj("aba_esq.baixo", (-hx, -wp / 2, zx), X, Y, (L, wp), -Z, False, ["aba_esq"]),
        maj("aba_esq.borda_fora", (-hx, wp / 2, zx), -Y, Z, (wp, t), -X, True, ["aba_esq"]),
        maj("aba_esq.borda_ponta", (-hx + L, -wp / 2, zx), Y, Z, (wp, t), X, False, ["aba_esq"], 1.0),
        maj("aba_esq.borda_frente", (-hx, -wp / 2, zx), X, Z, (L, t), -Y, False, ["aba_esq"], 1.0),
        maj("aba_esq.borda_tras", (-hx, wp / 2, zx), X, Z, (L, t), Y, False, ["aba_esq"], 1.0),
        # aba pequena da direita (x de hx-L a hx)
        maj("aba_dir.topo", (hx - L, -wp / 2, zy), X, Y, (L, wp), Z, False, ["aba_dir"]),
        maj("aba_dir.baixo", (hx - L, -wp / 2, zx), X, Y, (L, wp), -Z, False, ["aba_dir"]),
        maj("aba_dir.borda_fora", (hx, -wp / 2, zx), Y, Z, (wp, t), X, True, ["aba_dir"]),
        maj("aba_dir.borda_ponta", (hx - L, -wp / 2, zx), Y, Z, (wp, t), -X, False, ["aba_dir"], 1.0),
        maj("aba_dir.borda_frente", (hx - L, -wp / 2, zx), X, Z, (L, t), -Y, False, ["aba_dir"], 1.0),
        maj("aba_dir.borda_tras", (hx - L, wp / 2, zx), X, Z, (L, t), Y, False, ["aba_dir"], 1.0),
    ]
    return lista


def _empacotar(retangulos, N, gutter):
    """Empacotamento em prateleiras: retangulos (nome, w_px, h_px) ordenados
    por altura, da esquerda para a direita, linha a linha. Devolve
    {nome: (x0, y0)} ou None se nao coube na grade N x N."""
    pos = {}
    x = y = gutter
    h_linha = 0
    for nome, w, h in sorted(retangulos, key=lambda r: (-r[2], -r[1])):
        if x + w + gutter > N:
            x = gutter
            y += h_linha + gutter
            h_linha = 0
        if y + h + gutter > N or x + w + gutter > N:
            return None
        pos[nome] = (x, y)
        x += w + gutter
        h_linha = max(h_linha, h)
    return pos


def _layout_uv(p):
    """Ilha por face, em pixels de uma grade nominal N x N, com a maior
    densidade (px/m) que ainda cabe. Deterministico: modulo e bake chamam
    isto e obtem o mesmo atlas. Devolve (layout, ppm, N); layout[nome] =
    {'px': (x0, y0, w, h), 'impressa': bool, ...}."""
    N = int(p["grade_atlas"])
    gutter = int(p["gutter_px"])
    majs = _majores(p)

    def rects(ppm):
        return [(m["nome"], max(2, int(round(m["tam"][0] * ppm * m["dens"]))),
                 max(2, int(round(m["tam"][1] * ppm * m["dens"]))))
                for m in majs]

    lo, hi = 50.0, 20000.0
    melhor = None
    for _ in range(48):
        meio = (lo + hi) / 2.0
        pos = _empacotar(rects(meio), N, gutter)
        if pos is None:
            hi = meio
        else:
            lo = meio
            melhor = (meio, pos)
    ppm, pos = melhor
    layout = {}
    for m, (nome, w, h) in zip(majs, rects(ppm)):
        x0, y0 = pos[nome]
        layout[nome] = {"px": (x0, y0, w, h), "impressa": m["impressa"], "dens": m["dens"],
                        "tam": m["tam"], "partes": m["partes"]}
    return layout, ppm, N


# ---------------------------------------------------------------- geometria

def _face(bm, vs, pontos):
    def V(x, y, z):
        k = (round(x, 6), round(y, 6), round(z, 6))
        v = vs.get(k)
        if v is None:
            v = bm.verts.new((x, y, z))
            vs[k] = v
        return v
    return bm.faces.new([V(*q) for q in pontos])


def _malha_corpo(bm, m):
    """Caixa oca aberta em cima: 5 faces externas, 5 internas e 4 bordas. As
    paredes +-Y sao mais altas (zy) que as +-X (zx) - ver _medidas."""
    hx, hy, t, zx, zy = m["hx"], m["hy"], m["t"], m["zx"], m["zy"]
    vs = {}
    F = lambda pts: _face(bm, vs, pts)  # noqa: E731
    F([(-hx, -hy, 0), (hx, -hy, 0), (hx, hy, 0), (-hx, hy, 0)])                                    # fundo
    F([(-hx + t, -hy + t, t), (hx - t, -hy + t, t), (hx - t, hy - t, t), (-hx + t, hy - t, t)])    # fundo interno
    for sy in (-1, 1):                                                                              # frente/tras
        y = sy * hy
        F([(-hx, y, 0), (hx, y, 0), (hx, y, zy), (-hx, y, zy)])
        yi = sy * (hy - t)                                                                          # internas
        F([(-hx + t, yi, t), (hx - t, yi, t), (hx - t, yi, zx), (hx, yi, zx), (hx, yi, zy),
           (-hx, yi, zy), (-hx, yi, zx), (-hx + t, yi, zx)])
        F([(-hx, y, zy), (hx, y, zy), (hx, yi, zy), (-hx, yi, zy)])                                 # borda
    for sx in (-1, 1):                                                                              # esquerda/direita
        x = sx * hx
        F([(x, -hy, 0), (x, hy, 0), (x, hy, zy), (x, hy - t, zy), (x, hy - t, zx),
           (x, -hy + t, zx), (x, -hy + t, zy), (x, -hy, zy)])
        xi = sx * (hx - t)
        F([(xi, -hy + t, t), (xi, hy - t, t), (xi, hy - t, zx), (xi, -hy + t, zx)])
        F([(x, -hy + t, zx), (xi, -hy + t, zx), (xi, hy - t, zx), (x, hy - t, zx)])                 # borda


def _malha_aba(bm, x0, x1, y0, y1, z0, z1):
    vs = {}
    F = lambda pts: _face(bm, vs, pts)  # noqa: E731
    F([(x0, y0, z0), (x1, y0, z0), (x1, y1, z0), (x0, y1, z0)])
    F([(x0, y0, z1), (x1, y0, z1), (x1, y1, z1), (x0, y1, z1)])
    F([(x0, y0, z0), (x1, y0, z0), (x1, y0, z1), (x0, y0, z1)])
    F([(x0, y1, z0), (x1, y1, z0), (x1, y1, z1), (x0, y1, z1)])
    F([(x0, y0, z0), (x0, y1, z0), (x0, y1, z1), (x0, y0, z1)])
    F([(x1, y0, z0), (x1, y1, z0), (x1, y1, z1), (x1, y0, z1)])


def _terminar_parte(bm, parte, p, layout, ppm, N, majs, pivo):
    """Chanfro real (bmesh), UV por face maior (projecao no plano dela para
    a ilha do layout), normais customizadas (faces maiores planas, chanfro
    suave) e translacao para a origem da parte. Devolve a Mesh."""
    chanfro = p["chanfro"]
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    marca = bm.faces.layers.int.new("maior")
    for f in bm.faces:
        f[marca] = 1
    bmesh.ops.bevel(bm, geom=bm.edges[:], offset=chanfro, segments=int(p["segmentos_chanfro"]),
                    affect="EDGES", profile=0.5, clamp_overlap=True)
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    bm.normal_update()

    candidatas = [m for m in majs if parte in m["partes"]]
    eps = chanfro * 1.6
    uv_layer = bm.loops.layers.uv.new("UVMap")
    for f in bm.faces:
        c = f.calc_center_median()
        melhor, melhor_dot = None, -2.0
        for m in candidatas:
            d = c - m["origem"]
            if abs(d.dot(m["normal"])) > eps:
                continue
            u, v = d.dot(m["u"]), d.dot(m["v"])
            if u < -eps or v < -eps or u > m["tam"][0] + eps or v > m["tam"][1] + eps:
                continue
            dot = f.normal.dot(m["normal"])
            if dot > melhor_dot:
                melhor, melhor_dot = m, dot
        if melhor is None:
            raise RuntimeError("face sem ilha em %s: centro %s normal %s" % (parte, tuple(c), tuple(f.normal)))
        x0, y0, w, h = layout[melhor["nome"]]["px"]
        esc = ppm * melhor["dens"] / N
        for lp in f.loops:
            d = lp.vert.co - melhor["origem"]
            lp[uv_layer].uv = (x0 / N + d.dot(melhor["u"]) * esc, y0 / N + d.dot(melhor["v"]) * esc)

    # Normais: face maior fica plana; no chanfro, o vertice que toca uma face
    # maior herda a normal dela (e o 'harden normals' feito a mao - sem isto
    # a face plana ganha gradiente de sombreamento pelas normais inclinadas).
    normais = []
    for f in bm.faces:
        for lp in f.loops:
            if f[marca]:
                normais.append(tuple(f.normal))
                continue
            vizinhas = [g.normal for g in lp.vert.link_faces if g[marca]]
            if not vizinhas:
                vizinhas = [g.normal for g in lp.vert.link_faces]
            n = Vector((0, 0, 0))
            for g in vizinhas:
                n += g
            normais.append(tuple(n.normalized()))
    if pivo is not None:
        bmesh.ops.translate(bm, verts=bm.verts, vec=-Vector(pivo))
    malha = bpy.data.meshes.new("caixa.%s" % parte)
    bm.to_mesh(malha)
    bm.free()
    _sombrear_suave(malha)
    try:
        malha.normals_split_custom_set(normais)
    except (AttributeError, RuntimeError) as e:
        print("[caixa] AVISO: sem normais customizadas (%s)" % e)
    malha.update()
    return malha


def geometria_caixa(p=None):
    """Malhas do corpo e das 4 abas (com UV e chanfro), mais o layout do
    atlas. E o que o bake_caixa.py usa para gerar as texturas: a geometria
    e IDENTICA a da cena final por construcao."""
    p = dict(PARAMS_PADRAO, **(p or {}))
    m = _medidas(p)
    hx, hy, t, zx, zy, ez, L, wp = m["hx"], m["hy"], m["t"], m["zx"], m["zy"], m["ez"], m["L"], m["wp"]
    layout, ppm, N = _layout_uv(p)
    majs = _majores(p)

    bm = bmesh.new()
    _malha_corpo(bm, m)
    corpo = _terminar_parte(bm, "corpo", p, layout, ppm, N, majs, None)

    # (nome, pivo no mundo, caixa (x0,x1,y0,y1,z0,z1) fechada, eixo de giro, sinal, angulo)
    abas_def = [
        ("aba_frente", (0.0, -hy - t, zy), (-hx, hx, -hy, -hy + L, zy, ez), 0, +1.0, p["abertura_grande"]),
        ("aba_tras", (0.0, hy + t, zy), (-hx, hx, hy - L, hy, zy, ez), 0, -1.0, p["abertura_grande"]),
        ("aba_esq", (-hx - t, 0.0, zx), (-hx, -hx + L, -wp / 2, wp / 2, zx, zy), 1, -1.0, p["abertura_pequena"]),
        ("aba_dir", (hx + t, 0.0, zx), (hx - L, hx, -wp / 2, wp / 2, zx, zy), 1, +1.0, p["abertura_pequena"]),
    ]
    abas = []
    for nome, pivo, cx, eixo, sinal, ang in abas_def:
        bm = bmesh.new()
        _malha_aba(bm, *cx)
        malha = _terminar_parte(bm, nome, p, layout, ppm, N, majs, pivo)
        abas.append({"nome": nome, "pivo": Vector(pivo), "malha": malha, "eixo": eixo, "sinal": sinal, "angulo": ang})
    return {"corpo": corpo, "abas": abas, "layout": layout, "ppm": ppm, "grade": N, "medidas": m, "params": p}


# ---------------------------------------------------------------- espuma

def _malha_espuma(nome, rng, raio, secao=0.014, dobra=(0.12, 0.28), aneis=40, segmentos=20):
    """Packing peanut classico ("tipo cheetos", Revisao 4, item 2): tubo
    extrudado de secao levemente trilobada e ondulada (com torcao, como sai
    da matriz), dobrado por deformacao senoidal ao longo do eixo - uma
    senoide inteira da o S (zero nas pontas e no meio); uma segunda
    harmonica forte, num plano perpendicular, da o "8" -, pontas
    arredondadas (superelipse) e superficie porosa por ruido fino radial.
    O eixo maior fica em X local com ~2*raio de comprimento. No fim a malha
    e recentrada na caixa envolvente e normalizada para o vertice mais
    distante da origem ficar a 'raio': e isso que 'caixa_raio' e
    'caixa_extensoes' medem, entao a arrumacao em repouso e a trajetoria
    nao dependem da forma. Tudo sai do 'rng' (semente fixa)."""
    L = 2.0 * raio
    r0 = secao / 2.0
    # secao: tres lobulos + leve achatamento, torcidos ao longo do eixo
    a3 = rng.uniform(0.10, 0.20)
    a2 = rng.uniform(0.0, 0.10)
    fase3 = rng.uniform(0.0, math.tau)
    fase2 = rng.uniform(0.0, math.tau)
    torcao = rng.uniform(-1.2, 1.2)
    # dobra: plano do S sorteado em volta do eixo; "8" em ~40% dos flocos
    A1 = rng.uniform(*dobra) * L
    psi = rng.uniform(0.0, math.tau)
    em_oito = rng.random() < 0.4
    A2 = A1 * (rng.uniform(0.45, 0.8) if em_oito else rng.uniform(0.0, 0.2))
    fase_oito = rng.uniform(0.0, math.tau)
    e1 = Vector((0.0, math.cos(psi), math.sin(psi)))
    e2 = Vector((0.0, -math.sin(psi), math.cos(psi)))
    expo = rng.uniform(2.2, 3.2)          # 2 = ponta elipsoidal; maior = mais cheia
    d1 = Vector((rng.uniform(-50, 50), rng.uniform(-50, 50), rng.uniform(-50, 50)))
    d2 = Vector((rng.uniform(-50, 50), rng.uniform(-50, 50), rng.uniform(-50, 50)))

    def centro(s):
        return (Vector((L * (s - 0.5), 0.0, 0.0)) + e1 * (A1 * math.sin(math.tau * s))
                + e2 * (A2 * math.sin(2.0 * math.tau * s + fase_oito)))

    def perfil(s):
        u = abs(2.0 * s - 1.0)
        return max(0.0, 1.0 - u ** expo) ** (1.0 / expo)

    bm = bmesh.new()
    n_an, n_seg = int(aneis), int(segmentos)
    fileiras = []
    N = None
    h = 1e-3
    for i in range(n_an):
        # espacamento em cosseno: aneis mais juntos nas pontas, onde o perfil
        # muda rapido; uniforme deixaria a ponta facetada
        s = 0.5 * (1.0 - math.cos(math.pi * i / (n_an - 1)))
        c = centro(s)
        T = (centro(min(s + h, 1.0)) - centro(max(s - h, 0.0))).normalized()
        # quadro de rotacao minima (transporte paralelo): a secao acompanha
        # a curva sem virar de repente
        if N is None:
            N = Vector((0.0, 0.0, 1.0)) if abs(T.z) < 0.9 else Vector((0.0, 1.0, 0.0))
        N = (N - T * N.dot(T)).normalized()
        B = T.cross(N)
        if i == 0 or i == n_an - 1:
            fileiras.append([bm.verts.new(c)])       # polo da ponta
            continue
        pf = perfil(s)
        fileira = []
        for j in range(n_seg):
            th = math.tau * j / n_seg
            r = r0 * pf * (1.0 + a3 * math.cos(3.0 * th + fase3 + torcao * s) + a2 * math.cos(2.0 * th + fase2))
            radial = N * math.cos(th) + B * math.sin(th)
            pos = c + radial * r
            # poros: duas oitavas de ruido radial, atenuadas na ponta (onde a
            # direcao radial degenera no polo)
            ru = 0.08 * noise.noise(pos * 260.0 + d1) + 0.04 * noise.noise(pos * 640.0 + d2)
            fileira.append(bm.verts.new(pos + radial * (r0 * ru * pf)))
        fileiras.append(fileira)
    for i in range(n_an - 1):
        a, b = fileiras[i], fileiras[i + 1]
        for j in range(n_seg):
            j2 = (j + 1) % n_seg
            if len(a) == 1:
                bm.faces.new((a[0], b[j], b[j2]))
            elif len(b) == 1:
                bm.faces.new((a[j], a[j2], b[0]))
            else:
                bm.faces.new((a[j], a[j2], b[j2], b[j]))
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    # recentra na caixa envolvente (o "8" nao e antissimetrico) e normaliza
    lo = Vector((min(v.co.x for v in bm.verts), min(v.co.y for v in bm.verts), min(v.co.z for v in bm.verts)))
    hi = Vector((max(v.co.x for v in bm.verts), max(v.co.y for v in bm.verts), max(v.co.z for v in bm.verts)))
    meio = (lo + hi) / 2.0
    for v in bm.verts:
        v.co -= meio
    maior = max(v.co.length for v in bm.verts)
    for v in bm.verts:
        v.co *= raio / maior
    malha = bpy.data.meshes.new(nome)
    bm.to_mesh(malha)
    bm.free()
    _sombrear_suave(malha)
    return malha


def _extensoes(malha, rot):
    R = rot.to_matrix() if hasattr(rot, "to_matrix") else Euler(rot).to_matrix()
    mx = my = mz = 0.0
    for v in malha.vertices:
        w = R @ v.co
        mx = max(mx, abs(w.x))
        my = max(my, abs(w.y))
        mz = max(mz, abs(w.z))
    return mx, my, mz


def _encolher(malha, fator):
    if fator < 1.0:
        for v in malha.vertices:
            v.co *= fator
        malha.update()


# ---------------------------------------------------------------- materiais

def _material_base(nome):
    """Pega ou cria o material por nome e reconstroi os nos do zero: e o que
    impede 'caixa.papelao.001' a cada rodada na aba Scripting do cliente."""
    mat = bpy.data.materials.get(nome)
    if mat is None:
        mat = bpy.data.materials.new(nome)
    mat.use_nodes = True
    nt = mat.node_tree
    nt.nodes.clear()
    saida = nt.nodes.new("ShaderNodeOutputMaterial")
    bsdf = nt.nodes.new("ShaderNodeBsdfPrincipled")
    saida.location = (600, 0)
    bsdf.location = (300, 0)
    nt.links.new(bsdf.outputs["BSDF"], saida.inputs["Surface"])
    if mat.animation_data:
        mat.animation_data_clear()
    if nt.animation_data:
        nt.animation_data_clear()
    return mat, nt, bsdf


def _carregar_imagem(caminho, nao_cor=False):
    """Carrega e empacota (o .blend gravado tem de ser autocontido). None se
    o arquivo nao existe."""
    import os
    if not os.path.exists(caminho):
        return None
    img = bpy.data.images.load(caminho, check_existing=True)
    img.colorspace_settings.name = "Non-Color" if nao_cor else "sRGB"
    try:
        if not img.packed_file:
            img.pack()
    except RuntimeError as e:
        print("[caixa] AVISO: nao empacotou %s: %s" % (caminho, e))
    return img


# Tinta x papelao. Specular 0 e sheen 0 na tinta (medidos na versao branca:
# no close do beat 7 o reflexo das luzes punha um piso de ~130 sRGB no cinza
# da engrenagem). O papelao nao tem sheen; o brilho dele vem do bake de
# rugosidade da Meshy, e a tinta fica RUG_TINTA_EXTRA mais fosca que ele.
SPEC_PAPELAO, SPEC_TINTA = 0.35, 0.0
RUG_PAPELAO_SEM_TEXTURA = 0.72
RUG_TINTA_EXTRA = 0.12


def _no_textura(nt, img, loc, nao_cor):
    tex = nt.nodes.new("ShaderNodeTexImage")
    tex.location = loc
    tex.image = img
    tex.interpolation = "Cubic"
    if img is not None:
        img.colorspace_settings.name = "Non-Color" if nao_cor else "sRGB"
    return tex


def _material_papelao(nome, p, imagens, logo, layout, ppm, N):
    """Papelao com as tres texturas do bake + decal da logo projetado em UV
    sobre a ilha 'topo' (as duas abas grandes), com o gamma medido."""
    mat, nt, bsdf = _material_base(nome)
    bsdf.inputs["Specular IOR Level"].default_value = SPEC_PAPELAO
    try:
        bsdf.inputs["Sheen Weight"].default_value = 0.0
    except KeyError:
        pass
    uv = nt.nodes.new("ShaderNodeUVMap")
    uv.location = (-1300, 0)
    uv.uv_map = "UVMap"

    # cor
    if imagens.get("cor") is not None:
        t_cor = _no_textura(nt, imagens["cor"], (-700, 300), False)
        nt.links.new(uv.outputs["UV"], t_cor.inputs["Vector"])
        cor_papel = t_cor.outputs["Color"]
    else:
        cor_papel = None
        bsdf.inputs["Base Color"].default_value = cor_linear(COR_PAPELAO)
    # rugosidade
    if imagens.get("rugosidade") is not None:
        t_rug = _no_textura(nt, imagens["rugosidade"], (-700, -50), True)
        nt.links.new(uv.outputs["UV"], t_rug.inputs["Vector"])
        rug_papel = t_rug.outputs["Color"]
    else:
        rug_papel = None
        bsdf.inputs["Roughness"].default_value = RUG_PAPELAO_SEM_TEXTURA
    # normal
    if imagens.get("normal") is not None:
        t_nrm = _no_textura(nt, imagens["normal"], (-700, -400), True)
        nt.links.new(uv.outputs["UV"], t_nrm.inputs["Vector"])
        nmap = nt.nodes.new("ShaderNodeNormalMap")
        nmap.location = (-300, -400)
        nmap.space = "TANGENT"
        nmap.uv_map = "UVMap"
        nmap.inputs["Strength"].default_value = 1.0
        nt.links.new(t_nrm.outputs["Color"], nmap.inputs["Color"])
        nt.links.new(nmap.outputs["Normal"], bsdf.inputs["Normal"])

    if logo is None:
        if cor_papel is not None:
            nt.links.new(cor_papel, bsdf.inputs["Base Color"])
        if rug_papel is not None:
            nt.links.new(rug_papel, bsdf.inputs["Roughness"])
        return mat

    # --- decal da logo, em UV: o quadrado da imagem cobre 'lado' metros do
    # topo (o DESENHO ocupa largura_logo da largura), centrado na ilha topo.
    imagem, fracao, centro_img = logo
    x0, y0, w, h = layout["topo"]["px"]
    centro_uv = ((x0 + w / 2.0) / N, (y0 + h / 2.0) / N)
    lado_m = p["largura_logo"] * p["exterior"][0] / fracao
    q = lado_m * ppm / N                      # lado do quadrado, em UV
    mapa = nt.nodes.new("ShaderNodeMapping")
    mapa.location = (-1000, 500)
    mapa.vector_type = "POINT"
    mapa.inputs["Scale"].default_value = (1.0 / q, 1.0 / q, 1.0)
    mapa.inputs["Location"].default_value = (centro_img[0] - centro_uv[0] / q, centro_img[1] - centro_uv[1] / q, 0.0)
    nt.links.new(uv.outputs["UV"], mapa.inputs["Vector"])
    t_logo = nt.nodes.new("ShaderNodeTexImage")
    t_logo.location = (-700, 600)
    t_logo.image = imagem
    t_logo.extension = "CLIP"                 # fora do quadrado: alfa 0
    t_logo.interpolation = "Cubic"
    nt.links.new(mapa.outputs["Vector"], t_logo.inputs["Vector"])
    mascara = t_logo.outputs["Alpha"]

    ultimo = t_logo.outputs["Color"]
    if abs(p["saturacao_logo"] - 1.0) > 1e-3:
        hsv = nt.nodes.new("ShaderNodeHueSaturation")
        hsv.location = (-450, 600)
        hsv.inputs["Saturation"].default_value = p["saturacao_logo"]
        nt.links.new(ultimo, hsv.inputs["Color"])
        ultimo = hsv.outputs["Color"]
    if abs(p["gamma_logo"] - 1.0) > 1e-3:
        gam = nt.nodes.new("ShaderNodeGamma")
        gam.location = (-250, 600)
        gam.inputs["Gamma"].default_value = p["gamma_logo"]
        nt.links.new(ultimo, gam.inputs["Color"])
        ultimo = gam.outputs["Color"]
    mix_cor = nt.nodes.new("ShaderNodeMixRGB")
    mix_cor.location = (0, 400)
    mix_cor.blend_type = "MIX"
    mix_cor.inputs["Color1"].default_value = cor_linear(COR_PAPELAO)
    if cor_papel is not None:
        nt.links.new(cor_papel, mix_cor.inputs["Color1"])
    nt.links.new(ultimo, mix_cor.inputs["Color2"])
    nt.links.new(mascara, mix_cor.inputs["Fac"])
    nt.links.new(mix_cor.outputs["Color"], bsdf.inputs["Base Color"])

    # tinta mais fosca que o papelao: rugosidade do bake + extra, pela mascara
    mais = nt.nodes.new("ShaderNodeMath")
    mais.location = (-300, -50)
    mais.operation = "ADD"
    mais.use_clamp = True
    mais.inputs[1].default_value = RUG_TINTA_EXTRA
    if rug_papel is not None:
        nt.links.new(rug_papel, mais.inputs[0])
    else:
        mais.inputs[0].default_value = RUG_PAPELAO_SEM_TEXTURA
    mix_rug = nt.nodes.new("ShaderNodeMixRGB")
    mix_rug.location = (0, 100)
    mix_rug.inputs["Color1"].default_value = (RUG_PAPELAO_SEM_TEXTURA,) * 3 + (1.0,)
    if rug_papel is not None:
        nt.links.new(rug_papel, mix_rug.inputs["Color1"])
    nt.links.new(mais.outputs["Value"], mix_rug.inputs["Color2"])
    nt.links.new(mascara, mix_rug.inputs["Fac"])
    nt.links.new(mix_rug.outputs["Color"], bsdf.inputs["Roughness"])

    spec = nt.nodes.new("ShaderNodeMath")
    spec.location = (0, -200)
    spec.operation = "MULTIPLY_ADD"
    nt.links.new(mascara, spec.inputs[0])
    spec.inputs[1].default_value = SPEC_TINTA - SPEC_PAPELAO
    spec.inputs[2].default_value = SPEC_PAPELAO
    nt.links.new(spec.outputs["Value"], bsdf.inputs["Specular IOR Level"])
    return mat


def _material_etiqueta(nome, imagens):
    mat, nt, bsdf = _material_base(nome)
    bsdf.inputs["Specular IOR Level"].default_value = 0.4
    uv = nt.nodes.new("ShaderNodeUVMap")
    uv.location = (-1000, 0)
    uv.uv_map = "UVMap"
    if imagens.get("cor") is not None:
        t = _no_textura(nt, imagens["cor"], (-600, 300), False)
        nt.links.new(uv.outputs["UV"], t.inputs["Vector"])
        nt.links.new(t.outputs["Color"], bsdf.inputs["Base Color"])
    else:
        bsdf.inputs["Base Color"].default_value = (0.9, 0.9, 0.88, 1.0)
    if imagens.get("rugosidade") is not None:
        t = _no_textura(nt, imagens["rugosidade"], (-600, 0), True)
        nt.links.new(uv.outputs["UV"], t.inputs["Vector"])
        nt.links.new(t.outputs["Color"], bsdf.inputs["Roughness"])
    else:
        bsdf.inputs["Roughness"].default_value = 0.6
    if imagens.get("normal") is not None:
        t = _no_textura(nt, imagens["normal"], (-600, -300), True)
        nt.links.new(uv.outputs["UV"], t.inputs["Vector"])
        nmap = nt.nodes.new("ShaderNodeNormalMap")
        nmap.location = (-300, -300)
        nmap.space = "TANGENT"
        nmap.uv_map = "UVMap"
        nt.links.new(t.outputs["Color"], nmap.inputs["Color"])
        nt.links.new(nmap.outputs["Normal"], bsdf.inputs["Normal"])
    return mat


def _material_espuma(nome):
    """Packing peanut de amido (Revisao 4, item 2): creme-amarelo sorteado
    por floco entre COR_ESPUMA_CLARA e COR_ESPUMA_ESCURA (Object Info >
    Random), fosco (rugosidade 0,85, especular quase zero: sem brilho),
    subsurface 0,2 com raio amarelado - a espuma deixa a luz entrar um
    pouco e e isso que tira a cara de plastico -, e poros por ruido fino em
    bump, com as cavidades um pouco mais escuras. Nada de branco puro."""
    mat, nt, bsdf = _material_base(nome)
    bsdf.inputs["Roughness"].default_value = 0.85
    bsdf.inputs["Specular IOR Level"].default_value = 0.08
    for chave, valor in (("Sheen Weight", 0.0), ("Coat Weight", 0.0), ("Subsurface Weight", 0.2),
                         ("Subsurface Radius", (1.0, 0.8, 0.4)), ("Subsurface Scale", 0.004)):
        try:
            bsdf.inputs[chave].default_value = valor
        except KeyError:
            pass                              # nome de outra versao do Principled
    # cor por floco
    info = nt.nodes.new("ShaderNodeObjectInfo")
    info.location = (-900, 400)
    mix = nt.nodes.new("ShaderNodeMixRGB")
    mix.location = (-600, 400)
    mix.blend_type = "MIX"
    mix.inputs["Color1"].default_value = _cor_espuma(COR_ESPUMA_CLARA)
    mix.inputs["Color2"].default_value = _cor_espuma(COR_ESPUMA_ESCURA)
    nt.links.new(info.outputs["Random"], mix.inputs["Fac"])
    # poros: ruido em coordenadas de objeto (acompanha o floco girando e
    # encolhendo). Celula de ~1,5 mm: com 1,1 mm (escala 900) o close a 40
    # cm em 540x960 nao mostrava poro nenhum - 2,5 px por celula some no
    # filtro; a 1080x1920 do final fica com o dobro.
    coords = nt.nodes.new("ShaderNodeTexCoord")
    coords.location = (-1200, -100)
    ruido = nt.nodes.new("ShaderNodeTexNoise")
    ruido.location = (-900, -100)
    ruido.inputs["Scale"].default_value = 650.0
    ruido.inputs["Detail"].default_value = 4.0
    ruido.inputs["Roughness"].default_value = 0.6
    nt.links.new(coords.outputs["Object"], ruido.inputs["Vector"])
    bump = nt.nodes.new("ShaderNodeBump")
    bump.location = (-300, -300)
    bump.inputs["Strength"].default_value = 0.6
    bump.inputs["Distance"].default_value = 0.0008
    nt.links.new(ruido.outputs["Fac"], bump.inputs["Height"])
    nt.links.new(bump.outputs["Normal"], bsdf.inputs["Normal"])
    # cavidades 10% mais escuras: cor x (0,90 + 0,10 x ruido)
    sombra = nt.nodes.new("ShaderNodeMath")
    sombra.location = (-600, 100)
    sombra.operation = "MULTIPLY_ADD"
    sombra.inputs[1].default_value = 0.10
    sombra.inputs[2].default_value = 0.90
    nt.links.new(ruido.outputs["Fac"], sombra.inputs[0])
    mult = nt.nodes.new("ShaderNodeMixRGB")
    mult.location = (-300, 300)
    mult.blend_type = "MULTIPLY"
    mult.inputs["Fac"].default_value = 1.0
    nt.links.new(mix.outputs["Color"], mult.inputs["Color1"])
    nt.links.new(sombra.outputs["Value"], mult.inputs["Color2"])
    nt.links.new(mult.outputs["Color"], bsdf.inputs["Base Color"])
    return mat


def _medir_conteudo(img):
    """Fracao da largura da imagem ocupada pelo desenho (pela alfa) e o centro
    dele em UV: 'logo com 45% da largura' e o desenho, nao o arquivo."""
    import numpy as np
    w, h = img.size
    px = np.empty(w * h * 4, dtype=np.float32)
    img.pixels.foreach_get(px)
    alfa = px.reshape(h, w, 4)[:, :, 3] > 0.05
    linhas = np.where(alfa.any(axis=1))[0]
    colunas = np.where(alfa.any(axis=0))[0]
    if len(linhas) == 0 or len(colunas) == 0:
        return 1.0, (0.5, 0.5)
    x0, x1 = colunas[0], colunas[-1] + 1
    y0, y1 = linhas[0], linhas[-1] + 1
    fracao = max((x1 - x0) / w, (y1 - y0) / h)
    centro = ((x0 + x1) / (2.0 * w), (y0 + y1) / (2.0 * h))
    return fracao, centro


def _carregar_logo(caminho):
    import os
    if os.path.exists(caminho):
        img = _carregar_imagem(caminho, nao_cor=False)
        return img, False
    import numpy as np
    img = bpy.data.images.new("caixa.logo_provisoria", 256, 256, alpha=True)
    px = np.zeros((256, 256, 4), dtype=np.float32)
    px[32:224, 32:224] = (0.85, 0.35, 0.10, 1.0)
    px[96:160, 96:160] = (0.0, 0.0, 0.0, 0.0)
    img.pixels.foreach_set(px.ravel())
    img.pack()
    return img, True


# ---------------------------------------------------------------- etiqueta

# A malha da etiqueta viaja num PNG RGB de 8 bits (assets/caixa_etiqueta_
# malha.png), lido como bytes: o modulo so pode carregar PNGs e nao pode
# depender de base64/zlib. Formato (little-endian, uint16 salvo onde dito):
#   'ETQ1' | nv | nt | bbox (6 x uint32: (coord + 4) * 1e5) | verts nv x 3
#   uint16 (quantizados na bbox) | uv nv x 2 uint16 (/65535) | tris nt x 3
#   uint16. Os 3 canais de cada pixel sao 3 bytes seguidos; A = 255.
def _decodificar_malha(img):
    import numpy as np
    w, h = img.size
    px = np.empty(w * h * 4, dtype=np.float32)
    img.pixels.foreach_get(px)
    b = np.rint(px.reshape(-1, 4)[:, :3] * 255.0).astype(np.uint8).ravel()
    if b[:4].tobytes() != b"ETQ1":
        raise ValueError("assinatura da malha da etiqueta nao confere")
    nv, nt = np.frombuffer(b[4:8].tobytes(), dtype="<u2")
    nv, nt = int(nv), int(nt)
    bb = np.frombuffer(b[8:32].tobytes(), dtype="<u4").astype(np.float64) / 1e5 - 4.0
    i = 32
    vq = np.frombuffer(b[i:i + nv * 6].tobytes(), dtype="<u2").reshape(nv, 3).astype(np.float64)
    i += nv * 6
    uvq = np.frombuffer(b[i:i + nv * 4].tobytes(), dtype="<u2").reshape(nv, 2).astype(np.float64)
    i += nv * 4
    tris = np.frombuffer(b[i:i + nt * 6].tobytes(), dtype="<u2").reshape(nt, 3).astype(np.int64)
    verts = bb[:3] + vq / 65535.0 * (bb[3:] - bb[:3])
    return verts, uvq / 65535.0, tris


def _construir_etiqueta(p, col, corpo):
    import numpy as np
    caminho = _caminho_asset(p["etiqueta"]["malha"])
    img = _carregar_imagem(caminho, nao_cor=True)
    if img is None:
        print("[caixa] AVISO: etiqueta ausente (%s); a caixa fica sem ela" % caminho)
        return None
    verts, uv, tris = _decodificar_malha(img)
    malha = bpy.data.meshes.new("caixa.etiqueta")
    malha.from_pydata([tuple(v) for v in verts], [], [tuple(int(k) for k in t) for t in tris])
    malha.validate()
    camada = malha.uv_layers.new(name="UVMap")
    vi = np.empty(len(malha.loops), dtype=np.int64)
    malha.loops.foreach_get("vertex_index", vi)
    camada.data.foreach_set("uv", uv[vi].astype(np.float32).ravel())
    _sombrear_suave(malha)
    malha.update()
    obj = bpy.data.objects.new("caixa.etiqueta", malha)
    obj.parent = corpo
    col.objects.link(obj)
    imagens = {
        "cor": _carregar_imagem(_caminho_asset(p["etiqueta"]["cor"]), False),
        "normal": _carregar_imagem(_caminho_asset(p["etiqueta"]["normal"]), True),
        "rugosidade": _carregar_imagem(_caminho_asset(p["etiqueta"]["rugosidade"]), True),
    }
    obj.data.materials.append(_material_etiqueta("caixa.etiqueta", imagens))
    return obj


# ---------------------------------------------------------------- construir

def construir_caixa(cena, colecao_pai=None, params=None):
    """Cria corpo, abas, tampa (Empty), etiqueta e espumas na sub-colecao
    'caixa'. Devolve objetos e medidas. Idempotente."""
    p = dict(PARAMS_PADRAO)
    if params:
        p.update(params)
    limpar_colecao(NOME)
    col = _colecao(cena, colecao_pai, NOME)
    rng = random.Random(p["semente"])

    geo = geometria_caixa(p)
    m = geo["medidas"]
    ex, ey, ez, t = m["ex"], m["ey"], m["ez"], m["t"]
    layout, ppm, N = geo["layout"], geo["ppm"], geo["grade"]
    interior = (ex - 2 * t, ey - 2 * t, m["zx"] - t)     # ate a base das abas pequenas
    ix, iy, iz = interior

    nomes = nomes_texturas(p)
    imagens = {
        "cor": _carregar_imagem(_caminho_asset(nomes["cor"]), False),
        "normal": _carregar_imagem(_caminho_asset(nomes["normal"]), True),
        "rugosidade": _carregar_imagem(_caminho_asset(nomes["rugosidade"]), True),
    }
    faltam = [k for k, v in imagens.items() if v is None]
    if faltam:
        print("[caixa] AVISO: texturas do bake ausentes: %s (papelao liso no lugar)" % faltam)

    largura_logo = p["largura_logo"] * ex
    if p["com_logo"]:
        imagem, provisoria = _carregar_logo(_caminho_asset(p["logo"]))
        if provisoria:
            print("[caixa] AVISO: logo nao encontrada; usando quadrado provisorio")
        fracao, centro_img = _medir_conteudo(imagem)
        logo = (imagem, fracao, centro_img)
    else:
        # Sem logo: nada e carregado, e 'logo_provisoria' nao pode acusar
        # falta de um arquivo que nao e usado.
        imagem, provisoria, logo = None, False, None
    mat_papelao = _material_papelao("caixa.papelao", p, imagens, logo, layout, ppm, N)

    corpo = bpy.data.objects.new("caixa.corpo", geo["corpo"])
    col.objects.link(corpo)
    corpo.data.materials.append(mat_papelao)

    abas = []
    for a in geo["abas"]:
        obj = bpy.data.objects.new("caixa.%s" % a["nome"], a["malha"])
        obj.location = a["pivo"]
        obj.rotation_mode = "XYZ"
        obj.parent = corpo
        obj.data.materials.append(mat_papelao)
        obj["caixa_eixo"] = int(a["eixo"])
        obj["caixa_sinal"] = float(a["sinal"])
        obj["caixa_angulo"] = float(a["angulo"])
        col.objects.link(obj)
        abas.append(obj)

    # 'tampa': Empty no centro do topo (a emenda das abas grandes, onde a
    # camera final mergulha), solto (a coreografia grava location e rotation
    # nele por quadro no beat 1) e hide_render para o checador de colisoes da
    # coreografia pula-lo - ele nao voa mais para longe.
    tampa = bpy.data.objects.new("caixa.tampa", None)
    tampa.empty_display_type = "PLAIN_AXES"
    tampa.empty_display_size = 0.1
    tampa.location = (0.0, 0.0, ez)
    tampa.hide_render = True
    col.objects.link(tampa)
    marcador = bpy.data.objects.new("caixa.logo", None)
    marcador.empty_display_type = "ARROWS"
    marcador.empty_display_size = 0.05
    marcador.parent = tampa
    marcador.hide_render = True
    col.objects.link(marcador)

    etiqueta = _construir_etiqueta(p, col, corpo)

    # Alcance do funil das abas abertas: os flocos passam por cima disto.
    L = m["L"]
    ag, ap = math.radians(p["abertura_grande"]), math.radians(p["abertura_pequena"])
    funil = (m["hx"] + t + max(0.0, -math.cos(ap)) * (L + t) + t,
             m["hy"] + t + max(0.0, -math.cos(ag)) * (L + t) + t,
             max(m["zy"] + (L + t) * math.sin(ag), m["zx"] + (L + t) * math.sin(ap)) + t)

    # --- espumas: packing peanuts; a arrumacao (camada de cima + laterais)
    # e a mesma da versao anterior, so muda o que _malha_espuma devolve ---
    mat_espuma = _material_espuma("caixa.espuma")
    espumas = []
    ux, uy, uz = p["u1"]
    n = int(p["n_espumas"])
    r_min, r_max = p["raio_espuma"]
    s_min, s_max = p["secao_espuma"]
    FOLGA_ESPUMA = 0.003
    camada = iz - uz
    vao_x, vao_y = (ix - ux) / 2.0, (iy - uy) / 2.0
    ocupados = []
    for i in range(n):
        raio = rng.uniform(r_min, r_max)
        malha = _malha_espuma("caixa.espuma.%03d" % (i + 1), rng, raio,
                              secao=rng.uniform(s_min, s_max), dobra=p["dobra_espuma"])
        obj = bpy.data.objects.new("caixa.espuma.%03d" % (i + 1), malha)
        obj.data.materials.append(mat_espuma)
        # O tubo ja e liso (o chanfro era para os cantos amassados da versao
        # anterior); o subsurf de nivel 1 so arredonda as 20 arestas da
        # secao, que apareciam na silhueta do close a 40 cm.
        sub = obj.modifiers.new("suave", "SUBSURF")
        sub.levels = 1
        sub.render_levels = 1
        if rng.random() < 0.62:
            rot = Euler((rng.uniform(-0.35, 0.35), rng.uniform(-0.35, 0.35), rng.uniform(0, math.tau)))
            rx, ry, rz = _extensoes(malha, rot)
            if 2 * rz + 2 * FOLGA_ESPUMA > camada:
                f = (camada - 2 * FOLGA_ESPUMA) / (2 * rz)
                _encolher(malha, f)
                raio *= f
                rx, ry, rz = rx * f, ry * f, rz * f
            z_min = t + uz + rz + FOLGA_ESPUMA
            z_max = t + iz - rz - FOLGA_ESPUMA
            z = rng.uniform(z_min, max(z_min, z_max))
            for _ in range(40):
                x = rng.uniform(-ix / 2 + rx, ix / 2 - rx)
                y = rng.uniform(-iy / 2 + ry, iy / 2 - ry)
                if all((x - ox) ** 2 + (y - oy) ** 2 > (0.8 * (raio + orr)) ** 2 for ox, oy, orr in ocupados):
                    break
            ocupados.append((x, y, raio))
        else:
            lado = rng.choice(("x", "y"))
            sinal = rng.choice((-1.0, 1.0))
            pequeno = lambda: rng.uniform(-0.15, 0.15)  # noqa: E731
            if lado == "x":
                rot = Euler((pequeno(), rng.choice((-1.0, 1.0)) * math.pi / 2 + pequeno(), pequeno()))
                rx, ry, rz = _extensoes(malha, rot)
                vao, largo = vao_x, rx
            else:
                rot = Euler((rng.choice((-1.0, 1.0)) * math.pi / 2 + pequeno(), pequeno(), pequeno()))
                rx, ry, rz = _extensoes(malha, rot)
                vao, largo = vao_y, ry
            if 2 * largo + 2 * FOLGA_ESPUMA > vao:
                f = (vao - 2 * FOLGA_ESPUMA) / (2 * largo)
                _encolher(malha, f)
                raio *= f
                rx, ry, rz = rx * f, ry * f, rz * f
            if lado == "x":
                x = sinal * (ux / 2 + vao_x / 2)
                y = rng.uniform(-iy / 2 + ry, iy / 2 - ry)
            else:
                x = rng.uniform(-ix / 2 + rx, ix / 2 - rx)
                y = sinal * (uy / 2 + vao_y / 2)
            z = rng.uniform(t + 0.25, t + uz - rz)
        obj.location = (x, y, z)
        obj.rotation_euler = rot
        obj["caixa_repouso"] = list(obj.location)
        obj["caixa_rot_repouso"] = list(obj.rotation_euler)
        obj["caixa_raio"] = raio
        obj["caixa_extensoes"] = [rx, ry, rz]
        col.objects.link(obj)
        espumas.append(obj)

    return {
        "corpo": corpo,
        "tampa": tampa,
        "abas": abas,
        "etiqueta": etiqueta,
        "logo": marcador,
        "espumas": espumas,
        "interior": interior,
        "exterior_corpo": (ex, ey, m["zy"]),
        # (x, y, espessura da aba): a 'tampa' agora tem 8 mm de altura
        "exterior_tampa": (ex, ey, t),
        "altura_tampa": t,
        "topo_tampa_z": ez,
        "base_tampa_z": m["zy"],
        "centro_logo": Vector((0.0, 0.0, ez)),
        "centro_logo_local": Vector((0.0, 0.0, 0.0)),
        "normal_logo": Vector((0.0, 0.0, 1.0)),
        "largura_logo": largura_logo if p["com_logo"] else 0.0,
        "com_logo": bool(p["com_logo"]),
        "funil": funil,
        "layout_uv": layout,
        "ppm": ppm,
        "colecao": col,
        "logo_provisoria": provisoria,
        "imagem_logo": imagem,
        "texturas_ausentes": faltam,
        "texturas": nomes,
        "params": p,
    }


# ---------------------------------------------------------------- animacao

def fcurves_de(animation_data):
    """Fcurves da acao de um animation_data, em qualquer Blender 4.2+."""
    try:
        return animation_data.action.fcurves
    except AttributeError:
        slot = animation_data.action_slot
        return animation_data.action.layers[0].strips[0].channelbag(slot).fcurves


def _suavizar(obj, q_ini, q_fim, easing, canais=None, interpolacao="BEZIER"):
    ad = obj.animation_data
    if ad is None or ad.action is None:
        return
    for fc in fcurves_de(ad):
        if canais is not None and fc.data_path not in canais:
            continue
        for kp in fc.keyframe_points:
            if q_ini - 0.5 <= kp.co.x <= q_fim + 0.5:
                kp.interpolation = interpolacao
                kp.easing = easing
                if interpolacao == "BEZIER":
                    kp.handle_left_type = "AUTO_CLAMPED"
                    kp.handle_right_type = "AUTO_CLAMPED"
        fc.update()


def _chave(obj, quadro, loc=None, rot=None, escala=None):
    if loc is not None:
        obj.location = loc
        obj.keyframe_insert("location", frame=quadro)
    if rot is not None:
        obj.rotation_euler = rot
        obj.keyframe_insert("rotation_euler", frame=quadro)
    if escala is not None:
        obj.scale = (escala, escala, escala)
        obj.keyframe_insert("scale", frame=quadro)


def _chave_aba(aba, quadro, graus):
    rot = [0.0, 0.0, 0.0]
    rot[int(aba["caixa_eixo"])] = float(aba["caixa_sinal"]) * math.radians(graus)
    aba.rotation_euler = rot
    aba.keyframe_insert("rotation_euler", frame=quadro)


def animar_tampa(objs, q_ini, q_fim, abrir=True, easing="EASE_IN_OUT", ordem=None, **_ignorados):
    """Abre (ou fecha) as 4 abas. Abrir: grandes de 0 a 70% do intervalo ate
    ~120 graus com leve overshoot; pequenas de 45% a 100% ate ~110 graus.
    Fechar: pequenas primeiro (0-55%), grandes por cima (30-100%), sem
    overshoot (bateriam no topo). 'lado' e outros kw da versao antiga sao
    aceitos e ignorados."""
    p = objs.get("params", PARAMS_PADRAO)
    ordem = ordem or p.get("ordem", "grandes_primeiro")
    sobre = float(p.get("sobrepasso", 0.05))
    grandes = [a for a in objs["abas"] if int(a["caixa_eixo"]) == 0]
    pequenas = [a for a in objs["abas"] if int(a["caixa_eixo"]) == 1]
    primeiras, segundas = (grandes, pequenas) if ordem == "grandes_primeiro" else (pequenas, grandes)
    if not abrir:
        primeiras, segundas = segundas, primeiras
    n = float(q_fim - q_ini)
    janelas = ((primeiras, 0.0, 0.70), (segundas, 0.45, 1.0)) if abrir else ((primeiras, 0.0, 0.55), (segundas, 0.30, 1.0))
    for abas, a, b in janelas:
        q_a = int(round(q_ini + a * n))
        q_b = int(round(q_ini + b * n))
        for aba in abas:
            ang = float(aba["caixa_angulo"])
            if abrir:
                _chave_aba(aba, q_a, 0.0)
                _chave_aba(aba, int(round(q_a + 0.78 * (q_b - q_a))), ang * (1.0 + sobre))
                _chave_aba(aba, q_b, ang)
            else:
                _chave_aba(aba, q_a, ang)
                _chave_aba(aba, q_b, 0.0)
            _suavizar(aba, q_a, q_b, easing, canais=("rotation_euler",))


def _saida_do_retangulo(ini, direcao, rx, ry):
    """Distancia ao longo de 'direcao' (unitaria, XY) de ini ate sair do
    retangulo |x|<rx, |y|<ry."""
    d = float("inf")
    if abs(direcao.x) > 1e-9:
        d = min(d, ((rx if direcao.x > 0 else -rx) - ini.x) / direcao.x)
    if abs(direcao.y) > 1e-9:
        d = min(d, ((ry if direcao.y > 0 else -ry) - ini.y) / direcao.y)
    return max(d, 0.0)


def _trajetoria_espuma(obj, i, semente, objs):
    """Arco balistico de uma espuma: sobe reto pela boca, ganha velocidade
    horizontal, passa POR CIMA das abas abertas e cai ate fora do quadro,
    encolhendo no fim. Deterministico por (semente, i) para a volta refazer o
    mesmo caminho. Devolve (atraso 0..1, duracao 0..1, [(u, loc, rot, escala)])."""
    rng = random.Random(semente * 1000 + i)
    p = objs.get("params", PARAMS_PADRAO)
    ini = Vector(obj["caixa_repouso"])
    rot0 = Vector(obj["caixa_rot_repouso"])
    raio = float(obj["caixa_raio"])
    fx, fy, fz = objs["funil"]
    z_fim = float(p.get("z_fora_do_quadro", -1.3)) - raio

    base = math.atan2(ini.y, ini.x) if ini.xy.length > 0.05 else rng.uniform(0, math.tau)
    ang = base + rng.uniform(-0.9, 0.9)
    direcao = Vector((math.cos(ang), math.sin(ang), 0.0))
    # Ate sair do funil (alcance das abas + raio + folga), medido do repouso.
    D = _saida_do_retangulo(ini, direcao, fx + raio + 0.04, fy + raio + 0.04)
    dist_total = D + rng.uniform(0.5, 1.3)

    g = 3.2                                           # espuma leve: freia no ar
    z_livre = fz + raio + 0.03                        # acima das pontas das abas: pode andar
    apice = z_livre + rng.uniform(0.28, 0.55)
    t_sobe = math.sqrt(2.0 * (apice - ini.z) / g)
    t_total = t_sobe + math.sqrt(2.0 * (apice - z_fim) / g)

    def t_em(z, subindo):
        # instante em que a parabola passa por z (ramo de subida ou descida)
        disc = max(2.0 * (apice - z) / g, 0.0)
        return t_sobe - math.sqrt(disc) if subindo else t_sobe + math.sqrt(disc)

    # O floco sobe RETO ate passar das pontas das abas (as abas abertas
    # inclinam para fora: quem anda de lado antes disso entra nelas por
    # dentro - o teste mediu 48 de 48 batendo na primeira versao) e so entao
    # anda de lado e gira; precisa ter saido do funil quando desce de volta a
    # altura das pontas. Rampa curta no inicio para nao parecer um chute.
    t_s = t_em(z_livre, True)
    t_livre = t_em(z_livre, False)
    rampa = 0.08 * t_total
    v_h = max(D / max(t_livre - t_s - rampa, 1e-3), dist_total / max(t_total - t_s - rampa / 2, 1e-3))
    giro = Vector((rng.uniform(-1, 1), rng.uniform(-1, 1), rng.uniform(-1, 1))) * rng.uniform(4.0, 9.0)
    u_fade = 0.72

    pontos = []
    passos = 20
    for k in range(passos + 1):
        u = k / passos
        t = u * t_total
        if t <= t_sobe:
            z = ini.z + g * t_sobe * t - 0.5 * g * t * t
        else:
            td = t - t_sobe
            z = apice - 0.5 * g * td * td
        dt = t - t_s
        if dt <= 0.0:
            s = 0.0
        elif dt < rampa:
            s = v_h * dt * dt / (2.0 * rampa)
        else:
            s = v_h * (dt - rampa / 2.0)
        s = min(s, dist_total)
        loc = ini + direcao * s
        loc.z = z
        if u <= u_fade:
            escala = 1.0
        else:
            w = (u - u_fade) / (1.0 - u_fade)
            escala = max(0.0, 1.0 - w * w * (3.0 - 2.0 * w))
        pontos.append((u, loc, rot0 + giro * max(0.0, t - t_s), escala))
    atraso = rng.uniform(0.0, 0.35)
    duracao = rng.uniform(0.5, 0.65) * (1.0 - atraso)
    return atraso, duracao, pontos


def animar_espuma(objs, q_ini, q_fim, semente=7, easing="EASE_IN_OUT"):
    """Cada espuma salta em arco por cima das abas, gira e cai ate fora do
    quadro, encolhendo ate sumir; fica la (escala 0) ate q_fim."""
    n = q_fim - q_ini
    for i, obj in enumerate(objs["espumas"]):
        atraso, duracao, pontos = _trajetoria_espuma(obj, i, semente, objs)
        q_a = q_ini + atraso * n
        q_b = q_a + duracao * n
        _chave(obj, q_ini, pontos[0][1], pontos[0][2], pontos[0][3])
        _chave(obj, int(round(q_a)), pontos[0][1], pontos[0][2], pontos[0][3])
        for u, loc, rot, esc in pontos[1:]:
            _chave(obj, int(round(q_a + u * (q_b - q_a))), loc, rot, esc)
        _chave(obj, q_fim, pontos[-1][1], pontos[-1][2], pontos[-1][3])
        _suavizar(obj, q_ini, q_fim, easing, canais=("location", "scale"))
        _suavizar(obj, q_ini, q_fim, "AUTO", canais=("rotation_euler",), interpolacao="LINEAR")
        obj["caixa_pouso"] = list(pontos[-1][1])


def animar_espuma_voltar(objs, q_ini, q_fim, semente=7, easing="EASE_IN_OUT"):
    """Inverso de animar_espuma (beat 6): de fora do quadro de volta ao
    repouso, pelo mesmo arco, crescendo de 0 a 1 ao entrar."""
    n = q_fim - q_ini
    for i, obj in enumerate(objs["espumas"]):
        atraso, duracao, pontos = _trajetoria_espuma(obj, i, semente, objs)
        pontos = pontos[::-1]
        q_a = q_ini + atraso * n
        q_b = q_a + duracao * n
        _chave(obj, q_ini, pontos[0][1], pontos[0][2], pontos[0][3])
        _chave(obj, int(round(q_a)), pontos[0][1], pontos[0][2], pontos[0][3])
        for k, (u, loc, rot, esc) in enumerate(pontos[1:], start=1):
            _chave(obj, int(round(q_a + (k / (len(pontos) - 1)) * (q_b - q_a))), loc, rot, esc)
        _chave(obj, q_fim, pontos[-1][1], pontos[-1][2], pontos[-1][3])
        _suavizar(obj, q_ini, q_fim, easing, canais=("location", "scale"))
        _suavizar(obj, q_ini, q_fim, "AUTO", canais=("rotation_euler",), interpolacao="LINEAR")
