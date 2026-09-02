# Modulo CABO do anuncio do Snapmaker U1.
#
# Cabo de energia preto (borracha) como curva Bezier com bevel redondo de
# 8 mm, e um plugue IEC C13 modelado: bico que entra na tomada, corpo moldado
# com chanfro, friso, alivio de tensao com nervuras e tres contatos metalicos
# nas janelas do bico. So definicoes aqui - nada roda no import. Quem integra
# e mod_coreografia.py; quem prova este modulo sozinho e teste_cabo.py.
#
# DECISOES:
#
# - O C13 e a ponta FEMEA do cabo (os pinos macho ficam no C14 da maquina, e o
#   mod_u1 ja os tem dentro da tomada). Por isso os "3 pinos metalicos" aqui
#   sao os contatos de latao vistos dentro das tres janelas do bico, recuados
#   1,5 mm - e o que se ve olhando a cara de um C13 de verdade. Se um dia se
#   quiser pinos salientes, e o parametro 'saliencia_contatos'.
#
# - A origem do Empty 'cabo.plugue' e o CENTRO DA CARA do bico, e o eixo local
#   +Y e a direcao em que o plugue entra. Encaixado, o Empty esta exatamente
#   em ponto_tomada com +Y = direcao_entrada: quem integra nao precisa saber
#   medida nenhuma do plugue.
#
# - O cabo acompanha o plugue por KEYFRAMES NOS PONTOS DA CURVA (co e handles),
#   um por quadro, e nao por hooks: hook depende de objeto auxiliar e de
#   modificador, e o ponto animado direto e o que sobrevive a qualquer versao
#   do Blender e a qualquer cena. Como o plugue tambem recebe uma chave por
#   quadro, calculada pela mesma funcao, cabo e plugue nunca se separam - o que
#   aconteceria se o plugue fosse interpolado pelo Blender e o cabo por aqui.
#   O easing entra na PARAMETRIZACAO do arco, nao na chave.
#
# - Depois de conectado o cabo sai reto do alivio de tensao por uns
#   centimetros (o alivio e rigido), cai numa curva de gravidade ate o chao e
#   segue pelo chao para fora do quadro ('ponto_fora'). Nao ha tomada na
#   parede: o cabo vem de fora da cena.
#
# ACHADO NA PREVIA (EEVEE Next 4.2), para quem monta o render final: em
# close (85 mm a 20 cm) o chanfro do corpo do plugue sai CHUVISCADO onde a
# luz bate rasante - acne de sombra, nao ruido de amostragem. Medido em
# variantes separadas: 48 amostras nao resolve; desligar o jitter PIORA
# (o chanfro inteiro chuvisca); shadow_maximum_resolution=0,0002 e
# shadow_filter_radius=2 nas luzes nao resolvem; o que resolve sozinho e
# cena.eevee.shadow_ray_count = 4 e shadow_step_count = 8 (padrao 1 e 6).
# E ajuste de cena, nao de material: o teste daqui liga, o modulo ambiente
# precisa ligar no render final.
#
# Eixos e medidas seguem docs/ESPECIFICACAO.md: metros, Z para cima, frente
# em -Y (a traseira do U1 e a tomada ficam em +Y), chao em z = 0.

import math

import bmesh
import bpy
from mathutils import Matrix, Vector

NOME = "cabo"

PARAMS_PADRAO = {
    # Tomada do U1 substituto (mod_u1: coluna em x=0,245, face da coluna em
    # y=0,2575, aro 3,5 mm a frente, 0,125 m do chao). A coreografia passa a
    # de verdade; isto e so para o modulo construir sozinho.
    "ponto_tomada": (0.245, 0.261, 0.125),
    "direcao_entrada": (0.0, -1.0, 0.0),
    # Onde o cabo sai do quadro; None = calculado atras da tomada, no chao.
    "ponto_fora": None,
    "z_chao": 0.0,
    "diametro_cabo": 0.008,
    "cor_cabo": "#111111",
    "rugosidade_cabo": 0.5,
    # O plastico moldado do plugue e um pouco mais lustroso que a borracha.
    "rugosidade_plugue": 0.38,
    "cor_contato": (0.80, 0.56, 0.24),
    "saliencia_contatos": 0.0,     # >0 faz os contatos sairem da cara (mm em m)
    # Quanto a cara do bico entra alem de ponto_tomada. 0 = so encosta, que e
    # o certo quando a tomada do modelo e uma face plana; num C14 com bolso
    # pode ser ate 0,006.
    "penetracao": 0.0,
    "resolucao_bevel": 6,
    "resolucao_curva": 24,
    # Lado do qual o cabo vem e para onde sai: +1 = para o lado de
    # (normal x Z), -1 = o oposto.
    "lado": 1.0,
}

