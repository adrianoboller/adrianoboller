# Modulo U1 - a impressora do cliente (modelo Meshy AI), limpa, no lugar do
# substituto parametrico (Revisao 3, item 1). O substituto continua em
# scripts/mod_u1_substituto.py.bak como referencia.
#
# O QUE E O MODELO (medido em scratchpad/impressora.blend, ver
# scripts/limpar_impressora.py, que roda uma vez e produz
# assets/impressora_limpa.glb):
# - Malha da Meshy: 1.877.138 triangulos, 5.644 ilhas, TODAS com borda
#   aberta (retalhos sobrepostos, nao cascas). A frente ja aponta para -Y:
#   porta escura com puxador a direita, wordmark 'snapmaker' em cima a
#   esquerda, tela pintada em cima a direita. Quatro bobinas de filamento
#   nas laterais, quatro tubos em arco subindo do fundo do topo, quatro
#   cabecotes estacionados atras, mesa dourada dentro da camara.
# - Limpeza: 2.780 ilhas de lixo descartadas (2.690 triangulos isolados + 90
#   fragmentos a mais de 1,5 mm de qualquer retalho), buracos internos
#   tapados por laco (holes_fill, sem mover vertice), decimacao Collapse
#   com UV ate o orcamento de 400 mil triangulos, escala para o envelope do
#   U1 real (0,584 x 0,499 x 0,730 m), base em z = 0, centro em (0, 0).
# - A ESCALA NAO E UNIFORME: a malha crua e 1,666 x 1,600 x 2,000; com Z
#   mandando (fator 0,365) X e Y passariam do alvo (0,608 e 0,584), entao
#   cada um encolhe so o que passa: X 96%, Y 85% do uniforme. As bobinas
#   ficam 15% mais achatadas em Y. E o menor estiramento que cumpre o
#   envelope; a caixa (interior 0,704 x 0,604) e a coreografia foram
#   calibradas nele. Uniforme seria 0,608 x 0,584 x 0,730 e caberia na
#   caixa com 10 mm em Y - decisao anotada, nao tomada.
# - O envelope inclui tubos e bobinas: o aro do corpo fica a 0,450 m (62% da
#   altura) e os conectores dos tubos a 0,507; os arcos sao o resto. Params 'tubos' e 'bobinas'
#   (padrao True, como o cliente mandou) apagam essas pecas; a escala nao
#   muda com eles - o envelope medido ('dimensoes') muda.
#
# O QUE FOI ACRESCENTADO (o modelo nao tinha, ou tinha pintado):
# - Tela: a Meshy pintou a tela como um mosaico de 58 retalhos pretos - nao
#   serve de emissor (nenhuma UV plana). O plano 'u1.tela' (90 x 60 mm, aro
#   preto de 6 mm) cobre exatamente o retangulo pintado, 2 mm a frente do
#   bisel, com o mesmo material _mat_tela do substituto (boot/UI, barra de
#   progresso, 'ligada'/'standby', especular condicional).
# - Botao e tomada IEC: nao existem no modelo; entram numa coluna na
#   traseira, canto que fica a direita de quem olha a frente (+X), como no
#   substituto e nas fotos do U1 real.
# - Fitas de LED: duas dentro do vao do topo, na frente, com uma area light
#   cada (a malha emissiva nao ilumina no EEVEE sem sonda cozida).
# - Painel traseiro: a janela de tras da Meshy e um VAO aberto (a previa da
#   traseira mostrava o chao da camara e a porta por dentro); entra um
#   acrilico fume de 3 mm cobrindo o vao medido, como o painel transparente
#   do U1 real.
# - Cabecotes e mesa vieram da Meshy como pecas proprias (u1.cabecote.1..4,
#   u1.mesa); 'puxador' e um Empty no puxador pintado (a coreografia so le a
#   posicao dele).
#
# ENTREGA DA MALHA: o GLB limpo tem 25,8 MB (393.991 triangulos, 3 texturas
# 2048^2 PNG embutidas); em zlib+base64 da 28,0 MB, bem acima dos 8 MB do
# criterio de embutir, entao viaja como ARQUIVO ao lado do .blend do
# cliente: assets/impressora_limpa.glb (ou o caminho em
# params['arquivo_impressora']). construir_u1 procura ao lado do .blend
# aberto, na pasta de trabalho e em PASTA_ASSETS, e falha com a lista dos
# caminhos tentados se nao achar.
#
# API: identica a do substituto - construir_u1(cena, colecao_pai, params)
# devolve as mesmas chaves ('raiz', 'corpo', 'tela', 'botao', 'tomada',
# 'cabecotes', 'porta', 'puxador', 'mesa', 'leds', 'led', 'luzes_led',
# 'tubos', 'colecao', 'dimensoes', 'dimensoes_nominais', 'envelope',
# 'placeholders', 'posicao_tela', 'posicao_tomada', 'posicao_botao',
# 'botao_afunda_local', 'materiais', mais 'aro', 'camara', 'logo', 'carro',
# 'hastes' que aqui sao None/[] porque a Meshy nao os separa; 'painel_traseiro'
# e o acrilico que fecha o vao traseiro e 'porta_vidro' e a propria porta); animar_tela, animar_botao, animar_ligar, apagar_tela,
# ponto_no_mundo e fcurves_de sao os mesmos. Todo objeto da colecao 'u1'
# chama-se 'u1.<peca>': a coreografia trata qualquer outro nome como modelo
# de fora e recusa rodar.

import base64
import math
import os
import tempfile
import zlib

import bpy
import bmesh
from mathutils import Vector, Matrix


def fcurves_de(animation_data):
    """Fcurves da acao de um animation_data, em qualquer Blender 4.2+."""
    # Action.fcurves virou legado no 4.4 (slotted actions); no 5.0 pode nao existir.
    try:
        return animation_data.action.fcurves
    except AttributeError:
        slot = animation_data.action_slot
        return animation_data.action.layers[0].strips[0].channelbag(slot).fcurves


NOME = "u1"

# Dimensoes externas oficiais do U1 (m): largura X, profundidade Y, altura Z.
# O GLB limpo foi escalado para este envelope (com tubos e bobinas).
LARGURA = 0.584
PROFUNDIDADE = 0.499
ALTURA = 0.730

PASTA_ASSETS = "/home/user/adrianoboller/anuncio-u1/assets"
ARQUIVO_IMPRESSORA = "impressora_limpa.glb"

# Malha embutida (zlib+base64) - vazia porque o GLB limpo nao coube nos 8 MB;
# fica o mecanismo: se um dia couber, cola-se a string aqui e o arquivo
# deixa de ser necessario.
IMPRESSORA_B64 = ""

# Pontos de ancoragem no GLB limpo, em metros, MEDIDOS por raio contra a
# malha final em limpar_impressora.py (etapa acabar). Mudou o GLB, roda-se a
# etapa de novo e cola-se a saida aqui.
ANCORAS = {
    # Retangulo que a Meshy pintou: 98,5 x 61,3 mm centrado aqui. A tela da
    # Meshy e REBAIXADA num bisel: raios -Y -> +Y numa grade de 2 mm sobre o
    # retangulo dao mediana y -0,2347 (o fundo) e minimo -0,2393 (o bisel,
    # 4,6 mm a frente; 35% dos pontos ficam >= 3 mm a frente do fundo). O y
    # aqui e o do ponto MAIS SALIENTE: com o vidro no fundo, um pedaco do
    # bisel furava a tela (ponto preto na previa do close).
    "tela_centro": (0.1081, -0.2393, 0.4026),   # x, y do ponto mais saliente, z
    "tela_tamanho": (0.090, 0.060),             # vidro (m); aro de 6 mm cobre os 98,5 x 61,3 pintados
    "tras_y": 0.1569,                           # face de tras do corpo na coluna (raio +Y -> -Y)
    "coluna_x": 0.1577,
    "tomada_z": 0.100,
    "botao_z": 0.150,
    "puxador": (0.1437, -0.2494, 0.1862),
    "topo_z": 0.4501,                           # topo do aro (raio -Z em x +-0,20, y -0,156)
    # Fitas de LED: no vao do topo, junto a parede da frente; a 0,156 o raio
    # ja toca o carro do eixo X a z 0,353, entao as fitas ficam na frente dele.
    "vao_topo_y": -0.175,
    # Janela traseira: a Meshy deixou um VAO aberto (raios +Y -> -Y numa grade
    # de 5 mm: vao em x -0,130..0,120, z 0,070..0,230; face em volta a
    # y 0,1569). O U1 real tem painel transparente ai; entra um acrilico.
    "vao_tras": (-0.130, 0.120, 0.070, 0.230),  # x0, x1, z0, z1
    # Centros das pecas u1.cabecote.1..4 do GLB (so usados se faltarem no arquivo).
    "cabecotes": ((-0.1006, 0.1221, 0.4113), (-0.0328, 0.1234, 0.4175), (0.0422, 0.1342, 0.4148), (0.1040, 0.1215, 0.4078)),
}

