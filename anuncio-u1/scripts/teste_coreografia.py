# Prova da coreografia: cena vazia, todos os modulos, os sete beats, e
# renders para olhar. Roda com:
#   bash scripts/previa.sh scripts/teste_coreografia.py
#
# Modos (variavel MODO):
#   chave  (padrao) um quadro por beat (meio) -> saida/beat_N.png, 540x960,
#          16 amostras; QUADROS="350,545" acrescenta quadros extras
#          (saida/quadro_NNN.png); BEATS="1,4" limita os beats; PASTA=...
#          troca a pasta de saida; LARG/ALT/AMOSTRAS trocam o tamanho.
#   lote   quadros INI..FIM de PASSO em PASSO -> saida/previa_seq/quadro_NNNN.png
#          em 360x640, 8 amostras (previa do video, em lotes de < 10 min:
#          scripts/lotes.sh faz os lotes).
#   video  junta saida/previa_seq/quadro_*.png num MP4 a 15 fps (FPS_VIDEO)
#          com o ffmpeg do Blender -> saida/previa_20s.mp4 (NOME_VIDEO).
#   nada   so constroi, coreografa e confere (rapido; para depurar).
#
# Outras variaveis: DURACAO_S (25 padrao, 20 ou 15), CAIXA_SOME=0 (o U1 para no ar na
# frente da caixa em vez de ela sumir por baixo), U1_NOME=MeuU1 (exercita o
# modelo real com um bloco), SONDA_VEL="160-200,260-300" (velocidade da
# camera por quadro, m/quadro, para achar paradas fora de corte),
# SONDA_ENQ="40,75,150" (fracao da altura/largura do quadro que o U1 e a
# caixa ocupam - a medida do "produto >= 60% da altura" da revisao 2; o
# padrao sao os quadros de medicao pedidos), PASSO_COLISAO,
# SONDA_ROT="1-160" (revisao 4: velocidade angular da caixa em graus/quadro,
# bob e inclinacao), SONDA_CABO="235-305" (plugue projetado por quadro: em
# que quadro entra no quadro, altura), MEDIR_FUNDO="180" (luminancia por
# regiao do quadro_180.png renderizado, para posicionar a legenda).
#
# Custo medido por software (llvmpipe, 4 nucleos): 16-41 s por quadro da cena
# completa a 360x640 com 8 amostras (o previa.sh fala em 6 s: e para os
# corpos de prova dos modulos, nao para a cena inteira).
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
import mod_ambiente, mod_caixa, mod_u1, mod_cabo, mod_cartela, mod_coreografia  # noqa: E401
for m in (mod_ambiente, mod_caixa, mod_u1, mod_cabo, mod_cartela, mod_coreografia):
    importlib.reload(m)

RAIZ = os.path.dirname(AQUI)
SAIDA = os.environ.get("PASTA") or os.path.join(RAIZ, "saida")
MODO = os.environ.get("MODO", "chave")
DURACAO = float(os.environ.get("DURACAO_S", "25"))

bpy.ops.wm.read_factory_settings(use_empty=True)

if MODO == "video":
    # Junta saida/previa_seq/quadro_*.png (de 2 em 2 quadros) num MP4 a 15
    # fps pelo VSE, com o ffmpeg embutido do Blender. Nao constroi a cena.
    pasta = os.path.join(SAIDA, "previa_seq")
    quadros = sorted(f for f in os.listdir(pasta) if f.startswith("quadro_") and f.endswith(".png"))
    cena = bpy.context.scene
    cena.sequence_editor_create()
    # 4.4+ renomeou sequences -> strips (no 4.2 'sequences' existe e esta
    # VAZIA: testar por None, nao por verdade - medido, 'or' caia em strips).
    faixas = getattr(cena.sequence_editor, "sequences", None)
    if faixas is None:
        faixas = cena.sequence_editor.strips
    faixa = faixas.new_image("previa", os.path.join(pasta, quadros[0]), 1, 1)
    for f in quadros[1:]:
        faixa.elements.append(f)
    fps = int(os.environ.get("FPS_VIDEO", "15"))
    cena.render.fps = fps
    cena.render.fps_base = 1.0
    cena.frame_start = 1
    cena.frame_end = len(quadros)
    cena.render.resolution_x = int(os.environ.get("LARG", "360"))
    cena.render.resolution_y = int(os.environ.get("ALT", "640"))
    cena.render.resolution_percentage = 100
    cena.render.image_settings.file_format = "FFMPEG"
    cena.render.ffmpeg.format = "MPEG4"
    cena.render.ffmpeg.codec = "H264"
    cena.render.ffmpeg.constant_rate_factor = "HIGH"
    cena.render.ffmpeg.ffmpeg_preset = "GOOD"
    cena.render.ffmpeg.gopsize = fps
    # Os PNGs ja estao em sRGB com AgX aplicado: o VSE nao pode transformar de novo.
    cena.view_settings.view_transform = "Standard"
    cena.render.filepath = os.path.join(SAIDA, os.environ.get("NOME_VIDEO", "previa_20s.mp4"))
    bpy.ops.render.render(animation=True)
    print("[teste] video: %d quadros a %d fps -> %s" % (len(quadros), fps, cena.render.filepath))
    raise SystemExit(0)

