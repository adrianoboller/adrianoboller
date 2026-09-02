# Teste do modulo U1: cena vazia, so o U1 substituto, camera e luz simples.
# Renderiza em 540x960 e o autor abre cada PNG e olha:
#   previa_u1_frente.png        - frente 3/4, tela em standby (ja ligou)
#   previa_u1_tras.png          - traseira com tomada e botao, camara acesa
#   previa_u1_tela.png          - close da tela com a UI ligada
#   previa_u1_tela_boot.png     - meio do boot
#   previa_u1_cabecote.png      - close de um cabecote
#   previa_u1_ligando_antes.png - traseira 3/4 antes do botao (tudo apagado)
#   previa_u1_ligando.png       - traseira 3/4 no meio de animar_ligar
# Os *_rodada1.png e *_rodada2.png sao os das rodadas anteriores, para comparar.
#
# PARTE=cena (rodada 3): o U1 dentro do AMBIENTE do anuncio (world, chao e
# rig do mod_ambiente), nos enquadramentos em que a revisao viu defeito, com
# a medida no pixel de cada um:
#   previa_u1_cena_tela.png     - q359: camera a 0,26 m da tela com a UI
#                                 acesa; L do fundo da UI em TODA a tela
#   previa_u1_cena_foto_c.png   - foto C (q445): mesa de cima, luzes da camara
#                                 a 10 W; faixa continua >= 250 ao longo das hastes
#   previa_u1_cena_foto_a.png   - foto A (q389): cabecotes estacionados; a
#                                 'barra branca' (labio do casco sob o aro)
#   previa_u1_cena_lateral_*.png - lateral (q185) com a key do ambiente, com
#                                 key 1x1 m a 2,5 m e com softbox 1x2 vertical;
#                                 amplitude horizontal de L na face
# PARTE=modulo roda so os renders de cima; sem PARTE roda os dois. PASTA=...
# troca a pasta de saida (padrao saida/).
#
# Provas numericas (asserts): idempotencia, envelope, imagens empacotadas,
# luzes escondidas antes de ligar, material do cliente com Principled
# desligado sobrando + Emission ligado -> a chave cai no Emission, a
# luminancia da janela traseira sobe entre 'antes' e 'ligando', e os
# criterios da parte de cena (tela, hastes, labio).
#
# Uso: bash scripts/previa.sh scripts/teste_u1.py

import math
import os
import sys

import bpy
from mathutils import Vector

RAIZ = "/home/user/adrianoboller/anuncio-u1"
sys.path.insert(0, os.path.join(RAIZ, "scripts"))

import importlib
import mod_u1
import mod_ambiente
importlib.reload(mod_u1)
importlib.reload(mod_ambiente)

LARGURA, ALTURA_IMG = int(os.environ.get("LARG", "540")), int(os.environ.get("ALT", "960"))
AMOSTRAS = int(os.environ.get("AMOSTRAS", "16"))
SAIDA = os.environ.get("PASTA") or os.path.join(RAIZ, "saida")
PARTE = os.environ.get("PARTE", "")


def cena_limpa():
    bpy.ops.wm.read_factory_settings(use_empty=True)
    cena = bpy.context.scene
    cena.unit_settings.system = "METRIC"
    cena.unit_settings.scale_length = 1.0
    cena.render.fps = 30
    cena.render.resolution_x = LARGURA
    cena.render.resolution_y = ALTURA_IMG
    cena.render.resolution_percentage = 100
    # EEVEE Next; no Blender 5 o identificador volta a ser BLENDER_EEVEE.
    try:
        cena.render.engine = "BLENDER_EEVEE_NEXT"
    except TypeError:
        cena.render.engine = "BLENDER_EEVEE"
    cena.eevee.taa_render_samples = AMOSTRAS
    try:
        cena.eevee.use_raytracing = True
    except AttributeError:
        pass
    cena.view_settings.view_transform = "AgX"
    try:
        cena.view_settings.look = "AgX - Medium High Contrast"
    except TypeError:
        pass
    return cena


def luz(nome, tipo, pos, alvo, energia, tamanho=1.0, cor=(1, 1, 1)):
    dados = bpy.data.lights.new(nome, tipo)
    dados.energy = energia
    dados.color = cor
    if tipo == "AREA":
        dados.size = tamanho
    obj = bpy.data.objects.new(nome, dados)
    bpy.context.scene.collection.objects.link(obj)
    obj.location = pos
    direcao = Vector(alvo) - Vector(pos)
    obj.rotation_euler = direcao.to_track_quat("-Z", "Y").to_euler()
    return obj


