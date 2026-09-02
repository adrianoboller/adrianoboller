# Modulo CAIXA do anuncio do Snapmaker U1.
#
# Caixa de produto premium (estilo caixa de iPhone): corpo rigido + tampa
# solta que levanta, logo IMPRESSA no topo da tampa (decal no material, sem
# relevo) e espuminhas soltas por dentro. So definicoes aqui - nada roda no
# import. Quem monta a cena e chama as animacoes e mod_coreografia.py; quem
# prova este modulo sozinho e teste_caixa.py.
#
# Eixos e medidas seguem docs/ESPECIFICACAO.md: metros, Z para cima, frente
# em -Y, origem no centro da base da caixa.
#
# Achado na previa (EEVEE Next 4.2): a parede de 8 mm deixa a luz das area
# lights VAZAR para a face interna da tampa como um chuvisco branco - nao e
# ruido de amostragem (persistiu a 48 amostras, sem bump e com mais raios de
# sombra). Some com luz.use_shadow_jitter = True nas luzes. Quem monta as
# luzes (modulo ambiente) precisa ligar isso; o teste daqui liga.
#
# Rodada 2 (o que a revisao mediu e o que foi feito):
# - Logo palida: a revisao mediu a engrenagem cinza a 136 sRGB (fonte 56) e
#   apontou o Sheen 0,35 branco da tampa. Ele entra na conta, mas nao e o
#   grosso: medido aqui no topo ortografico, zerar sheen E specular no
#   material inteiro moveu o cinza de 164 para 163. A causa medida e a
#   EXPOSICAO sob o AgX: o papel branco esta a ~7x acima de 1,0 linear
#   (albedo constante 0,04 rende 147 sRGB; 0,18 rende 212; 0,88 rende 243),
#   e o ombro do AgX comprime a diferenca papel/tinta. O modulo nao manda na
#   luz da cena, entao a tinta e escurecida por um no Gamma ('gamma_logo',
#   medido no teste) - e o papel fica com Sheen 0,08 na cor do papel e, na
#   area do decal, Sheen 0 e specular 0 (no close do beat 7 o reflexo das
#   luzes na tinta era um piso de ~130 sRGB; medido, ver SPEC_TINTA). O
#   multiplicador de saturacao 1,3 saiu (padrao 1,0): nao devolvia o escuro.
# - Logo "pequena" no beat 1: medido de cima com camera ortografica, a
#   engrenagem ocupa 44,9% da largura da tampa - o calculo pela alfa esta
#   certo. O que a revisao viu (~28-33%) e geometria: no beat 1 a caixa esta
#   girada ~35 graus e o topo aparece pela DIAGONAL (~1,35x a largura),
#   enquanto a engrenagem redonda nao cresce com o giro. Nao e defeito do
#   modulo; quem quiser a logo maior nesse plano sobe 'largura_logo'.
# - Espuma: flocos de 6 a 10 cm (raio 0,03-0,05), 48 pecas, ruido em duas
#   oitavas, cantos amassados por planos aleatorios, chanfro e subsurface
#   0,15 - a 2,5 m os de 3-6 cm liam como arroz.
# - Afundamento no topo do U1: a folga usava 0,7*raio, e o raio era o do
#   eixo maior. Agora a base do floco vem da extensao REAL do floco girado
#   (max |z| dos vertices) mais 3 mm; se o floco nao cabe na camada, ele e
#   reduzido em vez de atravessar a tampa.
# - Materiais e actions nao vazam mais: bpy.data.materials.get(nome) com
#   os nos reconstruidos, e limpar_colecao remove a action orfa.
# - Flocos do vao lateral saiam ATRAVES da parede (4 medidos na revisao):
#   o arco agora so ganha deslocamento horizontal depois de passar da boca
#   do corpo (instante tirado da propria parabola); o teste mede 0 furos.
# - A imagem da logo e empacotada (img.pack()) para o .blend ser autocontido.

import math
import random

import bmesh
import bpy
from mathutils import Euler, Vector, noise

NOME = "caixa"

# Medidas da especificacao. O interior e o U1 mais folga de espuma; tudo o
# mais deriva daqui, para que mudar uma medida nao exija cacar numero solto.
PARAMS_PADRAO = {
    "interior": (0.66, 0.58, 0.80),
    "parede": 0.008,
    "altura_tampa": 0.12,
    "folga": 0.002,
    "chanfro": 0.0025,
    "segmentos_chanfro": 3,
    "cor": "clara",
    "logo": "logo_engineprint.png",   # relativo a assets/; ou caminho absoluto
    "largura_logo": 0.45,             # fracao da largura externa da tampa
    # Saturacao 1,3 foi a primeira tentativa contra o AgX e nao devolvia o
    # escuro do cinza (medido); fica em 1,0 e a correcao e por gamma.
    "saturacao_logo": 1.0,
    # Gamma > 1 escurece a tinta antes do AgX. Medido no teste (topo
    # ortografico, papel a 243 sRGB): gamma 1,0 -> cinza 164; 1,5 -> 104;
    # 1,7 -> 84; 2,0 -> 58 (a fonte e 56); o laranja em 2,0 mede (237,135,6).
    # Depende da exposicao da cena: quem iluminar diferente mede de novo e
    # passa outro valor.
    "gamma_logo": 2.0,
    "n_espumas": 48,
    "raio_espuma": (0.03, 0.05),       # meio eixo maior: flocos de 6 a 10 cm
    "semente": 7,
    # Onde o U1 vai ficar dentro da caixa: as espumas se arrumam em volta
    # desse volume, mesmo sem o U1 existir na cena de teste.
    "u1": (0.584, 0.499, 0.730),
}

