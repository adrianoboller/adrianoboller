# Modulo COREOGRAFIA do anuncio do Snapmaker U1.
#
# E o integrador: constroi a cena com os outros modulos (ambiente, caixa, u1,
# cabo, cartela, camera) e escreve os sete beats do storyboard sobre eles.
# So definicoes aqui - nada roda no import. Quem prova e teste_coreografia.py;
# o arquivo unico (anuncio_u1.py, gerado por montar.py) chama construir_tudo,
# coreografar e configurar_render.
#
# DECISOES:
#
# - A LINHA DO TEMPO E DADO. A tabela BEATS (segundos) e a da especificacao;
#   ROTEIRO da a fracao de cada beat em que cada acao acontece. Todo quadro
#   sai de quadro(t, fator): mudar a duracao (preset de 15 s = fator 0,75)
#   escala tudo junto, inclusive as folgas anti-colisao, que sao razoes.
#
# - A CAIXA DESCE E SOME no beat 2 (em vez de o U1 pousar ao lado dela) - e
#   o PADRAO, com o parametro 'caixa_some' para o cliente escolher. O motivo
#   e o beat 3: a camera orbita 180 graus ate a traseira para ver o cabo
#   entrar numa tomada a 12 cm do chao. Com a caixa de 0,8 m atras do U1 (ou
#   ao lado), ela entraria entre a camera e a tomada em parte da orbita. Com
#   o U1 sozinho na origem, a orbita e os closes do beat 5 tem 360 graus
#   livres, e o rig de luzes (centrado na origem) continua certo. No beat 6 a
#   caixa volta pelo chao enquanto o U1 flutua acima dela - e o mesmo truque
#   ao contrario, e a ordem (U1 sobe, caixa sobe, U1 desce, espuma volta,
#   tampa fecha) e a que nao atravessa nada: conferir_colisoes mede isso
#   quadro a quadro.
#   Com caixa_some=False (o texto do cliente ao pe da letra: 'o U1 sai da
#   caixa') o U1 sobe, DESLIZA para -Y ('deslocamento_u1', 2,1 m) e pousa na
#   frente da caixa, que fica parada atras dele, fora do raio 1,7 da orbita
#   (a face da caixa fica a 1,8 m do centro do U1). Os dois rigs (camera e
#   luzes) acompanham o U1 por chave de posicao nos beats 3-5 - as chaves
#   (azimute, raio, altura) continuam iguais nos dois modos - e no beat 6 o
#   U1 volta por cima da caixa e desce nela. O plano do beat 6 nesse modo e
#   de lado (az -30), porque de frente o U1 estaria colado na camera.
#
# - A CAMERA anda num Empty 'camera.orbita' na origem: as chaves sao
#   (azimute, raio, altura) em vez de XYZ. Orbita vira uma rampa de angulo
#   (arco exato, nao poligono), dolly vira uma rampa de raio, e as duas se
#   misturam com Bezier continuo entre beats - a camera nunca para seca:
#   entre um beat e outro a chave e uma so, e as chaves de Bezier
#   auto-clamped nao zeram a velocidade no meio de um trecho. Os cortes do
#   beat 5 e o corte para a cartela sao chaves CONSTANT no ultimo quadro do
#   plano anterior, e o obturador do motion blur abre em START (nao CENTER),
#   para o quadro do corte nao borrar entre dois planos.
#
# - O FOCO tem Empty proprio ('camera.foco'): nos beats 1-6 ele copia o alvo
#   da camera (foco segue o que a camera olha); no beat 7 o alvo fica 1 m
#   abaixo da camera (para o Track To nao degenerar olhando reto para baixo)
#   e o foco fica na logo; na cartela o foco vai para a cartela.
#
# - O que nao deve aparecer e ESCONDIDO por chave de hide_render: o cabo
#   antes de entrar (estaria deitado no chao atras do U1 desde o quadro 1), a
#   tampa enquanto flutua a 1,6 m ao lado (apareceria nas orbitas), a cartela
#   antes do corte. E o que o modulo de cada peca nao oferece.
#
# RODADA 2 (o que a revisao em docs/REVISAO-RODADA-1.md mediu e o que mudou):
#
# - FLASH: a chamada com forca=1.0 saiu (o padrao do ambiente e 0,5 = emissao
#   8, um quadro de pico e decaimento), e o veu agora e CONSTANT em q-1 e q:
#   o quadro anterior a cada corte (359, 389, 419) nao expoe mais meio flash
#   sob o obturador START. Provado rerenderizando q-1/q/q+1 dos tres cortes.
#
# - LUZ NUNCA MUDA POR CHAVE CONSTANT NO MEIO DE UM PLANO. O specular do rim
#   (0 nos planos largos, 0,5 nos do produto - ver construir_tudo) vai por
#   chavear_especular do ambiente: rampa Bezier de 12 quadros que so comeca
#   quando a camera ja passou de azimute ~0 (q_ini + 0,25*(q_orb-q_ini) =
#   q178), onde o produto cobre a poca do reflexo no chao. No beat 2 o rim
#   sobe a 0,3 entre q(0,50) e q(0,90) - o chao ali esta coberto pelo U1 e
#   pela caixa - para o U1 branco recortar contra o rose (momento-heroi). Os
#   cortes de verdade (q450) continuam corte: rampa=0.
#
# - MODELO REAL IDEMPOTENTE: _u1_real guarda a matriz ORIGINAL de cada
#   objeto do cliente numa propriedade ('anuncio.matriz_original', mais o pai
#   e a inversa do pai) na primeira rodada e a RESTAURA no inicio de toda
#   rodada seguinte (tambem quando se volta ao substituto), antes de medir e
#   cozinhar de novo; a colecao 'u1' do substituto e limpa antes; a raiz e
#   sempre nova. A revisao provou o oposto: a segunda rodada media o
#   substituto e o bloco juntos, e a terceira dobrava a rotacao. Provado com
#   tres rodadas do arquivo unico (substituto, real, real): contagens iguais
#   entre 2 e 3 e a matriz do cubo igual nas duas.
#   E o substituto RECUSA rodar se existir uma colecao 'u1' que nao seja a
#   dele (limpar_colecao apagaria o modelo do cliente em silencio).
#
# - BEAT 3: azimute monotono (105 em q_orb -> 110 -> 120 em q_fim; antes
#   recuava 5 graus e a camera parava em q269 por ser minimo local), raio
#   1,7 -> 1,25 -> 1,15 (push-in leve no ligar) e o LIGAR e um evento de luz:
#   animar_ligar do u1 acende as fitas e as area lights da camara (o interior
#   aparece pelo acrilico traseiro). O plugue voa de mais alto (origem a 0,45
#   m, arco de 0,30) para cruzar o quadro contra o corpo branco e a faixa
#   rose em vez de preto sobre o chao preto.
#
# - BEAT 4: o dolly termina em q(0,78) e a MESMA chave e repetida em q_fim-1
#   com CONSTANT: duas chaves iguais seguram a tela parada 19 quadros. Boot
#   em 0,60 e UI em 0,85, para a UI entrar com a camera parada. Chave
#   intermediaria em q(0,30) com raio 2,1 e alvo no corpo para o meio da
#   orbita nao cortar o U1 na borda.
#
# - BEAT 5: rim 250/400/300 W nas fotos (era 550/650/800, estourava), fitas a
#   3,0 de emissao (abaixo do bloom), foto C reenquadrada de FORA da pegada
#   (camera acima e a frente-direita do aro, mesa e hastes na diagonal,
#   mesa no canto inferior direito), e cada foto e um push-in de 0,06 m em
#   30 quadros na direcao do sujeito com a lente indo de 50 a 52 mm (60 a 62
#   na A) em LINEAR - vida sem perder a ideia de 'foto'.
#
# - BEAT 6 E BEAT 1 aproveitam o 9:16: raio 2,2/2,1 e altura 1,0/1,7 no
#   beat 6 (era 3,0/2,8 e 1,3/2,3: produto a 28% da altura), com uma chave no
#   pico da subida do U1 (alvo a 0,78 m) para o topo dele nao sair do quadro
#   enquanto flutua; beat 1 acaba a orbita em -80 graus (3/4 leve) e raio
#   2,1; e a tampa comeca RENTE ao chao (profundidade = topo_tampa_z, nao
#   +0,25): o primeiro quadro ja tem produto, nao 0,27 s de chao vazio.
#
# - BEAT 7, TRAVESSIA DE VERDADE: o mergulho e uma chave por quadro com um
#   perfil de Hermite (parte parado no apice, acelera e chega a 0,047
#   m/quadro em vez de parar) ate 0,12 m da logo, segue LINEAR ate 0,02 m
#   DENTRO da tampa (clip_start 0,01), e o veu preto (o plano do flash com
#   emissao 0) sobe de alfa 0 a 1 nos dois quadros antes de a camera tocar a
#   tampa, para o preto nascer da propria logo - antes era um corte seco a
#   12 cm. A abertura vai de f/2,8 a f/8 no mergulho (LINEAR) para a logo
#   ficar nitida ate o veu. O corte cai na cartela com a logo JA visivel e
#   maior no centro (animar_cartela com logo_ja_visivel e escala inicial
#   1,6), que viaja ao repouso enquanto o texto entra: match cut logo -> logo.
#   O veu se aproxima da camera (1,5 cm) so nesses quadros: a 25 cm ele
#   estaria atras da tampa e nao apareceria.
#
# - Imagens empacotadas (empacotar_imagens do ambiente) antes de salvar o
#   .blend: a revisao mediu logo e telas apontando para a pasta temporaria.
#
# - O que a rodada 2 mediu e NAO era o que parecia: (a) a 'barra branca' da
#   foto A nao e a fita de LED - com a fita a 3,0 e a 1,2 o render e igual;
#   e o labio branco do casco sob o aro, visto de cima com a key em cima
#   (geometria do u1, nao luz); (b) o veu preto cobria so metade do quadro
#   nao por causa do plano, e sim do DoF com o foco a 2,7 cm/negativo (ver
#   'foco_min'); (c) baixar a camera no momento-heroi piorava o recorte
#   (ver 'camera_heroi'); (d) o rig de luz da orbita ficou em +90 (o
#   ambiente mediu +60 mais chapado que +90 no cubo branco; a face lateral
#   'slab' e o tamanho da key, nao o angulo do rim).
#
# Eixos e medidas seguem docs/ESPECIFICACAO.md: metros, Z para cima, frente
# em -Y, origem no centro da base da caixa, chao em z = 0.

import math
import os

import bpy
from mathutils import Matrix, Vector

import mod_ambiente
import mod_cabo
import mod_caixa
import mod_cartela
import mod_u1

NOME = "coreografia"
FPS = 30.0
DURACAO_REFERENCIA = 20.0

# Propriedades gravadas nos objetos do cliente (modelo real) na primeira
# rodada, para as seguintes partirem da pose ORIGINAL e nao da cozida.
PROP_MATRIZ = "anuncio.matriz_original"
PROP_PAI = "anuncio.pai_original"
PROP_PAI_INVERSA = "anuncio.pai_inversa_original"

