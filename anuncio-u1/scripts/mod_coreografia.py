# Modulo COREOGRAFIA do anuncio do Snapmaker U1.
#
# E o integrador: constroi a cena com os outros modulos (ambiente, caixa, u1,
# cabo, cartela, camera) e escreve os sete beats do storyboard sobre eles.
# So definicoes aqui - nada roda no import. Quem prova e teste_coreografia.py;
# o arquivo unico (anuncio_u1.py, gerado por montar.py) chama construir_tudo,
# coreografar e configurar_render.
#
# DECISOES:
#
# - A LINHA DO TEMPO E DADO. A tabela BEATS (segundos) e a da especificacao;
#   ROTEIRO da a fracao de cada beat em que cada acao acontece. Todo quadro
#   sai de quadro(t, fator): mudar a duracao (preset de 15 s = fator 0,75)
#   escala tudo junto, inclusive as folgas anti-colisao, que sao razoes.
#
# - A CAIXA DESCE E SOME no beat 2 (em vez de o U1 pousar ao lado dela). O
#   motivo e o beat 3: a camera orbita 180 graus ate a traseira para ver o
#   cabo entrar numa tomada a 12 cm do chao. Com a caixa de 0,8 m atras do
#   U1 (ou ao lado), ela entraria entre a camera e a tomada em parte da
#   orbita. Com o U1 sozinho na origem, a orbita e os closes do beat 5 tem
#   360 graus livres, e o rig de luzes (centrado na origem) continua certo.
#   No beat 6 a caixa volta pelo chao enquanto o U1 flutua acima dela - e o
#   mesmo truque ao contrario, e a ordem (U1 sobe, caixa sobe, U1 desce,
#   espuma volta, tampa fecha) e a que nao atravessa nada: conferir_colisoes
#   mede isso quadro a quadro.
#
# - A CAMERA anda num Empty 'camera.orbita' na origem: as chaves sao
#   (azimute, raio, altura) em vez de XYZ. Orbita vira uma rampa de angulo
#   (arco exato, nao poligono), dolly vira uma rampa de raio, e as duas se
#   misturam com Bezier continuo entre beats - a camera nunca para seca:
#   entre um beat e outro a chave e uma so, e as chaves de Bezier
#   auto-clamped nao zeram a velocidade no meio de um trecho. Os cortes do
#   beat 5 e o corte para a cartela sao chaves CONSTANT no ultimo quadro do
#   plano anterior, e o obturador do motion blur abre em START (nao CENTER),
#   para o quadro do corte nao borrar entre dois planos.
#
# - O FOCO tem Empty proprio ('camera.foco'): nos beats 1-6 ele copia o alvo
#   da camera (foco segue o que a camera olha); no beat 7 o alvo fica 1 m
#   abaixo da camera (para o Track To nao degenerar olhando reto para baixo)
#   e o foco fica na logo; na cartela o foco vai para a cartela.
#
# - O que nao deve aparecer e ESCONDIDO por chave de hide_render: o cabo
#   antes de entrar (estaria deitado no chao atras do U1 desde o quadro 1), a
#   tampa enquanto flutua a 1,6 m ao lado (apareceria nas orbitas), a cartela
#   antes do corte. E o que o modulo de cada peca nao oferece.
#
# Eixos e medidas seguem docs/ESPECIFICACAO.md: metros, Z para cima, frente
# em -Y, origem no centro da base da caixa, chao em z = 0.

import math
import os

import bpy
from mathutils import Matrix, Vector

import mod_ambiente
import mod_cabo
import mod_caixa
import mod_cartela
import mod_u1

NOME = "coreografia"
FPS = 30.0
DURACAO_REFERENCIA = 20.0

# Tabela de beats da especificacao (segundos, na duracao de referencia).
BEATS = (
    {"n": 1, "nome": "caixa_sobe", "t_ini": 0.0, "t_fim": 2.5},
    {"n": 2, "nome": "abre", "t_ini": 2.5, "t_fim": 5.5},
    {"n": 3, "nome": "traseira", "t_ini": 5.5, "t_fim": 9.0},
    {"n": 4, "nome": "tela", "t_ini": 9.0, "t_fim": 12.0},
    {"n": 5, "nome": "fotos", "t_ini": 12.0, "t_fim": 15.0},
    {"n": 6, "nome": "volta", "t_ini": 15.0, "t_fim": 17.0},
    {"n": 7, "nome": "cartela", "t_ini": 17.0, "t_fim": 20.0},
)
PRESETS = {"20s": 1.0, "15s": 0.75}

# Fracao de cada beat (0 = inicio, 1 = fim) em que cada acao comeca e acaba.
# A ordem dentro dos beats 2 e 6 e a que nao atravessa nada - ver cabecalho.
ROTEIRO = {
    2: {
        "tampa": (0.00, 0.30),          # tampa sai rapido, antes da espuma
        "espuma": (0.22, 0.85),         # explode depois que a tampa saiu de cima
        "u1_sobe": (0.50, 0.75),        # so depois de toda espuma ter saltado
        "caixa_desce": (0.62, 0.92),    # a caixa some pelo chao sob o U1
        "u1_desce": (0.80, 1.00),       # U1 pousa no chao, na origem
    },
    3: {
        "orbita": (0.00, 0.48),         # frente -> traseira pelo lado +X
        "cabo": (0.19, 0.71),
        "botao": (0.77, 0.98),
    },
    4: {
        "orbita": (0.00, 0.56),         # traseira -> frente pelo lado -X
        "boot": 0.58,                   # boot de ~0,8 s, corte seco para a UI
        "ui": 0.84,
    },
    5: {"fotos": (0.0, 1.0 / 3.0, 2.0 / 3.0)},
    6: {
        "u1_sobe": (0.00, 0.267),
        "caixa_sobe": (0.033, 0.333),
        "u1_desce": (0.40, 0.667),
        "espuma": (0.60, 0.867),
        "tampa": (0.80, 1.00),
    },
    7: {
        "sobe_para_logo": (0.00, 0.244),
        "mergulho": (0.244, 0.444),     # quadro da travessia = fim do mergulho
        "cartela": (0.456, 0.833),
    },
}

PARAMS_PADRAO = {
    # '' = U1 substituto; nome de objeto ou de colecao = modelo real do cliente.
    "u1_nome": "",
    "duracao_s": DURACAO_REFERENCIA,
    "cor_caixa": "clara",
    "pasta_assets": None,          # None = <raiz do projeto>/assets
    # Modelo real: rotacao em Z (graus) que poe a frente dele em -Y, e os
    # pontos de tela/tomada/botao nas coordenadas ORIGINAIS do arquivo dele
    # (antes de centralizar); None = heuristica pelo bounding box.
    "u1_rotacao_z": 0.0,
    "u1_tela": None,
    "u1_tomada": None,
    "u1_botao": None,
    # Objetos do modelo real que recebem animacao (nomes); '' = nao anima.
    "u1_tela_objeto": "",
    "u1_botao_objeto": "",
    "u1_led_objeto": "",
    "u1": {},                      # params extras do mod_u1 (cor_corpo...)
    "ambiente": {},
    "camera": {},
    "cartela": {},
    "caixa": {},
    # Quanto o U1 sobe acima do topo do corpo da caixa ao sair/entrar.
    "folga_u1": 0.14,
    # Deslocamento da cartela para cima no quadro (m a 2 m): tira a linha 4
    # da zona de legendas do Reels e a logo do brilho do horizonte.
    "cartela_subida": 0.13,
}


