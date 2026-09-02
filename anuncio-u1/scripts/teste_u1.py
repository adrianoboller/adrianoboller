# Teste do modulo U1: cena vazia, so o U1 substituto, camera e luz simples.
# Renderiza 4 quadros em 540x960 e o autor abre cada PNG e olha:
#   previa_u1_frente.png   - frente 3/4, tela desligada
#   previa_u1_tras.png     - traseira com tomada e botao
#   previa_u1_tela.png     - close da tela com a UI ligada
#   previa_u1_cabecote.png - close de um cabecote
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


def main():
    cena = cena_limpa()
    raiz_col = bpy.data.collections.new("ANUNCIO")
    cena.collection.children.link(raiz_col)
    objs = mod_u1.construir_u1(cena, raiz_col, {})
    # Idempotencia: construir de novo nao pode duplicar nada.
    n1 = len(bpy.data.objects)
    objs = mod_u1.construir_u1(cena, raiz_col, {})
    n2 = len(bpy.data.objects)
    assert n1 == n2, "construir_u1 duplicou objetos: %d -> %d" % (n1, n2)
    assert bpy.data.collections.get("u1.001") is None
    print("[teste_u1] idempotente: %d objetos nas duas rodadas" % n2)
    # O envelope medido tem de bater com a ficha: foi assim que a primeira
    # versao entregou 0,545 de profundidade dizendo 0,499.
    for medido, nominal, eixo in zip(objs["dimensoes"], objs["dimensoes_nominais"], "XYZ"):
        assert abs(medido - nominal) < 0.002, "envelope %s = %.4f, nominal %.4f" % (eixo, medido, nominal)
    mn, mx = objs["envelope"]
    print("[teste_u1] envelope medido: %.4f x %.4f x %.4f  (y %.4f..%.4f, z %.4f..%.4f)" % (
        objs["dimensoes"][0], objs["dimensoes"][1], objs["dimensoes"][2], mn.y, mx.y, mn.z, mx.z))
    estudio(cena)
    cam, alvo = camera(cena)

    # Botao 5..25 (LEDs da camara acendem ate 25); tela: boot 40..60, UI de 60 em diante.
    mod_u1.animar_botao(objs, 5, 25)
    mod_u1.animar_tela(objs, 40, 60, 90)

    L, P, A = objs["dimensoes"]
    tela = objs["posicao_tela"]["centro"]
    tomada = objs["posicao_tomada"]["ponto"]
    botao = objs["posicao_botao"]["centro"]
    cab = objs["cabecotes"][0].matrix_world.translation

    # Quadro 30: LEDs acesos, tela ainda desligada.
    foto(cena, cam, alvo, (-1.45, -1.95, 0.95), (0.0, 0.0, 0.34), 60, 30, "previa_u1_frente.png")
    meio_tras = (tomada + botao) / 2
    foto(cena, cam, alvo, (0.75, 0.95, 0.42), (meio_tras.x - 0.10, meio_tras.y, meio_tras.z + 0.06), 60, 30, "previa_u1_tras.png")
    foto(cena, cam, alvo, (tela.x + 0.08, tela.y - 0.30, tela.z + 0.05), tela, 85, 75, "previa_u1_tela.png")
    # Meio do boot (quadro 50): imagem de boot acesa e barra pela metade.
    foto(cena, cam, alvo, (tela.x + 0.08, tela.y - 0.30, tela.z + 0.05), tela, 85, 50, "previa_u1_tela_boot.png")
    # Camera por dentro do vao do topo: mais baixo que isso ela entra no casco.
    foto(cena, cam, alvo, (cab.x - 0.12, cab.y - 0.20, cab.z + 0.32), (cab.x, cab.y, cab.z + 0.01), 85, 75, "previa_u1_cabecote.png")

main()
