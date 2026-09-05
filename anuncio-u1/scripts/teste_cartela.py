# Prova do modulo cartela: cena vazia, fundo escuro com o brilho rose no terco
# de baixo (como a camera do beat 7 ve o gradiente), camera 35 mm 9:16 a 2 m
# com DoF f/2,8 focado na raiz, 540x960. Seis quadros da cartela (agora com
# CINCO linhas - revisao 4) e dois da legenda "Snapmaker U1" do momento-heroi
# (legenda_nascendo, legenda), a 360x640:
#   ini / meio / fim / saida  - a entrada e a saida de sempre;
#   celular                   - o quadro assentado a 360x640, para ler cada
#                               linha no tamanho de um celular (o criterio);
#   matchcut                  - q_ini+1 com logo_ja_visivel=True: a logo tem
#                               de estar inteira e o texto ainda ausente;
#   matchcut_centro           - o mesmo com logo_origem=(0,0) e escala 2,5:
#                               a logo nasce grande no centro do quadro (como
#                               a logo da tampa no fim do mergulho) e, no
#                               quadro seguinte (matchcut_viagem, meio da sua
#                               fatia), ja esta a caminho do repouso com a
#                               marca entrando.
# Roda com:
#   bash scripts/previa.sh scripts/teste_cartela.py
#
# Quem roda isto abre os PNGs e olha: legivel em celular (a linha 4 nao pode
# ser pequena), centrado, nada cortado nas bordas, hierarquia clara, marca
# sem peso de bold. O script tambem MEDE: a fracao da largura do quadro que
# cada linha ocupa (assentada e no inicio - e o numero que diz se corta), a
# altura da linha 4 no quadro (tem de ficar acima de ~70%), e, lendo o PNG
# assentado de volta, o maximo sRGB do branco e o R/B do cobre (R >= 180,
# B <= 80) - os criterios da revisao da rodada 1, em numero. O branco pedido
# era >= 0,95; medido, sob AgX Medium High Contrast isso exige forca ~3,7,
# acima do limiar de bloom do mod_ambiente (2,5) - o criterio aqui e o
# maximo sem bloom, >= 0,90 (forca 2,4 mede 0,92).

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
CELULAR = (360, 640)
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

# Fundo: o ponto escuro da paleta com o brilho rose embaixo. A coreografia
# poe a camera do beat 7 olhando 24 graus para cima, o horizonte a ~96% da
# altura e o brilho so no terco de baixo - a faixa aqui imita ISSO (medido na
# rodada 1 a faixa do teste ficava a 62-75%, onde a coreografia nao a tem), e
# e sobre a subida dela que a linha 4 tem de continuar legivel.
mundo = bpy.data.worlds.new("teste.mundo")
cena.world = mundo
mundo.use_nodes = True
nt = mundo.node_tree
fundo = nt.nodes.get("Background")
fundo.inputs["Strength"].default_value = 1.0
coord = nt.nodes.new("ShaderNodeTexCoord")
sep = nt.nodes.new("ShaderNodeSeparateXYZ")
# O Generated do world e a DIRECAO de visada (Z de -1 a 1); o ColorRamp so
# aceita 0..1, entao fac = (Z + 1) / 2. Paradas medidas contra o render:
# 0,10 -> pe do quadro, pico em 0,17 (~95% da altura), preto de novo em 0,36
# (~65%).
rampa = nt.nodes.new("ShaderNodeValToRGB")
rampa.color_ramp.interpolation = "EASE"
rampa.color_ramp.elements[0].position = 0.10
rampa.color_ramp.elements[0].color = tuple(0.04 * c for c in mod_cartela.cor_linear("#F4E6E4")[:3]) + (1.0,)
rampa.color_ramp.elements[1].position = 0.17
rampa.color_ramp.elements[1].color = tuple(0.16 * c for c in mod_cartela.cor_linear("#F4E6E4")[:3]) + (1.0,)
e = rampa.color_ramp.elements.new(0.36)
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
# Como a coreografia: parented na camera, com a subida do param.
mod_cartela.posicionar_cartela(objs, cam, parentear=True)
# DoF da camera do projeto (f/2,8) com o foco na raiz: a cartela tem de sair
# NITIDA - a revisao apontou que nada provava isso com DoF ligado.
cam_dados.dof.use_dof = True
cam_dados.dof.focus_object = objs["raiz"]
cam_dados.dof.aperture_fstop = 2.8

