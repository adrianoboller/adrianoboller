# Modulo U1 - substituto parametrico do Snapmaker U1 nas dimensoes reais.
#
# O modelo real vive no Blender do cliente e nao viaja ate aqui. Este
# substituto existe para a coreografia, a luz e os materiais serem validados
# em cima de algo que se pareca com a maquina; o dict que construir_u1
# devolve e as funcoes de animacao nao dependem da geometria daqui - usam a
# 'raiz' e os pontos ('posicao_tela', 'posicao_tomada', 'posicao_botao'),
# para funcionarem igual quando o modelo real entrar no lugar.
#
# O QUE FOI CONFIRMADO (fotos oficiais do Quick Start Guide V1.0.0 da
# Snapmaker, ficha tecnica em snapmaker.com/snapmaker-u1/specs, reviews da
# 3Dnatives e Tom's Hardware):
# - 584 x 499 x 730 mm, 18,2 kg, volume 270^3, tela 3,5" 320x480 touch.
# - Paineis plasticos injetados BRANCOS (nao grafite: a paleta da
#   ESPECIFICACAO diz #1E2024, mas toda foto oficial mostra corpo branco com
#   aro superior preto e moldura da porta preta). Por isso a cor do corpo e
#   parametro ('cor_corpo'), com o branco real como padrao - a paleta manda
#   no resto da cena, nao no produto.
# - Porta de vidro FUMe na frente, moldura preta, dobradica a esquerda,
#   puxador vertical preto na borda direita; a porta ocupa a metade inferior
#   da frente.
# - Tela embutida nivelada na frente, no canto SUPERIOR DIREITO, paisagem;
#   wordmark "snapmaker" no canto superior esquerdo.
# - Topo aberto, com aro preto e 4 tampoes redondos nos cantos (o Top Cover e
#   opcional e vendido a parte). Os 4 cabecotes estacionam numa viga no FUNDO
#   do topo, numerados 1..4 da esquerda para a direita; cada um e um bloco
#   preto com trava laranja. Carro X branco, com o wordmark e uma ventoinha
#   laranja, correndo em duas hastes de fibra de carbono.
# - Painel traseiro transparente (plastico fume), com furacao de ventilacao.
# - Botao liga/desliga: gangorra VERMELHA, e a tomada IEC C14 logo ABAIXO
#   dela, numa coluna saliente branca na TRASEIRA, canto que fica a DIREITA de
#   quem olha a frente (+X); USB-A laranja e conector "Add-on" acima do botao.
# - Mesa: chapa de aco flexivel com PEI dourado texturizado, wordmark ao
#   centro, sobre um carro preto.
# - Laterais brancas com um grande rebaixo circular (encaixe dos porta-bobinas)
#   e furacao de ventilacao perto do fundo.
# - Duas barras de LED no teto da camara, na frente.
#
# O QUE FOI CHUTADO (nao achei medida publicada):
# - Alturas na traseira: tomada a 0,125 m do chao, botao a 0,175 m, USB a
#   0,25 m, coluna de 60 mm de largura centrada em x = +0,245 m (medido a olho
#   na foto 5.24 do guia, onde a tomada fica logo acima do pe arredondado).
# - Medidas da porta (0,47 x 0,41 m), da tela com moldura (0,104 x 0,070 m) e
#   da area ativa (74,4 x 49,6 mm, que e o que da uma diagonal de 3,5" em
#   3:2), raio dos cantos verticais (25 mm), posicao dos cabecotes na viga.
# - Nao ha LED de status visivel nas fotos; o 'LED que acende' do roteiro
#   aqui sao as barras de LED da camara e a janela vermelha do proprio botao.
# - Tubos PTFE em laco sobre os cabecotes so existem depois da instalacao,
#   entao ficam desligados por padrao (param 'tubos') - com eles o U1 nao
#   caberia na caixa.
#
# ENVELOPE: os 499 mm de profundidade do U1 real INCLUEM a porta, o puxador e
# os conectores da traseira. Aqui o casco branco mede 499 - 30 (porta +
# puxador) - 16 (coluna + botao) = 453 mm e e deslocado 7 mm para tras, de
# modo que o envelope externo fique simetrico em Y e meca exatamente 0,499.
# 'dimensoes' devolve o envelope MEDIDO da malha avaliada, nao o nominal -
# a primeira versao devolvia 0,499 com 0,545 de geometria e quem confiasse
# nisso para posicionar cabo ou caixa erraria 46 mm.
#
# BARRA DE BOOT: o PNG assets/tela_boot.png (gerado por tela_ui_fonte.html)
# ja traz a barra 80% cheia. O shader nao confia nisso: repinta o trilho
# inteiro com a cor do trilho e so entao pinta o branco ate 'progresso', de
# modo que a animacao fica certa com ou sem preenchimento cozido no asset. O
# retangulo do trilho e parametro ('barra_boot_px'), medido com numpy no PNG.
#
# RODADA 2 (revisao em docs/REVISAO-RODADA-1.md):
# - LIGAR E EVENTO DE LUZ: animar_ligar(objs, q_ini, q_fim) faz o que
#   animar_botao faz (afunda o botao, acende a janela vermelha) e, a partir do
#   fundo do curso, sobe as fitas de LED de 0 a 4 em 6 quadros (Bezier), liga
#   uma area light por fita (u1.led.luz.N, 0 -> 60 W) e poe a tela em
#   'standby' (cinza-escuro, emissao 0,15). As luzes existem porque no EEVEE
#   uma malha emissiva NAO ilumina nada sem sonda de irradiancia cozida: a
#   fita a 12 de forca do animar_botao brilhava sozinha e a camara continuava
#   preta pelo acrilico (medido na rodada 1: 'nada muda de q246 a q268'). As
#   luzes nascem com hide_render=True e energia 0; sem animar_ligar a cena e
#   a de antes. animar_botao continua igual (mesmo curso, mesmas forcas).
# - Material do cliente: a forca de emissao chaveada e a do no que esta
#   LIGADO ao Material Output (seguindo Surface, e Mix/Add Shader por
#   dentro); a varredura por ordem de nos so entra sem link. Vale para
#   animar_tela, animar_botao e animar_ligar - mesmo furo nos tres.
# - As imagens da tela sao empacotadas (img.pack()) ao carregar: o .blend do
#   cliente nao pode depender de %TEMP%.
# - Lateral: rasgos de ventilacao perto do fundo (confirmados: o proprio
#   cabecalho ja os listava e nao os construia; a Snapmaker e os projetos de
#   'side cover' da comunidade falam nos 'vent slots on the printer case'),
#   junta vertical entre o painel frontal e o lateral logo depois do canto
#   arredondado (CHUTE plausivel de carenagem injetada em duas pecas; nenhuma
#   foto oficial mostra uma junta horizontal, por isso nao ha uma) e o aro
#   preto do topo 10 mm acima do casco, com a boca em chanfro de 6 mm e o
#   bordo externo arredondado - o aro do U1 real e uma moldura visivel, nao
#   uma linha de 2 mm.
# - Ruido no acrilico/vidro a 16 amostras (pontos brancos em beat_3.png): a
#   rugosidade ja era 0,03/0,02 e o Principled 4.x nao tem 'Transmission
#   Roughness', entao a proposta nao tinha o que reduzir. MEDIDO (contagem de
#   pixels 0,18 acima da mediana 3x3 na janela do acrilico, q218 da cena
#   completa a 360x640/16): beat_3 da rodada 1 tem 890 pontos (540x960); com
#   o mod_ambiente da rodada 1 e este mod_u1, 372; com o rim escondido, 0;
#   com o mod_ambiente atual (rim de 2,2 m a z=1,0 -> 1,2 m a z=1,4), 0; e
#   rugosidade 0 nao muda nada (25 -> 29 pontos fora da janela). O chuvisco
#   era o painel do rim de 350 W refratado/refletido no acrilico e mal
#   denoised a 16 amostras - problema do ambiente, ja resolvido la; o
#   material do vidro NAO foi mexido. O que sobra a 16 amostras e um halo
#   claro nas bordas das paredes escuras vistas pelo acrilico (denoise da
#   refracao raytraced), que nao e ponto e some com amostras finais.
#
# RODADA 3 (teste_u1.py, PARTE=cena: o U1 no ambiente do anuncio, medido no
# pixel a 540x960/16; os numeros 'antes' sao do mod_u1 da rodada 2 nos mesmos
# enquadramentos):
# - PRETO DA TELA LIGADA: a 0,26 m da tela (fim do dolly do beat 4, q359) o
#   fundo da UI media L 69 max / 53 media no terco de cima contra 4 embaixo:
#   reflexo da faixa rose do world no vidro (Specular IOR Level 0,5, rugosidade
#   0,03). Subir a emissao nao serve, porque o fundo da UI e 0 e o reflexo
#   SOMA. O especular agora e 0,5 * (1 - (1 - reflexo_tela_ligada) * ligada):
#   com 0,2 sobrava L 19 de media em cima; com 0,05 (padrao) o pior terco da L
#   max 9,8 e media 9,2 (o 'standby' sozinho e 5,5). Criterio: L < 15 em toda
#   a tela.
# - HASTE 'LAMPADA FLUORESCENTE' (foto C, q445): a fita de LED e paralela a
#   haste e um cilindro reflete uma fonte como LINHA; com coat 1,0 a 0,08 a
#   linha era >= 250 por 79 mm continuos (170 mm no total) com as luzes da
#   camara a 10 W - e sem elas o max caia a 134: a linha e SO delas. Medido
#   por rugosidade (base e coat): 0,35 -> 80 mm, 0,5 -> 43, 0,65 -> 26, e 26
#   e o piso: a propria fita, que nessa camera passa na frente da haste (a
#   medida agora exclui por ray cast o que nao e a haste). Padrao: rugosidade
#   0,5 com coat 0,3 -> 5 mm (haste 1) e 0 (haste 2). Anisotropia 0,6 e 0,9
#   dao o MESMO pixel: o EEVEE ignora o Anisotropic do Principled; o parametro
#   fica para o Cycles.
# - BARRA BRANCA DA FOTO A (q389): nao era fita de LED (escondendo as fitas
#   nada muda) nem o aro (sem o aro ela cresce): era a parede do bolso do
#   casco, BRANCA, entre o topo da parede de tras da camara (z 0,68) e o aro
#   (z 0,706), vista por cima da frente. As fotos oficiais (guia rapido,
#   figs. 18-20) mostram o interior sob a moldura preto em toda a volta e a
#   moldura rente aos paineis: as paredes de tras e da frente da camara sobem
#   ate a face de baixo do aro (a da frente so acima do vao da porta) e o aro
#   passa a cobrir o casco inteiro (L x P), sem o degrau branco de 10 mm em
#   volta; o casco termina onde o aro comeca (faces coincidentes davam uma
#   franja hachurada na borda superior). Medido na linha do labio: 0 mm
#   >= 235 (era a linha inteira).
# - LATERAL 'SLAB' (q185/q300/q315): a hipotese da revisao era que a key de
#   2 x 2 m a 3,3 m (4,0 m do alvo, medido) chapa a face por construcao.
#   MEDIDO com a camera na normal da face +X a 2 m, 50 mm, rig a azimute+90,
#   amplitude horizontal das medias de coluna de L na faixa z 0,57-0,66:
#   key do ambiente 6,5 niveis; key 1 x 1 m a 2,5 m: 8,3; softbox 1 x 2 m
#   vertical a 2,5 m: 8,3. Criterio era >= 15: NENHUMA passa - a hipotese
#   MORREU medida, a chapa nao vem do tamanho da key (o rebaixo do disco, na
#   faixa z 0,30-0,42, da 25-32 niveis: o que faz gradiente e geometria, nao a
#   luz). O ambiente nao foi mexido. Para a coreografia/ambiente: a alavanca
#   e outra (posicao da key fora da normal, ou o rim/fill), nao o tamanho.

