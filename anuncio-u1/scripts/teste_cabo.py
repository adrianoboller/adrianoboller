# Prova do modulo cabo: cena vazia com um cubo grafite no lugar do U1, um
# retangulo marcando a tomada na traseira (ponto e direcao conhecidos), luz
# simples e camera atras em 3/4. Tres quadros (plugue longe / meio do arco /
# encaixado) mais um close no plugue encaixado, porque a 1,8 m um C13 tem
# 15 pixels e nao da para julgar se parece um C13.
# Roda com: bash scripts/previa.sh scripts/teste_cabo.py
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
QUAIS = os.environ.get("QUAIS", "ini,meio,fim,detalhe,queda,cara").split(",")

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

# Camera atras, 3/4, do lado +X e ATRAS da origem do plugue: o arco corre em
# profundidade, que e o eixo comprido do quadro 9:16. Com a camera de lado o
# arco corria na horizontal, e ou a origem ou a tomada caia fora do quadro.
# Camera baixa (30 cm): de cima, o plugue no alto do arco se projetava
# contra a base do cubo e parecia estar no chao.
cam_dados = bpy.data.cameras.new("teste.camera")
cam_dados.lens = 32.0
cam_dados.sensor_fit = "VERTICAL"
cam_dados.sensor_height = 36.0
cam = bpy.data.objects.new("teste.camera", cam_dados)
cam.location = (1.0, 1.60, 0.30)
col_teste.objects.link(cam)
alvo = bpy.data.objects.new("teste.alvo", None)
alvo.location = (0.32, 0.60, 0.16)
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
mod_cabo.animar_conexao(objs, ponto_tomada, direcao_entrada, Q_INI, Q_FIM)

# Conferencia numerica do encaixe: no ultimo quadro a cara do bico tem de
# estar em ponto_tomada, e no meio do clique 3 mm para fora.
for q in (Q_FIM - 4, Q_FIM):
    cena.frame_set(q)
    cara = objs["plugue"].matrix_world @ Vector((0, 0, 0))
    print("[teste] quadro %d: cara do plugue a %.1f mm da tomada" % (q, (cara - ponto_tomada).length * 1000.0))

# --- tres quadros ---
for rotulo, quadro in (("ini", Q_INI), ("meio", 50), ("fim", Q_FIM)):
    if rotulo not in QUAIS:
        continue
    cena.frame_set(quadro)
    cena.render.filepath = os.path.join(SAIDA, "previa_cabo_%s.png" % rotulo)
    bpy.ops.render.render(write_still=True)
    print("[teste] gravado", cena.render.filepath)

# Close no plugue encaixado, de tras/lado: e onde se julga o C13, o chanfro,
# os contatos e se o corpo nao atravessa o cubo.
if "detalhe" in QUAIS:
    cena.frame_set(Q_FIM)
    cam.location = (ponto_tomada.x + 0.13, ponto_tomada.y + 0.17, ponto_tomada.z + 0.09)
    alvo.location = (ponto_tomada.x, ponto_tomada.y + 0.035, ponto_tomada.z - 0.005)
    cam_dados.lens = 85.0
    cena.render.filepath = os.path.join(SAIDA, "previa_cabo_detalhe.png")
    bpy.ops.render.render(write_still=True)
    print("[teste] gravado", cena.render.filepath)

# Plugue encaixado visto de lado, a meio metro: e onde se julga a curva de
# gravidade do cabo ate o chao.
if "queda" in QUAIS:
    cena.frame_set(Q_FIM)
    cam.location = (ponto_tomada.x + 0.62, ponto_tomada.y + 0.30, ponto_tomada.z + 0.10)
    alvo.location = (ponto_tomada.x + 0.02, ponto_tomada.y + 0.16, ponto_tomada.z - 0.05)
    cam_dados.lens = 50.0
    cena.render.filepath = os.path.join(SAIDA, "previa_cabo_queda.png")
    bpy.ops.render.render(write_still=True)
    print("[teste] gravado", cena.render.filepath)

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
    cena.render.filepath = os.path.join(SAIDA, "previa_cabo_cara.png")
    bpy.ops.render.render(write_still=True)
    print("[teste] gravado", cena.render.filepath)
