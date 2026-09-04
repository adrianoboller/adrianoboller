# Limpeza da impressora do cliente (Meshy AI) - roda UMA vez, aqui, e produz
# assets/impressora_limpa.glb, que o mod_u1.py carrega.
#
# O que a malha crua e (medido em scratchpad/impressora.blend): 1.877.138
# triangulos em 1.038.215 vertices, 5.644 ilhas, TODAS com borda aberta - a
# Meshy entrega uma sopa de retalhos sobrepostos, nao cascas fechadas. A
# maior ilha tem 0,85% dos vertices e as ilhas com menos de 0,1% somam 37%
# da malha; por isso o descarte de lixo nao pode ser so por tamanho:
# descarta-se o que e triangulo isolado ou o que esta longe de qualquer
# retalho maior (flutuando fora da maquina).
#
# Orientacao no arquivo: a frente (porta de vidro, wordmark, tela) ja aponta
# para -Y; as 4 bobinas de filamento ficam nas laterais (|x| > 0,60), os 4
# tubos sobem acima do aro (z > 0,26) e os cabecotes estacionam no fundo do
# topo (y 0,28..0,52). Unidades locais do arquivo: a caixa envolvente e
# 1,666 x 1,600 x 2,000 (escala 0,025 no objeto, ignorada aqui).
#
# Etapas (cada uma cabe em menos de 10 min; --etapa tudo roda as tres):
#   separar - ilhas por BFS no bmesh, classificacao por regiao, uma malha por
#             peca (montada por indices); grava impressora_pecas.blend
#   acabar  - tapa buracos internos (holes_fill por laco), decima ate o
#             orcamento de triangulos, escala para o envelope do U1, mede
#             os pontos de ancoragem, materiais e exporta o GLB
#   conferir- reimporta o GLB numa cena vazia e confere nomes/UV/materiais
#
# Uso: blender -b scratchpad/impressora.blend -P scripts/limpar_impressora.py -- --etapa separar
#      blender -b scratchpad/impressora_pecas.blend -P scripts/limpar_impressora.py -- --etapa acabar
#      blender -b -P scripts/limpar_impressora.py -- --etapa conferir

import base64
import os
import sys
import time
import zlib

import numpy as np

import bpy
import bmesh
from mathutils import Vector

SCRATCH = "/tmp/claude-0/-home-user-adrianoboller/857c9466-78dd-51a3-8b8e-9f13227d5fd4/scratchpad"
RAIZ = "/home/user/adrianoboller/anuncio-u1"
DESTINO = os.path.join(RAIZ, "assets", "impressora_limpa.glb")
PECAS_BLEND = os.path.join(SCRATCH, "impressora_pecas.blend")
LIMPA_BLEND = os.path.join(SCRATCH, "impressora_limpa.blend")
TEXTURAS = {
    "cor": os.path.join(SCRATCH, "impressora_texture.png"),
    "metal_rugosidade": os.path.join(SCRATCH, "impressora_metallic_roughness.png"),
    "normal": os.path.join(SCRATCH, "impressora_normal.png"),
}

ALVO = (0.584, 0.499, 0.730)     # envelope do U1 real (m)
TRIS_ALVO = 400000               # orcamento total depois da decimacao
# Buraco interno so e tapado se o laco couber nisto (unidades locais; 0,12 =
# 44 mm depois da escala): maior que isso e abertura de projeto, nao defeito.
LACO_MAX_DIAG = 0.12
LACO_MAX_ARESTAS = 200

# Regioes em unidades locais (ver cabecalho e as vistas em scratchpad/vistas).
Z_ARO = 0.26            # topo do aro preto: acima disto so ha tubo
X_LADO = 0.60           # face lateral do corpo: fora disto so ha bobina/alimentador
PORTA = {"x": (-0.40, 0.36), "z": (-0.93, -0.08), "y": (-0.81, -0.745)}
PORTA_CAMADA = 0.012    # so a camada da frente da lamina (3,7 mm depois da escala) vira vidro
MESA = {"x": (-0.33, 0.33), "y": (-0.36, 0.34), "z": (-0.92, -0.82), "esp_max": 0.05}
CABECOTE = {"x": (-0.40, 0.40), "y": (0.28, 0.52), "z": (-0.06, 0.40), "larg_max": 0.17}
TELA_MESHY = {"x": (0.168, 0.449), "z": (0.019, 0.187)}   # onde a Meshy pintou a tela (vista de frente, +-1 px = 4 mm)