import math
import os

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

# Dimensoes externas oficiais (m): largura X, profundidade Y, altura Z.
LARGURA = 0.584
PROFUNDIDADE = 0.499
ALTURA = 0.730

PASTA_ASSETS = "/home/user/adrianoboller/anuncio-u1/assets"

PADROES = {
    "cor_corpo": "#E9EAE7",          # branco dos paineis (confirmado em foto)
    "imagem_boot": os.path.join(PASTA_ASSETS, "tela_boot.png"),
    "imagem_ui": os.path.join(PASTA_ASSETS, "tela_ui.png"),
    "tubos": False,
    "porta_aberta_graus": 0.0,
    # Trilho da barra de boot em pixels do PNG 480x320 (x0, x1, y_topo, y_base,
    # y crescendo para baixo, fim exclusivo) e a cor do trilho vazio. Medidos
    # em assets/tela_boot.png: left/right 90 px, bottom 44 px, 3 px de altura.
    "barra_boot_px": (90, 390, 273, 276),
    "cor_trilho_boot": "#2A2A2E",
    # Fracao do especular do vidro da tela que sobra com ela LIGADA (ver
    # RODADA 3 no cabecalho): 1,0 = reflete o world como desligada. MEDIDO no
    # fundo da UI a 0,26 m: 0,2 ainda dava L media 19 no terco de cima; 0,05
    # da 9 (o resto e o 'standby', 5,5 em toda a tela).
    "reflexo_tela_ligada": 0.05,
    # Hastes de fibra de carbono: rugosidade da resina (base e coat) e
    # anisotropia do escovado (so no Cycles: o EEVEE ignora o Anisotropic do
    # Principled - medido, 0,6 e 0,9 dao o mesmo pixel). Era 0,28/0,08; 0,35
    # ainda deixava 50 mm de linha >= 250 sob a fita de LED (RODADA 3).
    "rugosidade_hastes": 0.5,
    "anisotropia_hastes": 0.6,
}

# Quanto a porta+puxador (frente) e a coluna+botao (tras) saem do casco; o
# casco e o nominal menos isto, para o envelope medir PROFUNDIDADE.
SALIENCIA_FRENTE = 0.030
SALIENCIA_TRAS = 0.016


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


def _malha_cilindro(nome, raio, altura, eixo="Z", segmentos=48, raio2=None):
    bm = bmesh.new()
    bmesh.ops.create_cone(
        bm, cap_ends=True, cap_tris=False, segments=segmentos,
        radius1=raio, radius2=raio if raio2 is None else raio2, depth=altura,
    )
    if eixo == "X":
        bmesh.ops.rotate(bm, cent=(0, 0, 0), matrix=Matrix.Rotation(math.pi / 2, 3, "Y"), verts=bm.verts)
    elif eixo == "Y":
        bmesh.ops.rotate(bm, cent=(0, 0, 0), matrix=Matrix.Rotation(math.pi / 2, 3, "X"), verts=bm.verts)
    malha = bpy.data.meshes.new(nome)
    bm.to_mesh(malha)
    bm.free()
    _suavizar(malha)
    return malha