PADROES = {
    "imagem_boot": os.path.join(PASTA_ASSETS, "tela_boot.png"),
    "imagem_ui": os.path.join(PASTA_ASSETS, "tela_ui.png"),
    # Caminho do GLB limpo; None = procurar (ao lado do .blend, cwd, assets).
    "arquivo_impressora": None,
    "tubos": True,
    "bobinas": True,
    # Sem efeito no modelo da Meshy (a porta e parte do corpo); fica pela
    # compatibilidade com quem passava o parametro ao substituto.
    "porta_aberta_graus": 0.0,
    # Trilho da barra de boot em pixels do PNG 480x320 (x0, x1, y_topo, y_base,
    # y crescendo para baixo, fim exclusivo) e a cor do trilho vazio. Medidos
    # em assets/tela_boot.png: left/right 90 px, bottom 44 px, 3 px de altura.
    "barra_boot_px": (90, 390, 273, 276),
    "cor_trilho_boot": "#2A2A2E",
    # Fracao do especular do vidro da tela que sobra com ela LIGADA: 1,0 =
    # reflete o world como desligada. MEDIDO no substituto a 0,26 m: 0,2 ainda
    # dava L media 19 no terco de cima da UI; 0,05 da 9.
    "reflexo_tela_ligada": 0.05,
    # Tinta do vidro da porta: a Meshy pintou opaco e escuro, o vidro real e
    # fume; com o interior da Meshy ja escuro, #8E9096 (0,28 linear) deixava
    # a camara quase invisivel - #C8CACF transmite 0,58.
    "tinta_porta": "#C8CACF",
}


# ---------------------------------------------------------------------------
# Utilidades de cena
# ---------------------------------------------------------------------------

def limpar_colecao(nome):
    """Remove a sub-colecao e tudo que ela contem; rodar duas vezes nao duplica."""
    col = bpy.data.collections.get(nome)
    if col is None:
        return
    for obj in list(col.all_objects):
        dados = obj.data
        bpy.data.objects.remove(obj, do_unlink=True)
        # O bloco de dados fica orfao e sumiria no proximo save; remover ja
        # evita 'Cube.001' se multiplicando entre rodadas no mesmo arquivo.
        if dados is not None and dados.users == 0:
            if isinstance(dados, bpy.types.Mesh):
                bpy.data.meshes.remove(dados)
            elif isinstance(dados, bpy.types.Curve):
                bpy.data.curves.remove(dados)
            elif isinstance(dados, bpy.types.Light):
                # As area lights das fitas de LED; sem isto viram 'luz.001'.
                bpy.data.lights.remove(dados)
    for filha in list(col.children):
        limpar_colecao(filha.name)
    bpy.data.collections.remove(col)


def _limpar_dados_orfaos():
    """Materiais e imagens 'u1.*' sem usuario, sobras da rodada anterior.

    O importador glTF cria material e imagens novos a cada importacao; sem
    isto a segunda rodada teria 'u1.meshy.001' e 'u1.meshy.cor.001', e o
    teste de idempotencia (mesma contagem de materiais e imagens) falharia.
    """
    for mat in list(bpy.data.materials):
        if mat.name.startswith("u1.") and mat.users == 0:
            bpy.data.materials.remove(mat)
    for img in list(bpy.data.images):
        if img.name.startswith("u1.") and img.users == 0:
            bpy.data.images.remove(img)


def _colecao(nome, pai):
    col = bpy.data.collections.new(nome)
    pai.children.link(col)
    return col


def _cor(hexa, alfa=1.0):
    """'#RRGGBB' sRGB -> tupla linear, que e o que o Principled espera."""
    h = hexa.lstrip("#")
    c = [int(h[i:i + 2], 16) / 255.0 for i in (0, 2, 4)]

    def lin(v):
        return v / 12.92 if v <= 0.04045 else ((v + 0.055) / 1.055) ** 2.4

    return (lin(c[0]), lin(c[1]), lin(c[2]), alfa)


def _novo_objeto(nome, malha, col, pos=(0, 0, 0), pai=None):
    obj = bpy.data.objects.new(nome, malha)
    col.objects.link(obj)
    obj.location = Vector(pos)
    if pai is not None:
        # Os pais aqui estao todos sem rotacao na hora da construcao, entao a
        # inversa e so a translacao; evita depender de um view_layer.update().
        obj.location -= pai.matrix_world.translation
        obj.parent = pai
    return obj


def _empty(nome, col, pos, pai, tamanho=0.03):
    obj = bpy.data.objects.new(nome, None)
    obj.empty_display_type = "PLAIN_AXES"
    obj.empty_display_size = tamanho
    col.objects.link(obj)
    obj.location = Vector(pos)
    obj.parent = pai
    return obj


def _suavizar(malha):
    valores = [True] * len(malha.polygons)
    malha.polygons.foreach_set("use_smooth", valores)


def _malha_caixa(nome, dx, dy, dz):
    bm = bmesh.new()
    bmesh.ops.create_cube(bm, size=1.0)
    bmesh.ops.scale(bm, vec=(dx, dy, dz), verts=bm.verts)
    malha = bpy.data.meshes.new(nome)
    bm.to_mesh(malha)
    bm.free()
    return malha


def _malha_caixa_aberta(nome, dx, dy, dz, lado):
    """Caixa sem a face cuja normal e 'lado': o bolso da tomada, que mostra
    os pinos por dentro sem boolean nenhum."""
    bm = bmesh.new()
    bmesh.ops.create_cube(bm, size=1.0)
    bmesh.ops.scale(bm, vec=(dx, dy, dz), verts=bm.verts)
    alvo = Vector(lado)
    fora = [f for f in bm.faces if f.normal.dot(alvo) > 0.9]
    bmesh.ops.delete(bm, geom=fora, context="FACES")
    malha = bpy.data.meshes.new(nome)
    bm.to_mesh(malha)
    bm.free()
    return malha


def _malha_multicaixas(nome, caixas):
    """Varias caixas (dims, centro) numa malha so: e como se faz uma moldura
    retangular (4 barras) sem boolean."""
    bm = bmesh.new()
    for (dx, dy, dz), (x, y, z) in caixas:
        novo = bmesh.ops.create_cube(bm, size=1.0)["verts"]
        bmesh.ops.scale(bm, vec=(dx, dy, dz), verts=novo)
        bmesh.ops.translate(bm, vec=(x, y, z), verts=novo)
    malha = bpy.data.meshes.new(nome)
    bm.to_mesh(malha)
    bm.free()
    return malha


def _malha_moldura(nome, largura, altura, borda, espessura):
    """Moldura no plano XZ (normal Y): abertura largura x altura, barra 'borda'."""
    hx, hz = largura / 2.0 + borda / 2.0, altura / 2.0 + borda / 2.0
    return _malha_multicaixas(nome, [
        ((largura + 2 * borda, espessura, borda), (0, 0, hz)),
        ((largura + 2 * borda, espessura, borda), (0, 0, -hz)),
        ((borda, espessura, altura), (-hx, 0, 0)),
        ((borda, espessura, altura), (hx, 0, 0)),
    ])