CORES = {
    "clara": (0xF2, 0xED, 0xE6),
    "escura": (0x14, 0x14, 0x16),
}
COR_ESPUMA = (0xF6, 0xF6, 0xF4)

FPS = 30.0


# ---------------------------------------------------------------- utilidades

def _srgb_para_linear(c):
    # As cores da paleta estao em sRGB (hex); o Principled quer linear.
    c = c / 255.0
    return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4


def cor_linear(hex3):
    return tuple(_srgb_para_linear(v) for v in hex3) + (1.0,)


def limpar_colecao(nome):
    """Remove a sub-colecao <nome> e tudo o que ela contem (idempotencia)."""
    col = bpy.data.collections.get(nome)
    if col is None:
        return
    for obj in list(col.all_objects):
        dados = obj.data
        acao = obj.animation_data.action if obj.animation_data else None
        bpy.data.objects.remove(obj, do_unlink=True)
        # Malha orfa fica no arquivo ate o proximo salvar/recarregar;
        # apagar aqui evita acumular 'caixa.corpo.001' nas rodadas seguintes.
        if dados is not None and dados.users == 0:
            if isinstance(dados, bpy.types.Mesh):
                bpy.data.meshes.remove(dados)
        # A action da animacao tambem ficava orfa (65 por rodada, medido na
        # revisao): na aba Scripting do cliente isso acumula lixo no .blend.
        if acao is not None and acao.users == 0:
            bpy.data.actions.remove(acao)
    for filha in list(col.children):
        limpar_colecao(filha.name)
    bpy.data.collections.remove(col)


def _colecao(cena, colecao_pai, nome):
    if colecao_pai is None:
        colecao_pai = bpy.data.collections.get("ANUNCIO")
        if colecao_pai is None:
            colecao_pai = bpy.data.collections.new("ANUNCIO")
        if colecao_pai.name not in cena.collection.children:
            cena.collection.children.link(colecao_pai)
    col = bpy.data.collections.new(nome)
    colecao_pai.children.link(col)
    return col


def _caminho_asset(nome_arquivo):
    import os
    if os.path.isabs(nome_arquivo):
        return nome_arquivo
    # scripts/ e assets/ sao irmaos na raiz do projeto.
    try:
        raiz = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    except NameError:
        # Aba Scripting do Blender sem arquivo (colado direto): nao existe
        # __file__; vale a pasta do .blend, ou a de trabalho se nem ele ha.
        raiz = os.path.dirname(bpy.data.filepath) if bpy.data.filepath else os.getcwd()
    return os.path.join(raiz, "assets", nome_arquivo)


def _sombrear_suave(malha, suave=True):
    malha.polygons.foreach_set("use_smooth", [suave] * len(malha.polygons))
    malha.update()


def _chanfro(obj, largura, segmentos):
    # Em close, a aresta e o que separa premium de amador: o chanfro e
    # pequeno, mas obrigatorio. Limite por angulo para nao chanfrar a borda
    # fina da parede (8 mm) duas vezes.
    mod = obj.modifiers.new("chanfro", "BEVEL")
    mod.width = largura
    mod.segments = segmentos
    mod.limit_method = "ANGLE"
    mod.angle_limit = math.radians(60.0)
    try:
        # Sem isto, as faces grandes planas ganham gradiente de sombreamento
        # (normais suaves inclinadas pelo chanfro). Existe desde o 2.9x.
        mod.harden_normals = True
    except AttributeError:
        pass
    return mod


# ---------------------------------------------------------------- geometria

def _caixa_oca(nome, ext, parede, altura_total, aberta_em_cima, z_origem):
    """Malha de caixa oca com uma face aberta: 5 faces externas, 5 internas
    e o anel da borda. Construida a mao em bmesh, nao por boolean, para o
    chanfro cair em arestas limpas."""
    lx, ly = ext[0] / 2.0, ext[1] / 2.0
    ix, iy = lx - parede, ly - parede
    z0, z1 = 0.0, altura_total
    if aberta_em_cima:
        z_fundo_int = z0 + parede   # fundo fechado embaixo
        z_borda = z1
    else:
        z_fundo_int = z1 - parede   # "fundo" e o topo da tampa
        z_borda = z0

    bm = bmesh.new()

    def anel(x, y, z):
        return [bm.verts.new((sx * x, sy * y, z)) for sx, sy in ((-1, -1), (1, -1), (1, 1), (-1, 1))]

    ext_baixo = anel(lx, ly, z0)
    ext_cima = anel(lx, ly, z1)
    int_baixo = anel(ix, iy, z0 if not aberta_em_cima else z_fundo_int)
    int_cima = anel(ix, iy, z1 if aberta_em_cima else z_fundo_int)

    def lados(a, b, invertido=False):
        for i in range(4):
            j = (i + 1) % 4
            f = (a[i], a[j], b[j], b[i])
            bm.faces.new(f[::-1] if invertido else f)

    lados(ext_baixo, ext_cima)
    lados(int_baixo, int_cima, invertido=True)
    if aberta_em_cima:
        bm.faces.new(ext_baixo[::-1])          # fundo externo
        bm.faces.new(int_baixo)                # fundo interno
        for i in range(4):                     # anel da borda em cima
            j = (i + 1) % 4
            bm.faces.new((ext_cima[i], int_cima[i], int_cima[j], ext_cima[j]))
    else:
        bm.faces.new(ext_cima)                 # topo externo
        bm.faces.new(int_cima[::-1])           # topo interno
        for i in range(4):                     # anel da borda embaixo
            j = (i + 1) % 4
            bm.faces.new((ext_baixo[j], int_baixo[j], int_baixo[i], ext_baixo[i]))

    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    # Origem no ponto pedido: a tampa gira em torno do proprio centro.
    bmesh.ops.translate(bm, verts=bm.verts, vec=(0.0, 0.0, -z_origem))
    malha = bpy.data.meshes.new(nome)
    bm.to_mesh(malha)
    bm.free()
    return malha


