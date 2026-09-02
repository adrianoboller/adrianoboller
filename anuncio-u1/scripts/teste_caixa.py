# Prova do modulo caixa: cena vazia, luz de tres pontos, camera 3/4 a ~2,5 m,
# tres quadros (fechada / abrindo com espuma no ar / aberta com espuma no
# chao), um close no canto da tampa, um topo ortografico (que MEDE a largura
# e a cor da logo no pixel) e um close da espuma no chao.
# Roda com: bash scripts/previa.sh scripts/teste_caixa.py
#
# Quem roda isto abre os PNGs e olha - o script rodar sem erro nao prova nada.
# Os numeros impressos com [medida] sao os que a revisao pediu: eles saem do
# render e do estado da cena, nunca do que o codigo pretende.

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

# Poucos quadros e poucas amostras: ~10-20 s/quadro por software.
LARGURA, ALTURA = 540, 960
AMOSTRAS = int(os.environ.get("AMOSTRAS", "16"))   # AMOSTRAS=48 para conferir ruido
QUAIS = os.environ.get("QUAIS", "ini,meio,fim,detalhe,topo,espuma").split(",")

# Os PNGs da rodada anterior ficam ao lado, com sufixo, para comparar.
for nome in os.listdir(SAIDA) if os.path.isdir(SAIDA) else []:
    if nome.startswith("previa_caixa_") and nome.endswith(".png") and "_rodada" not in nome:
        antigo = os.path.join(SAIDA, nome)
        guardado = os.path.join(SAIDA, nome[:-4] + "_rodada1.png")
        if not os.path.exists(guardado):
            os.rename(antigo, guardado)

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
PARAMS = {"cor": "clara"}
objs = mod_caixa.construir_caixa(cena, raiz, PARAMS)
print("[teste] topo da tampa z=%.3f  centro_logo=%s  normal=%s  espumas=%d  logo_provisoria=%s" % (
    objs["topo_tampa_z"], tuple(round(v, 3) for v in objs["centro_logo"]),
    tuple(objs["normal_logo"]), len(objs["espumas"]), objs["logo_provisoria"]))

# --- idempotencia: segunda construcao nao pode criar material nem action ---
def _conta(prefixo):
    return (sum(1 for m in bpy.data.materials if m.name.startswith(prefixo)),
            sum(1 for a in bpy.data.actions if a.name.startswith(prefixo)))


mod_caixa.animar_tampa(objs, 1, 60, abrir=True)
mod_caixa.animar_espuma(objs, 12, 90)
antes = _conta("caixa.")
objs = mod_caixa.construir_caixa(cena, raiz, PARAMS)
mod_caixa.animar_tampa(objs, 1, 60, abrir=True)
mod_caixa.animar_espuma(objs, 12, 90)
depois = _conta("caixa.")
print("[medida] materiais 'caixa.*' apos 1a/2a construcao: %d/%d; actions: %d/%d; nomes: %s"
      % (antes[0], depois[0], antes[1], depois[1], sorted(m.name for m in bpy.data.materials if m.name.startswith("caixa."))))
assert depois[0] == antes[0] == 3, "materiais vazaram"
assert depois[1] == antes[1], "actions vazaram"
assert objs["imagem_logo"].packed_file is not None or objs["logo_provisoria"], "logo nao empacotada"
print("[medida] logo empacotada: %s" % (objs["imagem_logo"].packed_file is not None))

