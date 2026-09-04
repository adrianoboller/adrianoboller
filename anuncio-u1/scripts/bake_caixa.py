# Bake da caixa de papelao da Meshy para a geometria limpa do mod_caixa.
#
# Roda UMA vez, aqui, e gera assets/caixa_cor.png, caixa_normal.png (espaco
# tangente), caixa_rugosidade.png, e a etiqueta pendurada: assets/caixa_
# etiqueta_malha.png (a malha decimada, em bytes - ver _decodificar_malha no
# mod_caixa) + caixa_etiqueta_{cor,normal,rugosidade}.png (recortes das
# texturas originais, reempacotados). O mod_caixa so carrega esses PNGs.
#
# Uso (com o .blend da Meshy ja importado):
#   blender -b caixa2.blend -P scripts/bake_caixa.py -- [--res 4096]
#       [--etapa tudo|bake|etiqueta] [--mapas cor,normal,rugosidade]
#       [--fonte tras:0.45,0.35,0.95,0.75] [--ilhas caixa2_ilhas.npy]
#       [--tris-etiqueta 6000] [--etiqueta-modo bake|recorte] [--res-etiqueta 1024]
#
# Por que assim:
# - Selected-to-active com cage por extrusao (3 cm) e raio maximo (10 cm): a
#   Meshy escalada nao coincide com os planos da caixa limpa (as faces dela
#   abaulam e as arestas sao redondas), e a extrusao cobre a diferenca.
# - As ilhas NAO impressas (interior, fundo das abas, bordas de dobradica,
#   topo das abas pequenas) recebem papelao liso copiado de uma face lisa da
#   propria Meshy (--fonte): o raio dessas faces atravessaria a parede e
#   traria os icones espelhados para dentro.
# - A Meshy e um retalho de 377 ilhas, nao uma casca; a etiqueta e o conjunto
#   das ilhas que passam de x = 0,70 (Meshy) ou tem o centro alem de 0,672
#   - as que ficam grudadas na face +X (fita) continuam no corpo e viram
#   pintura, como manda a especificacao.
# - Envelope do corpo medido pelas modas do histograma de coordenadas (o bbox
#   do arquivo inclui a etiqueta): x [-0,944, 0,653], y [-0,754, 0,695],
#   z [-0,735, 0,718].
# - Etiqueta: a malha decimada (6 k tris) e desdobrada de novo (smart
#   project) e recebe um bake da Meshy original a 1024^2. A primeira versao
#   RECORTAVA as ilhas das texturas dela: uma ilha tem UV alongada pelo atlas
#   inteiro e o recorte por bbox arrastava 60% da textura do corpo (8,4 MB
#   para uma etiqueta de 13 cm). O recorte continua em --etiqueta-modo
#   recorte.

import os
import sys
import time

import bpy
import numpy as np
from mathutils import Matrix, Vector

AQUI = os.path.dirname(os.path.abspath(__file__))
RAIZ = os.path.dirname(AQUI)
ASSETS = os.path.join(RAIZ, "assets")
if AQUI not in sys.path:
    sys.path.insert(0, AQUI)
import mod_caixa  # noqa: E402

# Envelope do corpo da Meshy (mundo do caixa2.blend), medido - ver cabecalho.
ENVELOPE = ((-0.944, 0.653), (-0.754, 0.695), (-0.735, 0.718))
X_ETIQUETA_MAX, X_ETIQUETA_CENTRO = 0.70, 0.672


def args():
    a = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    d = {"res": 4096, "etapa": "tudo", "mapas": "cor,normal,rugosidade", "fonte": "tras:0.45,0.35,0.95,0.75",
         "ilhas": "", "tris_etiqueta": 6000, "res_etiqueta": 1024, "etiqueta_modo": "bake"}
    # fonte padrao do papelao liso: miolo da face traseira, acima dos icones
    i = 0
    while i < len(a):
        k = a[i].lstrip("-").replace("-", "_")
        d[k] = a[i + 1]
        i += 2
    d["res"] = int(d["res"])
    d["tris_etiqueta"] = int(d["tris_etiqueta"])
    d["res_etiqueta"] = int(d["res_etiqueta"])
    return d