GRUPOS = ["corpo", "porta", "tubos", "bobinas", "mesa", "cabecote.1", "cabecote.2", "cabecote.3", "cabecote.4"]


def _args():
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    etapa = "tudo"
    if "--etapa" in argv:
        etapa = argv[argv.index("--etapa") + 1]
    return etapa


def _log(*a):
    print("[limpar]", *a, flush=True)


# ---------------------------------------------------------------------------
# Etapa 1: separar
# ---------------------------------------------------------------------------

def ilhas_por_bfs(bm):
    """Etiqueta de ilha por vertice, por busca em largura no bmesh (linear;
    nao usa o operador de separar por partes soltas)."""
    n = len(bm.verts)
    etq = np.full(n, -1, dtype=np.int64)
    bm.verts.ensure_lookup_table()
    atual = 0
    for v0 in bm.verts:
        if etq[v0.index] >= 0:
            continue
        etq[v0.index] = atual
        pilha = [v0]
        while pilha:
            v = pilha.pop()
            for e in v.link_edges:
                w = e.other_vert(v)
                if etq[w.index] < 0:
                    etq[w.index] = atual
                    pilha.append(w)
        atual += 1
    return etq, atual


def etapa_separar():
    t0 = time.time()
    obj = bpy.data.objects["mesh_node"]
    me = obj.data
    nv, nf = len(me.vertices), len(me.polygons)
    co = np.empty(nv * 3, dtype=np.float64)
    me.vertices.foreach_get("co", co)
    co = co.reshape(-1, 3)
    # Todas as faces sao triangulos (loops = 3 x faces, medido): indices dos 3 vertices.
    lv = np.empty(len(me.loops), dtype=np.int64)
    me.loops.foreach_get("vertex_index", lv)
    assert len(me.loops) == 3 * nf, "malha nao e toda de triangulos"
    tri = lv.reshape(-1, 3)
    nrm = np.empty(nf * 3, dtype=np.float64)
    me.polygons.foreach_get("normal", nrm)
    nrm = nrm.reshape(-1, 3)

    bm = bmesh.new()
    bm.from_mesh(me)
    _log("bmesh carregado: %d faces em %.1fs" % (len(bm.faces), time.time() - t0))
    ilha_v, n_ilhas = ilhas_por_bfs(bm)
    _log("ilhas por BFS: %d em %.1fs" % (n_ilhas, time.time() - t0))
    ilha_f = ilha_v[tri[:, 0]]
    verts_ilha = np.bincount(ilha_v, minlength=n_ilhas)
    tris_ilha = np.bincount(ilha_f, minlength=n_ilhas)
    mn = np.full((n_ilhas, 3), 1e9)
    mx = np.full((n_ilhas, 3), -1e9)
    for c in range(3):
        np.minimum.at(mn[:, c], ilha_v, co[:, c])
        np.maximum.at(mx[:, c], ilha_v, co[:, c])
    ext = mx - mn
    cen = (mn + mx) / 2.0

    # --- lixo: triangulo isolado, ou ilha < 0,1% dos vertices longe (1,5 mm)
    # de qualquer retalho com >= 200 triangulos. So o tamanho nao serve: as
    # ilhas pequenas somam 37% da malha e a maioria e detalhe da maquina.
    grandes = np.where(tris_ilha >= 200)[0]
    pequena = verts_ilha < 0.001 * nv
    lixo = verts_ilha <= 3
    folga = 0.004
    for k in np.where(pequena & ~lixo)[0]:
        d = np.maximum(0.0, np.maximum(mn[grandes] - mx[k], mn[k] - mx[grandes])).max(axis=1)
        if d.min() > folga:
            lixo[k] = True
    _log("lixo: %d ilhas (%d triangulos isolados + %d longe de tudo), %d triangulos"
         % (lixo.sum(), (verts_ilha <= 3).sum(), lixo.sum() - (verts_ilha <= 3).sum(), tris_ilha[lixo].sum()))
    for k in np.argsort(-tris_ilha * lixo)[:8]:
        if lixo[k]:
            _log("   maior lixo: ilha %d, %d tris, centro %s, extensao %s" % (k, tris_ilha[k], np.round(cen[k], 3), np.round(ext[k], 3)))

    # --- classificacao por ilha
    grupo_ilha = np.zeros(n_ilhas, dtype=np.int64)     # 0 = corpo
    tubos = (mn[:, 2] > Z_ARO) & ~lixo
    bobinas = ((mn[:, 0] > X_LADO) | (mx[:, 0] < -X_LADO)) & ~lixo & ~tubos
    mesa = (~lixo & ~tubos & ~bobinas
            & (mn[:, 0] > MESA["x"][0]) & (mx[:, 0] < MESA["x"][1])
            & (mn[:, 1] > MESA["y"][0]) & (mx[:, 1] < MESA["y"][1])
            & (mn[:, 2] > MESA["z"][0]) & (mx[:, 2] < MESA["z"][1])
            & (ext[:, 2] < MESA["esp_max"]) & (tris_ilha > 100))
    # Cabecotes: 4 blocos no fundo do topo; o x de cada um vem dos tubos, que
    # sobem direto deles (4 grupos de x separados pelos 3 maiores vaos).
    xs = np.sort(cen[tubos & (cen[:, 2] > 0.5), 0])
    vaos = np.argsort(-np.diff(xs))[:3]
    cortes = np.sort(xs[vaos] + np.diff(xs)[vaos] / 2.0)
    x_cab = [xs[xs < cortes[0]].mean(), xs[(xs >= cortes[0]) & (xs < cortes[1])].mean(),
             xs[(xs >= cortes[1]) & (xs < cortes[2])].mean(), xs[xs >= cortes[2]].mean()]
    _log("x dos cabecotes (pelos tubos): %s" % np.round(x_cab, 3))
    cand = (~lixo & ~tubos & ~bobinas & ~mesa
            & (mn[:, 0] > CABECOTE["x"][0]) & (mx[:, 0] < CABECOTE["x"][1])
            & (mn[:, 1] > CABECOTE["y"][0]) & (mx[:, 1] < CABECOTE["y"][1])
            & (mn[:, 2] > CABECOTE["z"][0]) & (mx[:, 2] < CABECOTE["z"][1])
            & (ext[:, 0] < CABECOTE["larg_max"]))
    for k in np.where(cand)[0]:
        n = int(np.argmin([abs(cen[k, 0] - x) for x in x_cab]))
        if abs(cen[k, 0] - x_cab[n]) < 0.09:
            grupo_ilha[k] = 5 + n
    grupo_ilha[tubos] = 2
    grupo_ilha[bobinas] = 3
    grupo_ilha[mesa] = 4
    grupo_ilha[lixo] = -1

    grupo_f = grupo_ilha[ilha_f]
    # Porta: por FACE, na lamina da frente dentro do retangulo do vidro. A
    # Meshy pintou o vidro opaco e o modelou em camadas (medido: uma com
    # normal -Y a 2 mm da frente, outra com normal +Y 2 mm atras, e mais uma
    # -Y 4 mm atras). So a camada da FRENTE vira vidro; as outras sao
    # descartadas: o vidro do EEVEE e um slab de 4 mm (thickness no material),
    # e qualquer camada atras dele refrata de novo e cai no World - medido
    # no exp_porta: com as camadas todas a janela dava L 0,095, com so a
    # frente e slab 0,210, sem vidro 0,421.
    pc = co[tri].mean(axis=1)
    lamina = ((pc[:, 0] > PORTA["x"][0]) & (pc[:, 0] < PORTA["x"][1])
              & (pc[:, 2] > PORTA["z"][0]) & (pc[:, 2] < PORTA["z"][1])
              & (pc[:, 1] > PORTA["y"][0]) & (pc[:, 1] < PORTA["y"][1]) & (grupo_f == 0))
    y_frente = pc[lamina, 1].min()
    frente_porta = lamina & (nrm[:, 1] < -0.5) & (pc[:, 1] < y_frente + PORTA_CAMADA)
    grupo_f[frente_porta] = 1
    grupo_f[lamina & ~frente_porta] = -1
    _log("porta: lamina com %d tris (y min %.4f); camada da frente %d tris vira vidro, %d descartados"
         % (lamina.sum(), y_frente, frente_porta.sum(), (lamina & ~frente_porta).sum()))
    # Normais da frente: fracao apontando para fora (-Y) nas faces da lamina frontal.
    frente = (pc[:, 1] < -0.745) & (np.abs(nrm[:, 1]) > 0.7)
    _log("faces da frente com normal para fora: %.1f%% de %d" % (100.0 * (nrm[frente, 1] < 0).mean(), frente.sum()))
    # Tela pintada pela Meshy: bbox das faces escuras... nao da para ler cor
    # aqui sem a textura; usa o retangulo medido nas vistas (TELA_MESHY).
    for g, nome in enumerate(GRUPOS):
        _log("grupo %-11s %8d tris" % (nome, (grupo_f == g).sum()))
    _log("lixo descartado: %d tris; total mantido: %d" % ((grupo_f < 0).sum(), (grupo_f >= 0).sum()))

    # --- uma malha por grupo. bmesh.ops.split com 'dest' nao existe na API
    # Python do 4.2 ("keyword dest not working yet"), entao a peca e montada
    # pelos indices: vertices usados, faces reindexadas e a UV copiada loop a
    # loop (malha so de triangulos: os loops da face i sao 3i..3i+2).
    bm.free()
    uv = np.empty(len(me.loops) * 2, dtype=np.float64)
    me.uv_layers.active.data.foreach_get("uv", uv)
    uv = uv.reshape(-1, 3, 2)
    cena = bpy.context.scene
    col = bpy.data.collections.new("pecas")
    cena.collection.children.link(col)
    for g, nome in enumerate(GRUPOS):
        faces = np.where(grupo_f == g)[0]
        if len(faces) == 0:
            _log("grupo %s vazio" % nome)
            continue
        usados, novo_idx = np.unique(tri[faces].ravel(), return_inverse=True)
        malha = bpy.data.meshes.new("u1." + nome)
        malha.from_pydata(co[usados].tolist(), [], novo_idx.reshape(-1, 3).tolist())
        camada = malha.uv_layers.new(name="UVMap")
        camada.data.foreach_set("uv", uv[faces].ravel())
        malha.validate()
        malha.update()
        o = bpy.data.objects.new("u1." + nome, malha)
        col.objects.link(o)
        _log("peca u1.%s: %d faces, %d verts, uv=%s (%.1fs)" % (nome, len(malha.polygons), len(malha.vertices), malha.uv_layers.active.name, time.time() - t0))
    # A malha crua sai do arquivo de pecas (180 MB a menos).
    bpy.data.objects.remove(obj, do_unlink=True)
    bpy.data.meshes.remove(me)
    for o in list(bpy.data.objects):
        if o.type in ("CAMERA", "LIGHT"):
            bpy.data.objects.remove(o, do_unlink=True)
    bpy.ops.wm.save_as_mainfile(filepath=PECAS_BLEND, compress=True)
    _log("gravou %s em %.1fs" % (PECAS_BLEND, time.time() - t0))