def _malha_espuma(nome, rng, raio):
    """Floco de espuma: icosfera amassada por ruido em duas oitavas, alongada
    de forma desigual e com cantos AMASSADOS (vertices alem de planos
    aleatorios sao empurrados para o plano - facetas chatas como num pedaco
    de EPS quebrado). Icosfera lisa e o que lia como arroz a 2,5 m."""
    bm = bmesh.new()
    bmesh.ops.create_icosphere(bm, subdivisions=3, radius=1.0)
    desloc = Vector((rng.uniform(-50, 50), rng.uniform(-50, 50), rng.uniform(-50, 50)))
    desloc2 = Vector((rng.uniform(-50, 50), rng.uniform(-50, 50), rng.uniform(-50, 50)))
    freq = rng.uniform(1.0, 1.8)
    amp = rng.uniform(0.3, 0.45)
    amp2 = rng.uniform(0.10, 0.18)
    # Alongado num eixo e fino no eixo Z local: o eixo fino e o que se deita
    # na camada de cima (7 cm entre o U1 e a tampa) e o que atravessa o vao
    # lateral (~4 cm). O floco de embalagem e um "amendoim" torto.
    escala = Vector((rng.uniform(1.4, 2.2), rng.uniform(0.7, 1.0), rng.uniform(0.42, 0.62)))
    curva = rng.uniform(-0.4, 0.4)
    cintura = rng.uniform(0.0, 0.3)
    for v in bm.verts:
        n = noise.noise(v.co * freq + desloc)
        n2 = noise.noise(v.co * freq * 2.7 + desloc2)
        fator = 1.0 + amp * n + amp2 * n2
        x, y, z = v.co.x * escala.x, v.co.y * escala.y, v.co.z * escala.z
        aperto = 1.0 - cintura * math.exp(-(x / (0.45 * escala.x)) ** 2)
        y *= aperto
        z *= aperto
        z += curva * x * x
        v.co = Vector((x, y, z)) * fator
    # Cantos amassados: 2 a 4 planos aleatorios "cortam" o que sobra deles.
    for _ in range(rng.randint(2, 4)):
        normal = Vector((rng.gauss(0, 1), rng.gauss(0, 1), rng.gauss(0, 1))).normalized()
        alcance = max(v.co.dot(normal) for v in bm.verts)
        corte = alcance * rng.uniform(0.55, 0.8)
        for v in bm.verts:
            s = v.co.dot(normal)
            if s > corte:
                v.co -= normal * (s - corte) * 0.9
    # Normaliza pelo maior raio real: "floco de 6 a 10 cm" e a maior dimensao
    # do floco deformado, nao o raio da icosfera de partida.
    maior = max(v.co.length for v in bm.verts)
    for v in bm.verts:
        v.co *= raio / maior
    malha = bpy.data.meshes.new(nome)
    bm.to_mesh(malha)
    bm.free()
    _sombrear_suave(malha)
    return malha


def _extensoes(malha, rot):
    """Meias-extensoes (x, y, z) do floco girado por rot (Euler XYZ): o raio
    REAL em cada eixo do mundo, que e o que decide se ele cabe na camada."""
    R = rot.to_matrix() if hasattr(rot, "to_matrix") else Euler(rot).to_matrix()
    mx = my = mz = 0.0
    for v in malha.vertices:
        w = R @ v.co
        mx = max(mx, abs(w.x))
        my = max(my, abs(w.y))
        mz = max(mz, abs(w.z))
    return mx, my, mz


def _encolher(malha, fator):
    if fator < 1.0:
        for v in malha.vertices:
            v.co *= fator
        malha.update()


# ---------------------------------------------------------------- materiais

def _material_base(nome):
    """Pega ou cria o material por nome e reconstroi os nos do zero, como o
    mod_u1 faz: e o que impede 'caixa.papel.001' a cada rodada na aba
    Scripting do cliente (a revisao contou 3 materiais novos por rodada)."""
    mat = bpy.data.materials.get(nome)
    if mat is None:
        mat = bpy.data.materials.new(nome)
    mat.use_nodes = True
    nt = mat.node_tree
    nt.nodes.clear()
    saida = nt.nodes.new("ShaderNodeOutputMaterial")
    bsdf = nt.nodes.new("ShaderNodeBsdfPrincipled")
    saida.location = (600, 0)
    bsdf.location = (300, 0)
    nt.links.new(bsdf.outputs["BSDF"], saida.inputs["Surface"])
    if mat.animation_data:
        mat.animation_data_clear()
    if nt.animation_data:
        nt.animation_data_clear()
    return mat, nt, bsdf


# Papel x tinta. O sheen branco (tint padrao) sobre tinta escura entra na
# conta da logo palida (a revisao mediu 136 -> 112 sRGB ao zera-lo na cena do
# anuncio; aqui, com a luz do teste, 173 -> 164) - o grosso e a exposicao, ver
# o cabecalho. O papel fica com um sheen minimo NA COR DO PAPEL e a tinta
# zera. O specular da tinta tambem e ZERO: no beat 7 (camera a 12 cm da
# tampa, olhando reto para baixo) o reflexo das luzes do ambiente na tinta
# punha um piso de ~130 sRGB no cinza que nem gamma 2,4 baixava (medido:
# 130 -> 120); com specular 0 o mesmo quadro mede 77-94.
SHEEN_PAPEL, SHEEN_TINTA = 0.08, 0.0
RUG_PAPEL, RUG_TINTA = 0.78, 0.5
SPEC_PAPEL, SPEC_TINTA = 0.35, 0.0


