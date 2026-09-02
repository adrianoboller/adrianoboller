# Modulo CARTELA do anuncio do Snapmaker U1.
#
# Quatro linhas de texto, estilo Apple (sans fina, tracking largo, branca,
# centrada, hierarquia clara) e, acima delas, a logo da EnginePrint num plano
# com alfa. So definicoes aqui - nada roda no import. Quem integra e
# mod_coreografia.py; quem prova este modulo sozinho e teste_cartela.py.
#
# DECISOES:
#
# - O Empty 'cartela.raiz' segue a CONVENCAO DE CAMERA: o texto vive no plano
#   XY local, le-se ao longo de +X, sobe em +Y e o normal (+Z) aponta para
#   quem olha. Assim a coreografia poe a cartela em frente a camera com uma
#   linha - posicionar_cartela(objs, camera, distancia) faz
#   raiz.matrix_world = camera.matrix_world @ Translation(0, subida, -distancia) -
#   sem saber medida nenhuma do bloco. Com parentear=True a raiz vira filha da
#   camera e acompanha qualquer movimento dela sem chave nenhuma (no beat 7 a
#   camera continua derivando depois do corte). No teste a raiz fica em pe
#   olhando -Y.
#
# - 'subida' (0,18 m a 2 m = 8,7% da altura do quadro) sobe o bloco: a linha 4,
#   a que vende, saia a 78% da altura (medido na rodada 1), em cima da
#   transicao escuro -> rose do gradiente e dentro da faixa de legendas do
#   Reels. Era 0,13 na coreografia; a revisao mediu que nao bastava.
#
# - O tamanho das letras NAO e digitado: e MEDIDO. As linhas nascem com os
#   tamanhos relativos de 'tamanhos', o modulo mede a largura de cada uma
#   (bound_box depois do depsgraph) com o tracking FINAL, e escala o conjunto
#   para a linha mais larga caber em 'largura_max' - que por padrao deriva da
#   camera do projeto (35 mm, sensor 36 no lado maior, formato 9:16, a 2 m:
#   1,157 m de largura visivel, 0,83 disso = 0,960 m; era 0,9 e o bloco ficou
#   8% menor a pedido da revisao, para caber com folga). Assim trocar a fonte
#   (que muda a largura dos glifos) nao corta nada nas bordas.
#
# - Hierarquia pelo TAMANHO, nao pelo peso: a marca sai na fonte regular/light
#   a 1,3x (era bold a 1,0x - a revisao mediu FreeSans Bold como "pesada, anos
#   90"; a Apple faz hierarquia com tamanho e no maximo semibold). O slot
#   'fonte_forte' continua existindo para o cliente trocar a fonte da marca;
#   a lista dele agora prefere Segoe UI Light / Semilight antes de Semibold, e
#   no Linux cai em FreeSans regular (nunca mais em Bold).
#
# - O fade NAO e pela forca da emissao, e pelo ALFA. Emissao com forca 0 e
#   PRETO, nao invisivel: sobre a faixa rose do gradiente as letras entrariam
#   como silhuetas escuras. Cada linha tem Emission misturada com Transparent
#   por um Value 'alfa', que e o que recebe as chaves; o material e BLENDED.
#
# - Forca da emissao MEDIDA no render, nao chutada: com 1,0 o "branco" saia a
#   0,82 sRGB (209/255) sob AgX Medium High Contrast - cinza. A revisao propos
#   1,8 "para chegar a 0,95"; medido (varredura, mesmo AgX), o branco satura
#   devagar: 1,8 -> 227, 2,2 -> 232, 2,6 -> 236, 3,0 -> 239, 4,0 -> 245. Os
#   0,95 sRGB (242) exigiriam ~3,7, acima do limiar de bloom (2,5) do
#   mod_ambiente - o texto floresceria. Fica 2,4 (0,92 sRGB, #EBEBEB, o
#   maior valor sem bloom); mais branco que isso e decisao do look, nao da
#   forca. O cobre da linha 3 vai a 2,0: o AgX empalidece cor saturada quando
#   a forca sobe (B medido 55/65/73/87 para 2,2/2,4/2,6/3,0 a 270 px, e a 540 px
#   a mediana encosta no maximo: 78 com 2,2), e o criterio da revisao e
#   R >= 180 e B <= 80 no pixel do render - com 2,0 mede R 229 / B 73. O
#   teste_cartela imprime os dois numeros.
#
# - Tracking: o 'space_character' do TextCurve e um MULTIPLICADOR do avanco de
#   cada glifo (1,0 = a fonte como desenhada), nao um acrescimo em em. O pedido
#   e em fracao de em; a conversao e space_character = 1 + tracking /
#   AVANCO_MEDIO, com o avanco medio de uma sans minuscula em ~0,55 em.
#   Tracking final POR LINHA: 0,08 em na marca (1,145) e 0,12 em nas outras
#   (1,218) - a revisao mediu os 0,05 de antes como "tracking praticamente
#   normal", e uma marca a 1,3x com o mesmo tracking das linhas pequenas
#   ficaria esparramada demais.
#
# - Match cut (beat 7): animar_cartela(..., logo_ja_visivel=True) poe a logo
#   com alfa 1 JA em q_ini (chave CONSTANT em q_ini-1, para o motion blur nao
#   vazar meio fade para o quadro anterior ao corte) e so as linhas entram
#   escalonadas depois - o corte 'logo da tampa -> logo da cartela' precisa
#   que a logo nao pisque. 'logo_origem' e 'logo_escala_inicial' (opcionais)
#   deixam a logo nascer maior e no centro e viajar ate o repouso enquanto o
#   texto entra, para casar o tamanho aparente com o ultimo quadro do mergulho.
#
# - Fonte: nenhuma sans fina de verdade existe aqui (/usr/share/fonts tem
#   DejaVu, Liberation, FreeSans, Loma, IPA, Unifont). FreeSans e o clone
#   metrico da Helvetica - a mais proxima do que a Apple usava antes da SF -
#   e e o padrao no Linux; no Windows do cliente as listas preferem Segoe UI
#   Light / Semilight, que e o "fino" de verdade. Sem nenhuma, fica a Bfont do
#   Blender (mais gorda; funciona). Fonte e logo sao embutidas (pack) para o
#   .blend nao depender do caminho (a revisao mediu as imagens apontando para
#   a pasta temporaria).
#
# Eixos e medidas seguem docs/ESPECIFICACAO.md: metros, Z para cima, frente
# em -Y.

