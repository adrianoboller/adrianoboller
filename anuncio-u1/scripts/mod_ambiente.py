# Modulo AMBIENTE do anuncio do Snapmaker U1.
#
# Entrega o "estudio": fundo infinito no WORLD (gradiente preto -> rose-branco
# MESCLADO por manchas de ruido), SEM CHAO, quatro luzes de estudio (key,
# fill, rim, top), a camera principal com profundidade de campo, a
# configuracao de render (EEVEE Next, AgX, motion blur, raytracing, bloom no
# compositor), o FLASH de foto do beat 5 e o obturador por chave (whoosh).
# So definicoes aqui - nada roda no import. Quem integra e mod_coreografia.py;
# quem prova este modulo sozinho e teste_ambiente.py.
#
# REVISAO 2 (docs/ESPECIFICACAO.md, item 2): NAO EXISTE CHAO.
#
# O plano de 400 m, a fusao "chao infinito" e a sombra de contato sairam. Os
# objetos flutuam num vazio e o fundo e so o world. O que isso muda por aqui:
# - a silhueta do produto branco depende SO do rim e da key (nao ha mais chao
#   escuro por tras da metade de baixo): teste_ambiente confere um cubo
#   branco de 4 azimutes;
# - a "poca" do reflexo do rim no chao, que obrigava a coreografia a zerar o
#   specular_factor do rim nos planos largos, deixou de existir - a API
#   (chavear_especular) fica, porque a coreografia a usa e nao faz mal;
# - o gradiente precisa ser bonito em 360 graus, porque agora a camera VE o
#   hemisferio de baixo (nao ha chao tapando) e o produto o reflete.
#
# O GRADIENTE: direcao no ESPACO DE MUNDO, rose em cima, preto embaixo,
# manchas em volta.
#
# 'Generated' no world e a direcao de visada normalizada em coordenadas do
# mundo: nao depende de onde a camera esta, so de para onde o raio aponta -
# e por isso o fundo e identico em qualquer angulo da orbita e identico no
# reflexo (reflexo tambem e uma direcao). A rampa base e funcao do Z dessa
# direcao (seno da elevacao): preto abaixo de 'elevacao'[0], rose acima de
# 'elevacao'[1], smoothstep entre os dois. O padrao (-0,42 .. 0,05, ou seja,
# -25 a +3 graus) poe a transicao no terco de cima do quadro de uma camera
# que olha 15 a 25 graus para baixo, que e a camera do anuncio: rose atras da
# metade de cima do produto e preto atras da de baixo.
#
# A MESCLA: um Noise Texture de grande escala sobre a mesma direcao
# ('escala_manchas' = quantas manchas cabem no diametro da esfera; 2-3
# oitavas = Detail 'oitavas_manchas' - 1, baixo contraste) e SOMADO ao fator
# da rampa antes do ColorRamp. Onde a rampa esta no meio, o ruido empurra a
# fronteira para cima e para baixo (dedos de preto entrando no rose e
# vice-versa); onde ela satura, so a metade do ruido que aponta para a outra
# cor tem efeito (manchas escuras no rose, manchas rose no preto). A
# amplitude e 'mesclagem' (0 = rampa lisa, 1 = o ruido vale a rampa inteira)
# vezes um peso que e cheio em volta do centro da transicao e cai, com a
# distancia em Z a esse centro ('largura_manchas'), ate 'manchas_nos_polos'
# no zenite e no nadir: as manchas vivem em volta da transicao e os polos
# ficam quase puros - e o preto quase puro no nadir e o que mantem o texto
# branco da cartela legivel.
#
# O AgX empalidece cor clara: o rose #F4E6E4 saia cinza (saturacao 0,04).
# Compensa na cor do world ('saturacao_clara' 6,0, medido para ~0,10 de
# saturacao no PNG), nao na paleta. 'forca_mundo' 1,8 leva o rose pleno a L
# 0,86 depois do AgX.
#
# O world e DOIS Backgrounds, separados por Light Path -> Is Camera Ray:
# 'fundo_camera' (o gradiente inteiro, Strength livre - e por ele que a
# coreografia escurece so o que a camera ve no momento-heroi) e 'fundo_luz'
# (o mesmo gradiente, Strength ligado a uma mascara que apaga o hemisferio de
# baixo entre -30 e -9 graus). O probe do EEVEE nao e ocluido por geometria:
# sem a mascara, o resto de rose das manchas de baixo iluminaria todo objeto
# por baixo. Acima de -9 graus as duas versoes sao iguais, entao o cromo
# reflete o mesmo ceu que a camera ve.
#
# Banding: ruido fino (grao abaixo do pixel: escala 3000 = 0,019 grau, contra
# 0,028 grau por pixel a 1920 de altura) de +-0,05% somado ao fator ANTES do
# ColorRamp, e dither_intensity no maximo na saida. Medido na rodada 1: o
# ruido do shader nesta amplitude nao muda nenhum numero do perfil; o dither
# de saida e o que faz o servico (+-2 niveis por pixel). O teste procura
# PATAMAR seguido de SALTO no perfil de uma coluna.
#
# HISTORICO (rodada 1, ainda vale para as luzes): o ponto branco do quadro 1
# era a luz DIFUSA do rim no chao ao pe do proprio painel (2,2 m de painel
# centrado a 1,0 m atravessava o chao). O painel de 1,2 m a 1,4 m ficou: e a
# altura em que o rim recorta a silhueta sem inundar a face frontal.
#
# OBTURADOR POR CHAVE (revisao 2, item 5 - estilo Instagram): o whoosh visual
# e motion blur mais forte SO nos movimentos largos. render.motion_blur_shutter
# aceita keyframe (medido no 4.2.5: SceneAction, interpola); animar_obturador
# escreve base 0,5 e 'forte' 0,7 nos trechos que a coreografia pede, com
# rampa LINEAR de 4 quadros nas bordas, e limpa a fcurve antes (idempotente).
#
# Eixos e medidas seguem docs/ESPECIFICACAO.md: metros, Z para cima, frente
# em -Y (a camera padrao fica em -Y olhando +Y). Nao ha chao; a origem (z = 0)
# e so a referencia onde o produto "para no ar".

import math

import bpy

NOME = "ambiente"
NOME_CAMERA = "camera"

# Deslocamento do rig de luz em relacao ao azimute da camera, na orbita.
# 90 poe o rim EXATAMENTE atras do produto e recorta a silhueta. A revisao
# propos 60 (rig 15 -> 165 em vez de 195) para a face lateral, que saia
# como 'slab branco liso' no q185, ganhar gradiente. MEDIDO no teste (cubo
# branco semi-fosco de 0,4 m, camera na normal da face, 50 mm): amplitude
# horizontal das medias de coluna na face = 1,4 / 1,7 / 4,3 / 5,6 / 1,2
# niveis de 255 com offset 30 / 60 / 90 / 120 / 150, e vertical 3,8 / 2,9 /
# 3,5 (60 / 90 / 120). Tudo chapado, e 60 e MAIS chapado que 90: com 60 a
# key (2 x 2 m a 3,3 m) cai a 12 graus da normal da face, luz frontal; o
# rim, a 138 graus, nem toca a face. A chapa vem do tamanho da key, nao do
# lugar do rim - o offset nao e a alavanca. Por isso o padrao continua 90 e
# o 60 fica como opcao (offset=OFFSET_RIM_LATERAL), com o numero medido
# para quem quiser insistir.
OFFSET_RIM_LATERAL = 60.0
OFFSET_RIM_ATRAS = 90.0