# ---------------------------------------------------------------------------
# Etapa 2: acabar (buracos, decimacao, escala, materiais, exportar)
# ---------------------------------------------------------------------------

def _uniao_por_arestas(n, ev):
    """Etiqueta de componente por vertice, propagando o minimo pelas arestas."""
    etq = np.arange(n, dtype=np.int64)
    for _ in range(100000):
        m = np.minimum(etq[ev[:, 0]], etq[ev[:, 1]])
        novo = etq.copy()
        np.minimum.at(novo, ev[:, 0], m)
        np.minimum.at(novo, ev[:, 1], m)
        novo = novo[novo]
        if np.array_equal(novo, etq):
            return etq
        etq = novo
    return etq


def tapar_buracos(obj):
    """Tapa os lacos de borda INTERNOS de cada ilha: por ilha, o laco de maior
    extensao e o contorno do retalho (fica); os outros, se couberem em
    LACO_MAX_DIAG / LACO_MAX_ARESTAS, sao buracos e recebem holes_fill (que nao
    move vertice nenhum). Devolve (lacos tapados, triangulos novos)."""
    me = obj.data
    bm = bmesh.new()
    bm.from_mesh(me)
    bm.verts.ensure_lookup_table()
    bm.edges.ensure_lookup_table()
    n = len(bm.verts)
    co = np.array([v.co[:] for v in bm.verts])
    ev = np.array([(e.verts[0].index, e.verts[1].index) for e in bm.edges], dtype=np.int64)
    borda = np.array([e.is_boundary for e in bm.edges])
    if not borda.any():
        bm.free()
        return 0, 0
    ilha = _uniao_por_arestas(n, ev)
    evb = ev[borda]
    idx_b = np.where(borda)[0]
    # Componentes so entre arestas de borda (= lacos, ou cadeias abertas)
    etq = _uniao_por_arestas(n, evb)
    laco = etq[evb[:, 0]]
    ids, inv = np.unique(laco, return_inverse=True)
    n_lacos = len(ids)
    lmn = np.full((n_lacos, 3), 1e9)
    lmx = np.full((n_lacos, 3), -1e9)
    for c in range(3):
        for col_ in (0, 1):
            np.minimum.at(lmn[:, c], inv, co[evb[:, col_], c])
            np.maximum.at(lmx[:, c], inv, co[evb[:, col_], c])
    diag = np.linalg.norm(lmx - lmn, axis=1)
    arestas = np.bincount(inv, minlength=n_lacos)
    ilha_laco = ilha[evb[np.unique(inv, return_index=True)[1], 0]]
    # Contorno externo = maior diagonal dentro da ilha.
    maior = {}
    for k in range(n_lacos):
        i = ilha_laco[k]
        if i not in maior or diag[k] > diag[maior[i]]:
            maior[i] = k
    externo = np.zeros(n_lacos, dtype=bool)
    externo[list(maior.values())] = True
    buraco = ~externo & (diag <= LACO_MAX_DIAG) & (arestas <= LACO_MAX_ARESTAS) & (arestas >= 3)
    sel = np.where(buraco[inv])[0]
    if len(sel) == 0:
        bm.free()
        return 0, 0
    edges = [bm.edges[int(idx_b[i])] for i in sel]
    antes = len(bm.faces)
    r = bmesh.ops.holes_fill(bm, edges=edges, sides=LACO_MAX_ARESTAS)
    novas = r.get("faces", [])
    if novas:
        bmesh.ops.triangulate(bm, faces=novas)
    depois = len(bm.faces)
    bm.to_mesh(me)
    bm.free()
    me.update()
    return int(buraco.sum()), depois - antes


