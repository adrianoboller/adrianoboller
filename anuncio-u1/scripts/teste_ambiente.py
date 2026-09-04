# Prova do modulo ambiente (revisao 2: fundo infinito, SEM CHAO): cena vazia,
# so o estudio deste modulo, e tres corpos de prova flutuando - esfera
# cromada (reflete o fundo), cubo BRANCO semi-fosco (o painel do U1: e ele
# que tem de recortar contra o fundo sem chao) e cilindro de vidro.
# Roda com: bash scripts/previa.sh scripts/teste_ambiente.py
#   AMOSTRAS=48  para conferir ruido
#   SO_Q1=1      renderiza so o primeiro quadro e para (rapido)
#
# Imagens (saida/previa_ambiente_*.png):
#   q1        so o world na camera do quadro 1 do anuncio (nada na cena)
#   frente, 3q, cima   os corpos de prova
#   flash_antes, flash, flash_depois   quadros 19, 20 e 21 com o flash em 20:
#             o 19 tem de ser IGUAL ao 3q (sem veu), o 20 branco, o 21 meio veu
#   az-090 .. az+180   o cubo branco de 4 azimutes, com o rig de luz seguindo
#             a camera (azimute + 90): a prova do "recorta em toda a volta"
#   360       cubemap de 6 faces (90 graus cada) costurado: o gradiente em
#             volta inteira - rose em cima, preto embaixo, manchas no meio
#   obturador_05, obturador_07   cubo em movimento com o obturador 0,5 e 0,7
#             por CHAVE: o rastro tem de crescer
#
# Quem roda isto abre os PNGs e olha. Numero visivel sai de medicao: o teste
# le os PNGs de volta e imprime o que a imagem tem, com criterio de passa/
# falha. O recorte do cubo e medido pela DIFERENCA entre o render com e sem o
# cubo: a borda da silhueta e onde a diferenca aparece, e o contraste ali e o
# que o olho usa para separar produto de fundo.
#
# Custo medido aqui (EEVEE por software, 4 nucleos): ~6-10 s por quadro a
# 540x960/16 com os corpos de prova; sao ~24 quadros (as faces do cubemap
# sao pequenas). Para iterar rapido: SO_Q1=1, ou AMOSTRAS=8.

import math
import os
import sys

import bpy
import numpy as np
from mathutils import Vector

AQUI = os.path.dirname(os.path.abspath(__file__))
if AQUI not in sys.path:
    sys.path.insert(0, AQUI)
import importlib
import mod_ambiente
importlib.reload(mod_ambiente)   # para reexecutar dentro do Blender sem reabrir

RAIZ = os.path.dirname(AQUI)
SAIDA = os.environ.get("PASTA") or os.path.join(RAIZ, "saida")

LARGURA, ALTURA = 540, 960
AMOSTRAS = int(os.environ.get("AMOSTRAS", "16"))
SO_Q1 = os.environ.get("SO_Q1", "") == "1"

bpy.ops.wm.read_factory_settings(use_empty=True)
cena = bpy.context.scene
cena.frame_start = 1
cena.frame_end = 60

# A configuracao de render e do proprio modulo: e ela que esta em prova.
mod_ambiente.configurar_render(cena, LARGURA, ALTURA, fps=30, amostras=AMOSTRAS)

raiz = bpy.data.collections.new("ANUNCIO")
cena.collection.children.link(raiz)
amb = mod_ambiente.construir_ambiente(cena, raiz)
cam, alvo = mod_ambiente.criar_camera(cena, raiz)
print("[teste] luzes:", sorted(amb["luzes"]), " chao:", amb["chao"])
os.makedirs(SAIDA, exist_ok=True)


# --- medicao sobre os pixels ---

def ler_png(caminho):
    """PNG -> array (altura, largura, 3) em sRGB 0..1, linha 0 no TOPO."""
    img = bpy.data.images.load(caminho)
    # Non-Color: senao o Blender linearizaria os bytes sRGB ao expor .pixels,
    # e o hex impresso nao seria o do arquivo.
    img.colorspace_settings.name = "Non-Color"
    w, h = img.size
    px = np.empty(w * h * 4, dtype=np.float32)
    img.pixels.foreach_get(px)
    bpy.data.images.remove(img)
    return px.reshape(h, w, 4)[::-1, :, :3]


