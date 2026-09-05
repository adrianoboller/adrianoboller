# Prova do modulo cabo: cena vazia com um cubo grafite no lugar do U1, um
# retangulo marcando a tomada na traseira (ponto e direcao conhecidos), luz
# simples e camera atras, olhando ao longo do voo. Tres quadros (plugue
# longe / meio do voo / encaixado), um close no plugue encaixado (porque a
# 1,8 m um C13 tem 15 pixels e nao da para julgar se parece um C13), a
# vista lateral do cabo conectado (a catenaria) e a cara do plugue no ar.
# Roda com: bash scripts/previa.sh scripts/teste_cabo.py
#   TRAJETO=arco     prova o voo antigo (previas com sufixo _arco)
#   QUAIS=fim,detalhe  so alguns quadros
#
# O script tambem MEDE (revisao 4, trajeto reto): a altura do plugue em
# todo quadro do voo (tem de ser a da tomada, +-1 mm), o angulo entre o
# eixo do plugue e a normal da tomada (0), a flecha do cabo conectado
# (fracao do vao, comparada com o parametro) e o encaixe (0 mm no fim, 3 mm
# no meio do clique). As previas da rodada 3 (arco) ficaram como
# previa_cabo_*_rodada3.png.
#
# Quem roda isto abre os PNGs e olha - o script rodar sem erro nao prova nada.

import math
import os
import sys

import bpy
from mathutils import Vector

AQUI = os.path.dirname(os.path.abspath(__file__))
if AQUI not in sys.path:
    sys.path.insert(0, AQUI)
import importlib
import mod_cabo
importlib.reload(mod_cabo)   # para reexecutar dentro do Blender sem reabrir

RAIZ = os.path.dirname(AQUI)
SAIDA = os.path.join(RAIZ, "saida")

LARGURA, ALTURA = 540, 960
AMOSTRAS = int(os.environ.get("AMOSTRAS", "16"))
# QUAIS=detalhe,cara para rerenderizar so os closes ao mexer no plugue.
QUAIS = os.environ.get("QUAIS", "ini,meio,fim,detalhe,catenaria,cara").split(",")
TRAJETO = os.environ.get("TRAJETO", "reto")
SUFIXO = "" if TRAJETO == "reto" else "_" + TRAJETO

# --- cena limpa ---
bpy.ops.wm.read_factory_settings(use_empty=True)
cena = bpy.context.scene
cena.unit_settings.system = "METRIC"
cena.unit_settings.scale_length = 1.0
cena.render.fps = 30
cena.frame_start = 1
cena.frame_end = 105

try:
    cena.render.engine = "BLENDER_EEVEE_NEXT"
except TypeError:
    # Blender 5 renomeou o EEVEE Next de volta para BLENDER_EEVEE.
    cena.render.engine = "BLENDER_EEVEE"
cena.eevee.taa_render_samples = AMOSTRAS
try:
    cena.eevee.use_shadows = True
    cena.eevee.use_raytracing = False
    # O chanfro do corpo do plugue, em close e com luz rasante, sai
    # chuviscado de acne de sombra. Medido em 5 variantes: nem 48 amostras,
    # nem jitter, nem resolucao/filtro da luz resolvem; so isto (ver
    # cabecalho do mod_cabo). Quem monta o render final precisa carregar.
    cena.eevee.shadow_ray_count = 4
    cena.eevee.shadow_step_count = 8
except AttributeError:
    pass
cena.render.resolution_x = LARGURA
cena.render.resolution_y = ALTURA
cena.render.resolution_percentage = 100
cena.render.image_settings.file_format = "PNG"
cena.view_settings.view_transform = "AgX"
try:
    cena.view_settings.look = "AgX - Medium High Contrast"
except TypeError:
    pass

# Mundo cinza-claro: cabo preto e plugue preto precisam de fundo mais claro
# que eles para a silhueta ler.
mundo = bpy.data.worlds.new("teste.mundo")
cena.world = mundo
mundo.use_nodes = True
fundo = mundo.node_tree.nodes.get("Background")
fundo.inputs["Color"].default_value = (0.20, 0.19, 0.19, 1.0)
fundo.inputs["Strength"].default_value = 0.8