import math
import os

import bpy
from mathutils import Matrix, Vector

NOME = "cartela"

# Avanco medio de um glifo minusculo de sans em fracao de em: e o que converte
# tracking (fracao de em) em space_character (multiplicador). Aproximacao;
# o que importa e a largura final, e essa e medida.
AVANCO_MEDIO = 0.55

FONTES_FINAS = (
    "C:/Windows/Fonts/segoeuil.ttf",           # Segoe UI Light (Windows)
    "C:/Windows/Fonts/segoeuisl.ttf",          # Segoe UI Semilight
    "C:/Windows/Fonts/segoeui.ttf",
    "/usr/share/fonts/truetype/freefont/FreeSans.ttf",
    "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
)
# A fonte da MARCA. Light/Semilight vem antes do Semibold de proposito: a
# hierarquia vem do tamanho (1,3x), e um bold cheio pesa demais (medido na
# rodada 1). Nenhuma Bold na lista - no Linux o fallback e o FreeSans regular.
FONTES_FORTES = (
    "C:/Windows/Fonts/segoeuil.ttf",           # Segoe UI Light (Windows)
    "C:/Windows/Fonts/segoeuisl.ttf",          # Segoe UI Semilight
    "C:/Windows/Fonts/seguisb.ttf",            # Segoe UI Semibold
    "C:/Windows/Fonts/segoeui.ttf",
    "/usr/share/fonts/truetype/freefont/FreeSans.ttf",
    "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
)