# --- espuma em repouso: base do floco acima do topo do U1, nada dentro dele ---
ux, uy, uz = mod_caixa.PARAMS_PADRAO["u1"]
e = mod_caixa.PARAMS_PADRAO["parede"]
ix, iy, iz = objs["interior"]
cena.frame_set(1)
pior_topo, pior_lado, maiores = 1.0, 1.0, []
for esp in objs["espumas"]:
    M = esp.matrix_world
    pts = [M @ v.co for v in esp.data.vertices]
    zmin = min(p.z for p in pts)
    zmax = max(p.z for p in pts)
    xs = [abs(p.x) for p in pts]
    ys = [abs(p.y) for p in pts]
    maiores.append(2 * float(esp["caixa_raio"]))
    assert zmax <= e + iz + 1e-4, "%s atravessa o topo interno da tampa (%.4f)" % (esp.name, zmax)
    assert max(xs) <= ix / 2 + 1e-4 and max(ys) <= iy / 2 + 1e-4, "%s atravessa a parede" % esp.name
    if esp.location.z > e + uz:
        # camada de cima: a base tem de ficar acima do topo do U1 (0,738)
        pior_topo = min(pior_topo, zmin - (e + uz))
    else:
        # vao lateral: nenhum vertice dentro da pegada do U1 (um ponto esta
        # dentro quando sobra folga nos DOIS eixos; a penetracao e a menor)
        penetracao = max(min(ux / 2 - abs(p.x), uy / 2 - abs(p.y)) for p in pts)
        pior_lado = min(pior_lado, -penetracao)
print("[medida] flocos: %d; maior eixo %.1f-%.1f cm (media %.1f); menor folga base->topo do U1 = %+.1f mm; "
      "menor folga lateral ao U1 = %+.1f mm" % (len(maiores), 100 * min(maiores), 100 * max(maiores),
                                                100 * sum(maiores) / len(maiores), 1000 * pior_topo, 1000 * pior_lado))
assert pior_topo >= 0.0025, "floco afunda no topo do U1"
assert pior_lado >= 0.0, "floco dentro do U1"

# --- trajetoria: enquanto abaixo da boca, o floco fica dentro da pegada ---
furos = 0
for i, esp in enumerate(objs["espumas"]):
    raio = float(esp["caixa_raio"])
    _, _, pontos = mod_caixa._trajetoria_espuma(esp, i, 7, objs)
    for _, loc, _ in pontos:
        if loc.z >= objs["exterior_corpo"][2]:
            break          # saiu pela boca: dali em diante pode ir para fora
        if abs(loc.x) > ix / 2 - raio * 0.3 or abs(loc.y) > iy / 2 - raio * 0.3:
            furos += 1
            break
print("[medida] flocos que cruzam a parede antes de sair pela boca: %d" % furos)
assert furos == 0, "espuma atravessa a parede"

# --- quadros ---
def render(rotulo):
    cena.render.filepath = os.path.join(SAIDA, "previa_caixa_%s.png" % rotulo)
    bpy.ops.render.render(write_still=True)
    print("[teste] gravado", cena.render.filepath)
    return cena.render.filepath


for rotulo, quadro in (("ini", 1), ("meio", 30), ("fim", 90)):
    if rotulo in QUAIS:
        cena.frame_set(quadro)
        render(rotulo)

# Close no canto da tampa fechada. E o unico jeito de conferir chanfro e
# nitidez da logo - a 2,5 m eles nao aparecem.
ex, ey, _ = objs["exterior_tampa"]
if "detalhe" in QUAIS:
    cena.frame_set(1)
    cam.location = (ex / 2 + 0.16, -ey / 2 - 0.20, objs["topo_tampa_z"] + 0.11)
    alvo.location = (ex / 2 - 0.04, -ey / 2 + 0.04, objs["topo_tampa_z"] - 0.01)
    cam_dados.lens = 85.0
    render("detalhe")

# Close da espuma no chao (quadro 90): tamanho, facetas e subsurface so se
# veem de perto.
if "espuma" in QUAIS:
    cena.frame_set(90)
    pousos = [Vector(esp["caixa_pouso"]) for esp in objs["espumas"]]
    # o floco mais para a frente/direita, e os dois vizinhos dele
    perto = sorted(pousos, key=lambda p: -(p.x - p.y))[:1][0]
    viz = sorted(pousos, key=lambda p: (p - perto).length)[:4]
    centro = sum(viz, Vector()) / len(viz)
    cam.location = centro + Vector((0.45, -0.55, 0.42))
    alvo.location = centro + Vector((0, 0, 0.02))
    cam_dados.lens = 70.0
    render("espuma")