# ---------------------------------------------------------------- tempo

def fator_duracao(duracao_s):
    return float(duracao_s) / DURACAO_REFERENCIA


def quadro(t, fator=1.0):
    """Segundo (na referencia de 20 s) -> quadro, com o fator de duracao."""
    return max(1, int(round(t * FPS * fator)))


def quadros_do_beat(n, fator=1.0):
    b = BEATS[n - 1]
    return quadro(b["t_ini"], fator), quadro(b["t_fim"], fator)


def q_em(n, fracao, fator=1.0):
    """Quadro na fracao 'fracao' do beat n."""
    a, b = quadros_do_beat(n, fator)
    return int(round(a + fracao * (b - a)))


def quadros_chave(fator=1.0):
    """Um quadro por beat (o meio de cada um), para a previa."""
    return [(b["n"], q_em(b["n"], 0.5, fator)) for b in BEATS]


# ---------------------------------------------------------------- utilidades

def _raiz_projeto():
    try:
        return os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    except NameError:
        # Aba Scripting do Blender sem arquivo: vale a pasta do .blend.
        return os.path.dirname(bpy.data.filepath) if bpy.data.filepath else os.getcwd()


def _asset(p, nome):
    pasta = p.get("pasta_assets") or os.path.join(_raiz_projeto(), "assets")
    return os.path.join(pasta, nome)


def _colecao_raiz(cena):
    col = bpy.data.collections.get("ANUNCIO")
    if col is None:
        col = bpy.data.collections.new("ANUNCIO")
    if col.name not in cena.collection.children:
        cena.collection.children.link(col)
    return col


def _chave(obj, quadro_, loc=None, rot=None):
    if loc is not None:
        obj.location = loc
        obj.keyframe_insert("location", frame=quadro_)
    if rot is not None:
        obj.rotation_euler = rot
        obj.keyframe_insert("rotation_euler", frame=quadro_)


def _interpolar(dono, q_ini, q_fim, interp="BEZIER", easing="EASE_IN_OUT", canais=None):
    """Interpolacao/easing so nas chaves do intervalo (nao mexe em outro beat)."""
    ad = getattr(dono, "animation_data", None)
    if ad is None or ad.action is None:
        return
    for fc in ad.action.fcurves:
        if canais is not None and fc.data_path not in canais:
            continue
        for kp in fc.keyframe_points:
            if q_ini - 0.5 <= kp.co.x <= q_fim + 0.5:
                kp.interpolation = interp
                kp.easing = easing
                if interp == "BEZIER":
                    kp.handle_left_type = "AUTO_CLAMPED"
                    kp.handle_right_type = "AUTO_CLAMPED"
        fc.update()


def _chave_rim_especular(objs, quadro_, valor):
    """specular_factor do rim com chave constante: 0,0 nos planos largos, 0,5
    nos planos do produto (que esconde o ponto de espelho no chao).

    Medido no quadro 1 (chao vazio): o reflexo do painel do rim no chao e uma
    barra branca com rastro ate o pe do quadro; esconder o rim ou zerar o
    especular do chao a apaga, e 0,2 e 0,05 so a escurecem um pouco - e um
    painel de 350 W espelhado em Fresnel rasante, ~100x acima do branco, e
    so o zero resolve. As outras tres luzes espelham fora do quadro.
    """
    dados = objs["ambiente"]["luzes"]["rim"].data
    dados.specular_factor = valor
    try:
        dados.keyframe_insert("specular_factor", frame=quadro_)
    except (RuntimeError, TypeError):
        # Versao sem a propriedade animavel: fica o valor dos planos largos.
        dados.specular_factor = 0.0
        return
    for fc in dados.animation_data.action.fcurves:
        if fc.data_path == "specular_factor":
            for kp in fc.keyframe_points:
                kp.interpolation = "CONSTANT"
            fc.update()


def _chave_visivel(obj, quadro_, visivel):
    """hide_render com chave (booleano: a chave ja e constante). So o render:
    chavear hide_viewport reconstroi as relacoes do depsgraph a cada chave e
    derrubou o Blender 4.2 (segfault) ao iterar a colecao do cabo."""
    obj.hide_render = not visivel
    obj.keyframe_insert("hide_render", frame=quadro_)


def _esconder_entre(objetos, q_some, q_volta, q_primeiro=1):
    """Visivel ate q_some-1, escondido de q_some a q_volta-1, visivel de
    q_volta. Devolve os objetos que receberam chave (os ja escondidos pelo
    modulo - cortadores de boolean - ficam como estao)."""
    tocados = []
    for obj in list(objetos):
        if obj.hide_render:
            continue
        if q_some > q_primeiro:
            _chave_visivel(obj, q_primeiro, True)
        _chave_visivel(obj, q_some, False)
        if q_volta is not None:
            _chave_visivel(obj, q_volta, True)
        tocados.append(obj)
    return tocados


def _ajustar(objeto, nome, valor):
    try:
        setattr(objeto, nome, valor)
        return True
    except (AttributeError, TypeError, ValueError):
        return False


# ---------------------------------------------------------------- U1 real

def _bbox_mundo(objetos):
    dg = bpy.context.evaluated_depsgraph_get()
    mn = Vector((1e9, 1e9, 1e9))
    mx = Vector((-1e9, -1e9, -1e9))
    for obj in objetos:
        if obj.hide_render or obj.type not in ("MESH", "CURVE", "FONT", "SURFACE", "META"):
            continue
        ev = obj.evaluated_get(dg)
        for canto in ev.bound_box:
            w = ev.matrix_world @ Vector(canto)
            for i in range(3):
                mn[i] = min(mn[i], w[i])
                mx[i] = max(mx[i], w[i])
    return mn, mx