PARAMS_PADRAO = {
    "textos": (
        "EnginePrint",
        "qualidade excepcional",
        "13 unidades restantes",
        "compre em engineprint.com.br",
    ),
    # Tamanho relativo (em) de cada linha; o absoluto sai da medicao. A marca
    # a 1,3x na fonte regular substitui o bold (hierarquia por tamanho); como
    # ela passou a ser a linha mais larga e dita a escala, a linha 4 subiu de
    # 0,42 para 0,46 para nao cair abaixo de ~11 px de x-altura num celular
    # de 640 px (medido: 10 px com 0,42).
    "tamanhos": (1.3, 0.62, 0.62, 0.46),
    # Distancia entre a linha de base de uma linha e a de baixo, em fracao do
    # tamanho da linha de CIMA - o que mantem a proporcao quando escala.
    "entrelinhas": (1.30, 1.45, 1.80),
    # Qual linha usa o slot 'fonte_forte' (a marca). Com as listas padrao ele
    # resolve para a mesma fonte regular/light; o slot existe para o cliente
    # dar outra fonte so para a marca.
    "fortes": (True, False, False, False),
    "cor_texto": "#FFFFFF",
    "cor_destaque": "#C8641F",     # cobre da logo, na linha 3
    "linha_destaque": 3,           # None = tudo branco
    # Forcas MEDIDAS no render (ver cabecalho): 2,4 e o branco mais branco
    # sem bloom (0,92 sRGB); o AgX empalidece cor saturada quando a forca
    # sobe, entao o cobre fica em 2,0 (R 229 / B 73, dentro de R >= 180 e
    # B <= 80). Quem passar mais que 2,2 no destaque perde o cobre.
    "forca_destaque": 2.0,
    "forca_texto": 2.4,
    # Fonte: caminho explicito ou None (percorre as listas acima).
    "fonte_fina": None,
    "fonte_forte": None,
    "extrusao": 0.0,
    "chanfro": 0.0,                # emissao nao sombreia: chanfro so engorda
    "resolucao": 12,               # curvas dos glifos: de perto, 12 e liso
    # Tracking em fracao de em: assentado (um numero, ou um por linha: a marca
    # grande pede menos que as linhas pequenas) e no inicio da entrada. O
    # inicial e um TETO: medido, 0,25 em poe a linha mais larga a 1,22 da
    # largura do quadro (cortada dos dois lados) - por isso cada linha recebe
    # o maior tracking inicial que ainda cabe no quadro (ver construir_cartela).
    "tracking": (0.08, 0.12, 0.12, 0.12),
    "tracking_inicial": 0.25,
    # Folga da borda ao limitar o tracking inicial: a largura cresce um pouco
    # mais que linearmente com o space_character (medido 1,356x para 1,33x).
    "folga_borda": 0.94,
    "com_logo": True,
    "logo": "logo_engineprint.png",   # relativo a assets/; ou caminho absoluto
    "largura_logo": 0.30,             # do plano, em fracao da largura do bloco
    "espaco_logo": 0.35,              # entre a base da logo e o topo da linha 1, em fracao do tamanho da linha 1
    # Camera do projeto, so para derivar largura_max (None = derivar).
    "largura_max": None,
    "fracao_largura": 0.83,        # era 0,9; bloco 8% menor (revisao)
    "distancia": 2.0,
    "lente": 35.0,
    "sensor": 36.0,
    "proporcao": 9.0 / 16.0,
    # Pose padrao da raiz: em pe, olhando -Y (a camera padrao esta em -Y),
    # com o centro do bloco a 1 m do chao. A coreografia sobrescreve.
    "posicao": (0.0, 0.0, 1.0),
    "rotacao": (math.pi / 2.0, 0.0, 0.0),
    # Quanto posicionar_cartela sobe o bloco no quadro (m, no 'para cima' da
    # camera): a 2 m, 0,18 m = 8,7% da altura - tira a linha 4 da transicao
    # escuro -> rose e da faixa de legendas do Reels.
    "subida": 0.18,
}

FPS = 30.0


# ---------------------------------------------------------------- utilidades

def _srgb_para_linear(c):
    c = c / 255.0
    return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4


def cor_linear(hexa):
    """'#RRGGBB' -> (r, g, b, 1) linear, que e o que os nos de shader querem."""
    h = hexa.lstrip("#")
    return tuple(_srgb_para_linear(int(h[i:i + 2], 16)) for i in (0, 2, 4)) + (1.0,)


def limpar_colecao(nome):
    """Remove a sub-colecao <nome> e tudo o que ela contem (idempotencia)."""
    col = bpy.data.collections.get(nome)
    if col is not None:
        for obj in list(col.all_objects):
            dados = obj.data
            bpy.data.objects.remove(obj, do_unlink=True)
            # Curva de texto orfa acumularia 'cartela.linha.1.001' a cada
            # rodada; some junto com o objeto.
            if dados is not None and dados.users == 0:
                if isinstance(dados, bpy.types.Curve):
                    bpy.data.curves.remove(dados)
                elif isinstance(dados, bpy.types.Mesh):
                    bpy.data.meshes.remove(dados)
        for filha in list(col.children):
            limpar_colecao(filha.name)
        bpy.data.collections.remove(col)
    # Materiais do modulo sem dono tambem, pelo mesmo motivo.
    for mat in list(bpy.data.materials):
        if mat.name.startswith(nome + ".") and mat.users == 0:
            bpy.data.materials.remove(mat)


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
    if os.path.isabs(nome_arquivo):
        return nome_arquivo
    # scripts/ e assets/ sao irmaos na raiz do projeto.
    try:
        raiz = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    except NameError:
        # Aba Scripting do Blender sem arquivo (colado direto): nao existe
        # __file__; vale a pasta do .blend, ou a de trabalho se nem ele ha.
        raiz = os.path.dirname(bpy.data.filepath) if bpy.data.filepath else os.getcwd()
    return os.path.join(raiz, "assets", nome_arquivo)