# Tabela de beats da especificacao (segundos, na duracao de referencia).
BEATS = (
    {"n": 1, "nome": "caixa_sobe", "t_ini": 0.0, "t_fim": 2.5},
    {"n": 2, "nome": "abre", "t_ini": 2.5, "t_fim": 5.5},
    {"n": 3, "nome": "traseira", "t_ini": 5.5, "t_fim": 9.0},
    {"n": 4, "nome": "tela", "t_ini": 9.0, "t_fim": 12.0},
    {"n": 5, "nome": "fotos", "t_ini": 12.0, "t_fim": 15.0},
    {"n": 6, "nome": "volta", "t_ini": 15.0, "t_fim": 17.0},
    {"n": 7, "nome": "cartela", "t_ini": 17.0, "t_fim": 20.0},
)
PRESETS = {"20s": 1.0, "15s": 0.75}

# Fracao de cada beat (0 = inicio, 1 = fim) em que cada acao comeca e acaba.
# A ordem dentro dos beats 2 e 6 e a que nao atravessa nada - ver cabecalho.
ROTEIRO = {
    2: {
        "tampa": (0.00, 0.30),          # tampa sai rapido, antes da espuma
        "espuma": (0.22, 0.85),         # explode depois que a tampa saiu de cima
        "u1_sobe": (0.50, 0.75),        # so depois de toda espuma ter saltado
        "caixa_desce": (0.62, 0.92),    # a caixa some pelo chao sob o U1
        "u1_desce": (0.80, 1.00),       # U1 pousa no chao, na origem
        "u1_desliza": (0.75, 0.94),     # (caixa_some=False) U1 vai para -Y no ar
        "rim": (0.50, 0.90),            # rim a 0,3 no momento-heroi
    },
    3: {
        "orbita": (0.00, 0.48),         # frente -> traseira pelo lado +X
        "rim": 0.25,                    # fracao da orbita em que o rim comeca a subir
        "cabo": (0.19, 0.71),
        "botao": (0.77, 0.98),
        "push_in": (0.77, 1.00),        # raio 1,25 -> 1,15 no ligar
    },
    4: {
        "orbita": (0.00, 0.56),         # traseira -> frente pelo lado -X
        "dolly": 0.78,                  # fim do dolly; a chave repete em q_fim-1
        "boot": 0.60,                   # boot de ~0,8 s, corte seco para a UI
        "ui": 0.85,
    },
    5: {"fotos": (0.0, 1.0 / 3.0, 2.0 / 3.0)},
    6: {
        "u1_sobe": (0.00, 0.267),
        "caixa_sobe": (0.033, 0.333),
        "u1_desce": (0.40, 0.667),
        "espuma": (0.60, 0.867),
        "tampa": (0.80, 1.00),
    },
    7: {
        "sobe_para_logo": (0.00, 0.20),
        "mergulho": (0.20, 0.444),      # quadro da travessia = fim do mergulho
        "cartela": (0.444, 0.86),
    },
}

PARAMS_PADRAO = {
    # '' = U1 substituto; nome de objeto ou de colecao = modelo real do cliente.
    "u1_nome": "",
    "duracao_s": DURACAO_REFERENCIA,
    "cor_caixa": "clara",
    "pasta_assets": None,          # None = <raiz do projeto>/assets
    # Modelo real: rotacao em Z (graus) que poe a frente dele em -Y, e os
    # pontos de tela/tomada/botao nas coordenadas ORIGINAIS do arquivo dele
    # (antes de centralizar); None = heuristica pelo bounding box.
    "u1_rotacao_z": 0.0,
    "u1_tela": None,
    "u1_tomada": None,
    "u1_botao": None,
    # Objetos do modelo real que recebem animacao (nomes); '' = nao anima.
    "u1_tela_objeto": "",
    "u1_botao_objeto": "",
    "u1_led_objeto": "",
    "u1": {},                      # params extras do mod_u1 (cor_corpo...)
    "ambiente": {},
    "camera": {},
    "cartela": {},
    "caixa": {},
    # Quanto o U1 sobe acima do topo do corpo da caixa ao sair/entrar.
    "folga_u1": 0.14,
    # Deslocamento da cartela para cima no quadro (m a 2 m). None = o padrao
    # medido pelo modulo cartela (0,18: linha 4 fora da faixa de legendas).
    "cartela_subida": None,
    # True (padrao): a caixa afunda pelo chao no beat 2 e volta no beat 6.
    # False: o U1 desliza para -Y e pousa na frente da caixa (ver cabecalho).
    "caixa_some": True,
    "deslocamento_u1": 2.1,        # m para -Y, so com caixa_some=False
    # Rig de luz na orbita: rig = azimute da camera + offset. 90 poe o rim
    # atras do produto (padrao medido do ambiente); 60 e a opcao lateral.
    "offset_rig_orbita": mod_ambiente.OFFSET_RIM_ATRAS,
    # Beat 3: de onde o plugue parte (altura, m) e altura do arco.
    "origem_cabo_z": 0.45,
    "arco_cabo": 0.30,
    # Beat 5: energia do rim e da key em cada foto (W), das area lights da
    # camara (W; 60 e o valor do ligar) e emissao das fitas de LED nas fotos.
    # Medido: na foto C (de cima, mesa a 1,2 m) as luzes da camara a 60 W e a
    # key a 240 W estouravam mesa e carro em branco; na A a fita a 3,0 era
    # uma barra branca atravessando o quadro.
    "rim_fotos": (250.0, 400.0, 150.0),
    "key_fotos": (300.0, 260.0, 110.0),
    "luz_camara_fotos": (60.0, 60.0, 10.0),
    "forca_fitas": 3.0,
    "forca_fitas_fotos": 1.2,
    # Beat 7: distancia da logo no apice, a que o mergulho chega 'devagar',
    # e quanto a camera entra na tampa; quadros da travessia e do veu.
    # 'v_perto': velocidade (m/quadro) com que o mergulho chega a 'perto' -
    # medido: chegando a 0,047 m/quadro a 0,12 m da logo (39% da distancia
    # por quadro) o q545 era um borrao; a 0,02 a logo le ate o veu. A
    # travessia acelera dali (o veu ja cobre). 'foco_min': o foco nunca fica a
    # menos disto da camera - com o foco a 2,7 cm (e negativo dentro da
    # tampa) o DoF do EEVEE deixava o veu cobrindo so metade do quadro
    # (q548/q549 medidos); a 7,3 cm (q547) o veu cobria tudo.
    # O mergulho e em duas fases: Hermite do apice ('alto', parado) ate
    # 'meio' no primeiro terco dos quadros, e dali EXPONENCIAL ate 'perto'
    # (a mesma fracao da distancia por quadro, ~15%: o borrao de movimento
    # relativo a logo e constante e pequeno, e a chegada e naturalmente
    # devagar, ~0,02 m/quadro a 0,12 m). Uma Hermite so, medida, chegava a
    # 0,033 m/quadro e o q545 ainda borrava.
    "mergulho": {"alto": 1.8, "meio": 0.9, "perto": 0.12, "dentro": -0.02, "travessia": 3, "veu": 2,
                 "foco_min": 0.07, "f_ini": 2.8, "f_fim": 8.0},
    "logo_escala_inicial": 1.5,    # match cut: logo maior no centro em q_t
    "cartela_fracao": 0.40,        # fatia do intervalo que cada elemento gasta entrando
    # Beat 2, momento-heroi: altura da camera com o U1 no alto e quanto o
    # alvo fica acima da base dele. A revisao propos BAIXAR a camera (1,6 ->
    # 1,25) para o U1 recortar contra a transicao escura; rendido q140/q150
    # com 1,25, 1,6 e 1,85 (alvo z_alto + 0,05): a 1,25 o U1 fica INTEIRO
    # dentro da faixa rose (branco sobre rose, o que se queria evitar), a
    # 1,6 a base dele encosta na transicao, e a 1,85 - olhando mais para
    # baixo - o chao escuro sobe atras dele e o U1 recorta contra o preto
    # com o rose so em cima. A causa e geometrica: quanto mais alta a camera,
    # mais chao escuro atras do produto; baixar fazia o contrario.
    "camera_heroi": {"z": 1.85, "alvo": 0.05, "raio": 2.5},
}


# ---------------------------------------------------------------- tempo

def fator_duracao(duracao_s):
    return float(duracao_s) / DURACAO_REFERENCIA


def quadro(t, fator=1.0):
    """Segundo (na referencia de 20 s) -> quadro, com o fator de duracao."""
    return max(1, int(round(t * FPS * fator)))


def quadros_do_beat(n, fator=1.0):
    b = BEATS[n - 1]
    return quadro(b["t_ini"], fator), quadro(b["t_fim"], fator)


def q_em(n, fracao, fator=1.0):
    """Quadro na fracao 'fracao' do beat n."""
    a, b = quadros_do_beat(n, fator)
    return int(round(a + fracao * (b - a)))


def quadros_chave(fator=1.0):
    """Um quadro por beat (o meio de cada um), para a previa."""
    return [(b["n"], q_em(b["n"], 0.5, fator)) for b in BEATS]


# ---------------------------------------------------------------- utilidades

def _raiz_projeto():
    try:
        return os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    except NameError:
        # Aba Scripting do Blender sem arquivo: vale a pasta do .blend.
        return os.path.dirname(bpy.data.filepath) if bpy.data.filepath else os.getcwd()


def _asset(p, nome):
    pasta = p.get("pasta_assets") or os.path.join(_raiz_projeto(), "assets")
    return os.path.join(pasta, nome)


def _colecao_raiz(cena):
    col = bpy.data.collections.get("ANUNCIO")
    if col is None:
        col = bpy.data.collections.new("ANUNCIO")
    if col.name not in cena.collection.children:
        cena.collection.children.link(col)
    return col


def _chave(obj, quadro_, loc=None, rot=None):
    if loc is not None:
        obj.location = loc
        obj.keyframe_insert("location", frame=quadro_)
    if rot is not None:
        obj.rotation_euler = rot
        obj.keyframe_insert("rotation_euler", frame=quadro_)


def _interpolar(dono, q_ini, q_fim, interp="BEZIER", easing="EASE_IN_OUT", canais=None):
    """Interpolacao/easing so nas chaves do intervalo (nao mexe em outro beat)."""
    ad = getattr(dono, "animation_data", None)
    if ad is None or ad.action is None:
        return
    for fc in ad.action.fcurves:
        if canais is not None and fc.data_path not in canais:
            continue
        for kp in fc.keyframe_points:
            if q_ini - 0.5 <= kp.co.x <= q_fim + 0.5:
                kp.interpolation = interp
                kp.easing = easing
                if interp == "BEZIER":
                    kp.handle_left_type = "AUTO_CLAMPED"
                    kp.handle_right_type = "AUTO_CLAMPED"
        fc.update()


def _interp_nas_chaves(dono, quadros, interp):
    """Interpolacao so nas chaves dos quadros dados (todas as fcurves do dono)."""
    ad = getattr(dono, "animation_data", None)
    if ad is None or ad.action is None:
        return
    for fc in ad.action.fcurves:
        for kp in fc.keyframe_points:
            if int(round(kp.co.x)) in quadros:
                kp.interpolation = interp
        fc.update()