def ilhas_por_uniao(ev, nv):
    """Rotulo de ilha por vertice: propagacao vetorizada do rotulo minimo
    (converge em ~100 iteracoes para 4,6 M arestas; 80 s medidos)."""
    rot = np.arange(nv, dtype=np.int64)
    for _ in range(100000):
        mn = np.minimum(rot[ev[:, 0]], rot[ev[:, 1]])
        novo = rot.copy()
        np.minimum.at(novo, ev[:, 0], mn)
        np.minimum.at(novo, ev[:, 1], mn)
        novo = novo[novo]
        if np.array_equal(novo, rot):
            break
        rot = novo
    return np.unique(rot, return_inverse=True)[1]


def malha_rapida(nome, co, tris, uv):
    """Mesh de triangulos por foreach_set (from_pydata com 3 M faces demora
    minutos)."""
    me = bpy.data.meshes.new(nome)
    me.vertices.add(len(co))
    me.vertices.foreach_set("co", np.ascontiguousarray(co, dtype=np.float32).ravel())
    nf = len(tris)
    me.loops.add(nf * 3)
    me.loops.foreach_set("vertex_index", np.ascontiguousarray(tris, dtype=np.int32).ravel())
    me.polygons.add(nf)
    me.polygons.foreach_set("loop_start", (np.arange(nf) * 3).astype(np.int32))
    me.polygons.foreach_set("loop_total", np.full(nf, 3, dtype=np.int32))
    me.update(calc_edges=True)
    camada = me.uv_layers.new(name="UVMap")
    camada.data.foreach_set("uv", np.ascontiguousarray(uv, dtype=np.float32).ravel())
    me.polygons.foreach_set("use_smooth", [True] * nf)
    me.update()
    return me


def subconjunto(co, tris_all, uv_loops, sel_faces):
    tris = tris_all[sel_faces]
    usados, inv = np.unique(tris.ravel(), return_inverse=True)
    loops = np.stack([sel_faces * 3, sel_faces * 3 + 1, sel_faces * 3 + 2], axis=1).ravel()
    return co[usados], inv.reshape(-1, 3), uv_loops[loops]


def separar_meshy(a):
    """Devolve (corpo_obj, etiqueta_obj) alinhados com a caixa limpa e a
    matriz de alinhamento."""
    o = bpy.data.objects["mesh_node"]
    m = o.data
    nv = len(m.vertices)
    co = np.empty(nv * 3, dtype=np.float32)
    m.vertices.foreach_get("co", co)
    co = co.reshape(-1, 3)
    M = np.array(o.matrix_world)
    cow = co @ M[:3, :3].T + M[:3, 3]
    nf = len(m.polygons)
    tris = np.empty(nf * 3, dtype=np.int64)
    m.polygons.foreach_get("vertices", tris)
    tris = tris.reshape(-1, 3)
    uv = np.empty(len(m.loops) * 2, dtype=np.float32)
    m.uv_layers.active.data.foreach_get("uv", uv)
    uv = uv.reshape(-1, 2)

    cache = a["ilhas"] or os.path.join(os.path.dirname(bpy.data.filepath), "caixa2_ilhas.npy")
    if os.path.exists(cache):
        inv = np.load(cache)
    else:
        t0 = time.time()
        ne = len(m.edges)
        ev = np.empty(ne * 2, dtype=np.int64)
        m.edges.foreach_get("vertices", ev)
        inv = ilhas_por_uniao(ev.reshape(-1, 2), nv).astype(np.int32)
        np.save(cache, inv)
        print("[bake] ilhas calculadas em %.0fs" % (time.time() - t0))
    n_ilhas = int(inv.max()) + 1
    # ilha da etiqueta: passa de X_ETIQUETA_MAX ou tem o centro alem de X_ETIQUETA_CENTRO
    soma = np.zeros(n_ilhas)
    np.add.at(soma, inv, cow[:, 0])
    cont = np.bincount(inv, minlength=n_ilhas)
    xmax = np.full(n_ilhas, -9.0)
    np.maximum.at(xmax, inv, cow[:, 0])
    etiq = (xmax > X_ETIQUETA_MAX) | (soma / np.maximum(cont, 1) > X_ETIQUETA_CENTRO)
    ilha_f = inv[tris[:, 0]]
    f_et = np.where(etiq[ilha_f])[0]
    f_co = np.where(~etiq[ilha_f])[0]
    print("[bake] Meshy: %d ilhas; etiqueta = %d ilhas, %d tris; corpo = %d tris" % (n_ilhas, etiq.sum(), len(f_et), len(f_co)))

    (x0, x1), (y0, y1), (z0, z1) = ENVELOPE
    ex, ey, ez = mod_caixa.PARAMS_PADRAO["exterior"]
    s = Vector((ex / (x1 - x0), ey / (y1 - y0), ez / (z1 - z0)))
    print("[bake] fatores de escala Meshy -> caixa: %.3f / %.3f / %.3f (icones esticam %.2fx na vertical)" % (s.x, s.y, s.z, s.z / ((s.x + s.y) / 2)))
    A = Matrix.Translation((-(x0 + x1) / 2 * s.x, -(y0 + y1) / 2 * s.y, -z0 * s.z)) @ Matrix.Diagonal((s.x, s.y, s.z, 1.0))

    mat = m.materials[0]
    objs = []
    for nome, faces in (("meshy_corpo", f_co), ("meshy_etiqueta", f_et)):
        c, t, u = subconjunto(cow, tris, uv, faces)
        me = malha_rapida(nome, c, t, u)
        me.materials.append(mat)
        ob = bpy.data.objects.new(nome, me)
        ob.matrix_world = A
        bpy.context.scene.collection.objects.link(ob)
        objs.append(ob)
    o.hide_render = True
    o.hide_set(True)
    return objs[0], objs[1], A