def _carregar_fonte(explicita, candidatas):
    """Primeira fonte que existir; None = Bfont (a do Blender)."""
    caminhos = ([explicita] if explicita else []) + list(candidatas)
    for caminho in caminhos:
        if caminho and os.path.exists(caminho):
            fonte = bpy.data.fonts.load(caminho, check_existing=True)
            try:
                # Embutida: o .blend do cliente nao depende deste caminho.
                if not fonte.packed_file:
                    fonte.pack()
            except RuntimeError:
                pass
            return fonte
    return None


def _tracking_para_espacamento(tracking):
    return 1.0 + tracking / AVANCO_MEDIO


def _por_linha(valor, n):
    """Um numero vale para todas as linhas; uma sequencia e por linha (a
    ultima repete se faltar)."""
    try:
        seq = list(valor)
    except TypeError:
        return [valor] * n
    if not seq:
        return [0.0] * n
    return [seq[i] if i < len(seq) else seq[-1] for i in range(n)]


def _largura_visivel(distancia, lente, sensor, proporcao):
    """Largura do quadro 9:16 a 'distancia' da camera: o sensor de 36 mm vai
    para o lado MAIOR (a altura), e a largura e a altura vezes 9/16."""
    altura = distancia * sensor / lente
    return altura * proporcao


# ---------------------------------------------------------------- materiais

def _material_texto(nome, cor_hex, forca):
    """Emission x Transparent misturados por um Value 'alfa' (0 = invisivel).
    E o alfa que recebe as chaves do fade - ver cabecalho."""
    mat = bpy.data.materials.new(nome)
    mat.use_nodes = True
    nt = mat.node_tree
    for n in list(nt.nodes):
        nt.nodes.remove(n)
    saida = nt.nodes.new("ShaderNodeOutputMaterial")
    saida.location = (400, 0)
    mistura = nt.nodes.new("ShaderNodeMixShader")
    mistura.location = (200, 0)
    transp = nt.nodes.new("ShaderNodeBsdfTransparent")
    transp.location = (0, 80)
    emis = nt.nodes.new("ShaderNodeEmission")
    emis.location = (0, -80)
    emis.inputs["Color"].default_value = cor_linear(cor_hex)
    emis.inputs["Strength"].default_value = forca
    alfa = nt.nodes.new("ShaderNodeValue")
    alfa.name = "alfa"
    alfa.label = "alfa"
    alfa.location = (0, 220)
    alfa.outputs[0].default_value = 1.0
    nt.links.new(alfa.outputs[0], mistura.inputs["Fac"])
    nt.links.new(transp.outputs[0], mistura.inputs[1])
    nt.links.new(emis.outputs[0], mistura.inputs[2])
    nt.links.new(mistura.outputs[0], saida.inputs["Surface"])
    _transparente(mat)
    return mat


def _material_logo(nome, imagem):
    """Cor da imagem em Emission (nao depende de luz), alfa da imagem vezes o
    Value 'alfa' do fade."""
    mat = bpy.data.materials.new(nome)
    mat.use_nodes = True
    nt = mat.node_tree
    for n in list(nt.nodes):
        nt.nodes.remove(n)
    saida = nt.nodes.new("ShaderNodeOutputMaterial")
    saida.location = (600, 0)
    mistura = nt.nodes.new("ShaderNodeMixShader")
    mistura.location = (400, 0)
    transp = nt.nodes.new("ShaderNodeBsdfTransparent")
    transp.location = (200, 80)
    emis = nt.nodes.new("ShaderNodeEmission")
    emis.location = (200, -80)
    emis.inputs["Strength"].default_value = 1.0
    tex = nt.nodes.new("ShaderNodeTexImage")
    tex.location = (-200, 0)
    tex.image = imagem
    tex.interpolation = "Cubic"
    tex.extension = "CLIP"
    alfa = nt.nodes.new("ShaderNodeValue")
    alfa.name = "alfa"
    alfa.label = "alfa"
    alfa.location = (-200, 260)
    alfa.outputs[0].default_value = 1.0
    produto = nt.nodes.new("ShaderNodeMath")
    produto.operation = "MULTIPLY"
    produto.location = (100, 260)
    nt.links.new(tex.outputs["Alpha"], produto.inputs[0])
    nt.links.new(alfa.outputs[0], produto.inputs[1])
    nt.links.new(tex.outputs["Color"], emis.inputs["Color"])
    nt.links.new(produto.outputs[0], mistura.inputs["Fac"])
    nt.links.new(transp.outputs[0], mistura.inputs[1])
    nt.links.new(emis.outputs[0], mistura.inputs[2])
    nt.links.new(mistura.outputs[0], saida.inputs["Surface"])
    _transparente(mat)
    return mat