PARAMS_PADRAO = {
    "cor_escura": "#050507",
    "cor_clara": "#F4E6E4",
    # Rampa base por ELEVACAO: Z da direcao de visada (seno da elevacao) em
    # que o fundo e preto puro e em que e rose pleno; smoothstep entre os
    # dois. (-0,55, 0,20) = -33 a +12 graus, centro a -10: a transicao fica
    # no terco de cima do quadro de uma camera que olha 15-25 graus para
    # baixo (a do anuncio), rose atras da metade de cima do produto e preto
    # atras da de baixo. Larga de proposito: as manchas empurram a fronteira
    # e uma rampa estreita viraria borda dura. E dado, nao numero solto.
    "elevacao": (-0.55, 0.20),
    # Mescla por manchas (ver cabecalho): amplitude do ruido em fracao da
    # rampa (0 = gradiente liso, 1 = o ruido vale a rampa inteira), escala
    # (manchas por diametro da esfera de direcoes: 2 = manchas de ~60 graus),
    # numero de oitavas (2 a 3, baixo contraste) e quanto da amplitude sobra
    # no zenite e no nadir (o preto quase puro embaixo e o que segura a
    # legibilidade da cartela).
    # MEDIDO em grade (so o world, 270x480, 2 azimutes + cubemap): 0,45/2,2/
    # 2,5 oitavas/rugosidade 0,5 interpenetrava em dedos finos (fumaca);
    # 0,30/1,6/2 oitavas virava um horizonte ondulado (faixa de novo); 0,55/
    # 1,8/2 oitavas/rugosidade 0,3 sobre a rampa larga da lobos suaves de
    # ~60 graus com o rose predominando em cima. 0,7 ja invertia o quadro em
    # alguns azimutes (blob preto cobrindo o topo).
    "mesclagem": 0.55,
    "escala_manchas": 1.8,
    "oitavas_manchas": 2.0,
    "rugosidade_manchas": 0.3,    # peso das oitavas finas (0 = so a grande)
    # 0,3 deixava o nadir em L 0,166 no cubemap (criterio 0,15): 0,2.
    "manchas_nos_polos": 0.2,
    # Rampa base por elevacao: LINEAR (padrao) ou SMOOTHSTEP. Com smoothstep
    # E o EASE da rampa final sao dois S encadeados e a borda dos lobos
    # endurece; um S so (o final) deixa as manchas suaves.
    "rampa_base": "LINEAR",
    # Meia-largura (em Z) da zona em que as manchas tem amplitude cheia, em
    # volta do centro da transicao; fora dela o peso cai (quadratico) ate
    # 'manchas_nos_polos'. 0,8 leva o nadir e o zenite ao minimo.
    "largura_manchas": 0.8,
    # Interpolacao da rampa final (fator somado -> cor): EASE e um S que
    # comprime os meios-tons (borda mais definida); LINEAR deixa a borda mais
    # difusa. Um so segmento em qualquer caso (no intermediario vira degrau).
    "interpolacao_final": "EASE",
    # Forca do Background que a CAMERA ve: 1,8 leva o rose pleno a L 0,86
    # depois do AgX (0,55 dava cinza).
    "forca_mundo": 1.8,
    # Forca da versao que ILUMINA (probe), como fracao de forca_mundo. 1,0 =
    # o cromo reflete exatamente o ceu que a camera ve acima da mascara.
    "forca_luz": 1.0,
    # Mascara da versao que ilumina: Z em que ela e 0 e em que volta a 1
    # (-30 a -9 graus). O probe nao e ocluido por geometria; sem isto o resto
    # de rose das manchas de baixo iluminaria todo objeto por baixo.
    "mascara_luz": (-0.50, -0.15),
    # O AgX empalidece cor clara: o rose #F4E6E4 saia cinza (saturacao 0,04
    # com 1,6). Compensa na cor do world, nao na paleta (a mesma licao da logo
    # no modulo caixa). 6,0 e o medido para ~0,10 de saturacao no PNG.
    "saturacao_clara": 6.0,
    # Amplitude do ruido fino no fator do ramp (anti-banding). 0,0015 ainda
    # dava degrau de 3,8 niveis entre linhas vizinhas na parte ingreme.
    "dither": 0.0005,
    "escala_dither": 3000.0,  # grao do ruido: abaixo do pixel a 1920 de altura
    # Luzes: posicao (m), tamanho (x, y em m), energia (W), cor. As posicoes
    # sao relativas ao rig, um Empty na origem que a coreografia gira junto
    # com a orbita - o rim so recorta a silhueta se ficar ATRAS do produto do
    # ponto de vista da camera, e "atras" muda a cada quadro numa orbita.
    "alvo_luzes": (0.0, 0.0, 0.42),
    "luzes": {
        # 'abertura' e o spread da area light, em graus: e a colmeia do softbox
        # (o rim com 40 graus recorta a silhueta sem espalhar luz de lado).
        # 'especular' e o multiplicador de especular da luz (so EEVEE).
        "key":  {"pos": (2.2, -2.4, 2.8), "tam": (2.0, 2.0), "energia": 350.0, "cor": (1.0, 0.95, 0.90), "abertura": 100.0},
        "fill": {"pos": (-3.0, -2.0, 1.6), "tam": (3.0, 3.0), "energia": 110.0, "cor": (0.90, 0.94, 1.0), "abertura": 120.0},
        # O rim fica baixo (perto da altura do produto) e longe: painel de
        # 1,2 m centrado a 1,4 m, 40 graus de abertura. A posicao foi medida
        # na rodada 1 contra o chao (a luz difusa ao pe de um painel de 2,2 m
        # a 1,0 m era o ponto branco do quadro 1); sem chao o que importa e
        # que dali ele recorta a aresta do produto branco sem inundar a face
        # frontal, e o teste_ambiente confere isso nos 4 azimutes. Especular
        # 0,6: acima disso o recorte na aresta do cubo estourava.
        "rim":  {"pos": (0.6, 2.8, 1.4), "tam": (0.3, 1.2), "energia": 350.0, "cor": (1.0, 1.0, 1.0), "abertura": 40.0, "especular": 0.6},
        "top":  {"pos": (0.0, 0.3, 3.6), "tam": (3.0, 3.0), "energia": 80.0, "cor": (1.0, 0.98, 0.96), "abertura": 100.0},
    },
    # Flash
    "forca_flash": 16.0,      # emissao a forca=1.0; o padrao de animar_flash e 0.5 = 8
    "distancia_flash": 0.25,  # do sensor; acima do clip_start padrao (0,1)
    # Alfa do veu no quadro seguinte ao pico: o decaimento que le como flash
    # de foto em vez de dois quadros brancos solidos (revisao, beat 5). A
    # proposta era 0,35 - MEDIDO: 0,35 x 8 de emissao = 2,8 de radiancia
    # misturada sobre a cena, e o AgX satura em L 0,96 (branco de novo). O
    # mix e em radiancia linear, nao em 'porcentagem de branco': 0,05 x 8 =
    # 0,4 sobre a cena da L media 0,75 e minimo 0,66 (base 0,41) - meio veu
    # com a cena visivel por tras. 0,08 -> 0,81; 0,12 -> 0,85; 0,20 -> 0,90.
    "decaimento_flash": 0.05,
}