def _malha_anel(nome, r_int, r_ext, esp, eixo="Y", segmentos=48):
    """Anel chato (coroa circular extrudada): e o que faz uma grade ler como grade."""
    bm = bmesh.new()
    cima, baixo = [], []
    for k in range(segmentos):
        a = 2 * math.pi * k / segmentos
        c, s_ = math.cos(a), math.sin(a)
        cima.append((bm.verts.new((r_int * c, r_int * s_, esp / 2)), bm.verts.new((r_ext * c, r_ext * s_, esp / 2))))
        baixo.append((bm.verts.new((r_int * c, r_int * s_, -esp / 2)), bm.verts.new((r_ext * c, r_ext * s_, -esp / 2))))
    for k in range(segmentos):
        j = (k + 1) % segmentos
        bm.faces.new((cima[k][0], cima[k][1], cima[j][1], cima[j][0]))
        bm.faces.new((baixo[j][0], baixo[j][1], baixo[k][1], baixo[k][0]))
        bm.faces.new((cima[k][1], baixo[k][1], baixo[j][1], cima[j][1]))
        bm.faces.new((cima[j][0], baixo[j][0], baixo[k][0], cima[k][0]))
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    if eixo == "X":
        bmesh.ops.rotate(bm, cent=(0, 0, 0), matrix=Matrix.Rotation(math.pi / 2, 3, "Y"), verts=bm.verts)
    elif eixo == "Y":
        bmesh.ops.rotate(bm, cent=(0, 0, 0), matrix=Matrix.Rotation(math.pi / 2, 3, "X"), verts=bm.verts)
    malha = bpy.data.meshes.new(nome)
    bm.to_mesh(malha)
    bm.free()
    _suavizar(malha)
    return malha


def _anel(nome, col, r_int, r_ext, esp, pos, mat, eixo="Y", pai=None):
    malha = _malha_anel(nome, r_int, r_ext, esp, eixo)
    obj = _novo_objeto(nome, malha, col, pos, pai)
    if mat is not None:
        malha.materials.append(mat)
    # Chanfro por angulo: as laterais do anel ficam planas e so a quina quebra.
    _chanfro(obj, min(0.0004, esp / 3), 1)
    return obj


def _peso_chanfro(malha, peso_vertical, peso_outros):
    """Bevel weight por aresta: cantos verticais largos, o resto so quebra de quina."""
    attr = malha.attributes.get("bevel_weight_edge")
    if attr is None:
        attr = malha.attributes.new(name="bevel_weight_edge", type="FLOAT", domain="EDGE")
    for i, aresta in enumerate(malha.edges):
        a = malha.vertices[aresta.vertices[0]].co
        b = malha.vertices[aresta.vertices[1]].co
        vertical = abs(a.x - b.x) < 1e-6 and abs(a.y - b.y) < 1e-6
        attr.data[i].value = peso_vertical if vertical else peso_outros


def _chanfro(obj, largura, segmentos=4, por_peso=False, nome="chanfro"):
    mod = obj.modifiers.new(nome, "BEVEL")
    mod.width = largura
    mod.segments = segmentos
    mod.limit_method = "WEIGHT" if por_peso else "ANGLE"
    mod.angle_limit = math.radians(30)
    # Harden normals: as faces planas continuam planas e so o chanfro arredonda;
    # sem isso, malha suave com chanfro vira "bolha" nos closes.
    mod.harden_normals = True
    mod.use_clamp_overlap = True
    return mod


def _caixa(nome, col, dims, pos, mat, chanfro=0.002, segmentos=3, pai=None, peso=None, suave=True):
    malha = _malha_caixa(nome, *dims)
    # suave=False para as pecas grandes cortadas por boolean: com use_smooth,
    # o ngon que o corte do disco cria interpola normais entre vertices a
    # meio metro de distancia e aparece uma diagonal de sombra na lateral.
    # Face plana + harden_normals no Bevel arredonda so as arestas.
    if suave:
        _suavizar(malha)
    if peso is not None:
        _peso_chanfro(malha, peso[0], peso[1])
    obj = _novo_objeto(nome, malha, col, pos, pai)
    if mat is not None:
        malha.materials.append(mat)
    if chanfro > 0:
        _chanfro(obj, chanfro, segmentos, por_peso=peso is not None)
    return obj


def _cilindro(nome, col, raio, altura, pos, mat, eixo="Z", pai=None, segmentos=48, raio2=None):
    malha = _malha_cilindro(nome, raio, altura, eixo, segmentos, raio2)
    obj = _novo_objeto(nome, malha, col, pos, pai)
    if mat is not None:
        malha.materials.append(mat)
    return obj


def _cortador(nome, col, dims_ou_malha, pos, alvo, pai=None):
    """Cubo invisivel que recorta 'alvo' por boolean; fica na colecao para o limpar_colecao achar."""
    if isinstance(dims_ou_malha, tuple):
        malha = _malha_caixa(nome, *dims_ou_malha)
    else:
        malha = dims_ou_malha
    obj = _novo_objeto(nome, malha, col, pos, pai)
    obj.hide_render = True
    obj.display_type = "WIRE"
    obj.hide_viewport = True
    mod = alvo.modifiers.new("corte_" + nome.split(".")[-1], "BOOLEAN")
    mod.operation = "DIFFERENCE"
    mod.object = obj
    try:
        mod.solver = "EXACT"
    except TypeError:
        # Blender 5 removeu 'EXACT' em favor de outro nome; o padrao serve.
        pass
    return obj


def _transparente(mat):
    # DITHERED, nao BLENDED: no EEVEE Next o modo Blended nao passa pelo
    # raytracing (so por sondas), e com Transmission o vidro saia como um
    # painel cinza opaco - foi o que a primeira previa mostrou. Dithered e o
    # que ve a camara atras do vidro. Em 4.1 e antes o nome era blend_method.
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
    # 'Anisotropic' virou 'Anisotropic' com socket proprio em 4.0; se um nome
    # sumir numa versao futura, o material sai sem aquele ajuste em vez de
    # abortar a construcao inteira.
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


def _mat_aluminio(nome, eixo):
    """Aluminio escovado: anisotropia 0,8 com tangente radial no eixo da peca."""
    mat, nt, bsdf = _material(nome)
    _entrada(bsdf, "Base Color", _cor("#C9CBCE"))
    _entrada(bsdf, "Metallic", 1.0)
    _entrada(bsdf, "Roughness", 0.32)
    _entrada(bsdf, "Anisotropic", 0.8)
    tang = nt.nodes.new("ShaderNodeTangent")
    tang.direction_type = "RADIAL"
    tang.axis = eixo
    tang.location = (-300, -200)
    nt.links.new(tang.outputs["Tangent"], bsdf.inputs["Tangent"])
    return mat