def _transparente(mat):
    try:
        mat.surface_render_method = "BLENDED"   # 4.2+
    except AttributeError:
        mat.blend_method = "BLEND"              # 4.1 e antes
    try:
        mat.show_transparent_back = False
    except AttributeError:
        pass
    try:
        mat.shadow_method = "NONE"              # sumiu no 4.2; existe antes
    except AttributeError:
        pass


def _carregar_logo(caminho):
    if os.path.exists(caminho):
        img = bpy.data.images.load(caminho, check_existing=True)
        try:
            # Embutida: o .blend gravado no Windows do cliente apontava para
            # %TEMP% (medido na revisao); sem o pack, a logo vira rosa.
            if not img.packed_file:
                img.pack()
        except RuntimeError:
            pass
        return img, False
    # Sem o PNG, um quadrado cobre com furo, para a cartela nao ficar sem nada.
    import numpy as np
    img = bpy.data.images.new("cartela.logo_provisoria", 256, 256, alpha=True)
    px = np.zeros((256, 256, 4), dtype=np.float32)
    px[32:224, 32:224] = (0.58, 0.13, 0.014, 1.0)
    px[96:160, 96:160] = (0.0, 0.0, 0.0, 0.0)
    img.pixels.foreach_set(px.ravel())
    img.pack()
    return img, True


def _medir_conteudo(img):
    """Fracao (largura, altura) da imagem ocupada pelos pixels opacos e o
    centro deles, para o plano da logo ser dimensionado pelo DESENHO e nao
    pela margem transparente do PNG."""
    import numpy as np
    w, h = img.size
    if w == 0 or h == 0:
        return (1.0, 1.0), (0.5, 0.5)
    px = np.empty(w * h * 4, dtype=np.float32)
    img.pixels.foreach_get(px)
    alfa = px.reshape(h, w, 4)[:, :, 3]
    ys, xs = np.where(alfa > 0.5)
    if len(xs) == 0:
        return (1.0, 1.0), (0.5, 0.5)
    x0, x1, y0, y1 = xs.min(), xs.max() + 1, ys.min(), ys.max() + 1
    return ((x1 - x0) / w, (y1 - y0) / h), ((x0 + x1) / (2.0 * w), (y0 + y1) / (2.0 * h))


# ---------------------------------------------------------------- construir

def _linha(nome, texto, fonte, tamanho, espacamento, p, col, raiz):
    curva = bpy.data.curves.new(nome, "FONT")
    curva.body = texto
    if fonte is not None:
        curva.font = fonte
    curva.size = tamanho
    curva.space_character = espacamento
    curva.align_x = "CENTER"
    # Origem na linha de base: a posicao Y do objeto E a linha de base, e a
    # subida de 3 cm da entrada nao depende da altura dos glifos.
    curva.align_y = "TOP_BASELINE"
    curva.extrude = p["extrusao"]
    curva.bevel_depth = p["chanfro"]
    curva.bevel_resolution = 1
    curva.resolution_u = p["resolucao"]
    curva.fill_mode = "BOTH"
    obj = bpy.data.objects.new(nome, curva)
    col.objects.link(obj)
    obj.parent = raiz
    return obj


def _largura(obj):
    """Largura em X do objeto avaliado, no espaco local (o texto e plano)."""
    xs = [v[0] for v in obj.bound_box]
    return max(xs) - min(xs)