PARAMS_CAMERA_PADRAO = {
    "lente": 35.0,
    "sensor": 36.0,
    "f": 2.8,
    "pos": (1.6, -2.4, 1.2),
    "alvo": (0.0, 0.0, 0.42),
    "dof": True,
}

PARAMS_RENDER_PADRAO = {
    "shutter": 0.5,
    "bloom": True,
    "limiar_bloom": 2.5,       # so o que esta bem acima de 1.0 floresce
    "mistura_bloom": -0.75,    # -1 = sem glare, 0 = meio a meio
    "tamanho_bloom": 6,
    "video": False,            # True: FFMPEG H.264
    "caminho_saida": None,
}


# ---------------------------------------------------------------- utilidades

def _srgb_para_linear(c):
    # A paleta esta em sRGB (hex); os nos de shader querem linear.
    c = c / 255.0
    return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4


def cor_linear(hexa):
    h = hexa.lstrip("#")
    return tuple(_srgb_para_linear(int(h[i:i + 2], 16)) for i in (0, 2, 4)) + (1.0,)


def saturar(cor, fator):
    # Afasta cada canal do cinza de mesma luminancia; 1.0 nao muda nada.
    r, g, b = cor[:3]
    y = 0.2126 * r + 0.7152 * g + 0.0722 * b
    return tuple(max(0.0, y + (c - y) * fator) for c in (r, g, b)) + (1.0,)


def limpar_colecao(nome):
    """Remove a sub-colecao <nome> e tudo o que ela contem (idempotencia)."""
    col = bpy.data.collections.get(nome)
    if col is None:
        return
    for obj in list(col.all_objects):
        dados = obj.data
        bpy.data.objects.remove(obj, do_unlink=True)
        # Dado orfao fica no arquivo ate o proximo salvar; apagar aqui evita
        # acumular 'ambiente.chao.001' e 'camera.principal.001' nas rodadas.
        if dados is not None and dados.users == 0:
            for colecao in (bpy.data.meshes, bpy.data.lights, bpy.data.cameras):
                try:
                    colecao.remove(dados)
                    break
                except (TypeError, ReferenceError):
                    continue
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


def _mesclar(padrao, params):
    p = dict(padrao)
    if params:
        for k, v in params.items():
            if isinstance(v, dict) and isinstance(p.get(k), dict):
                d = {kk: dict(vv) if isinstance(vv, dict) else vv for kk, vv in p[k].items()}
                for kk, vv in v.items():
                    if isinstance(vv, dict) and isinstance(d.get(kk), dict):
                        d[kk].update(vv)
                    else:
                        d[kk] = vv
                p[k] = d
            else:
                p[k] = v
    return p


def _entrada(no, nome, valor):
    # Nome de socket muda entre versoes (Specular -> Specular IOR Level no
    # 4.0); quem nao existe e ignorado em vez de derrubar o script.
    soquete = no.inputs.get(nome)
    if soquete is not None:
        soquete.default_value = valor


def _material(nome):
    mat = bpy.data.materials.get(nome)
    if mat is not None:
        bpy.data.materials.remove(mat)
    mat = bpy.data.materials.new(nome)
    mat.use_nodes = True
    nt = mat.node_tree
    bsdf = nt.nodes.get("Principled BSDF")
    return mat, nt, bsdf


def _apontar(obj, alvo):
    tr = obj.constraints.new("TRACK_TO")
    tr.target = alvo
    tr.track_axis = "TRACK_NEGATIVE_Z"
    tr.up_axis = "UP_Y"
    return tr


# ---------------------------------------------------------------- world

def _map_range(nt, local, de, para, valor=None, suave=False):
    no = nt.nodes.new("ShaderNodeMapRange")
    no.location = local
    no.inputs["From Min"].default_value = de[0]
    no.inputs["From Max"].default_value = de[1]
    no.inputs["To Min"].default_value = para[0]
    no.inputs["To Max"].default_value = para[1]
    if suave:
        try:
            no.interpolation_type = "SMOOTHSTEP"
        except AttributeError:
            pass
    if valor is not None:
        nt.links.new(valor, no.inputs["Value"])
    return no


def _ruido_na_direcao(nt, local, direcao, escala, detalhe, rugosidade):
    """Noise Texture sobre a direcao de visada escalada: 'escala' = manchas por
    diametro da esfera de direcoes. Devolve o socket Fac."""
    esc = nt.nodes.new("ShaderNodeVectorMath")
    esc.operation = "SCALE"
    esc.location = local
    esc.inputs["Scale"].default_value = escala
    nt.links.new(direcao, esc.inputs[0])
    ruido = nt.nodes.new("ShaderNodeTexNoise")
    ruido.location = (local[0] + 200, local[1])
    ruido.inputs["Scale"].default_value = 1.0
    _entrada(ruido, "Detail", detalhe)
    _entrada(ruido, "Roughness", rugosidade)
    _entrada(ruido, "Distortion", 0.0)
    nt.links.new(esc.outputs["Vector"], ruido.inputs["Vector"])
    return ruido.outputs["Fac"]