def _material_papel(nome, cor_hex, imagem=None, escala_logo=None, z_topo_local=None,
                    centro_uv=(0.5, 0.5), saturacao=1.0, gamma=1.0):
    """Papel fosco com toque de grao. Se houver imagem, ela entra como tinta
    impressa: mistura pela alfa na Base Color e troca rugosidade, sheen e
    specular por valores de tinta, sem relevo. A projecao e plana no espaco
    local do objeto (coordenada Object), so na face de cima (z local acima
    de z_topo_local)."""
    mat, nt, bsdf = _material_base(nome)

    cor = cor_linear(cor_hex)
    bsdf.inputs["Base Color"].default_value = cor
    bsdf.inputs["Roughness"].default_value = RUG_PAPEL
    bsdf.inputs["Specular IOR Level"].default_value = SPEC_PAPEL
    try:
        # Sheen e o que da o "toque de papel" na luz rasante. So existe no
        # Principled v2 (4.0+); em versoes velhas o nome era "Sheen".
        bsdf.inputs["Sheen Weight"].default_value = SHEEN_PAPEL
        bsdf.inputs["Sheen Roughness"].default_value = 0.6
        bsdf.inputs["Sheen Tint"].default_value = cor
    except KeyError:
        pass

    # Grao do papel: ruido fino em bump bem fraco. Sem isto o papel parece
    # plastico liso na luz de estudio.
    coord = nt.nodes.new("ShaderNodeTexCoord")
    coord.location = (-900, 0)
    ruido = nt.nodes.new("ShaderNodeTexNoise")
    ruido.location = (-300, -350)
    ruido.inputs["Scale"].default_value = 250.0
    ruido.inputs["Detail"].default_value = 2.0
    nt.links.new(coord.outputs["Object"], ruido.inputs["Vector"])
    bump = nt.nodes.new("ShaderNodeBump")
    bump.location = (0, -350)
    bump.inputs["Strength"].default_value = 0.02
    bump.inputs["Distance"].default_value = 0.001
    nt.links.new(ruido.outputs["Fac"], bump.inputs["Height"])
    nt.links.new(bump.outputs["Normal"], bsdf.inputs["Normal"])

    if imagem is None:
        return mat

    # --- decal da logo ---
    mapa = nt.nodes.new("ShaderNodeMapping")
    mapa.location = (-650, 250)
    mapa.vector_type = "POINT"
    inv = 1.0 / escala_logo
    # Location = centro do desenho em UV: o desenho (nao o arquivo) fica
    # centrado no topo da tampa.
    mapa.inputs["Location"].default_value = (centro_uv[0], centro_uv[1], 0.0)
    mapa.inputs["Scale"].default_value = (inv, inv, 1.0)
    nt.links.new(coord.outputs["Object"], mapa.inputs["Vector"])

    tex = nt.nodes.new("ShaderNodeTexImage")
    tex.location = (-400, 250)
    tex.image = imagem
    tex.extension = "CLIP"      # fora do quadrado da logo: alfa 0
    tex.interpolation = "Cubic"
    nt.links.new(mapa.outputs["Vector"], tex.inputs["Vector"])

    # A projecao Object atravessa o objeto: sem esta mascara a logo apareceria
    # espelhada por dentro da tampa. So vale onde z local e o topo.
    sep = nt.nodes.new("ShaderNodeSeparateXYZ")
    sep.location = (-650, 500)
    nt.links.new(coord.outputs["Object"], sep.inputs["Vector"])
    so_topo = nt.nodes.new("ShaderNodeMath")
    so_topo.location = (-400, 500)
    so_topo.operation = "GREATER_THAN"
    so_topo.inputs[1].default_value = z_topo_local - 0.0005
    nt.links.new(sep.outputs["Z"], so_topo.inputs[0])
    mascara = nt.nodes.new("ShaderNodeMath")
    mascara.location = (-150, 450)
    mascara.operation = "MULTIPLY"
    nt.links.new(tex.outputs["Alpha"], mascara.inputs[0])
    nt.links.new(so_topo.outputs["Value"], mascara.inputs[1])

    mix_cor = nt.nodes.new("ShaderNodeMixRGB")
    mix_cor.location = (50, 250)
    mix_cor.blend_type = "MIX"
    mix_cor.inputs["Color1"].default_value = cor
    nt.links.new(mascara.outputs["Value"], mix_cor.inputs["Fac"])
    ultimo = tex.outputs["Color"]
    if abs(saturacao - 1.0) > 1e-3:
        hsv = nt.nodes.new("ShaderNodeHueSaturation")
        hsv.location = (-250, 250)
        hsv.inputs["Saturation"].default_value = saturacao
        nt.links.new(ultimo, hsv.inputs["Color"])
        ultimo = hsv.outputs["Color"]
    if abs(gamma - 1.0) > 1e-3:
        # Escurece os meios-tons da tinta (o AgX levanta o escuro); o branco
        # e o transparente nao mudam, entao o papel em volta fica igual.
        gam = nt.nodes.new("ShaderNodeGamma")
        gam.location = (-100, 250)
        gam.inputs["Gamma"].default_value = gamma
        nt.links.new(ultimo, gam.inputs["Color"])
        ultimo = gam.outputs["Color"]
    nt.links.new(ultimo, mix_cor.inputs["Color2"])
    nt.links.new(mix_cor.outputs["Color"], bsdf.inputs["Base Color"])

    # Onde ha tinta, o material vira tinta: rugosidade, sheen e specular
    # trocam pela mascara (valor = papel + mascara * (tinta - papel)).
    def _pela_mascara(entrada, papel, tinta, y):
        m = nt.nodes.new("ShaderNodeMath")
        m.location = (50, y)
        m.operation = "MULTIPLY_ADD"
        nt.links.new(mascara.outputs["Value"], m.inputs[0])
        m.inputs[1].default_value = tinta - papel
        m.inputs[2].default_value = papel
        nt.links.new(m.outputs["Value"], bsdf.inputs[entrada])

    _pela_mascara("Roughness", RUG_PAPEL, RUG_TINTA, 50)
    _pela_mascara("Specular IOR Level", SPEC_PAPEL, SPEC_TINTA, -100)
    try:
        _pela_mascara("Sheen Weight", SHEEN_PAPEL, SHEEN_TINTA, -250)
    except KeyError:
        pass
    return mat