def _chanfro(obj, largura, segmentos=4, nome="chanfro"):
    mod = obj.modifiers.new(nome, "BEVEL")
    mod.width = largura
    mod.segments = segmentos
    mod.limit_method = "ANGLE"
    mod.angle_limit = math.radians(30)
    # Harden normals: as faces planas continuam planas e so o chanfro arredonda;
    # sem isso, malha suave com chanfro vira "bolha" nos closes.
    mod.harden_normals = True
    mod.use_clamp_overlap = True
    return mod


def _caixa(nome, col, dims, pos, mat, chanfro=0.002, segmentos=3, pai=None, suave=True):
    malha = _malha_caixa(nome, *dims)
    if suave:
        _suavizar(malha)
    obj = _novo_objeto(nome, malha, col, pos, pai)
    if mat is not None:
        malha.materials.append(mat)
    if chanfro > 0:
        _chanfro(obj, chanfro, segmentos)
    return obj


def _transparente(mat):
    # DITHERED, nao BLENDED: no EEVEE Next o modo Blended nao passa pelo
    # raytracing (so por sondas), e com Transmission o vidro saia como um
    # painel cinza opaco - foi o que a primeira previa do substituto mostrou.
    # Dithered e o que ve a camara atras do vidro. Em 4.1 e antes o nome era
    # blend_method.
    try:
        mat.surface_render_method = "DITHERED"
    except AttributeError:
        mat.blend_method = "HASHED"
    # Refracao por raytracing no EEVEE Next; o nome antigo era screen refraction.
    for nome in ("use_raytrace_refraction", "use_screen_refraction"):
        try:
            setattr(mat, nome, True)
            break
        except AttributeError:
            continue
    mat.use_backface_culling = False


# ---------------------------------------------------------------------------
# Materiais (todos Principled/Emission, EEVEE primeiro)
# ---------------------------------------------------------------------------

def _material(nome):
    """Pega ou cria e sempre reconstroi os nos: idempotente e sem 'u1.x.001'."""
    mat = bpy.data.materials.get(nome)
    if mat is None:
        mat = bpy.data.materials.new(nome)
    mat.use_nodes = True
    nt = mat.node_tree
    nt.nodes.clear()
    saida = nt.nodes.new("ShaderNodeOutputMaterial")
    bsdf = nt.nodes.new("ShaderNodeBsdfPrincipled")
    saida.location = (400, 0)
    nt.links.new(bsdf.outputs["BSDF"], saida.inputs["Surface"])
    if hasattr(mat, "animation_data") and mat.animation_data:
        mat.animation_data_clear()
    if nt.animation_data:
        nt.animation_data_clear()
    return mat, nt, bsdf


def _entrada(bsdf, nome, valor):
    # Se um nome de socket sumir numa versao futura, o material sai sem aquele
    # ajuste em vez de abortar a construcao inteira.
    ent = bsdf.inputs.get(nome)
    if ent is not None:
        ent.default_value = valor


def _mat_plastico(nome, hexa, aspereza=0.45, coat=0.0):
    mat, nt, bsdf = _material(nome)
    _entrada(bsdf, "Base Color", _cor(hexa))
    _entrada(bsdf, "Roughness", aspereza)
    _entrada(bsdf, "Specular IOR Level", 0.5)
    if coat:
        _entrada(bsdf, "Coat Weight", coat)
        _entrada(bsdf, "Coat Roughness", 0.15)
    return mat


def _mat_vidro(nome, tinta="#5A5C62", aspereza=0.02, espessura=0.004):
    """Vidro de uma superficie so: Transmission com espessura de SLAB.

    A porta da Meshy e uma unica camada de faces, nao um solido. No EEVEE
    Next o modo de espessura padrao (SPHERE, thickness 0 = pelos limites do
    objeto) faz o raio refratado sair longe de onde entrou e a janela vira
    um borrao escuro: MEDIDO no exp_porta (janela da porta, 270x480/8): como
    estava L 0,095; SLAB de 4 mm 0,210; sem vidro nenhum 0,421 - com a tinta
    a 0,58 o esperado do vidro e ~0,24. Se a versao nao tiver o socket
    Thickness, fica como antes.
    """
    mat, nt, bsdf = _material(nome)
    _entrada(bsdf, "Base Color", _cor(tinta))
    _entrada(bsdf, "Transmission Weight", 1.0)
    _entrada(bsdf, "Roughness", aspereza)
    _entrada(bsdf, "IOR", 1.5)
    _transparente(mat)
    saida = next((n for n in nt.nodes if n.type == "OUTPUT_MATERIAL"), None)
    if saida is not None and saida.inputs.get("Thickness") is not None:
        saida.inputs["Thickness"].default_value = espessura
        try:
            mat.thickness_mode = "SLAB"
        except (AttributeError, TypeError):
            pass
    return mat


def _mat_emissivo(nome, hexa, forca=0.0):
    mat, nt, bsdf = _material(nome)
    _entrada(bsdf, "Base Color", _cor(hexa))
    _entrada(bsdf, "Roughness", 0.4)
    _entrada(bsdf, "Emission Color", _cor(hexa))
    _entrada(bsdf, "Emission Strength", forca)
    return mat


def _carregar_imagem(caminho, nome, largura=480, altura=320, cor=(0.0, 0.0, 0.0, 1.0)):
    """Carrega o PNG; sem ele, gera placeholder plano em bpy.data.images e avisa."""
    alternativa = os.path.join(PASTA_ASSETS, NOME + "_" + os.path.basename(caminho))
    for c in (caminho, alternativa):
        if c and os.path.exists(c):
            antiga = bpy.data.images.get(nome)
            if antiga is not None:
                bpy.data.images.remove(antiga)
            img = bpy.data.images.load(c)
            img.name = nome
            # Empacota no .blend: no Windows do cliente o PNG vive em %TEMP%
            # e some na limpeza; a fonte ja era empacotada, a imagem nao.
            try:
                img.pack()
            except RuntimeError as e:
                print("[u1] aviso: nao empacotou %s: %s" % (nome, e))
            if c != caminho:
                print("[u1] aviso: %s nao existe; usando %s" % (caminho, c))
            return img, False
    print("[u1] AVISO: imagem %s nao encontrada; gerando placeholder plano %dx%d" % (caminho, largura, altura))
    img = bpy.data.images.get(nome)
    if img is None or tuple(img.size) != (largura, altura):
        if img is not None:
            bpy.data.images.remove(img)
        img = bpy.data.images.new(nome, largura, altura, alpha=True)
    img.pixels = list(cor) * (largura * altura)
    return img, True