FPS = 30.0

# Medidas do C13 (chutadas em cima de plugues de mesa; norma IEC 60320 so fixa
# a cara e os pinos). Cara em y = 0, tudo cresce para -Y.
BICO = (0.0225, 0.0158, 0.0040, 0.0, -0.0075)      # largura, altura, chanfro, y_frente, y_tras
CORPO = (0.0300, 0.0215, 0.0065, -0.0075, -0.0430)
FRISO = (0.0312, 0.0227, 0.0072, -0.0270, -0.0300)
# Janelas dos contatos: mesmas posicoes dos pinos do C14 do mod_u1 (14 mm
# entre fase e neutro, terra 5 mm acima do centro).
JANELAS = ((-0.007, -0.001), (0.007, -0.001), (0.0, 0.004))
JANELA_DIMS = (0.0024, 0.012, 0.0050)      # x, y (profundidade do corte), z
CONTATO_DIMS = (0.0016, 0.0040, 0.0042)
RECUO_CONTATO = 0.0015
# Alivio de tensao: (y, raio) do corpo ate a saida do cabo. Nervuras
# alternadas e afinando - e isso que faz ler como plugue moldado e nao como
# um cilindro colado num bloco.
ALIVIO = (
    (-0.0410, 0.0092), (-0.0440, 0.0080), (-0.0460, 0.0070),
    (-0.0480, 0.0068), (-0.0500, 0.0057), (-0.0520, 0.0066), (-0.0540, 0.0055),
    (-0.0560, 0.0063), (-0.0580, 0.0053), (-0.0600, 0.0060), (-0.0620, 0.0051),
    (-0.0640, 0.0057), (-0.0660, 0.0049), (-0.0680, 0.0053), (-0.0700, 0.0044),
)
COMPRIMENTO_PLUGUE = 0.070     # da cara ate onde o cabo sai do alivio
SEGMENTOS_ALIVIO = 24


# ---------------------------------------------------------------- utilidades

def _srgb_para_linear(c):
    c = c / 255.0
    return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4


def cor_linear(hexstr):
    """'#RRGGBB' -> RGBA linear (o Principled quer linear; a paleta e sRGB)."""
    h = hexstr.lstrip("#")
    return tuple(_srgb_para_linear(int(h[i:i + 2], 16)) for i in (0, 2, 4)) + (1.0,)


def limpar_colecao(nome):
    """Remove a sub-colecao <nome> e tudo o que ela contem (idempotencia)."""
    col = bpy.data.collections.get(nome)
    if col is None:
        return
    for obj in list(col.all_objects):
        dados = obj.data
        bpy.data.objects.remove(obj, do_unlink=True)
        # Dado orfao fica ate salvar/recarregar; apagar aqui evita acumular
        # 'cabo.curva.001' nas rodadas seguintes.
        if dados is not None and dados.users == 0:
            if isinstance(dados, bpy.types.Mesh):
                bpy.data.meshes.remove(dados)
            elif isinstance(dados, bpy.types.Curve):
                bpy.data.curves.remove(dados)
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


def _material(nome, cor, rugosidade, metalico=0.0):
    mat = bpy.data.materials.get(nome)
    if mat is None:
        mat = bpy.data.materials.new(nome)
    mat.use_nodes = True
    b = mat.node_tree.nodes.get("Principled BSDF")
    b.inputs["Base Color"].default_value = cor
    b.inputs["Roughness"].default_value = rugosidade
    b.inputs["Metallic"].default_value = metalico
    return mat