def _material_espuma(nome):
    mat, nt, bsdf = _material_base(nome)
    bsdf.inputs["Base Color"].default_value = cor_linear(COR_ESPUMA)
    bsdf.inputs["Roughness"].default_value = 0.9
    bsdf.inputs["Specular IOR Level"].default_value = 0.25
    try:
        # Subsuperficie branca leve: e o que separa espuma de ceramica - a luz
        # entra um pouco e as sombras proprias ficam macias.
        bsdf.inputs["Subsurface Weight"].default_value = 0.15
        bsdf.inputs["Subsurface Radius"].default_value = (1.0, 1.0, 1.0)
        bsdf.inputs["Subsurface Scale"].default_value = 0.01
    except KeyError:
        pass
    return mat


def _medir_conteudo(img):
    """Fracao da largura da imagem ocupada pelo desenho (pela alfa) e o centro
    dele em UV. O PNG tem margem transparente: 'logo com 45% da largura' e o
    desenho, nao o quadrado do arquivo."""
    import numpy as np
    w, h = img.size
    px = np.empty(w * h * 4, dtype=np.float32)
    img.pixels.foreach_get(px)
    alfa = px.reshape(h, w, 4)[:, :, 3] > 0.05
    linhas = np.where(alfa.any(axis=1))[0]
    colunas = np.where(alfa.any(axis=0))[0]
    if len(linhas) == 0 or len(colunas) == 0:
        return 1.0, (0.5, 0.5)
    x0, x1 = colunas[0], colunas[-1] + 1
    y0, y1 = linhas[0], linhas[-1] + 1
    fracao = max((x1 - x0) / w, (y1 - y0) / h)
    centro = ((x0 + x1) / (2.0 * w), (y0 + y1) / (2.0 * h))
    return fracao, centro


def _carregar_logo(caminho):
    import os
    if os.path.exists(caminho):
        img = bpy.data.images.load(caminho, check_existing=True)
        # Empacotada no .blend: sem isto o arquivo gravado aponta para a
        # pasta temporaria e a logo vira rosa em outra maquina (medido na
        # revisao: packed_file = False nas tres imagens).
        try:
            if not img.packed_file:
                img.pack()
        except RuntimeError as e:
            print("[caixa] AVISO: nao empacotou a logo:", e)
        return img, False
    # Sem o PNG, um quadrado provisorio para a projecao ter o que mostrar.
    import numpy as np
    img = bpy.data.images.new("caixa.logo_provisoria", 256, 256, alpha=True)
    px = np.zeros((256, 256, 4), dtype=np.float32)
    px[32:224, 32:224] = (0.85, 0.35, 0.10, 1.0)
    px[96:160, 96:160] = (0.0, 0.0, 0.0, 0.0)
    img.pixels.foreach_set(px.ravel())
    img.pack()
    return img, True


# ---------------------------------------------------------------- construir