def _valor_em(dono, data_path, quadro_, indice=-1):
    """Valor de uma propriedade num quadro (da fcurve, se houver chave)."""
    ad = getattr(dono, "animation_data", None)
    if ad is not None and ad.action is not None:
        for fc in ad.action.fcurves:
            if fc.data_path == data_path and (indice < 0 or fc.array_index == indice):
                return fc.evaluate(quadro_)
    valor = dono.path_resolve(data_path)
    return valor[indice] if indice >= 0 else valor


def _chave_rim_especular(objs, quadro_, valor):
    """specular_factor do rim num CORTE (chave constante, com chave de espera
    no quadro anterior). Mantida pela assinatura; por dentro e o
    chavear_especular do ambiente com rampa=0. Para transicao dentro de um
    plano continuo use chavear_especular com rampa (ver _beat3): a chave
    constante no meio de um plano era o pop de luz do q165.

    Medido no quadro 1 (chao vazio): o reflexo do painel do rim no chao e uma
    barra branca com rastro ate o pe do quadro; esconder o rim ou zerar o
    especular do chao a apaga, e 0,2 e 0,05 so a escurecem um pouco - e um
    painel de 350 W espelhado em Fresnel rasante, ~100x acima do branco, e
    so o zero resolve. As outras tres luzes espelham fora do quadro.
    """
    mod_ambiente.chavear_especular(objs["ambiente"]["luzes"]["rim"], quadro_, para=valor, rampa=0)


def _chave_visivel(obj, quadro_, visivel):
    """hide_render com chave (booleano: a chave ja e constante). So o render:
    chavear hide_viewport reconstroi as relacoes do depsgraph a cada chave e
    derrubou o Blender 4.2 (segfault) ao iterar a colecao do cabo."""
    obj.hide_render = not visivel
    obj.keyframe_insert("hide_render", frame=quadro_)


def _esconder_entre(objetos, q_some, q_volta, q_primeiro=1):
    """Visivel ate q_some-1, escondido de q_some a q_volta-1, visivel de
    q_volta. Devolve os objetos que receberam chave (os ja escondidos pelo
    modulo - cortadores de boolean - ficam como estao)."""
    tocados = []
    for obj in list(objetos):
        if obj.hide_render:
            continue
        if q_some > q_primeiro:
            _chave_visivel(obj, q_primeiro, True)
        _chave_visivel(obj, q_some, False)
        if q_volta is not None:
            _chave_visivel(obj, q_volta, True)
        tocados.append(obj)
    return tocados


def _ajustar(objeto, nome, valor):
    try:
        setattr(objeto, nome, valor)
        return True
    except (AttributeError, TypeError, ValueError):
        return False


def _achatar(m):
    return [float(v) for linha in m for v in linha]


def _matriz(lista):
    v = list(lista)
    return Matrix((tuple(v[0:4]), tuple(v[4:8]), tuple(v[8:12]), tuple(v[12:16])))


# ---------------------------------------------------------------- U1 real

def _bbox_mundo(objetos):
    dg = bpy.context.evaluated_depsgraph_get()
    mn = Vector((1e9, 1e9, 1e9))
    mx = Vector((-1e9, -1e9, -1e9))
    for obj in objetos:
        if obj.hide_render or obj.type not in ("MESH", "CURVE", "FONT", "SURFACE", "META"):
            continue
        ev = obj.evaluated_get(dg)
        for canto in ev.bound_box:
            w = ev.matrix_world @ Vector(canto)
            for i in range(3):
                mn[i] = min(mn[i], w[i])
                mx[i] = max(mx[i], w[i])
    return mn, mx


def restaurar_modelo_cliente():
    """Devolve a pose ORIGINAL a todo objeto do cliente que uma rodada anterior
    cozinhou (parenteado em 'u1.raiz', girado e centralizado). Roda no inicio
    de toda rodada - inclusive quando se volta ao substituto -, para a
    rotacao e a centralizacao serem aplicadas uma vez so. Devolve a lista."""
    restaurados = []
    for obj in bpy.data.objects:
        if PROP_MATRIZ not in obj:
            continue
        nome_pai = obj.get(PROP_PAI, "")
        obj.parent = bpy.data.objects.get(nome_pai) if nome_pai else None
        if PROP_PAI_INVERSA in obj:
            obj.matrix_parent_inverse = _matriz(obj[PROP_PAI_INVERSA])
        else:
            obj.matrix_parent_inverse = Matrix.Identity(4)
        obj.matrix_world = _matriz(obj[PROP_MATRIZ])
        restaurados.append(obj)
    if restaurados:
        bpy.context.view_layer.update()
    return restaurados


def _remover_objeto(obj):
    dados = obj.data
    bpy.data.objects.remove(obj, do_unlink=True)
    if dados is not None and dados.users == 0:
        if isinstance(dados, bpy.types.Light):
            bpy.data.lights.remove(dados)
        elif isinstance(dados, bpy.types.Mesh):
            bpy.data.meshes.remove(dados)


def _limpar_u1_anterior(col_modelo):
    """Tira da cena o que a rodada anterior deixou como 'u1': o substituto (ou
    a raiz antiga e as luzes das fitas), sem tocar no modelo do cliente."""
    raiz_antiga = bpy.data.objects.get("u1.raiz")
    col_u1 = bpy.data.collections.get("u1")
    if col_u1 is not None and col_u1 is not col_modelo:
        mod_u1.limpar_colecao("u1")
        raiz_antiga = bpy.data.objects.get("u1.raiz")
    if raiz_antiga is not None:
        # A colecao do cliente se chama 'u1' (ou a raiz sobrou fora dela): o
        # que ainda desce da raiz antiga e nosso - os do cliente ja foram
        # restaurados e desparenteados por restaurar_modelo_cliente.
        for obj in [o for o in bpy.data.objects if _descende(o, raiz_antiga)]:
            if PROP_MATRIZ not in obj:
                _remover_objeto(obj)
        _remover_objeto(raiz_antiga)


def _u1_real(cena, col_pai, p):
    """Modelo do cliente por nome (objeto ou colecao). Devolve o mesmo dict
    que construir_u1, com 'real': True, ou None se o nome nao existe.

    O que faz: restaura a pose original do que uma rodada anterior cozinhou,
    limpa o 'u1' anterior, cria um Empty 'u1.raiz' NOVO, parenteia nele os
    objetos de topo do modelo mantendo a pose, aplica 'u1_rotacao_z', mede o
    bounding box avaliado e move a raiz para o modelo ficar centrado em XY com
    a base em z = 0 - a mesma pose do substituto, que e o que a caixa, o cabo
    e a camera esperam. Os pontos de tela/tomada/botao vem de params (nas
    coordenadas originais do arquivo dele, levadas pela mesma matriz) ou de
    uma heuristica pelo bounding box, documentada em _pontos_heuristicos.
    """
    nome = p["u1_nome"]
    col_modelo = bpy.data.collections.get(nome)
    if col_modelo is None and nome not in bpy.data.objects:
        return None
    restaurar_modelo_cliente()
    _limpar_u1_anterior(col_modelo)

    if col_modelo is not None:
        todos = set(o.name for o in col_modelo.all_objects)
        fontes = [o for o in col_modelo.all_objects if o.parent is None or o.parent.name not in todos]
        col = col_modelo
    else:
        fontes = [bpy.data.objects[nome]]
        col = bpy.data.collections.get("u1")
        if col is None:
            col = bpy.data.collections.new("u1")
            col_pai.children.link(col)

    raiz = bpy.data.objects.new("u1.raiz", None)
    raiz.empty_display_type = "ARROWS"
    raiz.empty_display_size = 0.2
    col.objects.link(raiz)
    # matrix_world de objeto recem-criado ou recem-movido so e valido depois
    # de uma avaliacao (medido: sem isto a rotacao do bloco de teste saiu 0).
    bpy.context.view_layer.update()
    originais = {}
    for obj in fontes:
        if PROP_MATRIZ not in obj:
            obj[PROP_MATRIZ] = _achatar(obj.matrix_world)
            obj[PROP_PAI] = obj.parent.name if obj.parent is not None else ""
            obj[PROP_PAI_INVERSA] = _achatar(obj.matrix_parent_inverse)
        originais[obj] = _matriz(obj[PROP_MATRIZ])
    # A rotacao e a centralizacao ficam COZIDAS nos filhos, e a raiz fica na
    # identidade: as chaves da coreografia escrevem a raiz em valores
    # absolutos (0, 0, z), iguais para o substituto e para o modelo real.
    rz = Matrix.Rotation(math.radians(p["u1_rotacao_z"]), 4, "Z")
    for obj in fontes:
        obj.parent = raiz
        obj.matrix_parent_inverse = Matrix.Identity(4)
        obj.matrix_world = rz @ originais[obj]
    bpy.context.view_layer.update()
    filhos = [o for o in bpy.data.objects if _descende(o, raiz)]
    mn, mx = _bbox_mundo(filhos)
    centro = (mn + mx) / 2.0
    m = Matrix.Translation((-centro.x, -centro.y, -mn.z)) @ rz
    for obj in fontes:
        obj.matrix_world = m @ originais[obj]
    bpy.context.view_layer.update()
    mn, mx = _bbox_mundo(filhos)
    dims = (mx.x - mn.x, mx.y - mn.y, mx.z - mn.z)
    print("[coreografia] modelo real '%s': %d objetos, envelope %.3f x %.3f x %.3f m" % (nome, len(filhos), *dims))

    def ponto(chave, heuristico):
        # Pontos dados nas coordenadas ORIGINAIS do arquivo do cliente vao
        # pela mesma matriz que levou a malha.
        v = p.get(chave)
        return (m @ Vector(v)) if v is not None else heuristico

    h = _pontos_heuristicos(dims)
    tela_obj = bpy.data.objects.get(p["u1_tela_objeto"]) if p["u1_tela_objeto"] else None
    botao_obj = bpy.data.objects.get(p["u1_botao_objeto"]) if p["u1_botao_objeto"] else None
    led_obj = bpy.data.objects.get(p["u1_led_objeto"]) if p["u1_led_objeto"] else None
    return {
        "raiz": raiz,
        "real": True,
        "tela": tela_obj,
        "botao": botao_obj,
        "led": led_obj,
        "cabecotes": [],
        "colecao": col,
        "dimensoes": dims,
        "dimensoes_nominais": (mod_u1.LARGURA, mod_u1.PROFUNDIDADE, mod_u1.ALTURA),
        "envelope": (mn, mx),
        "placeholders": {"boot": False, "ui": False},
        "posicao_tela": {"centro": ponto("u1_tela", h["tela"]), "normal": Vector((0, -1, 0))},
        "posicao_tomada": {"ponto": ponto("u1_tomada", h["tomada"]), "direcao": Vector((0, -1, 0)), "normal": Vector((0, 1, 0))},
        "posicao_botao": {"centro": ponto("u1_botao", h["botao"]), "normal": Vector((0, 1, 0))},
        "botao_afunda_local": Vector((0, -1, 0)),
        "materiais": {},
    }