def _sombrear_suave(malha, suave=True):
    malha.polygons.foreach_set("use_smooth", [suave] * len(malha.polygons))
    malha.update()


def _novo_objeto(nome, malha, col, pai, mat=None, pos=(0, 0, 0)):
    obj = bpy.data.objects.new(nome, malha)
    obj.location = pos
    if mat is not None and malha is not None:
        malha.materials.append(mat)
    col.objects.link(obj)
    if pai is not None:
        obj.parent = pai
    return obj


def _chanfro(obj, largura, segmentos, nome="chanfro"):
    mod = obj.modifiers.new(nome, "BEVEL")
    mod.width = largura
    mod.segments = segmentos
    mod.limit_method = "ANGLE"
    mod.angle_limit = math.radians(50.0)
    try:
        # Sem isto as faces planas do corpo ganham gradiente de sombreamento.
        mod.harden_normals = True
    except AttributeError:
        pass
    return mod


# ---------------------------------------------------------------- geometria

def _perfil_c13(largura, altura, chanfro):
    """Contorno do C13 no plano XZ local: retangulo com os dois cantos DE
    CIMA chanfrados (o terra fica em cima). Sentido anti-horario visto de +Y."""
    w, h, c = largura / 2.0, altura / 2.0, chanfro
    return [(-w, -h), (w, -h), (w, h - c), (w - c, h), (-w + c, h), (-w, h - c)]


def _bloco_c13(nome, col, pai, mat, largura, altura, chanfro, y_frente, y_tras):
    """Prisma do perfil C13 entre y_tras e y_frente, com tampas."""
    perfil = _perfil_c13(largura, altura, chanfro)
    bm = bmesh.new()
    anel_f = [bm.verts.new((x, y_frente, z)) for x, z in perfil]
    anel_t = [bm.verts.new((x, y_tras, z)) for x, z in perfil]
    n = len(perfil)
    for i in range(n):
        j = (i + 1) % n
        bm.faces.new((anel_t[i], anel_t[j], anel_f[j], anel_f[i]))
    bm.faces.new(anel_f)
    bm.faces.new(list(reversed(anel_t)))
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    malha = bpy.data.meshes.new(nome)
    bm.to_mesh(malha)
    bm.free()
    return _novo_objeto(nome, malha, col, pai, mat)


def _caixa(nome, col, pai, mat, dims, pos):
    bm = bmesh.new()
    bmesh.ops.create_cube(bm, size=1.0)
    bmesh.ops.scale(bm, vec=dims, verts=bm.verts)
    malha = bpy.data.meshes.new(nome)
    bm.to_mesh(malha)
    bm.free()
    return _novo_objeto(nome, malha, col, pai, mat, pos)


def _cortador(nome, col, pai, dims, pos, alvo):
    """Cubo invisivel que recorta 'alvo' por boolean; fica na colecao para o
    limpar_colecao achar."""
    obj = _caixa(nome, col, pai, None, dims, pos)
    obj.hide_render = True
    obj.hide_viewport = True
    obj.display_type = "WIRE"
    mod = alvo.modifiers.new("corte_" + nome.split(".")[-1], "BOOLEAN")
    mod.operation = "DIFFERENCE"
    mod.object = obj
    try:
        mod.solver = "EXACT"
    except TypeError:
        pass
    return obj


def _alivio(nome, col, pai, mat):
    """Alivio de tensao: aneis de raio variavel ao longo de -Y, com tampas."""
    bm = bmesh.new()
    aneis = []
    for y, r in ALIVIO:
        anel = []
        for k in range(SEGMENTOS_ALIVIO):
            a = 2.0 * math.pi * k / SEGMENTOS_ALIVIO
            anel.append(bm.verts.new((r * math.cos(a), y, r * math.sin(a))))
        aneis.append(anel)
    n = SEGMENTOS_ALIVIO
    for a0, a1 in zip(aneis[:-1], aneis[1:]):
        for i in range(n):
            j = (i + 1) % n
            bm.faces.new((a0[i], a0[j], a1[j], a1[i]))
    bm.faces.new(aneis[0])
    bm.faces.new(list(reversed(aneis[-1])))
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    malha = bpy.data.meshes.new(nome)
    bm.to_mesh(malha)
    bm.free()
    _sombrear_suave(malha)
    obj = _novo_objeto(nome, malha, col, pai, mat)
    # Suavizar so as nervuras (angulo baixo), mantendo as tampas planas.
    try:
        malha.set_sharp_from_angle(angle=math.radians(40.0))    # 4.1+
    except AttributeError:
        try:
            malha.use_auto_smooth = True                        # ate 4.0
            malha.auto_smooth_angle = math.radians(40.0)
        except AttributeError:
            pass
    return obj