# Topo ortografico: mede a largura da engrenagem contra a tampa e a cor da
# tinta no pixel. E a prova do 'logo pequena' e da 'logo palida'.
if "topo" in QUAIS:
    cena.frame_set(1)
    cam.constraints.remove(tr)
    cam.location = (0.0, 0.0, 3.0)
    cam.rotation_euler = (0.0, 0.0, 0.0)
    cam_dados.type = "ORTHO"
    cam_dados.sensor_fit = "VERTICAL"
    cam_dados.ortho_scale = 1.6          # 1,6 m na altura -> 600 px/m
    caminho = render("topo")

    import numpy as np
    img = bpy.data.images.load(caminho)
    w, h = img.size
    px = np.empty(w * h * 4, dtype=np.float32)
    img.pixels.foreach_get(px)
    rgb = px.reshape(h, w, 4)[::-1, :, :3]     # linha 0 = topo da imagem
    lum = rgb.mean(axis=2)
    sat = rgb.max(axis=2) - rgb.min(axis=2)
    cx, cy = w // 2, h // 2
    # largura da tampa na imagem: pela geometria (ortho_scale na altura) e
    # conferida pela borda do papel na linha central (o chao tambem e claro,
    # entao a borda e onde a luminancia salta acima do chao)
    px_por_m = h / cam_dados.ortho_scale
    tampa_px = int(round(ex * px_por_m))
    linha = lum[cy]
    chao_lum = np.median(linha[:20])
    claros = np.where(linha > (chao_lum + 1.0) / 2)[0]
    print("[medida] topo: tampa pela geometria %d px, pela borda do papel %d px" % (tampa_px, claros[-1] - claros[0] + 1))
    # so os 80% centrais da tampa: a aresta chanfrada e a sombra da borda
    # tambem sao 'mais escuras que o papel' e entrariam na conta
    m = int(tampa_px * 0.4)
    papel = np.median(lum[cy - m:cy - m + 30, cx - 30:cx + 30])
    tinta = (lum < papel * 0.8) | (sat > 0.12)
    sub = tinta[cy - m:cy + m, cx - m:cx + m]
    cols = np.where(sub.any(axis=0))[0]
    rows = np.where(sub.any(axis=1))[0]
    larg = cols[-1] - cols[0] + 1
    print("[medida] topo: tampa = %d px (%.3f m -> %.0f px/m); engrenagem = %d px = %.1f%% da largura da tampa "
          "(pedido %.0f%%); altura %.1f%%" % (tampa_px, ex, tampa_px / ex, larg, 100.0 * larg / tampa_px,
                                              100 * mod_caixa.PARAMS_PADRAO["largura_logo"],
                                              100.0 * (rows[-1] - rows[0] + 1) / tampa_px))

    def srgb(v):
        v = np.clip(np.asarray(v, dtype=np.float64), 0, 1)
        return np.where(v <= 0.0031308, v * 12.92, 1.055 * v ** (1 / 2.4) - 0.055) * 255

    r0, r1 = cy - m + rows[0], cy - m + rows[-1]
    faixa = slice(cx - m, cx + m)
    cinza_l = slice(r0 + int((r1 - r0) * 0.12), r0 + int((r1 - r0) * 0.38))
    lar_l = slice(r0 + int((r1 - r0) * 0.62), r0 + int((r1 - r0) * 0.88))
    cinza = srgb(rgb[cinza_l, faixa][tinta[cinza_l, faixa]].mean(axis=0))
    lar = srgb(rgb[lar_l, faixa][tinta[lar_l, faixa]].mean(axis=0))
    print("[medida] topo: papel %.0f sRGB; cinza da engrenagem (%.0f,%.0f,%.0f) [fonte 56,58,62; meta <= 80]; "
          "laranja (%.0f,%.0f,%.0f) R-B=%.0f [fonte 201,101,32; meta >= 90]" % (
              (srgb([papel])[0],) + tuple(cinza) + tuple(lar) + (lar[0] - lar[2],)))
