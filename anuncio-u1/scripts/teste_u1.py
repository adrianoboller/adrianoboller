# Teste do modulo U1 (impressora do cliente, Meshy limpa): cena vazia, SEM
# chao, World cinza-escuro liso, camera e tres area lights. Renderiza em
# 540x960/16 e o autor abre cada PNG e olha:
#   previa_u1_frente.png    - frente 3/4, tela APAGADA (quadro 1, antes de ligar)
#   previa_u1_tras.png      - traseira: coluna com botao e tomada
#   previa_u1_tela.png      - close da tela com a UI acesa (quadro 75)
#   previa_u1_topo.png      - close dos tubos e do topo (cabecotes, mesa)
#   previa_u1_ligando.png   - traseira 3/4 alta no meio de animar_ligar (q18)
#   previa_u1_porta_com/sem.png, previa_u1_ligando_antes/depois.png - pares
#                             em meia resolucao das provas do vidro e da luz
# Os previa_u1_*_rodada*.png sao do substituto, para comparar.
#
# Provas numericas (asserts), antes dos renders:
# - idempotencia: construir_u1 duas vezes -> mesma contagem de objetos,
#   malhas, materiais e imagens (o importador glTF cria material e imagens
#   novos a cada importacao; o modulo tem de limpar os orfaos);
# - envelope medido = 0,584 x 0,499 x 0,730 (+-3 mm: tela e aro 2,2 mm a
#   frente da face);
# - materiais da Meshy nas pecas (u1.meshy com as tres imagens, empacotadas)
#   e vidro na porta (Transmission 1, dithered, refracao raytraced);
# - imagens da tela empacotadas; luzes das fitas escondidas e a 0 W;
# - material do cliente (Principled sobrando + Emission ligado): a chave cai
#   no Emission;
# - a janela do vidro da porta mostra o interior: o padrao claro/escuro numa
#   grade 4x4 correlaciona (> 0,6) com o render sem a porta, e a media fica
#   acima de 35% dela (vidro opaco dava razao 0,23 e correlacao ~0);
# - ligar e evento de luz: so com World e fitas (estudio apagado), a janela
#   traseira clareia >= 1,3x entre q4 e q18.
#
# PARTE=provas so os asserts; PARTE=render so os renders; sem PARTE, os dois.
# FOTOS=frente,tras,tela,topo,ligando escolhe os renders (padrao: todos; a
# cena completa custa ~1 min por quadro no llvmpipe, e cada chamada tem de
# caber em 10 min). PASTA=... troca a pasta de saida (padrao saida/).
#
# Uso: bash scripts/previa.sh scripts/teste_u1.py

import os
import sys

import bpy
from mathutils import Vector

RAIZ = "/home/user/adrianoboller/anuncio-u1"
sys.path.insert(0, os.path.join(RAIZ, "scripts"))

import importlib
import mod_u1
importlib.reload(mod_u1)

LARGURA, ALTURA_IMG = int(os.environ.get("LARG", "540")), int(os.environ.get("ALT", "960"))
AMOSTRAS = int(os.environ.get("AMOSTRAS", "16"))
SAIDA = os.environ.get("PASTA") or os.path.join(RAIZ, "saida")
PARTE = os.environ.get("PARTE", "")
FOTOS = [f for f in os.environ.get("FOTOS", "frente,tras,tela,topo,ligando").split(",") if f]


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
    """World cinza-escuro LISO e tres areas; nenhum chao (revisao 2)."""
    mundo = bpy.data.worlds.new("mundo")
    cena.world = mundo
    mundo.use_nodes = True
    fundo = mundo.node_tree.nodes["Background"]
    fundo.inputs["Color"].default_value = (0.045, 0.045, 0.05, 1.0)
    fundo.inputs["Strength"].default_value = 1.0
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


def _janela(arquivo, x0, x1, y0, y1):
    import numpy as np
    img = bpy.data.images.load(arquivo)
    w, h = img.size
    px = np.array(img.pixels[:], dtype=np.float32).reshape(h, w, 4)[::-1]
    bpy.data.images.remove(img)
    rec = px[int(h * y0):int(h * y1), int(w * x0):int(w * x1), :3]
    return rec @ np.array([0.2126, 0.7152, 0.0722], dtype=np.float32)


def luminancia_media(arquivo, x0, x1, y0, y1):
    """Media da luminancia (0..1, sRGB do PNG) num retangulo em fracoes do quadro, y de cima."""
    return float(_janela(arquivo, x0, x1, y0, y1).mean())