def _mundo(p):
    nome = NOME + ".mundo"
    mundo = bpy.data.worlds.get(nome)
    if mundo is not None:
        bpy.data.worlds.remove(mundo)
    mundo = bpy.data.worlds.new(nome)
    mundo.use_nodes = True
    nt = mundo.node_tree
    for no in list(nt.nodes):
        nt.nodes.remove(no)

    saida = nt.nodes.new("ShaderNodeOutputWorld")
    saida.location = (1100, 0)

    # 'Generated' no world = direcao de visada normalizada, em coordenadas do
    # MUNDO (nao da camera): e o que faz o fundo ser o mesmo em toda a orbita
    # e o mesmo no reflexo do produto.
    coord = nt.nodes.new("ShaderNodeTexCoord")
    coord.location = (-1300, 0)
    direcao = coord.outputs["Generated"]
    sep = nt.nodes.new("ShaderNodeSeparateXYZ")
    sep.location = (-1100, 0)
    nt.links.new(direcao, sep.inputs["Vector"])
    z = sep.outputs["Z"]

    # Rampa base: preto abaixo de elevacao[0], rose acima de elevacao[1].
    rampa = _map_range(nt, (-900, 0), p["elevacao"], (0.0, 1.0), z, suave=(p["rampa_base"] == "SMOOTHSTEP"))

    # Manchas: ruido de grande escala (2-3 oitavas) mapeado para +-mesclagem,
    # com peso cheio em volta do centro da transicao e caindo (quadratico na
    # distancia em Z ao centro, 'largura_manchas') ate 'manchas_nos_polos'
    # no zenite e no nadir. O Noise do Blender concentra o Fac em ~0,3..0,7;
    # e essa faixa que vira +-1 antes de multiplicar pela amplitude.
    fac = _ruido_na_direcao(nt, (-1100, -350), direcao, p["escala_manchas"],
                            max(0.0, p["oitavas_manchas"] - 1.0), p["rugosidade_manchas"])
    manchas = _map_range(nt, (-700, -350), (0.3, 0.7), (-1.0, 1.0), fac)
    centro = 0.5 * (p["elevacao"][0] + p["elevacao"][1])
    dist = nt.nodes.new("ShaderNodeMath")
    dist.operation = "SUBTRACT"
    dist.location = (-1100, -600)
    nt.links.new(z, dist.inputs[0])
    dist.inputs[1].default_value = centro
    d2 = nt.nodes.new("ShaderNodeMath")
    d2.operation = "MULTIPLY"
    d2.location = (-900, -600)
    nt.links.new(dist.outputs["Value"], d2.inputs[0])
    nt.links.new(dist.outputs["Value"], d2.inputs[1])
    l2 = max(1e-6, float(p["largura_manchas"])) ** 2
    peso = _map_range(nt, (-700, -600), (0.0, l2), (p["mesclagem"], p["mesclagem"] * p["manchas_nos_polos"]),
                      d2.outputs["Value"])
    amplitude = nt.nodes.new("ShaderNodeMath")
    amplitude.operation = "MULTIPLY"
    amplitude.location = (-500, -350)
    nt.links.new(manchas.outputs["Result"], amplitude.inputs[0])
    nt.links.new(peso.outputs["Result"], amplitude.inputs[1])

    # Dither: ruido fino (grao abaixo do pixel) de +-dither, somado tambem.
    fac_d = _ruido_na_direcao(nt, (-1100, -900), direcao, p["escala_dither"], 0.0, 0.0)
    dither = _map_range(nt, (-700, -900), (0.35, 0.65), (-p["dither"], p["dither"]), fac_d)

    soma1 = nt.nodes.new("ShaderNodeMath")
    soma1.operation = "ADD"
    soma1.location = (-300, 0)
    nt.links.new(rampa.outputs["Result"], soma1.inputs[0])
    nt.links.new(amplitude.outputs["Value"], soma1.inputs[1])
    soma2 = nt.nodes.new("ShaderNodeMath")
    soma2.operation = "ADD"
    soma2.location = (-100, 0)
    nt.links.new(soma1.outputs["Value"], soma2.inputs[0])
    nt.links.new(dither.outputs["Result"], soma2.inputs[1])

    # Forma final: um ColorRamp EASE de dois pontos (smoothstep sobre o fator
    # somado, ja clampado em 0..1) - uma so curva em S, sem patamar no meio:
    # cada no intermediario do EASE viraria um degrau (licao da rodada 1).
    forma = nt.nodes.new("ShaderNodeValToRGB")
    forma.location = (100, 0)
    forma.color_ramp.interpolation = p["interpolacao_final"]
    pontos = forma.color_ramp.elements
    while len(pontos) > 1:
        pontos.remove(pontos[-1])
    pontos[0].position = 0.0
    pontos[0].color = (0.0, 0.0, 0.0, 1.0)
    e = pontos.new(1.0)
    e.color = (1.0, 1.0, 1.0, 1.0)
    nt.links.new(soma2.outputs["Value"], forma.inputs["Fac"])

    cor = nt.nodes.new("ShaderNodeMix")
    cor.data_type = "RGBA"
    cor.location = (400, 0)
    cor.inputs["A"].default_value = cor_linear(p["cor_escura"])
    cor.inputs["B"].default_value = saturar(cor_linear(p["cor_clara"]), p["saturacao_clara"])
    nt.links.new(forma.outputs["Color"], cor.inputs["Factor"])

    # Background da CAMERA: o gradiente inteiro, Strength livre (a coreografia
    # o escurece no momento-heroi). Nomeado, para quem procura nao depender
    # de qual socket esta ligado.
    fundo_cam = nt.nodes.new("ShaderNodeBackground")
    fundo_cam.name = fundo_cam.label = "fundo_camera"
    fundo_cam.location = (700, 100)
    fundo_cam.inputs["Strength"].default_value = p["forca_mundo"]
    nt.links.new(cor.outputs["Result"], fundo_cam.inputs["Color"])

    # Background da ILUMINACAO: o mesmo gradiente, apagado no hemisferio de
    # baixo (ver cabecalho: o probe nao e ocluido, e o resto de rose das
    # manchas de baixo iluminaria tudo por baixo).
    mascara = _map_range(nt, (-900, 350), p["mascara_luz"], (0.0, 1.0), z, suave=True)
    forca_luz = nt.nodes.new("ShaderNodeMath")
    forca_luz.operation = "MULTIPLY"
    forca_luz.location = (400, 350)
    forca_luz.inputs[1].default_value = p["forca_mundo"] * p["forca_luz"]
    nt.links.new(mascara.outputs["Result"], forca_luz.inputs[0])
    fundo_luz = nt.nodes.new("ShaderNodeBackground")
    fundo_luz.name = fundo_luz.label = "fundo_luz"
    fundo_luz.location = (700, 350)
    nt.links.new(cor.outputs["Result"], fundo_luz.inputs["Color"])
    nt.links.new(forca_luz.outputs["Value"], fundo_luz.inputs["Strength"])

    # Is Camera Ray: 1 no render pela camera, 0 no probe do world (EEVEE e
    # Cycles). Se o no faltar em alguma versao, fica so a versao da camera -
    # a cena fica um pouco mais clara por baixo, mas nao fica preta.
    try:
        caminho = nt.nodes.new("ShaderNodeLightPath")
        caminho.location = (700, -200)
        mistura = nt.nodes.new("ShaderNodeMixShader")
        mistura.location = (900, 0)
        nt.links.new(caminho.outputs["Is Camera Ray"], mistura.inputs["Fac"])
        nt.links.new(fundo_luz.outputs["Background"], mistura.inputs[1])
        nt.links.new(fundo_cam.outputs["Background"], mistura.inputs[2])
        nt.links.new(mistura.outputs["Shader"], saida.inputs["Surface"])
    except (RuntimeError, KeyError) as e:
        print("[ambiente] sem Light Path no world, iluminacao = camera:", e)
        nt.links.new(fundo_cam.outputs["Background"], saida.inputs["Surface"])
    return mundo


