# Modulo AMBIENTE do anuncio do Snapmaker U1.
#
# Entrega o "estudio": fundo em gradiente preto -> rose-branco, chao escuro
# com reflexo suave, quatro luzes de estudio (key, fill, rim, top), a camera
# principal com profundidade de campo, a configuracao de render (EEVEE Next,
# AgX, motion blur, raytracing, bloom no compositor) e o FLASH de foto do
# beat 5. So definicoes aqui - nada roda no import. Quem integra e
# mod_coreografia.py; quem prova este modulo sozinho e teste_ambiente.py.
#
# DECISAO: o gradiente vive no WORLD, nao num ciclorama.
#
# A camera orbita 360 graus e o produto e metal e vidro: tudo o que esta
# atras da camera aparece refletido no produto. Um ciclorama e uma parede:
# tem borda, tem costura onde o chao encontra a curva, e ou ele envolve a cena
# inteira (uma cupula, que e o world com mais passos) ou a camera, em alguma
# fase da orbita, enxerga o fim dele - direto ou num reflexo. O world nao tem
# borda. O gradiente aqui e funcao SO da ELEVACAO da direcao de visada (o Z
# do vetor normalizado): e por isso que ele e identico em qualquer angulo da
# orbita e identico no reflexo, porque reflexo tambem e uma direcao. Uma
# faixa rose baixa em volta do horizonte, escurecendo rumo ao zenite - e o
# fundo de estudio da Apple: um brilho atras do produto, nao um ceu branco.
#
# A CURVA do gradiente e feita para a camera do anuncio, nao para o ceu
# inteiro. A 35 mm em 9:16 ve 54 graus na vertical; olhando de leve para
# baixo (8 a 18 graus), o topo do quadro fica entre 9 e 19 graus de elevacao.
# As tres primeiras versoes so chegavam ao preto acima de ~35 graus, ou seja,
# NUNCA dentro do quadro: o topo saia cinza (#C0B8B6 medido) e o brilho caia
# no meio da imagem. Agora o brilho e MAXIMO no horizonte e abaixo dele, cai
# pela metade a ~7 graus e e preto a ~15 graus - a faixa e estreita, e e por
# isso que a forca pode ser alta (1,8) sem inundar a cena.
#
# A curva tem UM segmento de queda, de proposito. O ColorRamp em EASE faz um
# smoothstep entre cada par de pontos, com derivada zero nos nos: com cinco
# pontos na descida a previa mostrou dois "degraus" (patamares em 27 e 98 de
# 255 no perfil da coluna) que o olho le como faixas. Um so segmento e uma so
# curva em S, sem patamar. O rose: o AgX puxa cor clara para o branco, e o
# #F4E6E4 saia #D8D1D0 com saturacao 0,04. Medido em grade (forca x
# saturacao, so o world): forca 1,8 e saturacao 6 dao L 0,86 e saturacao
# ~0,10 depois do AgX - abaixo disso e cinza, acima vira salmao.
#
# O world e DOIS Backgrounds, separados por Light Path -> Is Camera Ray:
# - a camera ve o gradiente inteiro, com o rose tambem abaixo do horizonte
#   (o chao infinito funde nele, ver abaixo);
# - a iluminacao (o probe do EEVEE, que tambem e o fallback do raytracing)
#   ve o mesmo gradiente acima do horizonte e PRETO abaixo dele. Abaixo do
#   horizonte, no mundo real, ha o chao - mas o probe do EEVEE nao e
#   ocluido por geometria, e um hemisferio inteiro de rose a 1,8 de forca
#   iluminaria todo objeto por baixo. Como acima do horizonte as duas
#   versoes sao iguais, o cromo reflete o mesmo ceu que a camera ve; abaixo
#   dele o raio bate no chao (raytracing) ou cai no probe escuro, que e a
#   cor que o chao teria de qualquer jeito.
#
# O horizonte: o chao e um plano de 400 m e a borda dele cai a menos de 0,3
# grau abaixo do horizonte verdadeiro. A linha que APARECIA nas previas nao
# era a borda: era a curva, que valia 0,10 na elevacao zero e 1,0 a 7 graus -
# uma faixa escura entre o brilho e o chao, e o chao infinito copiava
# justamente essa cor escura. Fundo Apple nao tem horizonte. Agora a curva
# tem o valor maximo no horizonte, e o "chao infinito" (longe da origem, 6 ->
# 30 m, o material do chao vira emissao com a cor do horizonte e perde o
# especular) funde no proprio brilho. De perto o chao e chao; de longe e o
# brilho; a costura nao existe de nenhum angulo da orbita porque a camera
# nunca sai de perto da caixa.
#
# Banding: o gradiente recebe um ruido fino (Noise Texture sobre a direcao,
# +-0,05% no fator do ramp, grao abaixo do pixel: escala 3000 = 0,019 grau,
# contra 0,028 grau por pixel a 1920 de altura) ANTES do ColorRamp, e o
# render sai com dither_intensity no maximo. A versao anterior tinha grao de
# 2-3 px e amplitude 12x maior, e aparecia como mosqueado no ceu - e por ser
# deterministico nao sumia com mais amostras. Medido (world so, 540x960, com
# e sem cada um): o ruido do shader nesta amplitude nao muda nenhum numero
# do perfil; o dither de saida e o que faz o servico - poe +-2 niveis por
# pixel e derruba os niveis distintos numa coluna de 157 para 142. Na parte
# mais ingreme da curva a inclinacao e de 2 niveis por linha a 960 px (1 a
# 1920), que e rampa, nao escada: o teste procura PATAMAR seguido de salto.
#
# Eixos e medidas seguem docs/ESPECIFICACAO.md: metros, Z para cima, frente
# em -Y (a camera padrao fica em -Y olhando +Y), chao em z = 0.