def decimar(obj, ratio):
    if ratio >= 1.0:
        return
    mod = obj.modifiers.new("decimar", "DECIMATE")
    mod.decimate_type = "COLLAPSE"
    mod.ratio = ratio
    mod.use_collapse_triangulate = True
    dg = bpy.context.evaluated_depsgraph_get()
    ev = obj.evaluated_get(dg)
    nova = bpy.data.meshes.new_from_object(ev)
    nova.name = obj.data.name + ".dec"
    antiga = obj.data
    obj.modifiers.remove(mod)
    obj.data = nova
    nome = antiga.name
    bpy.data.meshes.remove(antiga)
    nova.name = nome


def _bbox(objs):
    mn = np.full(3, 1e9)
    mx = np.full(3, -1e9)
    for o in objs:
        for c in o.bound_box:
            w = o.matrix_world @ Vector(c)
            mn = np.minimum(mn, np.array(w[:]))
            mx = np.maximum(mx, np.array(w[:]))
    return mn, mx


def _imagem(nome, caminho, nao_cor):
    img = bpy.data.images.get(nome)
    if img is not None:
        bpy.data.images.remove(img)
    img = bpy.data.images.load(caminho)
    img.name = nome
    if nao_cor:
        img.colorspace_settings.name = "Non-Color"
    img.pack()
    return img