col_teste = bpy.data.collections.new("teste")
cena.collection.children.link(col_teste)


def material(nome, cor, rug, metal=0.0):
    m = bpy.data.materials.new(nome)
    m.use_nodes = True
    b = m.node_tree.nodes["Principled BSDF"]
    b.inputs["Base Color"].default_value = cor
    b.inputs["Roughness"].default_value = rug
    b.inputs["Metallic"].default_value = metal
    return m


def bloco(nome, dims, pos, mat):
    bpy.ops.mesh.primitive_cube_add(size=1.0, location=pos)
    o = bpy.context.active_object
    o.name = nome
    o.scale = dims
    o.data.materials.append(mat)
    for c in o.users_collection:
        c.objects.unlink(o)
    col_teste.objects.link(o)
    return o


# Chao claro: o cabo preto jogado no chao tem de aparecer.
chao = bloco("teste.chao", (40.0, 40.0, 0.001), (0, 0, -0.0005), material("teste.chao", (0.40, 0.38, 0.37, 1.0), 0.8))

# Cubo grafite nas dimensoes do U1, base no chao, frente em -Y.
L, P, A = 0.584, 0.499, 0.730
u1 = bloco("teste.u1", (L, P, A), (0, 0, A / 2.0), material("teste.grafite", (0.010, 0.011, 0.013, 1.0), 0.45))

# Retangulo da tomada na traseira (+Y): 32 x 26 mm, 1 mm saliente, cinza
# medio para o encaixe ler contra o grafite.
tras = P / 2.0
ponto_tomada = Vector((0.245, tras + 0.001, 0.125))
direcao_entrada = Vector((0.0, -1.0, 0.0))
bloco("teste.tomada", (0.032, 0.001, 0.026), (ponto_tomada.x, tras + 0.0005, ponto_tomada.z),
      material("teste.tomada", (0.30, 0.30, 0.32, 1.0), 0.5))


def luz(nome, tipo, loc, energia, tam, alvo, cor=(1, 1, 1)):
    dados = bpy.data.lights.new(nome, tipo)
    dados.energy = energia
    dados.color = cor
    if tipo == "AREA":
        dados.shape = "RECTANGLE"
        dados.size = tam
        dados.size_y = tam * 0.6
    try:
        dados.use_shadow_jitter = True
    except AttributeError:
        pass
    obj = bpy.data.objects.new(nome, dados)
    obj.location = loc
    col_teste.objects.link(obj)
    obj.rotation_euler = (Vector(alvo) - Vector(loc)).to_track_quat("-Z", "Y").to_euler()
    return obj


alvo_luz = (0.3, 0.6, 0.15)
luz("teste.principal", "AREA", (-1.2, 1.8, 2.2), 260.0, 1.6, alvo_luz, (1.0, 0.96, 0.92))
luz("teste.preenchimento", "AREA", (1.8, 1.4, 1.2), 90.0, 2.4, alvo_luz, (0.92, 0.95, 1.0))
# Contra-luz vinda da frente do cubo, rasante: da o fio de luz na borda do
# plugue e do cabo pretos.
luz("teste.contra", "AREA", (1.2, -1.4, 1.6), 240.0, 1.2, alvo_luz)

# Revisao 4 (trajeto reto): camera a 42 cm, um pouco para +X e ATRAS da
# origem do plugue, olhando ao longo do voo - o trajeto corre em
# profundidade (de lado, 1,2 m de reta nao cabem na largura do 9:16), e a
# camera acima do plugue faz a reta ler como reta, nao como um ponto que
# cresce. No trajeto 'arco' a mesma camera serve: a origem do arco fica ao
# lado.
cam_dados = bpy.data.cameras.new("teste.camera")
cam_dados.lens = 32.0
cam_dados.sensor_fit = "VERTICAL"
cam_dados.sensor_height = 36.0
cam = bpy.data.objects.new("teste.camera", cam_dados)
cam.location = (0.50, 1.95, 0.42)
col_teste.objects.link(cam)
alvo = bpy.data.objects.new("teste.alvo", None)
alvo.location = (0.27, 0.60, 0.15)
col_teste.objects.link(alvo)
tr = cam.constraints.new("TRACK_TO")
tr.target = alvo
tr.track_axis = "TRACK_NEGATIVE_Z"
tr.up_axis = "UP_Y"
cena.camera = cam