def estudio(cena):
    mundo = bpy.data.worlds.new("mundo")
    cena.world = mundo
    mundo.use_nodes = True
    fundo = mundo.node_tree.nodes["Background"]
    # Mundo escuro como o fundo do anuncio (#050507): e o que a tela desligada
    # e o vidro refletem. Com o mundo cinza da primeira versao a tela lia como
    # um adesivo cinza e o teste nao mostrava o que o anuncio vai mostrar.
    fundo.inputs["Color"].default_value = (0.012, 0.012, 0.015, 1.0)
    fundo.inputs["Strength"].default_value = 1.0
    # Chao ESCURO: o espelho da tela desligada, visto da camera da frente,
    # cai no chao (calculado: a reflexao do olhar sai para +X, -Y e para
    # baixo) - com chao claro a tela lia como um adesivo cinza mesmo com o
    # mundo preto. O branco do casco se ilumina pelas areas, nao pelo chao.
    malha = bpy.data.meshes.new("chao")
    malha.from_pydata([(-4, -4, 0), (4, -4, 0), (4, 4, 0), (-4, 4, 0)], [], [(0, 1, 2, 3)])
    chao = bpy.data.objects.new("chao", malha)
    cena.collection.objects.link(chao)
    mat = bpy.data.materials.new("chao")
    mat.use_nodes = True
    b = mat.node_tree.nodes["Principled BSDF"]
    b.inputs["Base Color"].default_value = (0.05, 0.05, 0.055, 1.0)
    b.inputs["Roughness"].default_value = 0.45
    malha.materials.append(mat)
    # Sem o mundo claro, as areas crescem para o branco do casco ter o que
    # refletir (luz de estudio, nao ambiente). O preenchimento fica PEQUENO e
    # alto: medido, a tela desligada vista da camera da frente espelha a
    # direcao (+0,68, -0,72, -0,15), e um softbox de 2,2 m em (1,6, -1,0, 0,9)
    # cai a 27 graus dela cobrindo 34 - a tela virava um retangulo cinza
    # uniforme (0,77 no PNG) com o material certo. A 46 graus e 0,8 m, sai.
    luz("chave", "AREA", (-1.1, -1.4, 1.7), (0, 0, 0.4), 700, 1.8)
    luz("preenchimento", "AREA", (1.9, -0.6, 1.4), (0, 0, 0.4), 220, 0.8)
    luz("contra", "AREA", (0.5, 1.6, 1.5), (0, 0, 0.5), 400, 1.2)


def camera(cena):
    dados = bpy.data.cameras.new("camera.teste")
    cam = bpy.data.objects.new("camera.teste", dados)
    cena.collection.objects.link(cam)
    alvo = bpy.data.objects.new("camera.alvo", None)
    cena.collection.objects.link(alvo)
    c = cam.constraints.new("TRACK_TO")
    c.target = alvo
    c.track_axis = "TRACK_NEGATIVE_Z"
    c.up_axis = "UP_Y"
    cena.camera = cam
    return cam, alvo


def foto(cena, cam, alvo, pos, olhar, lente, quadro, arquivo):
    cam.location = pos
    cam.data.lens = lente
    alvo.location = olhar
    cena.frame_set(quadro)
    cena.render.filepath = os.path.join(SAIDA, arquivo)
    bpy.ops.render.render(write_still=True)
    print("[teste_u1] gravou", cena.render.filepath)
    return cena.render.filepath


def luminancia_media(arquivo, x0, x1, y0, y1):
    """Media da luminancia (0..1, sRGB do PNG) num retangulo em fracoes do quadro, y de cima."""
    import numpy as np
    img = bpy.data.images.load(arquivo)
    w, h = img.size
    px = np.array(img.pixels[:], dtype=np.float32).reshape(h, w, 4)[::-1]
    bpy.data.images.remove(img)
    rec = px[int(h * y0):int(h * y1), int(w * x0):int(w * x1), :3]
    return float((rec @ np.array([0.2126, 0.7152, 0.0722], dtype=np.float32)).mean())


