# Prova do modulo cartela: cena vazia, fundo escuro, camera 35 mm 9:16 a 2 m
# olhando a cartela, 540x960, quatro quadros: inicio da entrada, meio,
# assentada e meio da saida. Roda com:
#   bash scripts/previa.sh scripts/teste_cartela.py
#
# Quem roda isto abre os PNGs e olha: legivel em celular (a linha 2 nao pode
# ser pequena), centrado, nada cortado nas bordas, hierarquia clara. O script
# tambem MEDE a fracao da largura do quadro que cada linha ocupa, com o
# tracking assentado e com o inicial - e o numero que diz se corta.

import math
import os
import sys

import bpy
from mathutils import Vector

AQUI = os.path.dirname(os.path.abspath(__file__))
if AQUI not in sys.path:
    sys.path.insert(0, AQUI)
import importlib
import mod_cartela
importlib.reload(mod_cartela)   # para reexecutar dentro do Blender sem reabrir

RAIZ = os.path.dirname(AQUI)
SAIDA = os.path.join(RAIZ, "saida")

LARGURA, ALTURA = 540, 960
AMOSTRAS = int(os.environ.get("AMOSTRAS", "16"))
Q_ENTRADA = (1, 60)      # 2 s de entrada
Q_SAIDA = (75, 95)

# --- cena limpa ---
bpy.ops.wm.read_factory_settings(use_empty=True)
cena = bpy.context.scene
cena.unit_settings.system = "METRIC"
cena.unit_settings.scale_length = 1.0
cena.render.fps = 30
cena.frame_start = 1
cena.frame_end = Q_SAIDA[1]

try:
    cena.render.engine = "BLENDER_EEVEE_NEXT"
except TypeError:
    # Blender 5 renomeou o EEVEE Next de volta para BLENDER_EEVEE.
    cena.render.engine = "BLENDER_EEVEE"
cena.eevee.taa_render_samples = AMOSTRAS
cena.render.resolution_x = LARGURA
cena.render.resolution_y = ALTURA
cena.render.resolution_percentage = 100
cena.render.image_settings.file_format = "PNG"
cena.view_settings.view_transform = "AgX"
try:
    cena.view_settings.look = "AgX - Medium High Contrast"
except TypeError:
    pass

# Fundo: o ponto escuro da paleta, com um leve brilho rose embaixo, imitando
# o gradiente do modulo ambiente - e sobre a faixa clara que a linha 4 tem
# de continuar legivel.
mundo = bpy.data.worlds.new("teste.mundo")
cena.world = mundo
mundo.use_nodes = True
nt = mundo.node_tree
fundo = nt.nodes.get("Background")
fundo.inputs["Strength"].default_value = 1.0
coord = nt.nodes.new("ShaderNodeTexCoord")
sep = nt.nodes.new("ShaderNodeSeparateXYZ")
# O Generated do world e a DIRECAO de visada (Z de -1 a 1); o ColorRamp so
# aceita 0..1, entao fac = (Z + 1) / 2. A faixa fica ABAIXO do horizonte
# (Z de -0,45 a -0,15), que e onde a camera a 2 m ve o pe do quadro.
rampa = nt.nodes.new("ShaderNodeValToRGB")
rampa.color_ramp.interpolation = "EASE"
rampa.color_ramp.elements[0].position = 0.26
rampa.color_ramp.elements[0].color = mod_cartela.cor_linear("#050507")
rampa.color_ramp.elements[1].position = 0.32
rampa.color_ramp.elements[1].color = tuple(0.10 * c for c in mod_cartela.cor_linear("#F4E6E4")[:3]) + (1.0,)
e = rampa.color_ramp.elements.new(0.46)
e.color = mod_cartela.cor_linear("#050507")
mapa = nt.nodes.new("ShaderNodeMath")
mapa.operation = "MULTIPLY_ADD"
mapa.inputs[1].default_value = 0.5
mapa.inputs[2].default_value = 0.5
nt.links.new(coord.outputs["Generated"], sep.inputs[0])
nt.links.new(sep.outputs["Z"], mapa.inputs[0])
nt.links.new(mapa.outputs[0], rampa.inputs["Fac"])
nt.links.new(rampa.outputs["Color"], fundo.inputs["Color"])