def construir_cartela(cena, colecao_pai=None, params=None):
    """Cria 'cartela.raiz' (Empty, convencao de camera: +Z para quem olha, +Y
    para cima), 'cartela.linha.1..4' (TEXT, emissao) e 'cartela.logo' (plano
    com alfa) na sub-colecao 'cartela'. Devolve os objetos e as medidas."""
    p = dict(PARAMS_PADRAO)
    if params:
        p.update(params)
    limpar_colecao(NOME)
    col = _colecao(cena, colecao_pai, NOME)

    largura_max = p["largura_max"]
    if largura_max is None:
        largura_max = p["fracao_largura"] * _largura_visivel(
            p["distancia"], p["lente"], p["sensor"], p["proporcao"])

    raiz = bpy.data.objects.new(NOME + ".raiz", None)
    raiz.empty_display_type = "PLAIN_AXES"
    raiz.empty_display_size = 0.1
    raiz.location = p["posicao"]
    raiz.rotation_euler = p["rotacao"]
    col.objects.link(raiz)

    fonte_fina = _carregar_fonte(p["fonte_fina"], FONTES_FINAS)
    fonte_forte = _carregar_fonte(p["fonte_forte"], FONTES_FORTES)
    n = len(p["textos"])
    esp_finais = [_tracking_para_espacamento(t) for t in _por_linha(p["tracking"], n)]

    # 1) linhas em tamanho RELATIVO, para medir.
    linhas = []
    for i, texto in enumerate(p["textos"]):
        fonte = fonte_forte if p["fortes"][i] else fonte_fina
        linhas.append(_linha("%s.linha.%d" % (NOME, i + 1), texto, fonte,
                             p["tamanhos"][i], esp_finais[i], p, col, raiz))
    bpy.context.view_layer.update()
    larguras_rel = [_largura(o) for o in linhas]
    mais_larga = max(larguras_rel)
    # 2) escala unica que poe a mais larga em largura_max.
    escala = largura_max / mais_larga if mais_larga > 0 else 1.0
    tamanhos = [t * escala for t in p["tamanhos"]]
    for obj, tam in zip(linhas, tamanhos):
        obj.data.size = tam

    # 3) empilhar: linha 1 no topo, descendo pelas entrelinhas; depois centrar
    # o conjunto (logo incluida) na origem da raiz.
    bases = [0.0]
    for i in range(1, len(linhas)):
        bases.append(bases[-1] - p["entrelinhas"][i - 1] * tamanhos[i - 1])
    # Altura de caixa alta ~0,72 em: topo do bloco de texto acima da base 1.
    topo_texto = bases[0] + 0.72 * tamanhos[0]
    # Descendente da ultima linha (~0,22 em) e o fundo do bloco.
    fundo = bases[-1] - 0.22 * tamanhos[-1]

    logo = None
    imagem = None
    altura_logo = 0.0
    if p["com_logo"]:
        imagem, provisoria = _carregar_logo(_caminho_asset(p["logo"]))
        (fx, fy), (cx, cy) = _medir_conteudo(imagem)
        largura_desenho = p["largura_logo"] * largura_max
        lado = largura_desenho / max(fx, 1e-6)      # plano inteiro (com margem)
        altura_desenho = lado * fy
        altura_logo = altura_desenho + p["espaco_logo"] * tamanhos[0]
        malha = bpy.data.meshes.new(NOME + ".logo")
        h = lado / 2.0
        malha.from_pydata([(-h, -h, 0), (h, -h, 0), (h, h, 0), (-h, h, 0)], [], [(0, 1, 2, 3)])
        uv = malha.uv_layers.new(name="UVMap")
        for l, co in zip(uv.data, ((0, 0), (1, 0), (1, 1), (0, 1))):
            l.uv = co
        malha.update()
        malha.materials.append(_material_logo(NOME + ".logo", imagem))
        logo = bpy.data.objects.new(NOME + ".logo", malha)
        col.objects.link(logo)
        logo.parent = raiz
        # Centro do DESENHO (nao do PNG) fica em cima da linha 1 com a folga.
        # Pixel (0,0) do Blender e o canto inferior esquerdo; cy cresce p/ cima.
        centro_y = topo_texto + p["espaco_logo"] * tamanhos[0] + altura_desenho / 2.0
        logo.location = (-(cx - 0.5) * lado, centro_y - (cy - 0.5) * lado, 0.0)

    topo = topo_texto + altura_logo
    deslocamento = -(topo + fundo) / 2.0     # centra o bloco em Y = 0
    for obj, base in zip(linhas, bases):
        obj.location = (0.0, base + deslocamento, 0.0)
    if logo is not None:
        logo.location.y += deslocamento

    # materiais
    for i, obj in enumerate(linhas):
        destaque = (i + 1) == p["linha_destaque"]
        mat = _material_texto(
            "%s.linha.%d" % (NOME, i + 1),
            p["cor_destaque"] if destaque else p["cor_texto"],
            p["forca_destaque"] if destaque else p["forca_texto"])
        obj.data.materials.append(mat)

    bpy.context.view_layer.update()
    larguras = [_largura(o) for o in linhas]
    # Tracking inicial por linha: o pedido, limitado pelo que cabe no quadro.
    # A largura e ~proporcional ao space_character, entao a linha de largura
    # L (assentada, esp_fim) chega a borda quando esp = esp_fim * quadro / L.
    largura_quadro = largura_max / p["fracao_largura"]
    esp_ini_pedido = _tracking_para_espacamento(p["tracking_inicial"])
    esp_iniciais = [
        min(esp_ini_pedido, esp_fim * p["folga_borda"] * largura_quadro / max(l, 1e-6))
        for l, esp_fim in zip(larguras, esp_finais)
    ]
    return {
        "raiz": raiz,
        "linhas": linhas,
        "logo": logo,
        "imagem_logo": imagem,
        "tamanhos": tamanhos,
        "larguras": larguras,
        "largura": max(larguras),
        "altura": topo - fundo,
        "largura_max": largura_max,
        "distancia": p["distancia"],
        "subida": p["subida"],
        # Compatibilidade: um so valor (o mais largo); por linha, ver abaixo.
        "espacamento_final": max(esp_finais),
        "espacamentos_finais": esp_finais,
        "espacamentos_iniciais": esp_iniciais,
        "fonte_fina": fonte_fina.name if fonte_fina else "Bfont",
        "fonte_forte": fonte_forte.name if fonte_forte else "Bfont",
    }