def provar_material_do_cliente(cena):
    """Principled sobrando DESLIGADO + Emission ligado ao Output: a chave tem de cair no Emission."""
    mat = bpy.data.materials.new("teste.tela_cliente")
    mat.use_nodes = True
    nt = mat.node_tree
    # O Principled de fabrica fica no material, sem link (e o 'sobrando').
    for link in list(nt.links):
        nt.links.remove(link)
    emissao = nt.nodes.new("ShaderNodeEmission")
    emissao.inputs["Strength"].default_value = 0.0
    saida = next(n for n in nt.nodes if n.type == "OUTPUT_MATERIAL")
    nt.links.new(emissao.outputs["Emission"], saida.inputs["Surface"])
    malha = bpy.data.meshes.new("teste.tela_cliente")
    malha.from_pydata([(0, 0, 0), (0.1, 0, 0), (0.1, 0, 0.07), (0, 0, 0.07)], [], [(0, 1, 2, 3)])
    malha.materials.append(mat)
    obj = bpy.data.objects.new("teste.tela_cliente", malha)
    cena.collection.objects.link(obj)
    obj.location = (5, 5, -5)  # fora de qualquer camera
    mod_u1.animar_tela({"tela": obj, "materiais": {}}, 40, 60, 90)
    caminhos = [fc.data_path for fc in nt.animation_data.action.fcurves]
    assert caminhos, "material do cliente: nenhuma chave gravada"
    assert all('nodes["%s"]' % emissao.name in c for c in caminhos), "chave caiu fora do Emission ligado: %s" % caminhos
    assert not any("Principled" in c for c in caminhos), "chave caiu no Principled desligado: %s" % caminhos
    assert emissao.inputs["Strength"].default_value == 4.0
    print("[teste_u1] material do cliente: chave em %s (Principled sobrando ignorado)" % caminhos[0])
    # Mesmo furo em animar_botao/animar_ligar: o objeto 'led' do cliente.
    mod_u1.animar_ligar({"botao": None, "led": obj, "materiais": {}}, 5, 25)
    assert not any("Principled" in fc.data_path for fc in nt.animation_data.action.fcurves)
    bpy.data.objects.remove(obj, do_unlink=True)
    bpy.data.meshes.remove(malha)
    bpy.data.materials.remove(mat)


