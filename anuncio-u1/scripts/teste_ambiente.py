# Prova do modulo ambiente: cena vazia, so o estudio deste modulo, e tres
# corpos de prova sobre o chao - esfera cromada (reflete o fundo), cubo
# grafite fosco (le a luz de estudio) e cilindro de vidro (refracao).
# Roda com: bash scripts/previa.sh scripts/teste_ambiente.py
#   AMOSTRAS=48  para conferir ruido
#   ANTES=1      repoe os parametros da rodada 1 (fusao 6-30, rim 1,0 m/2,2 m)
#                para provar que os criterios do q1 FALHAM com o defeito reposto
#   SO_Q1=1      renderiza so o primeiro quadro e para (rapido)
#
# Imagens (saida/previa_ambiente_*.png):
#   q1        chao vazio na camera do beat 1 - o primeiro quadro do anuncio
#   frente, 3q, cima   os corpos de prova
#   flash_antes, flash, flash_depois   quadros 19, 20 e 21 com o flash em 20:
#             o 19 tem de ser IGUAL ao 3q (sem veu), o 20 branco, o 21 meio veu
#   lateral_atras, lateral   face do cubo com o rig a azimute+90 e +60
#
# Quem roda isto abre os PNGs e olha: gradiente sem serrilhado, silhueta
# recortada pelo rim, reflexo no chao sutil, vidro transparente, primeiro
# quadro sem ponto branco nem faixa mosqueada, flash que nao vaza para tras.
# Numero visivel sai de medicao: o teste le os PNGs de volta e imprime o que
# a imagem tem, com criterio de passa/falha.
#
# Custo medido aqui (EEVEE por software, 4 nucleos): ~40 s por quadro a
# 540x960/16 com os corpos de prova - os 9 quadros levam ~7 min. Para iterar
# rapido: SO_Q1=1, ou AMOSTRAS=8.

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
SAIDA = os.path.join(RAIZ, "saida")

LARGURA, ALTURA = 540, 960
AMOSTRAS = int(os.environ.get("AMOSTRAS", "16"))
ANTES = os.environ.get("ANTES", "") == "1"
SO_Q1 = os.environ.get("SO_Q1", "") == "1"

bpy.ops.wm.read_factory_settings(use_empty=True)
cena = bpy.context.scene
cena.frame_start = 1
cena.frame_end = 60

# A configuracao de render e do proprio modulo: e ela que esta em prova.
mod_ambiente.configurar_render(cena, LARGURA, ALTURA, fps=30, amostras=AMOSTRAS)

raiz = bpy.data.collections.new("ANUNCIO")
cena.collection.children.link(raiz)
params_amb = None
if ANTES:
    # Os parametros com que a revisao viu o ponto branco e a faixa mosqueada.
    params_amb = {"fusao_chao": (6.0, 30.0),
                  "luzes": {"rim": {"pos": (0.6, 2.8, 1.0), "tam": (0.3, 2.2)}}}
amb = mod_ambiente.construir_ambiente(cena, raiz, params_amb)
cam, alvo = mod_ambiente.criar_camera(cena, raiz)
print("[teste] luzes:", sorted(amb["luzes"]))


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
    # tela), nao linear: os criterios da revisao foram escritos assim.
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


def media_h(a, k):
    # Media movel de k px so na horizontal.
    p = k // 2
    ac = np.pad(a, ((0, 0), (p, p)), mode="edge").cumsum(axis=1)
    ac = np.pad(ac, ((0, 0), (1, 0)))
    w = a.shape[1]
    return (ac[:, k:k + w] - ac[:, :w]) / float(k)


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