def alvo_do_bake(geo):
    """Corpo + 4 abas (pose fechada) numa malha so, com as MESMAS normais
    customizadas da cena final: o normal map em espaco tangente e relativo a
    normal de sombreamento do alvo, entao ela tem de ser identica a do render."""
    partes = [(geo["corpo"], Vector((0, 0, 0)))] + [(a["malha"], a["pivo"]) for a in geo["abas"]]
    cos, tris, uvs, nrm = [], [], [], []
    base = 0
    for me, pivo in partes:
        nv = len(me.vertices)
        c = np.empty(nv * 3, dtype=np.float32)
        me.vertices.foreach_get("co", c)
        c = c.reshape(-1, 3) + np.array(pivo, dtype=np.float32)
        nl = len(me.loops)
        vi = np.empty(nl, dtype=np.int32)
        me.loops.foreach_get("vertex_index", vi)
        # triangula a mao (n-gons e quads): leque por poligono, com UV e normal por loop
        u = np.empty(nl * 2, dtype=np.float32)
        me.uv_layers.active.data.foreach_get("uv", u)
        u = u.reshape(-1, 2)
        n = np.array([tuple(cn.vector) for cn in me.corner_normals], dtype=np.float32)
        for poly in me.polygons:
            ls = list(poly.loop_indices)
            for k in range(1, len(ls) - 1):
                trio = (ls[0], ls[k], ls[k + 1])
                tris.append([vi[i] + base for i in trio])
                uvs.append([u[i] for i in trio])
                nrm.append([n[i] for i in trio])
        cos.append(c)
        base += nv
    co = np.concatenate(cos)
    tris = np.array(tris, dtype=np.int32)
    uvs = np.array(uvs, dtype=np.float32).reshape(-1, 2)
    nrm = np.array(nrm, dtype=np.float32).reshape(-1, 3)
    me = malha_rapida("bake_alvo", co, tris, uvs)
    me.normals_split_custom_set([tuple(v) for v in nrm])
    ob = bpy.data.objects.new("bake_alvo", me)
    bpy.context.scene.collection.objects.link(ob)
    return ob


def imagem_nova(nome, res, cor, nao_cor):
    img = bpy.data.images.get(nome)
    if img is not None:
        bpy.data.images.remove(img)
    img = bpy.data.images.new(nome, res, res, alpha=False, float_buffer=False)
    img.colorspace_settings.name = "Non-Color" if nao_cor else "sRGB"
    px = np.empty((res * res, 4), dtype=np.float32)
    px[:] = list(cor) + [1.0]
    img.pixels.foreach_set(px.ravel())
    return img


def pixels(img):
    w, h = img.size
    px = np.empty(w * h * 4, dtype=np.float32)
    img.pixels.foreach_get(px)
    return px.reshape(h, w, 4)


def gravar_png(img, caminho, bw=False):
    img.filepath_raw = caminho
    img.file_format = "PNG"
    cena = bpy.context.scene
    cena.render.image_settings.file_format = "PNG"
    cena.render.image_settings.color_mode = "BW" if bw else "RGB"
    cena.render.image_settings.color_depth = "8"
    cena.render.image_settings.compression = 60
    img.save_render(caminho, scene=cena) if False else img.save()
    print("[bake] gravado %s (%.1f MB)" % (caminho, os.path.getsize(caminho) / 1e6))