def parte_modulo():
    cena = cena_limpa()
    raiz_col = bpy.data.collections.new("ANUNCIO")
    cena.collection.children.link(raiz_col)
    objs = mod_u1.construir_u1(cena, raiz_col, {})
    # Idempotencia: construir de novo nao pode duplicar nada.
    n1 = len(bpy.data.objects)
    n_luzes = len(bpy.data.lights)
    objs = mod_u1.construir_u1(cena, raiz_col, {})
    n2 = len(bpy.data.objects)
    assert n1 == n2, "construir_u1 duplicou objetos: %d -> %d" % (n1, n2)
    assert n_luzes == len(bpy.data.lights), "construir_u1 vazou dados de luz: %d -> %d" % (n_luzes, len(bpy.data.lights))
    assert bpy.data.collections.get("u1.001") is None
    print("[teste_u1] idempotente: %d objetos, %d luzes nas duas rodadas" % (n2, n_luzes))
    # O envelope medido tem de bater com a ficha: foi assim que a primeira
    # versao entregou 0,545 de profundidade dizendo 0,499.
    for medido, nominal, eixo in zip(objs["dimensoes"], objs["dimensoes_nominais"], "XYZ"):
        assert abs(medido - nominal) < 0.002, "envelope %s = %.4f, nominal %.4f" % (eixo, medido, nominal)
    mn, mx = objs["envelope"]
    print("[teste_u1] envelope medido: %.4f x %.4f x %.4f  (y %.4f..%.4f, z %.4f..%.4f)" % (
        objs["dimensoes"][0], objs["dimensoes"][1], objs["dimensoes"][2], mn.y, mx.y, mn.z, mx.z))
    # Imagens da tela empacotadas no .blend (o cliente nao tem a nossa /tmp).
    for nome in ("u1.tela_boot", "u1.tela_ui"):
        img = bpy.data.images[nome]
        assert img.packed_file is not None, "%s nao empacotada (%s)" % (nome, img.filepath)
        print("[teste_u1] %s empacotada: %d bytes" % (nome, img.packed_file.size))
    # Luzes das fitas: existem, escondidas e a 0 W antes de animar_ligar.
    assert len(objs["luzes_led"]) == 2
    assert all(l.hide_render and l.data.energy == 0.0 for l in objs["luzes_led"])

    estudio(cena)
    cam, alvo = camera(cena)
    provar_material_do_cliente(cena)

    # Liga 5..25 (fundo do curso em 15; fitas, luzes e standby 15..21);
    # tela: boot 40..60, UI de 60 em diante.
    mod_u1.animar_ligar(objs, 5, 25)
    mod_u1.animar_tela(objs, 40, 60, 90)

    L, P, A = objs["dimensoes"]
    tela = objs["posicao_tela"]["centro"]
    tomada = objs["posicao_tomada"]["ponto"]
    botao = objs["posicao_botao"]["centro"]
    cab = objs["cabecotes"][0].matrix_world.translation

    # Traseira 3/4 alta: acrilico, boca do aro e fitas pelo topo. Quadro 4
    # (tudo apagado) e 18 (meio da rampa de luz: fitas a ~2, luzes a ~12 W).
    tras34 = ((0.95, 1.05, 0.80), (0.05, 0.05, 0.36), 45)
    antes = foto(cena, cam, alvo, tras34[0], tras34[1], tras34[2], 4, "previa_u1_ligando_antes.png")
    ligando = foto(cena, cam, alvo, tras34[0], tras34[1], tras34[2], 18, "previa_u1_ligando.png")
    # Janela do acrilico nesse enquadramento (medido no PNG: x 300..520,
    # y 330..690 de 540x960). A primeira versao media 0,30..0,75 e caia no
    # casco branco - 0,709 de media com a camara apagada.
    l_antes = luminancia_media(antes, 0.57, 0.95, 0.35, 0.72)
    l_ligando = luminancia_media(ligando, 0.57, 0.95, 0.35, 0.72)
    print("[teste_u1] luminancia da janela traseira: antes %.3f -> ligando %.3f (x%.2f)" % (l_antes, l_ligando, l_ligando / max(l_antes, 1e-6)))
    assert l_ligando > l_antes * 1.3, "ligar nao e evento de luz: janela %.3f -> %.3f" % (l_antes, l_ligando)

    # Quadro 30: ligado (fitas, luzes, tela em standby), boot ainda nao.
    frente = foto(cena, cam, alvo, (-1.45, -1.95, 0.95), (0.0, 0.0, 0.34), 60, 30, "previa_u1_frente.png")
    meio_tras = (tomada + botao) / 2
    foto(cena, cam, alvo, (0.75, 0.95, 0.42), (meio_tras.x - 0.10, meio_tras.y, meio_tras.z + 0.06), 60, 30, "previa_u1_tras.png")
    foto(cena, cam, alvo, (tela.x + 0.08, tela.y - 0.30, tela.z + 0.05), tela, 85, 75, "previa_u1_tela.png")
    # Meio do boot (quadro 50): imagem de boot acesa e barra pela metade.
    foto(cena, cam, alvo, (tela.x + 0.08, tela.y - 0.30, tela.z + 0.05), tela, 85, 50, "previa_u1_tela_boot.png")
    # Camera por dentro do vao do topo: mais baixo que isso ela entra no casco.
    foto(cena, cam, alvo, (cab.x - 0.12, cab.y - 0.20, cab.z + 0.32), (cab.x, cab.y, cab.z + 0.01), 85, 75, "previa_u1_cabecote.png")


# ---------------------------------------------------------------------------
# Parte de cena (rodada 3): o U1 no ambiente do anuncio, medido no pixel
# ---------------------------------------------------------------------------

def ler_png(caminho):
    """PNG -> array (altura, largura, 3) em sRGB 0..1, linha 0 no TOPO."""
    import numpy as np
    img = bpy.data.images.load(caminho)
    # Non-Color: senao o Blender linearizaria os bytes sRGB e o nivel medido
    # nao seria o do arquivo (os criterios sao em niveis de 8 bits).
    img.colorspace_settings.name = "Non-Color"
    w, h = img.size
    px = np.empty(w * h * 4, dtype=np.float32)
    img.pixels.foreach_get(px)
    bpy.data.images.remove(img)
    return px.reshape(h, w, 4)[::-1, :, :3]


def lum255(a):
    return (0.2126 * a[..., 0] + 0.7152 * a[..., 1] + 0.0722 * a[..., 2]) * 255.0