def construir_caixa(cena, colecao_pai=None, params=None):
    """Cria corpo, tampa e espumas na sub-colecao 'caixa'. Devolve objetos e
    medidas. Idempotente: apaga a colecao anterior antes de construir."""
    p = dict(PARAMS_PADRAO)
    if params:
        p.update(params)
    limpar_colecao(NOME)
    col = _colecao(cena, colecao_pai, NOME)
    rng = random.Random(p["semente"])

    ix, iy, iz = p["interior"]
    e = p["parede"]
    folga = p["folga"]
    corpo_ext = (ix + 2 * e, iy + 2 * e, iz + e)
    tampa_ext = (corpo_ext[0] + 2 * folga + 2 * e, corpo_ext[1] + 2 * folga + 2 * e)
    h_tampa = p["altura_tampa"]
    # A tampa apoia o topo interno no topo do corpo, com a folga; o topo
    # externo fica uma parede acima.
    z_topo_tampa = corpo_ext[2] + folga + e
    z_base_tampa = z_topo_tampa - h_tampa
    z_centro_tampa = (z_topo_tampa + z_base_tampa) / 2.0

    cor_hex = CORES.get(p["cor"], CORES["clara"])

    # corpo: origem no centro da base (0,0,0)
    malha_corpo = _caixa_oca("caixa.corpo", corpo_ext, e, corpo_ext[2], True, 0.0)
    corpo = bpy.data.objects.new("caixa.corpo", malha_corpo)
    col.objects.link(corpo)
    _chanfro(corpo, p["chanfro"], p["segmentos_chanfro"])
    _sombrear_suave(malha_corpo)
    corpo.data.materials.append(_material_papel("caixa.papel", cor_hex))

    # tampa: origem no proprio centro, para girar em torno de si
    malha_tampa = _caixa_oca("caixa.tampa", tampa_ext, e, h_tampa, False, h_tampa / 2.0)
    tampa = bpy.data.objects.new("caixa.tampa", malha_tampa)
    tampa.location = (0.0, 0.0, z_centro_tampa)
    col.objects.link(tampa)
    _chanfro(tampa, p["chanfro"], p["segmentos_chanfro"])
    _sombrear_suave(malha_tampa)

    imagem, provisoria = _carregar_logo(_caminho_asset(p["logo"]))
    if provisoria:
        print("[caixa] AVISO: logo nao encontrada; usando quadrado provisorio")
    largura_logo = p["largura_logo"] * tampa_ext[0]
    fracao, centro_uv = _medir_conteudo(imagem)
    # O quadrado do arquivo e projetado maior que a logo, para que o DESENHO
    # ocupe a largura pedida.
    escala_quadrado = largura_logo / fracao
    tampa.data.materials.append(
        _material_papel("caixa.papel_tampa", cor_hex, imagem, escala_quadrado,
                        h_tampa / 2.0, centro_uv, p["saturacao_logo"], p["gamma_logo"])
    )

    # Marcador (Empty, nao renderiza) no centro da logo, filho da tampa: a
    # camera do beat 7 mira e atravessa este ponto, e ele acompanha a tampa.
    centro_local = Vector((0.0, 0.0, h_tampa / 2.0))
    marcador = bpy.data.objects.new("caixa.logo", None)
    marcador.empty_display_type = "ARROWS"
    marcador.empty_display_size = 0.05
    marcador.parent = tampa
    marcador.location = centro_local
    col.objects.link(marcador)

    # espumas
    mat_espuma = _material_espuma("caixa.espuma")
    espumas = []
    ux, uy, uz = p["u1"]
    n = int(p["n_espumas"])
    r_min, r_max = p["raio_espuma"]
    FOLGA_ESPUMA = 0.003
    camada = iz - uz                      # entre o topo do U1 e o topo interno
    vao_x, vao_y = (ix - ux) / 2.0, (iy - uy) / 2.0
    ocupados = []                         # (x, y, raio) da camada de cima
    for i in range(n):
        raio = rng.uniform(r_min, r_max)
        malha = _malha_espuma("caixa.espuma.%03d" % (i + 1), rng, raio)
        obj = bpy.data.objects.new("caixa.espuma.%03d" % (i + 1), malha)
        obj.data.materials.append(mat_espuma)
        # Chanfro arredonda a crista entre as facetas amassadas e o corpo;
        # a subdivisao por cima tira o que sobra de quina da icosfera.
        bis = obj.modifiers.new("chanfro", "BEVEL")
        bis.width = raio * 0.15
        bis.segments = 2
        bis.limit_method = "ANGLE"
        bis.angle_limit = math.radians(35.0)
        sub = obj.modifiers.new("suave", "SUBSURF")
        sub.levels = 1
        sub.render_levels = 1
        # Onde fica: a maior parte por cima do U1 (e de la que voa quando a
        # tampa abre); o resto nos vaos laterais entre o U1 e a parede. O
        # eixo fino do floco (z local) fica na direcao apertada, e a folga
        # sai da extensao REAL do floco girado - o 0,7*raio antigo enterrava
        # a base do floco 5-9 mm no topo do U1 (medido na revisao).
        if rng.random() < 0.62:
            rot = Euler((rng.uniform(-0.35, 0.35), rng.uniform(-0.35, 0.35), rng.uniform(0, math.tau)))
            rx, ry, rz = _extensoes(malha, rot)
            if 2 * rz + 2 * FOLGA_ESPUMA > camada:
                # Nao cabe deitado: encolhe em vez de atravessar a tampa.
                f = (camada - 2 * FOLGA_ESPUMA) / (2 * rz)
                _encolher(malha, f)
                raio *= f
                rx, ry, rz = rx * f, ry * f, rz * f
            z_min = e + uz + rz + FOLGA_ESPUMA
            z_max = e + iz - rz - FOLGA_ESPUMA
            z = rng.uniform(z_min, max(z_min, z_max))
            # Espalha na camada sem empilhar dois no mesmo lugar (ate 40
            # tentativas; depois aceita a sobreposicao).
            for _ in range(40):
                x = rng.uniform(-ix / 2 + rx, ix / 2 - rx)
                y = rng.uniform(-iy / 2 + ry, iy / 2 - ry)
                if all((x - ox) ** 2 + (y - oy) ** 2 > (0.8 * (raio + orr)) ** 2 for ox, oy, orr in ocupados):
                    break
            ocupados.append((x, y, raio))
        else:
            lado = rng.choice(("x", "y"))
            sinal = rng.choice((-1.0, 1.0))
            # O eixo fino (z local) atravessa o vao; sobra so um jogo pequeno
            # nos outros dois angulos, porque girar em torno do eixo longo
            # tambem gira o eixo fino para fora do vao (Euler XYZ: X, Y, Z).
            pequeno = lambda: rng.uniform(-0.15, 0.15)  # noqa: E731
            if lado == "x":
                rot = Euler((pequeno(), rng.choice((-1.0, 1.0)) * math.pi / 2 + pequeno(), pequeno()))
                rx, ry, rz = _extensoes(malha, rot)
                vao = vao_x
                largo = rx
            else:
                rot = Euler((rng.choice((-1.0, 1.0)) * math.pi / 2 + pequeno(), pequeno(), pequeno()))
                rx, ry, rz = _extensoes(malha, rot)
                vao = vao_y
                largo = ry
            if 2 * largo + 2 * FOLGA_ESPUMA > vao:
                # O vao tem ~4 cm: o floco que nao cabe atravessado encolhe.
                f = (vao - 2 * FOLGA_ESPUMA) / (2 * largo)
                _encolher(malha, f)
                raio *= f
                rx, ry, rz = rx * f, ry * f, rz * f
            # centro do vao: meio caminho entre o U1 e a parede
            if lado == "x":
                x = sinal * (ux / 2 + vao_x / 2)
                y = rng.uniform(-iy / 2 + ry, iy / 2 - ry)
            else:
                x = rng.uniform(-ix / 2 + rx, ix / 2 - rx)
                y = sinal * (uy / 2 + vao_y / 2)
            z = rng.uniform(e + 0.25, e + uz - rz)
        obj.location = (x, y, z)
        obj.rotation_euler = rot
        # Repouso guardado no objeto: as animacoes partem daqui e voltam para
        # ca, independentemente do estado em que a cena estiver.
        obj["caixa_repouso"] = list(obj.location)
        obj["caixa_rot_repouso"] = list(obj.rotation_euler)
        obj["caixa_raio"] = raio
        obj["caixa_extensoes"] = [rx, ry, rz]
        col.objects.link(obj)
        espumas.append(obj)

    return {
        "corpo": corpo,
        "tampa": tampa,
        "logo": marcador,
        "espumas": espumas,
        "interior": (ix, iy, iz),
        "exterior_corpo": corpo_ext,
        "exterior_tampa": (tampa_ext[0], tampa_ext[1], h_tampa),
        "altura_tampa": h_tampa,
        "topo_tampa_z": z_topo_tampa,
        "base_tampa_z": z_base_tampa,
        "centro_logo": Vector((0.0, 0.0, z_topo_tampa)),
        "centro_logo_local": centro_local,
        "normal_logo": Vector((0.0, 0.0, 1.0)),
        "largura_logo": largura_logo,
        "colecao": col,
        # Para a coreografia poder avisar na tela quando a logo e o quadrado.
        "logo_provisoria": provisoria,
        "imagem_logo": imagem,
    }


