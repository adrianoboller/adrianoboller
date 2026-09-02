# Prova do modulo ambiente: cena vazia, so o estudio deste modulo, e tres
# corpos de prova sobre o chao - esfera cromada (reflete o fundo), cubo
# grafite fosco (le a luz de estudio) e cilindro de vidro (refracao). Quatro
# imagens: frente, 3/4, de cima, e o 3/4 com o flash no pico.
# Roda com: bash scripts/previa.sh scripts/teste_ambiente.py
#
# Quem roda isto abre os PNGs e olha: gradiente sem serrilhado, silhueta
# recortada pelo rim, reflexo no chao sutil, vidro transparente, flash branco.

import math
import os
import sys

import bpy
from mathutils import Vector

AQUI = os.path.dirname(os.path.abspath(__file__))
if AQUI not in sys.path:
    sys.path.insert(0, AQUI)
import importlib
import mod_ambiente
importlib.reload(mod_ambiente)   # para reexecutar dentro do Blender sem reabrir

RAIZ = os.path.dirname(AQUI)
SAIDA = os.path.join(RAIZ, "saida")

LARGURA, ALTURA = 540, 960
AMOSTRAS = int(os.environ.get("AMOSTRAS", "16"))   # AMOSTRAS=48 para conferir ruido

bpy.ops.wm.read_factory_settings(use_empty=True)
cena = bpy.context.scene
cena.frame_start = 1
cena.frame_end = 30

# A configuracao de render e do proprio modulo: e ela que esta em prova.
mod_ambiente.configurar_render(cena, LARGURA, ALTURA, fps=30, amostras=AMOSTRAS)

raiz = bpy.data.collections.new("ANUNCIO")
cena.collection.children.link(raiz)
amb = mod_ambiente.construir_ambiente(cena, raiz)
cam, alvo = mod_ambiente.criar_camera(cena, raiz)
print("[teste] luzes:", sorted(amb["luzes"]))


# --- corpos de prova ---
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
               (-0.36, 0.12, 0.26), radius=0.26, segments=96, ring_count=48)
esfera.data.polygons.foreach_set("use_smooth", [True] * len(esfera.data.polygons))
cubo = corpo(bpy.ops.mesh.primitive_cube_add, "cubo",
             material("grafite", mod_ambiente.cor_linear("#1E2024"), 0.55),
             (0.40, -0.05, 0.20), rot=(0, 0, math.radians(22)), size=0.40)
cil = corpo(bpy.ops.mesh.primitive_cylinder_add, "cilindro",
            material("vidro", (0.95, 0.97, 0.98, 1), 0.02, vidro=True),
            (0.02, -0.50, 0.24), radius=0.11, depth=0.48, vertices=96)
cil.data.polygons.foreach_set("use_smooth", [True] * len(cil.data.polygons))
# a tampa do cilindro tambem ficou 'suave' - o auto smooth por angulo corrige
try:
    bpy.context.view_layer.objects.active = cil
    bpy.ops.object.shade_smooth_by_angle(angle=math.radians(35))
except (AttributeError, RuntimeError):
    pass

# flash no quadro 20 (quadros 19 e 21 sem veu)
mod_ambiente.animar_flash(amb, cam, 20, forca=1.0)

# --- quatro imagens ---
alvo.location = (0.0, -0.1, 0.30)
angulos = {
    "frente": (0.0, -3.0, 0.75),
    "3q": (2.0, -2.2, 1.25),
    "cima": (0.4, -1.5, 3.0),
}
caminhos = {}
for rotulo, pos in angulos.items():
    cam.location = pos
    cena.frame_set(1)
    cena.render.filepath = os.path.join(SAIDA, "previa_ambiente_%s.png" % rotulo)
    bpy.ops.render.render(write_still=True)
    caminhos[rotulo] = cena.render.filepath
    print("[teste] gravado", cena.render.filepath)

cam.location = angulos["3q"]
cena.frame_set(20)
cena.render.filepath = os.path.join(SAIDA, "previa_ambiente_flash.png")
bpy.ops.render.render(write_still=True)
caminhos["flash"] = cena.render.filepath
print("[teste] gravado", cena.render.filepath)


# --- medicao sobre os pixels ---
# Numero visivel sai de medicao: o relato anterior dizia "topo escuro, faixa
# rose" e os pixels diziam #C0B8B6 cinza. Aqui o teste le o PNG de volta e
# imprime o que a imagem tem, com criterio de passa/falha.
import numpy as np


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
    # tela), nao linear: os criterios da revisao foram escritos assim.
    return 0.2126 * a[..., 0] + 0.7152 * a[..., 1] + 0.0722 * a[..., 2]


def saturacao(cor):
    m = max(cor[:3])
    return 0.0 if m <= 0 else (m - min(cor[:3])) / m


falhas = []