def projetar(cena, cam, ponto):
    """Ponto do mundo -> (coluna, linha) no PNG, linha 0 no topo; None se atras da camera."""
    from bpy_extras.object_utils import world_to_camera_view
    v = world_to_camera_view(cena, cam, Vector(ponto))
    if v.z <= 0.0:
        return None
    return v.x * cena.render.resolution_x, (1.0 - v.y) * cena.render.resolution_y


def enquadrar(pos_cam, sujeito, lente, fx, fy):
    """Mesma conta de mod_coreografia._enquadrar: alvo que poe o sujeito na
    fracao (fx, fy) do quadro 9:16 (sensor de 36 mm no lado maior). Copiada
    para o teste do modulo nao carregar a coreografia inteira."""
    pos_cam, sujeito = Vector(pos_cam), Vector(sujeito)
    d = sujeito - pos_cam
    dist = d.length
    d.normalize()
    direita = d.cross(Vector((0, 0, 1)))
    if direita.length < 1e-6:
        direita = Vector((1, 0, 0))
    direita.normalize()
    cima = direita.cross(d).normalized()
    meia_altura = dist * 18.0 / lente
    meia_largura = meia_altura * 9.0 / 16.0
    return sujeito - direita * ((fx - 0.5) * 2.0 * meia_largura) + cima * ((fy - 0.5) * 2.0 * meia_altura)


def _apontar_rig(amb, cam_pos, offset):
    """Rig de luz = azimute da camera + offset, como a coreografia faz na orbita e nas fotos."""
    az = math.degrees(math.atan2(cam_pos[1], cam_pos[0]))
    amb["rig"].rotation_euler = (0.0, 0.0, math.radians(az + offset))


def _luzes_u1(objs, energia, forca_fitas):
    """Area lights da camara e fitas num valor fixo (as chaves de animar_ligar
    saem): cada foto do beat 5 tem o seu valor, em corte."""
    for luz in objs["luzes_led"]:
        luz.data.animation_data_clear()
        luz.data.energy = energia
        luz.hide_render = energia <= 0.0
    nt = objs["materiais"]["led"].node_tree
    nt.animation_data_clear()
    mod_u1._socket_forca_emissao(nt).default_value = forca_fitas


def maior_faixa(valores, limiar):
    """Maior sequencia de True consecutivos (amostras) em 'valores' >= limiar; None quebra."""
    maior = atual = 0
    for v in valores:
        if v is not None and v >= limiar:
            atual += 1
            maior = max(maior, atual)
        else:
            atual = 0
    return maior


def medir_tela(cena, cam, objs, arquivo):
    """L (0..255) do FUNDO da UI em toda a area da tela: para cada ponto de uma
    grade no vidro, se o pixel correspondente do PNG da UI e fundo (max canal
    < 20 de 255, com 4 px de folga em volta) ou cai na moldura (fora da area
    ativa), le o L do render no pixel projetado. Devolve o maximo por terco
    (cima, meio, baixo) e a media de cada terco."""
    import numpy as np
    img = ler_png(arquivo)
    ui = ler_png(mod_u1.PADROES["imagem_ui"])
    fundo_png = ui.max(axis=2) < (20.0 / 255.0)
    # Erosao de 6 px: um ponto da grade na borda de um icone nao pode contar
    # como fundo. MEDIDO com o especular a zero: com 4 px sobravam 8 pontos
    # de L 12-26, todos na linha 5 px acima do texto da barra de estado -
    # halo da interpolacao cubica da textura mais o filtro do render, nao
    # reflexo (o reflexo do world muda com o especular; o halo nao).
    k = 6
    ero = fundo_png.copy()
    for dy in range(-k, k + 1):
        for dx in range(-k, k + 1):
            ero &= np.roll(np.roll(fundo_png, dy, axis=0), dx, axis=1)
    tela = objs["tela"]
    ex, ey = 0.104 / 0.0744, 0.070 / 0.0496
    tercos = {0: [], 1: [], 2: []}
    h, w = img.shape[:2]
    for i in range(70):
        v = -0.035 + 0.0025 + (0.070 - 0.005) * i / 69.0
        for j in range(104):
            u = -0.052 + 0.0025 + (0.104 - 0.005) * j / 103.0
            # Vertices locais da face da frente da malha da tela (y = -0,002).
            mundo = tela.matrix_world @ Vector((u, -0.002, v))
            uv = (u / 0.104 + 0.5, v / 0.070 + 0.5)
            px = (uv[0] * ex + 0.5 - 0.5 * ex) * 480.0
            py = (1.0 - (uv[1] * ey + 0.5 - 0.5 * ey)) * 320.0
            if 0 <= px < 480 and 0 <= py < 320:
                if not ero[int(py), int(px)]:
                    continue
            pr = projetar(cena, cam, mundo)
            if pr is None:
                continue
            cx, cy = int(pr[0]), int(pr[1])
            if not (1 <= cx < w - 1 and 1 <= cy < h - 1):
                continue
            L = lum255(img[cy - 1:cy + 2, cx - 1:cx + 2]).mean()
            tercos[min(2, int(3 * (0.5 - v / 0.070)))].append(L)
    res = {}
    for t, nome in ((0, "cima"), (1, "meio"), (2, "baixo")):
        vals = np.array(tercos[t])
        res[nome] = (float(vals.max()), float(vals.mean()), len(vals))
    return res