# ---------------------------------------------------------------- animacao

def fcurves_de(animation_data):
    """Fcurves da acao de um animation_data, em qualquer Blender 4.2+."""
    # Action.fcurves virou legado no 4.4 (slotted actions); no 5.0 pode nao existir.
    try:
        return animation_data.action.fcurves
    except AttributeError:
        slot = animation_data.action_slot
        return animation_data.action.layers[0].strips[0].channelbag(slot).fcurves


def _suavizar(obj, q_ini, q_fim, easing, canais=None, interpolacao="BEZIER"):
    """Deixa Bezier + easing em todas as chaves do intervalo. So mexe nas
    chaves deste intervalo para nao alterar animacao de outro beat."""
    ad = obj.animation_data
    if ad is None or ad.action is None:
        return
    for fc in fcurves_de(ad):
        if canais is not None and fc.data_path not in canais:
            continue
        for kp in fc.keyframe_points:
            if q_ini - 0.5 <= kp.co.x <= q_fim + 0.5:
                kp.interpolation = interpolacao
                kp.easing = easing
                if interpolacao == "BEZIER":
                    kp.handle_left_type = "AUTO_CLAMPED"
                    kp.handle_right_type = "AUTO_CLAMPED"
        fc.update()


def _chave(obj, quadro, loc=None, rot=None):
    if loc is not None:
        obj.location = loc
        obj.keyframe_insert("location", frame=quadro)
    if rot is not None:
        obj.rotation_euler = rot
        obj.keyframe_insert("rotation_euler", frame=quadro)


def animar_tampa(objs, q_ini, q_fim, abrir=True, easing="EASE_IN_OUT",
                 subida=0.5, inclinacao=25.0, lado=1.0, afastamento=1.6):
    """Tampa sobe ~0,5 m inclinando ~25 graus e sai de cena para o lado +X
    (lado=-1 para -X). abrir=False faz o caminho inverso, do lado ate
    encaixar. Tres chaves: fechada, no alto inclinada, fora."""
    tampa = objs["tampa"]
    z0 = objs["topo_tampa_z"] - objs["altura_tampa"] / 2.0
    fechada = (Vector((0.0, 0.0, z0)), Vector((0.0, 0.0, 0.0)))
    # Inclina para o lado da saida: rotacao negativa em Y levanta a borda +X.
    inclinada = (
        Vector((lado * 0.08, 0.0, z0 + subida)),
        Vector((0.0, -lado * math.radians(inclinacao), 0.0)),
    )
    fora = (
        Vector((lado * afastamento, 0.0, z0 + subida + 0.25)),
        Vector((0.0, -lado * math.radians(inclinacao * 1.2), 0.0)),
    )
    q_meio = int(round(q_ini + (q_fim - q_ini) * (0.55 if abrir else 0.45)))
    seq = [fechada, inclinada, fora] if abrir else [fora, inclinada, fechada]
    for quadro, (loc, rot) in zip((q_ini, q_meio, q_fim), seq):
        _chave(tampa, quadro, loc, rot)
    _suavizar(tampa, q_ini, q_fim, easing)