# --- o modulo ---
raiz = bpy.data.collections.new("ANUNCIO")
cena.collection.children.link(raiz)
objs = mod_cabo.construir_cabo(cena, raiz, {"ponto_tomada": ponto_tomada, "direcao_entrada": direcao_entrada})
print("[teste] comprimento do cabo em repouso: %.3f m  ponto_fora=%s" % (
    objs["comprimento"], tuple(round(v, 3) for v in objs["ponto_fora"])))

Q_INI, Q_FIM = 1, 100
mod_cabo.animar_conexao(objs, ponto_tomada, direcao_entrada, Q_INI, Q_FIM, trajeto=TRAJETO)
print("[teste] trajeto=%s  ponto_fora=%s" % (TRAJETO, tuple(round(v, 3) for v in objs["ponto_fora"])))

# Conferencia numerica do encaixe: no ultimo quadro a cara do bico tem de
# estar em ponto_tomada, e no meio do clique 3 mm para fora.
for q in (Q_FIM - 4, Q_FIM):
    cena.frame_set(q)
    cara = objs["plugue"].matrix_world @ Vector((0, 0, 0))
    print("[teste] quadro %d: cara do plugue a %.1f mm da tomada" % (q, (cara - ponto_tomada).length * 1000.0))

# Revisao 4: o voo reto e horizontal (z do plugue = z da tomada em todo
# quadro) e alinhado (eixo +Y do plugue = direcao de entrada, 0 graus). Sao
# os dois numeros que dizem "vem reto, na horizontal, alinhado com a normal".
pior_dz, pior_ang, q_dz, q_ang = 0.0, 0.0, None, None
for q in range(Q_INI, Q_FIM + 1):
    cena.frame_set(q)
    m = objs["plugue"].matrix_world
    dz = abs(m.translation.z - ponto_tomada.z)
    eixo = (m.to_3x3() @ Vector((0, 1, 0))).normalized()
    ang = math.degrees(math.acos(max(-1.0, min(1.0, eixo.dot(direcao_entrada)))))
    if dz > pior_dz:
        pior_dz, q_dz = dz, q
    if ang > pior_ang:
        pior_ang, q_ang = ang, q
cena.frame_set(Q_INI)
origem = objs["plugue"].matrix_world.translation.copy()
print("[teste] voo: parte de %s (%.2f m atras da tomada); pior desvio de altura %.1f mm (q%s); pior angulo com a normal %.2f graus (q%s)%s" % (
    tuple(round(v, 3) for v in origem), (origem - ponto_tomada).length, pior_dz * 1000.0, q_dz, pior_ang, q_ang,
    "" if TRAJETO != "reto" else ("  OK" if pior_dz < 0.001 and pior_ang < 0.01 else "  <-- NAO E RETO/HORIZONTAL")))

# Flecha do cabo conectado: o ponto mais baixo da curva avaliada abaixo da
# corda (saida do alivio -> ponto_fora), em fracao do vao - o parametro
# 'catenaria' e 0,035, e o Bezier tem de reproduzi-lo.
cena.frame_set(Q_FIM)
bpy.context.view_layer.update()
dg = bpy.context.evaluated_depsgraph_get()
curva_ev = objs["curva"].evaluated_get(dg)
pontos = []
for sp in curva_ev.data.splines:
    pontos += [objs["curva"].matrix_world @ bp.co for bp in sp.bezier_points]