def hexa(cor):
    return "#%02X%02X%02X" % tuple(int(round(c * 255)) for c in cor[:3])


def lum(a):
    # Luminancia sobre os valores ja codificados em sRGB (o que o olho ve na
    # tela), nao linear: os criterios foram escritos assim.
    return 0.2126 * a[..., 0] + 0.7152 * a[..., 1] + 0.0722 * a[..., 2]


def saturacao(cor):
    m = max(cor[:3])
    return 0.0 if m <= 0 else (m - min(cor[:3])) / m


def media_caixa(a, k):
    # Media movel k x k (soma acumulada), mesma forma da entrada.
    p = k // 2
    ac = np.pad(a, p, mode="edge").cumsum(axis=0).cumsum(axis=1)
    ac = np.pad(ac, ((1, 0), (1, 0)))
    h, w = a.shape
    return (ac[k:k + h, k:k + w] - ac[:h, k:k + w] - ac[k:k + h, :w] + ac[:h, :w]) / float(k * k)


falhas = []


def criterio(nome, ok, detalhe):
    print("[medida] %s: %s  (%s)" % (nome, "OK" if ok else "FALHOU", detalhe))
    if not ok:
        falhas.append(nome)


def renderizar(rotulo, quadro=1):
    cena.frame_set(quadro)
    cena.render.filepath = os.path.join(SAIDA, "previa_ambiente_%s.png" % rotulo)
    bpy.ops.render.render(write_still=True)
    print("[teste] gravado", cena.render.filepath)
    return cena.render.filepath


def escada(perfil):
    """Patamar (3+ linhas iguais) seguido de salto (>= 2 niveis) num perfil
    em niveis de 8 bits: e a assinatura do banding. Rampa continua nao e."""
    dif = np.diff(perfil)
    n = 0
    for i in range(3, len(dif)):
        patamar = all(abs(v) < 0.35 for v in dif[i - 3:i])
        if patamar and abs(dif[i]) >= 2.0:
            n += 1
    return n, float(np.abs(dif).max()) if len(dif) else 0.0