def posicionar_cartela(objs, camera, distancia=None, subida=None, parentear=False):
    """Poe a raiz a 'distancia' em frente a camera, de cara para ela, com o
    'para cima' da camera, subida 'subida' m no quadro (None = o param do
    construtor). So mexe na raiz; a camera e apenas lida.

    parentear=True faz a raiz filha da camera (matrix_parent_inverse
    identidade, pose local = a translacao): a cartela acompanha qualquer
    movimento posterior da camera sem chave. A camera do projeto tem DoF com
    foco no 'camera.alvo'; a coreografia precisa levar o foco ate a raiz (ou
    keyar focus_distance = distancia) no beat 7, senao a cartela sai borrada -
    o teste_cartela prova a nitidez com DoF f/2,8 focado na raiz."""
    if distancia is None:
        distancia = objs["distancia"]
    if subida is None:
        subida = objs.get("subida", 0.0)
    raiz = objs["raiz"]
    local = Matrix.Translation(Vector((0.0, subida, -distancia)))
    if parentear:
        raiz.parent = camera
        raiz.matrix_parent_inverse = Matrix.Identity(4)
        raiz.matrix_basis = local
    else:
        raiz.parent = None
        raiz.matrix_world = camera.matrix_world @ local


# ---------------------------------------------------------------- animacao

def _no_alfa(obj):
    for mat in obj.data.materials:
        if mat is not None and mat.use_nodes:
            no = mat.node_tree.nodes.get("alfa")
            if no is not None:
                return no.outputs[0], mat.node_tree
    return None, None


def fcurves_de(animation_data):
    """Fcurves da acao de um animation_data, em qualquer Blender 4.2+."""
    # Action.fcurves virou legado no 4.4 (slotted actions); no 5.0 pode nao existir.
    try:
        return animation_data.action.fcurves
    except AttributeError:
        slot = animation_data.action_slot
        return animation_data.action.layers[0].strips[0].channelbag(slot).fcurves


def _chave_alfa(obj, quadro, valor, interpolacao=None):
    saida, nt = _no_alfa(obj)
    if saida is None:
        return
    saida.default_value = valor
    saida.keyframe_insert("default_value", frame=quadro)
    if interpolacao is None:
        return
    # So esta chave: e a que segura o valor ate o quadro seguinte (CONSTANT),
    # para o motion blur nao expor meio fade no quadro anterior ao corte.
    ad = nt.animation_data
    if ad is None or ad.action is None:
        return
    for fc in fcurves_de(ad):
        for kp in fc.keyframe_points:
            if abs(kp.co.x - quadro) < 0.5:
                kp.interpolation = interpolacao
        fc.update()


def _suavizar(dono, q_ini, q_fim, easing, interpolacao="BEZIER"):
    """Bezier + easing so nas chaves deste intervalo, para nao alterar a
    animacao de outro beat. 'dono' e qualquer ID com animation_data."""
    ad = getattr(dono, "animation_data", None)
    if ad is None or ad.action is None:
        return
    for fc in fcurves_de(ad):
        for kp in fc.keyframe_points:
            if q_ini - 0.5 <= kp.co.x <= q_fim + 0.5:
                kp.interpolation = interpolacao
                kp.easing = easing
                if interpolacao == "BEZIER":
                    kp.handle_left_type = "AUTO_CLAMPED"
                    kp.handle_right_type = "AUTO_CLAMPED"
        fc.update()


def _elementos(objs):
    """Ordem de entrada: a logo primeiro, depois as linhas de cima p/ baixo."""
    seq = []
    if objs.get("logo") is not None:
        seq.append(objs["logo"])
    seq.extend(objs["linhas"])
    return seq