# --- q1: o primeiro quadro do anuncio, chao vazio na camera do beat 1 ---
# Camera da coreografia no q1: (0, -2.2, 1.0) olhando (0, 0, 0.30), 35 mm, e
# o specular_factor do rim em 0 como ela chaveia nos planos largos. A revisao
# viu aqui um ponto branco (69%, 39%) e uma faixa marrom mosqueada (28-38%).
rim = amb["luzes"]["rim"].data
especular_rim = rim.specular_factor
rim.specular_factor = 0.0
cam.location = (0.0, -2.2, 1.0)
alvo.location = (0.0, 0.0, 0.30)
caminho_q1 = renderizar("q1")
a = ler_png(caminho_q1)
L = lum(a)
h, w = L.shape
# Mosqueado: na faixa 26-40% da altura, media de caixas de 12x6 px menos a
# media corrida de 5 caixas na horizontal (a curvatura do arco radial da
# fusao e lenta e nao conta); desvio por linha, em niveis de 8 bits. Um
# gradiente limpo da ~0; a revisao tinha 0,85 (medido no cabecalho do modulo).
faixa = L[int(h * 0.26):int(h * 0.40), :]
by, bx = 6, 12
fh, fw = (faixa.shape[0] // by) * by, (faixa.shape[1] // bx) * bx
caixas = faixa[:fh, :fw].reshape(fh // by, by, fw // bx, bx).mean(axis=(1, 3))
nucleo = np.ones(5) / 5.0
suave = np.array([np.convolve(linha, nucleo, mode="same") for linha in caixas])
mosqueado = (caixas - suave)[:, 2:-2].std(axis=1).mean() * 255
# Ponto branco: passa-alta so na HORIZONTAL (caixa de 5 px menos a media de
# 61 px da mesma linha) entre 30% e 60% da altura: um ponto de ~15 px salta,
# um gradiente vertical ou o arco da fusao (quase horizontal) nao. Calibrado:
# 17,5 niveis no q0001 da revisao e no codigo antigo (em 69%, 39%), 1,7-2,0
# no corrigido.
suave = media_caixa(L, 5)
alta = suave - media_h(suave, 61)
regiao = alta[int(h * 0.30):int(h * 0.60), :]
iy, ix = np.unravel_index(np.argmax(regiao), regiao.shape)
ponto = float(regiao.max()) * 255
cor_faixa = a[int(h * 0.33), w // 2]
print("[medida] q1: mosqueado %.2f niveis; ponto %.1f niveis em (%.0f%%, %.0f%%); faixa 33%%: %s L=%.3f"
      % (mosqueado, ponto, 100.0 * ix / w, 100.0 * (iy + int(h * 0.30)) / h, hexa(cor_faixa), lum(cor_faixa)))
criterio("q1 sem faixa mosqueada", mosqueado < 0.35, "%.2f niveis (< 0,35)" % mosqueado)
criterio("q1 sem ponto branco", ponto < 5.0, "%.1f niveis (< 5)" % ponto)
rim.specular_factor = especular_rim
if SO_Q1:
    print("[teste] FALHOU: " + ", ".join(falhas) if falhas else "[teste] q1 dentro do criterio")
    sys.exit(0)


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
ROT_CUBO = 22.0
POS_CUBO = (0.40, -0.05, 0.20)
cubo = corpo(bpy.ops.mesh.primitive_cube_add, "cubo",
             material("grafite", mod_ambiente.cor_linear("#1E2024"), 0.55),
             POS_CUBO, rot=(0, 0, math.radians(ROT_CUBO)), size=0.40)
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

# flash no quadro 20, forca padrao (0,5 = emissao 8): 19 sem veu, 21 meio veu
mod_ambiente.animar_flash(amb, cam, 20)

# --- imagens dos corpos de prova ---
alvo.location = (0.0, -0.1, 0.30)
angulos = {
    "frente": (0.0, -3.0, 0.75),
    "3q": (2.0, -2.2, 1.25),
    "cima": (0.4, -1.5, 3.0),
}
caminhos = {"q1": caminho_q1}
for rotulo, pos in angulos.items():
    cam.location = pos
    caminhos[rotulo] = renderizar(rotulo, 1)

cam.location = angulos["3q"]
for rotulo, quadro in (("flash_antes", 19), ("flash", 20), ("flash_depois", 21)):
    caminhos[rotulo] = renderizar(rotulo, quadro)

# --- lateral: a face +X do cubo de frente, rig a azimute+90 e a azimute+60 ---
# A camera fica na normal da face (azimute 22 graus, o giro do cubo); as duas
# chaves do rig ficam DEPOIS dos quadros ja renderizados, porque a fcurve
# extrapola a primeira chave para tras e mudaria o rig dos quadros 1-21.
az_lateral = ROT_CUBO
d = 1.8
cam.location = (POS_CUBO[0] + d * math.cos(math.radians(az_lateral)),
                POS_CUBO[1] + d * math.sin(math.radians(az_lateral)), 0.45)
alvo.location = POS_CUBO
cam.data.lens = 50.0
mod_ambiente.animar_rig(amb, 40, 41, az_lateral, az_lateral, azimutes=True, offset=mod_ambiente.OFFSET_RIM_ATRAS)
mod_ambiente.animar_rig(amb, 50, 51, az_lateral, az_lateral, azimutes=True, offset=mod_ambiente.OFFSET_RIM_LATERAL)
caminhos["lateral_atras"] = renderizar("lateral_atras", 40)
caminhos["lateral"] = renderizar("lateral", 50)
cam.data.lens = 35.0

# --- medidas: frente e 3q ---
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
        col30 = [L[i, 30] * 255 for i in range(int(h * 180 / 960), int(h * 420 / 960) + 1, int(h * 10 / 960))]
        print("[medida] frente: coluna 30, linhas 180-420 de 10 em 10: " + " ".join("%d" % round(v) for v in col30))
        pico = int(np.argmax(col30))
        sobe = all(col30[i + 1] >= col30[i] - 2 for i in range(pico))
        desce = all(col30[i + 1] <= col30[i] + 2 for i in range(pico, len(col30) - 1))
        criterio("sem linha no horizonte", sobe and desce, "unimodal: sobe=%s desce=%s" % (sobe, desce))
        # Banding: escada e PATAMAR (3+ linhas iguais) seguido de SALTO (>= 2
        # niveis). Rampa continua com 2 niveis por linha nao e escada - e a
        # inclinacao do gradiente a 960 px (1 por linha a 1920). O perfil e a
        # media de 20 colunas para o dither de saida (+-2 por pixel, de
        # proposito) nao contar como salto.
        ceu = L[int(h * 20 / 960):int(h * 320 / 960), 20:40].mean(axis=1) * 255
        dif = np.diff(ceu)
        escada = 0
        for i in range(3, len(dif)):
            patamar = all(abs(v) < 0.35 for v in dif[i - 3:i])
            if patamar and abs(dif[i]) >= 2.0:
                escada += 1
        print("[medida] frente: ceu (linhas 20-320, media de 20 colunas) inclinacao max %.2f niveis/linha, %d niveis distintos"
              % (np.abs(dif).max(), len(np.unique(np.round(ceu)))))
        criterio("sem escada no ceu", escada == 0 and np.abs(dif).max() <= 3.0,
                 "%d patamar+salto, inclinacao max %.2f (<= 3)" % (escada, np.abs(dif).max()))

# --- medidas: flash ---
# O quadro 19 (alfa 0 CONSTANT ate o 20) tem de ser a MESMA imagem do 3q no
# quadro 1: nada mais esta animado ate o 20. Com a rampa linear antiga o
# motion blur (obturador START) expunha meio flash no 19.
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
# Meio veu: mais claro que a cena, mas a cena visivel por tras - nem branco
# (0,35 de decaimento dava media 0,96) nem sumido.
criterio("flash decai no quadro seguinte", L3q.mean() + 0.15 < L21.mean() < 0.90 and L21.min() < 0.80,
         "q21: media %.3f entre 3q + 0,15 = %.3f e 0,90; min %.3f < 0,80" % (L21.mean(), L3q.mean() + 0.15, L21.min()))

# --- medidas: lateral ---
# Face do cubo: colunas 28-72%, linhas 40-60% (0,40 m a 1,8 m com 50 mm).
# Gradiente = amplitude das medias de coluna e de linha, em niveis. A
# hipotese da revisao (offset 60 da gradiente na lateral) MORREU medida:
# num cubo branco a amplitude foi 1,7 (+60) contra 4,3 (+90), ver o
# comentario das constantes OFFSET_* no modulo. Aqui se imprime a medida e
# se prova a API: o rig fica em azimute + offset nos dois quadros.
grad = {}
for rotulo in ("lateral_atras", "lateral"):
    Lf = lum(ler_png(caminhos[rotulo]))
    h, w = Lf.shape
    face = Lf[int(h * 0.40):int(h * 0.60), int(w * 0.28):int(w * 0.72)]
    colunas = face.mean(axis=0) * 255
    linhas = face.mean(axis=1) * 255
    grad[rotulo] = float(colunas.max() - colunas.min())
    print("[medida] %s: face L media %.3f, amplitude horizontal %.1f niveis (esq %.1f dir %.1f), vertical %.1f (topo %.1f base %.1f)"
          % (rotulo, face.mean(), grad[rotulo], colunas[:len(colunas) // 4].mean(), colunas[-len(colunas) // 4:].mean(),
             linhas.max() - linhas.min(), linhas[:len(linhas) // 4].mean(), linhas[-len(linhas) // 4:].mean()))
rig_luz = amb["rig"]
cena.frame_set(40)
ang40 = math.degrees(rig_luz.rotation_euler.z)
cena.frame_set(50)
ang50 = math.degrees(rig_luz.rotation_euler.z)
criterio("animar_rig com azimutes aplica o offset", abs(ang40 - (az_lateral + 90.0)) < 1e-3 and abs(ang50 - (az_lateral + 60.0)) < 1e-3,
         "rig q40 = %.1f (azimute %.0f + 90), q50 = %.1f (+ 60)" % (ang40, az_lateral, ang50))

# --- prova numerica: chavear_especular e Bezier, nunca constant ---
# Depois de todos os renders: a fcurve extrapola a primeira chave para tras e
# zeraria o especular do rim nos quadros ja gravados.
mod_ambiente.chavear_especular(amb["luzes"]["rim"], 100, 112, de=0.0, para=0.5)
fc = next(f for f in rim.animation_data.action.fcurves if f.data_path == "specular_factor")
valores = [fc.evaluate(q) for q in (99, 100, 103, 106, 109, 112, 113)]
interps = {int(round(kp.co.x)): kp.interpolation for kp in fc.keyframe_points}
print("[medida] chavear_especular 100->112: " + " ".join("q%d=%.3f" % (q, v) for q, v in zip((99, 100, 103, 106, 109, 112, 113), valores))
      + "; interpolacoes " + ", ".join("q%d=%s" % (q, i) for q, i in sorted(interps.items())))
# de=0 com o rim em 0,6: a chave de espera em q99 segura o 0,6 (CONSTANT) ate
# o salto pedido; as duas chaves da rampa sao Bezier e a subida e monotona.
monotona = all(valores[i + 1] >= valores[i] for i in range(1, len(valores) - 1))
criterio("rampa de especular e Bezier", interps.get(100) == "BEZIER" and interps.get(112) == "BEZIER" and monotona
         and abs(valores[1]) < 1e-6 and abs(valores[5] - 0.5) < 1e-6 and 0.15 < valores[3] < 0.35
         and abs(valores[0] - especular_rim) < 1e-6,
         "q100 e q112 Bezier, monotona, 0 em q100, 0,5 em q112, meio q106=%.3f, q99 segura %.2f" % (valores[3], valores[0]))
# de=None parte do valor animado: em q120 o rim esta em 0,5 (extrapolacao)
mod_ambiente.chavear_especular(amb["luzes"]["rim"], 120, rampa=6, para=0.0)
v120, v123, v126 = fc.evaluate(120), fc.evaluate(123), fc.evaluate(126)
criterio("rampa parte do valor atual", abs(v120 - 0.5) < 1e-6 and abs(v126) < 1e-6 and 0.1 < v123 < 0.4,
         "q120=%.3f q123=%.3f q126=%.3f" % (v120, v123, v126))
# rampa=0 e corte: uma chave so, CONSTANT
mod_ambiente.chavear_especular(amb["luzes"]["rim"], 140, rampa=0, para=0.5)
kp140 = next(kp for kp in fc.keyframe_points if int(round(kp.co.x)) == 140)
criterio("rampa 0 e corte CONSTANT", kp140.interpolation == "CONSTANT" and abs(fc.evaluate(139)) < 1e-6 and abs(fc.evaluate(140) - 0.5) < 1e-6,
         "q139=%.3f q140=%.3f interp=%s" % (fc.evaluate(139), fc.evaluate(140), kp140.interpolation))
rim.animation_data_clear()
rim.specular_factor = especular_rim


# --- prova: caminho do compositor do 5.0 (sem use_nodes/node_tree) ---
# Nao ha Blender 5.0 aqui. O que se prova e o ramo do codigo que o 5.0
# percorre: um objeto sem use_nodes nem node_tree, so com
# compositing_node_group. (1) Vazio: _arvore_compositor cria um
# CompositorNodeTree e o atribui. No 4.2 esse grupo solto recusa o no Render
# Layers ('Cannot add node of type CompositorNodeRLayers'), o que e regra do
# 4.2 e nao do codigo - por isso (2) o bloom inteiro e provado com a arvore
# do compositor da cena de verdade entregue por esse mesmo atributo.
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
# E o ramo do 4.2 continua no compositor da cena de verdade
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