import math

import bpy

NOME = "ambiente"
NOME_CAMERA = "camera"

PARAMS_PADRAO = {
    "cor_escura": "#050507",
    "cor_clara": "#F4E6E4",
    # Curva do gradiente: (fator, mistura). fator = (sen(elevacao) + 0,1) / 1,1:
    # 0 = 5,7 graus abaixo do horizonte, 0,091 = horizonte, 1 = zenite;
    # mistura 0 = preto, 1 = rose-branco. E dado, nao numero solto, para a
    # coreografia poder pedir um fundo mais fechado num beat.
    # Maximo no horizonte E abaixo dele (os dois primeiros pontos iguais a 1,0:
    # e a cor que o chao infinito copia, ver cor_horizonte), e UM segmento em
    # S ate o preto a 14,6 graus (0,34): metade a ~7 graus, 0,29 a 10 graus. A
    # camera do anuncio, de leve para baixo, tem o topo do quadro entre 9 e 19
    # graus: e ali que o preto tem de estar, nao a 35 graus como antes. Nao
    # acrescente pontos no meio da descida: cada no do EASE vira um patamar.
    "curva": ((0.0, 1.0), (0.09, 1.0), (0.34, 0.0), (1.0, 0.0)),
    # Forca do Background que a CAMERA ve. Com a faixa estreita, 1,8 e o que
    # leva o pico do brilho a L 0,86 depois do AgX (0,55 dava cinza).
    "forca_mundo": 1.8,
    # Forca da versao que ILUMINA (probe), como fracao de forca_mundo. 1,0 =
    # o cromo reflete exatamente o ceu que a camera ve; a protecao contra a
    # inundacao e o preto abaixo do horizonte, nao este numero.
    "forca_luz": 1.0,
    # O AgX empalidece cor clara: o rose #F4E6E4 saia cinza (saturacao 0,04
    # com 1,6). Compensa na cor do world, nao na paleta (a mesma licao da logo
    # no modulo caixa). 6,0 e o medido para ~0,10 de saturacao no PNG.
    "saturacao_clara": 6.0,
    # Amplitude do ruido no fator do ramp. 0,0015 ainda dava degrau de 3,8
    # niveis entre linhas vizinhas na parte ingreme da curva.
    "dither": 0.0005,
    "escala_dither": 3000.0,  # grao do ruido: abaixo do pixel a 1920 de altura
    # Chao
    "tamanho_chao": 400.0,
    "cor_chao": "#08080A",
    # Reflexo SUTIL: rugosidade 0,45 e especular 0,15 - com 0,35/0,2 as area
    # lights ainda viravam pocas brancas no chao (luminancia media 0,66
    # medida na frente; a meta e o chao escuro, com o produto "sentado").
    "rugosidade_chao": 0.45,
    "especular_chao": 0.15,
    # Onde o chao comeca a virar o brilho do horizonte e onde termina (m da
    # origem). A camera nunca passa de ~4 m da origem.
    "fusao_chao": (6.0, 30.0),
    # Luzes: posicao (m), tamanho (x, y em m), energia (W), cor. As posicoes
    # sao relativas ao rig, um Empty na origem que a coreografia gira junto
    # com a orbita - o rim so recorta a silhueta se ficar ATRAS do produto do
    # ponto de vista da camera, e "atras" muda a cada quadro numa orbita.
    "alvo_luzes": (0.0, 0.0, 0.42),
    "luzes": {
        # 'abertura' e o spread da area light, em graus: e a colmeia do softbox.
        # 180 (padrao) espalha luz de lado e enche o chao de reflexo; o rim
        # com 40 graus recorta a silhueta sem pintar o chao na frente.
        # 'especular' e o multiplicador de especular da luz (so EEVEE): abaixo
        # de 1 a luz continua iluminando o difuso e reflete menos no chao.
        "key":  {"pos": (2.2, -2.4, 2.8), "tam": (2.0, 2.0), "energia": 350.0, "cor": (1.0, 0.95, 0.90), "abertura": 100.0},
        "fill": {"pos": (-3.0, -2.0, 1.6), "tam": (3.0, 3.0), "energia": 110.0, "cor": (0.90, 0.94, 1.0), "abertura": 120.0},
        # O rim fica BAIXO (na altura do produto) e longe. O ponto onde o
        # reflexo dele cai no chao esta a t = z_cam / (z_cam + z_luz) do
        # caminho camera -> luz: com a camera a 0,75 m e o rim a 2,9 m era
        # t = 0,21, uma poca branca a 1,9 m NA FRENTE do produto (quanto mais
        # alto o rim, mais perto da camera cai a poca - o raciocinio anterior
        # estava invertido). A 1,0 m, t = 0,43 e a poca cai em y ~ -0,5, sob a
        # silhueta do produto: e o brilho no chao atras dele, o look Apple.
        # especular 0,6: o reflexo do rim no chao atras do produto era uma
        # cunha branca na camera de cima; 0,6 o deixa como brilho, e o recorte
        # no cromo e na aresta do cubo (que estava estourado) segue la.
        "rim":  {"pos": (0.6, 2.8, 1.0), "tam": (0.3, 2.2), "energia": 350.0, "cor": (1.0, 1.0, 1.0), "abertura": 40.0, "especular": 0.6},
        "top":  {"pos": (0.0, 0.3, 3.6), "tam": (3.0, 3.0), "energia": 80.0, "cor": (1.0, 0.98, 0.96), "abertura": 100.0},
    },
    # Flash
    "forca_flash": 16.0,      # emissao branca; com AgX, 1.0 nao chega ao branco
    "distancia_flash": 0.25,  # do sensor; acima do clip_start padrao (0,1)
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
    saida.location = (900, 0)

    # No world, 'Generated' e a direcao de visada normalizada, em coordenadas
    # do mundo: o Z dela e o seno da elevacao. E a unica coordenada que nao
    # depende de onde a camera esta - so de para onde o raio aponta.
    coord = nt.nodes.new("ShaderNodeTexCoord")
    coord.location = (-900, 0)
    sep = nt.nodes.new("ShaderNodeSeparateXYZ")
    sep.location = (-700, 0)
    nt.links.new(coord.outputs["Generated"], sep.inputs["Vector"])

    # O fator comeca 5,7 graus abaixo do horizonte (-0,10) para o brilho
    # continuar por baixo dele - e la que o chao infinito funde no ceu.
    faixa = nt.nodes.new("ShaderNodeMapRange")
    faixa.location = (-500, 0)
    faixa.inputs["From Min"].default_value = -0.10
    faixa.inputs["From Max"].default_value = 1.0
    faixa.inputs["To Min"].default_value = 0.0
    faixa.inputs["To Max"].default_value = 1.0
    nt.links.new(sep.outputs["Z"], faixa.inputs["Value"])

    # Dither: ruido fino sobre a direcao, somado ao fator antes do ramp.
    escala = nt.nodes.new("ShaderNodeVectorMath")
    escala.operation = "SCALE"
    escala.location = (-700, -300)
    escala.inputs["Scale"].default_value = p["escala_dither"]
    nt.links.new(coord.outputs["Generated"], escala.inputs[0])
    ruido = nt.nodes.new("ShaderNodeTexNoise")
    ruido.location = (-500, -300)
    ruido.inputs["Scale"].default_value = 1.0
    _entrada(ruido, "Detail", 0.0)
    _entrada(ruido, "Roughness", 0.0)
    nt.links.new(escala.outputs["Vector"], ruido.inputs["Vector"])
    amplitude = nt.nodes.new("ShaderNodeMapRange")
    amplitude.location = (-300, -300)
    amplitude.inputs["From Min"].default_value = 0.35
    amplitude.inputs["From Max"].default_value = 0.65
    amplitude.inputs["To Min"].default_value = -p["dither"]
    amplitude.inputs["To Max"].default_value = p["dither"]
    nt.links.new(ruido.outputs["Fac"], amplitude.inputs["Value"])
    soma = nt.nodes.new("ShaderNodeMath")
    soma.operation = "ADD"
    soma.location = (-100, 0)
    nt.links.new(faixa.outputs["Result"], soma.inputs[0])
    nt.links.new(amplitude.outputs["Result"], soma.inputs[1])

    # Uma so rampa preto -> rose faz a cor; a curva de mistura por elevacao
    # fica numa segunda rampa em escala de cinza, para a cor e a forma do
    # brilho serem ajustaveis separadas.
    forma = nt.nodes.new("ShaderNodeValToRGB")
    forma.location = (100, 0)
    forma.color_ramp.interpolation = "EASE"
    pontos = forma.color_ramp.elements
    while len(pontos) > 1:
        pontos.remove(pontos[-1])
    pontos[0].position = p["curva"][0][0]
    pontos[0].color = (p["curva"][0][1],) * 3 + (1.0,)
    for pos, mist in p["curva"][1:]:
        e = pontos.new(pos)
        e.color = (mist,) * 3 + (1.0,)
    nt.links.new(soma.outputs["Value"], forma.inputs["Fac"])

    cor = nt.nodes.new("ShaderNodeMix")
    cor.data_type = "RGBA"
    cor.location = (320, 0)
    cor.inputs["A"].default_value = cor_linear(p["cor_escura"])
    cor.inputs["B"].default_value = saturar(cor_linear(p["cor_clara"]), p["saturacao_clara"])
    nt.links.new(forma.outputs["Color"], cor.inputs["Factor"])

    # Background da CAMERA: o gradiente inteiro.
    fundo_cam = nt.nodes.new("ShaderNodeBackground")
    fundo_cam.location = (500, 100)
    fundo_cam.inputs["Strength"].default_value = p["forca_mundo"]
    nt.links.new(cor.outputs["Result"], fundo_cam.inputs["Color"])

    # Background da ILUMINACAO: o mesmo gradiente, apagado abaixo do horizonte
    # (ver cabecalho: o probe nao e ocluido pelo chao, e um hemisferio de rose
    # forte iluminaria tudo por baixo). A rampa de -0,04 a 0,0 e curta para a
    # versao da luz ser identica a da camera em toda a faixa que o cromo
    # reflete acima do horizonte.
    mascara = nt.nodes.new("ShaderNodeMapRange")
    mascara.location = (-500, 300)
    mascara.inputs["From Min"].default_value = -0.04
    mascara.inputs["From Max"].default_value = 0.0
    mascara.inputs["To Min"].default_value = 0.0
    mascara.inputs["To Max"].default_value = 1.0
    nt.links.new(sep.outputs["Z"], mascara.inputs["Value"])
    forca_luz = nt.nodes.new("ShaderNodeMath")
    forca_luz.operation = "MULTIPLY"
    forca_luz.location = (320, 300)
    forca_luz.inputs[1].default_value = p["forca_mundo"] * p["forca_luz"]
    nt.links.new(mascara.outputs["Result"], forca_luz.inputs[0])
    fundo_luz = nt.nodes.new("ShaderNodeBackground")
    fundo_luz.location = (500, 300)
    nt.links.new(cor.outputs["Result"], fundo_luz.inputs["Color"])
    nt.links.new(forca_luz.outputs["Value"], fundo_luz.inputs["Strength"])

    # Is Camera Ray: 1 no render pela camera, 0 no probe do world (EEVEE e
    # Cycles). Se o no faltar em alguma versao, fica so a versao da camera -
    # a cena inunda, mas nao fica preta.
    try:
        caminho = nt.nodes.new("ShaderNodeLightPath")
        caminho.location = (500, -200)
        mistura = nt.nodes.new("ShaderNodeMixShader")
        mistura.location = (700, 0)
        nt.links.new(caminho.outputs["Is Camera Ray"], mistura.inputs["Fac"])
        nt.links.new(fundo_luz.outputs["Background"], mistura.inputs[1])
        nt.links.new(fundo_cam.outputs["Background"], mistura.inputs[2])
        nt.links.new(mistura.outputs["Shader"], saida.inputs["Surface"])
    except (RuntimeError, KeyError) as e:
        print("[ambiente] sem Light Path no world, iluminacao = camera:", e)
        nt.links.new(fundo_cam.outputs["Background"], saida.inputs["Surface"])
    return mundo


# ---------------------------------------------------------------- chao

def cor_horizonte(p):
    """Cor linear do world na elevacao zero, ja com a forca - o que o chao copia.

    Com a curva padrao (mistura 1,0 no horizonte) e a propria cor do brilho:
    e assim que o chao infinito funde no brilho, sem faixa escura entre os dois.
    """
    escura = cor_linear(p["cor_escura"])
    clara = saturar(cor_linear(p["cor_clara"]), p["saturacao_clara"])
    m = p["curva"][0][1]
    return tuple((escura[i] * (1 - m) + clara[i] * m) * p["forca_mundo"] for i in range(3)) + (1.0,)


def _chao(col, p):
    nome = NOME + ".chao"
    t = p["tamanho_chao"] / 2.0
    malha = bpy.data.meshes.new(nome)
    malha.from_pydata([(-t, -t, 0), (t, -t, 0), (t, t, 0), (-t, t, 0)], [], [(0, 1, 2, 3)])
    malha.update()
    obj = bpy.data.objects.new(nome, malha)
    col.objects.link(obj)

    mat, nt, bsdf = _material(nome)
    _entrada(bsdf, "Base Color", cor_linear(p["cor_chao"]))
    _entrada(bsdf, "Metallic", 0.0)

    # Chao infinito: 'fusao' vai de 0 (perto, chao de verdade) a 1 (longe,
    # emissao com a cor do ceu no horizonte, sem especular). Coordenada de
    # objeto e nao de camera, para o resultado nao depender de onde a camera
    # esta - a orbita passa por todo lado.
    coord = nt.nodes.new("ShaderNodeTexCoord")
    coord.location = (-900, 300)
    comp = nt.nodes.new("ShaderNodeVectorMath")
    comp.operation = "LENGTH"
    comp.location = (-700, 300)
    nt.links.new(coord.outputs["Object"], comp.inputs[0])
    fusao = nt.nodes.new("ShaderNodeMapRange")
    fusao.location = (-500, 300)
    fusao.inputs["From Min"].default_value = p["fusao_chao"][0]
    fusao.inputs["From Max"].default_value = p["fusao_chao"][1]
    fusao.inputs["To Min"].default_value = 0.0
    fusao.inputs["To Max"].default_value = 1.0
    try:
        fusao.interpolation_type = "SMOOTHSTEP"
    except AttributeError:
        pass
    nt.links.new(comp.outputs["Value"], fusao.inputs["Value"])

    rug = nt.nodes.new("ShaderNodeMapRange")
    rug.location = (-300, 400)
    rug.inputs["To Min"].default_value = p["rugosidade_chao"]
    rug.inputs["To Max"].default_value = 1.0
    nt.links.new(fusao.outputs["Result"], rug.inputs["Value"])
    nt.links.new(rug.outputs["Result"], bsdf.inputs["Roughness"])

    esp = nt.nodes.new("ShaderNodeMapRange")
    esp.location = (-300, 200)
    esp.inputs["To Min"].default_value = p["especular_chao"]
    esp.inputs["To Max"].default_value = 0.0
    nt.links.new(fusao.outputs["Result"], esp.inputs["Value"])
    if bsdf.inputs.get("Specular IOR Level") is not None:
        nt.links.new(esp.outputs["Result"], bsdf.inputs["Specular IOR Level"])

    _entrada(bsdf, "Emission Color", cor_horizonte(p))
    nt.links.new(fusao.outputs["Result"], bsdf.inputs["Emission Strength"])
    malha.materials.append(mat)
    return obj


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
    """Cria world, chao, rig com 4 luzes na sub-colecao 'ambiente'."""
    p = _mesclar(PARAMS_PADRAO, params)
    limpar_colecao(NOME)
    col = _colecao(cena, colecao_pai, NOME)

    mundo = _mundo(p)
    cena.world = mundo
    chao = _chao(col, p)

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
        "chao": chao,
        "rig": rig,
        "alvo_luzes": alvo,
        "luzes": luzes,
        "flash": None,
        "cor_escura": p["cor_escura"],
        "cor_clara": p["cor_clara"],
        "z_chao": 0.0,
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


def _bloom(cena, p):
    """Glare/bloom no compositor. Os nos mudam de nome e de forma entre versoes."""
    cena.use_nodes = True
    nt = cena.node_tree
    for no in list(nt.nodes):
        nt.nodes.remove(no)
    camadas = nt.nodes.new("CompositorNodeRLayers")
    camadas.location = (-400, 0)
    saida = nt.nodes.new("CompositorNodeComposite")
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
            cena.use_nodes = False
    else:
        cena.use_nodes = False
    return p


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


def animar_flash(objs, camera, quadro, forca=1.0, largura=1):
    """Flash de foto: veu branco parented na camera, alfa 0 -> 1 -> 0 em 3 quadros.

    'forca' multiplica a emissao (1.0 = branco estourado com AgX); 'largura' e
    quantos quadros de cada lado do pico (1 = os 3 quadros pedidos).
    """
    p = objs.get("params", PARAMS_PADRAO)
    obj = _plano_flash(objs, camera, p["forca_flash"] * forca, p["distancia_flash"])
    mat = obj.data.materials[0]
    nt = mat.node_tree
    alfa = nt.nodes["alfa"].outputs[0]
    emissao = next(n for n in nt.nodes if n.type == "EMISSION").inputs["Strength"]
    # Forca por flash: a coreografia pode pedir um flash mais fraco no beat 5
    # sem trocar o material.
    for q, a in ((quadro - largura, 0.0), (quadro, 1.0), (quadro + largura, 0.0)):
        alfa.default_value = a
        alfa.keyframe_insert("default_value", frame=q)
        emissao.default_value = p["forca_flash"] * forca
        emissao.keyframe_insert("default_value", frame=q)
    # Linear e nao Bezier: com so tres chaves, o Bezier passaria de 1.0 no
    # pico e daria um quadro extra meio branco de cada lado.
    acao = nt.animation_data.action if nt.animation_data else None
    if acao is not None:
        for fc in acao.fcurves:
            for kp in fc.keyframe_points:
                kp.interpolation = "LINEAR"
    alfa.default_value = 0.0
    return obj


def animar_rig(objs, quadro_ini, quadro_fim, angulo_ini, angulo_fim, easing="EASE_IN_OUT"):
    """Gira o rig das luzes em Z (graus), para o rim acompanhar a orbita da camera."""
    rig = objs["rig"]
    for q, ang in ((quadro_ini, angulo_ini), (quadro_fim, angulo_fim)):
        rig.rotation_euler = (0.0, 0.0, math.radians(ang))
        rig.keyframe_insert("rotation_euler", index=2, frame=q)
    acao = rig.animation_data.action
    for fc in acao.fcurves:
        for kp in fc.keyframe_points:
            kp.interpolation = "BEZIER"
            kp.easing = easing