def _mat_tela(nome, img_boot, img_ui, barra_px=(90, 390, 273, 276), cor_trilho="#2A2A2E",
              reflexo_ligada=0.2, vidro=(0.104, 0.070)):
    """Vidro preto brilhante que vira tela: emissao = imagem, forca comeca em 0.

    Nos nomeados 'ligada', 'mistura' e 'progresso' sao o que animar_tela chaveia:
    - ligada:    0 desligada (vidro preto), 1 acesa
    - mistura:   0 imagem de boot, 1 interface (corte seco em q_ui_ini)
    - progresso: 0..1 preenche a barra do boot por mascara em UV
    A imagem ocupa so a area ativa de 3,5" (74,4 x 49,6 mm, 3:2) centrada no
    vidro de 'vidro' metros; a moldura em volta fica preta pela extensao CLIP
    do Image Texture.
    Ligada, o Specular IOR Level cai para 'reflexo_ligada' do valor de vidro:
    o reflexo do world (a faixa rose) competia com a UI e o preto da tela
    media L 45-51 no terco de cima contra 5-6 embaixo (medido no substituto).
    """
    mat, nt, bsdf = _material(nome)
    _entrada(bsdf, "Base Color", (0.005, 0.005, 0.006, 1.0))
    # F0 de vidro comum (0,5) e quase polido: com 0,7/0,06 a tela desligada
    # refletia o mundo como um adesivo cinza uniforme em vez de vidro preto.
    _entrada(bsdf, "Roughness", 0.03)
    _entrada(bsdf, "Specular IOR Level", 0.5)
    _entrada(bsdf, "Emission Strength", 0.0)

    uv = nt.nodes.new("ShaderNodeUVMap")
    uv.location = (-1400, 0)
    mapa = nt.nodes.new("ShaderNodeMapping")
    mapa.location = (-1200, 0)
    ex = vidro[0] / 0.0744
    ey = vidro[1] / 0.0496
    mapa.inputs["Scale"].default_value = (ex, ey, 1.0)
    mapa.inputs["Location"].default_value = (0.5 - 0.5 * ex, 0.5 - 0.5 * ey, 0.0)
    nt.links.new(uv.outputs["UV"], mapa.inputs["Vector"])

    tex_boot = nt.nodes.new("ShaderNodeTexImage")
    tex_boot.name = "imagem_boot"
    tex_boot.image = img_boot
    tex_boot.extension = "CLIP"
    tex_boot.interpolation = "Cubic"
    tex_boot.location = (-900, 200)
    tex_ui = nt.nodes.new("ShaderNodeTexImage")
    tex_ui.name = "imagem_ui"
    tex_ui.image = img_ui
    tex_ui.extension = "CLIP"
    tex_ui.interpolation = "Cubic"
    tex_ui.location = (-900, -200)
    nt.links.new(mapa.outputs["Vector"], tex_boot.inputs["Vector"])
    nt.links.new(mapa.outputs["Vector"], tex_ui.inputs["Vector"])

    # Barra de progresso do boot: o trilho (retangulo 'barra_px' do PNG
    # 480x320) e repintado inteiro com a cor do trilho e depois preenchido de
    # branco da esquerda ate 'progresso'. Repintar o trilho inteiro e o que
    # impede DUAS barras quando o PNG ja vem com preenchimento cozido.
    sep = nt.nodes.new("ShaderNodeSeparateXYZ")
    sep.location = (-900, -450)
    nt.links.new(mapa.outputs["Vector"], sep.inputs["Vector"])
    progresso = nt.nodes.new("ShaderNodeValue")
    progresso.name = "progresso"
    progresso.label = "progresso"
    progresso.outputs[0].default_value = 0.0
    progresso.location = (-900, -650)
    x0, x1, y0, y1 = barra_px
    u0, u1 = x0 / 480.0, x1 / 480.0
    v0, v1 = 1.0 - y1 / 320.0, 1.0 - y0 / 320.0

    def _math(op, a, b, loc):
        n = nt.nodes.new("ShaderNodeMath")
        n.operation = op
        n.location = loc
        if isinstance(a, tuple):
            nt.links.new(a[0].outputs[a[1]], n.inputs[0])
        else:
            n.inputs[0].default_value = a
        if isinstance(b, tuple):
            nt.links.new(b[0].outputs[b[1]], n.inputs[1])
        else:
            n.inputs[1].default_value = b
        return n

    fim = _math("MULTIPLY_ADD", (progresso, 0), u1 - u0, (-700, -650))
    fim.inputs[2].default_value = u0
    dentro_u0 = _math("GREATER_THAN", (sep, "X"), u0, (-700, -450))
    dentro_u1 = _math("LESS_THAN", (sep, "X"), u1, (-500, -450))
    dentro_v0 = _math("GREATER_THAN", (sep, "Y"), v0, (-700, -550))
    dentro_v1 = _math("LESS_THAN", (sep, "Y"), v1, (-500, -550))
    m1 = _math("MULTIPLY", (dentro_u0, 0), (dentro_u1, 0), (-300, -450))
    m2 = _math("MULTIPLY", (dentro_v0, 0), (dentro_v1, 0), (-300, -550))
    trilho = _math("MULTIPLY", (m1, 0), (m2, 0), (-100, -500))
    antes_do_fim = _math("LESS_THAN", (sep, "X"), (fim, 0), (-100, -650))
    cheio = _math("MULTIPLY", (trilho, 0), (antes_do_fim, 0), (100, -600))

    boot_com_trilho = nt.nodes.new("ShaderNodeMixRGB")
    boot_com_trilho.location = (-500, 200)
    boot_com_trilho.inputs["Color2"].default_value = _cor(cor_trilho)
    nt.links.new(tex_boot.outputs["Color"], boot_com_trilho.inputs["Color1"])
    nt.links.new(trilho.outputs["Value"], boot_com_trilho.inputs["Fac"])
    boot_com_barra = nt.nodes.new("ShaderNodeMixRGB")
    boot_com_barra.location = (-350, 250)
    boot_com_barra.inputs["Color2"].default_value = (1.0, 1.0, 1.0, 1.0)
    nt.links.new(boot_com_trilho.outputs["Color"], boot_com_barra.inputs["Color1"])
    nt.links.new(cheio.outputs["Value"], boot_com_barra.inputs["Fac"])

    mistura = nt.nodes.new("ShaderNodeValue")
    mistura.name = "mistura"
    mistura.label = "mistura"
    mistura.outputs[0].default_value = 0.0
    mistura.location = (-500, -50)
    troca = nt.nodes.new("ShaderNodeMixRGB")
    troca.location = (-250, 100)
    nt.links.new(mistura.outputs[0], troca.inputs["Fac"])
    nt.links.new(boot_com_barra.outputs["Color"], troca.inputs["Color1"])
    nt.links.new(tex_ui.outputs["Color"], troca.inputs["Color2"])

    ligada = nt.nodes.new("ShaderNodeValue")
    ligada.name = "ligada"
    ligada.label = "ligada"
    ligada.outputs[0].default_value = 0.0
    ligada.location = (-250, -250)
    # Tela real e bem mais clara que o vidro em volta: 4x para AgX nao apagar.
    forca = _math("MULTIPLY", (ligada, 0), 4.0, (-50, -250))
    # Especular = 0,5 * (1 - (1 - reflexo_ligada) * ligada): vidro preto
    # desligada, quase sem reflexo acesa. Subir a emissao nao resolveria: o
    # reflexo SOMA a radiancia da imagem, e o preto da UI e preto (0) - so
    # tirar o reflexo o deixa preto. Se o socket nao existir numa versao, a
    # tela fica com o especular fixo de vidro, como antes.
    especular_ligada = _math("MULTIPLY_ADD", (ligada, 0), -(1.0 - reflexo_ligada), (-50, -100))
    especular_ligada.inputs[2].default_value = 1.0
    especular = _math("MULTIPLY", (especular_ligada, 0), 0.5, (100, -100))
    for nome_socket in ("Specular IOR Level", "Specular"):
        if bsdf.inputs.get(nome_socket) is not None:
            nt.links.new(especular.outputs["Value"], bsdf.inputs[nome_socket])
            break
    # 'standby': a tela acesa mas sem imagem, logo depois de ligar (cinza
    # escuro, emissao 0,15). E um segundo termo SOMADO a radiancia - a imagem
    # so tem uma entrada de emissao no Principled, entao a forca vai para 1 e
    # a cor carrega os dois termos: imagem*4*ligada + cinza*0,15*standby.
    standby = nt.nodes.new("ShaderNodeValue")
    standby.name = "standby"
    standby.label = "standby"
    standby.outputs[0].default_value = 0.0
    standby.location = (-250, -400)
    forca_standby = _math("MULTIPLY", (standby, 0), 0.15, (-50, -400))
    cor_ligada = nt.nodes.new("ShaderNodeVectorMath")
    cor_ligada.operation = "SCALE"
    cor_ligada.location = (150, -150)
    nt.links.new(troca.outputs["Color"], cor_ligada.inputs[0])
    nt.links.new(forca.outputs["Value"], cor_ligada.inputs["Scale"])
    cor_standby = nt.nodes.new("ShaderNodeVectorMath")
    cor_standby.operation = "SCALE"
    cor_standby.location = (150, -350)
    cor_standby.inputs[0].default_value = (0.80, 0.80, 0.85)
    nt.links.new(forca_standby.outputs["Value"], cor_standby.inputs["Scale"])
    soma = nt.nodes.new("ShaderNodeVectorMath")
    soma.operation = "ADD"
    soma.location = (300, -250)
    nt.links.new(cor_ligada.outputs["Vector"], soma.inputs[0])
    nt.links.new(cor_standby.outputs["Vector"], soma.inputs[1])
    nt.links.new(soma.outputs["Vector"], bsdf.inputs["Emission Color"])
    _entrada(bsdf, "Emission Strength", 1.0)
    return mat