def _u1_real(cena, col_pai, p):
    """Modelo do cliente por nome (objeto ou colecao). Devolve o mesmo dict
    que construir_u1, com 'real': True, ou None se o nome nao existe.

    O que faz: cria (ou reaproveita) o Empty 'u1.raiz', parenteia nele os
    objetos de topo do modelo mantendo a pose, aplica 'u1_rotacao_z', mede o
    bounding box avaliado e move a raiz para o modelo ficar centrado em XY com
    a base em z = 0 - a mesma pose do substituto, que e o que a caixa, o cabo
    e a camera esperam. Os pontos de tela/tomada/botao vem de params (nas
    coordenadas originais do arquivo dele, levadas pela mesma matriz) ou de
    uma heuristica pelo bounding box, documentada em _pontos_heuristicos.
    """
    nome = p["u1_nome"]
    fontes = []
    col_modelo = bpy.data.collections.get(nome)
    if col_modelo is not None:
        todos = set(o.name for o in col_modelo.all_objects)
        fontes = [o for o in col_modelo.all_objects if o.parent is None or o.parent.name not in todos]
        col = col_modelo
    elif nome in bpy.data.objects:
        fontes = [bpy.data.objects[nome]]
        col = bpy.data.collections.get("u1")
        if col is None:
            col = bpy.data.collections.new("u1")
            col_pai.children.link(col)
    else:
        return None

    raiz = bpy.data.objects.get("u1.raiz")
    if raiz is None or raiz.type != "EMPTY":
        raiz = bpy.data.objects.new("u1.raiz", None)
        raiz.empty_display_type = "ARROWS"
        raiz.empty_display_size = 0.2
        col.objects.link(raiz)
    raiz.parent = None
    raiz.matrix_world = Matrix.Identity(4)
    if raiz.animation_data:
        raiz.animation_data_clear()
    # matrix_world de objeto recem-criado ou recem-movido so e valido depois
    # de uma avaliacao (medido: sem isto a rotacao do bloco de teste saiu 0).
    bpy.context.view_layer.update()
    fontes = [o for o in fontes if o is not raiz]
    originais = {obj: obj.matrix_world.copy() for obj in fontes}
    # A rotacao e a centralizacao ficam COZIDAS nos filhos, e a raiz fica na
    # identidade: as chaves da coreografia escrevem a raiz em valores
    # absolutos (0, 0, z), iguais para o substituto e para o modelo real.
    rz = Matrix.Rotation(math.radians(p["u1_rotacao_z"]), 4, "Z")
    for obj in fontes:
        obj.parent = raiz
        obj.matrix_parent_inverse = Matrix.Identity(4)
        obj.matrix_world = rz @ originais[obj]
    bpy.context.view_layer.update()
    filhos = [o for o in bpy.data.objects if _descende(o, raiz)]
    mn, mx = _bbox_mundo(filhos)
    centro = (mn + mx) / 2.0
    m = Matrix.Translation((-centro.x, -centro.y, -mn.z)) @ rz
    for obj in fontes:
        obj.matrix_world = m @ originais[obj]
    bpy.context.view_layer.update()
    mn, mx = _bbox_mundo(filhos)
    dims = (mx.x - mn.x, mx.y - mn.y, mx.z - mn.z)
    print("[coreografia] modelo real '%s': %d objetos, envelope %.3f x %.3f x %.3f m" % (nome, len(filhos), *dims))

    def ponto(chave, heuristico):
        # Pontos dados nas coordenadas ORIGINAIS do arquivo do cliente vao
        # pela mesma matriz que levou a malha.
        v = p.get(chave)
        return (m @ Vector(v)) if v is not None else heuristico

    h = _pontos_heuristicos(dims)
    tela_obj = bpy.data.objects.get(p["u1_tela_objeto"]) if p["u1_tela_objeto"] else None
    botao_obj = bpy.data.objects.get(p["u1_botao_objeto"]) if p["u1_botao_objeto"] else None
    led_obj = bpy.data.objects.get(p["u1_led_objeto"]) if p["u1_led_objeto"] else None
    return {
        "raiz": raiz,
        "real": True,
        "tela": tela_obj,
        "botao": botao_obj,
        "led": led_obj,
        "cabecotes": [],
        "colecao": col,
        "dimensoes": dims,
        "dimensoes_nominais": (mod_u1.LARGURA, mod_u1.PROFUNDIDADE, mod_u1.ALTURA),
        "envelope": (mn, mx),
        "placeholders": {"boot": False, "ui": False},
        "posicao_tela": {"centro": ponto("u1_tela", h["tela"]), "normal": Vector((0, -1, 0))},
        "posicao_tomada": {"ponto": ponto("u1_tomada", h["tomada"]), "direcao": Vector((0, -1, 0)), "normal": Vector((0, 1, 0))},
        "posicao_botao": {"centro": ponto("u1_botao", h["botao"]), "normal": Vector((0, 1, 0))},
        "botao_afunda_local": Vector((0, -1, 0)),
        "materiais": {},
    }


def _descende(obj, raiz):
    o = obj.parent
    while o is not None:
        if o is raiz:
            return True
        o = o.parent
    return False


def _pontos_heuristicos(dims):
    """Onde ficam tela, tomada e botao num U1 de dimensoes 'dims', em fracao do
    envelope, medidas no substituto (que seguiu as fotos): tela no canto
    superior direito da frente (x = +0,30 L, z = 0,80 A), tomada e botao na
    coluna traseira direita (x = +0,42 L; z = 0,17 A e 0,24 A)."""
    L, P, A = dims
    return {
        "tela": Vector((0.30 * L, -P / 2.0, 0.80 * A)),
        "tomada": Vector((0.42 * L, P / 2.0, 0.17 * A)),
        "botao": Vector((0.42 * L, P / 2.0, 0.24 * A)),
    }


# ---------------------------------------------------------------- construir

