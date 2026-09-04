# Prova do modulo caixa (versao 2, caixa de papelao da Meshy): cena vazia,
# SEM chao e sem plano (Revisao 2: os objetos flutuam; o World e um cinza
# escuro liso), luz de tres pontos, e seis quadros:
#   ini        fechada, 3/4 de frente
#   meio       abas no meio da abertura, espuma no ar
#   fim        abas abertas, espuma ja fora do quadro
#   topo_close close do topo: fita pela emenda e papelao limpo (a logo saiu
#              a pedido do cliente; COM_LOGO=1 religa o decal para prova)
#   etiqueta   close da etiqueta pendurada (a malha da Meshy)
#   topo       ortografico de cima: sem logo MEDE que o miolo nao tem tinta;
#              com COM_LOGO=1 mede largura, centro e cor da logo
# Roda com: bash scripts/previa.sh scripts/teste_caixa.py
#   QUAIS=ini,fim  AMOSTRAS=48  para recortar / conferir ruido.
#   Os seis quadros por software (llvmpipe) levam ~1 min cada; se a memoria
#   apertar, divida com QUAIS.
#
# Quem roda isto abre os PNGs e olha - o script rodar sem erro nao prova nada.
# Os numeros [medida] saem do render e do estado da cena, nunca do que o
# codigo pretende.

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
importlib.reload(mod_caixa)

RAIZ = os.path.dirname(AQUI)
SAIDA = os.path.join(RAIZ, "saida")
os.makedirs(SAIDA, exist_ok=True)

LARGURA, ALTURA = 540, 960
AMOSTRAS = int(os.environ.get("AMOSTRAS", "16"))
QUAIS = os.environ.get("QUAIS", "ini,meio,fim,topo_close,etiqueta,topo").split(",")

# Os PNGs da versao branca ficam ao lado, com sufixo, para comparar. So os
# seis quadros que ela tinha: um rename generico rebatizava os quadros novos
# (topo_close, etiqueta) da rodada anterior como se fossem dela.
for rotulo in ("ini", "meio", "fim", "detalhe", "espuma", "topo"):
    antigo = os.path.join(SAIDA, "previa_caixa_%s.png" % rotulo)
    guardado = os.path.join(SAIDA, "previa_caixa_%s_v1branca.png" % rotulo)
    if os.path.exists(antigo) and not os.path.exists(guardado):
        os.rename(antigo, guardado)

# --- cena limpa (SEM isto a cena tem o cubo padrao de 2 m dentro do qual a
# caixa some - foi o que enganou o primeiro teste de geometria) ---
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

mundo = bpy.data.worlds.new("teste.mundo")
cena.world = mundo
mundo.use_nodes = True
fundo = mundo.node_tree.nodes.get("Background")
fundo.inputs["Color"].default_value = (0.11, 0.105, 0.105, 1.0)
fundo.inputs["Strength"].default_value = 0.6

col_teste = bpy.data.collections.new("teste")
cena.collection.children.link(col_teste)


def luz(nome, tipo, loc, energia, tam, cor=(1, 1, 1), alvo=(0, 0, 0.5)):
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


luz("teste.principal", "AREA", (1.8, -2.2, 2.6), 200.0, 1.6, (1.0, 0.96, 0.92))
luz("teste.preenchimento", "AREA", (-2.6, -1.4, 1.6), 80.0, 2.2, (0.92, 0.95, 1.0))
luz("teste.contra", "AREA", (0.6, 2.6, 2.4), 220.0, 1.2)

cam_dados = bpy.data.cameras.new("teste.camera")
cam_dados.lens = 35.0
cam_dados.sensor_fit = "VERTICAL"
cam_dados.sensor_height = 36.0
cam = bpy.data.objects.new("teste.camera", cam_dados)
col_teste.objects.link(cam)
alvo = bpy.data.objects.new("teste.alvo", None)
col_teste.objects.link(alvo)
tr = cam.constraints.new("TRACK_TO")
tr.target = alvo
tr.track_axis = "TRACK_NEGATIVE_Z"
tr.up_axis = "UP_Y"
cena.camera = cam


def enquadrar(loc, mira, lente=35.0):
    cam.location = loc
    alvo.location = mira
    cam_dados.lens = lente