def forca_da_luz_do_mundo(mundo):
    """Socket do multiplicador de forca do Background que ILUMINA
    ('fundo_luz' x mascara): e por ele que a coreografia abaixa a luz do ceu
    num trecho (o topo da caixa visto de cima no beat 7) sem mudar o que a
    camera ve. None se o world nao tem a arvore esperada."""
    if mundo is None or not mundo.use_nodes:
        return None
    no = mundo.node_tree.nodes.get("fundo_luz")
    if no is None or not no.inputs["Strength"].is_linked:
        return None
    origem = no.inputs["Strength"].links[0].from_node
    if origem.type == "MATH" and len(origem.inputs) > 1 and not origem.inputs[1].is_linked:
        return origem.inputs[1]
    return None


def fundo_da_camera(mundo):
    """Socket Strength do Background que so a CAMERA ve (o que a coreografia
    escurece no momento-heroi): pelo nome 'fundo_camera'; num world de outra
    origem, o Background cujo Strength nao esta ligado. None se nao ha."""
    if mundo is None or not mundo.use_nodes:
        return None
    no = mundo.node_tree.nodes.get("fundo_camera")
    if no is not None:
        return no.inputs["Strength"]
    for no in mundo.node_tree.nodes:
        if no.type == "BACKGROUND" and not no.inputs["Strength"].is_linked:
            return no.inputs["Strength"]
    return None


# ---------------------------------------------------------------- luzes

def _luz(col, nome_curto, cfg, rig, alvo):
    nome = "%s.luz.%s" % (NOME, nome_curto)
    dados = bpy.data.lights.new(nome, "AREA")
    dados.shape = "RECTANGLE"
    dados.size, dados.size_y = cfg["tam"]
    dados.energy = cfg["energia"]
    dados.color = cfg["cor"]
    dados.spread = math.radians(cfg.get("abertura", 180.0))
    if "especular" in cfg:
        # Atributo do EEVEE; se sumir numa versao, a luz fica com 1,0.
        _ajustar(dados, "specular_factor", cfg["especular"])
    try:
        # Sem isto a luz vaza por parede fina (8 mm da caixa) e a face interna
        # sai chuviscada - achado do modulo caixa. Nao existe antes do 4.2.
        dados.use_shadow_jitter = True
    except AttributeError:
        pass
    obj = bpy.data.objects.new(nome, dados)
    obj.location = cfg["pos"]
    obj.parent = rig
    col.objects.link(obj)
    _apontar(obj, alvo)
    return obj


# ---------------------------------------------------------------- API

def construir_ambiente(cena, colecao_pai=None, params=None):
    """Cria world (fundo infinito, sem chao) e rig com 4 luzes na sub-colecao 'ambiente'."""
    p = _mesclar(PARAMS_PADRAO, params)
    limpar_colecao(NOME)
    col = _colecao(cena, colecao_pai, NOME)

    mundo = _mundo(p)
    cena.world = mundo

    rig = bpy.data.objects.new(NOME + ".rig", None)
    rig.empty_display_type = "PLAIN_AXES"
    rig.empty_display_size = 0.5
    col.objects.link(rig)
    alvo = bpy.data.objects.new(NOME + ".alvo", None)
    alvo.empty_display_type = "SPHERE"
    alvo.empty_display_size = 0.1
    alvo.location = p["alvo_luzes"]
    alvo.parent = rig
    col.objects.link(alvo)

    luzes = {k: _luz(col, k, cfg, rig, alvo) for k, cfg in p["luzes"].items()}

    objs = {
        "colecao": col,
        "mundo": mundo,
        # Nao ha chao (revisao 2). A chave fica, com None, para quem lia
        # amb["chao"] nao quebrar.
        "chao": None,
        "rig": rig,
        "alvo_luzes": alvo,
        "luzes": luzes,
        "flash": None,
        "cor_escura": p["cor_escura"],
        "cor_clara": p["cor_clara"],
        "params": p,
    }
    objs.update(luzes)
    return objs


def criar_camera(cena, colecao=None, nome="camera.principal", params=None):
    """Camera 35 mm full-frame com alvo (Track To) e DoF no alvo. Devolve (camera, alvo)."""
    p = _mesclar(PARAMS_CAMERA_PADRAO, params)
    limpar_colecao(NOME_CAMERA)
    col = _colecao(cena, colecao, NOME_CAMERA)

    dados = bpy.data.cameras.new(nome)
    dados.lens = p["lente"]
    # 'sensor 36' = full frame. Em AUTO os 36 mm vao para o lado MAIOR do
    # quadro; no 9:16 vertical e a altura - o que e a foto de celular com o
    # sensor em pe, e o que da 54 graus verticais com a 35 mm.
    dados.sensor_fit = "AUTO"
    dados.sensor_width = p["sensor"]
    dados.clip_start = 0.05
    dados.clip_end = 500.0
    cam = bpy.data.objects.new(nome, dados)
    cam.location = p["pos"]
    col.objects.link(cam)

    alvo = bpy.data.objects.new("camera.alvo", None)
    alvo.empty_display_type = "SPHERE"
    alvo.empty_display_size = 0.08
    alvo.location = p["alvo"]
    col.objects.link(alvo)
    _apontar(cam, alvo)

    dados.dof.use_dof = p["dof"]
    dados.dof.focus_object = alvo
    dados.dof.aperture_fstop = p["f"]
    dados.dof.aperture_blades = 9

    cena.camera = cam
    return cam, alvo


def _ajustar(objeto, nome, valor):
    # Atributo que mudou de nome ou sumiu entre versoes: tenta, nao derruba.
    try:
        setattr(objeto, nome, valor)
        return True
    except (AttributeError, TypeError, ValueError):
        return False


def _arvore_compositor(cena):
    """Arvore de nos do compositor da cena, criando-a se preciso.

    Ate o 4.x ela e Scene.node_tree (que so existe depois de use_nodes = True).
    No 5.0 Scene.use_nodes e Scene.node_tree sairam da API: o compositor e um
    node group em Scene.compositing_node_group, que comeca None. Sem nenhum
    dos dois, levanta RuntimeError e configurar_render segue sem bloom.
    """
    _ajustar(cena, "use_nodes", True)
    nt = getattr(cena, "node_tree", None)
    if nt is not None:
        return nt
    if hasattr(cena, "compositing_node_group"):
        nt = cena.compositing_node_group
        if nt is None:
            nt = bpy.data.node_groups.new(NOME + ".compositor", "CompositorNodeTree")
            cena.compositing_node_group = nt
        return nt
    raise RuntimeError("cena sem node_tree nem compositing_node_group")