params = {"duracao_s": DURACAO, "pasta_assets": os.path.join(RAIZ, "assets"),
          "caixa_some": os.environ.get("CAIXA_SOME", "1") not in ("0", "false", "False")}
if os.environ.get("HEROI"):
    # Variantes da camera do momento-heroi (beat 2): "z,alvo,raio".
    z, alvo, raio = [float(v) for v in os.environ["HEROI"].split(",")]
    params["camera_heroi"] = {"z": z, "alvo": alvo, "raio": raio}
if os.environ.get("U1_NOME"):
    # Exercita o caminho do MODELO REAL sem o modelo real: um bloco nas
    # dimensoes do U1, deslocado e girado de proposito, com um filho 'tela'
    # emissivo. O que se prova: parentear, medir, centralizar, apoiar no chao,
    # pontos por heuristica e animar_tela num material de fora.
    import bmesh
    from mathutils import Vector
    nome = os.environ["U1_NOME"]
    bm = bmesh.new()
    bmesh.ops.create_cube(bm, size=1.0)
    bmesh.ops.scale(bm, vec=(0.55, 0.47, 0.70), verts=bm.verts)
    malha = bpy.data.meshes.new(nome)
    bm.to_mesh(malha)
    bm.free()
    bloco = bpy.data.objects.new(nome, malha)
    bloco.location = (1.3, -0.8, 0.9)
    bloco.rotation_euler = (0, 0, 0.6)
    bpy.context.scene.collection.objects.link(bloco)
    tela_m = bpy.data.meshes.new(nome + ".tela")
    tela_m.from_pydata([(-0.05, -0.236, 0.2), (0.05, -0.236, 0.2), (0.05, -0.236, 0.27), (-0.05, -0.236, 0.27)], [], [(0, 1, 2, 3)])
    tela = bpy.data.objects.new(nome + ".tela", tela_m)
    tela.parent = bloco
    bpy.context.scene.collection.objects.link(tela)
    mat = bpy.data.materials.new("cliente.tela")
    mat.use_nodes = True
    tela_m.materials.append(mat)
    params.update({"u1_nome": nome, "u1_rotacao_z": -34.4, "u1_tela_objeto": nome + ".tela"})
objs = mod_coreografia.construir_tudo(params)
if os.environ.get("U1_NOME"):
    u1 = objs["u1"]
    mn, mx = u1["envelope"]
    print("[teste] modelo real: envelope x %.3f..%.3f y %.3f..%.3f z %.3f..%.3f  tela %s  tomada %s" % (
        mn.x, mx.x, mn.y, mx.y, mn.z, mx.z,
        tuple(round(v, 3) for v in u1["posicao_tela"]["centro"]), tuple(round(v, 3) for v in u1["posicao_tomada"]["ponto"])))
mod_coreografia.coreografar(objs)
print("[teste] quadros: 1..%d  fator=%.2f  travessia=%d  caixa_some=%s" % (
    objs["cena"].frame_end, objs["fator"], objs["q_travessia"], params["caixa_some"]))
for n, q in mod_coreografia.quadros_chave(objs["fator"]):
    a, b = mod_coreografia.quadros_do_beat(n, objs["fator"])
    print("[teste] beat %d: %d..%d  chave %d" % (n, a, b, q))
mod_coreografia.conferir_colisoes(objs, passo=int(os.environ.get("PASSO_COLISAO", "1")))