def medir_haste(cena, cam, img, objs, haste, raio_px=3):
    """L maximo numa janela de (2r+1)^2 px ao longo do eixo da haste, uma
    amostra por milimetro; None fora do quadro ou onde outra peca cobre a
    haste (ray cast da camera: o carro X e, na foto C, as FITAS de LED, que
    ficam entre a camera e a haste e a 1,2 de emissao dao >= 250 sozinhas -
    26 mm que nao mudavam com nenhuma rugosidade)."""
    h, w = img.shape[:2]
    comp = haste.dimensions.x
    origem = cam.matrix_world.translation
    deps = bpy.context.evaluated_depsgraph_get()
    vals = []
    for i in range(int(comp * 1000)):
        x = -comp / 2.0 + i / 1000.0
        # Ponto no topo do cilindro (raio 6 mm), que e o que a camera de cima ve.
        mundo = haste.matrix_world @ Vector((x, 0.0, 0.006))
        toque = cena.ray_cast(deps, origem, (mundo - origem).normalized())
        if not toque[0] or toque[4] is None or toque[4].name != haste.name:
            vals.append(None)
            continue
        pr = projetar(cena, cam, mundo)
        if pr is None:
            vals.append(None)
            continue
        cx, cy = int(pr[0]), int(pr[1])
        if not (raio_px <= cx < w - raio_px and raio_px <= cy < h - raio_px):
            vals.append(None)
            continue
        vals.append(float(lum255(img[cy - raio_px:cy + raio_px + 1, cx - raio_px:cx + raio_px + 1]).max()))
    return vals


def medir_face_lateral(cena, cam, img, objs, z0, z1):
    """Amplitude horizontal (max - min das medias de coluna, em niveis) de L na
    face lateral +X entre z0 e z1, com 3 cm de folga nas bordas da frente e de tras."""
    import numpy as np
    mn, mx = objs["envelope"]
    P = mod_u1.PROFUNDIDADE - mod_u1.SALIENCIA_FRENTE - mod_u1.SALIENCIA_TRAS
    cantos = [projetar(cena, cam, (mx.x, y, z)) for y in (-P / 2 + 0.03, P / 2 - 0.03) for z in (z0, z1)]
    xs = [c[0] for c in cantos]
    ys = [c[1] for c in cantos]
    x0, x1 = int(min(xs)), int(max(xs))
    y0, y1 = int(min(ys)), int(max(ys))
    face = lum255(img[y0:y1, x0:x1])
    colunas = face.mean(axis=0)
    return float(colunas.max() - colunas.min()), float(face.mean()), (x0, x1, y0, y1)