def bake(alvo, fonte, geo, a):
    cena = bpy.context.scene
    cena.render.engine = "CYCLES"
    cena.cycles.device = "CPU"
    cena.cycles.samples = 1
    cena.cycles.use_adaptive_sampling = False
    cena.cycles.use_denoising = False
    bk = cena.render.bake
    bk.use_selected_to_active = True
    bk.use_cage = False
    bk.cage_extrusion = 0.03
    bk.max_ray_distance = 0.10
    bk.margin = 8
    try:
        bk.margin_type = "EXTEND"
    except (AttributeError, TypeError):
        pass
    bk.use_clear = False
    bk.normal_space = "TANGENT"
    bk.use_pass_direct = False
    bk.use_pass_indirect = False
    bk.use_pass_color = True

    mat = bpy.data.materials.new("bake_alvo")
    mat.use_nodes = True
    tex = mat.node_tree.nodes.new("ShaderNodeTexImage")
    mat.node_tree.nodes.active = tex
    alvo.data.materials.append(mat)

    # cor de pre-preenchimento: mediana do papelao da propria Meshy
    base = pixels(bpy.data.images["base_color"])
    mr = pixels(bpy.data.images["metallic_roughness"])
    kraft = np.median(base[::8, ::8, :3].reshape(-1, 3), axis=0)
    rug_med = float(np.median(mr[::8, ::8, 1]))
    print("[bake] kraft mediano (sRGB) %s; rugosidade mediana %.3f" % (np.round(kraft * 255), rug_med))

    for ob in bpy.context.view_layer.objects:
        ob.select_set(False)
    fonte.select_set(True)
    alvo.select_set(True)
    bpy.context.view_layer.objects.active = alvo

    res = a["res"]
    mapas = a["mapas"].split(",")
    saidas = {}
    for mapa in mapas:
        t0 = time.time()
        if mapa == "cor":
            img = imagem_nova("caixa_cor", res, kraft, False)
            tex.image = img
            bpy.ops.object.bake(type="DIFFUSE", pass_filter={"COLOR"}, use_selected_to_active=True,
                                cage_extrusion=bk.cage_extrusion, max_ray_distance=bk.max_ray_distance,
                                margin=bk.margin, use_clear=False, target="IMAGE_TEXTURES")
        elif mapa == "normal":
            img = imagem_nova("caixa_normal", res, (0.5, 0.5, 1.0), True)
            tex.image = img
            bpy.ops.object.bake(type="NORMAL", use_selected_to_active=True, normal_space="TANGENT",
                                cage_extrusion=bk.cage_extrusion, max_ray_distance=bk.max_ray_distance,
                                margin=bk.margin, use_clear=False, target="IMAGE_TEXTURES")
        elif mapa == "rugosidade":
            img = imagem_nova("caixa_rugosidade", res, (rug_med,) * 3, True)
            tex.image = img
            bpy.ops.object.bake(type="ROUGHNESS", use_selected_to_active=True,
                                cage_extrusion=bk.cage_extrusion, max_ray_distance=bk.max_ray_distance,
                                margin=bk.margin, use_clear=False, target="IMAGE_TEXTURES")
        else:
            continue
        print("[bake] %s: %.0fs" % (mapa, time.time() - t0))
        preencher_nao_impressas(img, geo, a, res, neutro=(0.5, 0.5, 1.0) if mapa == "normal" else None)
        gravar_png(img, os.path.join(ASSETS, "caixa_%s.png" % mapa), bw=(mapa == "rugosidade"))
        saidas[mapa] = img
    return saidas