# Enquadramento (revisao 2): fracao da altura e da largura do 9:16 que o
# envelope do U1 e o da caixa ocupam, por projecao dos cantos pela camera.
# Padrao (25 s): meio do beat 1, caixa parada no fim dele, momento-heroi,
# meio da orbita traseira, meio da orbita da frente (q372; q396 ja e o dolly
# na tela), pico do U1 no beat 6,
# caixa fechada no fim do beat 6 e apice do beat 7 - os mesmos instantes dos
# q040/q075/q150/q218/q315/q466/q507 pedidos na linha do tempo de 20 s.
quadros_enq = [int(v) for v in os.environ.get("SONDA_ENQ", "1,42,84,180,276,372,396,579,620,649").split(",") if v.strip()]
mod_coreografia.medir_enquadramento(objs, quadros_enq)
# A caixa tem de estar FORA do quadro no quadro em que parte (1), no ultimo
# em que ainda e visivel ao sumir (beat 2) e no primeiro em que volta (beat 6).
if params["caixa_some"] and "_q_caixa_some" in objs:
    fora = mod_coreografia.medir_enquadramento(
        objs, [1, objs["_q_caixa_some"] - 1, objs["_q_caixa_volta"]], alvos=("caixa",))
    for q_, res in fora.items():
        print("[teste] caixa em q%d: %s%s" % (q_, res["caixa"][2], "" if res["caixa"][2] == "fora" else "  <-- DEVIA ESTAR FORA"))
    print("[teste] profundidades: partida %.2f m, saida %.2f m, volta %.2f m" % (
        objs["profundidade_caixa"], objs["profundidade_saida"], objs["profundidade_volta"]))
if objs.get("_obturador"):
    print("[teste] obturador por chave: " + " ".join("q%d=%.2f" % (q_, v) for q_, v in objs["_obturador"]))

# Sonda da camera nos quadros-chave: posicao no mundo, alvo e lente - e o
# que diz se o enquadramento e o planejado antes de gastar um render.
cam, alvo = objs["camera"], objs["alvo"]
for n, q in mod_coreografia.quadros_chave(objs["fator"]):
    objs["cena"].frame_set(q)
    p = cam.matrix_world.translation
    a = alvo.matrix_world.translation
    print("[sonda] q%3d beat %d: cam (%.2f, %.2f, %.2f)  alvo (%.2f, %.2f, %.2f)  dist %.2f  lente %.0f  f/%.1f"
          % (q, n, p.x, p.y, p.z, a.x, a.y, a.z, (p - a).length, cam.data.lens, cam.data.dof.aperture_fstop))

# Velocidade da camera por quadro (m/quadro) nos trechos pedidos: parada
# fora de corte (< 0,005) e solavanco aparecem aqui antes do render.
if os.environ.get("SONDA_VEL"):
    rim = objs["ambiente"]["luzes"]["rim"].data
    for trecho in os.environ["SONDA_VEL"].split(","):
        a, b = [int(v) for v in trecho.split("-")]
        objs["cena"].frame_set(a - 1)
        p0 = cam.matrix_world.translation.copy()
        for q in range(a, b + 1):
            objs["cena"].frame_set(q)
            p1 = cam.matrix_world.translation.copy()
            spec = getattr(rim, "specular_factor", -1.0)
            print("[vel] q%3d  %.4f m/quadro  cam (%.2f, %.2f, %.2f)  rim spec %.2f  alfa veu %s" % (
                q, (p1 - p0).length, p1.x, p1.y, p1.z, spec,
                "%.2f" % objs["ambiente"]["flash"].data.materials[0].node_tree.nodes["alfa"].outputs[0].default_value
                if objs["ambiente"].get("flash") else "-"))
            p0 = p1
objs["cena"].frame_set(1)