print("[teste] fontes: fina=%s forte=%s" % (objs["fonte_fina"], objs["fonte_forte"]))
print("[teste] logo empacotada: %s" % (bool(objs["imagem_logo"].packed_file) if objs["imagem_logo"] else "sem logo"))
print("[teste] largura_max=%.3f m  bloco=%.3f x %.3f m  subida=%.2f m" % (
    objs["largura_max"], objs["largura"], objs["altura"], objs["subida"]))
for i, (tam, larg) in enumerate(zip(objs["tamanhos"], objs["larguras"])):
    print("[teste] linha %d: tamanho %.3f m (x-altura ~%.0f px em 1920, ~%.0f em 640)  largura %.3f m  esp final %.3f" % (
        i + 1, tam, 0.52 * tam / (2.0 * 36.0 / 35.0) * 1920, 0.52 * tam / (2.0 * 36.0 / 35.0) * 640, larg,
        objs["espacamentos_finais"][i]))

mod_cartela.animar_cartela(objs, *Q_ENTRADA)
mod_cartela.animar_cartela_saida(objs, *Q_SAIDA)


def projecao(obj):
    """Cantos do bound_box projetados pela camera (35 mm, sensor 36 no lado
    maior): x e y normalizados em -1..1 (y para cima)."""
    inv = cam.matrix_world.inverted()
    xs, ys = [], []
    for v in obj.bound_box:
        p = inv @ (obj.matrix_world @ Vector(v))
        if p.z >= -1e-6:
            continue
        # sensor de 36 mm no lado maior (altura): +-18 mm cobre +-1 em Y.
        ys.append(p.y / -p.z * 35.0 / 18.0)
        xs.append(p.x / -p.z * 35.0 / 18.0 / (LARGURA / float(ALTURA)))
    return xs, ys


def fracao_no_quadro(obj):
    """Fracao da largura e da altura do quadro ocupada pelo objeto e a borda
    mais distante do centro (> 1 = corta)."""
    xs, ys = projecao(obj)
    if not xs:
        return 0.0, 0.0, 0.0
    return (max(xs) - min(xs)) / 2.0, (max(ys) - min(ys)) / 2.0, max(abs(v) for v in xs)


def faixa_vertical(obj):
    """Topo e fundo do objeto em fracao da altura do quadro (0 = topo)."""
    _, ys = projecao(obj)
    if not ys:
        return 0.0, 0.0
    return (1.0 - max(ys)) / 2.0, (1.0 - min(ys)) / 2.0


def medir_png(caminho, faixas):
    """Le o PNG de volta e, em cada faixa vertical (y0, y1 em fracao), devolve
    o maximo sRGB por canal e a mediana dos pixels claros - o numero que diz
    se o branco e branco e se o cobre continua cobre depois do AgX."""
    import numpy as np
    img = bpy.data.images.load(caminho, check_existing=False)
    w, h = img.size
    px = np.empty(w * h * 4, dtype=np.float32)
    img.pixels.foreach_get(px)
    a = px.reshape(h, w, 4)[::-1, :, :3]        # linha 0 = topo
    a8 = np.clip(a * 255.0 + 0.5, 0, 255).astype(int)
    saida = []
    for y0, y1 in faixas:
        f = a8[int(y0 * h):max(int(y1 * h), int(y0 * h) + 1)]
        lum = f.sum(axis=2)
        claros = f[lum > lum.max() * 0.6] if lum.max() > 0 else f.reshape(-1, 3)
        saida.append((f.reshape(-1, 3).max(axis=0), np.median(claros, axis=0)))
    bpy.data.images.remove(img)
    return saida