def preencher_nao_impressas(img, geo, a, res, neutro=None):
    """Copia papelao liso da ilha --fonte para toda ilha nao impressa (com
    8 px de margem, que cabem no gutter de 16). Densidade 0,5 = a fonte e
    reduzida 2x antes de ser ladrilhada por reflexao."""
    lay, N = geo["layout"], geo["grade"]
    px = pixels(img)
    f = res / float(N)

    def rect(nome):
        x0, y0, w, h = lay[nome]["px"]
        return int(round(x0 * f)), int(round(y0 * f)), int(round(w * f)), int(round(h * f))

    # --fonte nome[:x0,y0,x1,y1] em fracoes da ilha. O padrao evita as
    # bordas (chanfro e sombra de aresta), os icones no pe da face e a fita:
    # o fundo da Meshy e fitado no meio e mais escuro (medido no 1o bake).
    nome_fonte, _, frac = a["fonte"].partition(":")
    fx0, fy0, fx1, fy1 = [float(v) for v in frac.split(",")] if frac else (0.15, 0.15, 0.85, 0.85)
    sx, sy, sw, sh = rect(nome_fonte)
    fonte = px[sy + int(sh * fy0):sy + int(sh * fy1), sx + int(sw * fx0):sx + int(sw * fx1), :3]
    fonte_meia = fonte[:fonte.shape[0] // 2 * 2, :fonte.shape[1] // 2 * 2].reshape(fonte.shape[0] // 2, 2, fonte.shape[1] // 2, 2, 3).mean(axis=(1, 3))
    if neutro is not None:
        # normal: a media da fonte deve ser a normal plana; recentra
        fonte = fonte - fonte.mean(axis=(0, 1)) + np.array(neutro, dtype=np.float32)
        fonte_meia = fonte_meia - fonte_meia.mean(axis=(0, 1)) + np.array(neutro, dtype=np.float32)

    def ladrilho(src, h, w):
        reps = (h // src.shape[0] + 2, w // src.shape[1] + 2)
        esp = np.concatenate([src, src[::-1]], axis=0)
        esp = np.concatenate([esp, esp[:, ::-1]], axis=1)
        return np.tile(esp, (reps[0] // 2 + 1, reps[1] // 2 + 1, 1))[:h, :w]

    n = 0
    for nome, ilha in lay.items():
        if ilha["impressa"]:
            continue
        x0, y0, w, h = rect(nome)
        m = 8
        X0, Y0, X1, Y1 = max(x0 - m, 0), max(y0 - m, 0), min(x0 + w + m, res), min(y0 + h + m, res)
        src = fonte_meia if ilha["dens"] < 0.75 else fonte
        px[Y0:Y1, X0:X1, :3] = np.clip(ladrilho(src, Y1 - Y0, X1 - X0), 0, 1)
        n += 1
    img.pixels.foreach_set(px.ravel())
    print("[bake] %d ilhas nao impressas preenchidas a partir de '%s' (%dx%d px de fonte)" % (n, a["fonte"], fonte.shape[1], fonte.shape[0]))


# ---------------------------------------------------------------- etiqueta

def decimar(ob, n_tris):
    nf = len(ob.data.polygons)
    mod = ob.modifiers.new("dec", "DECIMATE")
    mod.ratio = min(1.0, n_tris / float(nf))
    mod.use_collapse_triangulate = True
    dg = bpy.context.evaluated_depsgraph_get()
    me = bpy.data.meshes.new_from_object(ob.evaluated_get(dg))
    ob.modifiers.remove(mod)
    print("[bake] etiqueta decimada: %d -> %d tris, %d verts" % (nf, len(me.polygons), len(me.vertices)))
    return me


def _desdobrar(ob):
    """Smart project na malha decimada: ilhas novas, sem sobreposicao, para o
    bake. Operadores de edicao funcionam em background com o objeto ativo."""
    import math
    for o in bpy.context.view_layer.objects:
        o.select_set(False)
    ob.select_set(True)
    bpy.context.view_layer.objects.active = ob
    bpy.ops.object.mode_set(mode="EDIT")
    bpy.ops.mesh.select_all(action="SELECT")
    bpy.ops.uv.smart_project(angle_limit=math.radians(66.0), island_margin=0.015, correct_aspect=True,
                             scale_to_bounds=False)
    bpy.ops.object.mode_set(mode="OBJECT")


def _bake_etiqueta(fonte, alvo, res):
    """Cor, normal (tangente) e rugosidade da etiqueta da Meshy sobre a malha
    decimada. Cage curto: a etiqueta e uma folha fina e um raio comprido
    acharia o verso dela."""
    cena = bpy.context.scene
    cena.render.engine = "CYCLES"
    cena.cycles.device = "CPU"
    cena.cycles.samples = 1
    cena.cycles.use_adaptive_sampling = False
    bk = cena.render.bake
    bk.use_selected_to_active = True
    bk.use_cage = False
    bk.cage_extrusion = 0.004
    bk.max_ray_distance = 0.012
    bk.margin = 4
    bk.use_clear = False
    bk.normal_space = "TANGENT"
    bk.use_pass_direct = False
    bk.use_pass_indirect = False
    bk.use_pass_color = True
    mat = bpy.data.materials.new("bake_etiqueta")
    mat.use_nodes = True
    tex = mat.node_tree.nodes.new("ShaderNodeTexImage")
    mat.node_tree.nodes.active = tex
    alvo.data.materials.append(mat)
    for o in bpy.context.view_layer.objects:
        o.select_set(False)
    fonte.hide_set(False)
    fonte.hide_render = False
    fonte.select_set(True)
    alvo.select_set(True)
    bpy.context.view_layer.objects.active = alvo
    saidas = {}
    for mapa, tipo, cor, nao_cor, arq in (
            ("cor", "DIFFUSE", (0.9, 0.9, 0.88), False, "caixa_etiqueta_cor.png"),
            ("normal", "NORMAL", (0.5, 0.5, 1.0), True, "caixa_etiqueta_normal.png"),
            ("rugosidade", "ROUGHNESS", (0.6, 0.6, 0.6), True, "caixa_etiqueta_rugosidade.png")):
        t0 = time.time()
        img = imagem_nova("et_" + mapa, res, cor, nao_cor)
        tex.image = img
        kw = dict(use_selected_to_active=True, cage_extrusion=bk.cage_extrusion, max_ray_distance=bk.max_ray_distance,
                  margin=bk.margin, use_clear=False, target="IMAGE_TEXTURES")
        if tipo == "DIFFUSE":
            bpy.ops.object.bake(type=tipo, pass_filter={"COLOR"}, **kw)
        elif tipo == "NORMAL":
            bpy.ops.object.bake(type=tipo, normal_space="TANGENT", **kw)
        else:
            bpy.ops.object.bake(type=tipo, **kw)
        # texels que nenhum raio achou (fora das ilhas + margem) ficam com a
        # cor de pre-preenchimento; conta quantos mudaram, como prova
        px = pixels(img)
        # tolerancia acima da quantizacao em byte (0,9 -> 230 -> 0,902)
        tocados = np.abs(px[..., :3] - np.array(cor, dtype=np.float32)).max(axis=2) > 2.5 / 255.0
        print("[bake] etiqueta %s: %.0fs, %.1f%% dos texels tocados" % (mapa, time.time() - t0, 100.0 * tocados.mean()))
        gravar_png(img, os.path.join(ASSETS, arq), bw=(mapa == "rugosidade"))
        saidas[mapa] = img
    return saidas


def etiqueta(ob, A, a):
    me = decimar(ob, a["tris_etiqueta"])
    if a["etiqueta_modo"] == "bake":
        # new_from_object copia o material da Meshy: com ele no slot 0 o
        # Cycles gravava no no de imagem ativo DELE (a base_color da Meshy,
        # em memoria) e o meu slot ficava intocado - 1a rodada saiu lisa.
        me.materials.clear()
        alvo = bpy.data.objects.new("etiqueta_alvo", me)
        alvo.matrix_world = A
        bpy.context.scene.collection.objects.link(alvo)
        _desdobrar(alvo)
        _bake_etiqueta(ob, alvo, a["res_etiqueta"])
        _codificar_etiqueta(me, A, uv_por_loop=True)
        return
    _etiqueta_recorte(me, A, a)


def _codificar_etiqueta(me, A, uv_por_loop, uv_novo=None):
    """Malha em bytes num PNG RGB (formato: ver mod_caixa._decodificar_malha).
    Com UV por loop (costuras do smart project), os vertices sao duplicados
    onde a UV muda, para o formato continuar 'uma UV por vertice'."""
    nv, nf = len(me.vertices), len(me.polygons)
    co = np.empty(nv * 3, dtype=np.float32)
    me.vertices.foreach_get("co", co)
    co = co.reshape(-1, 3)
    tris = np.empty(nf * 3, dtype=np.int64)
    me.polygons.foreach_get("vertices", tris)
    tris = tris.reshape(-1, 3)
    if uv_novo is None:
        uvl = np.empty(len(me.loops) * 2, dtype=np.float32)
        me.uv_layers.active.data.foreach_get("uv", uvl)
        uvl = uvl.reshape(-1, 2)
    else:
        uvl = uv_novo[tris.ravel()]
    if uv_por_loop:
        chave = np.stack([tris.ravel(), np.rint(uvl[:, 0] * 65535), np.rint(uvl[:, 1] * 65535)], axis=1)
        unicos, inv = np.unique(chave, axis=0, return_inverse=True)
        co = co[unicos[:, 0].astype(np.int64)]
        uvv = unicos[:, 1:] / 65535.0
        tris = inv.reshape(-1, 3)
        print("[bake] etiqueta: %d vertices apos separar costuras de UV (eram %d)" % (len(co), nv))
    else:
        uvv = np.zeros((nv, 2), dtype=np.float32)
        uvv[tris.ravel()] = uvl
    _gravar_malha_png(co, uvv, tris, A)


def _etiqueta_recorte(me, A, a):
    nv, nf = len(me.vertices), len(me.polygons)
    co = np.empty(nv * 3, dtype=np.float32)
    me.vertices.foreach_get("co", co)
    co = co.reshape(-1, 3)
    M = np.array(A)
    cow = co @ M[:3, :3].T + M[:3, 3]          # espaco local do corpo (que esta na origem)
    tris = np.empty(nf * 3, dtype=np.int64)
    me.polygons.foreach_get("vertices", tris)
    tris = tris.reshape(-1, 3)
    uvl = np.empty(len(me.loops) * 2, dtype=np.float32)
    me.uv_layers.active.data.foreach_get("uv", uvl)
    uvl = uvl.reshape(-1, 2)
    # UV por vertice (as ilhas da Meshy sao continuas em UV)
    uvv = np.zeros((nv, 2), dtype=np.float32)
    uvv[tris.ravel()] = uvl
    desvio = np.abs(uvv[tris.ravel()] - uvl).max()
    print("[bake] etiqueta: maior desvio UV por vertice %.5f (0 = uma UV por vertice)" % desvio)

    ne = len(me.edges)
    ev = np.empty(ne * 2, dtype=np.int64)
    me.edges.foreach_get("vertices", ev)
    inv = ilhas_por_uniao(ev.reshape(-1, 2), nv)
    n_ilhas = int(inv.max()) + 1
    # recorte por ilha nas texturas originais (2048) e reempacotamento
    src = {k: pixels(bpy.data.images[k]) for k in ("base_color", "metallic_roughness", "normal")}
    S = src["base_color"].shape[0]
    rects = []
    for k in range(n_ilhas):
        u = uvv[inv == k]
        u0, v0 = np.floor(u.min(0) * S).astype(int) - 2
        u1, v1 = np.ceil(u.max(0) * S).astype(int) + 2
        u0, v0 = max(u0, 0), max(v0, 0)
        u1, v1 = min(u1, S), min(v1, S)
        rects.append((k, u0, v0, u1 - u0, v1 - v0))
    # O empacotador de prateleiras e simples; tamanhos intermediarios evitam
    # pular de 2048 direto para 4096 por causa de poucos pixels.
    pos = None
    for R in (1024, 1280, 1536, 1792, 2048, 2560, 3072, 4096):
        if R < a["res_etiqueta"]:
            continue
        pos = mod_caixa._empacotar([(k, w, h) for k, _, _, w, h in rects], R, 3)
        if pos is not None:
            break
    area = sum(w * h for _, _, _, w, h in rects)
    print("[bake] etiqueta: area dos recortes %.0f px2 (= %d x %d)" % (area, area ** 0.5, area ** 0.5))
    print("[bake] etiqueta: %d ilhas de UV em atlas %d x %d" % (n_ilhas, R, R))
    dest = {k: np.zeros((R, R, 4), dtype=np.float32) for k in src}
    for d in dest.values():
        d[..., 3] = 1.0
    dest["normal"][..., :3] = (0.5, 0.5, 1.0)
    uv_novo = np.zeros_like(uvv)
    for k, u0, v0, w, h in rects:
        x0, y0 = pos[k]
        for nome in src:
            dest[nome][y0:y0 + h, x0:x0 + w] = src[nome][v0:v0 + h, u0:u0 + w]
        sel = inv == k
        uv_novo[sel, 0] = (uvv[sel, 0] * S - u0 + x0) / R
        uv_novo[sel, 1] = (uvv[sel, 1] * S - v0 + y0) / R
    for nome, arq, nao_cor in (("base_color", "caixa_etiqueta_cor.png", False),
                               ("normal", "caixa_etiqueta_normal.png", True),
                               ("metallic_roughness", "caixa_etiqueta_rugosidade.png", True)):
        d = dest[nome]
        if nome == "metallic_roughness":
            d = d.copy()
            d[..., :3] = d[..., 1:2]      # a rugosidade e o canal G
        img = imagem_nova("et_" + nome, R, (0, 0, 0), nao_cor)
        img.pixels.foreach_set(d.ravel())
        gravar_png(img, os.path.join(ASSETS, arq), bw=(nome == "metallic_roughness"))

    _codificar_etiqueta(me, A, uv_por_loop=False, uv_novo=uv_novo)


def _gravar_malha_png(co, uv_novo, tris, A):
    M = np.array(A)
    cow = co @ M[:3, :3].T + M[:3, 3]          # espaco local do corpo (que esta na origem)
    nv, nf = len(cow), len(tris)
    bb = np.concatenate([cow.min(0), cow.max(0)]).astype(np.float64)
    vq = np.rint((cow - bb[:3]) / np.maximum(bb[3:] - bb[:3], 1e-9) * 65535).astype("<u2")
    uq = np.rint(np.clip(uv_novo, 0, 1) * 65535).astype("<u2")
    tq = tris.astype("<u2")
    assert nv < 65536 and nf < 65536
    cab = b"ETQ1" + np.array([nv, nf], dtype="<u2").tobytes() + np.rint((bb + 4.0) * 1e5).astype("<u4").tobytes()
    corpo = cab + vq.tobytes() + uq.tobytes() + tq.tobytes()
    npx = (len(corpo) + 2) // 3
    W = 256
    H = (npx + W - 1) // W
    buf = np.zeros(W * H * 3, dtype=np.uint8)
    buf[:len(corpo)] = np.frombuffer(corpo, dtype=np.uint8)
    px = np.ones((W * H, 4), dtype=np.float32)
    px[:, :3] = buf.reshape(-1, 3) / 255.0
    img = imagem_nova("et_malha", W, (0, 0, 0), True)
    bpy.data.images.remove(img)
    img = bpy.data.images.new("et_malha", W, H, alpha=True, float_buffer=False)
    img.colorspace_settings.name = "Non-Color"
    img.pixels.foreach_set(px.ravel())
    caminho = os.path.join(ASSETS, "caixa_etiqueta_malha.png")
    cena = bpy.context.scene
    cena.render.image_settings.color_mode = "RGBA"
    img.filepath_raw = caminho
    img.file_format = "PNG"
    img.save()
    print("[bake] malha da etiqueta: %d bytes em %dx%d px -> %s (%.0f KB)" % (len(corpo), W, H, caminho, os.path.getsize(caminho) / 1e3))

    # prova do caminho de volta: reler o PNG gravado e comparar
    bpy.data.images.remove(img)
    img2 = bpy.data.images.load(caminho)
    img2.colorspace_settings.name = "Non-Color"
    v2, uv2, t2 = mod_caixa._decodificar_malha(img2)
    err_v = np.abs(v2 - cow).max()
    err_uv = np.abs(uv2 - uv_novo).max()
    ok_t = np.array_equal(t2, tris)
    print("[bake] etiqueta relida: erro max vertices %.2e m, UV %.2e, triangulos iguais: %s" % (err_v, err_uv, ok_t))
    assert ok_t and err_v < 1e-4
    print("[bake] etiqueta em coordenadas da caixa: x %.3f..%.3f y %.3f..%.3f z %.3f..%.3f" % tuple(np.concatenate([cow.min(0), cow.max(0)])[[0, 3, 1, 4, 2, 5]]))


def main():
    a = args()
    t0 = time.time()
    geo = mod_caixa.geometria_caixa()
    print("[bake] geometria limpa: ppm %.1f, grade %d" % (geo["ppm"], geo["grade"]))
    corpo_m, etiq_m, A = separar_meshy(a)
    print("[bake] Meshy separada em %.0fs" % (time.time() - t0))
    if a["etapa"] in ("tudo", "bake"):
        alvo = alvo_do_bake(geo)
        etiq_m.hide_render = True
        etiq_m.hide_set(True)
        bake(alvo, corpo_m, geo, a)
    if a["etapa"] in ("tudo", "etiqueta"):
        etiqueta(etiq_m, A, a)
    print("[bake] total %.0fs" % (time.time() - t0))


main()