# ---------------------------------------------------------------------------
# A malha da Meshy: achar o GLB e importar
# ---------------------------------------------------------------------------

def _caminhos_candidatos(explicito):
    """Onde procurar o GLB: o caminho dado, ao lado do .blend aberto (com e
    sem 'assets/'), na pasta de trabalho e na pasta de assets daqui."""
    cands = []
    if explicito:
        cands.append(explicito)
    if bpy.data.filepath:
        pasta = os.path.dirname(bpy.data.filepath)
        cands += [os.path.join(pasta, "assets", ARQUIVO_IMPRESSORA), os.path.join(pasta, ARQUIVO_IMPRESSORA)]
    cwd = os.getcwd()
    cands += [os.path.join(cwd, "assets", ARQUIVO_IMPRESSORA), os.path.join(cwd, ARQUIVO_IMPRESSORA),
              os.path.join(PASTA_ASSETS, ARQUIVO_IMPRESSORA)]
    vistos = []
    for c in cands:
        if c not in vistos:
            vistos.append(c)
    return vistos


def _arquivo_impressora(explicito):
    """Caminho do GLB: embutido (se IMPRESSORA_B64 existir, vai para a pasta
    temporaria) ou um dos candidatos; erro claro com a lista tentada."""
    if IMPRESSORA_B64:
        pasta = os.path.join(tempfile.gettempdir(), "anuncio_u1_assets")
        os.makedirs(pasta, exist_ok=True)
        caminho = os.path.join(pasta, ARQUIVO_IMPRESSORA)
        with open(caminho, "wb") as f:
            f.write(zlib.decompress(base64.b64decode(IMPRESSORA_B64)))
        return caminho
    cands = _caminhos_candidatos(explicito)
    for c in cands:
        if os.path.exists(c):
            return c
    raise RuntimeError(
        "[u1] nao achei a malha da impressora (%s). Ponha o arquivo ao lado do seu .blend "
        "(ou em assets/ ao lado dele), ou passe o caminho em params['arquivo_impressora']. "
        "Tentei: %s" % (ARQUIVO_IMPRESSORA, "; ".join(cands)))


def _importar_glb(caminho, col):
    """Importa o GLB e devolve os objetos novos, ja so na colecao 'col'."""
    antes = set(bpy.data.objects)
    bpy.ops.import_scene.gltf(filepath=caminho)
    novos = [o for o in bpy.data.objects if o not in antes]
    for o in novos:
        for c in list(o.users_collection):
            c.objects.unlink(o)
        col.objects.link(o)
        o.select_set(False)
    return novos


def _nomear_imagens_da_meshy(mat):
    """Imagens do material importado com nome fixo (pelo socket que
    alimentam) e empacotadas: o .blend do cliente nao pode depender do GLB
    depois de rodar."""
    if mat is None or not mat.use_nodes:
        return
    nt = mat.node_tree
    for no in nt.nodes:
        if no.type != "TEX_IMAGE" or no.image is None:
            continue
        destino = None
        for link in no.outputs["Color"].links:
            t = link.to_node.type
            if t == "BSDF_PRINCIPLED":
                destino = "cor"
            elif t in ("SEPARATE_COLOR", "SEPRGB"):
                destino = "metal_rugosidade"
            elif t == "NORMAL_MAP":
                destino = "normal"
        if destino is None:
            continue
        nome = "u1.meshy." + destino
        if no.image.name != nome:
            velha = bpy.data.images.get(nome)
            if velha is not None and velha is not no.image:
                bpy.data.images.remove(velha)
            no.image.name = nome
        if no.image.packed_file is None:
            try:
                no.image.pack()
            except RuntimeError as e:
                print("[u1] aviso: nao empacotou %s: %s" % (nome, e))


# ---------------------------------------------------------------------------
# Construcao
# ---------------------------------------------------------------------------