def _trajetoria_espuma(obj, i, semente, objs):
    """Arco de uma espuma, do repouso dentro da caixa ate o chao fora dela.
    Deterministico por (semente, i) para a volta refazer o mesmo caminho.
    Devolve (atraso 0..1, duracao 0..1, [(t, loc, rot)])."""
    rng = random.Random(semente * 1000 + i)
    ini = Vector(obj["caixa_repouso"])
    rot0 = Vector(obj["caixa_rot_repouso"])
    raio = float(obj["caixa_raio"])
    ext = objs["exterior_tampa"]
    # Direcao de saida: para fora do lado em que a espuma esta, com espalhamento.
    base = math.atan2(ini.y, ini.x) if ini.xy.length > 0.05 else rng.uniform(0, math.tau)
    ang = base + rng.uniform(-1.1, 1.1)
    # Pousa fora da pegada da caixa em qualquer angulo: a meia diagonal do
    # corpo e ~0,46 m, e a distancia minima fica acima disso com sobra.
    dist = rng.uniform(0.55, 1.05) + raio
    fim = Vector((math.cos(ang) * dist, math.sin(ang) * dist, raio * 0.7))
    apice = max(objs["topo_tampa_z"] + 0.35, ini.z + rng.uniform(0.45, 0.85))
    # Gravidade reduzida: espuma e leve e freia no ar; a queda "real" a 9,8
    # parece pedra. Numero escolhido olhando a previa.
    g = 3.2
    t_sobe = math.sqrt(2.0 * (apice - ini.z) / g)
    t_desce = math.sqrt(2.0 * max(apice - fim.z, 0.01) / g)
    T = t_sobe + t_desce
    giro = Vector((rng.uniform(-1, 1), rng.uniform(-1, 1), rng.uniform(-1, 1))) * rng.uniform(4.0, 9.0)
    # Nada de deslocamento horizontal enquanto o floco nao passou da boca do
    # corpo: a revisao mediu quatro flocos do vao lateral (repouso a z~0,3)
    # saindo ATRAVES da parede. O instante da saida vem da propria parabola.
    z_saida = objs["exterior_corpo"][2] + raio
    u_saida = 0.0
    if ini.z < z_saida < apice:
        # ini.z + g*t_sobe*t - 0.5*g*t^2 = z_saida  (raiz menor)
        disc = (g * t_sobe) ** 2 - 2.0 * g * (z_saida - ini.z)
        t_s = (g * t_sobe - math.sqrt(max(disc, 0.0))) / g
        u_saida = min(t_s / T, 0.9)
    pontos = []
    passos = 16
    for k in range(passos + 1):
        u = k / passos
        t = u * T
        # Horizontal comeca devagar e acelera: a espuma sobe quase reta ao sair
        # da caixa e so depois abre o arco - e o que evita atravessar a parede.
        v = max(0.0, (u - u_saida) / (1.0 - u_saida))
        w = v * (0.45 + 0.55 * v)
        xy = ini.xy.lerp(fim.xy, w)
        if t <= t_sobe:
            z = ini.z + g * t_sobe * t - 0.5 * g * t * t
        else:
            td = t - t_sobe
            z = apice - 0.5 * g * td * td
        z = max(z, fim.z)
        pontos.append((u, Vector((xy.x, xy.y, z)), rot0 + giro * t))
    atraso = rng.uniform(0.0, 0.35)
    duracao = rng.uniform(0.5, 0.65) * (1.0 - atraso)
    return atraso, duracao, pontos


def animar_espuma(objs, q_ini, q_fim, semente=7, easing="EASE_IN_OUT"):
    """Cada espuma salta em arco (parabola com gravidade), gira e cai no chao
    fora da caixa; depois fica parada ate q_fim."""
    n = q_fim - q_ini
    for i, obj in enumerate(objs["espumas"]):
        atraso, duracao, pontos = _trajetoria_espuma(obj, i, semente, objs)
        q_a = q_ini + atraso * n
        q_b = q_a + duracao * n
        _chave(obj, q_ini, pontos[0][1], pontos[0][2])
        _chave(obj, int(round(q_a)), pontos[0][1], pontos[0][2])
        for u, loc, rot in pontos[1:]:
            _chave(obj, int(round(q_a + u * (q_b - q_a))), loc, rot)
        _chave(obj, q_fim, pontos[-1][1], pontos[-1][2])
        _suavizar(obj, q_ini, q_fim, easing, canais=("location",))
        # Giro linear: Bezier em rotacao faria a espuma parar de girar a cada
        # amostra.
        _suavizar(obj, q_ini, q_fim, "AUTO", canais=("rotation_euler",), interpolacao="LINEAR")
        obj["caixa_pouso"] = list(pontos[-1][1])


def animar_espuma_voltar(objs, q_ini, q_fim, semente=7, easing="EASE_IN_OUT"):
    """Inverso de animar_espuma (beat 6): do chao de volta ao repouso dentro
    da caixa, pelo mesmo arco. Mesma semente = mesmo caminho."""
    n = q_fim - q_ini
    for i, obj in enumerate(objs["espumas"]):
        atraso, duracao, pontos = _trajetoria_espuma(obj, i, semente, objs)
        pontos = pontos[::-1]
        q_a = q_ini + atraso * n
        q_b = q_a + duracao * n
        _chave(obj, q_ini, pontos[0][1], pontos[0][2])
        _chave(obj, int(round(q_a)), pontos[0][1], pontos[0][2])
        for k, (u, loc, rot) in enumerate(pontos[1:], start=1):
            _chave(obj, int(round(q_a + (k / (len(pontos) - 1)) * (q_b - q_a))), loc, rot)
        _chave(obj, q_fim, pontos[-1][1], pontos[-1][2])
        _suavizar(obj, q_ini, q_fim, easing, canais=("location",))
        _suavizar(obj, q_ini, q_fim, "AUTO", canais=("rotation_euler",), interpolacao="LINEAR")