def _mat_fibra_carbono(nome, rugosidade=0.35, anisotropia=0.6):
    """Sarja de fibra de carbono procedural: duas ondas cruzadas, resina por cima.

    'rugosidade' vale para a base E para o coat, e o coat cai a 0,3: com o
    coat cheio a 0,08 a haste era um espelho cilindrico da area light da
    camara - uma linha branca continua ao longo da haste (a 'lampada
    fluorescente' da foto C), porque a fita de LED e PARALELA a haste e um
    cilindro reflete uma fonte como linha. A anisotropia fica na base, com a
    tangente radial no eixo da haste (o escovado; so o Cycles a usa)."""
    mat, nt, bsdf = _material(nome)
    coord = nt.nodes.new("ShaderNodeTexCoord")
    coord.location = (-900, 0)
    onda_a = nt.nodes.new("ShaderNodeTexWave")
    onda_a.wave_type = "BANDS"
    onda_a.bands_direction = "X"
    onda_a.inputs["Scale"].default_value = 900.0
    onda_a.inputs["Distortion"].default_value = 0.0
    onda_a.location = (-600, 100)
    onda_b = nt.nodes.new("ShaderNodeTexWave")
    onda_b.wave_type = "BANDS"
    onda_b.bands_direction = "Z"
    onda_b.inputs["Scale"].default_value = 900.0
    onda_b.location = (-600, -150)
    nt.links.new(coord.outputs["Object"], onda_a.inputs["Vector"])
    nt.links.new(coord.outputs["Object"], onda_b.inputs["Vector"])
    mult = nt.nodes.new("ShaderNodeMath")
    mult.operation = "MULTIPLY"
    mult.location = (-400, 0)
    nt.links.new(onda_a.outputs["Fac"], mult.inputs[0])
    nt.links.new(onda_b.outputs["Fac"], mult.inputs[1])
    mistura = nt.nodes.new("ShaderNodeMixRGB")
    mistura.location = (-200, 0)
    mistura.inputs["Color1"].default_value = _cor("#0B0B0D")
    mistura.inputs["Color2"].default_value = _cor("#34363B")
    nt.links.new(mult.outputs["Value"], mistura.inputs["Fac"])
    nt.links.new(mistura.outputs["Color"], bsdf.inputs["Base Color"])
    _entrada(bsdf, "Roughness", rugosidade)
    _entrada(bsdf, "Metallic", 0.15)
    _entrada(bsdf, "Coat Weight", 0.3)
    _entrada(bsdf, "Coat Roughness", rugosidade)
    _entrada(bsdf, "Anisotropic", anisotropia)
    tang = nt.nodes.new("ShaderNodeTangent")
    tang.direction_type = "RADIAL"
    tang.axis = "X"
    nt.links.new(tang.outputs["Tangent"], bsdf.inputs["Tangent"])
    return mat


def _mat_pei(nome):
    """PEI dourado com micro-textura: ruido fino no bump e na aspereza."""
    mat, nt, bsdf = _material(nome)
    coord = nt.nodes.new("ShaderNodeTexCoord")
    coord.location = (-900, 0)
    ruido = nt.nodes.new("ShaderNodeTexNoise")
    ruido.inputs["Scale"].default_value = 2500.0
    ruido.inputs["Detail"].default_value = 4.0
    ruido.inputs["Roughness"].default_value = 0.6
    ruido.location = (-650, 0)
    nt.links.new(coord.outputs["Object"], ruido.inputs["Vector"])
    bump = nt.nodes.new("ShaderNodeBump")
    bump.inputs["Strength"].default_value = 0.25
    bump.inputs["Distance"].default_value = 0.0004
    bump.location = (-300, -250)
    nt.links.new(ruido.outputs["Fac"], bump.inputs["Height"])
    nt.links.new(bump.outputs["Normal"], bsdf.inputs["Normal"])
    mapa = nt.nodes.new("ShaderNodeMapRange")
    mapa.inputs["From Min"].default_value = 0.3
    mapa.inputs["From Max"].default_value = 0.7
    mapa.inputs["To Min"].default_value = 0.35
    mapa.inputs["To Max"].default_value = 0.65
    mapa.location = (-300, 100)
    nt.links.new(ruido.outputs["Fac"], mapa.inputs["Value"])
    nt.links.new(mapa.outputs["Result"], bsdf.inputs["Roughness"])
    _entrada(bsdf, "Base Color", _cor("#B98F57"))
    _entrada(bsdf, "Metallic", 0.45)
    _entrada(bsdf, "Specular IOR Level", 0.6)
    return mat


