# Teste do modulo U1: cena vazia, so o U1 substituto, camera e luz simples.
# Renderiza em 540x960 e o autor abre cada PNG e olha:
#   previa_u1_frente.png        - frente 3/4, tela em standby (ja ligou)
#   previa_u1_tras.png          - traseira com tomada e botao, camara acesa
#   previa_u1_tela.png          - close da tela com a UI ligada
#   previa_u1_tela_boot.png     - meio do boot
#   previa_u1_cabecote.png      - close de um cabecote
#   previa_u1_ligando_antes.png - traseira 3/4 antes do botao (tudo apagado)
#   previa_u1_ligando.png       - traseira 3/4 no meio de animar_ligar
# Os *_rodada1.png sao os da rodada anterior, para comparar.
#
# Provas numericas (asserts): idempotencia, envelope, imagens empacotadas,
# luzes escondidas antes de ligar, material do cliente com Principled
# desligado sobrando + Emission ligado -> a chave cai no Emission, e a
# luminancia da janela traseira sobe entre 'antes' e 'ligando'.
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
importlib.reload(mod_u1)

LARGURA, ALTURA_IMG = 540, 960
AMOSTRAS = 16


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
    cena.render.filepath = os.path.join(RAIZ, "saida", arquivo)
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


def main():
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

main()
