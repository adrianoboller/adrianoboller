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
#   raiz.matrix_world = camera.matrix_world @ Translation(0, 0, -distancia) -
#   sem saber medida nenhuma do bloco. No teste a raiz fica em pe olhando -Y.
#
# - O tamanho das letras NAO e digitado: e MEDIDO. As linhas nascem com os
#   tamanhos relativos de 'tamanhos', o modulo mede a largura de cada uma
#   (bound_box depois do depsgraph) com o tracking FINAL, e escala o conjunto
#   para a linha mais larga caber em 'largura_max' - que por padrao deriva da
#   camera do projeto (35 mm, sensor 36 no lado maior, formato 9:16, a 2 m:
#   1,157 m de largura visivel, 0,9 disso = 1,041 m). Assim trocar a fonte
#   (que muda a largura dos glifos) nao corta nada nas bordas.
#
# - O fade NAO e pela forca da emissao, e pelo ALFA. Emissao com forca 0 e
#   PRETO, nao invisivel: sobre a faixa rose do gradiente as letras entrariam
#   como silhuetas escuras. Cada linha tem Emission (branca, forca 1,0 - nao
#   estoura no AgX) misturada com Transparent por um Value 'alfa', que e o que
#   recebe as chaves; o material e BLENDED.
#
# - Tracking: o 'space_character' do TextCurve e um MULTIPLICADOR do avanco de
#   cada glifo (1,0 = a fonte como desenhada), nao um acrescimo em em. O
#   pedido "tracking 0,25 -> 0,05" e em fracao de em; a conversao e
#   space_character = 1 + tracking / AVANCO_MEDIO, com o avanco medio de uma
#   sans minuscula em ~0,55 em. Fica 1,45 -> 1,09.
#
# - Fonte: nenhuma sans fina de verdade existe aqui (/usr/share/fonts tem
#   DejaVu, Liberation, FreeSans, Loma, IPA, Unifont). FreeSans e o clone
#   metrico da Helvetica - a mais proxima do que a Apple usava antes da SF -
#   e e o padrao no Linux; no Windows do cliente a lista prefere Segoe UI
#   Light / Semibold, que e o "fino" de verdade. Sem nenhuma, fica a Bfont do
#   Blender (mais gorda; funciona). A fonte e embutida (pack) para o .blend
#   nao depender do caminho.
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
    "C:/Windows/Fonts/segoeui.ttf",
    "/usr/share/fonts/truetype/freefont/FreeSans.ttf",
    "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
)
FONTES_FORTES = (
    "C:/Windows/Fonts/seguisb.ttf",            # Segoe UI Semibold (Windows)
    "C:/Windows/Fonts/segoeuib.ttf",
    "/usr/share/fonts/truetype/freefont/FreeSansBold.ttf",
    "/usr/share/fonts/truetype/liberation/LiberationSans-Bold.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
)

PARAMS_PADRAO = {
    "textos": (
        "EnginePrint",
        "qualidade excepcional",
        "13 unidades restantes",
        "compre em engineprint.com.br",
    ),
    # Tamanho relativo (em) de cada linha; o absoluto sai da medicao.
    "tamanhos": (1.0, 0.62, 0.62, 0.42),
    # Distancia entre a linha de base de uma linha e a de baixo, em fracao do
    # tamanho da linha de CIMA - o que mantem a proporcao quando escala.
    "entrelinhas": (1.30, 1.45, 1.80),
    # Peso: linha 1 forte, as outras finas.
    "fortes": (True, False, False, False),
    "cor_texto": "#FFFFFF",
    "cor_destaque": "#C8641F",     # cobre da logo, na linha 3
    "linha_destaque": 3,
    # O AgX escurece e empalidece cor saturada a forca 1,0; o cobre saia
    # marrom. So a linha de destaque sobe um pouco.
    "forca_destaque": 1.6,
    "forca_texto": 1.0,
    # Fonte: caminho explicito ou None (percorre as listas acima).
    "fonte_fina": None,
    "fonte_forte": None,
    "extrusao": 0.0,
    "chanfro": 0.0,                # emissao nao sombreia: chanfro so engorda
    "resolucao": 12,               # curvas dos glifos: de perto, 12 e liso
    # Tracking em fracao de em: assentado e no inicio da entrada. O inicial e
    # um TETO: medido, 0,25 em poe a linha mais larga a 1,22 da largura do
    # quadro (cortada dos dois lados) - por isso cada linha recebe o maior
    # tracking inicial que ainda cabe no quadro (ver construir_cartela).
    "tracking": 0.05,
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
    "fracao_largura": 0.9,
    "distancia": 2.0,
    "lente": 35.0,
    "sensor": 36.0,
    "proporcao": 9.0 / 16.0,
    # Pose padrao da raiz: em pe, olhando -Y (a camera padrao esta em -Y),
    # com o centro do bloco a 1 m do chao. A coreografia sobrescreve.
    "posicao": (0.0, 0.0, 1.0),
    "rotacao": (math.pi / 2.0, 0.0, 0.0),
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
    raiz = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
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
    esp_fim = _tracking_para_espacamento(p["tracking"])

    # 1) linhas em tamanho RELATIVO, para medir.
    linhas = []
    for i, texto in enumerate(p["textos"]):
        fonte = fonte_forte if p["fortes"][i] else fonte_fina
        linhas.append(_linha("%s.linha.%d" % (NOME, i + 1), texto, fonte,
                             p["tamanhos"][i], esp_fim, p, col, raiz))
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
        for l in larguras
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
        "espacamento_final": esp_fim,
        "espacamentos_iniciais": esp_iniciais,
        "fonte_fina": fonte_fina.name if fonte_fina else "Bfont",
        "fonte_forte": fonte_forte.name if fonte_forte else "Bfont",
    }