def _construir_plugue(col, p, m_plugue, m_contato):
    raiz = bpy.data.objects.new("cabo.plugue", None)
    raiz.empty_display_type = "ARROWS"
    raiz.empty_display_size = 0.02
    raiz.rotation_mode = "QUATERNION"
    col.objects.link(raiz)

    partes = {}
    bico = _bloco_c13("cabo.plugue.bico", col, raiz, m_plugue, *BICO)
    _chanfro(bico, 0.0007, 2)
    # Janelas dos contatos: cortadas DEPOIS do chanfro na pilha, para a
    # borda da janela ficar viva como no plastico moldado.
    for i, (px, pz) in enumerate(JANELAS):
        _cortador("cabo.cortador.janela.%d" % (i + 1), col, raiz, JANELA_DIMS,
                  (px, BICO[3] - JANELA_DIMS[1] / 2.0 + 0.006, pz), bico)
    partes["bico"] = bico

    corpo = _bloco_c13("cabo.plugue.corpo", col, raiz, m_plugue, *CORPO)
    _chanfro(corpo, 0.0022, 4)
    partes["corpo"] = corpo
    friso = _bloco_c13("cabo.plugue.friso", col, raiz, m_plugue, *FRISO)
    _chanfro(friso, 0.0006, 2)
    partes["friso"] = friso
    partes["alivio"] = _alivio("cabo.plugue.alivio", col, raiz, m_plugue)

    contatos = []
    for i, (px, pz) in enumerate(JANELAS):
        y = BICO[3] - RECUO_CONTATO - CONTATO_DIMS[1] / 2.0 + p["saliencia_contatos"]
        c = _caixa("cabo.plugue.contato.%d" % (i + 1), col, raiz, m_contato, CONTATO_DIMS, (px, y, pz))
        _chanfro(c, 0.0003, 1)
        contatos.append(c)
    partes["contatos"] = contatos
    return raiz, partes


def _construir_curva(col, p, m_cabo):
    cd = bpy.data.curves.new("cabo.curva", "CURVE")
    cd.dimensions = "3D"
    cd.bevel_depth = p["diametro_cabo"] / 2.0
    cd.bevel_resolution = p["resolucao_bevel"]
    cd.fill_mode = "FULL"
    cd.use_fill_caps = True
    cd.resolution_u = p["resolucao_curva"]
    sp = cd.splines.new("BEZIER")
    sp.bezier_points.add(3)          # 4 pontos: saida, queda, meio do chao, fora
    for bp in sp.bezier_points:
        bp.handle_left_type = "FREE"
        bp.handle_right_type = "FREE"
    sp.use_smooth = True
    cd.materials.append(m_cabo)
    obj = bpy.data.objects.new("cabo.curva", cd)
    col.objects.link(obj)
    return obj


# ---------------------------------------------------------------- pose

def _lateral(normal, lado):
    lat = normal.cross(Vector((0, 0, 1)))
    if lat.length < 1e-6:
        # Tomada virada para cima/baixo: qualquer horizontal serve.
        lat = Vector((1, 0, 0))
    return lat.normalized() * lado


def _quat_eixo_y(eixo_y):
    """Rotacao que leva o +Y local do plugue para eixo_y, com o Z para cima
    (sem rolagem: um plugue de mesa voa 'em pe')."""
    eixo_y = eixo_y.normalized()
    if abs(eixo_y.z) > 0.999:
        return eixo_y.to_track_quat("Y", "X")
    return eixo_y.to_track_quat("Y", "Z")


def _matriz_plugue(pos, quat):
    return Matrix.Translation(pos) @ quat.to_matrix().to_4x4()