def material_meshy():
    """Principled com as tres texturas da Meshy: cor, rugosidade = canal G e
    metalico = canal B de metallic_roughness (convencao glTF), normal map.
    E o mesmo grafo que o importador glTF monta - e que o exportador
    reconhece de volta como metallicRoughnessTexture."""
    mat = bpy.data.materials.get("u1.meshy") or bpy.data.materials.new("u1.meshy")
    mat.use_nodes = True
    nt = mat.node_tree
    nt.nodes.clear()
    saida = nt.nodes.new("ShaderNodeOutputMaterial")
    saida.location = (600, 0)
    bsdf = nt.nodes.new("ShaderNodeBsdfPrincipled")
    bsdf.location = (250, 0)
    nt.links.new(bsdf.outputs["BSDF"], saida.inputs["Surface"])
    cor = nt.nodes.new("ShaderNodeTexImage")
    cor.image = _imagem("u1.meshy.cor", TEXTURAS["cor"], False)
    cor.location = (-500, 300)
    nt.links.new(cor.outputs["Color"], bsdf.inputs["Base Color"])
    mr = nt.nodes.new("ShaderNodeTexImage")
    mr.image = _imagem("u1.meshy.metal_rugosidade", TEXTURAS["metal_rugosidade"], True)
    mr.location = (-500, 0)
    sep = nt.nodes.new("ShaderNodeSeparateColor")
    sep.location = (-150, 0)
    nt.links.new(mr.outputs["Color"], sep.inputs["Color"])
    nt.links.new(sep.outputs["Green"], bsdf.inputs["Roughness"])
    nt.links.new(sep.outputs["Blue"], bsdf.inputs["Metallic"])
    nm = nt.nodes.new("ShaderNodeTexImage")
    nm.image = _imagem("u1.meshy.normal", TEXTURAS["normal"], True)
    nm.location = (-500, -350)
    mapa = nt.nodes.new("ShaderNodeNormalMap")
    mapa.location = (-150, -350)
    nt.links.new(nm.outputs["Color"], mapa.inputs["Color"])
    nt.links.new(mapa.outputs["Normal"], bsdf.inputs["Normal"])
    return mat


