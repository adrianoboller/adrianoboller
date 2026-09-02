# Prova do modulo caixa: cena vazia, luz de tres pontos, camera 3/4 a ~2,5 m,
# tres quadros (fechada / abrindo com espuma no ar / aberta com espuma no
# chao). Roda com: bash scripts/previa.sh scripts/teste_caixa.py
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
import mod_caixa
importlib.reload(mod_caixa)   # para reexecutar dentro do Blender sem reabrir

RAIZ = os.path.dirname(AQUI)
SAIDA = os.path.join(RAIZ, "saida")

# Poucos quadros e poucas amostras: ~6 s/quadro por software.
LARGURA, ALTURA = 540, 960
AMOSTRAS = int(os.environ.get("AMOSTRAS", "16"))   # AMOSTRAS=48 para conferir ruido

# --- cena limpa ---
bpy.ops.wm.read_factory_settings(use_empty=True)
cena = bpy.context.scene
cena.unit_settings.system = "METRIC"
cena.unit_settings.scale_length = 1.0
cena.render.fps = 30
cena.frame_start = 1
cena.frame_end = 90

try:
    cena.render.engine = "BLENDER_EEVEE_NEXT"
except TypeError:
    # Blender 5 renomeou o EEVEE Next de volta para BLENDER_EEVEE.
    cena.render.engine = "BLENDER_EEVEE"
cena.eevee.taa_render_samples = AMOSTRAS
try:
    cena.eevee.use_shadows = True
    cena.eevee.use_raytracing = False
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

# mundo neutro, escuro, para o papel claro ler contra o fundo
mundo = bpy.data.worlds.new("teste.mundo")
cena.world = mundo
mundo.use_nodes = True
fundo = mundo.node_tree.nodes.get("Background")
fundo.inputs["Color"].default_value = (0.12, 0.11, 0.11, 1.0)
fundo.inputs["Strength"].default_value = 0.6

col_teste = bpy.data.collections.new("teste")
cena.collection.children.link(col_teste)

# chao, para a espuma ter onde cair e a caixa ter sombra
bpy.ops.mesh.primitive_plane_add(size=120.0, location=(0, 0, 0))
chao = bpy.context.active_object
chao.name = "teste.chao"
mat_chao = bpy.data.materials.new("teste.chao")
mat_chao.use_nodes = True
b = mat_chao.node_tree.nodes["Principled BSDF"]
b.inputs["Base Color"].default_value = (0.32, 0.30, 0.30, 1.0)
b.inputs["Roughness"].default_value = 0.85
chao.data.materials.append(mat_chao)
for c in chao.users_collection:
    c.objects.unlink(chao)
col_teste.objects.link(chao)


def luz(nome, tipo, loc, energia, tam, cor=(1, 1, 1)):
    dados = bpy.data.lights.new(nome, tipo)
    dados.energy = energia
    dados.color = cor
    if tipo == "AREA":
        dados.shape = "RECTANGLE"
        dados.size = tam
        dados.size_y = tam * 0.6
    try:
        # Sem isto a luz vaza pela parede de 8 mm e a face interna da tampa
        # sai chuviscada (ver cabecalho do mod_caixa). Nao existe antes do 4.2.
        dados.use_shadow_jitter = True
    except AttributeError:
        pass
    obj = bpy.data.objects.new(nome, dados)
    obj.location = loc
    col_teste.objects.link(obj)
    alvo = Vector((0, 0, 0.5))
    obj.rotation_euler = (alvo - Vector(loc)).to_track_quat("-Z", "Y").to_euler()
    return obj


luz("teste.principal", "AREA", (1.8, -2.2, 2.6), 200.0, 1.6, (1.0, 0.96, 0.92))
luz("teste.preenchimento", "AREA", (-2.6, -1.4, 1.6), 80.0, 2.2, (0.92, 0.95, 1.0))
luz("teste.contra", "AREA", (0.6, 2.6, 2.4), 220.0, 1.2)

# camera 3/4 a ~2,5 m
cam_dados = bpy.data.cameras.new("teste.camera")
cam_dados.lens = 35.0
cam_dados.sensor_fit = "VERTICAL"
cam_dados.sensor_height = 36.0
cam = bpy.data.objects.new("teste.camera", cam_dados)
cam.location = (1.55, -1.75, 1.45)
col_teste.objects.link(cam)
alvo = bpy.data.objects.new("teste.alvo", None)
alvo.location = (0.15, 0.0, 0.65)
col_teste.objects.link(alvo)
tr = cam.constraints.new("TRACK_TO")
tr.target = alvo
tr.track_axis = "TRACK_NEGATIVE_Z"
tr.up_axis = "UP_Y"
cena.camera = cam
print("[teste] distancia da camera ao alvo: %.2f m" % (Vector(cam.location) - Vector(alvo.location)).length)

# --- o modulo ---
raiz = bpy.data.collections.new("ANUNCIO")
cena.collection.children.link(raiz)
objs = mod_caixa.construir_caixa(cena, raiz, {"cor": "clara"})
print("[teste] topo da tampa z=%.3f  centro_logo=%s  normal=%s  espumas=%d" % (
    objs["topo_tampa_z"], tuple(round(v, 3) for v in objs["centro_logo"]),
    tuple(objs["normal_logo"]), len(objs["espumas"])))

mod_caixa.animar_tampa(objs, 1, 60, abrir=True)
mod_caixa.animar_espuma(objs, 12, 90)

# --- tres quadros ---
for rotulo, quadro in (("ini", 1), ("meio", 30), ("fim", 90)):
    cena.frame_set(quadro)
    cena.render.filepath = os.path.join(SAIDA, "previa_caixa_%s.png" % rotulo)
    bpy.ops.render.render(write_still=True)
    print("[teste] gravado", cena.render.filepath)

# Quarto quadro: close no canto da tampa fechada. E o unico jeito de conferir
# chanfro e nitidez da logo - a 2,5 m eles nao aparecem.
cena.frame_set(1)
# Canto frontal direito da tampa em primeiro plano, logo ao fundo: a aresta
# chanfrada e o que tem de aparecer aqui.
ex, ey, _ = objs["exterior_tampa"]
cam.location = (ex / 2 + 0.16, -ey / 2 - 0.20, objs["topo_tampa_z"] + 0.11)
alvo.location = (ex / 2 - 0.04, -ey / 2 + 0.04, objs["topo_tampa_z"] - 0.01)
cam_dados.lens = 85.0
cena.render.filepath = os.path.join(SAIDA, "previa_caixa_detalhe.png")
bpy.ops.render.render(write_still=True)
print("[teste] gravado", cena.render.filepath)