# Revisao 2, item 3: produto >= 60% da altura do quadro nos planos gerais.
# Medido: a 1,55 m com 35 mm a caixa fechada (0,8 m) ocupa ~65%.
enquadrar((1.10, -1.25, 1.05), (0.0, 0.0, 0.40))

# --- o modulo ---
raiz = bpy.data.collections.new("ANUNCIO")
cena.collection.children.link(raiz)
# COM_LOGO=1 religa o decal (desligado a pedido do cliente) para provar que
# o caminho continua funcionando; o padrao e a caixa limpa.
# RES_TEX=4k troca para o conjunto 4096^2 (o padrao do modulo e 2k).
PARAMS = {"com_logo": os.environ.get("COM_LOGO", "0") == "1",
          "resolucao_texturas": os.environ.get("RES_TEX", "2k")}
objs = mod_caixa.construir_caixa(cena, raiz, PARAMS)
assert objs["com_logo"] == PARAMS["com_logo"]
assert objs["logo_provisoria"] is False or PARAMS["com_logo"], "logo_provisoria acusou sem logo"
print("[teste] topo z=%.3f  centro_logo=%s  interior=%s  abas=%d  etiqueta=%s  espumas=%d  ppm=%.0f  texturas ausentes=%s" % (
    objs["topo_tampa_z"], tuple(round(v, 3) for v in objs["centro_logo"]), tuple(round(v, 3) for v in objs["interior"]),
    len(objs["abas"]), objs["etiqueta"].name if objs["etiqueta"] else None, len(objs["espumas"]), objs["ppm"], objs["texturas_ausentes"]))
assert not objs["texturas_ausentes"], "faltam texturas do bake: rode scripts/bake_caixa.py"
assert objs["etiqueta"] is not None, "etiqueta nao carregou"
for k in ("corpo", "tampa", "abas", "etiqueta", "logo", "espumas", "interior", "exterior_corpo", "exterior_tampa",
          "altura_tampa", "topo_tampa_z", "base_tampa_z", "centro_logo", "centro_logo_local", "normal_logo", "largura_logo"):
    assert k in objs, "chave %s sumiu da API" % k

# --- idempotencia: segunda construcao nao pode criar material nem action ---
def _conta(prefixo):
    return (sum(1 for m in bpy.data.materials if m.name.startswith(prefixo)),
            sum(1 for a in bpy.data.actions if a.name.startswith(prefixo)),
            sum(1 for i in bpy.data.images if i.name.startswith(prefixo)))


mod_caixa.animar_tampa(objs, 1, 40, abrir=True, lado=1.0)      # 'lado' da API antiga: aceito
mod_caixa.animar_espuma(objs, 18, 90)
antes = _conta("caixa")
objs = mod_caixa.construir_caixa(cena, raiz, PARAMS)
mod_caixa.animar_tampa(objs, 1, 40, abrir=True, lado=1.0)
mod_caixa.animar_espuma(objs, 18, 90)
depois = _conta("caixa")
print("[medida] materiais/actions/imagens 'caixa*' apos 1a/2a construcao: %s / %s; materiais: %s" % (
    antes, depois, sorted(m.name for m in bpy.data.materials if m.name.startswith("caixa"))))
assert depois == antes, "materiais, actions ou imagens vazaram"
assert depois[0] == 3, "esperava caixa.papelao, caixa.etiqueta e caixa.espuma"
for chave in ("cor", "normal", "rugosidade"):
    nome = objs["texturas"][chave]
    img = bpy.data.images.get(nome)
    assert img is not None and img.packed_file is not None, "%s nao empacotada" % chave
    print("[medida] textura %s = %s: %dx%d, %.1f MB, empacotada, colorspace %s" % (
        chave, nome, img.size[0], img.size[1], os.path.getsize(mod_caixa._caminho_asset(nome)) / 1e6, img.colorspace_settings.name))
if objs["com_logo"]:
    assert objs["imagem_logo"].packed_file is not None or objs["logo_provisoria"]