def construir_u1(cena, colecao_pai, params=None):
    """Carrega a impressora limpa na sub-colecao 'u1', acrescenta tela,
    botao, tomada e fitas de LED, e devolve referencias e pontos."""
    p = dict(PADROES)
    if params:
        p.update(params)

    limpar_colecao(NOME)
    _limpar_dados_orfaos()
    col = _colecao(NOME, colecao_pai)

    # Raiz: a coreografia move isto e o U1 inteiro vai junto.
    raiz = bpy.data.objects.new("u1.raiz", None)
    raiz.empty_display_type = "ARROWS"
    raiz.empty_display_size = 0.2
    col.objects.link(raiz)

    caminho = _arquivo_impressora(p["arquivo_impressora"])
    pecas = _importar_glb(caminho, col)
    por_nome = {}
    for o in pecas:
        # O importador pode acrescentar sufixo se o nome ja existir fora da
        # colecao 'u1'; o nome canonico e o do GLB.
        base = o.name.split(".0")[0] if o.name.startswith("u1.") else "u1." + o.name
        o.name = base
        if o.data is not None:
            o.data.name = base
        por_nome[base] = o
        o.parent = raiz
        o.matrix_parent_inverse = Matrix.Identity(4)
        if o.type == "MESH":
            o.data.polygons.foreach_set("use_smooth", [True] * len(o.data.polygons))
    corpo = por_nome.get("u1.corpo")
    if corpo is None:
        raise RuntimeError("[u1] o GLB %s nao tem 'u1.corpo'; e a malha limpa certa?" % caminho)
    print("[u1] impressora carregada de %s: %d pecas, %d triangulos" % (
        caminho, len(pecas), sum(len(o.data.polygons) for o in pecas if o.type == "MESH")))

    # Pecas opcionais: apagadas (nao escondidas) para o envelope medido e a
    # contagem de objetos refletirem o que renderiza.
    for chave, nome in (("tubos", "u1.tubos"), ("bobinas", "u1.bobinas")):
        o = por_nome.get(nome)
        if o is not None and not p[chave]:
            dados = o.data
            bpy.data.objects.remove(o, do_unlink=True)
            if dados is not None and dados.users == 0:
                bpy.data.meshes.remove(dados)
            por_nome.pop(nome)

    # Materiais da Meshy: o importado fica (cor, rugosidade G, metalico B,
    # normal), so ganha nome fixo e imagens empacotadas. O vidro da porta e
    # refeito aqui: Transmission viaja no glTF, mas o modo de render do EEVEE
    # (dithered + refracao raytraced) nao, e sem ele a porta sai opaca.
    m_meshy = None
    for o in pecas:
        if o.type != "MESH" or o.name == "u1.porta":
            continue
        for m in o.data.materials:
            if m is not None:
                m_meshy = m
                break
        if m_meshy is not None:
            break
    if m_meshy is not None:
        m_meshy.name = "u1.meshy"
        _nomear_imagens_da_meshy(m_meshy)
        try:
            m_meshy.surface_render_method = "DITHERED"
        except AttributeError:
            pass
    m_vidro = _mat_vidro("u1.vidro", p["tinta_porta"], 0.02)
    porta = por_nome.get("u1.porta")
    if porta is not None:
        # O GLB traz um 'u1.vidro' proprio; _mat_vidro pegou esse mesmo bloco
        # pelo nome e refez os nos, entao trocar aqui e so garantia.
        porta.data.materials.clear()
        porta.data.materials.append(m_vidro)
    _limpar_dados_orfaos()

    # --- Materiais das pecas acrescentadas
    m_preto = _mat_plastico("u1.preto_fosco", "#15161A", aspereza=0.55)
    m_aro = _mat_plastico("u1.aro", "#0C0D10", aspereza=0.5)
    _entrada(m_aro.node_tree.nodes["Principled BSDF"], "Specular IOR Level", 0.3)
    # Coluna na cor dos paineis da Meshy (branco quente), como no U1 real;
    # cinza-escura destoava do corpo na previa da traseira.
    m_coluna = _mat_plastico("u1.coluna", "#E4E4E0", aspereza=0.45)
    m_camara = _mat_plastico("u1.camara", "#0F1013", aspereza=0.6)
    m_latao = _mat_plastico("u1.latao", "#C9A24A", aspereza=0.35)
    m_latao.node_tree.nodes["Principled BSDF"].inputs["Metallic"].default_value = 1.0
    m_led = _mat_emissivo("u1.led", "#FFF6E8", 0.0)
    m_botao = _mat_emissivo("u1.botao", "#D8241E", 0.0)
    m_botao.node_tree.nodes["Principled BSDF"].inputs["Roughness"].default_value = 0.3
    img_boot, ph_boot = _carregar_imagem(p["imagem_boot"], "u1.tela_boot", cor=(0, 0, 0, 1))
    img_ui, ph_ui = _carregar_imagem(p["imagem_ui"], "u1.tela_ui", cor=(0.09, 0.09, 0.1, 1))
    tela_l, tela_a = ANCORAS["tela_tamanho"]
    m_tela = _mat_tela("u1.tela", img_boot, img_ui, tuple(p["barra_boot_px"]), p["cor_trilho_boot"],
                       p["reflexo_tela_ligada"], (tela_l, tela_a))

    # --- Tela: plano com aro sobre o retangulo que a Meshy pintou --------------
    tx, ty_face, tz = ANCORAS["tela_centro"]
    borda = 0.006
    # Vidro 2 mm a frente do ponto mais saliente do bisel da Meshy (nada fura
    # a tela); o aro tem 8 mm de fundo para descer ate o rebaixo e nao deixar
    # fresta vista de lado.
    tela_malha = _malha_caixa("u1.tela", tela_l, 0.002, tela_a)
    tela = _novo_objeto("u1.tela", tela_malha, col, (tx, ty_face - 0.0012, tz), raiz)
    tela_malha.materials.append(m_tela)
    # UV so na face da frente (-Y): u cresce com x, v cresce com z.
    uv_layer = tela_malha.uv_layers.new(name="UVMap")
    for poly in tela_malha.polygons:
        for li in poly.loop_indices:
            v = tela_malha.vertices[tela_malha.loops[li].vertex_index].co
            uv_layer.data[li].uv = (v.x / tela_l + 0.5, v.z / tela_a + 0.5)
    _suavizar(tela_malha)
    _chanfro(tela, 0.0004, 2)
    aro_malha = _malha_moldura("u1.tela.aro", tela_l, tela_a, borda, 0.008)
    aro = _novo_objeto("u1.tela.aro", aro_malha, col, (tx, ty_face - 0.0012 + 0.0025, tz), raiz)
    aro_malha.materials.append(m_aro)
    _chanfro(aro, 0.0006, 2)
    centro_tela = Vector((tx, ty_face - 0.0022, tz))

    # --- Traseira: coluna com botao e tomada IEC ------------------------------
    col_x, tras = ANCORAS["coluna_x"], ANCORAS["tras_y"]
    esp_coluna = 0.016
    z_tomada, z_botao = ANCORAS["tomada_z"], ANCORAS["botao_z"]
    z_col = (z_tomada + z_botao) / 2.0
    coluna = _caixa("u1.coluna", col, (0.060, esp_coluna, 0.140), (col_x, tras + esp_coluna / 2.0 - 0.002, z_col), m_coluna,
                    chanfro=0.004, segmentos=4, pai=raiz)
    face_coluna = tras + esp_coluna - 0.002
    # Botao gangorra vermelho numa moldura preta.
    botao_aro = _novo_objeto("u1.botao.aro", _malha_moldura("u1.botao.aro", 0.017, 0.025, 0.0035, 0.004), col,
                             (col_x, face_coluna + 0.001, z_botao), raiz)
    botao_aro.data.materials.append(m_preto)
    botao = _caixa("u1.botao", col, (0.016, 0.006, 0.024), (col_x, face_coluna + 0.003, z_botao), m_botao,
                   chanfro=0.0015, segmentos=2, pai=raiz)
    _caixa("u1.botao.traco", col, (0.001, 0.001, 0.007), (col_x, face_coluna + 0.0065, z_botao + 0.006), m_coluna,
           chanfro=0, pai=raiz)
    # Painel traseiro: acrilico fume fechando o vao que a Meshy deixou aberto
    # (6 mm de sobra em volta, encostado na face de tras).
    x0, x1, z0, z1 = ANCORAS["vao_tras"]
    m_acrilico = _mat_vidro("u1.acrilico_traseiro", "#C4C6CC", 0.03, espessura=0.003)
    painel_traseiro = _caixa("u1.painel_traseiro", col, (x1 - x0 + 0.012, 0.003, z1 - z0 + 0.012),
                             ((x0 + x1) / 2.0, tras - 0.0015, (z0 + z1) / 2.0), m_acrilico, chanfro=0.0008, segmentos=2, pai=raiz)
    centro_botao = Vector((col_x, face_coluna + 0.006, z_botao))
    # Tomada IEC C14: carcaca preta 10 mm SALIENTE da coluna (como o encaixe
    # real), com o bolso escuro aberto para tras (caixa sem a face +Y) e 3
    # pinos de latao dentro. Tudo a frente da face da coluna, sem boolean:
    # com o bolso entrando na coluna, a face branca dela aparecia pela boca
    # (visto na previa da traseira).
    prof = 0.010
    tomada_aro = _novo_objeto("u1.tomada.aro", _malha_moldura("u1.tomada.aro", 0.024, 0.018, 0.004, prof + 0.001), col,
                              (col_x, face_coluna + prof / 2.0, z_tomada), raiz)
    tomada_aro.data.materials.append(m_preto)
    _chanfro(tomada_aro, 0.0006, 2)
    tomada = _novo_objeto("u1.tomada", _malha_caixa_aberta("u1.tomada", 0.024, prof, 0.018, (0, 1, 0)), col,
                          (col_x, face_coluna + 0.0005 + prof / 2.0, z_tomada), raiz)
    tomada.data.materials.append(m_camara)
    for j, (px, pz) in enumerate(((-0.007, -0.001), (0.007, -0.001), (0.0, 0.004))):
        _caixa("u1.tomada.pino.%d" % (j + 1), col, (0.0018, 0.007, 0.0045), (col_x + px, face_coluna + 0.0045, z_tomada + pz), m_latao,
               chanfro=0.0003, segmentos=1, pai=raiz)
    ponto_tomada = Vector((col_x, face_coluna + prof + 0.0015, z_tomada))

    # --- Fitas de LED no vao do topo, na frente, com uma area light cada ------
    leds, luzes = [], []
    z_led = ANCORAS["topo_z"] - 0.012
    y_led = ANCORAS["vao_topo_y"]
    for i, x in enumerate((-0.10, 0.10)):
        leds.append(_caixa("u1.led.%d" % (i + 1), col, (0.14, 0.010, 0.003), (x, y_led, z_led), m_led, chanfro=0.0005, segmentos=1, pai=raiz))
        luzes.append(_luz_de_fita("u1.led.luz.%d" % (i + 1), col, Vector((x, y_led, z_led - 0.004)), (0.14, 0.010), raiz))

    # --- Pontos que a coreografia mira: puxador e cabecotes -------------------
    puxador = _empty("u1.puxador", col, ANCORAS["puxador"], raiz)
    cabecotes = []
    for n in range(4):
        o = por_nome.get("u1.cabecote.%d" % (n + 1))
        if o is None:
            o = _empty("u1.cabecote.%d" % (n + 1), col, ANCORAS["cabecotes"][n], raiz)
        cabecotes.append(o)
    mesa = por_nome.get("u1.mesa")
    if mesa is None:
        mesa = _empty("u1.mesa", col, (0.0, 0.0, 0.05), raiz)

    cena.view_layers[0].update() if hasattr(cena, "view_layers") else None
    envelope_min, envelope_max = _envelope(col)
    dimensoes = tuple(envelope_max[i] - envelope_min[i] for i in range(3))

    return {
        "raiz": raiz,
        "corpo": corpo,
        # A Meshy nao separa aro, camara, logo, carro nem painel traseiro;
        # ficam None para quem so testa a chave, e 'hastes' fica vazio.
        "aro": None,
        "camara": None,
        "tela": tela,
        "logo": None,
        "botao": botao,
        "tomada": tomada,
        "cabecotes": cabecotes,
        "carro": None,
        "hastes": [],
        "porta": porta,
        "porta_vidro": porta,
        "puxador": puxador,
        "mesa": mesa,
        "painel_traseiro": painel_traseiro,
        "leds": leds,
        "led": leds[0],
        # Area lights das fitas (hide_render=True e 0 W ate animar_ligar).
        "luzes_led": luzes,
        "tubos": [por_nome["u1.tubos"]] if "u1.tubos" in por_nome else [],
        "bobinas": [por_nome["u1.bobinas"]] if "u1.bobinas" in por_nome else [],
        "pecas": por_nome,
        "arquivo": caminho,
        "colecao": col,
        # Envelope MEDIDO da malha avaliada (com modificadores), nao o nominal.
        "dimensoes": dimensoes,
        "dimensoes_nominais": (LARGURA, PROFUNDIDADE, ALTURA),
        "envelope": (envelope_min, envelope_max),
        "placeholders": {"boot": ph_boot, "ui": ph_ui},
        # Pontos em coordenadas de mundo (a raiz esta na identidade ao construir).
        "posicao_tela": {"centro": centro_tela.copy(), "normal": Vector((0, -1, 0))},
        "posicao_tomada": {"ponto": ponto_tomada.copy(), "direcao": Vector((0, -1, 0)), "normal": Vector((0, 1, 0))},
        "posicao_botao": {"centro": centro_botao.copy(), "normal": Vector((0, 1, 0))},
        # Direcao em que o botao afunda, no espaco local dele (a coreografia
        # pode girar a raiz; a animacao continua certa porque e local).
        "botao_afunda_local": Vector((0, -1, 0)),
        "materiais": {"tela": m_tela, "led": m_led, "botao": m_botao, "aro": m_aro, "meshy": m_meshy, "vidro": m_vidro},
    }