def construir_tudo(params=None):
    """Colecao ANUNCIO com ambiente, caixa, U1 (substituto ou real), cabo,
    cartela e camera. Devolve o dict que coreografar e configurar_render usam."""
    p = dict(PARAMS_PADRAO)
    if params:
        p.update(params)
    cena = bpy.context.scene
    col = _colecao_raiz(cena)

    # Rim com especular 0,5 (padrao do modulo: 0,6) nos beats 3-5; nos
    # planos largos (beats 1, 2, 6, 7) a coreografia CHAVEIA o specular_factor
    # do rim para 0,05 (ver _beat1/_beat3/_beat6): o reflexo dele no chao cai
    # no eixo da camera e saia como uma barra branca no horizonte com um
    # rastro ate o pe do quadro - no quadro 1, antes de a caixa emergir, era a
    # imagem inteira. Medido: 0,4 e 0,2 so encolhem a barra (e Fresnel
    # rasante, nao intensidade). As outras tres luzes espelham fora do
    # quadro; o recorte da aresta vem do difuso, que fica.
    pamb = {"luzes": {"rim": {"especular": 0.5}}}
    for k, v in p["ambiente"].items():
        if k == "luzes":
            for luz, cfg in v.items():
                pamb["luzes"].setdefault(luz, {}).update(cfg)
        else:
            pamb[k] = v
    amb = mod_ambiente.construir_ambiente(cena, col, pamb)

    u1 = None
    if p["u1_nome"]:
        u1 = _u1_real(cena, col, p)
        if u1 is None:
            print("[coreografia] AVISO: '%s' nao existe em bpy.data; usando o U1 substituto" % p["u1_nome"])
    if u1 is None:
        pu1 = {"imagem_boot": _asset(p, "tela_boot.png"), "imagem_ui": _asset(p, "tela_ui.png")}
        pu1.update(p["u1"])
        u1 = mod_u1.construir_u1(cena, col, pu1)
        u1["real"] = False

    pcaixa = {"cor": p["cor_caixa"], "logo": _asset(p, "logo_engineprint.png"), "u1": tuple(u1["dimensoes"])}
    pcaixa.update(p["caixa"])
    caixa = mod_caixa.construir_caixa(cena, col, pcaixa)
    ix, iy, iz = caixa["interior"]
    if u1["dimensoes"][0] > ix or u1["dimensoes"][1] > iy or u1["dimensoes"][2] > iz:
        print("[coreografia] AVISO: o U1 (%.3f x %.3f x %.3f) nao cabe no interior da caixa (%.3f x %.3f x %.3f)"
              % (tuple(u1["dimensoes"]) + (ix, iy, iz)))

    # O U1 nasce dentro da caixa, apoiado no fundo (uma parede acima do chao).
    parede = mod_caixa.PARAMS_PADRAO["parede"]
    u1["raiz"].location = Vector((0.0, 0.0, parede))
    u1["z_na_caixa"] = parede

    # O cabo e construido encaixado na tomada com o U1 no CHAO (beat 3): os
    # pontos do dict foram medidos com a raiz na identidade, que e essa pose.
    tomada = u1["posicao_tomada"]
    pcabo = {"ponto_tomada": tuple(tomada["ponto"]), "direcao_entrada": tuple(tomada["direcao"]),
             "z_chao": 0.0, "penetracao": -mod_cabo.BICO[4]}
    cabo = mod_cabo.construir_cabo(cena, col, pcabo)

    # Forca 1,8/2,2: com 1,0 o "branco" media 0,82 sRGB no render (pendencia
    # do modulo cartela); bloco mais compacto (logo 0,24, entrelinha 3 menor)
    # para a linha 4 subir acima da faixa de legendas do Reels.
    pcart = {"logo": _asset(p, "logo_engineprint.png"), "forca_texto": 1.8, "forca_destaque": 2.2,
             "largura_logo": 0.24, "entrelinhas": (1.30, 1.45, 1.55)}
    pcart.update(p["cartela"])
    cartela = mod_cartela.construir_cartela(cena, col, pcart)

    cam, alvo = mod_ambiente.criar_camera(cena, col, params=p["camera"])
    col_cam = bpy.data.collections.get(mod_ambiente.NOME_CAMERA)
    # Rig da camera: Empty na origem; a camera e filha e as chaves sao
    # (azimute no rig, raio e altura na camera) - ver cabecalho.
    rig = bpy.data.objects.new("camera.orbita", None)
    rig.empty_display_type = "PLAIN_AXES"
    rig.empty_display_size = 0.3
    col_cam.objects.link(rig)
    cam.parent = rig
    cam.matrix_parent_inverse = Matrix.Identity(4)
    foco = bpy.data.objects.new("camera.foco", None)
    foco.empty_display_type = "CUBE"
    foco.empty_display_size = 0.04
    col_cam.objects.link(foco)
    cam.data.dof.focus_object = foco

    return {
        "cena": cena,
        "colecao": col,
        "params": p,
        "fator": fator_duracao(p["duracao_s"]),
        "ambiente": amb,
        "caixa": caixa,
        "u1": u1,
        "cabo": cabo,
        "cartela": cartela,
        "camera": cam,
        "alvo": alvo,
        "foco": foco,
        "rig_camera": rig,
        "_chaves_camera": {},
    }


# ---------------------------------------------------------------- camera

def _cil(pos):
    """XYZ -> (azimute em graus, raio, z) no rig da camera (origem)."""
    x, y, z = pos
    return math.degrees(math.atan2(y, x)), math.hypot(x, y), z


def _chave_camera(objs, q, az, raio, z, alvo, foco=None, lente=None,
                  interp="BEZIER", easing="EASE_IN_OUT"):
    """Uma chave de camera: azimute (graus) no rig, raio e altura na camera,
    alvo do Track To, foco (= alvo se None) e lente. A interpolacao registrada
    vale para o trecho que COMECA nesta chave (CONSTANT = segura ate o corte)."""
    rig, cam = objs["rig_camera"], objs["camera"]
    rig.rotation_euler = (0.0, 0.0, math.radians(az))
    rig.keyframe_insert("rotation_euler", index=2, frame=q)
    cam.location = (raio, 0.0, z)
    cam.keyframe_insert("location", frame=q)
    objs["alvo"].location = Vector(alvo)
    objs["alvo"].keyframe_insert("location", frame=q)
    objs["foco"].location = Vector(alvo if foco is None else foco)
    objs["foco"].keyframe_insert("location", frame=q)
    if lente is not None:
        cam.data.lens = lente
        cam.data.keyframe_insert("lens", frame=q)
    objs["_chaves_camera"][q] = (interp, easing)


def _aplicar_interpolacao_camera(objs):
    registro = objs["_chaves_camera"]
    donos = (objs["rig_camera"], objs["camera"], objs["alvo"], objs["foco"], objs["camera"].data)
    for dono in donos:
        ad = dono.animation_data
        if ad is None or ad.action is None:
            continue
        for fc in ad.action.fcurves:
            for kp in fc.keyframe_points:
                interp, easing = registro.get(int(round(kp.co.x)), ("BEZIER", "EASE_IN_OUT"))
                kp.interpolation = interp
                kp.easing = easing
                if interp == "BEZIER":
                    kp.handle_left_type = "AUTO_CLAMPED"
                    kp.handle_right_type = "AUTO_CLAMPED"
            fc.update()
    # A lente so tem chave nos cortes: entre eles precisa segurar, nao rampar.
    ad = objs["camera"].data.animation_data
    if ad and ad.action:
        for fc in ad.action.fcurves:
            if fc.data_path == "lens":
                for kp in fc.keyframe_points:
                    kp.interpolation = "CONSTANT"
                fc.update()


def _enquadrar(pos_cam, sujeito, lente, fx=0.68, fy=0.74):
    """Alvo que poe 'sujeito' na fracao (fx, fy) do quadro (x para a direita,
    y para baixo, 0,5 = centro): o alvo e o centro do quadro, deslocado no
    plano do sujeito. 9:16 com o sensor de 36 mm no lado maior."""
    pos_cam = Vector(pos_cam)
    sujeito = Vector(sujeito)
    d = sujeito - pos_cam
    dist = d.length
    d.normalize()
    direita = d.cross(Vector((0, 0, 1)))
    if direita.length < 1e-6:
        direita = Vector((1, 0, 0))
    direita.normalize()
    cima = direita.cross(d).normalized()
    meia_altura = dist * 18.0 / lente
    meia_largura = meia_altura * 9.0 / 16.0
    return sujeito - direita * ((fx - 0.5) * 2.0 * meia_largura) + cima * ((fy - 0.5) * 2.0 * meia_altura)