def _bloom(cena, p):
    """Glare/bloom no compositor. Os nos mudam de nome e de forma entre versoes."""
    nt = _arvore_compositor(cena)
    for no in list(nt.nodes):
        nt.nodes.remove(no)
    camadas = nt.nodes.new("CompositorNodeRLayers")
    camadas.location = (-400, 0)
    try:
        saida = nt.nodes.new("CompositorNodeComposite")
    except RuntimeError:
        # Num node group de compositor a saida pode ser a do proprio grupo.
        saida = nt.nodes.new("NodeGroupOutput")
        nt.interface.new_socket("Image", in_out="OUTPUT", socket_type="NodeSocketColor")
    saida.location = (400, 0)
    glare = nt.nodes.new("CompositorNodeGlare")
    glare.location = (0, 0)
    glare.glare_type = "BLOOM"
    # 4.2: propriedades no no. 4.5+: viraram sockets de entrada, e a mistura
    # saiu do no (ele devolve Glare e Image separados). Cobrimos os dois.
    if not _ajustar(glare, "threshold", p["limiar_bloom"]):
        _entrada(glare, "Threshold", p["limiar_bloom"])
    if not _ajustar(glare, "mix", p["mistura_bloom"]):
        # sem 'mix', a forca e o que dosa; -0,75 de mix equivale a ~0,25
        _entrada(glare, "Strength", (p["mistura_bloom"] + 1.0))
    if not _ajustar(glare, "size", p["tamanho_bloom"]):
        _entrada(glare, "Size", p["tamanho_bloom"] / 9.0)
    _ajustar(glare, "quality", "HIGH")
    nt.links.new(camadas.outputs["Image"], glare.inputs["Image"])
    nt.links.new(glare.outputs["Image"], saida.inputs["Image"])


def configurar_render(cena, largura=1080, altura=1920, fps=30, amostras=64, params=None):
    """EEVEE Next + AgX + motion blur + raytracing + bloom. Devolve o dict de params usado."""
    p = _mesclar(PARAMS_RENDER_PADRAO, params)
    r = cena.render

    cena.unit_settings.system = "METRIC"
    cena.unit_settings.scale_length = 1.0
    r.fps = fps
    r.fps_base = 1.0
    r.resolution_x = largura
    r.resolution_y = altura
    r.resolution_percentage = 100

    try:
        r.engine = "BLENDER_EEVEE_NEXT"
    except TypeError:
        # Blender 5 renomeou o EEVEE Next de volta para BLENDER_EEVEE.
        r.engine = "BLENDER_EEVEE"
    ee = cena.eevee
    ee.taa_render_samples = amostras

    # Motion blur: no 4.2 o shutter mora em render; antes, em eevee.
    r.use_motion_blur = True
    if not _ajustar(r, "motion_blur_shutter", p["shutter"]):
        _ajustar(ee, "motion_blur_shutter", p["shutter"])

    # Raytracing (reflexo e refracao do vidro). Legado: SSR + SSR refraction.
    if not _ajustar(ee, "use_raytracing", True):
        _ajustar(ee, "use_ssr", True)
        _ajustar(ee, "use_ssr_refraction", True)
    try:
        ee.ray_tracing_options.use_denoise = True
        ee.ray_tracing_options.resolution_scale = "1"
    except AttributeError:
        pass

    # Sombras suaves: no Next sao sempre por area e o que se dosa e o numero
    # de raios/passos; no legado era um interruptor.
    if not _ajustar(ee, "use_shadows", True):
        _ajustar(ee, "use_soft_shadows", True)
    _ajustar(ee, "shadow_ray_count", 2)
    _ajustar(ee, "shadow_step_count", 4)
    _ajustar(ee, "use_shadow_jitter_viewport", True)

    # Cor: AgX, look de contraste medio-alto; filme opaco (o fundo E a cena).
    cena.view_settings.view_transform = "AgX"
    try:
        cena.view_settings.look = "AgX - Medium High Contrast"
    except TypeError:
        pass
    r.film_transparent = False
    # Dither maximo na saida: e a segunda metade do remedio contra banding.
    r.dither_intensity = 2.0

    if p["video"]:
        r.image_settings.file_format = "FFMPEG"
        r.image_settings.color_mode = "RGB"
        r.ffmpeg.format = "MPEG4"
        r.ffmpeg.codec = "H264"
        r.ffmpeg.constant_rate_factor = "HIGH"
        r.ffmpeg.ffmpeg_preset = "GOOD"
        r.ffmpeg.gopsize = fps
        _ajustar(r.ffmpeg, "audio_codec", "NONE")
    else:
        r.image_settings.file_format = "PNG"
        r.image_settings.color_mode = "RGB"
        r.image_settings.color_depth = "8"
        r.image_settings.compression = 50
    if p["caminho_saida"]:
        r.filepath = p["caminho_saida"]

    if p["bloom"]:
        try:
            _bloom(cena, p)
        except Exception as e:   # noqa: BLE001 - qualquer no ausente
            # Compositor sem saida ligada renderiza preto: melhor sem bloom
            # do que sem imagem.
            print("[ambiente] bloom desligado, compositor incompativel:", e)
            # No 5.0 use_nodes nao existe: escrever direto aqui levantava
            # dentro do proprio handler e derrubava o main() na ultima etapa.
            _ajustar(cena, "use_nodes", False)
    else:
        _ajustar(cena, "use_nodes", False)
    return p


def animar_obturador(cena, trechos, base=0.5, forte=0.7, rampa=4):
    """Motion blur mais forte SO nos movimentos largos: render.motion_blur_shutter
    = 'forte' dentro de cada (q_ini, q_fim) de 'trechos', 'base' fora, com
    rampa LINEAR de 'rampa' quadros nas bordas. Idempotente: a fcurve antiga
    e removida antes. Devolve a lista de chaves (quadro, valor).

    E o "whoosh visual" do estilo (revisao 2, item 5): o obturador aberto a
    0,7 arrasta a caixa que sobe e a orbita; nas fotos e no close da tela,
    0,5. Medido no 4.2.5: a propriedade aceita keyframe e interpola.
    """
    r = cena.render
    ad = cena.animation_data
    if ad is not None and ad.action is not None:
        for fc in list(fcurves_de(ad)):
            if fc.data_path == "render.motion_blur_shutter":
                fcurves_de(ad).remove(fc)
    chaves = [(1, base)]
    for q_a, q_b in sorted(trechos):
        chaves += [(q_a - rampa, base), (q_a, forte), (q_b, forte), (q_b + rampa, base)]
    # Chave repetida no mesmo quadro (trechos encostados): a ultima vale.
    ordenadas = {}
    for q, v in chaves:
        ordenadas[max(1, int(q))] = v
    gravadas = []
    for q, v in sorted(ordenadas.items()):
        r.motion_blur_shutter = v
        try:
            r.keyframe_insert("motion_blur_shutter", frame=q)
        except (RuntimeError, TypeError) as e:
            # Versao sem a propriedade animavel: fica o valor base, sem whoosh.
            print("[ambiente] obturador nao aceita chave:", e)
            r.motion_blur_shutter = base
            return []
        gravadas.append((q, v))
    for fc in fcurves_de(cena.animation_data):
        if fc.data_path == "render.motion_blur_shutter":
            for kp in fc.keyframe_points:
                kp.interpolation = "LINEAR"
            fc.update()
    r.motion_blur_shutter = base
    return gravadas