def _pontos_cabo(m_plugue, ponto_fora, z_chao, raio, lateral):
    """Os 4 pontos Bezier do cabo para uma pose do plugue: saida do alivio,
    queda no chao, meio do caminho pelo chao, saida do quadro.
    Devolve [(co, handle_esq, handle_dir), ...]."""
    frente = (m_plugue.to_3x3() @ Vector((0, 1, 0))).normalized()
    tras = -frente
    saida = m_plugue @ Vector((0, -COMPRIMENTO_PLUGUE, 0))
    z_cabo = z_chao + raio
    h = max(0.0, saida.z - z_cabo)

    dh = Vector((ponto_fora.x - saida.x, ponto_fora.y - saida.y, 0.0))
    if dh.length < 1e-6:
        dh = Vector((tras.x, tras.y, 0.0))
    dh.normalize()

    # Onde o cabo toca o chao: um cabo de 8 mm pendurado de h metros chega ao
    # chao a ~1,25 h de distancia, mais o trecho reto que o alivio impoe
    # (com 1,4 h a queda saia como uma diagonal dura na previa de lado).
    d1 = 0.14 + 1.25 * h
    p0 = saida
    h0 = saida + tras * min(0.08, 0.4 * d1)
    h0.z = max(h0.z, z_cabo)          # o cabo nao entra no chao ao decolar
    p1 = saida + dh * d1
    p1.z = z_cabo
    p3 = Vector(ponto_fora)
    p3.z = z_cabo
    reta = p3 - p1
    # Ondulacao lateral suave pelo chao: cabo jogado no chao nunca e reto.
    p2 = p1 + reta * 0.5 + lateral * 0.10
    dir13 = reta.normalized() if reta.length > 1e-6 else dh

    return [
        (p0, p0 - tras * 0.02, h0),
        (p1, p1 - dh * 0.5 * d1, p1 + dir13 * reta.length * 0.22),
        (p2, p2 - dir13 * reta.length * 0.25, p2 + dir13 * reta.length * 0.25),
        (p3, p3 - dir13 * reta.length * 0.30, p3 + dir13 * 0.2),
    ]


def _aplicar_pose(objs, pos, quat, quadro=None):
    """Poe plugue e cabo numa pose; com 'quadro', grava a chave."""
    plugue = objs["plugue"]
    plugue.location = pos
    plugue.rotation_quaternion = quat
    m = _matriz_plugue(pos, quat)
    pontos = _pontos_cabo(m, objs["ponto_fora"], objs["z_chao"], objs["raio"], objs["lateral"])
    cd = objs["curva"].data
    bps = cd.splines[0].bezier_points
    for i, (co, he, hd) in enumerate(pontos):
        bps[i].co = co
        bps[i].handle_left = he
        bps[i].handle_right = hd
    if quadro is None:
        return
    plugue.keyframe_insert("location", frame=quadro)
    plugue.keyframe_insert("rotation_quaternion", frame=quadro)
    for i in range(len(pontos)):
        base = "splines[0].bezier_points[%d]." % i
        cd.keyframe_insert(base + "co", frame=quadro)
        cd.keyframe_insert(base + "handle_left", frame=quadro)
        cd.keyframe_insert(base + "handle_right", frame=quadro)


def _ponto_fora_padrao(ponto, normal, lateral, z_chao, raio):
    p = ponto + normal * 1.7 + lateral * 0.5
    p.z = z_chao + raio
    return p


# ---------------------------------------------------------------- API