def _sujeitos_fotos(objs):
    """Os tres closes do beat 5: cabecotes, porta/puxador, mesa. Com o
    substituto sao os objetos; com o modelo real, fracoes do envelope."""
    u1 = objs["u1"]
    if u1.get("cabecotes") and u1.get("puxador") is not None and u1.get("mesa") is not None:
        cabs = u1["cabecotes"]
        meio = (cabs[1].matrix_world.translation + cabs[2].matrix_world.translation) / 2.0
        return {
            "cabecotes": meio + Vector((0.0, 0.0, 0.02)),
            "porta": u1["puxador"].matrix_world.translation.copy(),
            "mesa": u1["mesa"].matrix_world.translation.copy(),
        }
    L, P, A = u1["dimensoes"]
    return {
        "cabecotes": Vector((0.0, 0.25 * P, 0.80 * A)),
        "porta": Vector((0.35 * L, -P / 2.0, 0.35 * A)),
        "mesa": Vector((0.0, 0.0, 0.21 * A)),
    }


# ---------------------------------------------------------------- beats

def _beat1(objs, fator):
    """Caixa sobe do chao girando 2 voltas, rapido no inicio e assentando.
    O U1 (dentro) e as espumas vao junto, uma chave por quadro: a espuma esta
    fora do eixo, e so a chave por quadro faz o giro dela ser o mesmo da
    caixa sem parentear (a caixa afunda no beat 2 e a espuma nao pode ir)."""
    q_ini, q_fim = quadros_do_beat(1, fator)
    caixa, u1 = objs["caixa"], objs["u1"]
    n = float(q_fim - q_ini)
    voltas = 2.0                                   # inteiras: acaba com a frente em -Y
    profundidade = caixa["topo_tampa_z"] + 0.25    # some inteira sob o chao
    objs["profundidade_caixa"] = profundidade
    corpo, tampa, raiz = caixa["corpo"], caixa["tampa"], u1["raiz"]
    z_tampa = tampa.location.z
    z_u1 = raiz.location.z
    repousos = [(esp, Vector(esp["caixa_repouso"]), tuple(esp["caixa_rot_repouso"])) for esp in caixa["espumas"]]
    for f in range(q_ini, q_fim + 1):
        u = (f - q_ini) / n
        # Giro decai mais devagar que a subida: a caixa ja assentou em altura
        # e ainda gira um pouco - le como "assentar", nao como parar.
        s_rot = 1.0 - (1.0 - u) ** 2.2
        s_z = 1.0 - (1.0 - u) ** 2.8
        ang = -voltas * math.tau * (1.0 - s_rot)
        dz = -profundidade * (1.0 - s_z)
        _chave(corpo, f, (0.0, 0.0, dz), (0.0, 0.0, ang))
        _chave(tampa, f, (0.0, 0.0, z_tampa + dz), (0.0, 0.0, ang))
        _chave(raiz, f, (0.0, 0.0, z_u1 + dz), (0.0, 0.0, ang))
        R = Matrix.Rotation(ang, 3, "Z")
        for esp, p0, r0 in repousos:
            p = R @ p0
            _chave(esp, f, (p.x, p.y, p.z + dz), (r0[0], r0[1], r0[2] + ang))
    for obj in [corpo, tampa, raiz] + caixa["espumas"]:
        _interpolar(obj, q_ini, q_fim)

    # Camera frontal, um pouco alta, afastando de leve; o alvo acompanha o
    # centro da caixa que sobe.
    # A lente PRECISA de chave aqui: a fcurve extrapola a primeira chave para
    # tras, e sem esta os beats 1-4 saiam com os 60 mm da primeira foto do
    # beat 5 (medido: caixa 1,7x maior que o calculado).
    _chave_camera(objs, q_ini, -90.0, 2.2, 1.0, (0.0, 0.0, 0.30), lente=35.0)
    _chave_camera(objs, q_fim, -90.0, 2.6, 1.2, (0.0, 0.0, 0.45))
    _chave_rim_especular(objs, q_ini, 0.0)


def _beat2(objs, fator):
    """Tampa sai, espuma explode, U1 sobe, caixa afunda no chao, U1 pousa."""
    r = ROTEIRO[2]
    q_ini, q_fim = quadros_do_beat(2, fator)
    q = lambda fr: q_em(2, fr, fator)  # noqa: E731
    caixa, u1, amb = objs["caixa"], objs["u1"], objs["ambiente"]

    mod_caixa.animar_tampa(caixa, q(r["tampa"][0]), q(r["tampa"][1]), abrir=True, lado=1.0)
    # Fora do quadro a tampa ficaria flutuando a 1,6 m do lado: esconder ate
    # o beat 6 - a chave de volta e gravada la.
    objs["_q_tampa_some"] = q(r["tampa"][1]) + 1

    mod_caixa.animar_espuma(caixa, q(r["espuma"][0]), q(r["espuma"][1]))

    raiz = u1["raiz"]
    z_alto = caixa["exterior_corpo"][2] + objs["params"]["folga_u1"]
    objs["z_alto_u1"] = z_alto
    z0 = u1["z_na_caixa"]
    _chave(raiz, q(r["u1_sobe"][0]), (0.0, 0.0, z0))
    _chave(raiz, q(r["u1_sobe"][1]), (0.0, 0.0, z_alto))
    _chave(raiz, q(r["u1_desce"][0]), (0.0, 0.0, z_alto))
    _chave(raiz, q(r["u1_desce"][1]), (0.0, 0.0, 0.0))
    _interpolar(raiz, q(r["u1_sobe"][0]), q_fim)

    corpo = caixa["corpo"]
    _chave(corpo, q(r["caixa_desce"][0]), (0.0, 0.0, 0.0))
    _chave(corpo, q(r["caixa_desce"][1]), (0.0, 0.0, -objs["profundidade_caixa"]))
    _interpolar(corpo, q(r["caixa_desce"][0]), q(r["caixa_desce"][1]))

    # Camera acompanha o U1 subindo (alvo sobe com ele) e comeca a derivar
    # para +X, de onde a orbita do beat 3 parte.
    _chave_camera(objs, q(r["u1_sobe"][1]), -84.0, 2.5, 1.6, (0.0, 0.0, z_alto + 0.30))
    _chave_camera(objs, q_fim, -75.0, 2.3, 1.1, (0.0, 0.0, 0.37))
    mod_ambiente.animar_rig(amb, q_ini, q_fim, 0.0, 15.0)