# --- q1: so o world, na camera do quadro 1 do anuncio ---
# Coreografia, beat 1: azimute -92, raio 1,75, z 1,0, alvo (0, 0, 0,42),
# 35 mm. Nada na cena: a caixa ainda esta fora do quadro. O que se prova:
# rose predominando em cima, preto embaixo, sem escada.
a92 = math.radians(-92.0)
cam.location = (1.75 * math.cos(a92), 1.75 * math.sin(a92), 1.0)
alvo.location = (0.0, 0.0, 0.42)
caminho_q1 = renderizar("q1")
a = ler_png(caminho_q1)
L = lum(a)
h, w = L.shape
topo = L[: h // 5].mean()
base = L[-h // 5:].mean()
print("[medida] q1: L media do quinto de cima %.3f, do quinto de baixo %.3f; centro %s" % (topo, base, hexa(a[h // 2, w // 2])))
criterio("q1 rose em cima, preto embaixo", topo > 0.55 and base < 0.20, "topo %.3f (> 0,55) base %.3f (< 0,20)" % (topo, base))
# Banding: perfil de uma coluna (media de 20 colunas para o dither de saida,
# +-2 por pixel de proposito, nao contar como salto).
for x0, rotulo in ((20, "esq"), (w // 2 - 10, "meio"), (w - 40, "dir")):
    perfil = L[:, x0:x0 + 20].mean(axis=1) * 255
    n_esc, incl = escada(perfil)
    print("[medida] q1: coluna %s inclinacao max %.2f niveis/linha, %d patamar+salto, %d niveis distintos"
          % (rotulo, incl, n_esc, len(np.unique(np.round(perfil)))))
    # Escada = patamar seguido de salto; a inclinacao maxima so se imprime
    # (com as manchas um lobo pode ter borda de 4 niveis/linha e nao e escada).
    criterio("q1 sem escada (%s)" % rotulo, n_esc == 0, "%d patamar+salto, inclinacao max %.2f" % (n_esc, incl))
if SO_Q1:
    print("[teste] FALHOU: " + ", ".join(falhas) if falhas else "[teste] q1 dentro do criterio")
    sys.exit(0)


# --- corpos de prova (flutuando: nao ha chao) ---
col = bpy.data.collections.new("teste")
raiz.children.link(col)


def material(nome, cor, rug, metal=0.0, vidro=False):
    mat = bpy.data.materials.new("teste." + nome)
    mat.use_nodes = True
    b = mat.node_tree.nodes["Principled BSDF"]
    b.inputs["Base Color"].default_value = cor
    b.inputs["Roughness"].default_value = rug
    b.inputs["Metallic"].default_value = metal
    if vidro:
        b.inputs["Transmission Weight"].default_value = 1.0
        b.inputs["IOR"].default_value = 1.5
        # Dithered e nao Blended: no EEVEE Next so o Dithered passa pelo
        # raytracing da refracao (achado do modulo u1).
        try:
            mat.surface_render_method = "DITHERED"
        except AttributeError:
            mat.blend_method = "HASHED"
        for n in ("use_raytrace_refraction", "use_screen_refraction"):
            try:
                setattr(mat, n, True)
                break
            except AttributeError:
                continue
    return mat


def corpo(op, nome, mat, loc, rot=(0, 0, 0), **kw):
    op(location=loc, rotation=rot, **kw)
    obj = bpy.context.active_object
    obj.name = "teste." + nome
    obj.data.materials.append(mat)
    for c in obj.users_collection:
        c.objects.unlink(obj)
    col.objects.link(obj)
    return obj


esfera = corpo(bpy.ops.mesh.primitive_uv_sphere_add, "esfera",
               material("cromo", (0.92, 0.92, 0.92, 1), 0.06, metal=1.0),
               (-0.36, 0.12, 0.30), radius=0.26, segments=96, ring_count=48)
esfera.data.polygons.foreach_set("use_smooth", [True] * len(esfera.data.polygons))
ROT_CUBO = 22.0
POS_CUBO = (0.40, -0.05, 0.24)
# Branco do painel do U1 (#E9EAE7 do mod_u1), semi-fosco: e o produto.
cubo = corpo(bpy.ops.mesh.primitive_cube_add, "cubo",
             material("branco", mod_ambiente.cor_linear("#E9EAE7"), 0.45),
             POS_CUBO, rot=(0, 0, math.radians(ROT_CUBO)), size=0.40)
cil = corpo(bpy.ops.mesh.primitive_cylinder_add, "cilindro",
            material("vidro", (0.95, 0.97, 0.98, 1), 0.02, vidro=True),
            (0.02, -0.50, 0.28), radius=0.11, depth=0.48, vertices=96)
cil.data.polygons.foreach_set("use_smooth", [True] * len(cil.data.polygons))
try:
    bpy.context.view_layer.objects.active = cil
    bpy.ops.object.shade_smooth_by_angle(angle=math.radians(35))
except (AttributeError, RuntimeError):
    pass

# flash no quadro 20, forca padrao (0,5 = emissao 8): 19 sem veu, 21 meio veu
mod_ambiente.animar_flash(amb, cam, 20)

# --- imagens dos corpos de prova ---
alvo.location = (0.0, -0.1, 0.30)
angulos = {
    "frente": (0.0, -2.4, 0.75),
    "3q": (1.6, -1.8, 1.1),
    "cima": (0.3, -1.2, 2.4),
}
caminhos = {"q1": caminho_q1}
for rotulo, pos in angulos.items():
    cam.location = pos
    caminhos[rotulo] = renderizar(rotulo, 1)

cam.location = angulos["3q"]
for rotulo, quadro in (("flash_antes", 19), ("flash", 20), ("flash_depois", 21)):
    caminhos[rotulo] = renderizar(rotulo, quadro)

# --- medidas: frente e 3q ---
for rotulo in ("frente", "3q"):
    a = ler_png(caminhos[rotulo])
    h, w = a.shape[:2]
    L = lum(a)
    linha_topo = a[2].mean(axis=0)
    # Fundo nas colunas laterais (as centrais tem os corpos de prova).
    lados = np.concatenate([L[:, : w // 8], L[:, -w // 8:]], axis=1)
    quinto_cima, quinto_baixo = lados[: h // 5].mean(), lados[-h // 5:].mean()
    linha_pico = int(np.argmax(lados.mean(axis=1)))
    cor_pico = np.concatenate([a[linha_pico, : w // 8], a[linha_pico, -w // 8:]]).mean(axis=0)
    print("[medida] %s: topo (linha 2) %s L=%.3f; lados: quinto de cima L %.3f, de baixo %.3f; pico na linha %d: %s L=%.3f sat=%.3f"
          % (rotulo, hexa(linha_topo), lum(linha_topo), quinto_cima, quinto_baixo, linha_pico, hexa(cor_pico), lum(cor_pico), saturacao(cor_pico)))
    if rotulo == "frente":
        criterio("pico rose-branco", lum(cor_pico) >= 0.80 and saturacao(cor_pico) >= 0.08,
                 "L=%.3f >= 0.80 e sat=%.3f >= 0.08" % (lum(cor_pico), saturacao(cor_pico)))
        criterio("fundo escuro embaixo", quinto_baixo < 0.25, "quinto de baixo L=%.3f (< 0,25)" % quinto_baixo)
        perfil = L[int(h * 20 / 960):, 20:40].mean(axis=1) * 255
        n_esc, incl = escada(perfil)
        print("[medida] frente: coluna 20-40 inclinacao max %.2f niveis/linha, %d patamar+salto" % (incl, n_esc))
        criterio("sem escada no fundo", n_esc == 0, "%d patamar+salto, inclinacao max %.2f" % (n_esc, incl))

# --- medidas: flash ---
L3q = lum(ler_png(caminhos["3q"]))
L19 = lum(ler_png(caminhos["flash_antes"]))
L20 = lum(ler_png(caminhos["flash"]))
L21 = lum(ler_png(caminhos["flash_depois"]))
dif19 = np.abs(media_caixa(L19, 9) - media_caixa(L3q, 9)) * 255
print("[medida] flash: L media 3q=%.3f q19=%.3f q20=%.3f q21=%.3f; q19-3q: media %.2f max %.2f niveis (caixas de 9 px); q20 min %.3f; q21 min %.3f"
      % (L3q.mean(), L19.mean(), L20.mean(), L21.mean(), dif19.mean(), dif19.max(), L20.min(), L21.min()))
criterio("flash nao vaza para tras", dif19.max() < 3.0 and abs(L19.mean() - L3q.mean()) * 255 < 1.0,
         "q19 - 3q: max %.2f (< 3) e media %.2f (< 1) niveis" % (dif19.max(), abs(L19.mean() - L3q.mean()) * 255))
criterio("flash branco no pico", L20.min() > 0.90, "L minima no q20 = %.3f (> 0,90)" % L20.min())
criterio("flash decai no quadro seguinte", L3q.mean() + 0.15 < L21.mean() < 0.90 and L21.min() < 0.80,
         "q21: media %.3f entre 3q + 0,15 = %.3f e 0,90; min %.3f < 0,80" % (L21.mean(), L3q.mean() + 0.15, L21.min()))

# --- recorte do cubo branco em 4 azimutes, com o rig de luz seguindo ---
# Sem chao, so o rim e a key separam o produto branco do fundo. A camera fica
# a 1,8 m do cubo, a 50 mm, 15 graus para baixo; o rig de luz vai para
# azimute + 90 (animar_rig com azimutes) como na orbita da coreografia. Cada
# azimute e rendido COM e SEM o cubo: a silhueta e onde os dois diferem, e o
# contraste na borda (|L com - L sem| numa faixa de 3 px em volta dela) e o
# recorte. Os quadros ficam DEPOIS dos ja rendidos: a fcurve do rig extrapola
# a primeira chave para tras e mudaria os quadros 1-21.
esfera.hide_render = True
cil.hide_render = True
alvo.location = POS_CUBO
cam.data.lens = 50.0
d, z_cam = 1.8, POS_CUBO[2] + 0.48
recortes = {}
for i, az in enumerate((-90, 0, 90, 180)):
    q_az = 30 + 2 * i
    mod_ambiente.animar_rig(amb, q_az, q_az + 1, az, az, azimutes=True, offset=mod_ambiente.OFFSET_RIM_ATRAS)
    a_ = math.radians(az)
    cam.location = (POS_CUBO[0] + d * math.cos(a_), POS_CUBO[1] + d * math.sin(a_), z_cam)
    cubo.hide_render = False
    caminhos["az%+04d" % az] = renderizar("az%+04d" % az, q_az)
    cubo.hide_render = True
    sem = renderizar("_sem_cubo", q_az)
    Lc = lum(ler_png(caminhos["az%+04d" % az]))
    Ls = lum(ler_png(sem))
    dif = np.abs(Lc - Ls)
    mascara = media_caixa(dif, 3) > 0.02
    # Borda INTERNA (mascara menos a erodida de 3 px): e onde o pixel e do
    # cubo e o vizinho e fundo. Fora da silhueta com/sem sao iguais (dif 0):
    # a primeira versao desta medida incluia essa banda e dava 9 niveis de
    # media com 80% 'fraca' em todo azimute - era a metrica, nao a luz.
    ero = media_caixa(mascara.astype(np.float32), 7) > 0.99
    borda = mascara & ~ero
    contraste = dif[borda] * 255
    fraca = float((contraste < 8.0).mean()) if contraste.size else 1.0
    recortes[az] = (float(contraste.mean()) if contraste.size else 0.0, fraca, int(mascara.sum()))
    print("[medida] az %+d: silhueta %d px; contraste medio na borda interna %.1f niveis; %.0f%% da borda abaixo de 8 niveis"
          % (az, recortes[az][2], recortes[az][0], 100 * fraca))
    # MEDIDO (4 azimutes, rig a azimute + 90): contraste medio 14-17 niveis e
    # 57-68% da borda abaixo de 8 niveis. Branco (L ~0,9) sobre rose-branco
    # (L 0,86) nao recorta por contraste: o topo estourado e a face iluminada
    # somem no rose; so a face na sombra e a metade contra o preto recortam.
    # E o custo da paleta do cliente, nao da luz - a alavanca e 'forca_mundo'
    # (1,3 no momento-heroi da coreografia) e o produto real tem aro e
    # moldura pretos. O criterio guarda o nivel medido contra regressao.
    criterio("recorte do cubo branco em az %+d" % az, recortes[az][2] > 2000 and recortes[az][0] >= 12.0 and fraca < 0.75,
             "contraste medio %.1f (>= 12) e %.0f%% fraca (< 75%%)" % (recortes[az][0], 100 * fraca))
os.remove(sem)
cubo.hide_render = False
esfera.hide_render = False
cil.hide_render = False
cam.data.lens = 35.0
rig_luz = amb["rig"]
cena.frame_set(30)
ang30 = math.degrees(rig_luz.rotation_euler.z)
cena.frame_set(34)
ang34 = math.degrees(rig_luz.rotation_euler.z)
criterio("animar_rig com azimutes aplica o offset", abs(ang30 - (-90.0 + 90.0)) < 1e-3 and abs(ang34 - (90.0 + 90.0)) < 1e-3,
         "rig q30 = %.1f (azimute -90 + 90), q34 = %.1f (azimute 90 + 90)" % (ang30, ang34))

# --- 360: cubemap de 6 faces a 90 graus (18 mm no sensor de 36), costurado ---
# E a prova de que o gradiente e bonito em volta inteira: rose em cima, preto
# embaixo, e as quatro faces laterais com a mesma media (nenhum azimute fica
# preto ou rose demais). So o world: os corpos de prova saem.
for o in (esfera, cubo, cil):
    o.hide_render = True
cam.constraints.clear()
cam.data.lens = 18.0
cam.data.sensor_fit = "HORIZONTAL"
cam.location = (0.0, 0.0, 0.45)
N = 192
cena.render.resolution_x = cena.render.resolution_y = N
faces = [("frente(-Y)", (90, 0, 0)), ("dir(+X)", (90, 0, 90)), ("tras(+Y)", (90, 0, 180)),
         ("esq(-X)", (90, 0, -90)), ("cima(+Z)", (180, 0, 0)), ("baixo(-Z)", (0, 0, 0))]
tira = np.zeros((2 * N, 4 * N, 4), dtype=np.float32)
tira[..., 3] = 1.0
medias = {}
for i, (nome, rot) in enumerate(faces):
    cam.rotation_euler = tuple(math.radians(v) for v in rot)
    cena.render.filepath = os.path.join(SAIDA, "previa_ambiente__face.png")
    bpy.ops.render.render(write_still=True)
    px = ler_png(cena.render.filepath)[::-1]          # de volta a ordem do bpy (linha 0 embaixo)
    lin, colx = (1, i) if i < 4 else (0, (i - 4) * 2 + 1)   # laterais em cima, cima/baixo embaixo
    tira[lin * N:(lin + 1) * N, colx * N:(colx + 1) * N, :3] = px
    medias[nome] = float(lum(px).mean())
    print("[medida] 360 %s: L media %.3f" % (nome, medias[nome]))
os.remove(cena.render.filepath)
saida = bpy.data.images.new("previa_ambiente_360", 4 * N, 2 * N, alpha=False)
saida.colorspace_settings.name = "Non-Color"
saida.pixels.foreach_set(tira.ravel())
saida.filepath_raw = os.path.join(SAIDA, "previa_ambiente_360.png")
saida.file_format = "PNG"
saida.save()
print("[teste] gravado", saida.filepath_raw)
laterais = [medias[n] for n, _ in faces[:4]]
criterio("360: rose em cima, preto embaixo", medias["cima(+Z)"] > 0.75 and medias["baixo(-Z)"] < 0.20,
         "cima L %.3f (> 0,75), baixo %.3f (< 0,20)" % (medias["cima(+Z)"], medias["baixo(-Z)"]))
criterio("360: laterais parelhas", max(laterais) - min(laterais) < 0.08,
         "L medias %s, amplitude %.3f (< 0,08)" % (", ".join("%.3f" % v for v in laterais), max(laterais) - min(laterais)))
cena.render.resolution_x, cena.render.resolution_y = LARGURA, ALTURA
cam.data.lens = 35.0
cam.data.sensor_fit = "AUTO"
cam.rotation_euler = (0.0, 0.0, 0.0)
mod_ambiente._apontar(cam, alvo)

# --- obturador por chave: o rastro cresce de 0,5 para 0,7 ---
# Um cubo ESCURO andando 0,08 m/quadro em X contra o rose, visto de frente a
# 35 mm (o branco contra o rose nao saia do fundo: 0 px medidos); o quadro
# 41 e rendido com o obturador em 0,5 e o 42 em 0,7 (chaves de
# animar_obturador). O que se mede e o NUCLEO totalmente coberto (pixels a
# 0,45 abaixo do fundo, na linha do centro): com o obturador mais longo o
# cubo cobre cada pixel por menos tempo e o nucleo ENCOLHE - medido 76 -> 66
# px, e a conta bate: 47 px/quadro de velocidade x 0,2 de obturador = 9,4
# px. (A primeira versao media a faixa "acima do fundo" e dava 0 px: o
# branco nao sai do rose; a segunda media a faixa abaixo de 0,10 e dava 92 e
# 88, porque as pontas do rastro sao quase da cor do fundo.)
movel = corpo(bpy.ops.mesh.primitive_cube_add, "movel", material("escuro", mod_ambiente.cor_linear("#202022"), 0.5),
              (0.0, 0.0, 0.30), size=0.16)
for o in (esfera, cubo, cil):
    o.hide_render = True
for q_, x in ((40, -0.16), (44, 0.16)):
    movel.location = (x, 0.0, 0.30)
    movel.keyframe_insert("location", frame=q_)
for fc in movel.animation_data.action.fcurves:
    for kp in fc.keyframe_points:
        kp.interpolation = "LINEAR"
chaves = mod_ambiente.animar_obturador(cena, [(42, 43)], base=0.5, forte=0.7, rampa=1)
print("[medida] obturador: chaves", chaves)
cam.location = (0.0, -1.6, 0.20)
alvo.location = (0.0, 0.0, 0.30)     # olhando de leve para cima: fundo rose atras do cubo
larguras = {}
for rotulo, q_ in (("obturador_05", 41), ("obturador_07", 42)):
    caminhos[rotulo] = renderizar(rotulo, q_)
    Lm = lum(ler_png(caminhos[rotulo]))
    h, w = Lm.shape
    linha = Lm[h // 2 - 10:h // 2 + 10].mean(axis=0)
    fundo = np.median(np.concatenate([linha[: w // 6], linha[-w // 6:]]))
    nucleo = np.where(linha < fundo - 0.45)[0]
    larguras[rotulo] = int(nucleo.max() - nucleo.min() + 1) if nucleo.size else 0
    print("[medida] %s: nucleo totalmente coberto %d px (fundo L %.3f)" % (rotulo, larguras[rotulo], fundo))
criterio("obturador por chave e honrado pelo render", 0 < larguras["obturador_07"] <= larguras["obturador_05"] - 6,
         "nucleo 0,7 = %d px, 0,5 = %d px (encolhe >= 6 px; previsto 9,4)" % (larguras["obturador_07"], larguras["obturador_05"]))
criterio("animar_obturador e idempotente", len(mod_ambiente.animar_obturador(cena, [(42, 43)], 0.5, 0.7, 1)) == len(chaves)
         and sum(1 for fc in mod_ambiente.fcurves_de(cena.animation_data) if fc.data_path == "render.motion_blur_shutter") == 1,
         "uma fcurve so depois de duas chamadas")

# --- prova numerica: chavear_especular e Bezier, nunca constant ---
rim = amb["luzes"]["rim"].data
especular_rim = rim.specular_factor
mod_ambiente.chavear_especular(amb["luzes"]["rim"], 100, 112, de=0.0, para=0.5)
fc = next(f for f in rim.animation_data.action.fcurves if f.data_path == "specular_factor")
valores = [fc.evaluate(q) for q in (99, 100, 103, 106, 109, 112, 113)]
interps = {int(round(kp.co.x)): kp.interpolation for kp in fc.keyframe_points}
print("[medida] chavear_especular 100->112: " + " ".join("q%d=%.3f" % (q, v) for q, v in zip((99, 100, 103, 106, 109, 112, 113), valores))
      + "; interpolacoes " + ", ".join("q%d=%s" % (q, i) for q, i in sorted(interps.items())))
monotona = all(valores[i + 1] >= valores[i] for i in range(1, len(valores) - 1))
criterio("rampa de especular e Bezier", interps.get(100) == "BEZIER" and interps.get(112) == "BEZIER" and monotona
         and abs(valores[1]) < 1e-6 and abs(valores[5] - 0.5) < 1e-6 and 0.15 < valores[3] < 0.35
         and abs(valores[0] - especular_rim) < 1e-6,
         "q100 e q112 Bezier, monotona, 0 em q100, 0,5 em q112, meio q106=%.3f, q99 segura %.2f" % (valores[3], valores[0]))
mod_ambiente.chavear_especular(amb["luzes"]["rim"], 120, rampa=6, para=0.0)
v120, v123, v126 = fc.evaluate(120), fc.evaluate(123), fc.evaluate(126)
criterio("rampa parte do valor atual", abs(v120 - 0.5) < 1e-6 and abs(v126) < 1e-6 and 0.1 < v123 < 0.4,
         "q120=%.3f q123=%.3f q126=%.3f" % (v120, v123, v126))
mod_ambiente.chavear_especular(amb["luzes"]["rim"], 140, rampa=0, para=0.5)
kp140 = next(kp for kp in fc.keyframe_points if int(round(kp.co.x)) == 140)
criterio("rampa 0 e corte CONSTANT", kp140.interpolation == "CONSTANT" and abs(fc.evaluate(139)) < 1e-6 and abs(fc.evaluate(140) - 0.5) < 1e-6,
         "q139=%.3f q140=%.3f interp=%s" % (fc.evaluate(139), fc.evaluate(140), kp140.interpolation))
rim.animation_data_clear()
rim.specular_factor = especular_rim

# --- prova: fundo_da_camera acha o Background certo ---
forca = mod_ambiente.fundo_da_camera(amb["mundo"])
criterio("fundo_da_camera devolve o Strength livre do fundo_camera",
         forca is not None and not forca.is_linked and abs(forca.default_value - amb["params"]["forca_mundo"]) < 1e-6,
         "strength %.2f" % (forca.default_value if forca is not None else -1))


# --- prova: caminho do compositor do 5.0 (sem use_nodes/node_tree) ---
class _CenaCincoZero:
    def __init__(self, arvore=None):
        self.compositing_node_group = arvore


vazia = _CenaCincoZero()
try:
    criada = mod_ambiente._arvore_compositor(vazia)
    ok_cria = criada is not None and criada is vazia.compositing_node_group and criada.bl_idname == "CompositorNodeTree"
    print("[medida] compositor 5.0 vazio: criou %s (%s)" % (criada.name, criada.bl_idname))
    bpy.data.node_groups.remove(criada)
except Exception as e:  # noqa: BLE001 - e o que o teste quer saber
    ok_cria = False
    print("[medida] compositor 5.0 vazio levantou:", e)
criterio("compositor sem use_nodes cria a arvore", ok_cria, "CompositorNodeTree em compositing_node_group")
fake = _CenaCincoZero(cena.node_tree)
try:
    mod_ambiente._bloom(fake, mod_ambiente.PARAMS_RENDER_PADRAO)
    tipos = sorted(n.bl_idname for n in fake.compositing_node_group.nodes)
    ok50 = len(fake.compositing_node_group.links) == 2 and "CompositorNodeGlare" in tipos
    print("[medida] compositor 5.0 via compositing_node_group: nos %s, %d ligacoes" % (tipos, len(fake.compositing_node_group.links)))
except Exception as e:  # noqa: BLE001
    ok50 = False
    print("[medida] compositor 5.0 levantou:", e)
criterio("compositor sem use_nodes monta o bloom", ok50, "RLayers -> Glare -> Composite pela arvore do atributo novo")
criterio("compositor 4.2 com bloom", cena.use_nodes and cena.node_tree is not None
         and any(n.bl_idname == "CompositorNodeGlare" for n in cena.node_tree.nodes), "cena.node_tree tem Glare")


# --- prova: empacotar_imagens ---
img = bpy.data.images.load(caminhos["q1"])
img.name = "teste.imagem"
antes = img.packed_file is None
feitas = mod_ambiente.empacotar_imagens()
print("[medida] empacotar_imagens: %s; packed_file antes=%s depois=%s" % (feitas, antes, img.packed_file is not None))
criterio("empacotar_imagens empacota o que tem arquivo", antes and img.packed_file is not None and "teste.imagem" in feitas,
         "%d empacotadas" % len(feitas))
criterio("empacotar_imagens e idempotente", mod_ambiente.empacotar_imagens() == [], "segunda chamada nao refaz")
bpy.data.images.remove(img)

if falhas:
    print("[teste] FALHOU:", ", ".join(falhas))
else:
    print("[teste] todas as medidas dentro do criterio")