def _descende(obj, raiz):
    o = obj.parent
    while o is not None:
        if o is raiz:
            return True
        o = o.parent
    return False


def _pontos_heuristicos(dims):
    """Onde ficam tela, tomada e botao num U1 de dimensoes 'dims', em fracao do
    envelope, medidas no substituto (que seguiu as fotos): tela no canto
    superior direito da frente (x = +0,30 L, z = 0,80 A), tomada e botao na
    coluna traseira direita (x = +0,42 L; z = 0,17 A e 0,24 A)."""
    L, P, A = dims
    return {
        "tela": Vector((0.30 * L, -P / 2.0, 0.80 * A)),
        "tomada": Vector((0.42 * L, P / 2.0, 0.17 * A)),
        "botao": Vector((0.42 * L, P / 2.0, 0.24 * A)),
    }


def _purgar_acoes_orfas():
    """Actions sem usuario que os limpar_colecao dos modulos deixam para tras
    (uma por objeto animado, a cada rodada: medido +37 a +43 por rodada).
    Nao seriam salvas de qualquer jeito; sem isto cada rodada na cena do
    cliente acumula lixo no .blend aberto. Fake user e respeitado."""
    n = 0
    for acao in list(bpy.data.actions):
        if acao.users == 0 and not acao.use_fake_user:
            bpy.data.actions.remove(acao)
            n += 1
    return n


def _colecao_u1_e_de_fora():
    """True se existe uma colecao 'u1' com objetos que nao sao do substituto
    (o modelo do cliente com esse nome): limpar_colecao a apagaria."""
    col = bpy.data.collections.get("u1")
    if col is None:
        return False
    return any(not o.name.startswith("u1.") for o in col.all_objects)


# ---------------------------------------------------------------- construir

def construir_tudo(params=None):
    """Colecao ANUNCIO com ambiente, caixa, U1 (substituto ou real), cabo,
    cartela e camera. Devolve o dict que coreografar e configurar_render usam."""
    p = dict(PARAMS_PADRAO)
    if params:
        p.update(params)
    cena = bpy.context.scene
    col = _colecao_raiz(cena)
    _purgar_acoes_orfas()

    # Rim com especular 0,5 (padrao do modulo: 0,6) nos beats 3-5; nos
    # planos largos (beats 1, 2, 6, 7) a coreografia leva o specular_factor
    # do rim a 0 (chavear_especular, ver _beat1/_beat3/_beat6): o reflexo
    # dele no chao cai no eixo da camera e saia como uma barra branca no
    # horizonte com um rastro ate o pe do quadro - no quadro 1, antes de a
    # caixa emergir, era a imagem inteira. Medido: 0,4 e 0,2 so encolhem a
    # barra (e Fresnel rasante, nao intensidade). As outras tres luzes
    # espelham fora do quadro; o recorte da aresta vem do difuso, que fica.
    pamb = {"luzes": {"rim": {"especular": 0.5}}}
    for k, v in p["ambiente"].items():
        if k == "luzes":
            for luz, cfg in v.items():
                pamb["luzes"].setdefault(luz, {}).update(cfg)
        else:
            pamb[k] = v
    amb = mod_ambiente.construir_ambiente(cena, col, pamb)

    u1 = None
    if p["u1_nome"]:
        u1 = _u1_real(cena, col, p)
        if u1 is None:
            print("[coreografia] AVISO: '%s' nao existe em bpy.data; usando o U1 substituto" % p["u1_nome"])
    if u1 is None:
        # Rodada anterior com modelo real: devolve a pose original dele antes
        # de o substituto limpar a colecao 'u1' (a raiz antiga esta la).
        restaurar_modelo_cliente()
        if _colecao_u1_e_de_fora():
            raise RuntimeError("[coreografia] existe uma colecao 'u1' que nao e a do substituto; "
                               "para usa-la ponha o nome em U1_NOME - com U1_NOME vazio ela seria apagada")
        pu1 = {"imagem_boot": _asset(p, "tela_boot.png"), "imagem_ui": _asset(p, "tela_ui.png")}
        pu1.update(p["u1"])
        u1 = mod_u1.construir_u1(cena, col, pu1)
        u1["real"] = False

    pcaixa = {"cor": p["cor_caixa"], "logo": _asset(p, "logo_engineprint.png"), "u1": tuple(u1["dimensoes"])}
    pcaixa.update(p["caixa"])
    caixa = mod_caixa.construir_caixa(cena, col, pcaixa)
    if caixa.get("logo_provisoria"):
        print("[coreografia] AVISO: logo PROVISORIA na caixa (o PNG nao foi encontrado)")
    ix, iy, iz = caixa["interior"]
    if u1["dimensoes"][0] > ix or u1["dimensoes"][1] > iy or u1["dimensoes"][2] > iz:
        print("[coreografia] AVISO: o U1 (%.3f x %.3f x %.3f) nao cabe no interior da caixa (%.3f x %.3f x %.3f)"
              % (tuple(u1["dimensoes"]) + (ix, iy, iz)))

    # O U1 nasce dentro da caixa, apoiado no fundo (uma parede acima do chao).
    parede = mod_caixa.PARAMS_PADRAO["parede"]
    u1["raiz"].location = Vector((0.0, 0.0, parede))
    u1["z_na_caixa"] = parede

    # O cabo e construido encaixado na tomada com o U1 no CHAO (beat 3): os
    # pontos do dict foram medidos com a raiz na identidade, que e essa pose.
    tomada = u1["posicao_tomada"]
    pcabo = {"ponto_tomada": tuple(tomada["ponto"]), "direcao_entrada": tuple(tomada["direcao"]),
             "z_chao": 0.0, "penetracao": -mod_cabo.BICO[4]}
    cabo = mod_cabo.construir_cabo(cena, col, pcabo)

    # Forcas de emissao: as do modulo (2,4 / 2,0, medidas no render sob o
    # AgX); bloco mais compacto (logo 0,24, entrelinha 3 menor) para a linha
    # 4 subir acima da faixa de legendas do Reels.
    pcart = {"logo": _asset(p, "logo_engineprint.png"),
             "largura_logo": 0.24, "entrelinhas": (1.30, 1.45, 1.55)}
    pcart.update(p["cartela"])
    cartela = mod_cartela.construir_cartela(cena, col, pcart)

    cam, alvo = mod_ambiente.criar_camera(cena, col, params=p["camera"])
    # A travessia do beat 7 leva a camera a 2 cm dentro da tampa: o clip
    # padrao (0,05) cortaria a logo antes do veu.
    cam.data.clip_start = 0.01
    col_cam = bpy.data.collections.get(mod_ambiente.NOME_CAMERA)
    # Rig da camera: Empty na origem; a camera e filha e as chaves sao
    # (azimute no rig, raio e altura na camera) - ver cabecalho.
    rig = bpy.data.objects.new("camera.orbita", None)
    rig.empty_display_type = "PLAIN_AXES"
    rig.empty_display_size = 0.3
    col_cam.objects.link(rig)
    cam.parent = rig
    cam.matrix_parent_inverse = Matrix.Identity(4)
    foco = bpy.data.objects.new("camera.foco", None)
    foco.empty_display_type = "CUBE"
    foco.empty_display_size = 0.04
    col_cam.objects.link(foco)
    cam.data.dof.focus_object = foco
    _purgar_acoes_orfas()

    return {
        "cena": cena,
        "colecao": col,
        "params": p,
        "fator": fator_duracao(p["duracao_s"]),
        "ambiente": amb,
        "caixa": caixa,
        "u1": u1,
        "cabo": cabo,
        "cartela": cartela,
        "camera": cam,
        "alvo": alvo,
        "foco": foco,
        "rig_camera": rig,
        # Onde o U1 esta nos beats 3-5 (origem, ou -Y com caixa_some=False).
        "centro_u1": Vector((0.0, 0.0, 0.0)) if p["caixa_some"] else Vector((0.0, -p["deslocamento_u1"], 0.0)),
        "_chaves_camera": {},
        "_lentes_rampa": set(),
    }


# ---------------------------------------------------------------- camera

def _cil(pos):
    """XYZ -> (azimute em graus, raio, z) no rig da camera (origem)."""
    x, y, z = pos
    return math.degrees(math.atan2(y, x)), math.hypot(x, y), z


def _chave_camera(objs, q, az, raio, z, alvo, foco=None, lente=None,
                  interp="BEZIER", easing="EASE_IN_OUT"):
    """Uma chave de camera: azimute (graus) no rig, raio e altura na camera,
    alvo do Track To, foco (= alvo se None) e lente. A interpolacao registrada
    vale para o trecho que COMECA nesta chave (CONSTANT = segura ate o corte)."""
    rig, cam = objs["rig_camera"], objs["camera"]
    rig.rotation_euler = (0.0, 0.0, math.radians(az))
    rig.keyframe_insert("rotation_euler", index=2, frame=q)
    cam.location = (raio, 0.0, z)
    cam.keyframe_insert("location", frame=q)
    objs["alvo"].location = Vector(alvo)
    objs["alvo"].keyframe_insert("location", frame=q)
    objs["foco"].location = Vector(alvo if foco is None else foco)
    objs["foco"].keyframe_insert("location", frame=q)
    if lente is not None:
        cam.data.lens = lente
        cam.data.keyframe_insert("lens", frame=q)
    objs["_chaves_camera"][q] = (interp, easing)


def _chave_centro(objs, q, centro, interp="BEZIER"):
    """Posicao dos dois rigs (camera e luzes): so com caixa_some=False, quando
    o U1 nao esta na origem nos beats 3-5. Com o padrao nao ha chave e os
    rigs ficam na origem, como sempre."""
    if objs["params"]["caixa_some"]:
        return
    for rig in (objs["rig_camera"], objs["ambiente"]["rig"]):
        rig.location = Vector(centro)
        rig.keyframe_insert("location", frame=q)
        _interp_nas_chaves(rig, {q}, interp)


def _chave_f(objs, q, f):
    """Abertura (f-stop) da camera com chave; interpolacao pela do registro."""
    dof = objs["camera"].data.dof
    dof.aperture_fstop = f
    objs["camera"].data.keyframe_insert("dof.aperture_fstop", frame=q)


def _aplicar_interpolacao_camera(objs):
    registro = objs["_chaves_camera"]
    donos = (objs["rig_camera"], objs["camera"], objs["alvo"], objs["foco"], objs["camera"].data)
    for dono in donos:
        ad = dono.animation_data
        if ad is None or ad.action is None:
            continue
        for fc in ad.action.fcurves:
            for kp in fc.keyframe_points:
                interp, easing = registro.get(int(round(kp.co.x)), ("BEZIER", "EASE_IN_OUT"))
                kp.interpolation = interp
                kp.easing = easing
                if interp == "BEZIER":
                    kp.handle_left_type = "AUTO_CLAMPED"
                    kp.handle_right_type = "AUTO_CLAMPED"
            fc.update()
    # A lente so tem chave nos cortes: entre eles precisa segurar, nao
    # rampar - salvo o push-in de cada foto (chaves em _lentes_rampa, que
    # rampam LINEAR ate a chave seguinte, CONSTANT).
    ad = objs["camera"].data.animation_data
    if ad and ad.action:
        for fc in ad.action.fcurves:
            if fc.data_path == "lens":
                for kp in fc.keyframe_points:
                    kp.interpolation = "LINEAR" if int(round(kp.co.x)) in objs["_lentes_rampa"] else "CONSTANT"
                fc.update()