def material_vidro():
    """Vidro da porta: Transmission (KHR_materials_transmission no GLB); o
    modo de render do EEVEE e a espessura (slab) nao viajam no glTF - o
    mod_u1 os reaplica."""
    mat = bpy.data.materials.get("u1.vidro") or bpy.data.materials.new("u1.vidro")
    mat.use_nodes = True
    nt = mat.node_tree
    nt.nodes.clear()
    saida = nt.nodes.new("ShaderNodeOutputMaterial")
    bsdf = nt.nodes.new("ShaderNodeBsdfPrincipled")
    nt.links.new(bsdf.outputs["BSDF"], saida.inputs["Surface"])
    bsdf.inputs["Base Color"].default_value = (0.35, 0.36, 0.40, 1.0)
    bsdf.inputs["Transmission Weight"].default_value = 1.0
    bsdf.inputs["Roughness"].default_value = 0.02
    bsdf.inputs["IOR"].default_value = 1.5
    try:
        mat.surface_render_method = "DITHERED"
        mat.use_raytrace_refraction = True
    except AttributeError:
        pass
    return mat


def _raio(cena, origem, direcao, nome_obj=None):
    dg = bpy.context.evaluated_depsgraph_get()
    r = cena.ray_cast(dg, Vector(origem), Vector(direcao).normalized())
    if r[0] and (nome_obj is None or r[4].name == nome_obj):
        return r[1]
    return None