def _repouso(obj):
    """Posicao assentada do elemento: guardada no proprio objeto na primeira
    animacao, para a saida e uma segunda entrada nao acumularem deslocamento."""
    if "cartela_repouso" not in obj:
        obj["cartela_repouso"] = list(obj.location)
    return Vector(obj["cartela_repouso"])


def animar_cartela(objs, q_ini, q_fim, easing="EASE_IN_OUT", subida=0.03,
                   fracao_elemento=0.55, logo_ja_visivel=False,
                   logo_origem=None, logo_escala_inicial=1.0):
    """Entrada escalonada: a logo primeiro, depois as linhas 1..4. Cada
    elemento gasta 'fracao_elemento' do intervalo fazendo fade (alfa 0 -> 1),
    subindo 'subida' m e fechando o tracking; os inicios se distribuem para o
    ultimo terminar exatamente em q_fim - tudo assentado ate la.

    logo_ja_visivel=True (match cut): a logo esta com alfa 1 JA em q_ini
    (0 em q_ini-1, chave CONSTANT), sem fade nem subida; as linhas entram
    escalonadas no mesmo calendario de sempre. Opcionalmente a logo nasce em
    'logo_origem' (x, y locais da raiz; (0, 0) = centro do quadro) com escala
    'logo_escala_inicial' e viaja ate o repouso durante a sua fatia do
    intervalo - e o que casa o tamanho aparente com a logo da tampa."""
    seq = _elementos(objs)
    n = len(seq)
    total = float(q_fim - q_ini)
    dur = max(1.0, total * fracao_elemento)
    passo = (total - dur) / (n - 1) if n > 1 else 0.0
    esp_finais = objs.get("espacamentos_finais") or [objs["espacamento_final"]] * len(objs["linhas"])
    for i, obj in enumerate(seq):
        a = int(round(q_ini + i * passo))
        b = int(round(a + dur)) if i < n - 1 else q_fim
        repouso = _repouso(obj)
        esp_ini = esp_fim = None
        if obj.type == "FONT":
            k = objs["linhas"].index(obj)
            esp_ini, esp_fim = objs["espacamentos_iniciais"][k], esp_finais[k]
        ja_visivel = logo_ja_visivel and obj is objs.get("logo")
        if ja_visivel:
            origem = repouso if logo_origem is None else Vector((logo_origem[0], logo_origem[1], 0.0))
            escala_ini = float(logo_escala_inicial)
        else:
            origem = repouso + Vector((0.0, -subida, 0.0))
            escala_ini = 1.0
        # Invisivel ate a sua vez: chave no quadro anterior ao inicio, para o
        # elemento nao "aparecer" interpolando desde uma chave de outro beat.
        # Com a logo ja visivel, o 0 -> 1 fica entre a-1 e a como degrau.
        chaves = ((a - 1, 0.0, origem, esp_ini, escala_ini, "CONSTANT" if ja_visivel else None),
                  (a, 1.0 if ja_visivel else 0.0, origem, esp_ini, escala_ini, None),
                  (b, 1.0, repouso, esp_fim, 1.0, None))
        for quadro, alfa, pos, esp, escala, interp in chaves:
            _chave_alfa(obj, quadro, alfa, interp)
            obj.location = pos
            obj.keyframe_insert("location", frame=quadro)
            if escala_ini != 1.0:
                obj.scale = (escala, escala, escala)
                obj.keyframe_insert("scale", frame=quadro)
            if obj.type == "FONT":
                obj.data.space_character = esp
                obj.data.keyframe_insert("space_character", frame=quadro)
        _suavizar(obj, a, b, easing)
        _suavizar(obj.data, a, b, easing)
        _, nt = _no_alfa(obj)
        _suavizar(nt, a, b, easing)


def animar_cartela_saida(objs, q_ini, q_fim, easing="EASE_IN_OUT", deslocamento=0.02):
    """Fade out de tudo junto (alfa 1 -> 0), com uma leve continuacao da
    subida para nao parecer que a cartela 'apagou'."""
    for obj in _elementos(objs):
        repouso = _repouso(obj)
        for quadro, alfa, dy in ((q_ini, 1.0, 0.0), (q_fim, 0.0, deslocamento)):
            _chave_alfa(obj, quadro, alfa)
            obj.location = repouso + Vector((0.0, dy, 0.0))
            obj.keyframe_insert("location", frame=quadro)
        _suavizar(obj, q_ini, q_fim, easing)
        _, nt = _no_alfa(obj)
        _suavizar(nt, q_ini, q_fim, easing)