saida_cabo = objs["plugue"].matrix_world @ Vector((0, -mod_cabo.COMPRIMENTO_PLUGUE, 0))
fora = objs["ponto_fora"]
vao = (fora - saida_cabo).length
# Amostra a curva pela propria formula do modulo (a malha avaliada do bevel
# e um tubo; a spline de controle basta para a flecha da parabola).
if TRAJETO == "reto":
    ctrl = mod_cabo._pontos_cabo_reto(objs["plugue"].matrix_world, fora, objs["catenaria"])
    corda0, corda1 = ctrl[0][0], ctrl[3][0]
    flecha = 0.0
    for i in range(3):
        p0, h0, p1, h1 = ctrl[i][0], ctrl[i][2], ctrl[i + 1][0], ctrl[i + 1][1]
        for k in range(21):
            t = k / 20.0
            pt = mod_cabo._bezier3(p0, h0, h1, p1, t)
            u = max(0.0, min(1.0, (pt - corda0).dot(corda1 - corda0) / max(1e-9, (corda1 - corda0).length_squared)))
            na_corda = corda0 + (corda1 - corda0) * u
            flecha = max(flecha, na_corda.z - pt.z)
    print("[teste] cabo conectado: vao %.2f m, flecha %.3f m = %.3f do vao (parametro %.3f)  %s" % (
        vao, flecha, flecha / max(vao, 1e-9), objs["catenaria"],
        "OK" if abs(flecha / max(vao, 1e-9) - objs["catenaria"]) < 0.006 else "<-- FLECHA FORA"))
    print("[teste] cabo conectado: z na saida %.3f, z em ponto_fora %.3f (tomada %.3f): nao pende para o chao" % (
        saida_cabo.z, fora.z, ponto_tomada.z))


def render(rotulo):
    cena.render.filepath = os.path.join(SAIDA, "previa_cabo_%s%s.png" % (rotulo, SUFIXO))
    bpy.ops.render.render(write_still=True)
    print("[teste] gravado", cena.render.filepath)


# --- tres quadros ---
for rotulo, quadro in (("ini", Q_INI), ("meio", 50), ("fim", Q_FIM)):
    if rotulo not in QUAIS:
        continue
    cena.frame_set(quadro)
    render(rotulo)

# Close no plugue encaixado, de tras/lado: e onde se julga o C13, o chanfro,
# os contatos e se o corpo nao atravessa o cubo.
if "detalhe" in QUAIS:
    cena.frame_set(Q_FIM)
    cam.location = (ponto_tomada.x + 0.13, ponto_tomada.y + 0.17, ponto_tomada.z + 0.09)
    alvo.location = (ponto_tomada.x, ponto_tomada.y + 0.035, ponto_tomada.z - 0.005)
    cam_dados.lens = 85.0
    render("detalhe")

# Cabo conectado visto de lado e um pouco de cima, em diagonal: e onde se
# julga a catenaria - reto para tras com uma flecha leve, sem pender.
# (No trajeto 'arco' a mesma vista mostra a queda ate o chao.)
if "catenaria" in QUAIS or "queda" in QUAIS:
    cena.frame_set(Q_FIM)
    cam.location = (ponto_tomada.x + 1.05, ponto_tomada.y + 0.25, ponto_tomada.z + 0.30)
    alvo.location = (ponto_tomada.x + 0.22, ponto_tomada.y + 0.85, ponto_tomada.z - 0.03)
    cam_dados.lens = 35.0
    render("catenaria")

# Close do plugue no ar: cara com as tres janelas e os contatos, que so
# aparece de frente enquanto ele ainda voa.
if "cara" in QUAIS:
    cena.frame_set(38)
    cara = objs["plugue"].matrix_world
    frente = (cara.to_3x3() @ Vector((0, 1, 0))).normalized()
    pos = cara @ Vector((0, 0, 0))
    cam.location = pos + frente * 0.16 + Vector((0.07, 0.0, 0.05))
    alvo.location = pos - frente * 0.02
    cam_dados.lens = 85.0
    render("cara")