def etapa_acabar():
    t0 = time.time()
    cena = bpy.context.scene
    pecas = [o for o in bpy.data.objects if o.type == "MESH" and o.name.startswith("u1.")]
    tris0 = {o.name: len(o.data.polygons) for o in pecas}
    _log("pecas: %s" % ", ".join("%s %d" % (k, v) for k, v in tris0.items()))

    # --- buracos
    total_lacos = total_tris = 0
    for o in pecas:
        n, t = tapar_buracos(o)
        total_lacos += n
        total_tris += t
        _log("buracos em %s: %d lacos tapados, %d triangulos novos (%.1fs)" % (o.name, n, t, time.time() - t0))
    _log("buracos: %d lacos tapados, %d triangulos novos" % (total_lacos, total_tris))

    # --- decimacao: mesma razao para todas as pecas, ate o orcamento.
    total = sum(len(o.data.polygons) for o in pecas)
    ratio = min(1.0, TRIS_ALVO * 0.985 / total)
    _log("decimacao: %d -> alvo %d, razao %.3f" % (total, TRIS_ALVO, ratio))
    for o in pecas:
        antes = len(o.data.polygons)
        decimar(o, ratio)
        _log("   %s: %d -> %d tris (%.1fs)" % (o.name, antes, len(o.data.polygons), time.time() - t0))
    total_dec = sum(len(o.data.polygons) for o in pecas)
    _log("triangulos depois da decimacao: %d" % total_dec)

    # --- escala para o envelope: Z manda; X e Y seguem a mesma razao, e so
    # encolhem (nunca esticam) o que passar do alvo. Anotado no relatorio.
    mn, mx = _bbox(pecas)
    dims = mx - mn
    sz = ALVO[2] / dims[2]
    sx = min(sz, ALVO[0] / dims[0])
    sy = min(sz, ALVO[1] / dims[1])
    _log("envelope cru %s; fatores x %.4f y %.4f z %.4f (uniforme seria %.4f; x %.1f%% e y %.1f%% do uniforme)"
         % (np.round(dims, 4), sx, sy, sz, sz, 100 * sx / sz, 100 * sy / sz))
    centro = (mn + mx) / 2.0
    mn0 = mn.copy()
    for o in pecas:
        me = o.data
        n = len(me.vertices)
        co = np.empty(n * 3)
        me.vertices.foreach_get("co", co)
        co = co.reshape(-1, 3)
        co[:, 0] = (co[:, 0] - centro[0]) * sx
        co[:, 1] = (co[:, 1] - centro[1]) * sy
        co[:, 2] = (co[:, 2] - mn0[2]) * sz
        me.vertices.foreach_set("co", co.ravel())
        me.polygons.foreach_set("use_smooth", [True] * len(me.polygons))
        me.update()
        # Origem no centro da peca: matrix_world.translation passa a dizer
        # onde a peca esta (a coreografia le isso para mesa e puxador).
        c = (co.min(axis=0) + co.max(axis=0)) / 2.0
        co -= c
        me.vertices.foreach_set("co", co.ravel())
        me.update()
        o.location = Vector(c)
    bpy.context.view_layer.update()
    mn, mx = _bbox(pecas)
    _log("envelope final: %.4f x %.4f x %.4f  (x %.4f..%.4f, y %.4f..%.4f, z %.4f..%.4f)"
         % (tuple(mx - mn) + (mn[0], mx[0], mn[1], mx[1], mn[2], mx[2])))
    corpo = bpy.data.objects["u1.corpo"]
    mnc, mxc = _bbox([corpo])
    _log("envelope do corpo (sem tubos/bobinas): %.4f x %.4f x %.4f  (z ate %.4f, y %.4f..%.4f)" % (tuple(mxc - mnc) + (mxc[2], mnc[1], mxc[1])))

    def local_para_m(x, y, z):
        """Unidades locais do arquivo cru -> metros no modelo final."""
        return np.array([(x - centro[0]) * sx, (y - centro[1]) * sy, (z - mn0[2]) * sz])

    # --- materiais
    m_meshy = material_meshy()
    m_vidro = material_vidro()
    for o in pecas:
        o.data.materials.clear()
        o.data.materials.append(m_vidro if o.name == "u1.porta" else m_meshy)

    # --- pontos de ancoragem no modelo final (raio contra a malha escalada)
    bpy.context.view_layer.update()
    # Tela: retangulo que a Meshy pintou (TELA_MESHY), levado a metros; a
    # face da frente e medida por raio -Y -> +Y no centro dele.
    tx = (np.array(TELA_MESHY["x"]) - centro[0]) * sx
    tz = (np.array(TELA_MESHY["z"]) - mn0[2]) * sz
    tela_c = np.array([tx.mean(), 0.0, tz.mean()])
    p = _raio(cena, (tela_c[0], -2.0, tela_c[2]), (0, 1, 0))
    _log("tela: retangulo Meshy %.1f x %.1f mm, centro x %.4f z %.4f; face da frente em y = %s" % ((tx[1] - tx[0]) * 1000, (tz[1] - tz[0]) * 1000, tela_c[0], tela_c[2], None if p is None else round(p.y, 4)))
    # Traseira: face de tras do corpo na coluna do botao/tomada (x = +0,45 local)
    xb = (0.45 - centro[0]) * sx
    for nome, z in (("tomada", 0.100), ("botao", 0.150)):
        q = _raio(cena, (xb, 2.0, z), (0, -1, 0))
        _log("%s: x %.4f z %.3f; face de tras em y = %s (objeto %s)" % (nome, xb, z, None if q is None else round(q.y, 4), None))
    # Puxador: borda direita da porta, meia altura
    px = (0.41 - centro[0]) * sx
    pz = (-0.49 - mn0[2]) * sz
    q = _raio(cena, (px, -2.0, pz), (0, 1, 0))
    _log("puxador: x %.4f z %.4f; frente em y = %s" % (px, pz, None if q is None else round(q.y, 4)))
    # Cabecotes: centro de cada peca; topo do aro (z) e vao do topo (LEDs)
    for n in range(1, 5):
        o = bpy.data.objects.get("u1.cabecote.%d" % n)
        if o is not None:
            _log("cabecote %d: centro %s, %d tris" % (n, np.round(o.location[:], 4), len(o.data.polygons)))
    mesa = bpy.data.objects.get("u1.mesa")
    if mesa is not None:
        _log("mesa: centro %s, dims %s" % (np.round(mesa.location[:], 4), np.round(mesa.dimensions[:], 4)))
    for x in (-0.20, 0.0, 0.20):
        q = _raio(cena, (x, -0.5 * sy, 2.0), (0, 0, -1))
        _log("topo em (x %.2f, y %.3f): z = %s" % (x, -0.5 * sy, None if q is None else round(q.z, 4)))
    q = _raio(cena, (0.0, 0.0, 2.0), (0, 0, -1))
    _log("centro do topo: primeiro toque z = %s (%s)" % (None if q is None else round(q.z, 4), "vazio" if q is None else ""))

    # --- exportar
    for o in bpy.data.objects:
        o.select_set(o in pecas)
    bpy.context.view_layer.objects.active = pecas[0]
    bpy.ops.export_scene.gltf(
        filepath=DESTINO, export_format="GLB", use_selection=True,
        export_apply=True, export_texcoords=True, export_normals=True,
        export_materials="EXPORT", export_image_format="AUTO",
        export_animations=False, export_lights=False, export_cameras=False,
        export_extras=False, export_yup=True,
    )
    tam = os.path.getsize(DESTINO)
    with open(DESTINO, "rb") as f:
        dados = f.read()
    b64 = base64.b64encode(zlib.compress(dados, 9))
    _log("GLB exportado: %s, %.2f MB; zlib+base64 = %.2f MB (%s 8 MB: %s)"
         % (DESTINO, tam / 1e6, len(b64) / 1e6, "<=" if len(b64) <= 8e6 else ">", "embutir" if len(b64) <= 8e6 else "arquivo ao lado do .blend"))
    bpy.ops.wm.save_as_mainfile(filepath=LIMPA_BLEND, compress=True)
    _log("gravou %s; etapa acabar em %.1fs" % (LIMPA_BLEND, time.time() - t0))