def _enquadrar(pos_cam, sujeito, lente, fx=0.68, fy=0.74):
    """Alvo que poe 'sujeito' na fracao (fx, fy) do quadro (x para a direita,
    y para baixo, 0,5 = centro): o alvo e o centro do quadro, deslocado no
    plano do sujeito. 9:16 com o sensor de 36 mm no lado maior."""
    pos_cam = Vector(pos_cam)
    sujeito = Vector(sujeito)
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


def _sujeitos_fotos(objs):
    """Os tres closes do beat 5: cabecotes, porta/puxador, mesa. Com o
    substituto sao os objetos; com o modelo real, fracoes do envelope."""
    u1 = objs["u1"]
    if u1.get("cabecotes") and u1.get("puxador") is not None and u1.get("mesa") is not None:
        cabs = u1["cabecotes"]
        meio = (cabs[1].matrix_world.translation + cabs[2].matrix_world.translation) / 2.0
        return {
            "cabecotes": meio + Vector((0.0, 0.0, 0.02)),
            "porta": u1["puxador"].matrix_world.translation.copy(),
            "mesa": u1["mesa"].matrix_world.translation.copy(),
        }
    L, P, A = u1["dimensoes"]
    c = objs["centro_u1"]
    return {
        "cabecotes": c + Vector((0.0, 0.25 * P, 0.80 * A)),
        "porta": c + Vector((0.35 * L, -P / 2.0, 0.35 * A)),
        "mesa": c + Vector((0.0, 0.0, 0.21 * A)),
    }


# ---------------------------------------------------------------- beats

def _beat1(objs, fator):
    """Caixa sobe do chao girando 2 voltas, rapido no inicio e assentando.
    O U1 (dentro) e as espumas vao junto, uma chave por quadro: a espuma esta
    fora do eixo, e so a chave por quadro faz o giro dela ser o mesmo da
    caixa sem parentear (a caixa afunda no beat 2 e a espuma nao pode ir)."""
    q_ini, q_fim = quadros_do_beat(1, fator)
    caixa, u1 = objs["caixa"], objs["u1"]
    n = float(q_fim - q_ini)
    voltas = 2.0                                   # inteiras: acaba com a frente em -Y
    # Tampa RENTE ao chao no primeiro quadro (a revisao mediu 0,27 s de chao
    # vazio com +0,25): o topo da tampa esta em z = 0 no quadro 1.
    profundidade = caixa["topo_tampa_z"]
    objs["profundidade_caixa"] = profundidade
    corpo, tampa, raiz = caixa["corpo"], caixa["tampa"], u1["raiz"]
    z_tampa = tampa.location.z
    z_u1 = raiz.location.z
    repousos = [(esp, Vector(esp["caixa_repouso"]), tuple(esp["caixa_rot_repouso"])) for esp in caixa["espumas"]]
    for f in range(q_ini, q_fim + 1):
        u = (f - q_ini) / n
        # Giro decai mais devagar que a subida: a caixa ja assentou em altura
        # e ainda gira um pouco - le como "assentar", nao como parar.
        s_rot = 1.0 - (1.0 - u) ** 2.2
        s_z = 1.0 - (1.0 - u) ** 2.8
        ang = -voltas * math.tau * (1.0 - s_rot)
        dz = -profundidade * (1.0 - s_z)
        _chave(corpo, f, (0.0, 0.0, dz), (0.0, 0.0, ang))
        _chave(tampa, f, (0.0, 0.0, z_tampa + dz), (0.0, 0.0, ang))
        _chave(raiz, f, (0.0, 0.0, z_u1 + dz), (0.0, 0.0, ang))
        R = Matrix.Rotation(ang, 3, "Z")
        for esp, p0, r0 in repousos:
            p = R @ p0
            _chave(esp, f, (p.x, p.y, p.z + dz), (r0[0], r0[1], r0[2] + ang))
    for obj in [corpo, tampa, raiz] + caixa["espumas"]:
        _interpolar(obj, q_ini, q_fim)

    # Camera frontal, um pouco alta, afastando de leve; o alvo acompanha o
    # centro da caixa que sobe. Acaba em -80 graus (3/4 leve, nao frente
    # morta) e raio 2,1: caixa a ~38% da altura do 9:16.
    # A lente PRECISA de chave aqui: a fcurve extrapola a primeira chave para
    # tras, e sem esta os beats 1-4 saiam com os 60 mm da primeira foto do
    # beat 5 (medido: caixa 1,7x maior que o calculado).
    _chave_camera(objs, q_ini, -90.0, 2.2, 1.0, (0.0, 0.0, 0.30), lente=35.0)
    _chave_camera(objs, q_fim, -80.0, 2.1, 1.1, (0.0, 0.0, 0.42))
    _chave_rim_especular(objs, q_ini, 0.0)


def _beat2(objs, fator):
    """Tampa sai, espuma explode, U1 sobe, caixa afunda no chao (ou o U1
    desliza para a frente dela), U1 pousa."""
    r = ROTEIRO[2]
    q_ini, q_fim = quadros_do_beat(2, fator)
    q = lambda fr: q_em(2, fr, fator)  # noqa: E731
    caixa, u1, amb, p = objs["caixa"], objs["u1"], objs["ambiente"], objs["params"]

    mod_caixa.animar_tampa(caixa, q(r["tampa"][0]), q(r["tampa"][1]), abrir=True, lado=1.0)
    # Fora do quadro a tampa ficaria flutuando a 1,6 m do lado: esconder ate
    # o beat 6 - a chave de volta e gravada la.
    objs["_q_tampa_some"] = q(r["tampa"][1]) + 1

    mod_caixa.animar_espuma(caixa, q(r["espuma"][0]), q(r["espuma"][1]))

    raiz = u1["raiz"]
    z_alto = caixa["exterior_corpo"][2] + p["folga_u1"]
    objs["z_alto_u1"] = z_alto
    z0 = u1["z_na_caixa"]
    centro = objs["centro_u1"]
    _chave(raiz, q(r["u1_sobe"][0]), (0.0, 0.0, z0))
    _chave(raiz, q(r["u1_sobe"][1]), (0.0, 0.0, z_alto))
    if p["caixa_some"]:
        _chave(raiz, q(r["u1_desce"][0]), (0.0, 0.0, z_alto))
        _chave(raiz, q(r["u1_desce"][1]), (0.0, 0.0, 0.0))
    else:
        # Desliza no ar para -Y e pousa na frente da caixa, que fica parada.
        _chave(raiz, q(r["u1_desliza"][1]), (0.0, centro.y, 0.12))
        _chave(raiz, q_fim, (0.0, centro.y, 0.0))
    _interpolar(raiz, q(r["u1_sobe"][0]), q_fim)

    corpo = caixa["corpo"]
    if p["caixa_some"]:
        _chave(corpo, q(r["caixa_desce"][0]), (0.0, 0.0, 0.0))
        _chave(corpo, q(r["caixa_desce"][1]), (0.0, 0.0, -objs["profundidade_caixa"]))
        _interpolar(corpo, q(r["caixa_desce"][0]), q(r["caixa_desce"][1]))

    # Camera acompanha o U1 subindo (alvo sobe com ele) e comeca a derivar
    # para +X, de onde a orbita do beat 3 parte. Alvo em z_alto + 0,05 e
    # camera a 1,85 (medido, ver PARAMS_PADRAO['camera_heroi']): o U1
    # recorta contra o chao escuro em vez de branco sobre rose.
    ch = p["camera_heroi"]
    _chave_camera(objs, q(r["u1_sobe"][1]), -84.0, ch["raio"], ch["z"], (0.0, 0.0, z_alto + ch["alvo"]))
    _chave_centro(objs, q(r["u1_sobe"][1]), (0.0, 0.0, 0.0))
    _chave_camera(objs, q_fim, -75.0, 2.3, 1.1, centro + Vector((0.0, 0.0, 0.37)))
    _chave_centro(objs, q_fim, centro)
    mod_ambiente.animar_rig(amb, q_ini, q_fim, 0.0, 15.0)
    # Rim a 0,3 so no trecho em que o chao esta coberto (U1 no alto sobre a
    # caixa): recorte da silhueta branca sem a poca do reflexo aparecer.
    rim = amb["luzes"]["rim"]
    mod_ambiente.chavear_especular(rim, q(r["rim"][0]), para=0.3, rampa=12)
    mod_ambiente.chavear_especular(rim, q(r["rim"][1]) - 12, q(r["rim"][1]), para=0.0)


def _beat3(objs, fator):
    """Orbita ate a traseira; cabo entra e encaixa; LIGAR como evento de luz
    com um push-in leve."""
    r = ROTEIRO[3]
    q_ini, q_fim = quadros_do_beat(3, fator)
    q = lambda fr: q_em(3, fr, fator)  # noqa: E731
    u1, cabo, amb, cena, p = objs["u1"], objs["cabo"], objs["ambiente"], objs["cena"], objs["params"]
    centro = objs["centro_u1"]

    q_orb = q(r["orbita"][1])
    # O rim so sobe quando a camera passou de azimute ~0 (o produto cobre a
    # poca do reflexo no chao), em rampa Bezier: a chave constante em q_ini
    # era o pop de luz medido pela revisao (q160 -> q165).
    q_rim = int(round(q_ini + r["rim"] * (q_orb - q_ini)))
    mod_ambiente.chavear_especular(amb["luzes"]["rim"], q_rim, para=0.5, rampa=12)
    # Azimute MONOTONO (105 -> 110 -> 120) e raio sem inversao forte (1,7 ->
    # 1,25 -> 1,15): a camera nunca para nem recua no meio do plano.
    _chave_camera(objs, q_orb, 105.0, 1.7, 0.60, centro + Vector((0.15, 0.15, 0.30)))
    # (medido com 112/115: 0,004 m/quadro no ligar - quase parado 25 quadros;
    # com 110/120 o push-in e o giro somam ~0,008 m/quadro, sempre vivo).
    _chave_camera(objs, q(r["push_in"][0]), 110.0, 1.25, 0.45, centro + Vector((0.20, 0.20, 0.20)))
    _chave_camera(objs, q_fim, 120.0, 1.15, 0.42, centro + Vector((0.20, 0.20, 0.18)))
    # Rig de luz = azimute da camera + offset (90: rim atras do produto).
    off = p["offset_rig_orbita"]
    mod_ambiente.animar_rig(amb, q_ini, q_orb, None, 105.0, azimutes=True, offset=off)
    mod_ambiente.animar_rig(amb, q_orb, q_fim, 105.0, 115.0, azimutes=True, offset=off)

    # Tomada no mundo com o U1 ja no chao (a raiz esta na identidade aqui).
    cena.frame_set(q_ini)
    bpy.context.view_layer.update()
    ponto = mod_u1.ponto_no_mundo(u1, "posicao_tomada", "ponto")
    direcao = mod_u1.ponto_no_mundo(u1, "posicao_tomada", "direcao")
    normal = -direcao
    lateral = normal.cross(Vector((0, 0, 1))).normalized() * cabo.get("lado", 1.0)
    # Origem fora do quadro (a 16 graus de meio-campo horizontal, 1,3 m atras
    # e 0,9 m para o lado esta fora em qualquer ponto da orbita), a 0,45 m
    # do chao: o plugue cruza o quadro contra o corpo branco e a faixa rose,
    # nao preto sobre o chao preto (revisao, q205).
    origem = ponto + normal * 1.3 + lateral * 0.9
    origem.z = p["origem_cabo_z"]
    q_cabo = (q(r["cabo"][0]), q(r["cabo"][1]))
    mod_cabo.animar_conexao(cabo, ponto, direcao, q_cabo[0], q_cabo[1],
                            origem=origem, z_chao=0.0, penetracao=-mod_cabo.BICO[4],
                            altura_arco=p["arco_cabo"])
    objs["_q_cabo"] = q_cabo

    # Ligar e evento de luz: botao afunda, fitas e area lights da camara
    # acendem, tela em standby (o modulo cria uma luz no modelo real sem
    # fitas). As fitas a 3,0 ficam abaixo do limiar de bloom nas fotos.
    mod_u1.animar_ligar(u1, q(r["botao"][0]), q(r["botao"][1]), forca_fitas=p["forca_fitas"])