def empacotar_imagens():
    """img.pack() em toda imagem com arquivo e ainda nao empacotada.

    As imagens carregadas por load() apontam para a pasta temporaria de onde
    o script as extraiu (%TEMP% no Windows): limpar temporarios, reiniciar ou
    abrir o .blend em outra maquina deixava logo e telas rosa. A coreografia
    chama isto antes de salvar. Devolve a lista de nomes empacotados; imagem
    cujo arquivo sumiu e avisada e pulada, nao derruba o salvar.
    """
    feitas = []
    for img in bpy.data.images:
        if img.packed_file is not None or not img.filepath:
            continue
        if img.source not in {"FILE", "SEQUENCE"}:
            continue
        try:
            img.pack()
            feitas.append(img.name)
        except RuntimeError as e:
            print("[ambiente] nao empacotou '%s' (%s): %s" % (img.name, img.filepath, e))
    return feitas


# ---------------------------------------------------------------- flash

def _material_flash(forca):
    nome = NOME + ".flash"
    mat, nt, bsdf = _material(nome)
    for no in list(nt.nodes):
        nt.nodes.remove(no)
    saida = nt.nodes.new("ShaderNodeOutputMaterial")
    saida.location = (400, 0)
    mistura = nt.nodes.new("ShaderNodeMixShader")
    mistura.location = (200, 0)
    transparente = nt.nodes.new("ShaderNodeBsdfTransparent")
    transparente.location = (0, 100)
    emissao = nt.nodes.new("ShaderNodeEmission")
    emissao.location = (0, -100)
    emissao.inputs["Color"].default_value = (1.0, 1.0, 1.0, 1.0)
    emissao.inputs["Strength"].default_value = forca
    alfa = nt.nodes.new("ShaderNodeValue")
    alfa.name = alfa.label = "alfa"
    alfa.location = (0, 250)
    alfa.outputs[0].default_value = 0.0
    nt.links.new(alfa.outputs[0], mistura.inputs["Fac"])
    nt.links.new(transparente.outputs[0], mistura.inputs[1])
    nt.links.new(emissao.outputs[0], mistura.inputs[2])
    nt.links.new(mistura.outputs[0], saida.inputs["Surface"])
    # Blended: e um veu sobre a imagem, nao uma superficie que precisa de
    # profundidade nem de refracao. Em 4.1 e antes o nome era blend_method.
    try:
        mat.surface_render_method = "BLENDED"
    except AttributeError:
        mat.blend_method = "BLEND"
    _ajustar(mat, "shadow_method", "NONE")
    mat.use_backface_culling = False
    return mat, alfa


def _plano_flash(objs, camera, forca, distancia):
    nome = NOME + ".flash"
    obj = objs.get("flash")
    if obj is None or obj.name not in bpy.data.objects:
        malha = bpy.data.meshes.new(nome)
        # 6 m a 0,25 m da camera cobre qualquer lente que o anuncio use;
        # a folga e o que mantem o veu uniforme quando o DoF o borra.
        s = 3.0
        malha.from_pydata([(-s, -s, 0), (s, -s, 0), (s, s, 0), (-s, s, 0)], [], [(0, 1, 2, 3)])
        malha.update()
        obj = bpy.data.objects.new(nome, malha)
        objs["colecao"].objects.link(obj)
        mat, alfa = _material_flash(forca)
        malha.materials.append(mat)
        # O veu nao pode iluminar nem sombrear a cena no quadro do flash: a luz
        # do flash e a que ja esta la, o veu so branqueia a imagem.
        for atributo in ("visible_shadow", "visible_diffuse", "visible_glossy",
                         "visible_transmission", "visible_volume_scatter"):
            _ajustar(obj, atributo, False)
        objs["flash"] = obj
    obj.parent = camera
    obj.matrix_parent_inverse.identity()
    obj.location = (0.0, 0.0, -distancia)
    obj.rotation_euler = (0.0, 0.0, 0.0)
    return obj


def fcurves_de(animation_data):
    """Fcurves da acao de um animation_data, em qualquer Blender 4.2+."""
    # Action.fcurves virou legado no 4.4 (slotted actions); no 5.0 pode nao existir.
    try:
        return animation_data.action.fcurves
    except AttributeError:
        slot = animation_data.action_slot
        return animation_data.action.layers[0].strips[0].channelbag(slot).fcurves


def animar_flash(objs, camera, quadro, forca=0.5, largura=1, decaimento=None):
    """Flash de foto: veu branco parented na camera. Alfa 0 -> 1 -> 0,05 -> 0.

    Chaves do alfa: 0 em quadro-1 e 1 em quadro, as duas CONSTANT; depois
    'decaimento' (0,05, ver PARAMS_PADRAO) em quadro+largura e 0 em
    quadro+2*largura, LINEAR.
    'forca' multiplica a emissao (0,5 = 8: branco que le como flash, nao como
    quadro solido; 1,0 = 16, estourado).

    POR QUE CONSTANT nas duas primeiras: o render tem motion blur com o
    obturador em START, entao o quadro q expoe de q a q+0,5. Com a rampa
    linear 0 -> 1 entre q-1 e q, o quadro q-1 - o ULTIMO do plano anterior -
    ja expunha alfa 0,25..0,5 x 16 de emissao e saia branco (revisao: q359,
    o payoff da UI, veio quase branco). CONSTANT segura 0 ate q exato, e o
    pico segura 1 por todo o obturador de q. O decaimento e LINEAR de
    proposito: em q+1 o veu expoe 0,05 -> 0,025 (meio veu medido, L 0,75)
    e some em q+2.
    """
    p = objs.get("params", PARAMS_PADRAO)
    if decaimento is None:
        decaimento = p.get("decaimento_flash", 0.35)
    obj = _plano_flash(objs, camera, p["forca_flash"] * forca, p["distancia_flash"])
    mat = obj.data.materials[0]
    nt = mat.node_tree
    alfa = nt.nodes["alfa"].outputs[0]
    emissao = next(n for n in nt.nodes if n.type == "EMISSION").inputs["Strength"]
    chaves = (
        (quadro - 1, 0.0, "CONSTANT"),
        (quadro, 1.0, "CONSTANT"),
        (quadro + largura, decaimento, "LINEAR"),
        (quadro + 2 * largura, 0.0, "LINEAR"),
    )
    # Forca por flash: a coreografia pode pedir um flash mais fraco no beat 5
    # sem trocar o material - por isso a emissao tambem e chaveada.
    for q, a, _ in chaves:
        alfa.default_value = a
        alfa.keyframe_insert("default_value", frame=q)
        emissao.default_value = p["forca_flash"] * forca
        emissao.keyframe_insert("default_value", frame=q)
    # So as chaves DESTE flash mudam de interpolacao: o material e um so para
    # os tres flashes do beat 5, e mexer em todas desfaria as dos outros.
    interp = {q: i for q, _, i in chaves}
    acao = nt.animation_data.action if nt.animation_data else None
    if acao is not None:
        for fc in fcurves_de(nt.animation_data):
            for kp in fc.keyframe_points:
                i = interp.get(int(round(kp.co.x)))
                if i is not None:
                    kp.interpolation = i if fc.data_path.startswith('nodes["alfa"]') else "CONSTANT"
            fc.update()
    alfa.default_value = 0.0
    return obj