# ---------------------------------------------------------------------------
# Etapa 3: conferir (reimportar numa cena vazia)
# ---------------------------------------------------------------------------

def etapa_conferir():
    bpy.ops.wm.read_factory_settings(use_empty=True)
    t0 = time.time()
    bpy.ops.import_scene.gltf(filepath=DESTINO)
    _log("importado em %.1fs" % (time.time() - t0))
    tot = 0
    for o in bpy.data.objects:
        if o.type == "MESH":
            me = o.data
            tot += len(me.polygons)
            _log("   %-16s %8d tris %8d verts uv=%s mats=%s loc=%s dims=%s" % (
                o.name, len(me.polygons), len(me.vertices), [u.name for u in me.uv_layers],
                [m.name for m in me.materials], np.round(o.location[:], 3), np.round(o.dimensions[:], 3)))
        else:
            _log("   %-16s %s" % (o.name, o.type))
    _log("total %d tris" % tot)
    for m in bpy.data.materials:
        b = next((n for n in m.node_tree.nodes if n.type == "BSDF_PRINCIPLED"), None) if m.use_nodes else None
        _log("material %s: transmission=%s links=%d" % (m.name, None if b is None else b.inputs["Transmission Weight"].default_value, len(m.node_tree.links) if m.use_nodes else 0))
    for i in bpy.data.images:
        _log("imagem %s %s packed=%s cs=%s" % (i.name, tuple(i.size), i.packed_file is not None, i.colorspace_settings.name))
    mn, mx = _bbox([o for o in bpy.data.objects if o.type == "MESH"])
    _log("envelope reimportado: %s .. %s = %s" % (np.round(mn, 4), np.round(mx, 4), np.round(mx - mn, 4)))


if __name__ == "__main__":
    etapa = _args()
    if etapa in ("separar", "tudo"):
        etapa_separar()
    if etapa in ("acabar", "tudo"):
        etapa_acabar()
    if etapa in ("conferir", "tudo"):
        etapa_conferir()