# --- etiqueta: malha da Meshy, filha do corpo, na face +X ---
et = objs["etiqueta"]
pts = [et.matrix_world @ v.co for v in et.data.vertices]
bb = [min(p.x for p in pts), max(p.x for p in pts), min(p.y for p in pts), max(p.y for p in pts), min(p.z for p in pts), max(p.z for p in pts)]
print("[medida] etiqueta: %d verts, %d tris, pai=%s, bbox x %.3f..%.3f y %.3f..%.3f z %.3f..%.3f" % (
    len(et.data.vertices), len(et.data.polygons), et.parent.name if et.parent else None, *bb))
assert et.parent is objs["corpo"]
assert bb[0] > objs["exterior_corpo"][0] / 2 - 0.002, "etiqueta nao esta na face +X"
assert len(et.data.uv_layers) == 1

# --- abas: origem na dobradica, angulos, e nao atravessam a parede abertas ---
cena.frame_set(40)
bpy.context.view_layer.update()
hx, hy = objs["exterior_corpo"][0] / 2, objs["exterior_corpo"][1] / 2
for aba in objs["abas"]:
    ang = math.degrees(max(abs(a) for a in aba.rotation_euler))
    M = aba.matrix_world
    ps = [M @ v.co for v in aba.data.vertices]
    if int(aba["caixa_eixo"]) == 0:
        fora = min(abs(p.y) for p in ps) - hy
    else:
        fora = min(abs(p.x) for p in ps) - hx
    zt = max(p.z for p in ps)
    print("[medida] %s aberta: %.1f graus (pedido %.0f), pivo=%s, folga a parede %+.1f mm, ponta em z=%.3f" % (
        aba.name, ang, aba["caixa_angulo"], tuple(round(v, 3) for v in aba.location), 1000 * fora, zt))
    assert abs(ang - aba["caixa_angulo"]) < 0.5, "aba nao chegou no angulo"
    assert fora > 0.0, "%s atravessa a parede aberta" % aba.name
print("[medida] funil das abas (alcance x, y, topo z): %s" % (tuple(round(v, 3) for v in objs["funil"]),))
# no meio da abertura, a pequena so comeca depois de a grande passar de 85 graus
cena.frame_set(19)
bpy.context.view_layer.update()
g = [math.degrees(max(abs(a) for a in ab.rotation_euler)) for ab in objs["abas"] if int(ab["caixa_eixo"]) == 0]
pq = [math.degrees(max(abs(a) for a in ab.rotation_euler)) for ab in objs["abas"] if int(ab["caixa_eixo"]) == 1]
print("[medida] quadro 19: grandes a %.0f/%.0f graus, pequenas a %.0f/%.0f" % (g[0], g[1], pq[0], pq[1]))
assert max(pq) < 1.0 or min(g) > 85.0, "pequena abriu antes de a grande sair do caminho"

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
    assert zmax <= e + iz + 1e-4, "%s atravessa a base das abas (%.4f)" % (esp.name, zmax)
    assert max(xs) <= ix / 2 + 1e-4 and max(ys) <= iy / 2 + 1e-4, "%s atravessa a parede" % esp.name
    if esp.location.z > e + uz:
        pior_topo = min(pior_topo, zmin - (e + uz))
    else:
        penetracao = max(min(ux / 2 - abs(p.x), uy / 2 - abs(p.y)) for p in pts)
        pior_lado = min(pior_lado, -penetracao)
print("[medida] flocos: %d; maior eixo %.1f-%.1f cm (media %.1f); menor folga base->topo do U1 = %+.1f mm; "
      "menor folga lateral ao U1 = %+.1f mm" % (len(maiores), 100 * min(maiores), 100 * max(maiores),
                                                100 * sum(maiores) / len(maiores), 1000 * pior_topo, 1000 * pior_lado))
assert pior_topo >= 0.0025, "floco afunda no topo do U1"
assert pior_lado >= 0.0, "floco dentro do U1"