def correlacao_janela(arq_a, arq_b, x0, x1, y0, y1, n=4):
    """Correlacao (Pearson) entre as medias de uma grade n x n da mesma
    janela em dois PNGs. E a prova de que o vidro MOSTRA o que esta atras: a
    media sozinha nao separa 'vidro com tinta' de 'painel opaco escuro', mas
    o padrao claro/escuro do interior (paredes claras em cima, mesa escura
    embaixo) so aparece atraves do vidro se a refracao chega la."""
    import numpy as np
    a, b = _janela(arq_a, x0, x1, y0, y1), _janela(arq_b, x0, x1, y0, y1)
    h, w = a.shape

    def celulas(m):
        return np.array([m[i * h // n:(i + 1) * h // n, j * w // n:(j + 1) * w // n].mean() for i in range(n) for j in range(n)])

    ca, cb = celulas(a), celulas(b)
    if ca.std() < 1e-6 or cb.std() < 1e-6:
        return 0.0
    return float(np.corrcoef(ca, cb)[0, 1])


def projetar(cena, cam, ponto):
    """Ponto do mundo -> (fracao x, fracao y do quadro, y de cima); None se atras."""
    from bpy_extras.object_utils import world_to_camera_view
    v = world_to_camera_view(cena, cam, Vector(ponto))
    if v.z <= 0.0:
        return None
    return v.x, 1.0 - v.y


def provar_material_do_cliente(cena):
    """Principled sobrando DESLIGADO + Emission ligado ao Output: a chave tem de cair no Emission."""
    mat = bpy.data.materials.new("teste.tela_cliente")
    mat.use_nodes = True
    nt = mat.node_tree
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
    caminhos = [fc.data_path for fc in mod_u1.fcurves_de(nt.animation_data)]
    assert caminhos, "material do cliente: nenhuma chave gravada"
    assert all('nodes["%s"]' % emissao.name in c for c in caminhos), "chave caiu fora do Emission ligado: %s" % caminhos
    assert emissao.inputs["Strength"].default_value == 4.0
    mod_u1.animar_ligar({"botao": None, "led": obj, "materiais": {}}, 5, 25)
    assert not any("Principled" in fc.data_path for fc in mod_u1.fcurves_de(nt.animation_data))
    print("[teste_u1] material do cliente: chave em %s (Principled sobrando ignorado)" % caminhos[0])
    bpy.data.objects.remove(obj, do_unlink=True)
    bpy.data.meshes.remove(malha)
    bpy.data.materials.remove(mat)


def contagens():
    return {"objetos": len(bpy.data.objects), "malhas": len(bpy.data.meshes),
            "materiais": len(bpy.data.materials), "imagens": len(bpy.data.images), "luzes": len(bpy.data.lights)}


def provas(cena, raiz_col):
    objs = mod_u1.construir_u1(cena, raiz_col, {})
    c1 = contagens()
    objs = mod_u1.construir_u1(cena, raiz_col, {})
    c2 = contagens()
    assert c1 == c2, "construir_u1 nao e idempotente: %s -> %s" % (c1, c2)
    assert bpy.data.collections.get("u1.001") is None
    assert all(o.name.startswith("u1.") for o in objs["colecao"].all_objects), \
        "objeto fora do padrao u1.<peca>: %s" % [o.name for o in objs["colecao"].all_objects if not o.name.startswith("u1.")]
    print("[teste_u1] idempotente: %s nas duas rodadas" % c2)
    tris = sum(len(o.data.polygons) for o in objs["colecao"].all_objects if o.type == "MESH")
    print("[teste_u1] malha: %s; %d objetos na colecao, %d triangulos" % (objs["arquivo"], len(objs["colecao"].all_objects), tris))
    for medido, nominal, eixo in zip(objs["dimensoes"], objs["dimensoes_nominais"], "XYZ"):
        assert abs(medido - nominal) < 0.003, "envelope %s = %.4f, nominal %.4f" % (eixo, medido, nominal)
    mn, mx = objs["envelope"]
    print("[teste_u1] envelope medido: %.4f x %.4f x %.4f  (x %.4f..%.4f, y %.4f..%.4f, z %.4f..%.4f)" % (
        objs["dimensoes"][0], objs["dimensoes"][1], objs["dimensoes"][2], mn.x, mx.x, mn.y, mx.y, mn.z, mx.z))
    corpo = objs["corpo"]
    dg = bpy.context.evaluated_depsgraph_get()
    ev = corpo.evaluated_get(dg)
    zc = max((ev.matrix_world @ Vector(c)).z for c in ev.bound_box)
    print("[teste_u1] corpo: %d tris, topo em z %.4f; tubos %s, bobinas %s, mesa %s, cabecotes %s" % (
        len(corpo.data.polygons), zc, [o.name for o in objs["tubos"]], [o.name for o in objs["bobinas"]],
        objs["mesa"].name, [o.name for o in objs["cabecotes"]]))
    # Materiais da Meshy: u1.meshy com as tres imagens, empacotadas e com nome fixo.
    m = corpo.data.materials[0]
    assert m is not None and m.name == "u1.meshy", "corpo sem u1.meshy: %s" % (m.name if m else None)
    imgs = {n.image.name for n in m.node_tree.nodes if n.type == "TEX_IMAGE" and n.image is not None}
    assert imgs == {"u1.meshy.cor", "u1.meshy.metal_rugosidade", "u1.meshy.normal"}, "imagens da Meshy: %s" % imgs
    for nome in sorted(imgs) + ["u1.tela_boot", "u1.tela_ui"]:
        img = bpy.data.images[nome]
        assert img.packed_file is not None, "%s nao empacotada (%s)" % (nome, img.filepath)
        print("[teste_u1] %s empacotada: %d bytes, %dx%d, %s" % (nome, img.packed_file.size, img.size[0], img.size[1], img.colorspace_settings.name))
    bsdf = next(n for n in m.node_tree.nodes if n.type == "BSDF_PRINCIPLED")
    assert bsdf.inputs["Base Color"].is_linked and bsdf.inputs["Roughness"].is_linked and bsdf.inputs["Metallic"].is_linked and bsdf.inputs["Normal"].is_linked, \
        "u1.meshy sem os quatro links (cor, rugosidade, metalico, normal)"
    for o in objs["tubos"] + objs["bobinas"] + [objs["mesa"]] + objs["cabecotes"]:
        if o.type == "MESH":
            assert o.data.materials[0].name == "u1.meshy", "%s sem u1.meshy" % o.name
    # Vidro da porta
    porta = objs["porta"]
    assert porta is not None and porta.type == "MESH"
    mv = porta.data.materials[0]
    assert mv.name == "u1.vidro"
    bv = next(n for n in mv.node_tree.nodes if n.type == "BSDF_PRINCIPLED")
    assert bv.inputs["Transmission Weight"].default_value == 1.0
    assert getattr(mv, "surface_render_method", "DITHERED") == "DITHERED"
    print("[teste_u1] porta: %d tris, u1.vidro transmission 1, %s, refracao raytraced %s" % (
        len(porta.data.polygons), getattr(mv, "surface_render_method", "?"), getattr(mv, "use_raytrace_refraction", "?")))
    # Luzes das fitas: existem, escondidas e a 0 W antes de animar_ligar.
    assert len(objs["luzes_led"]) == 2
    assert all(l.hide_render and l.data.energy == 0.0 for l in objs["luzes_led"])
    for chave in ("posicao_tela", "posicao_tomada", "posicao_botao"):
        print("[teste_u1] %s: %s" % (chave, {k: tuple(round(x, 4) for x in v) for k, v in objs[chave].items()}))
    provar_material_do_cliente(cena)
    return objs


def renders(cena, objs):
    estudio(cena)
    cam, alvo = camera(cena)
    # Liga 5..25 (fundo do curso em 15; fitas, luzes e standby 15..21);
    # tela: boot 40..60, UI de 60 em diante.
    mod_u1.animar_ligar(objs, 5, 25)
    mod_u1.animar_tela(objs, 40, 60, 90)
    tela = objs["posicao_tela"]["centro"]
    tomada = objs["posicao_tomada"]["ponto"]
    botao = objs["posicao_botao"]["centro"]

    # Frente 3/4, quadro 1: tudo apagado, tela desligada.
    if "frente" in FOTOS:
        foto(cena, cam, alvo, (-1.05, -1.35, 0.70), (0.0, 0.0, 0.36), 50, 1, "previa_u1_frente.png")
    # Vidro da porta: a media na janela da porta muda quando a porta some.
    # Mede em resolucao baixa para custar menos de um quadro.
    cam.location = (-1.05, -1.35, 0.70)
    alvo.location = (0.0, 0.0, 0.36)
    cam.data.lens = 50
    bpy.context.view_layer.update()
    px = projetar(cena, cam, objs["porta"].matrix_world.translation)
    if px is not None and "frente" in FOTOS:
        rx, ry = cena.render.resolution_x, cena.render.resolution_y
        cena.render.resolution_x, cena.render.resolution_y = rx // 2, ry // 2
        amostras = cena.eevee.taa_render_samples
        cena.eevee.taa_render_samples = 8
        com = foto(cena, cam, alvo, (-1.05, -1.35, 0.70), (0.0, 0.0, 0.36), 50, 1, "previa_u1_porta_com.png")
        objs["porta"].hide_render = True
        sem = foto(cena, cam, alvo, (-1.05, -1.35, 0.70), (0.0, 0.0, 0.36), 50, 1, "previa_u1_porta_sem.png")
        objs["porta"].hide_render = False
        cena.render.resolution_x, cena.render.resolution_y = rx, ry
        cena.eevee.taa_render_samples = amostras
        jan = (px[0] - 0.05, px[0] + 0.05, px[1] - 0.06, px[1] + 0.06)
        l_com, l_sem = luminancia_media(com, *jan), luminancia_media(sem, *jan)
        corr = correlacao_janela(com, sem, *jan)
        print("[teste_u1] janela da porta (%.2f..%.2f x %.2f..%.2f): com vidro L %.3f, sem vidro L %.3f (razao %.2f); correlacao do padrao 4x4: %.2f"
              % (jan[0], jan[1], jan[2], jan[3], l_com, l_sem, l_com / max(l_sem, 1e-6), corr))
        # Vidro com tinta 0,58 transmite ~metade; opaco preto dava razao 0,23
        # e correlacao ~0 (medido no exp_porta).
        assert l_com / max(l_sem, 1e-6) > 0.35 and corr > 0.6, \
            "a porta nao se comporta como vidro: com %.3f sem %.3f, correlacao %.2f" % (l_com, l_sem, corr)
    # Traseira: coluna com botao e tomada, quadro 1.
    meio_tras = (tomada + botao) / 2
    if "tras" in FOTOS:
        foto(cena, cam, alvo, (0.62, 0.85, 0.33), (meio_tras.x - 0.06, meio_tras.y, meio_tras.z + 0.03), 60, 1, "previa_u1_tras.png")
    # Close da tela com a UI (quadro 75).
    if "tela" in FOTOS:
        foto(cena, cam, alvo, (tela.x + 0.08, tela.y - 0.30, tela.z + 0.05), tela, 85, 75, "previa_u1_tela.png")
    # Tubos e topo (quadro 30: ligado, fitas acesas).
    if "topo" in FOTOS:
        foto(cena, cam, alvo, (-0.55, -0.75, 0.95), (0.0, 0.10, 0.52), 60, 30, "previa_u1_topo.png")
    # Ligando: traseira 3/4 alta no meio da rampa (q18).
    if "ligando" in FOTOS:
        foto(cena, cam, alvo, (0.95, 1.05, 0.80), (0.05, 0.05, 0.36), 45, 18, "previa_u1_ligando.png")
        # Ligar e evento de luz: a janela traseira (acrilico sobre o vao)
        # clareia entre q4 (tudo apagado) e q18 (fitas a ~2, luzes a ~12 W).
        # SEM as luzes do estudio: com elas a janela ja media L 0,44 apagada
        # (a key entra pelo topo aberto da Meshy e a contraluz bate na
        # traseira) e as fitas somavam 6% - o substituto tinha camara
        # fechada. So World + fitas e o que prova que as area lights das
        # fitas iluminam a camara. Baixa resolucao para custar menos de um
        # quadro.
        rx, ry = cena.render.resolution_x, cena.render.resolution_y
        cena.render.resolution_x, cena.render.resolution_y = rx // 2, ry // 2
        amostras = cena.eevee.taa_render_samples
        cena.eevee.taa_render_samples = 8
        estudio_luzes = [o for o in cena.collection.objects if o.type == "LIGHT"]
        for o in estudio_luzes:
            o.hide_render = True
        antes = foto(cena, cam, alvo, (0.95, 1.05, 0.80), (0.05, 0.05, 0.36), 45, 4, "previa_u1_ligando_antes.png")
        depois = foto(cena, cam, alvo, (0.95, 1.05, 0.80), (0.05, 0.05, 0.36), 45, 18, "previa_u1_ligando_depois.png")
        for o in estudio_luzes:
            o.hide_render = False
        cena.render.resolution_x, cena.render.resolution_y = rx, ry
        cena.eevee.taa_render_samples = amostras
        bpy.context.view_layer.update()
        pj = projetar(cena, cam, objs["painel_traseiro"].matrix_world.translation)
        if pj is not None:
            jan = (pj[0] - 0.05, pj[0] + 0.05, pj[1] - 0.04, pj[1] + 0.04)
            l_antes, l_depois = luminancia_media(antes, *jan), luminancia_media(depois, *jan)
            print("[teste_u1] janela traseira (%.2f..%.2f x %.2f..%.2f): apagado L %.3f -> ligando L %.3f (x%.2f)"
                  % (jan[0], jan[1], jan[2], jan[3], l_antes, l_depois, l_depois / max(l_antes, 1e-6)))
            assert l_depois > l_antes * 1.3, "ligar nao e evento de luz: janela %.3f -> %.3f" % (l_antes, l_depois)


cena = cena_limpa()
raiz_col = bpy.data.collections.new("ANUNCIO")
cena.collection.children.link(raiz_col)
if PARTE in ("", "provas"):
    objs = provas(cena, raiz_col)
else:
    objs = mod_u1.construir_u1(cena, raiz_col, {})
if PARTE in ("", "render"):
    renders(cena, objs)