def _beat3(objs, fator):
    """Orbita ate a traseira; cabo entra e encaixa; botao afunda, LED acende."""
    r = ROTEIRO[3]
    q_ini, q_fim = quadros_do_beat(3, fator)
    q = lambda fr: q_em(3, fr, fator)  # noqa: E731
    u1, cabo, amb, cena = objs["u1"], objs["cabo"], objs["ambiente"], objs["cena"]

    q_orb = q(r["orbita"][1])
    # Da orbita em diante o produto esconde o reflexo do rim no chao.
    _chave_rim_especular(objs, q_ini, 0.5)
    _chave_camera(objs, q_orb, 105.0, 1.5, 0.60, (0.15, 0.15, 0.30))
    _chave_camera(objs, q_fim, 100.0, 1.25, 0.42, (0.20, 0.20, 0.18))
    # Rim atras do produto do ponto de vista da camera: rig = azimute + 90.
    mod_ambiente.animar_rig(amb, q_ini, q_orb, 15.0, 195.0)
    mod_ambiente.animar_rig(amb, q_orb, q_fim, 195.0, 190.0)

    # Tomada no mundo com o U1 ja no chao (a raiz esta na identidade aqui).
    cena.frame_set(q_ini)
    bpy.context.view_layer.update()
    ponto = mod_u1.ponto_no_mundo(u1, "posicao_tomada", "ponto")
    direcao = mod_u1.ponto_no_mundo(u1, "posicao_tomada", "direcao")
    normal = -direcao
    lateral = normal.cross(Vector((0, 0, 1))).normalized() * cabo.get("lado", 1.0)
    # Origem fora do quadro (a 16 graus de meio-campo horizontal, 1,3 m atras
    # e 0,9 m para o lado esta fora em qualquer ponto da orbita), no chao.
    origem = ponto + normal * 1.3 + lateral * 0.9
    origem.z = 0.02
    q_cabo = (q(r["cabo"][0]), q(r["cabo"][1]))
    mod_cabo.animar_conexao(cabo, ponto, direcao, q_cabo[0], q_cabo[1],
                            origem=origem, z_chao=0.0, penetracao=-mod_cabo.BICO[4])
    objs["_q_cabo"] = q_cabo

    if u1.get("botao") is not None:
        mod_u1.animar_botao(u1, q(r["botao"][0]), q(r["botao"][1]))
    else:
        print("[coreografia] modelo real sem 'u1_botao_objeto': botao/LED nao animados")


def _beat4(objs, fator):
    """Orbita de volta pela frente e dolly ate a tela; boot rapido e UI."""
    r = ROTEIRO[4]
    q_ini, q_fim = quadros_do_beat(4, fator)
    q = lambda fr: q_em(4, fr, fator)  # noqa: E731
    u1, amb, cena = objs["u1"], objs["ambiente"], objs["cena"]

    cena.frame_set(q_ini)
    bpy.context.view_layer.update()
    tela = mod_u1.ponto_no_mundo(u1, "posicao_tela", "centro")
    normal = mod_u1.ponto_no_mundo(u1, "posicao_tela", "normal")
    # Tela de 0,104 m a ~69% da largura do quadro: 0,26 m a 35 mm.
    pos_fim = tela + normal * 0.26 + Vector((0.02, 0.0, 0.015))
    az_fim, r_fim, z_fim = _cil(pos_fim)
    az_fim += 360.0          # continua girando no mesmo sentido (105 -> 292)
    q_orb = q(r["orbita"][1])
    _chave_camera(objs, q(0.30), 180.0, 1.9, 0.80, (0.0, 0.0, 0.42))
    _chave_camera(objs, q_orb, 250.0, 1.35, 0.72, (tela + Vector((-0.08, 0.0, -0.10))))
    # Ultima chave em q_fim-1, CONSTANT: o corte da primeira foto e em q_fim,
    # e duas chaves no mesmo quadro fariam a foto sobrescrever o close da
    # tela (medido: o quadro 350 saiu apontando para os cabecotes).
    _chave_camera(objs, q_fim - 1, az_fim, r_fim, z_fim, tela, interp="CONSTANT")
    mod_ambiente.animar_rig(amb, q_ini, q_orb, 190.0, 340.0)
    mod_ambiente.animar_rig(amb, q_orb, q_fim - 1, 340.0, az_fim + 90.0)

    if u1.get("tela") is not None:
        mod_u1.animar_tela(u1, q(r["boot"]), q(r["ui"]), q_fim)
    else:
        print("[coreografia] modelo real sem 'u1_tela_objeto': tela nao animada")


def _beat5(objs, fator):
    """Tres fotos: cortes secos com flash, closes ancorados no canto inferior
    direito, cada um com um drift lento e a luz mudando de angulo."""
    q_ini, q_fim = quadros_do_beat(5, fator)
    cena, amb, cam = objs["cena"], objs["ambiente"], objs["camera"]
    cena.frame_set(q_ini)
    bpy.context.view_layer.update()
    s = _sujeitos_fotos(objs)
    cortes = [q_em(5, fr, fator) for fr in ROTEIRO[5]["fotos"]] + [q_fim]

    # (sujeito, camera no inicio, deslocamento do drift, lente, rig relativo, energia key, energia rim)
    fotos = [
        (s["cabecotes"], s["cabecotes"] + Vector((-0.22, -0.38, 0.30)), Vector((0.03, 0.02, -0.02)), 60.0, +45.0, 300.0, 550.0),
        (s["porta"], s["porta"] + Vector((0.38, -0.52, 0.18)), Vector((-0.02, 0.02, 0.01)), 50.0, -50.0, 260.0, 650.0),
        (s["mesa"], s["mesa"] + Vector((-0.12, -0.16, 0.86)), Vector((0.02, 0.02, -0.03)), 50.0, +70.0, 240.0, 800.0),
    ]
    luzes = amb["luzes"]
    padrao_key = luzes["key"].data.energy
    padrao_rim = luzes["rim"].data.energy
    rig_luz = amb["rig"]
    # Antes do primeiro corte as energias precisam de chave com o valor
    # padrao, senao a primeira chave extrapola para tras e muda os beats 1-4.
    for luz, val in ((luzes["key"], padrao_key), (luzes["rim"], padrao_rim)):
        luz.data.energy = val
        luz.data.keyframe_insert("energy", frame=cortes[0] - 1)
    objs["_chaves_rig_luz"] = {}
    for i, (sujeito, pos, drift, lente, rig_rel, e_key, e_rim) in enumerate(fotos):
        q_a, q_b = cortes[i], cortes[i + 1] - 1
        for q_, p_ in ((q_a, pos), (q_b, pos + drift)):
            az, raio, z = _cil(p_)
            alvo = _enquadrar(p_, sujeito, lente)
            _chave_camera(objs, q_, az, raio, z, alvo, foco=sujeito, lente=lente,
                          interp="LINEAR" if q_ == q_a else "CONSTANT")
        mod_ambiente.animar_flash(amb, cam, q_a, forca=1.0)
        # Luz da foto: rig girado em relacao a camera (key mais lateral) e
        # rim mais forte; tudo em chave constante, e um corte.
        az_cam = _cil(pos)[0]
        objs["_chaves_rig_luz"][q_a] = az_cam + 90.0 + rig_rel
        for luz, val in ((luzes["key"], e_key), (luzes["rim"], e_rim)):
            luz.data.energy = val
            luz.data.keyframe_insert("energy", frame=q_a)
    # De volta ao padrao no corte do beat 6.
    for luz, val in ((luzes["key"], padrao_key), (luzes["rim"], padrao_rim)):
        luz.data.energy = val
        luz.data.keyframe_insert("energy", frame=q_fim)
        for fc in luz.data.animation_data.action.fcurves:
            for kp in fc.keyframe_points:
                kp.interpolation = "CONSTANT"
    objs["_q_rig_luz_padrao"] = q_fim