def _luz_de_fita(nome, col, pos, tamanho, raiz):
    """Area light retangular do tamanho da fita, apontando para baixo, escondida e a 0 W."""
    dados = bpy.data.lights.new(nome, "AREA")
    dados.shape = "RECTANGLE"
    dados.size, dados.size_y = tamanho
    dados.energy = 0.0
    dados.color = _cor("#FFF6E8")[:3]
    luz = bpy.data.objects.new(nome, dados)
    col.objects.link(luz)
    luz.location = pos
    luz.parent = raiz
    luz.hide_render = True
    return luz


def _envelope(col):
    """Caixa envolvente, em mundo, de tudo que renderiza na colecao (malha avaliada)."""
    dg = bpy.context.evaluated_depsgraph_get()
    mn = Vector((1e9, 1e9, 1e9))
    mx = Vector((-1e9, -1e9, -1e9))
    for obj in col.all_objects:
        if obj.hide_render or obj.type not in ("MESH", "FONT", "CURVE"):
            continue
        ev = obj.evaluated_get(dg)
        for canto in ev.bound_box:
            w = ev.matrix_world @ Vector(canto)
            for i in range(3):
                mn[i] = min(mn[i], w[i])
                mx[i] = max(mx[i], w[i])
    return mn, mx


def ponto_no_mundo(objs, chave, campo="centro"):
    """Ponto/direcao de objs[chave] levado pela matriz atual da raiz.

    Os pontos do dict sao medidos com a raiz na identidade; depois que a
    coreografia move o U1, e isto que devolve o lugar certo. Direcoes
    ('normal', 'direcao') giram sem transladar.
    """
    m = objs["raiz"].matrix_world
    v = objs[chave][campo]
    if campo in ("normal", "direcao"):
        return (m.to_3x3() @ v).normalized()
    return m @ v


# ---------------------------------------------------------------------------
# Animacao (so nos proprios objetos; Bezier suave) - igual ao substituto
# ---------------------------------------------------------------------------

def _suavizar_fcurves(anim, quadros, easing="EASE_IN_OUT", interp="BEZIER"):
    if anim is None or anim.action is None:
        return
    for fc in fcurves_de(anim):
        for kp in fc.keyframe_points:
            if int(round(kp.co.x)) in quadros:
                kp.interpolation = interp
                kp.easing = easing


def _no_valor(mat, nome):
    """Acha o Value node chaveavel; None se o material for o do cliente."""
    if mat is None or not mat.use_nodes:
        return None
    return mat.node_tree.nodes.get(nome)


def _chave_socket(socket, valor, quadro):
    socket.default_value = valor
    socket.keyframe_insert("default_value", frame=quadro)


def _socket_forca_emissao(nt):
    """Socket de forca de emissao do no LIGADO ao Material Output.

    Parte do Output ativo, segue Surface e desce por Mix/Add Shader ate um
    Emission ('Strength') ou Principled ('Emission Strength'). So sem link
    algum cai na varredura por ordem de nos - a varredura sozinha chaveava um
    Principled sobrando desligado e a tela do cliente nunca acendia.
    """
    saida = None
    try:
        saida = nt.get_output_node("ALL")
    except (AttributeError, TypeError):
        pass
    if saida is None:
        outs = [n for n in nt.nodes if n.type == "OUTPUT_MATERIAL"]
        saida = next((n for n in outs if n.is_active_output), outs[0] if outs else None)

    def _do_no(no, vistos):
        if no is None or no in vistos:
            return None
        vistos.add(no)
        if no.type == "EMISSION":
            return no.inputs.get("Strength")
        if no.type == "BSDF_PRINCIPLED":
            return no.inputs.get("Emission Strength")
        for ent in no.inputs:
            if ent.type == "SHADER" and ent.is_linked:
                s = _do_no(ent.links[0].from_node, vistos)
                if s is not None:
                    return s
        return None

    if saida is not None:
        surf = saida.inputs.get("Surface")
        if surf is not None and surf.is_linked:
            s = _do_no(surf.links[0].from_node, set())
            if s is not None:
                return s
    for no in nt.nodes:
        s = no.inputs.get("Emission Strength") if no.type == "BSDF_PRINCIPLED" else (no.inputs.get("Strength") if no.type == "EMISSION" else None)
        if s is not None:
            return s
    return None


def _acender(mat, q_ini, q_fim, forca, easing="EASE_IN_OUT", de=0.0):
    """Forca de emissao do material: 'de' em q_ini -> 'forca' em q_fim (Bezier)."""
    if mat is None or not mat.use_nodes:
        return False
    s = _socket_forca_emissao(mat.node_tree)
    if s is None:
        return False
    _chave_socket(s, de, q_ini)
    _chave_socket(s, forca, q_fim)
    _suavizar_fcurves(mat.node_tree.animation_data, {q_ini, q_fim}, easing)
    return True