# --- trajetoria sem chao: dentro da pegada ate a boca; fora do funil das
# abas quando abaixo das pontas delas; termina fora do quadro com escala 0 ---
fx, fy, fz = objs["funil"]
z_boca = objs["exterior_corpo"][2]
hx, hy = objs["exterior_corpo"][0] / 2, objs["exterior_corpo"][1] / 2
t_ = mod_caixa.PARAMS_PADRAO["parede"]
furos = abas_batidas = 0
z_fim_max, esc_fim_max = -9.0, 0.0
for i, esp in enumerate(objs["espumas"]):
    raio = float(esp["caixa_raio"])
    rx, ry, rz = [float(v) for v in esp["caixa_extensoes"]]
    _, _, pontos = mod_caixa._trajetoria_espuma(esp, i, 7, objs)
    saiu = False
    rot0 = Vector(esp["caixa_rot_repouso"])
    for _, loc, rot, esc in pontos:
        if not saiu and loc.z < z_boca + raio:
            if abs(loc.x) > ix / 2 - raio * 0.3 or abs(loc.y) > iy / 2 - raio * 0.3:
                furos += 1
                break
            continue
        saiu = True
        # abaixo das pontas das abas, o floco nao pode estar na COROA entre a
        # parede e o alcance das abas (o miolo e livre: o funil e aberto).
        # Enquanto nao gira, valem as extensoes de repouso; depois, o raio.
        girou = (Vector(rot) - rot0).length > 1e-6
        ex_, ey_ = (raio, raio) if girou else (rx, ry)
        if loc.z - raio < fz:
            em_x = abs(loc.x) + ex_ > hx + t_ and abs(loc.x) - ex_ < fx
            em_y = abs(loc.y) + ey_ > hy + t_ and abs(loc.y) - ey_ < fy
            if (em_x and abs(loc.y) - ey_ < fy) or (em_y and abs(loc.x) - ex_ < fx):
                abas_batidas += 1
                break
    z_fim_max = max(z_fim_max, pontos[-1][1].z)
    esc_fim_max = max(esc_fim_max, pontos[-1][3])
print("[medida] flocos que cruzam a parede antes da boca: %d; que batem no funil das abas: %d; "
      "fim mais alto z=%.2f (quadro acaba em ~-0,65); escala final maxima %.2f" % (furos, abas_batidas, z_fim_max, esc_fim_max))
assert furos == 0 and abas_batidas == 0
assert z_fim_max < -1.0 and esc_fim_max == 0.0

# --- quadros ---
def render(rotulo):
    cena.render.filepath = os.path.join(SAIDA, "previa_caixa_%s.png" % rotulo)
    bpy.ops.render.render(write_still=True)
    print("[teste] gravado", cena.render.filepath)
    return cena.render.filepath


if "ini" in QUAIS:
    cena.frame_set(1)
    render("ini")
if "meio" in QUAIS:
    # abas no meio (grandes ~90, pequenas comecando) e espuma subindo
    enquadrar((1.35, -1.55, 1.25), (0.0, 0.0, 0.70))
    cena.frame_set(34)
    render("meio")
if "fim" in QUAIS:
    enquadrar((1.35, -1.55, 1.25), (0.0, 0.0, 0.62))
    cena.frame_set(90)
    render("fim")

ex, ey, _ = objs["exterior_tampa"]
if "topo_close" in QUAIS:
    cena.frame_set(1)
    enquadrar((0.35, -0.75, objs["topo_tampa_z"] + 0.55), (0.0, -0.02, objs["topo_tampa_z"]), 50.0)
    render("topo_close")
if "etiqueta" in QUAIS:
    cena.frame_set(1)
    c = Vector(((bb[0] + bb[1]) / 2, (bb[2] + bb[3]) / 2, (bb[4] + bb[5]) / 2))
    enquadrar(c + Vector((0.62, -0.22, 0.10)), c, 85.0)
    render("etiqueta")