def _beat6(objs, fator):
    """Corte ao plano geral: U1 sobe, caixa volta pelo chao, U1 entra, espuma
    volta, tampa fecha; camera sobe."""
    r = ROTEIRO[6]
    q_ini, q_fim = quadros_do_beat(6, fator)
    q = lambda fr: q_em(6, fr, fator)  # noqa: E731
    caixa, u1, amb, cabo = objs["caixa"], objs["u1"], objs["ambiente"], objs["cabo"]

    _chave_camera(objs, q_ini, -80.0, 3.0, 1.3, (0.0, 0.0, 0.55), lente=35.0)
    _chave_camera(objs, q_fim, -84.0, 2.8, 2.3, (0.0, 0.0, 0.70))
    mod_ambiente.animar_rig(amb, q_ini, q_fim, 10.0, 6.0)
    _chave_rim_especular(objs, q_ini, 0.0)

    raiz = u1["raiz"]
    z_alto = objs["z_alto_u1"]
    _chave(raiz, q(r["u1_sobe"][0]), (0.0, 0.0, 0.0))
    _chave(raiz, q(r["u1_sobe"][1]), (0.0, 0.0, z_alto))
    _chave(raiz, q(r["u1_desce"][0]), (0.0, 0.0, z_alto))
    _chave(raiz, q(r["u1_desce"][1]), (0.0, 0.0, u1["z_na_caixa"]))
    _interpolar(raiz, q_ini, q_fim)

    corpo = caixa["corpo"]
    _chave(corpo, q(r["caixa_sobe"][0]), (0.0, 0.0, -objs["profundidade_caixa"]))
    _chave(corpo, q(r["caixa_sobe"][1]), (0.0, 0.0, 0.0))
    _interpolar(corpo, q(r["caixa_sobe"][0]), q(r["caixa_sobe"][1]))

    mod_caixa.animar_espuma_voltar(caixa, q(r["espuma"][0]), q(r["espuma"][1]))
    mod_caixa.animar_tampa(caixa, q(r["tampa"][0]), q(r["tampa"][1]), abrir=False, lado=1.0)
    _esconder_entre([caixa["tampa"]], objs["_q_tampa_some"], q(r["tampa"][0]) - 1)

    # Cabo: visivel so do inicio do voo ate o corte do beat 6 (o modulo nao
    # acompanha o U1 subindo, e o plugue no chao apareceria nos beats 1-2).
    visiveis = _esconder_entre(list(cabo["colecao"].all_objects), 1, objs["_q_cabo"][0])
    for obj in visiveis:
        _chave_visivel(obj, q_ini, False)
    # Tela apaga no corte: a maquina volta para a caixa desligada.
    mat = u1.get("materiais", {}).get("tela")
    no = mat.node_tree.nodes.get("ligada") if mat and mat.use_nodes else None
    if no is not None:
        no.outputs[0].default_value = 0.0
        no.outputs[0].keyframe_insert("default_value", frame=q_ini)


def _beat7(objs, fator):
    """Camera sobe para o eixo da logo, mergulha ate atravessar a tampa;
    corte para a cartela, parented na camera, que entra e fica."""
    r = ROTEIRO[7]
    q_ini, q_fim = quadros_do_beat(7, fator)
    q = lambda fr: q_em(7, fr, fator)  # noqa: E731
    caixa, cartela, cena, cam, amb = objs["caixa"], objs["cartela"], objs["cena"], objs["camera"], objs["ambiente"]
    p = objs["params"]

    cena.frame_set(q_ini)
    bpy.context.view_layer.update()
    logo = caixa["tampa"].matrix_world @ caixa["centro_logo_local"]
    normal = (caixa["tampa"].matrix_world.to_3x3() @ caixa["normal_logo"]).normalized()
    q_topo, q_t = q(r["sobe_para_logo"][1]), q(r["mergulho"][1])
    objs["q_travessia"] = q_t
    # O alvo fica 1 m abaixo da camera, 4 mm para +Y: e o que define o "para
    # cima" do quadro (+Y = logo em pe) sem o Track To degenerar na vertical.
    # O foco fica na logo o mergulho inteiro.
    alto = logo + normal * 1.8
    _chave_camera(objs, q_topo, -90.0, 0.0, alto.z, (0.0, 0.004, alto.z - 1.0), foco=logo,
                  interp="QUAD", easing="EASE_IN")
    baixo = logo + normal * 0.12
    _chave_camera(objs, q_t - 1, -90.0, 0.0, baixo.z, (0.0, 0.004, baixo.z - 1.0), foco=logo,
                  interp="CONSTANT")
    mod_ambiente.animar_rig(amb, q_ini, q_topo, 6.0, 0.0)

    # Corte: camera limpa, de costas para a cena, olhando 24 graus para cima.
    # Medido com 12 graus: o horizonte caia a 78% da altura e o brilho (ceu
    # ate 15 graus acima dele mais o chao emissivo abaixo) cobria de 50% a
    # 95% do quadro - as linhas 2-4 ficavam sobre rose claro. Com 24 graus o
    # horizonte fica a ~96% e o brilho so no terco de baixo, sob o bloco.
    z_cam = 1.0
    dist_alvo = 4.0
    alvo_z = z_cam + dist_alvo * math.tan(math.radians(24.0))
    raio0 = 4.0
    _chave_camera(objs, q_t, 90.0, raio0, z_cam, (0.0, raio0 + dist_alvo, alvo_z), interp="LINEAR")
    cena.frame_set(q_t)
    bpy.context.view_layer.update()
    mod_cartela.posicionar_cartela(cartela, cam, cartela["distancia"])
    raiz = cartela["raiz"]
    raiz.parent = cam
    raiz.matrix_parent_inverse = Matrix.Identity(4)
    raiz.matrix_basis = Matrix.Translation((0.0, p["cartela_subida"], -cartela["distancia"]))
    bpy.context.view_layer.update()
    foco_cartela = raiz.matrix_world.translation.copy()
    objs["foco"].location = foco_cartela
    objs["foco"].keyframe_insert("location", frame=q_t)
    deriva = 0.12
    _chave_camera(objs, q_fim, 90.0, raio0 + deriva, z_cam, (0.0, raio0 + deriva + dist_alvo, alvo_z),
                  foco=foco_cartela + Vector((0.0, deriva, 0.0)), interp="LINEAR")

    elementos = list(cartela["linhas"]) + ([cartela["logo"]] if cartela.get("logo") else [])
    _esconder_entre(elementos, 1, q_t)
    mod_cartela.animar_cartela(cartela, q_t + 1, q(r["cartela"][1]))