def _afundar_botao(objs, q_ini, q_fim, easing, profundidade):
    """Curso do botao (afunda e volta); devolve o quadro do fundo do curso."""
    botao = objs["botao"]
    eixo = objs.get("botao_afunda_local", Vector((0, -1, 0)))
    meio = (q_ini + q_fim) // 2
    repouso = botao.location.copy()
    botao.location = repouso
    botao.keyframe_insert("location", frame=q_ini)
    botao.location = repouso + eixo * profundidade
    botao.keyframe_insert("location", frame=meio)
    botao.location = repouso
    botao.keyframe_insert("location", frame=q_fim)
    _suavizar_fcurves(botao.animation_data, {q_ini, meio, q_fim}, easing)
    return meio


def animar_tela(objs, q_boot_ini, q_ui_ini, q_fim, easing="EASE_IN_OUT", duracao_fade=6):
    """Desligada -> boot (fade rapido + barra) -> corte para a UI em q_ui_ini.

    Le o material de objs['tela']. Com o material daqui, usa os Value nodes
    'ligada', 'mistura' e 'progresso'. Com um material de fora, cai para o que
    existir: chaveia a Emission Strength do Principled/Emission encontrado.
    """
    tela = objs["tela"]
    mat = tela.active_material
    nt = mat.node_tree if mat and mat.use_nodes else None
    if nt is None:
        print("[u1] animar_tela: material da tela sem nos; nada a animar")
        return
    ligada = _no_valor(mat, "ligada")
    mistura = _no_valor(mat, "mistura")
    progresso = _no_valor(mat, "progresso")

    if ligada is not None:
        s = ligada.outputs[0]
        _chave_socket(s, 0.0, q_boot_ini)
        _chave_socket(s, 1.0, q_boot_ini + duracao_fade)
        _chave_socket(s, 1.0, q_fim)
        # O standby (se animar_ligar o acendeu) apaga junto com a subida do
        # boot; sem animar_ligar antes, e 0 -> 0 e nao muda nada.
        standby = _no_valor(mat, "standby")
        if standby is not None:
            s = standby.outputs[0]
            _chave_socket(s, s.default_value, q_boot_ini)
            _chave_socket(s, 0.0, q_boot_ini + duracao_fade)
    else:
        # Material do cliente: a forca do no ligado ao Output.
        s = _socket_forca_emissao(nt)
        if s is not None:
            _chave_socket(s, 0.0, q_boot_ini)
            _chave_socket(s, 4.0, q_boot_ini + duracao_fade)
        else:
            print("[u1] animar_tela: material sem forca de emissao; tela nao acende")
    if progresso is not None:
        s = progresso.outputs[0]
        _chave_socket(s, 0.0, q_boot_ini + 2)
        _chave_socket(s, 1.0, max(q_boot_ini + 3, q_ui_ini - 2))
    if mistura is not None:
        s = mistura.outputs[0]
        _chave_socket(s, 0.0, q_ui_ini - 1)
        _chave_socket(s, 1.0, q_ui_ini)

    anim = nt.animation_data
    _suavizar_fcurves(anim, {q_boot_ini, q_boot_ini + duracao_fade, q_fim, q_boot_ini + 2, max(q_boot_ini + 3, q_ui_ini - 2)}, easing)
    # A troca boot -> UI e um corte seco, como na maquina real.
    if mistura is not None and anim and anim.action:
        for fc in fcurves_de(anim):
            if "mistura" in fc.data_path:
                for kp in fc.keyframe_points:
                    kp.interpolation = "CONSTANT"


def _materiais_de_led(objs):
    """Materiais das fitas/LED: os devolvidos pelo modulo ou o do objeto 'led' do cliente."""
    mats = objs.get("materiais", {})
    alvos = [m for m in (mats.get("led"), mats.get("botao")) if m is not None]
    if not alvos and objs.get("led") is not None and objs["led"].active_material:
        alvos = [objs["led"].active_material]
    return alvos


def animar_botao(objs, q_ini, q_fim, easing="EASE_IN_OUT", profundidade=0.002):
    """Botao afunda 2 mm e volta; LEDs da camara e a janela do botao acendem.

    Mantida como na rodada 1 (curso, quadros e forcas iguais). Para ligar como
    evento de luz use animar_ligar.
    """
    meio = _afundar_botao(objs, q_ini, q_fim, easing, profundidade)
    for m in _materiais_de_led(objs):
        _acender(m, meio, q_fim, 12.0 if m.name == "u1.led" else 3.0, easing)


def animar_ligar(objs, quadro_ini, quadro_fim, easing="EASE_IN_OUT", profundidade=0.002,
                 forca_fitas=4.0, energia_luz=60.0, duracao=6, standby=1.0):
    """Ligar como evento de luz: botao, janela do botao, fitas, luzes internas e tela em standby.

    O botao afunda entre quadro_ini e quadro_fim como em animar_botao; no fundo
    do curso (meio) comeca a luz: fitas de LED 0 -> forca_fitas e area lights
    das fitas 0 -> energia_luz em 'duracao' quadros (Bezier), janela do botao
    0 -> 3, e a tela vai de preto ao cinza 'standby' (Value 'standby' 0 ->
    standby, emissao 0,15). As luzes ficam escondidas do render ate meio-1.
    Com o modelo do cliente sem fitas nem luzes, cria UMA area light no topo
    do envelope (objs['envelope']) e a guarda em objs['luzes_led'].
    """
    if objs.get("botao") is not None:
        q0 = _afundar_botao(objs, quadro_ini, quadro_fim, easing, profundidade)
    else:
        q0 = (quadro_ini + quadro_fim) // 2
    q1 = q0 + max(1, duracao)

    mats = objs.get("materiais", {})
    if mats.get("botao") is not None:
        _acender(mats["botao"], q0, q1, 3.0, easing)
    for m in [m for m in _materiais_de_led(objs) if m is not mats.get("botao")]:
        _acender(m, q0, q1, forca_fitas, easing)

    luzes = list(objs.get("luzes_led") or [])
    if not luzes and objs.get("envelope") is not None and objs.get("raiz") is not None:
        mn, mx = objs["envelope"]
        centro = Vector(((mn.x + mx.x) / 2, (mn.y + mx.y) / 2, mx.z - 0.04))
        col = objs.get("colecao") or objs["raiz"].users_collection[0]
        luzes = [_luz_de_fita("u1.led.luz.1", col, centro, (0.30, 0.02), objs["raiz"])]
        objs["luzes_led"] = luzes
        print("[u1] animar_ligar: modelo sem fitas de LED; criada uma area light no topo do envelope")
    for luz in luzes:
        luz.hide_render = True
        luz.keyframe_insert("hide_render", frame=q0 - 1)
        luz.hide_render = False
        luz.keyframe_insert("hide_render", frame=q0)
        luz.data.energy = 0.0
        luz.data.keyframe_insert("energy", frame=q0)
        luz.data.energy = energia_luz
        luz.data.keyframe_insert("energy", frame=q1)
        _suavizar_fcurves(luz.data.animation_data, {q0, q1}, easing)

    mat_tela = mats.get("tela")
    if mat_tela is None and objs.get("tela") is not None:
        mat_tela = objs["tela"].active_material
    no = _no_valor(mat_tela, "standby")
    if no is not None:
        s = no.outputs[0]
        _chave_socket(s, 0.0, q0)
        _chave_socket(s, standby, q1)
        _suavizar_fcurves(mat_tela.node_tree.animation_data, {q0, q1}, easing)
    else:
        print("[u1] animar_ligar: tela sem no 'standby' (material do cliente); a tela so acende em animar_tela")


def apagar_tela(objs, quadro):
    """Chave direta: 'ligada' e 'standby' a 0 em 'quadro' (a maquina volta desligada para a caixa)."""
    mat = objs.get("materiais", {}).get("tela")
    if mat is None and objs.get("tela") is not None:
        mat = objs["tela"].active_material
    for nome in ("ligada", "standby"):
        no = _no_valor(mat, nome)
        if no is not None:
            _chave_socket(no.outputs[0], 0.0, quadro)