def construir_cabo(cena, colecao_pai, params=None):
    """Cria plugue C13 + cabo na sub-colecao 'cabo', encaixados em
    params['ponto_tomada'] com o cabo em repouso. Devolve referencias e
    medidas."""
    p = dict(PARAMS_PADRAO)
    if params:
        p.update(params)
    limpar_colecao(NOME)
    col = _colecao(cena, colecao_pai, NOME)

    m_cabo = _material("cabo.borracha", cor_linear(p["cor_cabo"]), p["rugosidade_cabo"])
    m_plugue = _material("cabo.plugue", cor_linear(p["cor_cabo"]), p["rugosidade_plugue"])
    m_contato = _material("cabo.contato", tuple(p["cor_contato"]) + (1.0,), 0.30, metalico=1.0)

    plugue, partes = _construir_plugue(col, p, m_plugue, m_contato)
    curva = _construir_curva(col, p, m_cabo)

    ponto = Vector(p["ponto_tomada"])
    direcao = Vector(p["direcao_entrada"]).normalized()
    normal = -direcao
    raio = p["diametro_cabo"] / 2.0
    lateral = _lateral(normal, p["lado"])
    ponto_fora = Vector(p["ponto_fora"]) if p["ponto_fora"] is not None else \
        _ponto_fora_padrao(ponto, normal, lateral, p["z_chao"], raio)

    objs = {
        "plugue": plugue,
        "curva": curva,
        "partes": partes,
        "contatos": partes["contatos"],
        "colecao": col,
        "comprimento_plugue": COMPRIMENTO_PLUGUE,
        "raio": raio,
        "z_chao": p["z_chao"],
        "ponto_fora": ponto_fora,
        "lateral": lateral,
        "lado": p["lado"],
        "penetracao": p["penetracao"],
        "materiais": {"cabo": m_cabo, "plugue": m_plugue, "contato": m_contato},
    }
    _aplicar_pose(objs, ponto + direcao * p["penetracao"], _quat_eixo_y(direcao))
    objs["comprimento"] = curva.data.splines[0].calc_length(resolution=32)
    return objs


def _ease(u, easing):
    u = min(1.0, max(0.0, u))
    if easing == "LINEAR":
        return u
    if easing == "EASE_IN":
        return u * u * u
    if easing == "EASE_OUT":
        return 1.0 - (1.0 - u) ** 3
    # EASE_IN_OUT: smoothstep cubico. O smootherstep (quintico) para
    # completamente no fim e o plugue "rasteja" ate a tomada; o cubico chega
    # com a "leve desaceleracao" pedida e o clique faz o resto.
    return u * u * (3.0 - 2.0 * u)


def _bezier3(p0, p1, p2, p3, t):
    s = 1.0 - t
    return p0 * (s * s * s) + p1 * (3 * s * s * t) + p2 * (3 * s * t * t) + p3 * (t * t * t)


def _bezier3_tangente(p0, p1, p2, p3, t):
    s = 1.0 - t
    return (p1 - p0) * (3 * s * s) + (p2 - p1) * (6 * s * t) + (p3 - p2) * (3 * t * t)