def _rig_luz_cortes(objs):
    """Chaves CONSTANT do rig de luz nos cortes do beat 5, gravadas DEPOIS de
    todos os animar_rig (que rebaixam toda chave para Bezier)."""
    amb = objs["ambiente"]
    rig = amb["rig"]
    chaves = objs.get("_chaves_rig_luz", {})
    if not chaves:
        return
    q_padrao = objs["_q_rig_luz_padrao"]
    # O valor no quadro do retorno e o que o animar_rig do beat 6 gravou.
    objs["cena"].frame_set(q_padrao)
    ang_padrao = rig.rotation_euler.z
    primeiro = min(chaves)
    objs["cena"].frame_set(primeiro - 1)
    ang_antes = rig.rotation_euler.z
    rig.rotation_euler = (0.0, 0.0, ang_antes)
    rig.keyframe_insert("rotation_euler", index=2, frame=primeiro - 1)
    for q_, ang in chaves.items():
        rig.rotation_euler = (0.0, 0.0, math.radians(ang))
        rig.keyframe_insert("rotation_euler", index=2, frame=q_)
    rig.rotation_euler = (0.0, 0.0, ang_padrao)
    rig.keyframe_insert("rotation_euler", index=2, frame=q_padrao)
    quadros = set(chaves) | {primeiro - 1}
    for fc in rig.animation_data.action.fcurves:
        for kp in fc.keyframe_points:
            if int(round(kp.co.x)) in quadros:
                kp.interpolation = "CONSTANT"
        fc.update()


def coreografar(objs, fator=None):
    """Os sete beats sobre os objetos de construir_tudo."""
    if fator is None:
        fator = objs.get("fator", 1.0)
    objs["fator"] = fator
    cena = objs["cena"]
    cena.frame_start = 1
    cena.frame_end = quadros_do_beat(7, fator)[1]
    cena.frame_set(1)
    _beat1(objs, fator)
    _beat2(objs, fator)
    _beat3(objs, fator)
    _beat4(objs, fator)
    _beat5(objs, fator)
    _beat6(objs, fator)
    _beat7(objs, fator)
    _rig_luz_cortes(objs)
    _aplicar_interpolacao_camera(objs)
    cena.frame_set(1)
    return objs


# ---------------------------------------------------------------- render

def configurar_render(objs, largura=1080, altura=1920, amostras=64, video=False, caminho_saida=None):
    """Render do modulo ambiente mais o que os outros modulos pediram."""
    cena = objs["cena"]
    p = mod_ambiente.configurar_render(cena, largura, altura, fps=int(FPS), amostras=amostras,
                                       params={"video": video, "caminho_saida": caminho_saida})
    ee = cena.eevee
    # Chanfro do plugue em close chuvisca com menos que isto (achado do cabo).
    _ajustar(ee, "shadow_ray_count", 4)
    _ajustar(ee, "shadow_step_count", 8)
    # Obturador abre no quadro e fecha depois: o quadro de um corte nao
    # mistura o plano anterior (ver cabecalho).
    _ajustar(cena.render, "motion_blur_position", "START")
    return p


def renderizar_quadro(objs, quadro_, caminho):
    cena = objs["cena"]
    cena.frame_set(quadro_)
    cena.render.filepath = caminho
    bpy.ops.render.render(write_still=True)
    return caminho


# ---------------------------------------------------------------- conferencia

def conferir_colisoes(objs, passo=1):
    """Mede, quadro a quadro nos beats 2 e 6, se o U1 atravessa o fundo da
    caixa e quantas espumas estao dentro do volume do U1. Devolve um dict com
    os piores casos; imprime. Numero visivel sai de medicao."""
    cena, caixa, u1 = objs["cena"], objs["caixa"], objs["u1"]
    fator = objs["fator"]
    parede = mod_caixa.PARAMS_PADRAO["parede"]
    mn, mx = u1["envelope"]
    piores = {"u1_abaixo_do_fundo_m": 0.0, "espumas_no_u1": 0, "quadro_pior": None, "u1_x_tampa": 0}
    tampa = caixa["tampa"]
    ext_t = caixa["exterior_tampa"]
    for n in (2, 6):
        a, b = quadros_do_beat(n, fator)
        for f in range(a, b + 1, passo):
            cena.frame_set(f)
            zu = u1["raiz"].matrix_world.translation.z
            zc = caixa["corpo"].matrix_world.translation.z
            fundo = zc + parede
            if zu < fundo - 1e-4:
                piores["u1_abaixo_do_fundo_m"] = max(piores["u1_abaixo_do_fundo_m"], fundo - zu)
            dentro = 0
            for esp in caixa["espumas"]:
                p_ = esp.matrix_world.translation
                raio = float(esp["caixa_raio"])
                if (mn.x + raio * 0.5 < p_.x < mx.x - raio * 0.5 and mn.y + raio * 0.5 < p_.y < mx.y - raio * 0.5
                        and zu + mn.z + raio * 0.5 < p_.z < zu + mx.z - raio * 0.5):
                    dentro += 1
            if dentro > piores["espumas_no_u1"]:
                piores["espumas_no_u1"] = dentro
                piores["quadro_pior"] = f
            # Tampa x U1: a tampa e oca, e fechada envolve os 4 cm de cima do
            # U1 - isso e o normal. Colisao e o topo do U1 passar do TETO
            # interno dela (ou a parede lateral dela cruzar o U1 quando ela
            # esta deslocada em X, o que a inclinacao da saida evita).
            tm = tampa.matrix_world
            teto = tm.translation.z + ext_t[2] / 2.0 - parede
            base = tm.translation.z - ext_t[2] / 2.0
            sobre = abs(tm.translation.x) < ext_t[0] / 2.0 + mx.x and abs(tm.translation.y) < ext_t[1] / 2.0 + mx.y
            deslocada = abs(tm.translation.x) > 0.005 or abs(tm.translation.y) > 0.005
            if sobre and not tampa.hide_render:
                topo_u1 = zu + mx.z
                if topo_u1 > teto + 1e-3 and zu + mn.z < teto:
                    piores["u1_x_tampa"] += 1
                elif deslocada and base < topo_u1 - 1e-3 and abs(tm.translation.x) < mx.x + ext_t[0] / 2.0 - 0.02:
                    piores["u1_x_tampa"] += 1
    print("[coreografia] colisoes: U1 abaixo do fundo da caixa = %.4f m; espumas dentro do U1 (pior quadro %s) = %d; quadros com tampa x U1 = %d"
          % (piores["u1_abaixo_do_fundo_m"], piores["quadro_pior"], piores["espumas_no_u1"], piores["u1_x_tampa"]))
    cena.frame_set(1)
    return piores