def criterio(nome, ok, detalhe):
    print("[medida] %s: %s  (%s)" % (nome, "OK" if ok else "FALHOU", detalhe))
    if not ok:
        falhas.append(nome)


for rotulo in ("frente", "3q"):
    a = ler_png(caminhos[rotulo])
    h, w = a.shape[:2]
    L = lum(a)
    topo = a[2].mean(axis=0)
    # Pico do brilho: a linha mais clara na metade superior, nas colunas
    # laterais (as centrais tem os corpos de prova).
    faixa = np.concatenate([L[: h // 2, : w // 6], L[: h // 2, -w // 6:]], axis=1)
    linha_pico = int(np.argmax(faixa.mean(axis=1)))
    cor_pico = np.concatenate([a[linha_pico, : w // 6], a[linha_pico, -w // 6:]]).mean(axis=0)
    print("[medida] %s: topo (linha 2) %s L=%.3f; pico do brilho na linha %d: %s L=%.3f sat=%.3f"
          % (rotulo, hexa(topo), lum(topo), linha_pico, hexa(cor_pico), lum(cor_pico), saturacao(cor_pico)))
    if rotulo == "frente":
        # Na frente (camera a 0,75 m, 8,8 graus para baixo) o topo do quadro
        # esta a 18 graus de elevacao: tem de ser preto. No 3/4 (17,7 graus
        # para baixo) o topo fica a 9 graus, dentro do brilho - so se imprime.
        criterio("topo preto", lum(topo) < 20 / 255.0, "L(topo)=%.3f < %.3f" % (lum(topo), 20 / 255.0))
        criterio("pico rose-branco", lum(cor_pico) >= 0.85 and saturacao(cor_pico) >= 0.10,
                 "L=%.3f >= 0.85 e sat=%.3f >= 0.10" % (lum(cor_pico), saturacao(cor_pico)))
        # Chao puro: da linha 680 para baixo nao ha corpo de prova (o pe do
        # cilindro, o mais proximo, cai na linha ~620).
        chao = L[int(h * 680 / 960):int(h * 950 / 960)]
        fracao = float((chao > 0.5).mean())
        print("[medida] frente: chao (linhas 680-950) L media=%.3f max=%.3f fracao L>0,5=%.3f"
              % (chao.mean(), chao.max(), fracao))
        criterio("chao escuro", fracao < 0.01 and chao.max() < 0.6,
                 "fracao L>0,5 = %.3f (< 0,01) e max = %.3f (< 0,6)" % (fracao, chao.max()))
        # Sem linha do horizonte: na coluna 30, das linhas 180 a 420, a
        # luminancia sobe ate o brilho e desce para o chao, sem minimo local
        # no meio (tolerancia de 2 niveis de 8 bits).
        col = [L[i, 30] * 255 for i in range(int(h * 180 / 960), int(h * 420 / 960) + 1, int(h * 10 / 960))]
        print("[medida] frente: coluna 30, linhas 180-420 de 10 em 10: " + " ".join("%d" % round(v) for v in col))
        pico = int(np.argmax(col))
        sobe = all(col[i + 1] >= col[i] - 2 for i in range(pico))
        desce = all(col[i + 1] <= col[i] + 2 for i in range(pico, len(col) - 1))
        criterio("sem linha no horizonte", sobe and desce, "unimodal: sobe=%s desce=%s" % (sobe, desce))
        # Banding: escada e PATAMAR (3+ linhas iguais) seguido de SALTO (>= 2
        # niveis). Rampa continua com 2 niveis por linha nao e escada - e a
        # inclinacao do gradiente a 960 px (1 por linha a 1920). O perfil e a
        # media de 20 colunas para o dither de saida (+-2 por pixel, de
        # proposito) nao contar como salto.
        ceu = L[int(h * 20 / 960):int(h * 320 / 960), 20:40].mean(axis=1) * 255
        d = np.diff(ceu)
        escada = 0
        for i in range(3, len(d)):
            patamar = all(abs(v) < 0.35 for v in d[i - 3:i])
            if patamar and abs(d[i]) >= 2.0:
                escada += 1
        print("[medida] frente: ceu (linhas 20-320, media de 20 colunas) inclinacao max %.2f niveis/linha, %d niveis distintos"
              % (np.abs(d).max(), len(np.unique(np.round(ceu)))))
        criterio("sem escada no ceu", escada == 0 and np.abs(d).max() <= 3.0,
                 "%d patamar+salto, inclinacao max %.2f (<= 3)" % (escada, np.abs(d).max()))

flash = ler_png(caminhos["flash"])
criterio("flash branco", lum(flash).min() > 0.97, "L minima = %.3f" % lum(flash).min())

if falhas:
    print("[teste] FALHOU:", ", ".join(falhas))
else:
    print("[teste] todas as medidas dentro do criterio")