def posicionar_cartela(objs, camera, distancia=None):
    """Poe a raiz a 'distancia' em frente a camera, de cara para ela, com o
    'para cima' da camera. So mexe na raiz; a camera e apenas lida."""
    if distancia is None:
        distancia = objs["distancia"]
    objs["raiz"].matrix_world = camera.matrix_world @ Matrix.Translation(Vector((0.0, 0.0, -distancia)))


# ---------------------------------------------------------------- animacao

def _no_alfa(obj):
    for mat in obj.data.materials:
        if mat is not None and mat.use_nodes:
            no = mat.node_tree.nodes.get("alfa")
            if no is not None:
                return no.outputs[0], mat.node_tree
    return None, None


def _chave_alfa(obj, quadro, valor):
    saida, _ = _no_alfa(obj)
    if saida is None:
        return
    saida.default_value = valor
    saida.keyframe_insert("default_value", frame=quadro)


def _suavizar(dono, q_ini, q_fim, easing, interpolacao="BEZIER"):
    """Bezier + easing so nas chaves deste intervalo, para nao alterar a
    animacao de outro beat. 'dono' e qualquer ID com animation_data."""
    ad = getattr(dono, "animation_data", None)
    if ad is None or ad.action is None:
        return
    for fc in ad.action.fcurves:
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
                   fracao_elemento=0.55):
    """Entrada escalonada: a logo primeiro, depois as linhas 1..4. Cada
    elemento gasta 'fracao_elemento' do intervalo fazendo fade (alfa 0 -> 1),
    subindo 'subida' m e fechando o tracking; os inicios se distribuem para o
    ultimo terminar exatamente em q_fim - tudo assentado ate la."""
    seq = _elementos(objs)
    n = len(seq)
    total = float(q_fim - q_ini)
    dur = max(1.0, total * fracao_elemento)
    passo = (total - dur) / (n - 1) if n > 1 else 0.0
    esp_fim = objs["espacamento_final"]
    for i, obj in enumerate(seq):
        a = int(round(q_ini + i * passo))
        b = int(round(a + dur)) if i < n - 1 else q_fim
        repouso = _repouso(obj)
        esp_ini = esp_fim
        if obj.type == "FONT":
            esp_ini = objs["espacamentos_iniciais"][objs["linhas"].index(obj)]
        # Invisivel ate a sua vez: chave no quadro anterior ao inicio, para o
        # elemento nao "aparecer" interpolando desde uma chave de outro beat.
        for quadro, alfa, deslocamento, esp in ((a - 1, 0.0, -subida, esp_ini),
                                                (a, 0.0, -subida, esp_ini),
                                                (b, 1.0, 0.0, esp_fim)):
            _chave_alfa(obj, quadro, alfa)
            obj.location = repouso + Vector((0.0, deslocamento, 0.0))
            obj.keyframe_insert("location", frame=quadro)
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