def _beat4(objs, fator):
    """Orbita de volta pela frente e dolly ate a tela; tela PARADA no close
    enquanto o boot roda e a UI entra."""
    r = ROTEIRO[4]
    q_ini, q_fim = quadros_do_beat(4, fator)
    q = lambda fr: q_em(4, fr, fator)  # noqa: E731
    u1, amb, cena, p = objs["u1"], objs["ambiente"], objs["cena"], objs["params"]
    centro = objs["centro_u1"]

    cena.frame_set(q_ini)
    bpy.context.view_layer.update()
    tela = mod_u1.ponto_no_mundo(u1, "posicao_tela", "centro")
    normal = mod_u1.ponto_no_mundo(u1, "posicao_tela", "normal")
    # Tela de 0,104 m a ~69% da largura do quadro: 0,26 m a 35 mm.
    pos_fim = tela + normal * 0.26 + Vector((0.02, 0.0, 0.015))
    az_fim, r_fim, z_fim = _cil(pos_fim - centro)
    az_fim += 360.0          # continua girando no mesmo sentido (120 -> 292)
    q_orb = q(r["orbita"][1])
    q_dolly = q(r["dolly"])
    # Meio da orbita com raio 2,1 e alvo no corpo: o U1 inteiro no quadro
    # (a revisao mediu o U1 cortado na borda esquerda em q315).
    _chave_camera(objs, q(0.30), 180.0, 2.1, 0.80, centro + Vector((0.0, 0.0, 0.42)))
    # Em q_orb o alvo ainda e o corpo (era a tela: com o centro do quadro no
    # canto direito da frente, o lado esquerdo do U1 saia cortado em q315,
    # medido no render); o dolly leva o alvo ate a tela.
    _chave_camera(objs, q_orb, 250.0, 1.45, 0.72, centro + Vector((0.0, 0.0, 0.42)))
    # Fim do dolly em q(0,78) e a MESMA chave em q_fim-1 (CONSTANT): duas
    # chaves iguais zeram o handle e seguram a tela parada 19 quadros; o
    # corte da primeira foto e em q_fim, e duas chaves no mesmo quadro fariam
    # a foto sobrescrever o close (medido: q350 apontando aos cabecotes).
    _chave_camera(objs, q_dolly, az_fim, r_fim, z_fim, tela)
    _chave_camera(objs, q_fim - 1, az_fim, r_fim, z_fim, tela, interp="CONSTANT")
    off = p["offset_rig_orbita"]
    mod_ambiente.animar_rig(amb, q_ini, q_orb, None, 250.0, azimutes=True, offset=off)
    mod_ambiente.animar_rig(amb, q_orb, q_dolly, 250.0, az_fim, azimutes=True, offset=off)

    if u1.get("tela") is not None:
        mod_u1.animar_tela(u1, q(r["boot"]), q(r["ui"]), q_fim)
    else:
        print("[coreografia] modelo real sem 'u1_tela_objeto': tela nao animada")


def _beat5(objs, fator):
    """Tres fotos: cortes secos com flash, closes ancorados no canto inferior
    direito, cada um com um push-in lento e a luz mudando de angulo."""
    q_ini, q_fim = quadros_do_beat(5, fator)
    cena, amb, cam, p = objs["cena"], objs["ambiente"], objs["camera"], objs["params"]
    centro = objs["centro_u1"]
    cena.frame_set(q_ini)
    bpy.context.view_layer.update()
    s = _sujeitos_fotos(objs)
    cortes = [q_em(5, fr, fator) for fr in ROTEIRO[5]["fotos"]] + [q_fim]
    e_rims, e_keys, e_cam = p["rim_fotos"], p["key_fotos"], p["luz_camara_fotos"]

    # (sujeito, camera no inicio, lente, enquadramento (fx, fy), rig relativo, energia key, energia rim)
    # Foto C de FORA da pegada: acima e a frente-direita do aro, olhando a
    # mesa pelo topo aberto; mesa e hastes na diagonal, mesa embaixo a direita.
    fotos = [
        (s["cabecotes"], s["cabecotes"] + Vector((-0.22, -0.38, 0.30)), 60.0, (0.68, 0.74), +45.0, e_keys[0], e_rims[0]),
        (s["porta"], s["porta"] + Vector((0.38, -0.52, 0.18)), 50.0, (0.68, 0.74), -50.0, e_keys[1], e_rims[1]),
        (s["mesa"], s["mesa"] + Vector((0.30, -0.30, 1.20)), 50.0, (0.70, 0.74), +70.0, e_keys[2], e_rims[2]),
    ]
    # Luz propria do U1 nas fotos: fitas mais fracas (chave de espera em
    # q_ini-1 e corte em q_ini) e area lights da camara por foto (cortes).
    u1 = objs["u1"]
    s_led = _socket_emissao(u1.get("materiais", {}).get("led"))
    if s_led is not None:
        nt = u1["materiais"]["led"].node_tree
        s_led.default_value = _valor_em(nt, s_led.path_from_id("default_value"), q_ini - 1)
        s_led.keyframe_insert("default_value", frame=q_ini - 1)
        s_led.default_value = p["forca_fitas_fotos"]
        s_led.keyframe_insert("default_value", frame=q_ini)
        _interp_nas_chaves(nt, {q_ini - 1, q_ini}, "CONSTANT")
    for luz in list(u1.get("luzes_led") or []):
        dados = luz.data
        chaves_luz = [(q_ini - 1, _valor_em(dados, "energy", q_ini - 1))] + list(zip(cortes[:-1], e_cam))
        for q_, e in chaves_luz:
            dados.energy = e
            dados.keyframe_insert("energy", frame=q_)
        _interp_nas_chaves(dados, {q_ for q_, _ in chaves_luz}, "CONSTANT")
    luzes = amb["luzes"]
    padrao_key = luzes["key"].data.energy
    padrao_rim = luzes["rim"].data.energy
    # Antes do primeiro corte as energias precisam de chave com o valor
    # padrao, senao a primeira chave extrapola para tras e muda os beats 1-4.
    for luz, val in ((luzes["key"], padrao_key), (luzes["rim"], padrao_rim)):
        luz.data.energy = val
        luz.data.keyframe_insert("energy", frame=cortes[0] - 1)
    objs["_chaves_rig_luz"] = {}
    for i, (sujeito, pos, lente, (fx, fy), rig_rel, e_key, e_rim) in enumerate(fotos):
        q_a, q_b = cortes[i], cortes[i + 1] - 1
        # Push-in: 0,06 m na direcao do sujeito ao longo da foto, e a lente
        # de 50 a 52 mm (LINEAR) - a foto quase parada lia como still.
        direcao = (Vector(sujeito) - Vector(pos)).normalized()
        for q_, p_, lente_ in ((q_a, pos, lente), (q_b, pos + direcao * 0.06, lente + 2.0)):
            az, raio, z = _cil(p_ - centro)
            alvo = _enquadrar(p_, sujeito, lente_, fx, fy)
            _chave_camera(objs, q_, az, raio, z, alvo, foco=sujeito, lente=lente_,
                          interp="LINEAR" if q_ == q_a else "CONSTANT")
        objs["_lentes_rampa"].add(q_a)
        mod_ambiente.animar_flash(amb, cam, q_a)
        # Luz da foto: rig girado em relacao a camera (key mais lateral) e
        # rim mais forte; tudo em chave constante, e um corte.
        az_cam = _cil(pos - centro)[0]
        objs["_chaves_rig_luz"][q_a] = az_cam + 90.0 + rig_rel
        for luz, val in ((luzes["key"], e_key), (luzes["rim"], e_rim)):
            luz.data.energy = val
            luz.data.keyframe_insert("energy", frame=q_a)
    # De volta ao padrao no corte do beat 6.
    for luz, val in ((luzes["key"], padrao_key), (luzes["rim"], padrao_rim)):
        luz.data.energy = val
        luz.data.keyframe_insert("energy", frame=q_fim)
        for fc in luz.data.animation_data.action.fcurves:
            if fc.data_path == "energy":
                for kp in fc.keyframe_points:
                    kp.interpolation = "CONSTANT"
                fc.update()
    objs["_q_rig_luz_padrao"] = q_fim


def _socket_emissao(mat):
    if mat is None or not mat.use_nodes:
        return None
    try:
        return mod_u1._socket_forca_emissao(mat.node_tree)
    except (AttributeError, RuntimeError):
        return None


def _segurar_e_zerar(dono, socket, quadro_):
    """Chave de espera (valor atual, CONSTANT) em quadro-1 e 0 em quadro: um
    corte, sem a rampa Bezier que a chave sozinha faria desde o beat 4."""
    caminho = socket.path_from_id("default_value")
    atual = _valor_em(dono, caminho, quadro_ - 1)
    socket.default_value = atual
    socket.keyframe_insert("default_value", frame=quadro_ - 1)
    socket.default_value = 0.0
    socket.keyframe_insert("default_value", frame=quadro_)
    _interp_nas_chaves(dono, {quadro_ - 1, quadro_}, "CONSTANT")