# Topo ortografico: mede a largura da engrenagem contra a caixa e a cor da
# tinta no pixel (sobre papelao, nao sobre papel branco).
if "topo" in QUAIS:
    cena.frame_set(1)
    cam.constraints.remove(tr)
    cam.location = (0.0, 0.0, 3.0)
    cam.rotation_euler = (0.0, 0.0, 0.0)
    cam_dados.type = "ORTHO"
    cam_dados.sensor_fit = "VERTICAL"
    cam_dados.ortho_scale = 1.6
    caminho = render("topo")

    import numpy as np
    img = bpy.data.images.load(caminho)
    w, h = img.size
    px = np.empty(w * h * 4, dtype=np.float32)
    img.pixels.foreach_get(px)
    rgb = px.reshape(h, w, 4)[::-1, :, :3]
    lum = rgb.mean(axis=2)
    cx, cy = w // 2, h // 2
    px_por_m = h / cam_dados.ortho_scale
    caixa_px = int(round(ex * px_por_m))
    linha = lum[cy]
    fundo_lum = np.median(linha[:20])
    claros = np.where(linha > fundo_lum * 2.5)[0]
    print("[medida] topo: caixa pela geometria %d px, pela borda do papelao %d px" % (caixa_px, claros[-1] - claros[0] + 1))
    m = int(caixa_px * 0.4)
    sub = rgb[cy - m:cy + m, cx - m:cx + m]
    kraft = np.median(sub.reshape(-1, 3), axis=0)
    k_lum, k_rb = kraft.mean(), kraft[0] - kraft[2]
    # tinta cinza: bem mais escura que o papelao; tinta laranja: mais R-B
    tinta = (sub.mean(axis=2) < k_lum * 0.55) | ((sub[..., 0] - sub[..., 2]) > k_rb + 0.15)
    if not objs["com_logo"]:
        # Sem logo o centro do topo e so papelao e fita: nenhum pixel de tinta.
        fita = sub.mean(axis=2) > k_lum * 1.08
        print("[medida] topo sem logo: pixels de tinta no miolo (80%% do topo): %d de %d; pixels mais claros que o "
              "papelao (fita): %.1f%%; papelao mediano (%.0f,%.0f,%.0f) sRGB" % (
                  int(tinta.sum()), tinta.size, 100.0 * fita.mean(), *tuple(np.where(kraft <= 0.0031308, kraft * 12.92, 1.055 * kraft ** (1 / 2.4) - 0.055) * 255)))
        assert tinta.sum() < 0.001 * tinta.size, "ha tinta no topo sem logo"
        QUAIS = []
    cols = np.where(tinta.any(axis=0))[0] if objs["com_logo"] else np.array([0, 1])
    rows = np.where(tinta.any(axis=1))[0] if objs["com_logo"] else np.array([0, 1])
    larg = cols[-1] - cols[0] + 1
if "topo" in QUAIS and objs["com_logo"]:
    centro_x = (cols[0] + cols[-1]) / 2.0 - m
    centro_y = (rows[0] + rows[-1]) / 2.0 - m
    print("[medida] topo: caixa = %d px (%.3f m -> %.0f px/m); engrenagem = %d px = %.1f%% da largura (pedido %.0f%%); "
          "altura %.1f%%; centro deslocado (%.1f, %.1f) px do centro do topo" % (
              caixa_px, ex, caixa_px / ex, larg, 100.0 * larg / caixa_px, 100 * mod_caixa.PARAMS_PADRAO["largura_logo"],
              100.0 * (rows[-1] - rows[0] + 1) / caixa_px, centro_x, centro_y))

    def srgb(v):
        v = np.clip(np.asarray(v, dtype=np.float64), 0, 1)
        return np.where(v <= 0.0031308, v * 12.92, 1.055 * v ** (1 / 2.4) - 0.055) * 255

    r0, r1 = rows[0], rows[-1]
    faixa = slice(cols[0], cols[-1] + 1)
    cinza_l = slice(r0 + int((r1 - r0) * 0.12), r0 + int((r1 - r0) * 0.38))
    lar_l = slice(r0 + int((r1 - r0) * 0.62), r0 + int((r1 - r0) * 0.88))
    cinza = srgb(sub[cinza_l, faixa][tinta[cinza_l, faixa]].mean(axis=0))
    lar = srgb(sub[lar_l, faixa][tinta[lar_l, faixa]].mean(axis=0))
    print("[medida] topo: papelao (%.0f,%.0f,%.0f) sRGB; cinza da engrenagem (%.0f,%.0f,%.0f) [fonte 56,58,62; meta <= 80]; "
          "laranja (%.0f,%.0f,%.0f) R-B=%.0f [fonte 201,101,32; meta >= 90]" % (
              tuple(srgb(kraft)) + tuple(cinza) + tuple(lar) + (lar[0] - lar[2],)))
    assert 0.40 * caixa_px <= larg <= 0.50 * caixa_px, "logo fora dos 45%% (+-5) da largura: %d px" % larg
    assert abs(centro_x) < 0.02 * caixa_px and abs(centro_y) < 0.02 * caixa_px, "logo fora do centro"