col_teste = bpy.data.collections.new("teste")
cena.collection.children.link(col_teste)

# camera 35 mm, sensor 36 no lado maior (como mod_ambiente), a 2 m
cam_dados = bpy.data.cameras.new("teste.camera")
cam_dados.lens = 35.0
cam_dados.sensor_fit = "AUTO"
cam_dados.sensor_width = 36.0
cam = bpy.data.objects.new("teste.camera", cam_dados)
cam.location = (0.0, -2.0, 1.0)
cam.rotation_euler = (math.pi / 2.0, 0.0, 0.0)   # olhando +Y, Z para cima
col_teste.objects.link(cam)
cena.camera = cam

# --- o modulo ---
raiz = bpy.data.collections.new("ANUNCIO")
cena.collection.children.link(raiz)
objs = mod_cartela.construir_cartela(cena, raiz)
mod_cartela.posicionar_cartela(objs, cam)
print("[teste] fontes: fina=%s forte=%s" % (objs["fonte_fina"], objs["fonte_forte"]))
print("[teste] largura_max=%.3f m  bloco=%.3f x %.3f m" % (objs["largura_max"], objs["largura"], objs["altura"]))
for i, (tam, larg) in enumerate(zip(objs["tamanhos"], objs["larguras"])):
    print("[teste] linha %d: tamanho %.3f m (x-altura ~%.0f px em 1920)  largura %.3f m" % (
        i + 1, tam, 0.52 * tam / (2.0 * 36.0 / 35.0) * 1920, larg))

mod_cartela.animar_cartela(objs, *Q_ENTRADA)
mod_cartela.animar_cartela_saida(objs, *Q_SAIDA)


def fracao_no_quadro(obj):
    """Fracao da largura e da altura do quadro ocupada pelo objeto, projetando
    os cantos do bound_box com a camera (35 mm, sensor 36 no lado maior)."""
    inv = cam.matrix_world.inverted()
    xs, ys = [], []
    for v in obj.bound_box:
        p = inv @ (obj.matrix_world @ Vector(v))
        if p.z >= -1e-6:
            continue
        # sensor de 36 mm no lado maior (altura): +-18 mm cobre +-1 em Y.
        ys.append(p.y / -p.z * 35.0 / 18.0)
        xs.append(p.x / -p.z * 35.0 / 18.0 / (LARGURA / float(ALTURA)))
    if not xs:
        return 0.0, 0.0, 0.0
    return (max(xs) - min(xs)) / 2.0, (max(ys) - min(ys)) / 2.0, max(abs(v) for v in xs)


quadros = (
    ("ini", Q_ENTRADA[0] + 9),
    ("meio", (Q_ENTRADA[0] + Q_ENTRADA[1]) // 2),
    ("fim", Q_ENTRADA[1]),
    ("saida", (Q_SAIDA[0] + Q_SAIDA[1]) // 2),
)
for rotulo, quadro in quadros:
    cena.frame_set(quadro)
    bpy.context.view_layer.update()
    for i, obj in enumerate(objs["linhas"]):
        fx, fy, borda = fracao_no_quadro(obj)
        aviso = "  <-- CORTA" if borda > 1.0 else ""
        print("[teste] q%3d linha %d: %.2f da largura, %.3f da altura, borda em %.2f  esp=%.2f (ini %.2f)%s" % (
            quadro, i + 1, fx, fy, borda, obj.data.space_character, objs["espacamentos_iniciais"][i], aviso))
    cena.render.filepath = os.path.join(SAIDA, "previa_cartela_%s.png" % rotulo)
    bpy.ops.render.render(write_still=True)
    print("[teste] gravado", cena.render.filepath)