def _desligar_u1(objs, quadro_):
    """No corte do beat 6 a maquina volta desligada para a caixa: tela, fitas,
    janela do botao e as area lights da camara apagam de uma vez."""
    u1 = objs["u1"]
    mats = u1.get("materiais", {})
    mat_tela = mats.get("tela")
    if mat_tela is None and u1.get("tela") is not None:
        mat_tela = u1["tela"].active_material
    if mat_tela is not None and mat_tela.use_nodes:
        nt = mat_tela.node_tree
        if nt.nodes.get("ligada") is not None:
            for nome in ("ligada", "standby"):
                no = nt.nodes.get(nome)
                if no is not None:
                    _segurar_e_zerar(nt, no.outputs[0], quadro_)
        else:
            s = _socket_emissao(mat_tela)
            if s is not None:
                _segurar_e_zerar(nt, s, quadro_)
    for mat in (mats.get("led"), mats.get("botao")):
        s = _socket_emissao(mat)
        if s is not None:
            _segurar_e_zerar(mat.node_tree, s, quadro_)
    for luz in list(u1.get("luzes_led") or []):
        dados = luz.data
        dados.energy = _valor_em(dados, "energy", quadro_ - 1)
        dados.keyframe_insert("energy", frame=quadro_ - 1)
        dados.energy = 0.0
        dados.keyframe_insert("energy", frame=quadro_)
        _interp_nas_chaves(dados, {quadro_ - 1, quadro_}, "CONSTANT")
        _chave_visivel(luz, quadro_, False)


def _beat6(objs, fator):
    """Corte ao plano geral: U1 sobe, caixa volta pelo chao, U1 entra, espuma
    volta, tampa fecha; camera sobe."""
    r = ROTEIRO[6]
    q_ini, q_fim = quadros_do_beat(6, fator)
    q = lambda fr: q_em(6, fr, fator)  # noqa: E731
    caixa, u1, amb, cabo, p = objs["caixa"], objs["u1"], objs["ambiente"], objs["cabo"], objs["params"]
    centro = objs["centro_u1"]
    z_alto = objs["z_alto_u1"]

    if p["caixa_some"]:
        # 9:16 aproveitado: raio 2,2/2,1 e altura 1,0/1,7 (era 3,0/2,8 e
        # 1,3/2,3, produto a 28% da altura). No pico da subida do U1 o alvo
        # sobe a 0,78 m: com o alvo a 0,45 o topo dele (1,68 m) saia do quadro.
        _chave_camera(objs, q_ini, -80.0, 2.2, 1.0, (0.0, 0.0, 0.45), lente=35.0)
        _chave_camera(objs, q(r["u1_sobe"][1]), -81.0, 2.3, 1.15, (0.0, 0.0, 0.78))
        _chave_camera(objs, q_fim, -84.0, 2.1, 1.7, (0.0, 0.0, 0.60))
    else:
        # De lado: o U1 esta 2,1 m na frente da caixa e de frente ele
        # ficaria colado na camera.
        _chave_camera(objs, q_ini, -30.0, 3.3, 1.3, (0.0, -1.0, 0.45), lente=35.0)
        _chave_camera(objs, q_fim, -45.0, 2.8, 1.9, (0.0, -0.2, 0.60))
    # Os rigs voltam a origem no corte (so tem efeito com caixa_some=False).
    _chave_centro(objs, q_ini - 1, centro, interp="CONSTANT")
    _chave_centro(objs, q_ini, (0.0, 0.0, 0.0), interp="CONSTANT")
    mod_ambiente.animar_rig(amb, q_ini, q_fim, 10.0, 6.0)
    _chave_rim_especular(objs, q_ini, 0.0)

    raiz = u1["raiz"]
    if p["caixa_some"]:
        _chave(raiz, q(r["u1_sobe"][0]), (0.0, 0.0, 0.0))
        _chave(raiz, q(r["u1_sobe"][1]), (0.0, 0.0, z_alto))
        _chave(raiz, q(r["u1_desce"][0]), (0.0, 0.0, z_alto))
    else:
        _chave(raiz, q(r["u1_sobe"][0]), (0.0, centro.y, 0.0))
        _chave(raiz, q(r["u1_sobe"][1]), (0.0, centro.y * 0.5, z_alto))
        _chave(raiz, q(r["u1_desce"][0]), (0.0, 0.0, z_alto))
    _chave(raiz, q(r["u1_desce"][1]), (0.0, 0.0, u1["z_na_caixa"]))
    _interpolar(raiz, q_ini, q_fim)

    corpo = caixa["corpo"]
    if p["caixa_some"]:
        _chave(corpo, q(r["caixa_sobe"][0]), (0.0, 0.0, -objs["profundidade_caixa"]))
        _chave(corpo, q(r["caixa_sobe"][1]), (0.0, 0.0, 0.0))
        _interpolar(corpo, q(r["caixa_sobe"][0]), q(r["caixa_sobe"][1]))

    mod_caixa.animar_espuma_voltar(caixa, q(r["espuma"][0]), q(r["espuma"][1]))
    mod_caixa.animar_tampa(caixa, q(r["tampa"][0]), q(r["tampa"][1]), abrir=False, lado=1.0)
    _esconder_entre([caixa["tampa"]], objs["_q_tampa_some"], q(r["tampa"][0]) - 1)

    # Cabo: visivel so do inicio do voo ate o corte do beat 6 (o modulo nao
    # acompanha o U1 subindo, e o plugue no chao apareceria nos beats 1-2).
    # A invisibilidade do sumico depende da camera do beat 6 olhar de -Y
    # (o cabo esta atras, em +Y).
    visiveis = _esconder_entre(list(cabo["colecao"].all_objects), 1, objs["_q_cabo"][0])
    for obj in visiveis:
        _chave_visivel(obj, q_ini, False)
    # A maquina volta para a caixa desligada (tela, fitas, luzes).
    _desligar_u1(objs, q_ini)


def _perfil_mergulho(u, s):
    """Hermite em [0, 1]: 1 -> 0, parte parado (derivada 0) e chega com
    derivada -s (nao para na logo: segue para a travessia)."""
    return (2.0 * u ** 3 - 3.0 * u ** 2 + 1.0) - s * (u ** 3 - u ** 2)


def _veu_preto(objs, q_ini, q_fim, q_solta, distancia=0.0105):
    """O plano do flash como veu PRETO: emissao 0, alfa 0 -> 1 (LINEAR) de
    q_ini a q_fim, segura ate q_solta-1 e volta a 0 em q_solta (CONSTANT).
    O plano chega a 'distancia' da camera nesses quadros: a 25 cm (o padrao
    do flash) ele ficaria atras da tampa quando a camera esta a 12 cm dela.
    1,05 cm = logo depois do clip_start (1 cm): a 1,5 cm, com a camera ja
    dentro da tampa (q549), um floco de espuma da camada de cima entrava
    entre a camera e o veu e aparecia como uma mancha (medido no render)."""
    amb, cam = objs["ambiente"], objs["camera"]
    plano = amb.get("flash")
    if plano is None:
        plano = mod_ambiente.animar_flash(amb, cam, q_ini - 1000, forca=0.0)
    nt = plano.data.materials[0].node_tree
    alfa = nt.nodes["alfa"].outputs[0]
    emissao = next(n for n in nt.nodes if n.type == "EMISSION").inputs["Strength"]
    chaves = ((q_ini, 0.0, "LINEAR"), (q_fim, 1.0, "CONSTANT"), (q_solta, 0.0, "CONSTANT"))
    for q_, a, _ in chaves:
        alfa.default_value = a
        alfa.keyframe_insert("default_value", frame=q_)
        emissao.default_value = 0.0
        emissao.keyframe_insert("default_value", frame=q_)
    interp = {q_: i for q_, _, i in chaves}
    for fc in nt.animation_data.action.fcurves:
        for kp in fc.keyframe_points:
            i = interp.get(int(round(kp.co.x)))
            if i is not None:
                kp.interpolation = i if fc.data_path.startswith('nodes["alfa"]') else "CONSTANT"
        fc.update()
    alfa.default_value = 0.0
    # Distancia do plano: chave em 1 (padrao) para o beat 5 nao mudar.
    z0 = plano.location.z
    for q_, z in ((1, z0), (q_ini, -distancia), (q_solta, z0)):
        plano.location = (0.0, 0.0, z)
        plano.keyframe_insert("location", frame=q_)
    _interp_nas_chaves(plano, {1, q_ini, q_solta}, "CONSTANT")
    plano.location = (0.0, 0.0, z0)
    return plano


def _beat7(objs, fator):
    """Camera sobe para o eixo da logo, mergulha e ATRAVESSA a tampa sob um
    veu preto; corte para a cartela (parented na camera) com a logo ja
    visivel no centro - match cut logo -> logo."""
    r = ROTEIRO[7]
    q_ini, q_fim = quadros_do_beat(7, fator)
    q = lambda fr: q_em(7, fr, fator)  # noqa: E731
    caixa, cartela, cena, cam, amb = objs["caixa"], objs["cartela"], objs["cena"], objs["camera"], objs["ambiente"]
    p = objs["params"]
    m = p["mergulho"]

    cena.frame_set(q_ini)
    bpy.context.view_layer.update()
    logo = caixa["tampa"].matrix_world @ caixa["centro_logo_local"]
    normal = (caixa["tampa"].matrix_world.to_3x3() @ caixa["normal_logo"]).normalized()
    q_topo, q_t = q(r["sobe_para_logo"][1]), q(r["mergulho"][1])
    objs["q_travessia"] = q_t
    n_trav = max(1, int(m["travessia"]))
    q_perto = q_t - 1 - n_trav
    # O alvo fica 1 m abaixo da camera, 4 mm para +Y: e o que define o "para
    # cima" do quadro (+Y = logo em pe) sem o Track To degenerar na vertical.
    # O foco fica na logo o mergulho inteiro.
    def chave_altura(q_, d, interp):
        pos = logo + normal * d
        # Foco na logo, mas nunca a menos de foco_min da camera (ver PARAMS).
        foco = logo if d >= m["foco_min"] else pos - normal * m["foco_min"]
        _chave_camera(objs, q_, -90.0, 0.0, pos.z, (0.0, 0.004, pos.z - 1.0), foco=foco, interp=interp)

    # Mergulho: uma chave por quadro (ver PARAMS_PADRAO['mergulho']): Hermite
    # do apice (parado) ate 'meio' na fase A, exponencial ate 'perto' na
    # fase B (chega devagar, nao para), e a travessia ACELERA (u^2) ate
    # 'dentro' da tampa - o veu ja esta cobrindo. Abertura f/2,8 -> f/8.
    n_merg = max(2, q_perto - q_topo)
    n_a = max(1, int(round(n_merg / 3.0)))
    n_b = max(1, n_merg - n_a)
    razao = (m["perto"] / m["meio"]) ** (1.0 / n_b)        # fracao por quadro na fase B
    v_meio = m["meio"] * (1.0 - razao)                      # m/quadro no inicio da B
    s = v_meio * n_a / max(m["alto"] - m["meio"], 1e-6)     # a A termina nessa velocidade
    for f in range(q_topo, q_perto + 1):
        k = f - q_topo
        if k <= n_a:
            d = m["meio"] + (m["alto"] - m["meio"]) * _perfil_mergulho(k / float(n_a), s)
        else:
            d = m["meio"] * razao ** (k - n_a)
        chave_altura(f, d, "LINEAR")
    for f in range(q_perto + 1, q_t):
        u = (f - q_perto) / float(n_trav)
        d = m["perto"] + (m["dentro"] - m["perto"]) * u * u
        chave_altura(f, d, "CONSTANT" if f == q_t - 1 else "LINEAR")
    _chave_f(objs, q_topo, m["f_ini"])
    _chave_f(objs, q_t - 1, m["f_fim"])
    _chave_f(objs, q_t, m["f_ini"])
    mod_ambiente.animar_rig(amb, q_ini, q_topo, 6.0, 0.0)
    # Veu preto: alfa 0 -> 1 nos 'veu' quadros antes de a camera tocar a
    # tampa (q_perto+1 e o primeiro quadro da travessia), segura preto ate o
    # corte e solta no corte. O preto nasce da propria logo.
    n_veu = max(1, int(m["veu"]))
    q_veu_fim = q_t - 1 - max(0, n_trav - n_veu)
    _veu_preto(objs, q_veu_fim - n_veu, q_veu_fim, q_t)
    objs["_q_veu"] = (q_veu_fim - n_veu, q_veu_fim)

    # Corte: camera limpa, de costas para a cena, olhando 24 graus para cima.
    # Medido com 12 graus: o horizonte caia a 78% da altura e o brilho (ceu
    # ate 15 graus acima dele mais o chao emissivo abaixo) cobria de 50% a
    # 95% do quadro - as linhas 2-4 ficavam sobre rose claro. Com 24 graus o
    # horizonte fica a ~96% e o brilho so no terco de baixo, sob o bloco.
    z_cam = 1.0
    dist_alvo = 4.0
    alvo_z = z_cam + dist_alvo * math.tan(math.radians(24.0))
    raio0 = 4.0
    _chave_camera(objs, q_t, 90.0, raio0, z_cam, (0.0, raio0 + dist_alvo, alvo_z), interp="LINEAR")
    cena.frame_set(q_t)
    bpy.context.view_layer.update()
    mod_cartela.posicionar_cartela(cartela, cam, cartela["distancia"], subida=p["cartela_subida"], parentear=True)
    raiz = cartela["raiz"]
    bpy.context.view_layer.update()
    foco_cartela = raiz.matrix_world.translation.copy()
    objs["foco"].location = foco_cartela
    objs["foco"].keyframe_insert("location", frame=q_t)
    deriva = 0.12
    _chave_camera(objs, q_fim, 90.0, raio0 + deriva, z_cam, (0.0, raio0 + deriva + dist_alvo, alvo_z),
                  foco=foco_cartela + Vector((0.0, deriva, 0.0)), interp="LINEAR")

    # Cartela escondida ate o corte (visivel DE q_t, logo inclusive); a logo
    # entra ja com alfa 1, maior e no centro do quadro, e viaja ao repouso
    # enquanto as linhas entram: e o match cut com a logo da tampa.
    elementos = list(cartela["linhas"]) + ([cartela["logo"]] if cartela.get("logo") else [])
    _esconder_entre(elementos, 1, q_t)
    subida = cartela.get("subida", 0.0) if p["cartela_subida"] is None else p["cartela_subida"]
    # fracao 0,40 (padrao do modulo 0,55): com 0,55 a linha 1 entrava com a
    # logo ainda no meio da viagem e cruzava a engrenagem (q560 medido).
    mod_cartela.animar_cartela(cartela, q_t, q(r["cartela"][1]), fracao_elemento=p["cartela_fracao"],
                               logo_ja_visivel=True, logo_origem=(0.0, -subida),
                               logo_escala_inicial=p["logo_escala_inicial"])