def animar_conexao(objs, ponto_tomada, direcao_entrada, q_ini, q_fim, easing="EASE_IN_OUT",
                   origem=None, ponto_fora=None, z_chao=None, altura_arco=0.18,
                   distancia_alinhada=0.25, recuo=0.003, quadros_clique=8, penetracao=None):
    """Plugue vem de fora do quadro (de tras e de baixo, ~1 m) num arco
    suave, alinha-se com direcao_entrada nos ultimos centimetros e encaixa
    em ponto_tomada com desaceleracao e um micro-recuo de 'recuo' metros (o
    clique). O cabo acompanha e termina numa curva de gravidade ate o chao.
    Uma chave por quadro em plugue e cabo, so nos objetos deste modulo."""
    ponto = Vector(ponto_tomada)
    direcao = Vector(direcao_entrada).normalized()
    normal = -direcao
    if z_chao is not None:
        objs["z_chao"] = z_chao
    lateral = _lateral(normal, objs.get("lado", 1.0))
    objs["lateral"] = lateral
    if ponto_fora is not None:
        objs["ponto_fora"] = Vector(ponto_fora)
    else:
        objs["ponto_fora"] = _ponto_fora_padrao(ponto, normal, lateral, objs["z_chao"], objs["raio"])
    if penetracao is None:
        penetracao = objs.get("penetracao", 0.0)
    assento = ponto + direcao * penetracao

    if origem is None:
        # ~0,9 m atras e para o lado, a 2 cm do chao: o plugue "levanta" do
        # chao, onde o cabo ja estava jogado.
        origem = ponto + normal * 0.85 + lateral * 0.30
        origem.z = objs["z_chao"] + 0.02
    origem = Vector(origem)

    # Arco: Bezier cubico. O penultimo ponto de controle esta na reta da
    # tomada, entao a tangente final E direcao_entrada: o alinhamento sai da
    # geometria, sem blend de angulo.
    c1 = origem + (ponto - origem) * 0.45
    c1.z = max(origem.z, ponto.z) + altura_arco
    c2 = assento + normal * distancia_alinhada

    q_toque = max(q_ini + 1, q_fim - quadros_clique)
    q_ant = None
    for f in range(q_ini, q_fim + 1):
        if f <= q_toque:
            u = (f - q_ini) / float(q_toque - q_ini)
            s = _ease(u, easing)
            pos = _bezier3(origem, c1, c2, assento, s)
            tang = _bezier3_tangente(origem, c1, c2, assento, s)
            # O plugue segue a tangente do voo, mas decola deitado: nos
            # primeiros 20% a inclinacao entra aos poucos, senao ele sai do
            # chao apontando para cima como um foguete.
            eixo = Vector((tang.x, tang.y, tang.z * min(1.0, u / 0.2)))
            if eixo.length < 1e-6:
                eixo = direcao
            # Perto da tomada a tangente ja e a direcao; misturar garante o
            # alinhamento exato mesmo com easing que zera a velocidade.
            eixo = eixo.normalized().lerp(direcao, s * s).normalized()
        else:
            # Clique: encaixou, recua 'recuo' e assenta. Meio seno = para no
            # fim sem solavanco.
            v = (f - q_toque) / float(q_fim - q_toque)
            pos = assento + normal * (recuo * math.sin(math.pi * v))
            eixo = direcao
        quat = _quat_eixo_y(eixo)
        # Quaternions interpolam pelo caminho curto so se os sinais forem
        # coerentes entre chaves vizinhas.
        if q_ant is not None and quat.dot(q_ant) < 0.0:
            quat = -quat
        q_ant = quat
        _aplicar_pose(objs, pos, quat, quadro=f)

    _suavizar(objs, q_ini, q_fim)


def fcurves_de(animation_data):
    """Fcurves da acao de um animation_data, em qualquer Blender 4.2+."""
    # Action.fcurves virou legado no 4.4 (slotted actions); no 5.0 pode nao existir.
    try:
        return animation_data.action.fcurves
    except AttributeError:
        slot = animation_data.action_slot
        return animation_data.action.layers[0].strips[0].channelbag(slot).fcurves


def _suavizar(objs, q_ini, q_fim):
    """Bezier auto-clamped em todas as chaves do intervalo: com uma chave por
    quadro nao muda o caminho, mas deixa o motion blur (sub-quadro) suave."""
    for bloco in (objs["plugue"], objs["curva"].data):
        ad = bloco.animation_data
        if ad is None or ad.action is None:
            continue
        for fc in fcurves_de(ad):
            for kp in fc.keyframe_points:
                if q_ini - 0.5 <= kp.co.x <= q_fim + 0.5:
                    kp.interpolation = "BEZIER"
                    kp.handle_left_type = "AUTO_CLAMPED"
                    kp.handle_right_type = "AUTO_CLAMPED"
            fc.update()


def posicionar_repouso(objs, ponto_tomada, direcao_entrada, ponto_fora=None, z_chao=None, penetracao=None):
    """Sem animacao: plugue encaixado e cabo em repouso (para os beats em que
    o cabo ja esta conectado e a coreografia so move o U1)."""
    ponto = Vector(ponto_tomada)
    direcao = Vector(direcao_entrada).normalized()
    if z_chao is not None:
        objs["z_chao"] = z_chao
    lateral = _lateral(-direcao, objs.get("lado", 1.0))
    objs["lateral"] = lateral
    if ponto_fora is not None:
        objs["ponto_fora"] = Vector(ponto_fora)
    else:
        objs["ponto_fora"] = _ponto_fora_padrao(ponto, -direcao, lateral, objs["z_chao"], objs["raio"])
    if penetracao is None:
        penetracao = objs.get("penetracao", 0.0)
    _aplicar_pose(objs, ponto + direcao * penetracao, _quat_eixo_y(direcao))