def render(nome, quadro, tamanho=(LARGURA, ALTURA)):
    cena.render.resolution_x, cena.render.resolution_y = tamanho
    cena.frame_set(quadro)
    bpy.context.view_layer.update()
    cena.render.filepath = os.path.join(SAIDA, "previa_cartela_%s.png" % nome)
    bpy.ops.render.render(write_still=True)
    print("[teste] gravado", cena.render.filepath)
    return cena.render.filepath


quadros = (
    ("ini", Q_ENTRADA[0] + 9),
    ("meio", (Q_ENTRADA[0] + Q_ENTRADA[1]) // 2),
    ("fim", Q_ENTRADA[1]),
    ("saida", (Q_SAIDA[0] + Q_SAIDA[1]) // 2),
)
png_fim = None
for rotulo, quadro in quadros:
    cena.frame_set(quadro)
    bpy.context.view_layer.update()
    for i, obj in enumerate(objs["linhas"]):
        fx, fy, borda = fracao_no_quadro(obj)
        topo, fundo_ = faixa_vertical(obj)
        aviso = "  <-- CORTA" if borda > 1.0 else ""
        print("[teste] q%3d linha %d: %.2f da largura, %.3f da altura, y %.1f%%-%.1f%%, borda em %.2f  esp=%.2f (ini %.2f)%s" % (
            quadro, i + 1, fx, fy, 100 * topo, 100 * fundo_, borda, obj.data.space_character,
            objs["espacamentos_iniciais"][i], aviso))
    caminho = render(rotulo, quadro)
    if rotulo == "fim":
        png_fim = caminho

# Medicao no PNG assentado: uma faixa por linha (fundo da de cima ate o topo
# da de baixo nao entra, so a propria linha). A linha de destaque (cobre) e
# a ultima linha vem do modulo, nao de um indice fixo: a revisao 4 pos uma
# quinta linha e o cobre desceu da 3 para a 4.
cena.frame_set(Q_ENTRADA[1])
bpy.context.view_layer.update()
faixas = [faixa_vertical(o) for o in objs["linhas"]]
medidas = medir_png(png_fim, faixas)
n_linhas = len(objs["linhas"])
i_cobre = (mod_cartela.PARAMS_PADRAO["linha_destaque"] or 0) - 1
brancas = [i for i in range(n_linhas) if i != i_cobre]
for i, ((maximo, mediana), (topo, fundo_)) in enumerate(zip(medidas, faixas)):
    print("[teste] PNG fim linha %d (y %.1f%%-%.1f%%): max sRGB %s  mediana dos claros %s" % (
        i + 1, 100 * topo, 100 * fundo_, list(maximo), [int(v) for v in mediana]))
branco = min(medidas[i][0].min() for i in brancas) / 255.0
print("[teste] CRITERIO branco: min do max por canal nas linhas %s = %.3f sRGB (>= 0,90 sem bloom: %s)" % (
    "/".join(str(i + 1) for i in brancas), branco, "OK" if branco >= 0.90 else "FALHA"))
if 0 <= i_cobre < n_linhas:
    cobre = medidas[i_cobre][1]
    print("[teste] CRITERIO cobre (linha %d): R=%d B=%d (R >= 180 e B <= 80: %s)" % (
        i_cobre + 1, cobre[0], cobre[2], "OK" if cobre[0] >= 180 and cobre[2] <= 80 else "FALHA"))
print("[teste] CRITERIO linha %d assentada: fundo a %.1f%% da altura (< 70%%: %s)" % (
    n_linhas, 100 * faixas[-1][1], "OK" if faixas[-1][1] < 0.70 else "FALHA"))
topo_bloco = faixa_vertical(objs["logo"])[0] if objs.get("logo") is not None else faixas[0][0]
print("[teste] bloco assentado: do topo da logo a %.1f%% ao fundo da linha %d a %.1f%% da altura (%.1f%% do quadro)" % (
    100 * topo_bloco, n_linhas, 100 * faixas[-1][1], 100 * (faixas[-1][1] - topo_bloco)))

# O criterio de verdade e o celular: o quadro assentado a 360x640.
render("celular", Q_ENTRADA[1], CELULAR)

# --- legenda do momento-heroi (revisao 4, item 4a) ---
# Construida na mesma camera (filha dela), animada DEPOIS da saida da
# cartela (q110..q150: a cartela ja esta com alfa 0) e renderizada a 360x640
# no meio do fade in (linha nascendo) e assentada: o criterio e ler
# "Snapmaker U1" num celular e a linha de 1 px existir a 360 px de largura.
legenda = mod_cartela.construir_legenda(cena, raiz, cam, "Snapmaker U1")
Q_LEG = (110, 150)
mod_cartela.animar_legenda(legenda, *Q_LEG)
cena.frame_end = Q_LEG[1] + 1
# DoF como no momento-heroi: f/5,6 com o foco no produto, que esta a ~1,25 m
# da camera - a legenda a 1,2 m. Medido: com o foco da cartela (2 m, f/2,8)
# a legenda saia borrada, e isso era o teste, nao o modulo.
cam_dados.dof.focus_object = None
cam_dados.dof.focus_distance = 1.25
cam_dados.dof.aperture_fstop = 5.6
# Fundo como o topo do quadro do heroi (rose claro, L ~205 medido em q180):
# a legenda e tinta escura e so se prova sobre ele - sobre o preto da
# cartela nao apareceria. A rampa do world e substituida por uma cor lisa.
for lk in list(fundo.inputs["Color"].links):
    nt.links.remove(lk)
fundo.inputs["Color"].default_value = tuple(0.75 * c for c in mod_cartela.cor_linear("#F4E6E4")[:3]) + (1.0,)
em_px_640 = legenda["em"] / (2.0 * legenda["meia_altura"]) * 640
print("[teste] legenda: fonte %s, em %.4f m a %.2f m = %.1f px em 640 (x-altura ~%.0f px), texto %.3f m = %.0f%% da largura, linha %.2f mm (%.2f px em 640)" % (
    legenda["fonte"], legenda["em"], legenda["distancia"], em_px_640, 0.52 * em_px_640,
    legenda["largura_texto"], 100 * legenda["largura_texto"] / (2.0 * legenda["meia_largura"]),
    legenda["espessura"] * 1000.0, legenda["espessura"] / (2.0 * legenda["meia_altura"]) * 640))
alfa_leg, _ = mod_cartela._no_alfa(legenda["texto"])
for q in (Q_LEG[0] - 1, Q_LEG[0] + 4, Q_LEG[0] + 8, Q_LEG[1] - 8, Q_LEG[1], Q_LEG[1] + 1):
    cena.frame_set(q)
    bpy.context.view_layer.update()
    print("[teste] legenda q%d: alfa %.2f  linha escala %.2f  hide_render texto=%s linha=%s" % (
        q, alfa_leg.default_value, legenda["linha"].scale.x, legenda["texto"].hide_render, legenda["linha"].hide_render))
render("legenda_nascendo", Q_LEG[0] + 5, CELULAR)
render("legenda", Q_LEG[0] + 20, CELULAR)
cena.render.resolution_x, cena.render.resolution_y = LARGURA, ALTURA

# Match cut: mesma coreografia, mas a logo ja inteira em q_ini. As chaves
# caem nos mesmos quadros da entrada normal (mesmo calendario), entao da para
# reanimar por cima; q_ini+1 tem de mostrar a logo inteira e nenhuma linha.
mod_cartela.animar_cartela(objs, *Q_ENTRADA, logo_ja_visivel=True)
alfa_logo, _ = mod_cartela._no_alfa(objs["logo"])
for q in (Q_ENTRADA[0] - 1, Q_ENTRADA[0], Q_ENTRADA[0] + 1):
    cena.frame_set(q)
    bpy.context.view_layer.update()
    print("[teste] matchcut q%d: alfa da logo = %.2f" % (q, alfa_logo.default_value))
render("matchcut", Q_ENTRADA[0] + 1)

# Variante do match cut com a logo nascendo grande e centrada.
mod_cartela.animar_cartela(objs, *Q_ENTRADA, logo_ja_visivel=True, logo_origem=(0.0, 0.0),
                           logo_escala_inicial=2.5)
render("matchcut_centro", Q_ENTRADA[0] + 1)
render("matchcut_viagem", Q_ENTRADA[0] + 20)