def _rig_luz_cortes(objs):
    """Chaves CONSTANT do rig de luz nos cortes do beat 5, gravadas DEPOIS de
    todos os animar_rig (que rebaixam toda chave para Bezier)."""
    amb = objs["ambiente"]
    rig = amb["rig"]
    chaves = objs.get("_chaves_rig_luz", {})
    if not chaves:
        return
    q_padrao = objs["_q_rig_luz_padrao"]
    # O valor no quadro do retorno e o que o animar_rig do beat 6 gravou.
    objs["cena"].frame_set(q_padrao)
    ang_padrao = rig.rotation_euler.z
    primeiro = min(chaves)
    objs["cena"].frame_set(primeiro - 1)
    ang_antes = rig.rotation_euler.z
    rig.rotation_euler = (0.0, 0.0, ang_antes)
    rig.keyframe_insert("rotation_euler", index=2, frame=primeiro - 1)
    for q_, ang in chaves.items():
        rig.rotation_euler = (0.0, 0.0, math.radians(ang))
        rig.keyframe_insert("rotation_euler", index=2, frame=q_)
    rig.rotation_euler = (0.0, 0.0, ang_padrao)
    rig.keyframe_insert("rotation_euler", index=2, frame=q_padrao)
    quadros = set(chaves) | {primeiro - 1}
    for fc in rig.animation_data.action.fcurves:
        if fc.data_path != "rotation_euler":
            continue
        for kp in fc.keyframe_points:
            if int(round(kp.co.x)) in quadros:
                kp.interpolation = "CONSTANT"
        fc.update()


def coreografar(objs, fator=None):
    """Os sete beats sobre os objetos de construir_tudo."""
    if fator is None:
        fator = objs.get("fator", 1.0)
    objs["fator"] = fator
    cena = objs["cena"]
    cena.frame_start = 1
    cena.frame_end = quadros_do_beat(7, fator)[1]
    cena.frame_set(1)
    _beat1(objs, fator)
    _beat2(objs, fator)
    _beat3(objs, fator)
    _beat4(objs, fator)
    _beat5(objs, fator)
    _beat6(objs, fator)
    _beat7(objs, fator)
    _rig_luz_cortes(objs)
    _aplicar_interpolacao_camera(objs)
    cena.frame_set(1)
    # Uma action de node tree fica orfa durante a coreografia da segunda
    # rodada (medido: 'Shader NodetreeAction.003'); a purga de construir_tudo
    # vem antes dela, por isso outra aqui.
    _purgar_acoes_orfas()
    return objs


# ---------------------------------------------------------------- render

def configurar_render(objs, largura=1080, altura=1920, amostras=64, video=False, caminho_saida=None):
    """Render do modulo ambiente mais o que os outros modulos pediram."""
    cena = objs["cena"]
    p = mod_ambiente.configurar_render(cena, largura, altura, fps=int(FPS), amostras=amostras,
                                       params={"video": video, "caminho_saida": caminho_saida})
    ee = cena.eevee
    # Chanfro do plugue em close chuvisca com menos que isto (achado do cabo).
    _ajustar(ee, "shadow_ray_count", 4)
    _ajustar(ee, "shadow_step_count", 8)
    # Obturador abre no quadro e fecha depois: o quadro de um corte nao
    # mistura o plano anterior (ver cabecalho).
    _ajustar(cena.render, "motion_blur_position", "START")
    return p


def preparar_para_salvar():
    """Empacota as imagens no .blend (logo, telas): sem isto elas apontam para
    a pasta temporaria de onde o arquivo unico as extraiu. Devolve os nomes
    empacotados agora. O caminho de origem FICA gravado na imagem (e a
    convencao do Blender para arquivo empacotado, e e por ele que os modulos
    reaproveitam a imagem na rodada seguinte via check_existing - medido:
    apagar o caminho fazia a segunda rodada criar 'logo_engineprint.png.001');
    os pixels vem do .blend, provado reabrindo o arquivo em processo limpo."""
    return mod_ambiente.empacotar_imagens()


def renderizar_quadro(objs, quadro_, caminho):
    cena = objs["cena"]
    cena.frame_set(quadro_)
    cena.render.filepath = caminho
    bpy.ops.render.render(write_still=True)
    return caminho


# ---------------------------------------------------------------- conferencia

def conferir_colisoes(objs, passo=1):
    """Mede, quadro a quadro nos beats 2 e 6, se o U1 atravessa o fundo da
    caixa e quantas espumas estao dentro do volume do U1. Devolve um dict com
    os piores casos; imprime. Numero visivel sai de medicao."""
    cena, caixa, u1 = objs["cena"], objs["caixa"], objs["u1"]
    fator = objs["fator"]
    parede = mod_caixa.PARAMS_PADRAO["parede"]
    mn, mx = u1["envelope"]
    piores = {"u1_abaixo_do_fundo_m": 0.0, "espumas_no_u1": 0, "quadro_pior": None, "u1_x_tampa": 0}
    tampa = caixa["tampa"]
    ext_t = caixa["exterior_tampa"]
    ext_c = caixa["exterior_corpo"]
    for n in (2, 6):
        a, b = quadros_do_beat(n, fator)
        for f in range(a, b + 1, passo):
            cena.frame_set(f)
            pu = u1["raiz"].matrix_world.translation
            zu = pu.z
            pc = caixa["corpo"].matrix_world.translation
            fundo = pc.z + parede
            # So conta quando o U1 esta sobre a pegada da caixa (com
            # caixa_some=False ele pousa 2 m na frente dela).
            sobre_caixa = abs(pu.x - pc.x) < ext_c[0] / 2.0 and abs(pu.y - pc.y) < ext_c[1] / 2.0
            if sobre_caixa and zu < fundo - 1e-4:
                piores["u1_abaixo_do_fundo_m"] = max(piores["u1_abaixo_do_fundo_m"], fundo - zu)
            dentro = 0
            for esp in caixa["espumas"]:
                p_ = esp.matrix_world.translation - Vector((pu.x, pu.y, 0.0))
                raio = float(esp["caixa_raio"])
                if (mn.x + raio * 0.5 < p_.x < mx.x - raio * 0.5 and mn.y + raio * 0.5 < p_.y < mx.y - raio * 0.5
                        and zu + mn.z + raio * 0.5 < p_.z < zu + mx.z - raio * 0.5):
                    dentro += 1
            if dentro > piores["espumas_no_u1"]:
                piores["espumas_no_u1"] = dentro
                piores["quadro_pior"] = f
            # Tampa x U1: a tampa e oca, e fechada envolve os 4 cm de cima do
            # U1 - isso e o normal. Colisao e o topo do U1 passar do TETO
            # interno dela (ou a parede lateral dela cruzar o U1 quando ela
            # esta deslocada em X, o que a inclinacao da saida evita).
            tm = tampa.matrix_world
            teto = tm.translation.z + ext_t[2] / 2.0 - parede
            base = tm.translation.z - ext_t[2] / 2.0
            dx, dy = tm.translation.x - pu.x, tm.translation.y - pu.y
            sobre = abs(dx) < ext_t[0] / 2.0 + mx.x and abs(dy) < ext_t[1] / 2.0 + mx.y
            deslocada = abs(dx) > 0.005 or abs(dy) > 0.005
            if sobre and not tampa.hide_render:
                topo_u1 = zu + mx.z
                if topo_u1 > teto + 1e-3 and zu + mn.z < teto:
                    piores["u1_x_tampa"] += 1
                elif deslocada and base < topo_u1 - 1e-3 and abs(dx) < mx.x + ext_t[0] / 2.0 - 0.02:
                    piores["u1_x_tampa"] += 1
    print("[coreografia] colisoes: U1 abaixo do fundo da caixa = %.4f m; espumas dentro do U1 (pior quadro %s) = %d; quadros com tampa x U1 = %d"
          % (piores["u1_abaixo_do_fundo_m"], piores["quadro_pior"], piores["espumas_no_u1"], piores["u1_x_tampa"]))
    cena.frame_set(1)
    return piores