# Revisao 4: velocidade ANGULAR da caixa por quadro (graus/quadro, o angulo
# da rotacao relativa entre dois quadros seguidos - inclui o giro e a
# inclinacao), o bob (z do corpo depois de assentar) e a inclinacao (angulo
# entre o Z local e o Z do mundo). O criterio: maximo <= 1,5 graus/quadro.
if os.environ.get("SONDA_ROT"):
    corpo = objs["caixa"]["corpo"]
    a, b = [int(v) for v in os.environ["SONDA_ROT"].split("-")]
    objs["cena"].frame_set(a - 1)
    R0 = corpo.matrix_world.to_3x3().copy()
    pior = (0.0, None)
    zs = []
    incl_max = (0.0, None)
    for q in range(a, b + 1):
        objs["cena"].frame_set(q)
        R1 = corpo.matrix_world.to_3x3().copy()
        vel = math.degrees((R1 @ R0.transposed()).to_quaternion().angle)
        z = corpo.matrix_world.translation.z
        incl = math.degrees((R1 @ Vector((0, 0, 1))).angle(Vector((0, 0, 1))))
        if vel > pior[0]:
            pior = (vel, q)
        if incl > incl_max[0]:
            incl_max = (incl, q)
        # Bob "depois de assentar": entre o fim da subida (fracao subida_fim
        # do beat 2) e o inicio da descida da caixa.
        fl_ = dict(mod_coreografia.PARAMS_PADRAO["flutuar"], **(objs["params"].get("flutuar") or {}))
        q_assenta = mod_coreografia.q_em(2, fl_["subida_fim"], objs["fator"])
        q_desce = mod_coreografia.q_em(2, mod_coreografia.ROTEIRO[2]["caixa_desce"][0], objs["fator"])
        if q_assenta <= q <= q_desce:
            zs.append(z)
        if (q - a) % 4 == 0 or q == b:
            print("[rot] q%3d  %.3f graus/quadro  giro Z %7.2f  inclinacao %.2f graus  z %.4f" % (
                q, vel, math.degrees(R1.to_euler("XYZ").z), incl, z))
        R0 = R1
    print("[rot] MAXIMO %.3f graus/quadro em q%s (criterio <= 1,5: %s); inclinacao maxima %.2f graus (q%s); bob apos assentar: z de %.4f a %.4f (amplitude %.1f cm)" % (
        pior[0], pior[1], "OK" if pior[0] <= 1.5 else "FALHA", incl_max[0], incl_max[1],
        min(zs) if zs else 0.0, max(zs) if zs else 0.0, 100 * (max(zs) - min(zs)) / 2.0 if zs else 0.0))
    objs["cena"].frame_set(1)

# Revisao 4: onde o plugue esta em cada quadro do voo, projetado pela camera
# - o quadro em que ENTRA no quadro (tem de partir de fora), a altura (tem
# de ser a da tomada) e a distancia ate a tomada.
if os.environ.get("SONDA_CABO"):
    a, b = [int(v) for v in os.environ["SONDA_CABO"].split("-")]
    plugue = objs["cabo"]["plugue"]
    # A tomada com o U1 na cota de referencia (beat 3), nao no quadro 1
    # (dentro da caixa, 2 m abaixo do quadro).
    objs["cena"].frame_set(mod_coreografia.quadros_do_beat(3, objs["fator"])[0])
    tomada = mod_u1.ponto_no_mundo(objs["u1"], "posicao_tomada", "ponto").copy()
    primeiro = None
    for q in range(a, b + 1):
        objs["cena"].frame_set(q)
        pos = plugue.matrix_world.translation.copy()
        proj = mod_coreografia.projetar_no_quadro(objs, [pos])
        dentro = bool(proj) and abs(proj[0][0]) <= 9.0 / 16.0 and abs(proj[0][1]) <= 1.0
        if dentro and primeiro is None:
            primeiro = q
        if (q - a) % 3 == 0 or q == b or q == primeiro:
            print("[cabo] q%3d  plugue (%.2f, %.2f, %.3f)  a %.2f m da tomada (z %.3f)  %s" % (
                q, pos.x, pos.y, pos.z, (pos - tomada).length, tomada.z,
                ("no quadro x=%.2f y=%.2f" % proj[0]) if proj else "atras da camera" if not dentro else "no quadro"))
    print("[cabo] plugue entra no quadro em q%s (parte de fora: %s); ponto_fora %s; trajeto %s" % (
        primeiro, "OK" if primeiro and primeiro > a else "FALHA",
        tuple(round(v, 2) for v in objs["cabo"]["ponto_fora"]), objs["cabo"].get("trajeto")))
    objs["cena"].frame_set(1)

if objs.get("_q_legenda"):
    print("[teste] legenda: q%d..q%d" % objs["_q_legenda"])