def _valor_animado(dono, data_path, quadro, indice=-1):
    # Valor que uma propriedade tem num quadro: da fcurve se houver chave, senao
    # o valor atual - e o que "partir de onde esta" precisa saber.
    ad = getattr(dono, "animation_data", None)
    if ad is not None and ad.action is not None:
        for fc in fcurves_de(ad):
            if fc.data_path == data_path and (indice < 0 or fc.array_index == indice):
                return fc.evaluate(quadro)
    valor = dono.path_resolve(data_path)
    return valor[indice] if indice >= 0 else valor


def angulo_rig(azimute_camera, offset=OFFSET_RIM_LATERAL):
    """Angulo do rig de luz para um azimute de camera (graus): azimute + offset."""
    return azimute_camera + offset


def animar_rig(objs, quadro_ini, quadro_fim, angulo_ini, angulo_fim, easing="EASE_IN_OUT",
               azimutes=False, offset=OFFSET_RIM_ATRAS):
    """Gira o rig das luzes em Z (graus), para o rim acompanhar a orbita da camera.

    azimutes=False (comportamento antigo): angulo_ini/angulo_fim sao angulos
    do proprio rig. azimutes=True: sao azimutes da CAMERA, e o rig vai para
    azimute + offset - OFFSET_RIM_ATRAS (90, padrao: rim exatamente atras)
    ou OFFSET_RIM_LATERAL (60, a proposta da revisao para a lateral; medido
    chapado igual, ver o comentario das constantes). angulo_ini=None parte
    do angulo que o rig ja tem em quadro_ini (a chave do beat anterior), o
    caso do inicio de uma orbita.
    """
    rig = objs["rig"]
    if angulo_ini is None:
        angulo_ini = math.degrees(_valor_animado(rig, "rotation_euler", quadro_ini, 2))
    elif azimutes:
        angulo_ini = angulo_rig(angulo_ini, offset)
    if azimutes:
        angulo_fim = angulo_rig(angulo_fim, offset)
    for q, ang in ((quadro_ini, angulo_ini), (quadro_fim, angulo_fim)):
        rig.rotation_euler = (0.0, 0.0, math.radians(ang))
        rig.keyframe_insert("rotation_euler", index=2, frame=q)
    for fc in fcurves_de(rig.animation_data):
        for kp in fc.keyframe_points:
            kp.interpolation = "BEZIER"
            kp.easing = easing


# ---------------------------------------------------------------- fatores de luz

def _luzes(luzes):
    # Aceita o Object da luz, o Light, uma lista deles, o dict 'luzes' do
    # construir_ambiente ou o proprio dict devolvido por ele.
    if isinstance(luzes, dict):
        luzes = luzes.get("luzes", luzes).values()
    elif hasattr(luzes, "bl_rna"):
        luzes = (luzes,)
    for luz in luzes:
        dados = getattr(luz, "data", None)
        yield dados if dados is not None and hasattr(dados, "energy") else luz


def chavear_fator_luz(luzes, atributo, quadro_ini, quadro_fim=None, de=None, para=0.0,
                      rampa=12, easing="EASE_IN_OUT"):
    """Rampa Bezier de um fator de luz (specular_factor, diffuse_factor, energy...).

    Grava 'de' em quadro_ini e 'para' em quadro_fim (quadro_ini + rampa se
    None), Bezier com o easing dado - NUNCA constant: a chave constante do
    especular do rim no primeiro quadro do beat 3 fazia a cunha do reflexo no
    chao aparecer de um quadro para o outro no meio de um plano continuo
    (revisao, q160 -> q165). de=None parte do valor que a luz tem em
    quadro_ini. rampa=0 e um corte de proposito: uma chave so, CONSTANT.
    So as chaves gravadas aqui mudam de interpolacao. Devolve as luzes tocadas.
    """
    if quadro_fim is None:
        quadro_fim = quadro_ini + rampa
    tocadas = []
    for dados in _luzes(luzes):
        if not hasattr(dados, atributo):
            continue
        atual = _valor_animado(dados, atributo, quadro_ini)
        if quadro_fim > quadro_ini:
            inicio = atual if de is None else de
            chaves = [(quadro_ini, inicio, "BEZIER"), (quadro_fim, para, "BEZIER")]
            salto = abs(inicio - atual) > 1e-9
        else:
            chaves = [(quadro_ini, para, "CONSTANT")]
            salto = abs(para - atual) > 1e-9
        if salto:
            # O trecho ANTES de quadro_ini e governado pela chave anterior, que
            # e Bezier e rampearia devagar ate o valor novo (medido: um corte
            # em q140 depois de uma rampa que acabou em q126 dava 0,49 em
            # q139). Uma chave de espera em quadro_ini-1 segura o valor que a
            # luz tinha ate o instante do salto.
            chaves.insert(0, (quadro_ini - 1, _valor_animado(dados, atributo, quadro_ini - 1), "CONSTANT"))
        try:
            for q, v, _ in chaves:
                setattr(dados, atributo, v)
                dados.keyframe_insert(atributo, frame=q)
        except (RuntimeError, TypeError):
            # Versao sem a propriedade animavel: fica o valor final, sem rampa.
            setattr(dados, atributo, para)
            continue
        interp = {int(round(q)): i for q, _, i in chaves}
        for fc in fcurves_de(dados.animation_data):
            if fc.data_path != atributo:
                continue
            for kp in fc.keyframe_points:
                i = interp.get(int(round(kp.co.x)))
                if i is not None:
                    kp.interpolation = i
                    if i == "BEZIER":
                        kp.easing = easing
            fc.update()
        tocadas.append(dados)
    return tocadas


def chavear_especular(luzes, quadro_ini, quadro_fim=None, de=None, para=0.0, rampa=12,
                      easing="EASE_IN_OUT"):
    """specular_factor com rampa Bezier: chavear_fator_luz(..., 'specular_factor', ...)."""
    return chavear_fator_luz(luzes, "specular_factor", quadro_ini, quadro_fim, de, para, rampa, easing)