def parte_cena():
    cena = cena_limpa()
    raiz_col = bpy.data.collections.new("ANUNCIO")
    cena.collection.children.link(raiz_col)
    amb = mod_ambiente.construir_ambiente(cena, raiz_col)
    objs = mod_u1.construir_u1(cena, raiz_col, {})
    cam, alvo = camera(cena)
    offset = mod_ambiente.OFFSET_RIM_ATRAS
    mod_u1.animar_ligar(objs, 5, 25)
    mod_u1.animar_tela(objs, 40, 60, 90)
    tela = objs["posicao_tela"]["centro"]
    normal = objs["posicao_tela"]["normal"]
    falhas = []

    def criterio(nome, ok, detalhe):
        print("[teste_u1] %s: %s - %s" % ("OK   " if ok else "FALHA", nome, detalhe))
        if not ok:
            falhas.append(nome)

    # --- 1. q359: a tela acesa a 0,26 m, com o world do ambiente ------------
    # Mesma camera do fim do dolly do beat 4 (mod_coreografia._beat4): tela +
    # normal*0,26 + (0,02, 0, 0,015), alvo na tela, 35 mm; rig a azimute + 90.
    pos = tela + normal * 0.26 + Vector((0.02, 0.0, 0.015))
    _apontar_rig(amb, pos, offset)
    arq = foto(cena, cam, alvo, pos, tela, 35, 75, "previa_u1_cena_tela.png")
    bpy.context.view_layer.update()
    m = medir_tela(cena, cam, objs, arq)
    print("[teste_u1] fundo da UI (L de 255): cima max %.1f media %.1f (%d pontos), meio max %.1f media %.1f, baixo max %.1f media %.1f"
          % (m["cima"][0], m["cima"][1], m["cima"][2], m["meio"][0], m["meio"][1], m["baixo"][0], m["baixo"][1]))
    pior = max(v[0] for v in m.values())
    criterio("preto da tela ligada e preto (L < 15 em toda a tela)", pior < 15.0, "pior terco L max %.1f" % pior)

    # --- 4. lateral (q185): a key do ambiente contra key menor ----------------
    # Camera na normal da face +X (a do q185: coluna da tomada a direita), a
    # 2 m, 50 mm; rig a azimute + 90 (rim atras), como na orbita. Tudo apagado
    # (quadro 4): no q185 o U1 ainda nao ligou.
    L_, P_, A_ = objs["dimensoes"]
    pos_lat = Vector((L_ / 2 + 2.0, 0.0, 0.42))
    _apontar_rig(amb, pos_lat, offset)
    key = amb["luzes"]["key"].data
    key_pos = Vector(amb["luzes"]["key"].location)
    alvo_luz = Vector(amb["params"]["alvo_luzes"])
    direcao_key = (key_pos - alvo_luz).normalized()
    dist_key = (key_pos - alvo_luz).length
    variantes = [
        ("key2m", None, None),
        ("key1m", (1.0, 1.0), 2.5),
        ("softbox", (1.0, 2.0), 2.5),
    ]
    amplitudes = {}
    for rotulo, tam, dist in variantes:
        if tam is not None:
            key.size, key.size_y = tam
            amb["luzes"]["key"].location = alvo_luz + direcao_key * dist
        else:
            key.size, key.size_y = amb["params"]["luzes"]["key"]["tam"]
            amb["luzes"]["key"].location = key_pos
        arq = foto(cena, cam, alvo, pos_lat, (L_ / 2, 0.0, 0.40), 50, 4, "previa_u1_cena_lateral_%s.png" % rotulo)
        bpy.context.view_layer.update()
        img = ler_png(arq)
        amp_alto, media_alto, rec = medir_face_lateral(cena, cam, img, objs, 0.57, 0.66)
        amp_disco, media_disco, _ = medir_face_lateral(cena, cam, img, objs, 0.30, 0.42)
        amplitudes[rotulo] = amp_alto
        print("[teste_u1] lateral %s: key %.0fx%.0f cm a %.2f m; faixa alta (z 0,57-0,66, px x %d-%d y %d-%d) L media %.1f amplitude horizontal %.1f niveis; faixa do disco (z 0,30-0,42) media %.1f amplitude %.1f"
              % (rotulo, key.size * 100, key.size_y * 100, (Vector(amb["luzes"]["key"].location) - alvo_luz).length,
                 rec[0], rec[1], rec[2], rec[3], media_alto, amp_alto, media_disco, amp_disco))
    key.size, key.size_y = amb["params"]["luzes"]["key"]["tam"]
    amb["luzes"]["key"].location = key_pos
    print("[teste_u1] lateral: amplitude >= 15 niveis? " + ", ".join("%s %s (%.1f)" % (k, "sim" if v >= 15 else "nao", v) for k, v in amplitudes.items()))

    # --- 3. foto A (q389): cabecotes estacionados, produto no canto inferior direito
    cabs = objs["cabecotes"]
    cena.frame_set(60)
    bpy.context.view_layer.update()
    suj_a = (cabs[1].matrix_world.translation + cabs[2].matrix_world.translation) / 2.0 + Vector((0, 0, 0.02))
    pos_a = suj_a + Vector((-0.22, -0.38, 0.30))
    _luzes_u1(objs, 60.0, 1.2)
    _apontar_rig(amb, pos_a, offset + 45.0)
    amb["luzes"]["key"].data.energy = 300.0
    amb["luzes"]["rim"].data.energy = 250.0
    arq = foto(cena, cam, alvo, pos_a, enquadrar(pos_a, suj_a, 60.0, 0.68, 0.74), 60, 60, "previa_u1_cena_foto_a.png")
    bpy.context.view_layer.update()
    img = ler_png(arq)
    # O labio: a faixa da parede do bolso do casco (branca) entre o topo da
    # parede de tras da camara e a face de baixo do aro - a camera da foto A
    # olha por cima da frente para a PAREDE DE TRAS (diagnostico da rodada 3:
    # a barra some escondendo u1.corpo e nao muda escondendo as fitas).
    # Amostra a linha do meio dessa faixa (z 0,694, y da parede do bolso) da
    # esquerda a direita e mede L: com o labio branco ela e >= 235 de ponta a
    # ponta; com a parede subindo ate o aro o pixel projetado cai na parede.
    P_c = mod_u1.PROFUNDIDADE - mod_u1.SALIENCIA_FRENTE - mod_u1.SALIENCIA_TRAS
    y_labio = (P_c - 0.11) / 2.0 - 0.0005
    z_labio = 0.694
    vals = []
    for i in range(400):
        x = -0.20 + 0.40 * i / 399.0
        pr = projetar(cena, cam, (x, y_labio, z_labio))
        if pr is None or not (2 <= pr[0] < img.shape[1] - 2 and 2 <= pr[1] < img.shape[0] - 2):
            vals.append(None)
            continue
        cx, cy = int(pr[0]), int(pr[1])
        vals.append(float(lum255(img[cy - 2:cy + 3, cx - 2:cx + 3]).mean()))
    dentro = [v for v in vals if v is not None]
    faixa = maior_faixa(vals, 235.0)
    print("[teste_u1] foto A, linha do labio (y %.3f z %.3f): %d amostras no quadro, L media %.1f, maior faixa >= 235: %d mm"
          % (y_labio, z_labio, len(dentro), sum(dentro) / max(len(dentro), 1), faixa))
    criterio("sem barra branca do labio na foto A", faixa < 40, "maior faixa continua >= 235 na linha do labio: %d mm (< 40)" % faixa)

    # --- 2. foto C (q445): mesa de cima, luzes da camara a 10 W --------------
    mesa = objs["mesa"].matrix_world.translation.copy()
    pos_c = mesa + Vector((0.30, -0.30, 1.20))
    _luzes_u1(objs, 10.0, 1.2)
    _apontar_rig(amb, pos_c, offset + 70.0)
    amb["luzes"]["key"].data.energy = 110.0
    amb["luzes"]["rim"].data.energy = 150.0
    arq = foto(cena, cam, alvo, pos_c, enquadrar(pos_c, mesa, 50.0, 0.70, 0.74), 50, 60, "previa_u1_cena_foto_c.png")
    bpy.context.view_layer.update()
    img = ler_png(arq)
    pior_faixa = 0
    for haste in objs["hastes"]:
        vals = medir_haste(cena, cam, img, objs, haste)
        dentro = [v for v in vals if v is not None]
        faixa = maior_faixa(vals, 250.0)
        n250 = sum(1 for v in dentro if v >= 250.0)
        pior_faixa = max(pior_faixa, faixa)
        print("[teste_u1] foto C, %s: %d mm no quadro, L max %.1f, %d mm >= 250, maior faixa continua >= 250: %d mm"
              % (haste.name, len(dentro), max(dentro) if dentro else 0.0, n250, faixa))
    criterio("haste nao le como tubo fluorescente na foto C", pior_faixa < 10,
             "maior faixa continua >= 250 ao longo das hastes: %d mm (< 10)" % pior_faixa)

    if falhas:
        raise AssertionError("parte de cena: %d criterio(s) falhou: %s" % (len(falhas), ", ".join(falhas)))
    print("[teste_u1] parte de cena: todos os criterios passaram")


if PARTE in ("", "modulo"):
    parte_modulo()
if PARTE in ("", "cena"):
    parte_cena()