# Cartela assentada (fim da entrada, beat 7): topo da logo e fundo de cada
# linha em fracao da altura do quadro - a ultima tem de ficar acima de ~70%
# (faixa de legendas do Reels) e a logo abaixo do topo com folga.
q_cart = mod_coreografia.q_em(7, mod_coreografia.ROTEIRO[7]["cartela"][1], objs["fator"])
objs["cena"].frame_set(q_cart)
bpy.context.view_layer.update()
cart = objs["cartela"]
for nome, obj in [("logo", cart.get("logo"))] + [("linha %d" % (i + 1), o) for i, o in enumerate(cart["linhas"])]:
    if obj is None:
        continue
    proj = mod_coreografia.projetar_no_quadro(objs, mod_coreografia._cantos(obj))
    if proj:
        ys = [y for _, y in proj]
        xs = [x for x, _ in proj]
        print("[cartela] q%d %-8s y %5.1f%%-%5.1f%% da altura, largura %3.0f%%" % (
            q_cart, nome, 100 * (0.5 - max(ys) / 2.0), 100 * (0.5 - min(ys) / 2.0), 100 * (max(xs) - min(xs)) / (2 * 9.0 / 16.0)))
objs["cena"].frame_set(1)


def medir_fundo(caminho, colunas=5, linhas=10, ate=0.5):
    """Luminancia media (sRGB 0-255) por regiao de um PNG renderizado: a
    grade cobre a fracao 'ate' de cima do quadro. E o que decide onde a
    legenda branca do heroi cabe (revisao 4): sobre o rose ela nao le."""
    import numpy as np
    img = bpy.data.images.load(caminho, check_existing=False)
    w, h = img.size
    px = np.empty(w * h * 4, dtype=np.float32)
    img.pixels.foreach_get(px)
    a = px.reshape(h, w, 4)[::-1, :, :3]
    lum = (0.2126 * a[:, :, 0] + 0.7152 * a[:, :, 1] + 0.0722 * a[:, :, 2]) * 255.0
    print("[fundo] %s: luminancia media por regiao (linhas = decimos da altura ate %.0f%%, colunas = quintos da largura)" % (
        os.path.basename(caminho), 100 * ate))
    for j in range(linhas):
        y0, y1 = int(j * ate / linhas * h), int((j + 1) * ate / linhas * h)
        celulas = []
        for i in range(colunas):
            x0, x1 = int(i * w / colunas), int((i + 1) * w / colunas)
            celulas.append(lum[y0:y1, x0:x1].mean())
        print("[fundo]  %3.0f-%3.0f%%: " % (100 * j * ate / linhas, 100 * (j + 1) * ate / linhas)
              + "  ".join("%5.0f" % c for c in celulas))
    bpy.data.images.remove(img)


if MODO == "chave":
    mod_coreografia.configurar_render(objs, int(os.environ.get("LARG", "540")), int(os.environ.get("ALT", "960")),
                                      int(os.environ.get("AMOSTRAS", "16")))
    os.makedirs(SAIDA, exist_ok=True)
    for n, q in mod_coreografia.quadros_chave(objs["fator"]):
        if os.environ.get("BEATS") is not None and str(n) not in os.environ["BEATS"].split(","):
            continue
        caminho = mod_coreografia.renderizar_quadro(objs, q, os.path.join(SAIDA, "beat_%d.png" % n))
        print("[teste] gravado", caminho)
    for q in [int(v) for v in os.environ.get("QUADROS", "").split(",") if v.strip()]:
        caminho = mod_coreografia.renderizar_quadro(objs, q, os.path.join(SAIDA, "quadro_%03d.png" % q))
        print("[teste] gravado", caminho)
        if str(q) in os.environ.get("MEDIR_FUNDO", "").split(","):
            medir_fundo(caminho)
elif MODO == "lote":
    ini = int(os.environ.get("INI", "1"))
    fim = int(os.environ.get("FIM", str(objs["cena"].frame_end)))
    passo = int(os.environ.get("PASSO", "2"))
    pasta = os.path.join(SAIDA, "previa_seq")
    os.makedirs(pasta, exist_ok=True)
    mod_coreografia.configurar_render(objs, int(os.environ.get("LARG", "360")), int(os.environ.get("ALT", "640")),
                                      int(os.environ.get("AMOSTRAS", "8")))
    for q in range(ini, fim + 1, passo):
        mod_coreografia.renderizar_quadro(objs, q, os.path.join(pasta, "quadro_%04d.png" % q))
    print("[teste] lote %d..%d gravado em %s" % (ini, fim, pasta))