def _mat_vidro(nome, tinta="#5A5C62", aspereza=0.02):
    mat, nt, bsdf = _material(nome)
    _entrada(bsdf, "Base Color", _cor(tinta))
    _entrada(bsdf, "Transmission Weight", 1.0)
    _entrada(bsdf, "Roughness", aspereza)
    _entrada(bsdf, "IOR", 1.5)
    _transparente(mat)
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
              reflexo_ligada=0.2):
    """Vidro preto brilhante que vira tela: emissao = imagem, forca comeca em 0.

    Nos nomeados 'ligada', 'mistura' e 'progresso' sao o que animar_tela chaveia:
    - ligada:    0 desligada (vidro preto), 1 acesa
    - mistura:   0 imagem de boot, 1 interface (corte seco em q_ui_ini)
    - progresso: 0..1 preenche a barra do boot por mascara em UV
    A imagem ocupa so a area ativa de 3,5"; a moldura em volta fica preta
    pela extensao CLIP do Image Texture.
    Ligada, o Specular IOR Level cai para 'reflexo_ligada' do valor de vidro:
    o reflexo do world (a faixa rose) competia com a UI e o preto da tela
    media L 45-51 no terco de cima contra 5-6 embaixo (q359 da rodada 2).
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
    # Vidro 104x70 mm; area ativa 74,4x49,6 mm centrada: escala UV e recentra.
    ex = 0.104 / 0.0744
    ey = 0.070 / 0.0496
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
# Construcao
# ---------------------------------------------------------------------------

def construir_u1(cena, colecao_pai, params=None):
    """Cria o U1 substituto na sub-colecao 'u1' e devolve referencias e pontos."""
    p = dict(PADROES)
    if params:
        p.update(params)

    limpar_colecao(NOME)
    col = _colecao(NOME, colecao_pai)

    L, A = LARGURA, ALTURA
    # P aqui e o CASCO; o envelope com porta, puxador e coluna da PROFUNDIDADE.
    P = PROFUNDIDADE - SALIENCIA_FRENTE - SALIENCIA_TRAS
    frente = -P / 2.0
    tras = P / 2.0

    # Materiais
    m_painel = _mat_plastico("u1.painel", p["cor_corpo"], aspereza=0.42)
    m_preto = _mat_plastico("u1.preto_fosco", "#15161A", aspereza=0.55)
    m_preto_brilho = _mat_plastico("u1.preto_brilho", "#0C0D10", aspereza=0.3, coat=0.4)
    m_camara = _mat_plastico("u1.camara", "#0F1013", aspereza=0.6)
    m_alu_x = _mat_aluminio("u1.aluminio.x", "X")
    m_alu_y = _mat_aluminio("u1.aluminio.y", "Y")
    m_alu_z = _mat_aluminio("u1.aluminio.z", "Z")
    m_fibra = _mat_fibra_carbono("u1.fibra_carbono", p["rugosidade_hastes"], p["anisotropia_hastes"])
    m_pei = _mat_pei("u1.pei")
    m_vidro = _mat_vidro("u1.vidro", "#8E9096", 0.02)
    # Mesma polidez do vidro da porta: com 0,10 a refracao raytraced virava
    # borrao e a camara nao se lia atras do painel.
    m_acrilico = _mat_vidro("u1.acrilico_traseiro", "#C4C6CC", 0.03)
    m_laranja = _mat_plastico("u1.laranja", "#F5891E", aspereza=0.4)
    m_latao = _mat_plastico("u1.latao", "#C9A24A", aspereza=0.35)
    m_latao.node_tree.nodes["Principled BSDF"].inputs["Metallic"].default_value = 1.0
    m_borracha = _mat_plastico("u1.borracha", "#111111", aspereza=0.8)
    m_led = _mat_emissivo("u1.led", "#FFF6E8", 0.0)
    m_botao = _mat_emissivo("u1.botao", "#D8241E", 0.0)
    m_botao.node_tree.nodes["Principled BSDF"].inputs["Roughness"].default_value = 0.3
    m_cinza_texto = _mat_plastico("u1.texto", "#3A3B40", aspereza=0.5)
    # Aro do topo mais escuro e menos especular que o preto fosco generico:
    # na 3/4 alta a luz rasante clareava o preto e o aro lia como faixa clara.
    m_aro = _mat_plastico("u1.aro", "#0C0D10", aspereza=0.5)
    _entrada(m_aro.node_tree.nodes["Principled BSDF"], "Specular IOR Level", 0.3)

    img_boot, ph_boot = _carregar_imagem(p["imagem_boot"], "u1.tela_boot", cor=(0, 0, 0, 1))
    img_ui, ph_ui = _carregar_imagem(p["imagem_ui"], "u1.tela_ui", cor=(0.09, 0.09, 0.1, 1))
    m_tela = _mat_tela("u1.tela", img_boot, img_ui, tuple(p["barra_boot_px"]), p["cor_trilho_boot"],
                       p["reflexo_tela_ligada"])

    # Raiz: a coreografia move isto e o U1 inteiro vai junto.
    raiz = bpy.data.objects.new("u1.raiz", None)
    raiz.empty_display_type = "ARROWS"
    raiz.empty_display_size = 0.2
    col.objects.link(raiz)

    # --- Casco branco ------------------------------------------------------
    # Casco de 4 mm acima do chao (pes) ate a face de baixo do aro (A - 24 mm):
    # o aro preto, com o MESMO footprint, senta em cima dele, como a moldura
    # do U1 real. Casco e aro nao se sobrepoem: quando o aro cobria o casco
    # ate 14 mm abaixo do topo, as faces externas coincidentes viravam uma
    # franja hachurada (z-fighting) em toda a borda superior, medida no
    # close da tela e na frente 3/4.
    z_aro = A - 0.024
    corpo = _caixa("u1.corpo", col, (L, P, z_aro - 0.004), (0, 0, (z_aro + 0.004) / 2), m_painel,
                   chanfro=0.025, segmentos=8, pai=raiz, peso=(1.0, 0.16), suave=False)
    # Bolso do topo (a camara aparece por ele), abertura da porta, janela
    # traseira, bolso da tela e rebaixo circular das laterais.
    _cortador("u1.cortador.topo", col, (L - 0.09, P - 0.11, A), (0, 0, 0.05 + A / 2), corpo, raiz)
    _cortador("u1.cortador.porta", col, (0.46, 0.12, 0.40), (0, frente, 0.255), corpo, raiz)
    _cortador("u1.cortador.tras", col, (0.42, 0.12, 0.48), (-0.02, tras, 0.34), corpo, raiz)
    _cortador("u1.cortador.tela", col, (0.104, 0.02, 0.070), (0.175, frente - 0.006, 0.585), corpo, raiz)
    for lado, x in (("esq", -L / 2), ("dir", L / 2)):
        _cortador("u1.cortador.disco." + lado, col,
                  _malha_cilindro("u1.cortador.disco." + lado, 0.17, 0.012, "X", 64),
                  (x, 0.02, 0.36), corpo, raiz)
    # Lateral: rasgos de ventilacao perto do fundo e a junta vertical entre o
    # painel frontal e o lateral, logo depois do canto arredondado. Um so
    # cortador com todos os rasgos: cada boolean EXACT custa na avaliacao.
    rasgos = []
    for sx in (-1, 1):
        for k in range(8):
            rasgos.append(((0.008, 0.036, 0.005), (sx * L / 2, -0.168 + k * 0.048, 0.055)))
        rasgos.append(((0.0024, 0.0025, A), (sx * L / 2, frente + 0.034, A / 2)))
    _cortador("u1.cortador.lateral", col, _malha_multicaixas("u1.cortador.lateral", rasgos), (0, 0, 0), corpo, raiz)
    # Quebra de quina de 1,5 mm nas arestas que os booleans criaram.
    _chanfro(corpo, 0.0015, 2, nome="quina")

    # Aro preto do topo, 10 mm acima do casco, com abertura um pouco menor que
    # o bolso (faz o labio). A boca do aro e um chanfro de 6 mm (o cortador e
    # um 'funil' com a parte de cima alargada a 45 graus) e o bordo externo do
    # topo e arredondado com 6 mm: e o que faz a moldura ler como moldura.
    # O aro cobre o casco inteiro (L x P, mesmos cantos): nas fotos oficiais
    # a moldura preta e rente aos paineis brancos, sem degrau branco em volta.
    # (use_smooth no aro nao muda um pixel: o harden_normals do Bevel ja dita
    # as normais - medido a 64 amostras, suave e plano dao as mesmas linhas.)
    aro = _caixa("u1.aro", col, (L, P, 0.024), (0, 0, A - 0.012), m_aro,
                 chanfro=0.025, segmentos=6, pai=raiz, peso=(1.0, 0.10), suave=False)
    _peso_arestas_topo(aro.data, 0.30)
    _cortador("u1.cortador.aro", col, _malha_funil("u1.cortador.aro", L - 0.10, P - 0.12, 0.1, 0.006), (0, 0, A), aro, raiz)
    _chanfro(aro, 0.0015, 2, nome="quina")
    for i, (sx, sy) in enumerate(((-1, -1), (1, -1), (-1, 1), (1, 1))):
        _cilindro("u1.tampao.%d" % (i + 1), col, 0.009, 0.003,
                  (sx * (L / 2 - 0.03), sy * (P / 2 - 0.03), A - 0.0015), m_preto_brilho, pai=raiz)

    # Camara: chao, paredes laterais e o painel traseiro transparente.
    cx, cy = L - 0.09, P - 0.11
    camara = _caixa("u1.camara.chao", col, (cx, cy, 0.012), (0, 0, 0.056), m_camara, chanfro=0, pai=raiz)
    for lado, sx in (("esq", -1), ("dir", 1)):
        _caixa("u1.camara.parede." + lado, col, (0.012, cy, A - 0.06),
               (sx * (cx / 2 - 0.006), 0, 0.05 + (A - 0.06) / 2), m_camara, chanfro=0, pai=raiz)
    # Parede traseira preta so acima e abaixo da janela; a janela leva acrilico.
    _caixa("u1.camara.parede.tras.baixo", col, (cx, 0.012, 0.06), (0, cy / 2 - 0.006, 0.08), m_camara, chanfro=0, pai=raiz)
    # As paredes de tras e da frente sobem ate a face de baixo do aro
    # (z_aro): a parede de tras parava 26 mm antes e o casco branco aparecia
    # entre ela e o aro - a 'barra branca' da foto A da rodada 2. Nas fotos
    # oficiais o interior sob a moldura e preto em toda a volta.
    _caixa("u1.camara.parede.tras.cima", col, (cx, 0.012, z_aro - 0.58), (0, cy / 2 - 0.006, (0.58 + z_aro) / 2), m_camara, chanfro=0, pai=raiz)
    # Frente: so acima do vao da porta (0,055..0,455), para nao aparecer por ela.
    _caixa("u1.camara.parede.frente", col, (cx, 0.012, z_aro - 0.46), (0, -cy / 2 + 0.006, (0.46 + z_aro) / 2), m_camara, chanfro=0, pai=raiz)
    painel_traseiro = _caixa("u1.painel_traseiro", col, (0.42, 0.004, 0.48), (-0.02, tras - 0.03, 0.34), m_acrilico, chanfro=0.001, segmentos=2, pai=raiz)

    # Frente: wordmark e tela.
    logo_curva = bpy.data.curves.new("u1.logo", "FONT")
    logo_curva.body = "snapmaker"
    logo_curva.size = 0.026
    logo_curva.extrude = 0.0002
    logo_curva.align_x = "LEFT"
    logo_curva.materials.append(m_cinza_texto)
    logo = _novo_objeto("u1.logo", logo_curva, col, (-0.215, frente - 0.0002, 0.575), raiz)
    logo.rotation_euler = (math.pi / 2, 0, 0)

    tela_malha = _malha_caixa("u1.tela", 0.104, 0.004, 0.070)
    tela = _novo_objeto("u1.tela", tela_malha, col, (0.175, frente + 0.0018, 0.585), raiz)
    tela_malha.materials.append(m_tela)
    # UV so na face da frente (-Y): u cresce com x, v cresce com z.
    uv_layer = tela_malha.uv_layers.new(name="UVMap")
    for poly in tela_malha.polygons:
        for li in poly.loop_indices:
            v = tela_malha.vertices[tela_malha.loops[li].vertex_index].co
            uv_layer.data[li].uv = (v.x / 0.104 + 0.5, v.z / 0.070 + 0.5)
    _suavizar(tela_malha)
    _chanfro(tela, 0.0006, 2)
    centro_tela = Vector((0.175, frente, 0.585))

    # --- Porta de vidro (pivo na dobradica esquerda) ------------------------
    porta_l, porta_a, borda, esp = 0.47, 0.41, 0.014, 0.014
    porta = _caixa("u1.porta", col, (porta_l, esp, porta_a), (0, frente - esp / 2, 0.255), m_preto_brilho,
                   chanfro=0.002, segmentos=3, pai=raiz)
    _cortador("u1.cortador.porta.vao", col, (porta_l - 2 * borda, 0.05, porta_a - 2 * borda),
              (0, frente - esp / 2, 0.255), porta, raiz)
    # Origem na dobradica: mover o pivo e so deslocar a malha para o outro lado.
    dobradica = Vector((-porta_l / 2, frente - esp / 2, 0.255))
    for v in porta.data.vertices:
        v.co += Vector((porta_l / 2, 0, 0))
    porta.location = dobradica
    porta.rotation_euler = (0, 0, -math.radians(p["porta_aberta_graus"]))
    vidro = _caixa("u1.porta.vidro", col, (porta_l - 2 * borda + 0.006, 0.004, porta_a - 2 * borda + 0.006),
                   (0, frente - esp / 2, 0.255), m_vidro, chanfro=0.0008, segmentos=2, pai=None)
    vidro.parent = porta
    vidro.location = Vector((porta_l / 2, 0, 0))
    puxador = _caixa("u1.puxador", col, (0.012, 0.016, 0.085), (porta_l / 2 - 0.028, frente - esp - 0.008, 0.255),
                     m_preto_brilho, chanfro=0.003, segmentos=3, pai=None)
    puxador.parent = porta
    puxador.location = Vector((porta_l - 0.028, -esp / 2 - 0.008, 0))
    for i, (sx, sz) in enumerate(((-1, -1), (1, -1), (-1, 1), (1, 1))):
        par = _cilindro("u1.porta.parafuso.%d" % (i + 1), col, 0.0022, 0.001,
                        (0, 0, 0), m_alu_y, eixo="Y", pai=None, segmentos=16)
        par.parent = porta
        par.location = Vector((porta_l / 2 + sx * (porta_l / 2 - 0.007), -esp / 2 - 0.0005, sz * (porta_a / 2 - 0.007)))

    # --- Mecanica visivel pelo topo -----------------------------------------
    # Guias Y (aluminio escovado) nas duas laterais internas, e as duas hastes
    # de fibra de carbono do eixo X com o carro branco.
    for lado, sx in (("esq", -1), ("dir", 1)):
        _caixa("u1.guia.y." + lado, col, (0.012, cy - 0.05, 0.012), (sx * (cx / 2 - 0.02), 0.0, 0.640), m_alu_y, chanfro=0.001, segmentos=2, pai=raiz)
    hastes = []
    for i, y in enumerate((-0.045, 0.0)):
        hastes.append(_cilindro("u1.haste.%d" % (i + 1), col, 0.006, cx - 0.05, (0, y, 0.622), m_fibra, eixo="X", pai=raiz))
    carro = _caixa("u1.carro", col, (0.062, 0.075, 0.115), (-0.16, -0.022, 0.628), m_painel, chanfro=0.006, segmentos=4, pai=raiz)
    vent = _cilindro("u1.carro.ventoinha", col, 0.016, 0.003, (-0.16, -0.022 - 0.0375 - 0.001, 0.612), m_laranja, eixo="Y", pai=raiz)
    _cilindro("u1.carro.ventoinha.aro", col, 0.019, 0.002, (-0.16, -0.022 - 0.0375 - 0.0005, 0.612), m_preto, eixo="Y", pai=raiz, raio2=0.019)
    _caixa("u1.carro.fenda", col, (0.03, 0.002, 0.006), (-0.16, -0.022 - 0.0375 - 0.0005, 0.668), m_preto, chanfro=0.0005, segmentos=1, pai=raiz)
    # Fuso Z (aluminio, tangente radial em Z) no fundo da camara.
    _cilindro("u1.fuso.z", col, 0.006, 0.52, (0.0, cy / 2 - 0.03, 0.06 + 0.26), m_alu_z, pai=raiz, segmentos=32)

    # Viga de estacionamento no fundo do topo e os 4 cabecotes.
    _caixa("u1.viga", col, (cx - 0.03, 0.06, 0.09), (0, cy / 2 - 0.045, 0.655), m_preto, chanfro=0.003, segmentos=3, pai=raiz)
    cabecotes = []
    for i, x in enumerate((-0.135, -0.045, 0.045, 0.135)):
        cabecotes.append(_cabecote(col, i + 1, Vector((x, 0.115, 0.575)), m_preto, m_preto_brilho, m_laranja, m_alu_z, m_latao, raiz))

    # LEDs no teto da camara, na frente - e uma area light por fita, porque a
    # malha emissiva nao ilumina a camara no EEVEE. Nascem escondidas e a 0 W;
    # animar_ligar e quem as acende.
    leds = []
    luzes = []
    for i, x in enumerate((-0.11, 0.11)):
        leds.append(_caixa("u1.led.%d" % (i + 1), col, (0.16, 0.012, 0.003), (x, -cy / 2 + 0.03, A - 0.032), m_led, chanfro=0.0005, segmentos=1, pai=raiz))
        luzes.append(_luz_de_fita("u1.led.luz.%d" % (i + 1), col, Vector((x, -cy / 2 + 0.03, A - 0.036)), (0.16, 0.012), raiz))

    # --- Mesa -----------------------------------------------------------------
    _caixa("u1.mesa.carro", col, (0.30, 0.30, 0.012), (0, 0.0, 0.146), m_preto, chanfro=0.002, segmentos=2, pai=raiz)
    mesa = _caixa("u1.mesa", col, (0.285, 0.285, 0.0015), (0, 0.0, 0.1528), m_pei, chanfro=0.0006, segmentos=1, pai=raiz)

    # --- Traseira: coluna com USB, botao e tomada ------------------------------
    col_x, col_y = 0.245, tras
    coluna = _caixa("u1.coluna", col, (0.06, 0.016, 0.30), (col_x, col_y, 0.23), m_painel, chanfro=0.006, segmentos=4, pai=raiz)
    face_coluna = col_y + 0.008
    # USB-A laranja e conector "Add-on"
    _caixa("u1.usb.aro", col, (0.015, 0.004, 0.008), (col_x - 0.012, face_coluna, 0.255), m_preto, chanfro=0.0005, segmentos=1, pai=raiz)
    _caixa("u1.usb", col, (0.012, 0.003, 0.004), (col_x - 0.012, face_coluna + 0.0005, 0.255), m_laranja, chanfro=0.0003, segmentos=1, pai=raiz)
    _caixa("u1.addon", col, (0.010, 0.004, 0.008), (col_x + 0.012, face_coluna, 0.255), m_preto, chanfro=0.0005, segmentos=1, pai=raiz)
    # Botao gangorra vermelho no aro preto.
    _caixa("u1.botao.aro", col, (0.024, 0.005, 0.032), (col_x, face_coluna + 0.001, 0.175), m_preto, chanfro=0.0008, segmentos=2, pai=raiz)
    botao = _caixa("u1.botao", col, (0.016, 0.006, 0.024), (col_x, face_coluna + 0.004, 0.175), m_botao, chanfro=0.0015, segmentos=2, pai=raiz)
    _caixa("u1.botao.traco", col, (0.001, 0.001, 0.007), (col_x, face_coluna + 0.0075, 0.181), m_painel, chanfro=0, pai=raiz)
    centro_botao = Vector((col_x, face_coluna + 0.007, 0.175))
    # Tomada IEC C14: aro preto, cavidade escura e 3 pinos de latao.
    tomada_aro = _caixa("u1.tomada.aro", col, (0.032, 0.005, 0.026), (col_x, face_coluna + 0.001, 0.125), m_preto, chanfro=0.001, segmentos=2, pai=raiz)
    # Furo no aro E na coluna: sem os dois, a cavidade fica escondida dentro
    # do solido branco e a tomada rende como um quadrado liso.
    _cortador("u1.cortador.tomada.aro", col, (0.023, 0.03, 0.017), (col_x, face_coluna, 0.125), tomada_aro, raiz)
    _cortador("u1.cortador.tomada.coluna", col, (0.026, 0.03, 0.020), (col_x, face_coluna, 0.125), coluna, raiz)
    _chanfro(tomada_aro, 0.0006, 1, nome="quina")
    # Carcaca preta da tomada com bolso aberto para a frente; os pinos ficam
    # dentro do bolso, como num C14 de verdade.
    tomada = _caixa("u1.tomada", col, (0.030, 0.020, 0.024), (col_x, face_coluna - 0.011, 0.125), m_camara, chanfro=0.0, pai=raiz)
    _cortador("u1.cortador.tomada.bolso", col, (0.024, 0.024, 0.018), (col_x, face_coluna - 0.001, 0.125), tomada, raiz)
    for j, (px, pz) in enumerate(((-0.007, -0.001), (0.007, -0.001), (0.0, 0.004))):
        _caixa("u1.tomada.pino.%d" % (j + 1), col, (0.0018, 0.009, 0.0045), (col_x + px, face_coluna - 0.0075, 0.125 + pz), m_latao, chanfro=0.0003, segmentos=1, pai=raiz)
    ponto_tomada = Vector((col_x, face_coluna + 0.0035, 0.125))

    # Pes de borracha.
    for i, (sx, sy) in enumerate(((-1, -1), (1, -1), (-1, 1), (1, 1))):
        _cilindro("u1.pe.%d" % (i + 1), col, 0.022, 0.004, (sx * (L / 2 - 0.05), sy * (P / 2 - 0.05), 0.002), m_borracha, pai=raiz)

    tubos = []
    if p["tubos"]:
        tubos = _tubos(col, cabecotes, raiz)

    # O casco e mais curto na frente (porta + puxador) do que atras (coluna):
    # deslocar tudo 7 mm para +Y centra o ENVELOPE em y = 0, que e o que a
    # caixa e a coreografia esperam. Os filhos indiretos (porta, cabecotes)
    # vao junto com os pais.
    deslocamento_y = (SALIENCIA_FRENTE - SALIENCIA_TRAS) / 2.0
    for obj in col.objects:
        if obj.parent is raiz:
            obj.location.y += deslocamento_y
    desloc = Vector((0, deslocamento_y, 0))
    centro_tela += desloc
    centro_botao += desloc
    ponto_tomada += desloc

    cena.view_layers[0].update() if hasattr(cena, "view_layers") else None
    envelope_min, envelope_max = _envelope(col)
    dimensoes = tuple(envelope_max[i] - envelope_min[i] for i in range(3))

    return {
        "raiz": raiz,
        "corpo": corpo,
        "aro": aro,
        "camara": camara,
        "tela": tela,
        "logo": logo,
        "botao": botao,
        "tomada": tomada,
        "cabecotes": cabecotes,
        "carro": carro,
        "hastes": hastes,
        "porta": porta,
        "porta_vidro": vidro,
        "puxador": puxador,
        "mesa": mesa,
        "painel_traseiro": painel_traseiro,
        "leds": leds,
        "led": leds[0],
        # Area lights das fitas (hide_render=True e 0 W ate animar_ligar).
        "luzes_led": luzes,
        "tubos": tubos,
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
        "materiais": {"tela": m_tela, "led": m_led, "botao": m_botao, "aro": m_aro},
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


def _malha_multicaixas(nome, caixas):
    """Varias caixas (dims, centro) numa malha so, para um unico boolean."""
    bm = bmesh.new()
    for (dx, dy, dz), (x, y, z) in caixas:
        novo = bmesh.ops.create_cube(bm, size=1.0)["verts"]
        bmesh.ops.scale(bm, vec=(dx, dy, dz), verts=novo)
        bmesh.ops.translate(bm, vec=(x, y, z), verts=novo)
    malha = bpy.data.meshes.new(nome)
    bm.to_mesh(malha)
    bm.free()
    return malha


def _malha_funil(nome, dx, dy, dz, chanfro):
    """Caixa cuja parte de cima alarga a 45 graus a partir de 'chanfro' abaixo de z=0.

    Cortando o aro com isto, a boca ganha um chanfro de 'chanfro' de largura
    no topo (z=0 e o topo do aro) e paredes retas abaixo dele.
    """
    bm = bmesh.new()
    niveis = ((-dz / 2, 0.0), (-chanfro, 0.0), (dz / 2, dz / 2 + chanfro))
    aneis = []
    for z, alarga in niveis:
        hx, hy = dx / 2 + alarga, dy / 2 + alarga
        aneis.append([bm.verts.new(v) for v in ((-hx, -hy, z), (hx, -hy, z), (hx, hy, z), (-hx, hy, z))])
    bm.faces.new(aneis[0][::-1])
    for a, b in zip(aneis, aneis[1:]):
        for k in range(4):
            j = (k + 1) % 4
            bm.faces.new((a[k], a[j], b[j], b[k]))
    bm.faces.new(aneis[-1])
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    malha = bpy.data.meshes.new(nome)
    bm.to_mesh(malha)
    bm.free()
    return malha


def _peso_arestas_topo(malha, peso):
    """Bevel weight das arestas da face de cima: bordo arredondado so no topo."""
    attr = malha.attributes.get("bevel_weight_edge")
    if attr is None:
        return
    z_max = max(v.co.z for v in malha.vertices)
    for i, aresta in enumerate(malha.edges):
        a = malha.vertices[aresta.vertices[0]].co
        b = malha.vertices[aresta.vertices[1]].co
        if abs(a.z - z_max) < 1e-6 and abs(b.z - z_max) < 1e-6:
            attr.data[i].value = peso


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


def _cabecote(col, n, pos, m_preto, m_brilho, m_laranja, m_alu, m_latao, raiz):
    """Bloco preto docado, trava laranja, grade da ventoinha, bloco aquecedor e bico."""
    nome = "u1.cabecote.%d" % n
    corpo = _caixa(nome, col, (0.046, 0.062, 0.125), pos, m_preto, chanfro=0.0025, segmentos=3, pai=raiz)
    filhos = []
    # Trava SnapSwap laranja no alto da frente.
    filhos.append(_caixa(nome + ".trava", col, (0.030, 0.006, 0.010), pos + Vector((0, -0.032, 0.048)), m_laranja, chanfro=0.001, segmentos=2, pai=None))
    # Grade da ventoinha: fundo escuro rebaixado, tres aneis concentricos,
    # quatro raios e o cubo. Um disco liso lia como botao num close.
    centro_grade = pos + Vector((0, -0.0315, 0.010))
    filhos.append(_cilindro(nome + ".grade.fundo", col, 0.0145, 0.002, centro_grade + Vector((0, 0.0015, 0)), m_preto, eixo="Y", pai=None))
    for k, r_ext in enumerate((0.0140, 0.0100, 0.0060)):
        filhos.append(_anel("%s.grade.anel.%d" % (nome, k + 1), col, r_ext - 0.0016, r_ext, 0.0015, centro_grade, m_preto, eixo="Y", pai=None))
    for k in range(4):
        raio = _caixa("%s.grade.raio.%d" % (nome, k + 1), col, (0.0012, 0.0015, 0.0276), centro_grade, m_preto, chanfro=0.0003, segmentos=1, pai=None)
        raio.rotation_euler = (0, math.radians(45 * k), 0)
        filhos.append(raio)
    filhos.append(_cilindro(nome + ".grade.eixo", col, 0.0035, 0.0025, centro_grade, m_brilho, eixo="Y", pai=None, segmentos=24))
    # Fita de identificacao (lateral) e etiqueta.
    filhos.append(_caixa(nome + ".etiqueta", col, (0.047, 0.020, 0.012), pos + Vector((0, 0.010, 0.030)), m_laranja, chanfro=0.0008, segmentos=1, pai=None))
    # Conector de tubo no topo (branco) e bloco aquecedor + bico embaixo.
    filhos.append(_cilindro(nome + ".conector", col, 0.004, 0.010, pos + Vector((0, -0.015, 0.0675)), m_alu, pai=None, segmentos=24))
    filhos.append(_caixa(nome + ".aquecedor", col, (0.016, 0.020, 0.011), pos + Vector((0, -0.010, -0.068)), m_alu, chanfro=0.001, segmentos=2, pai=None))
    filhos.append(_cilindro(nome + ".bico", col, 0.0035, 0.007, pos + Vector((0, -0.010, -0.077)), m_latao, pai=None, segmentos=24, raio2=0.0012))
    # Fendas de ventilacao nas laterais e placa/conector no topo: e o que
    # separa "bloco preto" de "cabecote" num close.
    for lado, sx in (("esq", -1), ("dir", 1)):
        for k in range(4):
            filhos.append(_caixa("%s.fenda.%s.%d" % (nome, lado, k + 1), col, (0.0015, 0.020, 0.0025),
                                 pos + Vector((sx * 0.0232, -0.008, -0.030 + k * 0.007)), m_brilho, chanfro=0, pai=None))
    filhos.append(_caixa(nome + ".placa", col, (0.036, 0.040, 0.004), pos + Vector((0, 0.008, 0.0645)), m_brilho, chanfro=0.0008, segmentos=1, pai=None))
    filhos.append(_caixa(nome + ".conector.placa", col, (0.010, 0.012, 0.007), pos + Vector((0, 0.018, 0.070)), m_preto, chanfro=0.0006, segmentos=1, pai=None))
    # Bracadeira do dock, presa na viga atras do cabecote.
    filhos.append(_caixa(nome + ".dock", col, (0.054, 0.014, 0.040), pos + Vector((0, 0.038, 0.030)), m_preto, chanfro=0.002, segmentos=2, pai=None))
    filhos.append(_cilindro(nome + ".dock.esfera.1", col, 0.004, 0.004, pos + Vector((-0.018, 0.031, 0.030)), m_alu, eixo="Y", pai=None, segmentos=20))
    filhos.append(_cilindro(nome + ".dock.esfera.2", col, 0.004, 0.004, pos + Vector((0.018, 0.031, 0.030)), m_alu, eixo="Y", pai=None, segmentos=20))
    for f in filhos:
        f.parent = corpo
        f.location -= pos
    return corpo


def _tubos(col, cabecotes, raiz):
    """Lacos de PTFE do topo de cada cabecote ate a borda traseira (opcional)."""
    m = _mat_vidro("u1.ptfe", "#DDE0E4", 0.35)
    tubos = []
    for i, cab in enumerate(cabecotes):
        curva = bpy.data.curves.new("u1.tubo.%d" % (i + 1), "CURVE")
        curva.dimensions = "3D"
        curva.bevel_depth = 0.002
        curva.bevel_resolution = 4
        curva.materials.append(m)
        sp = curva.splines.new("BEZIER")
        sp.bezier_points.add(2)
        base = cab.location + Vector((0, -0.015, 0.0725))
        pts = (base, base + Vector((0, 0.06, 0.24)), base + Vector((0, 0.12, 0.0)))
        for bp, pt in zip(sp.bezier_points, pts):
            bp.co = pt
            bp.handle_left_type = bp.handle_right_type = "AUTO"
        obj = _novo_objeto("u1.tubo.%d" % (i + 1), curva, col, (0, 0, 0), raiz)
        tubos.append(obj)
    return tubos


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
# Animacao (so nos proprios objetos; Bezier suave)
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

    Mantida como na rodada 1 (curso, quadros e forcas iguais); so a busca do
    socket de emissao passou a seguir o link do Output, que para os
    materiais daqui da o mesmo socket. Para ligar como evento de luz use
    animar_ligar.
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
    60 W por fita porque a camara e escura (paredes #0F1013, chao preto): com
    25 W a janela traseira subia 1% de luminancia - o que a luz encontra e a
    mesa dourada, os cabecotes e o fundo.
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
