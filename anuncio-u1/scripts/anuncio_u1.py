# ============================================================================
# ANUNCIO SNAPMAKER U1 - EnginePrint - arquivo unico para o Blender (4.2+)
# ============================================================================
#
# COMO USAR: abra o Blender com a sua cena (ou uma vazia), aba Scripting,
# New, cole este arquivo inteiro, ajuste os parametros abaixo e Run Script.
# O script monta a colecao ANUNCIO (caixa, U1, cabo, luzes, camera, cartela),
# coreografa os 600 quadros (20 s a 30 fps) e configura o render (EEVEE
# Next, 1080x1920, AgX). Depois: Render > Render Animation.
#
# Para usar o SEU modelo do U1: ponha o nome do objeto (ou da colecao) em
# U1_NOME. O script parenteia o modelo num Empty 'u1.raiz', mede o envelope,
# centraliza em XY, apoia em z = 0 e o poe dentro da caixa. Se a frente do
# seu modelo nao aponta para -Y, use U1_ROTACAO_Z (graus). Os pontos da
# tela, da tomada e do botao vem de U1_TELA / U1_TOMADA / U1_BOTAO (XYZ nas
# coordenadas originais do seu arquivo); vazios, saem de uma heuristica pelo
# envelope (tela a 30% da largura para a direita e 80% da altura, na frente;
# tomada e botao a 42% para a direita e 17% / 24% da altura, atras). Para a
# tela acender e o botao afundar, indique os objetos em U1_TELA_OBJETO e
# U1_BOTAO_OBJETO (a tela precisa de um material com Emission).
#
# Rodar de novo nao duplica nada: cada modulo apaga a propria colecao antes.
# ============================================================================

# ---------------------------- PARAMETROS -----------------------------------
U1_NOME = ""                 # "" = U1 substituto; ou nome do objeto/colecao do seu modelo
U1_ROTACAO_Z = 0.0           # graus, para a frente do seu modelo apontar para -Y
U1_TELA = None               # (x, y, z) do centro da tela no seu arquivo, ou None
U1_TOMADA = None             # (x, y, z) da tomada IEC, ou None
U1_BOTAO = None              # (x, y, z) do botao liga/desliga, ou None
U1_TELA_OBJETO = ""          # nome do objeto da tela (para acender), ou ""
U1_BOTAO_OBJETO = ""         # nome do objeto do botao (para afundar), ou ""
U1_LED_OBJETO = ""           # nome de um objeto com LED (Emission), ou ""
DURACAO_S = 20               # 20 (referencia) ou 15 (preset frenetico)
COR_CAIXA = "clara"          # "clara" (#F2EDE6) ou "escura" (#141416)
RESOLUCAO = (1080, 1920)     # 9:16 vertical
AMOSTRAS = 64                # amostras do EEVEE no render final
SALVAR_BLEND = True          # grava anuncio_u1.blend ao lado do seu .blend
# ---------------------------------------------------------------------------

import base64 as _base64
import os as _os
import sys as _sys
import tempfile as _tempfile
import types as _types


def _registrar_modulo(nome, dic):
    """Transforma o dict de um modulo embutido num modulo de verdade em
    sys.modules, para os 'import mod_x' entre modulos funcionarem."""
    mod = _types.ModuleType(nome)
    mod.__dict__.update(dic)
    _sys.modules[nome] = mod
    return mod


# ============================================================================
# MODULO mod_ambiente (scripts/mod_ambiente.py), inteiro, dentro de uma funcao-namespace
# ============================================================================
def _modulo_ambiente():
    # Modulo AMBIENTE do anuncio do Snapmaker U1.
    #
    # Entrega o "estudio": fundo em gradiente preto -> rose-branco, chao escuro
    # com reflexo suave, quatro luzes de estudio (key, fill, rim, top), a camera
    # principal com profundidade de campo, a configuracao de render (EEVEE Next,
    # AgX, motion blur, raytracing, bloom no compositor) e o FLASH de foto do
    # beat 5. So definicoes aqui - nada roda no import. Quem integra e
    # mod_coreografia.py; quem prova este modulo sozinho e teste_ambiente.py.
    #
    # DECISAO: o gradiente vive no WORLD, nao num ciclorama.
    #
    # A camera orbita 360 graus e o produto e metal e vidro: tudo o que esta
    # atras da camera aparece refletido no produto. Um ciclorama e uma parede:
    # tem borda, tem costura onde o chao encontra a curva, e ou ele envolve a cena
    # inteira (uma cupula, que e o world com mais passos) ou a camera, em alguma
    # fase da orbita, enxerga o fim dele - direto ou num reflexo. O world nao tem
    # borda. O gradiente aqui e funcao SO da ELEVACAO da direcao de visada (o Z
    # do vetor normalizado): e por isso que ele e identico em qualquer angulo da
    # orbita e identico no reflexo, porque reflexo tambem e uma direcao. Uma
    # faixa rose baixa em volta do horizonte, escurecendo rumo ao zenite - e o
    # fundo de estudio da Apple: um brilho atras do produto, nao um ceu branco.
    #
    # A CURVA do gradiente e feita para a camera do anuncio, nao para o ceu
    # inteiro. A 35 mm em 9:16 ve 54 graus na vertical; olhando de leve para
    # baixo (8 a 18 graus), o topo do quadro fica entre 9 e 19 graus de elevacao.
    # As tres primeiras versoes so chegavam ao preto acima de ~35 graus, ou seja,
    # NUNCA dentro do quadro: o topo saia cinza (#C0B8B6 medido) e o brilho caia
    # no meio da imagem. Agora o brilho e MAXIMO no horizonte e abaixo dele, cai
    # pela metade a ~7 graus e e preto a ~15 graus - a faixa e estreita, e e por
    # isso que a forca pode ser alta (1,8) sem inundar a cena.
    #
    # A curva tem UM segmento de queda, de proposito. O ColorRamp em EASE faz um
    # smoothstep entre cada par de pontos, com derivada zero nos nos: com cinco
    # pontos na descida a previa mostrou dois "degraus" (patamares em 27 e 98 de
    # 255 no perfil da coluna) que o olho le como faixas. Um so segmento e uma so
    # curva em S, sem patamar. O rose: o AgX puxa cor clara para o branco, e o
    # #F4E6E4 saia #D8D1D0 com saturacao 0,04. Medido em grade (forca x
    # saturacao, so o world): forca 1,8 e saturacao 6 dao L 0,86 e saturacao
    # ~0,10 depois do AgX - abaixo disso e cinza, acima vira salmao.
    #
    # O world e DOIS Backgrounds, separados por Light Path -> Is Camera Ray:
    # - a camera ve o gradiente inteiro, com o rose tambem abaixo do horizonte
    #   (o chao infinito funde nele, ver abaixo);
    # - a iluminacao (o probe do EEVEE, que tambem e o fallback do raytracing)
    #   ve o mesmo gradiente acima do horizonte e PRETO abaixo dele. Abaixo do
    #   horizonte, no mundo real, ha o chao - mas o probe do EEVEE nao e
    #   ocluido por geometria, e um hemisferio inteiro de rose a 1,8 de forca
    #   iluminaria todo objeto por baixo. Como acima do horizonte as duas
    #   versoes sao iguais, o cromo reflete o mesmo ceu que a camera ve; abaixo
    #   dele o raio bate no chao (raytracing) ou cai no probe escuro, que e a
    #   cor que o chao teria de qualquer jeito.
    #
    # O horizonte: o chao e um plano de 400 m e a borda dele cai a menos de 0,3
    # grau abaixo do horizonte verdadeiro. A linha que APARECIA nas previas nao
    # era a borda: era a curva, que valia 0,10 na elevacao zero e 1,0 a 7 graus -
    # uma faixa escura entre o brilho e o chao, e o chao infinito copiava
    # justamente essa cor escura. Fundo Apple nao tem horizonte. Agora a curva
    # tem o valor maximo no horizonte, e o "chao infinito" (longe da origem, 6 ->
    # 30 m, o material do chao vira emissao com a cor do horizonte e perde o
    # especular) funde no proprio brilho. De perto o chao e chao; de longe e o
    # brilho; a costura nao existe de nenhum angulo da orbita porque a camera
    # nunca sai de perto da caixa.
    #
    # Banding: o gradiente recebe um ruido fino (Noise Texture sobre a direcao,
    # +-0,05% no fator do ramp, grao abaixo do pixel: escala 3000 = 0,019 grau,
    # contra 0,028 grau por pixel a 1920 de altura) ANTES do ColorRamp, e o
    # render sai com dither_intensity no maximo. A versao anterior tinha grao de
    # 2-3 px e amplitude 12x maior, e aparecia como mosqueado no ceu - e por ser
    # deterministico nao sumia com mais amostras. Medido (world so, 540x960, com
    # e sem cada um): o ruido do shader nesta amplitude nao muda nenhum numero
    # do perfil; o dither de saida e o que faz o servico - poe +-2 niveis por
    # pixel e derruba os niveis distintos numa coluna de 157 para 142. Na parte
    # mais ingreme da curva a inclinacao e de 2 niveis por linha a 960 px (1 a
    # 1920), que e rampa, nao escada: o teste procura PATAMAR seguido de salto.
    #
    # Eixos e medidas seguem docs/ESPECIFICACAO.md: metros, Z para cima, frente
    # em -Y (a camera padrao fica em -Y olhando +Y), chao em z = 0.

    import math

    import bpy

    NOME = "ambiente"
    NOME_CAMERA = "camera"

    PARAMS_PADRAO = {
        "cor_escura": "#050507",
        "cor_clara": "#F4E6E4",
        # Curva do gradiente: (fator, mistura). fator = (sen(elevacao) + 0,1) / 1,1:
        # 0 = 5,7 graus abaixo do horizonte, 0,091 = horizonte, 1 = zenite;
        # mistura 0 = preto, 1 = rose-branco. E dado, nao numero solto, para a
        # coreografia poder pedir um fundo mais fechado num beat.
        # Maximo no horizonte E abaixo dele (os dois primeiros pontos iguais a 1,0:
        # e a cor que o chao infinito copia, ver cor_horizonte), e UM segmento em
        # S ate o preto a 14,6 graus (0,34): metade a ~7 graus, 0,29 a 10 graus. A
        # camera do anuncio, de leve para baixo, tem o topo do quadro entre 9 e 19
        # graus: e ali que o preto tem de estar, nao a 35 graus como antes. Nao
        # acrescente pontos no meio da descida: cada no do EASE vira um patamar.
        "curva": ((0.0, 1.0), (0.09, 1.0), (0.34, 0.0), (1.0, 0.0)),
        # Forca do Background que a CAMERA ve. Com a faixa estreita, 1,8 e o que
        # leva o pico do brilho a L 0,86 depois do AgX (0,55 dava cinza).
        "forca_mundo": 1.8,
        # Forca da versao que ILUMINA (probe), como fracao de forca_mundo. 1,0 =
        # o cromo reflete exatamente o ceu que a camera ve; a protecao contra a
        # inundacao e o preto abaixo do horizonte, nao este numero.
        "forca_luz": 1.0,
        # O AgX empalidece cor clara: o rose #F4E6E4 saia cinza (saturacao 0,04
        # com 1,6). Compensa na cor do world, nao na paleta (a mesma licao da logo
        # no modulo caixa). 6,0 e o medido para ~0,10 de saturacao no PNG.
        "saturacao_clara": 6.0,
        # Amplitude do ruido no fator do ramp. 0,0015 ainda dava degrau de 3,8
        # niveis entre linhas vizinhas na parte ingreme da curva.
        "dither": 0.0005,
        "escala_dither": 3000.0,  # grao do ruido: abaixo do pixel a 1920 de altura
        # Chao
        "tamanho_chao": 400.0,
        "cor_chao": "#08080A",
        # Reflexo SUTIL: rugosidade 0,45 e especular 0,15 - com 0,35/0,2 as area
        # lights ainda viravam pocas brancas no chao (luminancia media 0,66
        # medida na frente; a meta e o chao escuro, com o produto "sentado").
        "rugosidade_chao": 0.45,
        "especular_chao": 0.15,
        # Onde o chao comeca a virar o brilho do horizonte e onde termina (m da
        # origem). A camera nunca passa de ~4 m da origem.
        "fusao_chao": (6.0, 30.0),
        # Luzes: posicao (m), tamanho (x, y em m), energia (W), cor. As posicoes
        # sao relativas ao rig, um Empty na origem que a coreografia gira junto
        # com a orbita - o rim so recorta a silhueta se ficar ATRAS do produto do
        # ponto de vista da camera, e "atras" muda a cada quadro numa orbita.
        "alvo_luzes": (0.0, 0.0, 0.42),
        "luzes": {
            # 'abertura' e o spread da area light, em graus: e a colmeia do softbox.
            # 180 (padrao) espalha luz de lado e enche o chao de reflexo; o rim
            # com 40 graus recorta a silhueta sem pintar o chao na frente.
            # 'especular' e o multiplicador de especular da luz (so EEVEE): abaixo
            # de 1 a luz continua iluminando o difuso e reflete menos no chao.
            "key":  {"pos": (2.2, -2.4, 2.8), "tam": (2.0, 2.0), "energia": 350.0, "cor": (1.0, 0.95, 0.90), "abertura": 100.0},
            "fill": {"pos": (-3.0, -2.0, 1.6), "tam": (3.0, 3.0), "energia": 110.0, "cor": (0.90, 0.94, 1.0), "abertura": 120.0},
            # O rim fica BAIXO (na altura do produto) e longe. O ponto onde o
            # reflexo dele cai no chao esta a t = z_cam / (z_cam + z_luz) do
            # caminho camera -> luz: com a camera a 0,75 m e o rim a 2,9 m era
            # t = 0,21, uma poca branca a 1,9 m NA FRENTE do produto (quanto mais
            # alto o rim, mais perto da camera cai a poca - o raciocinio anterior
            # estava invertido). A 1,0 m, t = 0,43 e a poca cai em y ~ -0,5, sob a
            # silhueta do produto: e o brilho no chao atras dele, o look Apple.
            # especular 0,6: o reflexo do rim no chao atras do produto era uma
            # cunha branca na camera de cima; 0,6 o deixa como brilho, e o recorte
            # no cromo e na aresta do cubo (que estava estourado) segue la.
            "rim":  {"pos": (0.6, 2.8, 1.0), "tam": (0.3, 2.2), "energia": 350.0, "cor": (1.0, 1.0, 1.0), "abertura": 40.0, "especular": 0.6},
            "top":  {"pos": (0.0, 0.3, 3.6), "tam": (3.0, 3.0), "energia": 80.0, "cor": (1.0, 0.98, 0.96), "abertura": 100.0},
        },
        # Flash
        "forca_flash": 16.0,      # emissao branca; com AgX, 1.0 nao chega ao branco
        "distancia_flash": 0.25,  # do sensor; acima do clip_start padrao (0,1)
    }

    PARAMS_CAMERA_PADRAO = {
        "lente": 35.0,
        "sensor": 36.0,
        "f": 2.8,
        "pos": (1.6, -2.4, 1.2),
        "alvo": (0.0, 0.0, 0.42),
        "dof": True,
    }

    PARAMS_RENDER_PADRAO = {
        "shutter": 0.5,
        "bloom": True,
        "limiar_bloom": 2.5,       # so o que esta bem acima de 1.0 floresce
        "mistura_bloom": -0.75,    # -1 = sem glare, 0 = meio a meio
        "tamanho_bloom": 6,
        "video": False,            # True: FFMPEG H.264
        "caminho_saida": None,
    }


    # ---------------------------------------------------------------- utilidades

    def _srgb_para_linear(c):
        # A paleta esta em sRGB (hex); os nos de shader querem linear.
        c = c / 255.0
        return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4


    def cor_linear(hexa):
        h = hexa.lstrip("#")
        return tuple(_srgb_para_linear(int(h[i:i + 2], 16)) for i in (0, 2, 4)) + (1.0,)


    def saturar(cor, fator):
        # Afasta cada canal do cinza de mesma luminancia; 1.0 nao muda nada.
        r, g, b = cor[:3]
        y = 0.2126 * r + 0.7152 * g + 0.0722 * b
        return tuple(max(0.0, y + (c - y) * fator) for c in (r, g, b)) + (1.0,)


    def limpar_colecao(nome):
        """Remove a sub-colecao <nome> e tudo o que ela contem (idempotencia)."""
        col = bpy.data.collections.get(nome)
        if col is None:
            return
        for obj in list(col.all_objects):
            dados = obj.data
            bpy.data.objects.remove(obj, do_unlink=True)
            # Dado orfao fica no arquivo ate o proximo salvar; apagar aqui evita
            # acumular 'ambiente.chao.001' e 'camera.principal.001' nas rodadas.
            if dados is not None and dados.users == 0:
                for colecao in (bpy.data.meshes, bpy.data.lights, bpy.data.cameras):
                    try:
                        colecao.remove(dados)
                        break
                    except (TypeError, ReferenceError):
                        continue
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


    def _mesclar(padrao, params):
        p = dict(padrao)
        if params:
            for k, v in params.items():
                if isinstance(v, dict) and isinstance(p.get(k), dict):
                    d = {kk: dict(vv) if isinstance(vv, dict) else vv for kk, vv in p[k].items()}
                    for kk, vv in v.items():
                        if isinstance(vv, dict) and isinstance(d.get(kk), dict):
                            d[kk].update(vv)
                        else:
                            d[kk] = vv
                    p[k] = d
                else:
                    p[k] = v
        return p


    def _entrada(no, nome, valor):
        # Nome de socket muda entre versoes (Specular -> Specular IOR Level no
        # 4.0); quem nao existe e ignorado em vez de derrubar o script.
        soquete = no.inputs.get(nome)
        if soquete is not None:
            soquete.default_value = valor


    def _material(nome):
        mat = bpy.data.materials.get(nome)
        if mat is not None:
            bpy.data.materials.remove(mat)
        mat = bpy.data.materials.new(nome)
        mat.use_nodes = True
        nt = mat.node_tree
        bsdf = nt.nodes.get("Principled BSDF")
        return mat, nt, bsdf


    def _apontar(obj, alvo):
        tr = obj.constraints.new("TRACK_TO")
        tr.target = alvo
        tr.track_axis = "TRACK_NEGATIVE_Z"
        tr.up_axis = "UP_Y"
        return tr


    # ---------------------------------------------------------------- world

    def _mundo(p):
        nome = NOME + ".mundo"
        mundo = bpy.data.worlds.get(nome)
        if mundo is not None:
            bpy.data.worlds.remove(mundo)
        mundo = bpy.data.worlds.new(nome)
        mundo.use_nodes = True
        nt = mundo.node_tree
        for no in list(nt.nodes):
            nt.nodes.remove(no)

        saida = nt.nodes.new("ShaderNodeOutputWorld")
        saida.location = (900, 0)

        # No world, 'Generated' e a direcao de visada normalizada, em coordenadas
        # do mundo: o Z dela e o seno da elevacao. E a unica coordenada que nao
        # depende de onde a camera esta - so de para onde o raio aponta.
        coord = nt.nodes.new("ShaderNodeTexCoord")
        coord.location = (-900, 0)
        sep = nt.nodes.new("ShaderNodeSeparateXYZ")
        sep.location = (-700, 0)
        nt.links.new(coord.outputs["Generated"], sep.inputs["Vector"])

        # O fator comeca 5,7 graus abaixo do horizonte (-0,10) para o brilho
        # continuar por baixo dele - e la que o chao infinito funde no ceu.
        faixa = nt.nodes.new("ShaderNodeMapRange")
        faixa.location = (-500, 0)
        faixa.inputs["From Min"].default_value = -0.10
        faixa.inputs["From Max"].default_value = 1.0
        faixa.inputs["To Min"].default_value = 0.0
        faixa.inputs["To Max"].default_value = 1.0
        nt.links.new(sep.outputs["Z"], faixa.inputs["Value"])

        # Dither: ruido fino sobre a direcao, somado ao fator antes do ramp.
        escala = nt.nodes.new("ShaderNodeVectorMath")
        escala.operation = "SCALE"
        escala.location = (-700, -300)
        escala.inputs["Scale"].default_value = p["escala_dither"]
        nt.links.new(coord.outputs["Generated"], escala.inputs[0])
        ruido = nt.nodes.new("ShaderNodeTexNoise")
        ruido.location = (-500, -300)
        ruido.inputs["Scale"].default_value = 1.0
        _entrada(ruido, "Detail", 0.0)
        _entrada(ruido, "Roughness", 0.0)
        nt.links.new(escala.outputs["Vector"], ruido.inputs["Vector"])
        amplitude = nt.nodes.new("ShaderNodeMapRange")
        amplitude.location = (-300, -300)
        amplitude.inputs["From Min"].default_value = 0.35
        amplitude.inputs["From Max"].default_value = 0.65
        amplitude.inputs["To Min"].default_value = -p["dither"]
        amplitude.inputs["To Max"].default_value = p["dither"]
        nt.links.new(ruido.outputs["Fac"], amplitude.inputs["Value"])
        soma = nt.nodes.new("ShaderNodeMath")
        soma.operation = "ADD"
        soma.location = (-100, 0)
        nt.links.new(faixa.outputs["Result"], soma.inputs[0])
        nt.links.new(amplitude.outputs["Result"], soma.inputs[1])

        # Uma so rampa preto -> rose faz a cor; a curva de mistura por elevacao
        # fica numa segunda rampa em escala de cinza, para a cor e a forma do
        # brilho serem ajustaveis separadas.
        forma = nt.nodes.new("ShaderNodeValToRGB")
        forma.location = (100, 0)
        forma.color_ramp.interpolation = "EASE"
        pontos = forma.color_ramp.elements
        while len(pontos) > 1:
            pontos.remove(pontos[-1])
        pontos[0].position = p["curva"][0][0]
        pontos[0].color = (p["curva"][0][1],) * 3 + (1.0,)
        for pos, mist in p["curva"][1:]:
            e = pontos.new(pos)
            e.color = (mist,) * 3 + (1.0,)
        nt.links.new(soma.outputs["Value"], forma.inputs["Fac"])

        cor = nt.nodes.new("ShaderNodeMix")
        cor.data_type = "RGBA"
        cor.location = (320, 0)
        cor.inputs["A"].default_value = cor_linear(p["cor_escura"])
        cor.inputs["B"].default_value = saturar(cor_linear(p["cor_clara"]), p["saturacao_clara"])
        nt.links.new(forma.outputs["Color"], cor.inputs["Factor"])

        # Background da CAMERA: o gradiente inteiro.
        fundo_cam = nt.nodes.new("ShaderNodeBackground")
        fundo_cam.location = (500, 100)
        fundo_cam.inputs["Strength"].default_value = p["forca_mundo"]
        nt.links.new(cor.outputs["Result"], fundo_cam.inputs["Color"])

        # Background da ILUMINACAO: o mesmo gradiente, apagado abaixo do horizonte
        # (ver cabecalho: o probe nao e ocluido pelo chao, e um hemisferio de rose
        # forte iluminaria tudo por baixo). A rampa de -0,04 a 0,0 e curta para a
        # versao da luz ser identica a da camera em toda a faixa que o cromo
        # reflete acima do horizonte.
        mascara = nt.nodes.new("ShaderNodeMapRange")
        mascara.location = (-500, 300)
        mascara.inputs["From Min"].default_value = -0.04
        mascara.inputs["From Max"].default_value = 0.0
        mascara.inputs["To Min"].default_value = 0.0
        mascara.inputs["To Max"].default_value = 1.0
        nt.links.new(sep.outputs["Z"], mascara.inputs["Value"])
        forca_luz = nt.nodes.new("ShaderNodeMath")
        forca_luz.operation = "MULTIPLY"
        forca_luz.location = (320, 300)
        forca_luz.inputs[1].default_value = p["forca_mundo"] * p["forca_luz"]
        nt.links.new(mascara.outputs["Result"], forca_luz.inputs[0])
        fundo_luz = nt.nodes.new("ShaderNodeBackground")
        fundo_luz.location = (500, 300)
        nt.links.new(cor.outputs["Result"], fundo_luz.inputs["Color"])
        nt.links.new(forca_luz.outputs["Value"], fundo_luz.inputs["Strength"])

        # Is Camera Ray: 1 no render pela camera, 0 no probe do world (EEVEE e
        # Cycles). Se o no faltar em alguma versao, fica so a versao da camera -
        # a cena inunda, mas nao fica preta.
        try:
            caminho = nt.nodes.new("ShaderNodeLightPath")
            caminho.location = (500, -200)
            mistura = nt.nodes.new("ShaderNodeMixShader")
            mistura.location = (700, 0)
            nt.links.new(caminho.outputs["Is Camera Ray"], mistura.inputs["Fac"])
            nt.links.new(fundo_luz.outputs["Background"], mistura.inputs[1])
            nt.links.new(fundo_cam.outputs["Background"], mistura.inputs[2])
            nt.links.new(mistura.outputs["Shader"], saida.inputs["Surface"])
        except (RuntimeError, KeyError) as e:
            print("[ambiente] sem Light Path no world, iluminacao = camera:", e)
            nt.links.new(fundo_cam.outputs["Background"], saida.inputs["Surface"])
        return mundo


    # ---------------------------------------------------------------- chao

    def cor_horizonte(p):
        """Cor linear do world na elevacao zero, ja com a forca - o que o chao copia.

        Com a curva padrao (mistura 1,0 no horizonte) e a propria cor do brilho:
        e assim que o chao infinito funde no brilho, sem faixa escura entre os dois.
        """
        escura = cor_linear(p["cor_escura"])
        clara = saturar(cor_linear(p["cor_clara"]), p["saturacao_clara"])
        m = p["curva"][0][1]
        return tuple((escura[i] * (1 - m) + clara[i] * m) * p["forca_mundo"] for i in range(3)) + (1.0,)


    def _chao(col, p):
        nome = NOME + ".chao"
        t = p["tamanho_chao"] / 2.0
        malha = bpy.data.meshes.new(nome)
        malha.from_pydata([(-t, -t, 0), (t, -t, 0), (t, t, 0), (-t, t, 0)], [], [(0, 1, 2, 3)])
        malha.update()
        obj = bpy.data.objects.new(nome, malha)
        col.objects.link(obj)

        mat, nt, bsdf = _material(nome)
        _entrada(bsdf, "Base Color", cor_linear(p["cor_chao"]))
        _entrada(bsdf, "Metallic", 0.0)

        # Chao infinito: 'fusao' vai de 0 (perto, chao de verdade) a 1 (longe,
        # emissao com a cor do ceu no horizonte, sem especular). Coordenada de
        # objeto e nao de camera, para o resultado nao depender de onde a camera
        # esta - a orbita passa por todo lado.
        coord = nt.nodes.new("ShaderNodeTexCoord")
        coord.location = (-900, 300)
        comp = nt.nodes.new("ShaderNodeVectorMath")
        comp.operation = "LENGTH"
        comp.location = (-700, 300)
        nt.links.new(coord.outputs["Object"], comp.inputs[0])
        fusao = nt.nodes.new("ShaderNodeMapRange")
        fusao.location = (-500, 300)
        fusao.inputs["From Min"].default_value = p["fusao_chao"][0]
        fusao.inputs["From Max"].default_value = p["fusao_chao"][1]
        fusao.inputs["To Min"].default_value = 0.0
        fusao.inputs["To Max"].default_value = 1.0
        try:
            fusao.interpolation_type = "SMOOTHSTEP"
        except AttributeError:
            pass
        nt.links.new(comp.outputs["Value"], fusao.inputs["Value"])

        rug = nt.nodes.new("ShaderNodeMapRange")
        rug.location = (-300, 400)
        rug.inputs["To Min"].default_value = p["rugosidade_chao"]
        rug.inputs["To Max"].default_value = 1.0
        nt.links.new(fusao.outputs["Result"], rug.inputs["Value"])
        nt.links.new(rug.outputs["Result"], bsdf.inputs["Roughness"])

        esp = nt.nodes.new("ShaderNodeMapRange")
        esp.location = (-300, 200)
        esp.inputs["To Min"].default_value = p["especular_chao"]
        esp.inputs["To Max"].default_value = 0.0
        nt.links.new(fusao.outputs["Result"], esp.inputs["Value"])
        if bsdf.inputs.get("Specular IOR Level") is not None:
            nt.links.new(esp.outputs["Result"], bsdf.inputs["Specular IOR Level"])

        _entrada(bsdf, "Emission Color", cor_horizonte(p))
        nt.links.new(fusao.outputs["Result"], bsdf.inputs["Emission Strength"])
        malha.materials.append(mat)
        return obj


    # ---------------------------------------------------------------- luzes

    def _luz(col, nome_curto, cfg, rig, alvo):
        nome = "%s.luz.%s" % (NOME, nome_curto)
        dados = bpy.data.lights.new(nome, "AREA")
        dados.shape = "RECTANGLE"
        dados.size, dados.size_y = cfg["tam"]
        dados.energy = cfg["energia"]
        dados.color = cfg["cor"]
        dados.spread = math.radians(cfg.get("abertura", 180.0))
        if "especular" in cfg:
            # Atributo do EEVEE; se sumir numa versao, a luz fica com 1,0.
            _ajustar(dados, "specular_factor", cfg["especular"])
        try:
            # Sem isto a luz vaza por parede fina (8 mm da caixa) e a face interna
            # sai chuviscada - achado do modulo caixa. Nao existe antes do 4.2.
            dados.use_shadow_jitter = True
        except AttributeError:
            pass
        obj = bpy.data.objects.new(nome, dados)
        obj.location = cfg["pos"]
        obj.parent = rig
        col.objects.link(obj)
        _apontar(obj, alvo)
        return obj


    # ---------------------------------------------------------------- API

    def construir_ambiente(cena, colecao_pai=None, params=None):
        """Cria world, chao, rig com 4 luzes na sub-colecao 'ambiente'."""
        p = _mesclar(PARAMS_PADRAO, params)
        limpar_colecao(NOME)
        col = _colecao(cena, colecao_pai, NOME)

        mundo = _mundo(p)
        cena.world = mundo
        chao = _chao(col, p)

        rig = bpy.data.objects.new(NOME + ".rig", None)
        rig.empty_display_type = "PLAIN_AXES"
        rig.empty_display_size = 0.5
        col.objects.link(rig)
        alvo = bpy.data.objects.new(NOME + ".alvo", None)
        alvo.empty_display_type = "SPHERE"
        alvo.empty_display_size = 0.1
        alvo.location = p["alvo_luzes"]
        alvo.parent = rig
        col.objects.link(alvo)

        luzes = {k: _luz(col, k, cfg, rig, alvo) for k, cfg in p["luzes"].items()}

        objs = {
            "colecao": col,
            "mundo": mundo,
            "chao": chao,
            "rig": rig,
            "alvo_luzes": alvo,
            "luzes": luzes,
            "flash": None,
            "cor_escura": p["cor_escura"],
            "cor_clara": p["cor_clara"],
            "z_chao": 0.0,
            "params": p,
        }
        objs.update(luzes)
        return objs


    def criar_camera(cena, colecao=None, nome="camera.principal", params=None):
        """Camera 35 mm full-frame com alvo (Track To) e DoF no alvo. Devolve (camera, alvo)."""
        p = _mesclar(PARAMS_CAMERA_PADRAO, params)
        limpar_colecao(NOME_CAMERA)
        col = _colecao(cena, colecao, NOME_CAMERA)

        dados = bpy.data.cameras.new(nome)
        dados.lens = p["lente"]
        # 'sensor 36' = full frame. Em AUTO os 36 mm vao para o lado MAIOR do
        # quadro; no 9:16 vertical e a altura - o que e a foto de celular com o
        # sensor em pe, e o que da 54 graus verticais com a 35 mm.
        dados.sensor_fit = "AUTO"
        dados.sensor_width = p["sensor"]
        dados.clip_start = 0.05
        dados.clip_end = 500.0
        cam = bpy.data.objects.new(nome, dados)
        cam.location = p["pos"]
        col.objects.link(cam)

        alvo = bpy.data.objects.new("camera.alvo", None)
        alvo.empty_display_type = "SPHERE"
        alvo.empty_display_size = 0.08
        alvo.location = p["alvo"]
        col.objects.link(alvo)
        _apontar(cam, alvo)

        dados.dof.use_dof = p["dof"]
        dados.dof.focus_object = alvo
        dados.dof.aperture_fstop = p["f"]
        dados.dof.aperture_blades = 9

        cena.camera = cam
        return cam, alvo


    def _ajustar(objeto, nome, valor):
        # Atributo que mudou de nome ou sumiu entre versoes: tenta, nao derruba.
        try:
            setattr(objeto, nome, valor)
            return True
        except (AttributeError, TypeError, ValueError):
            return False


    def _bloom(cena, p):
        """Glare/bloom no compositor. Os nos mudam de nome e de forma entre versoes."""
        cena.use_nodes = True
        nt = cena.node_tree
        for no in list(nt.nodes):
            nt.nodes.remove(no)
        camadas = nt.nodes.new("CompositorNodeRLayers")
        camadas.location = (-400, 0)
        saida = nt.nodes.new("CompositorNodeComposite")
        saida.location = (400, 0)
        glare = nt.nodes.new("CompositorNodeGlare")
        glare.location = (0, 0)
        glare.glare_type = "BLOOM"
        # 4.2: propriedades no no. 4.5+: viraram sockets de entrada, e a mistura
        # saiu do no (ele devolve Glare e Image separados). Cobrimos os dois.
        if not _ajustar(glare, "threshold", p["limiar_bloom"]):
            _entrada(glare, "Threshold", p["limiar_bloom"])
        if not _ajustar(glare, "mix", p["mistura_bloom"]):
            # sem 'mix', a forca e o que dosa; -0,75 de mix equivale a ~0,25
            _entrada(glare, "Strength", (p["mistura_bloom"] + 1.0))
        if not _ajustar(glare, "size", p["tamanho_bloom"]):
            _entrada(glare, "Size", p["tamanho_bloom"] / 9.0)
        _ajustar(glare, "quality", "HIGH")
        nt.links.new(camadas.outputs["Image"], glare.inputs["Image"])
        nt.links.new(glare.outputs["Image"], saida.inputs["Image"])


    def configurar_render(cena, largura=1080, altura=1920, fps=30, amostras=64, params=None):
        """EEVEE Next + AgX + motion blur + raytracing + bloom. Devolve o dict de params usado."""
        p = _mesclar(PARAMS_RENDER_PADRAO, params)
        r = cena.render

        cena.unit_settings.system = "METRIC"
        cena.unit_settings.scale_length = 1.0
        r.fps = fps
        r.fps_base = 1.0
        r.resolution_x = largura
        r.resolution_y = altura
        r.resolution_percentage = 100

        try:
            r.engine = "BLENDER_EEVEE_NEXT"
        except TypeError:
            # Blender 5 renomeou o EEVEE Next de volta para BLENDER_EEVEE.
            r.engine = "BLENDER_EEVEE"
        ee = cena.eevee
        ee.taa_render_samples = amostras

        # Motion blur: no 4.2 o shutter mora em render; antes, em eevee.
        r.use_motion_blur = True
        if not _ajustar(r, "motion_blur_shutter", p["shutter"]):
            _ajustar(ee, "motion_blur_shutter", p["shutter"])

        # Raytracing (reflexo e refracao do vidro). Legado: SSR + SSR refraction.
        if not _ajustar(ee, "use_raytracing", True):
            _ajustar(ee, "use_ssr", True)
            _ajustar(ee, "use_ssr_refraction", True)
        try:
            ee.ray_tracing_options.use_denoise = True
            ee.ray_tracing_options.resolution_scale = "1"
        except AttributeError:
            pass

        # Sombras suaves: no Next sao sempre por area e o que se dosa e o numero
        # de raios/passos; no legado era um interruptor.
        if not _ajustar(ee, "use_shadows", True):
            _ajustar(ee, "use_soft_shadows", True)
        _ajustar(ee, "shadow_ray_count", 2)
        _ajustar(ee, "shadow_step_count", 4)
        _ajustar(ee, "use_shadow_jitter_viewport", True)

        # Cor: AgX, look de contraste medio-alto; filme opaco (o fundo E a cena).
        cena.view_settings.view_transform = "AgX"
        try:
            cena.view_settings.look = "AgX - Medium High Contrast"
        except TypeError:
            pass
        r.film_transparent = False
        # Dither maximo na saida: e a segunda metade do remedio contra banding.
        r.dither_intensity = 2.0

        if p["video"]:
            r.image_settings.file_format = "FFMPEG"
            r.image_settings.color_mode = "RGB"
            r.ffmpeg.format = "MPEG4"
            r.ffmpeg.codec = "H264"
            r.ffmpeg.constant_rate_factor = "HIGH"
            r.ffmpeg.ffmpeg_preset = "GOOD"
            r.ffmpeg.gopsize = fps
            _ajustar(r.ffmpeg, "audio_codec", "NONE")
        else:
            r.image_settings.file_format = "PNG"
            r.image_settings.color_mode = "RGB"
            r.image_settings.color_depth = "8"
            r.image_settings.compression = 50
        if p["caminho_saida"]:
            r.filepath = p["caminho_saida"]

        if p["bloom"]:
            try:
                _bloom(cena, p)
            except Exception as e:   # noqa: BLE001 - qualquer no ausente
                # Compositor sem saida ligada renderiza preto: melhor sem bloom
                # do que sem imagem.
                print("[ambiente] bloom desligado, compositor incompativel:", e)
                cena.use_nodes = False
        else:
            cena.use_nodes = False
        return p


    # ---------------------------------------------------------------- flash

    def _material_flash(forca):
        nome = NOME + ".flash"
        mat, nt, bsdf = _material(nome)
        for no in list(nt.nodes):
            nt.nodes.remove(no)
        saida = nt.nodes.new("ShaderNodeOutputMaterial")
        saida.location = (400, 0)
        mistura = nt.nodes.new("ShaderNodeMixShader")
        mistura.location = (200, 0)
        transparente = nt.nodes.new("ShaderNodeBsdfTransparent")
        transparente.location = (0, 100)
        emissao = nt.nodes.new("ShaderNodeEmission")
        emissao.location = (0, -100)
        emissao.inputs["Color"].default_value = (1.0, 1.0, 1.0, 1.0)
        emissao.inputs["Strength"].default_value = forca
        alfa = nt.nodes.new("ShaderNodeValue")
        alfa.name = alfa.label = "alfa"
        alfa.location = (0, 250)
        alfa.outputs[0].default_value = 0.0
        nt.links.new(alfa.outputs[0], mistura.inputs["Fac"])
        nt.links.new(transparente.outputs[0], mistura.inputs[1])
        nt.links.new(emissao.outputs[0], mistura.inputs[2])
        nt.links.new(mistura.outputs[0], saida.inputs["Surface"])
        # Blended: e um veu sobre a imagem, nao uma superficie que precisa de
        # profundidade nem de refracao. Em 4.1 e antes o nome era blend_method.
        try:
            mat.surface_render_method = "BLENDED"
        except AttributeError:
            mat.blend_method = "BLEND"
        _ajustar(mat, "shadow_method", "NONE")
        mat.use_backface_culling = False
        return mat, alfa


    def _plano_flash(objs, camera, forca, distancia):
        nome = NOME + ".flash"
        obj = objs.get("flash")
        if obj is None or obj.name not in bpy.data.objects:
            malha = bpy.data.meshes.new(nome)
            # 6 m a 0,25 m da camera cobre qualquer lente que o anuncio use;
            # a folga e o que mantem o veu uniforme quando o DoF o borra.
            s = 3.0
            malha.from_pydata([(-s, -s, 0), (s, -s, 0), (s, s, 0), (-s, s, 0)], [], [(0, 1, 2, 3)])
            malha.update()
            obj = bpy.data.objects.new(nome, malha)
            objs["colecao"].objects.link(obj)
            mat, alfa = _material_flash(forca)
            malha.materials.append(mat)
            # O veu nao pode iluminar nem sombrear a cena no quadro do flash: a luz
            # do flash e a que ja esta la, o veu so branqueia a imagem.
            for atributo in ("visible_shadow", "visible_diffuse", "visible_glossy",
                             "visible_transmission", "visible_volume_scatter"):
                _ajustar(obj, atributo, False)
            objs["flash"] = obj
        obj.parent = camera
        obj.matrix_parent_inverse.identity()
        obj.location = (0.0, 0.0, -distancia)
        obj.rotation_euler = (0.0, 0.0, 0.0)
        return obj


    def animar_flash(objs, camera, quadro, forca=1.0, largura=1):
        """Flash de foto: veu branco parented na camera, alfa 0 -> 1 -> 0 em 3 quadros.

        'forca' multiplica a emissao (1.0 = branco estourado com AgX); 'largura' e
        quantos quadros de cada lado do pico (1 = os 3 quadros pedidos).
        """
        p = objs.get("params", PARAMS_PADRAO)
        obj = _plano_flash(objs, camera, p["forca_flash"] * forca, p["distancia_flash"])
        mat = obj.data.materials[0]
        nt = mat.node_tree
        alfa = nt.nodes["alfa"].outputs[0]
        emissao = next(n for n in nt.nodes if n.type == "EMISSION").inputs["Strength"]
        # Forca por flash: a coreografia pode pedir um flash mais fraco no beat 5
        # sem trocar o material.
        for q, a in ((quadro - largura, 0.0), (quadro, 1.0), (quadro + largura, 0.0)):
            alfa.default_value = a
            alfa.keyframe_insert("default_value", frame=q)
            emissao.default_value = p["forca_flash"] * forca
            emissao.keyframe_insert("default_value", frame=q)
        # Linear e nao Bezier: com so tres chaves, o Bezier passaria de 1.0 no
        # pico e daria um quadro extra meio branco de cada lado.
        acao = nt.animation_data.action if nt.animation_data else None
        if acao is not None:
            for fc in acao.fcurves:
                for kp in fc.keyframe_points:
                    kp.interpolation = "LINEAR"
        alfa.default_value = 0.0
        return obj


    def animar_rig(objs, quadro_ini, quadro_fim, angulo_ini, angulo_fim, easing="EASE_IN_OUT"):
        """Gira o rig das luzes em Z (graus), para o rim acompanhar a orbita da camera."""
        rig = objs["rig"]
        for q, ang in ((quadro_ini, angulo_ini), (quadro_fim, angulo_fim)):
            rig.rotation_euler = (0.0, 0.0, math.radians(ang))
            rig.keyframe_insert("rotation_euler", index=2, frame=q)
        acao = rig.animation_data.action
        for fc in acao.fcurves:
            for kp in fc.keyframe_points:
                kp.interpolation = "BEZIER"
                kp.easing = easing
    return locals()


mod_ambiente = _registrar_modulo('mod_ambiente', _modulo_ambiente())


# ============================================================================
# MODULO mod_caixa (scripts/mod_caixa.py), inteiro, dentro de uma funcao-namespace
# ============================================================================
def _modulo_caixa():
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

    import math
    import random

    import bmesh
    import bpy
    from mathutils import Vector, noise

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
        # O AgX empalidece cor saturada sob luz forte: o laranja da logo virava
        # pessego na previa. Compensacao leve na tinta, nao no papel.
        "saturacao_logo": 1.3,
        "n_espumas": 64,
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
            bpy.data.objects.remove(obj, do_unlink=True)
            # Malha orfa fica no arquivo ate o proximo salvar/recarregar;
            # apagar aqui evita acumular 'caixa.corpo.001' nas rodadas seguintes.
            if dados is not None and dados.users == 0:
                if isinstance(dados, bpy.types.Mesh):
                    bpy.data.meshes.remove(dados)
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
        raiz = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
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
        """Floco de espuma: icosfera amassada por ruido coerente e achatada de
        forma desigual. E o que faz parecer floco e nao bolinha."""
        bm = bmesh.new()
        bmesh.ops.create_icosphere(bm, subdivisions=2, radius=1.0)
        desloc = Vector((rng.uniform(-50, 50), rng.uniform(-50, 50), rng.uniform(-50, 50)))
        freq = rng.uniform(1.2, 2.0)
        amp = rng.uniform(0.3, 0.5)
        # Alongado num eixo e com uma curva leve: o floco de embalagem e um
        # "amendoim" torto, e seixo redondo e o que a icosfera daria sozinha.
        escala = Vector((rng.uniform(1.3, 2.1), rng.uniform(0.6, 0.95), rng.uniform(0.55, 0.9)))
        curva = rng.uniform(-0.45, 0.45)
        cintura = rng.uniform(0.0, 0.35)
        for v in bm.verts:
            n = noise.noise(v.co * freq + desloc)
            n2 = noise.noise(v.co * 5.0 + desloc) * 0.1
            fator = 1.0 + amp * n + n2
            x, y, z = v.co.x * escala.x, v.co.y * escala.y, v.co.z * escala.z
            # cintura no meio (amendoim) e curvatura ao longo do eixo maior
            aperto = 1.0 - cintura * math.exp(-(x / (0.45 * escala.x)) ** 2)
            y *= aperto
            z *= aperto
            z += curva * x * x
            v.co = Vector((x, y, z)) * fator
        # Normaliza pelo maior raio real: "floco de 3 a 6 cm" e a maior dimensao
        # do floco deformado, nao o raio da icosfera de partida. Sem isto os
        # alongados furavam o topo da tampa.
        maior = max(v.co.length for v in bm.verts)
        for v in bm.verts:
            v.co *= raio / maior
        malha = bpy.data.meshes.new(nome)
        bm.to_mesh(malha)
        bm.free()
        _sombrear_suave(malha)
        return malha


    # ---------------------------------------------------------------- materiais

    def _material_papel(nome, cor_hex, imagem=None, escala_logo=None, z_topo_local=None,
                        centro_uv=(0.5, 0.5), saturacao=1.0):
        """Papel fosco com toque de grao. Se houver imagem, ela entra como tinta
        impressa: mistura pela alfa na Base Color e muda a rugosidade, sem relevo.
        A projecao e plana no espaco local do objeto (coordenada Object), so na
        face de cima (z local acima de z_topo_local)."""
        mat = bpy.data.materials.new(nome)
        mat.use_nodes = True
        nt = mat.node_tree
        for n in list(nt.nodes):
            nt.nodes.remove(n)
        saida = nt.nodes.new("ShaderNodeOutputMaterial")
        bsdf = nt.nodes.new("ShaderNodeBsdfPrincipled")
        saida.location = (600, 0)
        bsdf.location = (300, 0)
        nt.links.new(bsdf.outputs["BSDF"], saida.inputs["Surface"])

        cor = cor_linear(cor_hex)
        bsdf.inputs["Base Color"].default_value = cor
        bsdf.inputs["Roughness"].default_value = 0.78
        bsdf.inputs["Specular IOR Level"].default_value = 0.35
        try:
            # Sheen e o que da o "toque de papel" na luz rasante. So existe no
            # Principled v2 (4.0+); em versoes velhas o nome era "Sheen".
            bsdf.inputs["Sheen Weight"].default_value = 0.35
            bsdf.inputs["Sheen Roughness"].default_value = 0.6
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
        hsv = nt.nodes.new("ShaderNodeHueSaturation")
        hsv.location = (-150, 250)
        hsv.inputs["Saturation"].default_value = saturacao
        nt.links.new(tex.outputs["Color"], hsv.inputs["Color"])
        nt.links.new(hsv.outputs["Color"], mix_cor.inputs["Color2"])
        nt.links.new(mix_cor.outputs["Color"], bsdf.inputs["Base Color"])

        # Tinta e um pouco mais acetinada que o papel: e o que faz "impresso" em
        # vez de "colado".
        mix_rug = nt.nodes.new("ShaderNodeMath")
        mix_rug.location = (50, 50)
        mix_rug.operation = "MULTIPLY_ADD"   # rug = 0.78 + mascara * (0.55 - 0.78)
        nt.links.new(mascara.outputs["Value"], mix_rug.inputs[0])
        mix_rug.inputs[1].default_value = 0.55 - 0.78
        mix_rug.inputs[2].default_value = 0.78
        nt.links.new(mix_rug.outputs["Value"], bsdf.inputs["Roughness"])
        return mat


    def _material_espuma(nome):
        mat = bpy.data.materials.new(nome)
        mat.use_nodes = True
        bsdf = mat.node_tree.nodes.get("Principled BSDF")
        bsdf.inputs["Base Color"].default_value = cor_linear(COR_ESPUMA)
        bsdf.inputs["Roughness"].default_value = 0.92
        bsdf.inputs["Specular IOR Level"].default_value = 0.25
        try:
            # Um pouco de subsuperficie tira o "gesso" da espuma branca.
            bsdf.inputs["Subsurface Weight"].default_value = 0.1
            bsdf.inputs["Subsurface Radius"].default_value = (0.01, 0.01, 0.01)
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
                            h_tampa / 2.0, centro_uv, p["saturacao_logo"])
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
        for i in range(n):
            raio = rng.uniform(0.015, 0.03)          # floco de 3 a 6 cm
            malha = _malha_espuma("caixa.espuma.%03d" % (i + 1), rng, raio)
            obj = bpy.data.objects.new("caixa.espuma.%03d" % (i + 1), malha)
            obj.data.materials.append(mat_espuma)
            # Subdivisao leve por cima do ruido: tira as quinas da icosfera.
            sub = obj.modifiers.new("suave", "SUBSURF")
            sub.levels = 1
            sub.render_levels = 1
            # Onde fica: a maior parte por cima do U1 (e de la que voa quando a
            # tampa abre); o resto nos vaos laterais entre o U1 e a parede.
            if rng.random() < 0.62 or raio > 0.02:
                # Camada de cima, entre o topo do U1 e o topo interno da tampa.
                x = rng.uniform(-ix / 2 + raio, ix / 2 - raio)
                y = rng.uniform(-iy / 2 + raio, iy / 2 - raio)
                z_min = e + uz + raio * 0.7
                z_max = e + iz - raio
                z = rng.uniform(z_min, max(z_min, z_max))
            else:
                # Vaos laterais: so os pequenos, porque o vao tem menos de 4 cm.
                lado = rng.choice(("x", "y"))
                sinal = rng.choice((-1.0, 1.0))
                if lado == "x":
                    x = sinal * (ux / 2 + (ix / 2 - ux / 2) / 2.0)
                    y = rng.uniform(-iy / 2 + raio, iy / 2 - raio)
                else:
                    x = rng.uniform(-ix / 2 + raio, ix / 2 - raio)
                    y = sinal * (uy / 2 + (iy / 2 - uy / 2) / 2.0)
                z = rng.uniform(e + 0.25, e + uz)
            obj.location = (x, y, z)
            obj.rotation_euler = (rng.uniform(0, math.tau), rng.uniform(0, math.tau), rng.uniform(0, math.tau))
            # Repouso guardado no objeto: as animacoes partem daqui e voltam para
            # ca, independentemente do estado em que a cena estiver.
            obj["caixa_repouso"] = list(obj.location)
            obj["caixa_rot_repouso"] = list(obj.rotation_euler)
            obj["caixa_raio"] = raio
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
        }


    # ---------------------------------------------------------------- animacao

    def _suavizar(obj, q_ini, q_fim, easing, canais=None, interpolacao="BEZIER"):
        """Deixa Bezier + easing em todas as chaves do intervalo. So mexe nas
        chaves deste intervalo para nao alterar animacao de outro beat."""
        ad = obj.animation_data
        if ad is None or ad.action is None:
            return
        for fc in ad.action.fcurves:
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
        pontos = []
        passos = 16
        for k in range(passos + 1):
            u = k / passos
            t = u * T
            # Horizontal comeca devagar e acelera: a espuma sobe quase reta ao sair
            # da caixa e so depois abre o arco - e o que evita atravessar a parede.
            w = u * (0.45 + 0.55 * u)
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
    return locals()


mod_caixa = _registrar_modulo('mod_caixa', _modulo_caixa())


# ============================================================================
# MODULO mod_u1 (scripts/mod_u1.py), inteiro, dentro de uma funcao-namespace
# ============================================================================
def _modulo_u1():
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

    import math
    import os

    import bpy
    import bmesh
    from mathutils import Vector, Matrix

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


    def _mat_fibra_carbono(nome):
        """Sarja de fibra de carbono procedural: duas ondas cruzadas, resina brilhante por cima."""
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
        _entrada(bsdf, "Roughness", 0.28)
        _entrada(bsdf, "Metallic", 0.15)
        _entrada(bsdf, "Coat Weight", 1.0)
        _entrada(bsdf, "Coat Roughness", 0.08)
        _entrada(bsdf, "Anisotropic", 0.6)
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


    def _mat_tela(nome, img_boot, img_ui, barra_px=(90, 390, 273, 276), cor_trilho="#2A2A2E"):
        """Vidro preto brilhante que vira tela: emissao = imagem, forca comeca em 0.

        Nos nomeados 'ligada', 'mistura' e 'progresso' sao o que animar_tela chaveia:
        - ligada:    0 desligada (vidro preto), 1 acesa
        - mistura:   0 imagem de boot, 1 interface (corte seco em q_ui_ini)
        - progresso: 0..1 preenche a barra do boot por mascara em UV
        A imagem ocupa so a area ativa de 3,5"; a moldura em volta fica preta
        pela extensao CLIP do Image Texture.
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
        nt.links.new(troca.outputs["Color"], bsdf.inputs["Emission Color"])

        ligada = nt.nodes.new("ShaderNodeValue")
        ligada.name = "ligada"
        ligada.label = "ligada"
        ligada.outputs[0].default_value = 0.0
        ligada.location = (-250, -250)
        # Tela real e bem mais clara que o vidro em volta: 4x para AgX nao apagar.
        forca = _math("MULTIPLY", (ligada, 0), 4.0, (-50, -250))
        nt.links.new(forca.outputs["Value"], bsdf.inputs["Emission Strength"])
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
        m_fibra = _mat_fibra_carbono("u1.fibra_carbono")
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

        img_boot, ph_boot = _carregar_imagem(p["imagem_boot"], "u1.tela_boot", cor=(0, 0, 0, 1))
        img_ui, ph_ui = _carregar_imagem(p["imagem_ui"], "u1.tela_ui", cor=(0.09, 0.09, 0.1, 1))
        m_tela = _mat_tela("u1.tela", img_boot, img_ui, tuple(p["barra_boot_px"]), p["cor_trilho_boot"])

        # Raiz: a coreografia move isto e o U1 inteiro vai junto.
        raiz = bpy.data.objects.new("u1.raiz", None)
        raiz.empty_display_type = "ARROWS"
        raiz.empty_display_size = 0.2
        col.objects.link(raiz)

        # --- Casco branco ------------------------------------------------------
        # Casco de 4 mm acima do chao (pes) ate 2 mm abaixo do topo (aro): assim
        # os pes ficam sobre z = 0 e o aro fecha o envelope em ALTURA.
        corpo = _caixa("u1.corpo", col, (L, P, A - 0.006), (0, 0, (A + 0.002) / 2), m_painel,
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
        # Quebra de quina de 1,5 mm nas arestas que os booleans criaram.
        _chanfro(corpo, 0.0015, 2, nome="quina")

        # Aro preto do topo, com abertura um pouco menor que o bolso (faz o labio).
        aro = _caixa("u1.aro", col, (L - 0.02, P - 0.02, 0.024), (0, 0, A - 0.012), m_preto,
                     chanfro=0.02, segmentos=6, pai=raiz, peso=(1.0, 0.12), suave=False)
        _cortador("u1.cortador.aro", col, (L - 0.10, P - 0.12, 0.1), (0, 0, A), aro, raiz)
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
        _caixa("u1.camara.parede.tras.cima", col, (cx, 0.012, A - 0.58 - 0.05), (0, cy / 2 - 0.006, 0.58 + (A - 0.58 - 0.05) / 2), m_camara, chanfro=0, pai=raiz)
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

        # LEDs no teto da camara, na frente.
        leds = []
        for i, x in enumerate((-0.11, 0.11)):
            leds.append(_caixa("u1.led.%d" % (i + 1), col, (0.16, 0.012, 0.003), (x, -cy / 2 + 0.03, A - 0.032), m_led, chanfro=0.0005, segmentos=1, pai=raiz))

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
            "materiais": {"tela": m_tela, "led": m_led, "botao": m_botao},
        }


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
        for fc in anim.action.fcurves:
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
        else:
            # Material do cliente: primeiro socket de forca de emissao que existir.
            for no in nt.nodes:
                s = no.inputs.get("Emission Strength") if no.type == "BSDF_PRINCIPLED" else (no.inputs.get("Strength") if no.type == "EMISSION" else None)
                if s is not None:
                    _chave_socket(s, 0.0, q_boot_ini)
                    _chave_socket(s, 4.0, q_boot_ini + duracao_fade)
                    break
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
            for fc in anim.action.fcurves:
                if "mistura" in fc.data_path:
                    for kp in fc.keyframe_points:
                        kp.interpolation = "CONSTANT"


    def animar_botao(objs, q_ini, q_fim, easing="EASE_IN_OUT", profundidade=0.002):
        """Botao afunda 2 mm e volta; LEDs da camara e a janela do botao acendem."""
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

        # LED: usa os materiais devolvidos; com modelo do cliente, procura
        # 'led' no dict e chaveia o que tiver forca de emissao.
        mats = objs.get("materiais", {})
        alvos = [m for m in (mats.get("led"), mats.get("botao")) if m is not None]
        if not alvos and objs.get("led") is not None and objs["led"].active_material:
            alvos = [objs["led"].active_material]
        for m in alvos:
            if not m.use_nodes:
                continue
            for no in m.node_tree.nodes:
                s = no.inputs.get("Emission Strength") if no.type == "BSDF_PRINCIPLED" else (no.inputs.get("Strength") if no.type == "EMISSION" else None)
                if s is None:
                    continue
                forca = 12.0 if m.name == "u1.led" else 3.0
                _chave_socket(s, 0.0, meio)
                _chave_socket(s, forca, q_fim)
                _suavizar_fcurves(m.node_tree.animation_data, {meio, q_fim}, easing)
                break
    return locals()


mod_u1 = _registrar_modulo('mod_u1', _modulo_u1())


# ============================================================================
# MODULO mod_cabo (scripts/mod_cabo.py), inteiro, dentro de uma funcao-namespace
# ============================================================================
def _modulo_cabo():
    # Modulo CABO do anuncio do Snapmaker U1.
    #
    # Cabo de energia preto (borracha) como curva Bezier com bevel redondo de
    # 8 mm, e um plugue IEC C13 modelado: bico que entra na tomada, corpo moldado
    # com chanfro, friso, alivio de tensao com nervuras e tres contatos metalicos
    # nas janelas do bico. So definicoes aqui - nada roda no import. Quem integra
    # e mod_coreografia.py; quem prova este modulo sozinho e teste_cabo.py.
    #
    # DECISOES:
    #
    # - O C13 e a ponta FEMEA do cabo (os pinos macho ficam no C14 da maquina, e o
    #   mod_u1 ja os tem dentro da tomada). Por isso os "3 pinos metalicos" aqui
    #   sao os contatos de latao vistos dentro das tres janelas do bico, recuados
    #   1,5 mm - e o que se ve olhando a cara de um C13 de verdade. Se um dia se
    #   quiser pinos salientes, e o parametro 'saliencia_contatos'.
    #
    # - A origem do Empty 'cabo.plugue' e o CENTRO DA CARA do bico, e o eixo local
    #   +Y e a direcao em que o plugue entra. Encaixado, o Empty esta exatamente
    #   em ponto_tomada com +Y = direcao_entrada: quem integra nao precisa saber
    #   medida nenhuma do plugue.
    #
    # - O cabo acompanha o plugue por KEYFRAMES NOS PONTOS DA CURVA (co e handles),
    #   um por quadro, e nao por hooks: hook depende de objeto auxiliar e de
    #   modificador, e o ponto animado direto e o que sobrevive a qualquer versao
    #   do Blender e a qualquer cena. Como o plugue tambem recebe uma chave por
    #   quadro, calculada pela mesma funcao, cabo e plugue nunca se separam - o que
    #   aconteceria se o plugue fosse interpolado pelo Blender e o cabo por aqui.
    #   O easing entra na PARAMETRIZACAO do arco, nao na chave.
    #
    # - Depois de conectado o cabo sai reto do alivio de tensao por uns
    #   centimetros (o alivio e rigido), cai numa curva de gravidade ate o chao e
    #   segue pelo chao para fora do quadro ('ponto_fora'). Nao ha tomada na
    #   parede: o cabo vem de fora da cena.
    #
    # ACHADO NA PREVIA (EEVEE Next 4.2), para quem monta o render final: em
    # close (85 mm a 20 cm) o chanfro do corpo do plugue sai CHUVISCADO onde a
    # luz bate rasante - acne de sombra, nao ruido de amostragem. Medido em
    # variantes separadas: 48 amostras nao resolve; desligar o jitter PIORA
    # (o chanfro inteiro chuvisca); shadow_maximum_resolution=0,0002 e
    # shadow_filter_radius=2 nas luzes nao resolvem; o que resolve sozinho e
    # cena.eevee.shadow_ray_count = 4 e shadow_step_count = 8 (padrao 1 e 6).
    # E ajuste de cena, nao de material: o teste daqui liga, o modulo ambiente
    # precisa ligar no render final.
    #
    # Eixos e medidas seguem docs/ESPECIFICACAO.md: metros, Z para cima, frente
    # em -Y (a traseira do U1 e a tomada ficam em +Y), chao em z = 0.

    import math

    import bmesh
    import bpy
    from mathutils import Matrix, Vector

    NOME = "cabo"

    PARAMS_PADRAO = {
        # Tomada do U1 substituto (mod_u1: coluna em x=0,245, face da coluna em
        # y=0,2575, aro 3,5 mm a frente, 0,125 m do chao). A coreografia passa a
        # de verdade; isto e so para o modulo construir sozinho.
        "ponto_tomada": (0.245, 0.261, 0.125),
        "direcao_entrada": (0.0, -1.0, 0.0),
        # Onde o cabo sai do quadro; None = calculado atras da tomada, no chao.
        "ponto_fora": None,
        "z_chao": 0.0,
        "diametro_cabo": 0.008,
        "cor_cabo": "#111111",
        "rugosidade_cabo": 0.5,
        # O plastico moldado do plugue e um pouco mais lustroso que a borracha.
        "rugosidade_plugue": 0.38,
        "cor_contato": (0.80, 0.56, 0.24),
        "saliencia_contatos": 0.0,     # >0 faz os contatos sairem da cara (mm em m)
        # Quanto a cara do bico entra alem de ponto_tomada. 0 = so encosta, que e
        # o certo quando a tomada do modelo e uma face plana; num C14 com bolso
        # pode ser ate 0,006.
        "penetracao": 0.0,
        "resolucao_bevel": 6,
        "resolucao_curva": 24,
        # Lado do qual o cabo vem e para onde sai: +1 = para o lado de
        # (normal x Z), -1 = o oposto.
        "lado": 1.0,
    }

    FPS = 30.0

    # Medidas do C13 (chutadas em cima de plugues de mesa; norma IEC 60320 so fixa
    # a cara e os pinos). Cara em y = 0, tudo cresce para -Y.
    BICO = (0.0225, 0.0158, 0.0040, 0.0, -0.0075)      # largura, altura, chanfro, y_frente, y_tras
    CORPO = (0.0300, 0.0215, 0.0065, -0.0075, -0.0430)
    FRISO = (0.0312, 0.0227, 0.0072, -0.0270, -0.0300)
    # Janelas dos contatos: mesmas posicoes dos pinos do C14 do mod_u1 (14 mm
    # entre fase e neutro, terra 5 mm acima do centro).
    JANELAS = ((-0.007, -0.001), (0.007, -0.001), (0.0, 0.004))
    JANELA_DIMS = (0.0024, 0.012, 0.0050)      # x, y (profundidade do corte), z
    CONTATO_DIMS = (0.0016, 0.0040, 0.0042)
    RECUO_CONTATO = 0.0015
    # Alivio de tensao: (y, raio) do corpo ate a saida do cabo. Nervuras
    # alternadas e afinando - e isso que faz ler como plugue moldado e nao como
    # um cilindro colado num bloco.
    ALIVIO = (
        (-0.0410, 0.0092), (-0.0440, 0.0080), (-0.0460, 0.0070),
        (-0.0480, 0.0068), (-0.0500, 0.0057), (-0.0520, 0.0066), (-0.0540, 0.0055),
        (-0.0560, 0.0063), (-0.0580, 0.0053), (-0.0600, 0.0060), (-0.0620, 0.0051),
        (-0.0640, 0.0057), (-0.0660, 0.0049), (-0.0680, 0.0053), (-0.0700, 0.0044),
    )
    COMPRIMENTO_PLUGUE = 0.070     # da cara ate onde o cabo sai do alivio
    SEGMENTOS_ALIVIO = 24


    # ---------------------------------------------------------------- utilidades

    def _srgb_para_linear(c):
        c = c / 255.0
        return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4


    def cor_linear(hexstr):
        """'#RRGGBB' -> RGBA linear (o Principled quer linear; a paleta e sRGB)."""
        h = hexstr.lstrip("#")
        return tuple(_srgb_para_linear(int(h[i:i + 2], 16)) for i in (0, 2, 4)) + (1.0,)


    def limpar_colecao(nome):
        """Remove a sub-colecao <nome> e tudo o que ela contem (idempotencia)."""
        col = bpy.data.collections.get(nome)
        if col is None:
            return
        for obj in list(col.all_objects):
            dados = obj.data
            bpy.data.objects.remove(obj, do_unlink=True)
            # Dado orfao fica ate salvar/recarregar; apagar aqui evita acumular
            # 'cabo.curva.001' nas rodadas seguintes.
            if dados is not None and dados.users == 0:
                if isinstance(dados, bpy.types.Mesh):
                    bpy.data.meshes.remove(dados)
                elif isinstance(dados, bpy.types.Curve):
                    bpy.data.curves.remove(dados)
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


    def _material(nome, cor, rugosidade, metalico=0.0):
        mat = bpy.data.materials.get(nome)
        if mat is None:
            mat = bpy.data.materials.new(nome)
        mat.use_nodes = True
        b = mat.node_tree.nodes.get("Principled BSDF")
        b.inputs["Base Color"].default_value = cor
        b.inputs["Roughness"].default_value = rugosidade
        b.inputs["Metallic"].default_value = metalico
        return mat


    def _sombrear_suave(malha, suave=True):
        malha.polygons.foreach_set("use_smooth", [suave] * len(malha.polygons))
        malha.update()


    def _novo_objeto(nome, malha, col, pai, mat=None, pos=(0, 0, 0)):
        obj = bpy.data.objects.new(nome, malha)
        obj.location = pos
        if mat is not None and malha is not None:
            malha.materials.append(mat)
        col.objects.link(obj)
        if pai is not None:
            obj.parent = pai
        return obj


    def _chanfro(obj, largura, segmentos, nome="chanfro"):
        mod = obj.modifiers.new(nome, "BEVEL")
        mod.width = largura
        mod.segments = segmentos
        mod.limit_method = "ANGLE"
        mod.angle_limit = math.radians(50.0)
        try:
            # Sem isto as faces planas do corpo ganham gradiente de sombreamento.
            mod.harden_normals = True
        except AttributeError:
            pass
        return mod


    # ---------------------------------------------------------------- geometria

    def _perfil_c13(largura, altura, chanfro):
        """Contorno do C13 no plano XZ local: retangulo com os dois cantos DE
        CIMA chanfrados (o terra fica em cima). Sentido anti-horario visto de +Y."""
        w, h, c = largura / 2.0, altura / 2.0, chanfro
        return [(-w, -h), (w, -h), (w, h - c), (w - c, h), (-w + c, h), (-w, h - c)]


    def _bloco_c13(nome, col, pai, mat, largura, altura, chanfro, y_frente, y_tras):
        """Prisma do perfil C13 entre y_tras e y_frente, com tampas."""
        perfil = _perfil_c13(largura, altura, chanfro)
        bm = bmesh.new()
        anel_f = [bm.verts.new((x, y_frente, z)) for x, z in perfil]
        anel_t = [bm.verts.new((x, y_tras, z)) for x, z in perfil]
        n = len(perfil)
        for i in range(n):
            j = (i + 1) % n
            bm.faces.new((anel_t[i], anel_t[j], anel_f[j], anel_f[i]))
        bm.faces.new(anel_f)
        bm.faces.new(list(reversed(anel_t)))
        bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
        malha = bpy.data.meshes.new(nome)
        bm.to_mesh(malha)
        bm.free()
        return _novo_objeto(nome, malha, col, pai, mat)


    def _caixa(nome, col, pai, mat, dims, pos):
        bm = bmesh.new()
        bmesh.ops.create_cube(bm, size=1.0)
        bmesh.ops.scale(bm, vec=dims, verts=bm.verts)
        malha = bpy.data.meshes.new(nome)
        bm.to_mesh(malha)
        bm.free()
        return _novo_objeto(nome, malha, col, pai, mat, pos)


    def _cortador(nome, col, pai, dims, pos, alvo):
        """Cubo invisivel que recorta 'alvo' por boolean; fica na colecao para o
        limpar_colecao achar."""
        obj = _caixa(nome, col, pai, None, dims, pos)
        obj.hide_render = True
        obj.hide_viewport = True
        obj.display_type = "WIRE"
        mod = alvo.modifiers.new("corte_" + nome.split(".")[-1], "BOOLEAN")
        mod.operation = "DIFFERENCE"
        mod.object = obj
        try:
            mod.solver = "EXACT"
        except TypeError:
            pass
        return obj


    def _alivio(nome, col, pai, mat):
        """Alivio de tensao: aneis de raio variavel ao longo de -Y, com tampas."""
        bm = bmesh.new()
        aneis = []
        for y, r in ALIVIO:
            anel = []
            for k in range(SEGMENTOS_ALIVIO):
                a = 2.0 * math.pi * k / SEGMENTOS_ALIVIO
                anel.append(bm.verts.new((r * math.cos(a), y, r * math.sin(a))))
            aneis.append(anel)
        n = SEGMENTOS_ALIVIO
        for a0, a1 in zip(aneis[:-1], aneis[1:]):
            for i in range(n):
                j = (i + 1) % n
                bm.faces.new((a0[i], a0[j], a1[j], a1[i]))
        bm.faces.new(aneis[0])
        bm.faces.new(list(reversed(aneis[-1])))
        bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
        malha = bpy.data.meshes.new(nome)
        bm.to_mesh(malha)
        bm.free()
        _sombrear_suave(malha)
        obj = _novo_objeto(nome, malha, col, pai, mat)
        # Suavizar so as nervuras (angulo baixo), mantendo as tampas planas.
        try:
            malha.set_sharp_from_angle(angle=math.radians(40.0))    # 4.1+
        except AttributeError:
            try:
                malha.use_auto_smooth = True                        # ate 4.0
                malha.auto_smooth_angle = math.radians(40.0)
            except AttributeError:
                pass
        return obj


    def _construir_plugue(col, p, m_plugue, m_contato):
        raiz = bpy.data.objects.new("cabo.plugue", None)
        raiz.empty_display_type = "ARROWS"
        raiz.empty_display_size = 0.02
        raiz.rotation_mode = "QUATERNION"
        col.objects.link(raiz)

        partes = {}
        bico = _bloco_c13("cabo.plugue.bico", col, raiz, m_plugue, *BICO)
        _chanfro(bico, 0.0007, 2)
        # Janelas dos contatos: cortadas DEPOIS do chanfro na pilha, para a
        # borda da janela ficar viva como no plastico moldado.
        for i, (px, pz) in enumerate(JANELAS):
            _cortador("cabo.cortador.janela.%d" % (i + 1), col, raiz, JANELA_DIMS,
                      (px, BICO[3] - JANELA_DIMS[1] / 2.0 + 0.006, pz), bico)
        partes["bico"] = bico

        corpo = _bloco_c13("cabo.plugue.corpo", col, raiz, m_plugue, *CORPO)
        _chanfro(corpo, 0.0022, 4)
        partes["corpo"] = corpo
        friso = _bloco_c13("cabo.plugue.friso", col, raiz, m_plugue, *FRISO)
        _chanfro(friso, 0.0006, 2)
        partes["friso"] = friso
        partes["alivio"] = _alivio("cabo.plugue.alivio", col, raiz, m_plugue)

        contatos = []
        for i, (px, pz) in enumerate(JANELAS):
            y = BICO[3] - RECUO_CONTATO - CONTATO_DIMS[1] / 2.0 + p["saliencia_contatos"]
            c = _caixa("cabo.plugue.contato.%d" % (i + 1), col, raiz, m_contato, CONTATO_DIMS, (px, y, pz))
            _chanfro(c, 0.0003, 1)
            contatos.append(c)
        partes["contatos"] = contatos
        return raiz, partes


    def _construir_curva(col, p, m_cabo):
        cd = bpy.data.curves.new("cabo.curva", "CURVE")
        cd.dimensions = "3D"
        cd.bevel_depth = p["diametro_cabo"] / 2.0
        cd.bevel_resolution = p["resolucao_bevel"]
        cd.fill_mode = "FULL"
        cd.use_fill_caps = True
        cd.resolution_u = p["resolucao_curva"]
        sp = cd.splines.new("BEZIER")
        sp.bezier_points.add(3)          # 4 pontos: saida, queda, meio do chao, fora
        for bp in sp.bezier_points:
            bp.handle_left_type = "FREE"
            bp.handle_right_type = "FREE"
        sp.use_smooth = True
        cd.materials.append(m_cabo)
        obj = bpy.data.objects.new("cabo.curva", cd)
        col.objects.link(obj)
        return obj


    # ---------------------------------------------------------------- pose

    def _lateral(normal, lado):
        lat = normal.cross(Vector((0, 0, 1)))
        if lat.length < 1e-6:
            # Tomada virada para cima/baixo: qualquer horizontal serve.
            lat = Vector((1, 0, 0))
        return lat.normalized() * lado


    def _quat_eixo_y(eixo_y):
        """Rotacao que leva o +Y local do plugue para eixo_y, com o Z para cima
        (sem rolagem: um plugue de mesa voa 'em pe')."""
        eixo_y = eixo_y.normalized()
        if abs(eixo_y.z) > 0.999:
            return eixo_y.to_track_quat("Y", "X")
        return eixo_y.to_track_quat("Y", "Z")


    def _matriz_plugue(pos, quat):
        return Matrix.Translation(pos) @ quat.to_matrix().to_4x4()


    def _pontos_cabo(m_plugue, ponto_fora, z_chao, raio, lateral):
        """Os 4 pontos Bezier do cabo para uma pose do plugue: saida do alivio,
        queda no chao, meio do caminho pelo chao, saida do quadro.
        Devolve [(co, handle_esq, handle_dir), ...]."""
        frente = (m_plugue.to_3x3() @ Vector((0, 1, 0))).normalized()
        tras = -frente
        saida = m_plugue @ Vector((0, -COMPRIMENTO_PLUGUE, 0))
        z_cabo = z_chao + raio
        h = max(0.0, saida.z - z_cabo)

        dh = Vector((ponto_fora.x - saida.x, ponto_fora.y - saida.y, 0.0))
        if dh.length < 1e-6:
            dh = Vector((tras.x, tras.y, 0.0))
        dh.normalize()

        # Onde o cabo toca o chao: um cabo de 8 mm pendurado de h metros chega ao
        # chao a ~1,25 h de distancia, mais o trecho reto que o alivio impoe
        # (com 1,4 h a queda saia como uma diagonal dura na previa de lado).
        d1 = 0.14 + 1.25 * h
        p0 = saida
        h0 = saida + tras * min(0.08, 0.4 * d1)
        h0.z = max(h0.z, z_cabo)          # o cabo nao entra no chao ao decolar
        p1 = saida + dh * d1
        p1.z = z_cabo
        p3 = Vector(ponto_fora)
        p3.z = z_cabo
        reta = p3 - p1
        # Ondulacao lateral suave pelo chao: cabo jogado no chao nunca e reto.
        p2 = p1 + reta * 0.5 + lateral * 0.10
        dir13 = reta.normalized() if reta.length > 1e-6 else dh

        return [
            (p0, p0 - tras * 0.02, h0),
            (p1, p1 - dh * 0.5 * d1, p1 + dir13 * reta.length * 0.22),
            (p2, p2 - dir13 * reta.length * 0.25, p2 + dir13 * reta.length * 0.25),
            (p3, p3 - dir13 * reta.length * 0.30, p3 + dir13 * 0.2),
        ]


    def _aplicar_pose(objs, pos, quat, quadro=None):
        """Poe plugue e cabo numa pose; com 'quadro', grava a chave."""
        plugue = objs["plugue"]
        plugue.location = pos
        plugue.rotation_quaternion = quat
        m = _matriz_plugue(pos, quat)
        pontos = _pontos_cabo(m, objs["ponto_fora"], objs["z_chao"], objs["raio"], objs["lateral"])
        cd = objs["curva"].data
        bps = cd.splines[0].bezier_points
        for i, (co, he, hd) in enumerate(pontos):
            bps[i].co = co
            bps[i].handle_left = he
            bps[i].handle_right = hd
        if quadro is None:
            return
        plugue.keyframe_insert("location", frame=quadro)
        plugue.keyframe_insert("rotation_quaternion", frame=quadro)
        for i in range(len(pontos)):
            base = "splines[0].bezier_points[%d]." % i
            cd.keyframe_insert(base + "co", frame=quadro)
            cd.keyframe_insert(base + "handle_left", frame=quadro)
            cd.keyframe_insert(base + "handle_right", frame=quadro)


    def _ponto_fora_padrao(ponto, normal, lateral, z_chao, raio):
        p = ponto + normal * 1.7 + lateral * 0.5
        p.z = z_chao + raio
        return p


    # ---------------------------------------------------------------- API

    def construir_cabo(cena, colecao_pai, params=None):
        """Cria plugue C13 + cabo na sub-colecao 'cabo', encaixados em
        params['ponto_tomada'] com o cabo em repouso. Devolve referencias e
        medidas."""
        p = dict(PARAMS_PADRAO)
        if params:
            p.update(params)
        limpar_colecao(NOME)
        col = _colecao(cena, colecao_pai, NOME)

        m_cabo = _material("cabo.borracha", cor_linear(p["cor_cabo"]), p["rugosidade_cabo"])
        m_plugue = _material("cabo.plugue", cor_linear(p["cor_cabo"]), p["rugosidade_plugue"])
        m_contato = _material("cabo.contato", tuple(p["cor_contato"]) + (1.0,), 0.30, metalico=1.0)

        plugue, partes = _construir_plugue(col, p, m_plugue, m_contato)
        curva = _construir_curva(col, p, m_cabo)

        ponto = Vector(p["ponto_tomada"])
        direcao = Vector(p["direcao_entrada"]).normalized()
        normal = -direcao
        raio = p["diametro_cabo"] / 2.0
        lateral = _lateral(normal, p["lado"])
        ponto_fora = Vector(p["ponto_fora"]) if p["ponto_fora"] is not None else \
            _ponto_fora_padrao(ponto, normal, lateral, p["z_chao"], raio)

        objs = {
            "plugue": plugue,
            "curva": curva,
            "partes": partes,
            "contatos": partes["contatos"],
            "colecao": col,
            "comprimento_plugue": COMPRIMENTO_PLUGUE,
            "raio": raio,
            "z_chao": p["z_chao"],
            "ponto_fora": ponto_fora,
            "lateral": lateral,
            "lado": p["lado"],
            "penetracao": p["penetracao"],
            "materiais": {"cabo": m_cabo, "plugue": m_plugue, "contato": m_contato},
        }
        _aplicar_pose(objs, ponto + direcao * p["penetracao"], _quat_eixo_y(direcao))
        objs["comprimento"] = curva.data.splines[0].calc_length(resolution=32)
        return objs


    def _ease(u, easing):
        u = min(1.0, max(0.0, u))
        if easing == "LINEAR":
            return u
        if easing == "EASE_IN":
            return u * u * u
        if easing == "EASE_OUT":
            return 1.0 - (1.0 - u) ** 3
        # EASE_IN_OUT: smoothstep cubico. O smootherstep (quintico) para
        # completamente no fim e o plugue "rasteja" ate a tomada; o cubico chega
        # com a "leve desaceleracao" pedida e o clique faz o resto.
        return u * u * (3.0 - 2.0 * u)


    def _bezier3(p0, p1, p2, p3, t):
        s = 1.0 - t
        return p0 * (s * s * s) + p1 * (3 * s * s * t) + p2 * (3 * s * t * t) + p3 * (t * t * t)


    def _bezier3_tangente(p0, p1, p2, p3, t):
        s = 1.0 - t
        return (p1 - p0) * (3 * s * s) + (p2 - p1) * (6 * s * t) + (p3 - p2) * (3 * t * t)


    def animar_conexao(objs, ponto_tomada, direcao_entrada, q_ini, q_fim, easing="EASE_IN_OUT",
                       origem=None, ponto_fora=None, z_chao=None, altura_arco=0.18,
                       distancia_alinhada=0.25, recuo=0.003, quadros_clique=8, penetracao=None):
        """Plugue vem de fora do quadro (de tras e de baixo, ~1 m) num arco
        suave, alinha-se com direcao_entrada nos ultimos centimetros e encaixa
        em ponto_tomada com desaceleracao e um micro-recuo de 'recuo' metros (o
        clique). O cabo acompanha e termina numa curva de gravidade ate o chao.
        Uma chave por quadro em plugue e cabo, so nos objetos deste modulo."""
        ponto = Vector(ponto_tomada)
        direcao = Vector(direcao_entrada).normalized()
        normal = -direcao
        if z_chao is not None:
            objs["z_chao"] = z_chao
        lateral = _lateral(normal, objs.get("lado", 1.0))
        objs["lateral"] = lateral
        if ponto_fora is not None:
            objs["ponto_fora"] = Vector(ponto_fora)
        else:
            objs["ponto_fora"] = _ponto_fora_padrao(ponto, normal, lateral, objs["z_chao"], objs["raio"])
        if penetracao is None:
            penetracao = objs.get("penetracao", 0.0)
        assento = ponto + direcao * penetracao

        if origem is None:
            # ~0,9 m atras e para o lado, a 2 cm do chao: o plugue "levanta" do
            # chao, onde o cabo ja estava jogado.
            origem = ponto + normal * 0.85 + lateral * 0.30
            origem.z = objs["z_chao"] + 0.02
        origem = Vector(origem)

        # Arco: Bezier cubico. O penultimo ponto de controle esta na reta da
        # tomada, entao a tangente final E direcao_entrada: o alinhamento sai da
        # geometria, sem blend de angulo.
        c1 = origem + (ponto - origem) * 0.45
        c1.z = max(origem.z, ponto.z) + altura_arco
        c2 = assento + normal * distancia_alinhada

        q_toque = max(q_ini + 1, q_fim - quadros_clique)
        q_ant = None
        for f in range(q_ini, q_fim + 1):
            if f <= q_toque:
                u = (f - q_ini) / float(q_toque - q_ini)
                s = _ease(u, easing)
                pos = _bezier3(origem, c1, c2, assento, s)
                tang = _bezier3_tangente(origem, c1, c2, assento, s)
                # O plugue segue a tangente do voo, mas decola deitado: nos
                # primeiros 20% a inclinacao entra aos poucos, senao ele sai do
                # chao apontando para cima como um foguete.
                eixo = Vector((tang.x, tang.y, tang.z * min(1.0, u / 0.2)))
                if eixo.length < 1e-6:
                    eixo = direcao
                # Perto da tomada a tangente ja e a direcao; misturar garante o
                # alinhamento exato mesmo com easing que zera a velocidade.
                eixo = eixo.normalized().lerp(direcao, s * s).normalized()
            else:
                # Clique: encaixou, recua 'recuo' e assenta. Meio seno = para no
                # fim sem solavanco.
                v = (f - q_toque) / float(q_fim - q_toque)
                pos = assento + normal * (recuo * math.sin(math.pi * v))
                eixo = direcao
            quat = _quat_eixo_y(eixo)
            # Quaternions interpolam pelo caminho curto so se os sinais forem
            # coerentes entre chaves vizinhas.
            if q_ant is not None and quat.dot(q_ant) < 0.0:
                quat = -quat
            q_ant = quat
            _aplicar_pose(objs, pos, quat, quadro=f)

        _suavizar(objs, q_ini, q_fim)


    def _suavizar(objs, q_ini, q_fim):
        """Bezier auto-clamped em todas as chaves do intervalo: com uma chave por
        quadro nao muda o caminho, mas deixa o motion blur (sub-quadro) suave."""
        for bloco in (objs["plugue"], objs["curva"].data):
            ad = bloco.animation_data
            if ad is None or ad.action is None:
                continue
            for fc in ad.action.fcurves:
                for kp in fc.keyframe_points:
                    if q_ini - 0.5 <= kp.co.x <= q_fim + 0.5:
                        kp.interpolation = "BEZIER"
                        kp.handle_left_type = "AUTO_CLAMPED"
                        kp.handle_right_type = "AUTO_CLAMPED"
                fc.update()


    def posicionar_repouso(objs, ponto_tomada, direcao_entrada, ponto_fora=None, z_chao=None, penetracao=None):
        """Sem animacao: plugue encaixado e cabo em repouso (para os beats em que
        o cabo ja esta conectado e a coreografia so move o U1)."""
        ponto = Vector(ponto_tomada)
        direcao = Vector(direcao_entrada).normalized()
        if z_chao is not None:
            objs["z_chao"] = z_chao
        lateral = _lateral(-direcao, objs.get("lado", 1.0))
        objs["lateral"] = lateral
        if ponto_fora is not None:
            objs["ponto_fora"] = Vector(ponto_fora)
        else:
            objs["ponto_fora"] = _ponto_fora_padrao(ponto, -direcao, lateral, objs["z_chao"], objs["raio"])
        if penetracao is None:
            penetracao = objs.get("penetracao", 0.0)
        _aplicar_pose(objs, ponto + direcao * penetracao, _quat_eixo_y(direcao))
    return locals()


mod_cabo = _registrar_modulo('mod_cabo', _modulo_cabo())


# ============================================================================
# MODULO mod_cartela (scripts/mod_cartela.py), inteiro, dentro de uma funcao-namespace
# ============================================================================
def _modulo_cartela():
    # Modulo CARTELA do anuncio do Snapmaker U1.
    #
    # Quatro linhas de texto, estilo Apple (sans fina, tracking largo, branca,
    # centrada, hierarquia clara) e, acima delas, a logo da EnginePrint num plano
    # com alfa. So definicoes aqui - nada roda no import. Quem integra e
    # mod_coreografia.py; quem prova este modulo sozinho e teste_cartela.py.
    #
    # DECISOES:
    #
    # - O Empty 'cartela.raiz' segue a CONVENCAO DE CAMERA: o texto vive no plano
    #   XY local, le-se ao longo de +X, sobe em +Y e o normal (+Z) aponta para
    #   quem olha. Assim a coreografia poe a cartela em frente a camera com uma
    #   linha - posicionar_cartela(objs, camera, distancia) faz
    #   raiz.matrix_world = camera.matrix_world @ Translation(0, 0, -distancia) -
    #   sem saber medida nenhuma do bloco. No teste a raiz fica em pe olhando -Y.
    #
    # - O tamanho das letras NAO e digitado: e MEDIDO. As linhas nascem com os
    #   tamanhos relativos de 'tamanhos', o modulo mede a largura de cada uma
    #   (bound_box depois do depsgraph) com o tracking FINAL, e escala o conjunto
    #   para a linha mais larga caber em 'largura_max' - que por padrao deriva da
    #   camera do projeto (35 mm, sensor 36 no lado maior, formato 9:16, a 2 m:
    #   1,157 m de largura visivel, 0,9 disso = 1,041 m). Assim trocar a fonte
    #   (que muda a largura dos glifos) nao corta nada nas bordas.
    #
    # - O fade NAO e pela forca da emissao, e pelo ALFA. Emissao com forca 0 e
    #   PRETO, nao invisivel: sobre a faixa rose do gradiente as letras entrariam
    #   como silhuetas escuras. Cada linha tem Emission (branca, forca 1,0 - nao
    #   estoura no AgX) misturada com Transparent por um Value 'alfa', que e o que
    #   recebe as chaves; o material e BLENDED.
    #
    # - Tracking: o 'space_character' do TextCurve e um MULTIPLICADOR do avanco de
    #   cada glifo (1,0 = a fonte como desenhada), nao um acrescimo em em. O
    #   pedido "tracking 0,25 -> 0,05" e em fracao de em; a conversao e
    #   space_character = 1 + tracking / AVANCO_MEDIO, com o avanco medio de uma
    #   sans minuscula em ~0,55 em. Fica 1,45 -> 1,09.
    #
    # - Fonte: nenhuma sans fina de verdade existe aqui (/usr/share/fonts tem
    #   DejaVu, Liberation, FreeSans, Loma, IPA, Unifont). FreeSans e o clone
    #   metrico da Helvetica - a mais proxima do que a Apple usava antes da SF -
    #   e e o padrao no Linux; no Windows do cliente a lista prefere Segoe UI
    #   Light / Semibold, que e o "fino" de verdade. Sem nenhuma, fica a Bfont do
    #   Blender (mais gorda; funciona). A fonte e embutida (pack) para o .blend
    #   nao depender do caminho.
    #
    # Eixos e medidas seguem docs/ESPECIFICACAO.md: metros, Z para cima, frente
    # em -Y.

    import math
    import os

    import bpy
    from mathutils import Matrix, Vector

    NOME = "cartela"

    # Avanco medio de um glifo minusculo de sans em fracao de em: e o que converte
    # tracking (fracao de em) em space_character (multiplicador). Aproximacao;
    # o que importa e a largura final, e essa e medida.
    AVANCO_MEDIO = 0.55

    FONTES_FINAS = (
        "C:/Windows/Fonts/segoeuil.ttf",           # Segoe UI Light (Windows)
        "C:/Windows/Fonts/segoeui.ttf",
        "/usr/share/fonts/truetype/freefont/FreeSans.ttf",
        "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    )
    FONTES_FORTES = (
        "C:/Windows/Fonts/seguisb.ttf",            # Segoe UI Semibold (Windows)
        "C:/Windows/Fonts/segoeuib.ttf",
        "/usr/share/fonts/truetype/freefont/FreeSansBold.ttf",
        "/usr/share/fonts/truetype/liberation/LiberationSans-Bold.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
    )

    PARAMS_PADRAO = {
        "textos": (
            "EnginePrint",
            "qualidade excepcional",
            "13 unidades restantes",
            "compre em engineprint.com.br",
        ),
        # Tamanho relativo (em) de cada linha; o absoluto sai da medicao.
        "tamanhos": (1.0, 0.62, 0.62, 0.42),
        # Distancia entre a linha de base de uma linha e a de baixo, em fracao do
        # tamanho da linha de CIMA - o que mantem a proporcao quando escala.
        "entrelinhas": (1.30, 1.45, 1.80),
        # Peso: linha 1 forte, as outras finas.
        "fortes": (True, False, False, False),
        "cor_texto": "#FFFFFF",
        "cor_destaque": "#C8641F",     # cobre da logo, na linha 3
        "linha_destaque": 3,
        # O AgX escurece e empalidece cor saturada a forca 1,0; o cobre saia
        # marrom. So a linha de destaque sobe um pouco.
        "forca_destaque": 1.6,
        "forca_texto": 1.0,
        # Fonte: caminho explicito ou None (percorre as listas acima).
        "fonte_fina": None,
        "fonte_forte": None,
        "extrusao": 0.0,
        "chanfro": 0.0,                # emissao nao sombreia: chanfro so engorda
        "resolucao": 12,               # curvas dos glifos: de perto, 12 e liso
        # Tracking em fracao de em: assentado e no inicio da entrada. O inicial e
        # um TETO: medido, 0,25 em poe a linha mais larga a 1,22 da largura do
        # quadro (cortada dos dois lados) - por isso cada linha recebe o maior
        # tracking inicial que ainda cabe no quadro (ver construir_cartela).
        "tracking": 0.05,
        "tracking_inicial": 0.25,
        # Folga da borda ao limitar o tracking inicial: a largura cresce um pouco
        # mais que linearmente com o space_character (medido 1,356x para 1,33x).
        "folga_borda": 0.94,
        "com_logo": True,
        "logo": "logo_engineprint.png",   # relativo a assets/; ou caminho absoluto
        "largura_logo": 0.30,             # do plano, em fracao da largura do bloco
        "espaco_logo": 0.35,              # entre a base da logo e o topo da linha 1, em fracao do tamanho da linha 1
        # Camera do projeto, so para derivar largura_max (None = derivar).
        "largura_max": None,
        "fracao_largura": 0.9,
        "distancia": 2.0,
        "lente": 35.0,
        "sensor": 36.0,
        "proporcao": 9.0 / 16.0,
        # Pose padrao da raiz: em pe, olhando -Y (a camera padrao esta em -Y),
        # com o centro do bloco a 1 m do chao. A coreografia sobrescreve.
        "posicao": (0.0, 0.0, 1.0),
        "rotacao": (math.pi / 2.0, 0.0, 0.0),
    }

    FPS = 30.0


    # ---------------------------------------------------------------- utilidades

    def _srgb_para_linear(c):
        c = c / 255.0
        return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4


    def cor_linear(hexa):
        """'#RRGGBB' -> (r, g, b, 1) linear, que e o que os nos de shader querem."""
        h = hexa.lstrip("#")
        return tuple(_srgb_para_linear(int(h[i:i + 2], 16)) for i in (0, 2, 4)) + (1.0,)


    def limpar_colecao(nome):
        """Remove a sub-colecao <nome> e tudo o que ela contem (idempotencia)."""
        col = bpy.data.collections.get(nome)
        if col is not None:
            for obj in list(col.all_objects):
                dados = obj.data
                bpy.data.objects.remove(obj, do_unlink=True)
                # Curva de texto orfa acumularia 'cartela.linha.1.001' a cada
                # rodada; some junto com o objeto.
                if dados is not None and dados.users == 0:
                    if isinstance(dados, bpy.types.Curve):
                        bpy.data.curves.remove(dados)
                    elif isinstance(dados, bpy.types.Mesh):
                        bpy.data.meshes.remove(dados)
            for filha in list(col.children):
                limpar_colecao(filha.name)
            bpy.data.collections.remove(col)
        # Materiais do modulo sem dono tambem, pelo mesmo motivo.
        for mat in list(bpy.data.materials):
            if mat.name.startswith(nome + ".") and mat.users == 0:
                bpy.data.materials.remove(mat)


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
        if os.path.isabs(nome_arquivo):
            return nome_arquivo
        # scripts/ e assets/ sao irmaos na raiz do projeto.
        raiz = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
        return os.path.join(raiz, "assets", nome_arquivo)


    def _carregar_fonte(explicita, candidatas):
        """Primeira fonte que existir; None = Bfont (a do Blender)."""
        caminhos = ([explicita] if explicita else []) + list(candidatas)
        for caminho in caminhos:
            if caminho and os.path.exists(caminho):
                fonte = bpy.data.fonts.load(caminho, check_existing=True)
                try:
                    # Embutida: o .blend do cliente nao depende deste caminho.
                    if not fonte.packed_file:
                        fonte.pack()
                except RuntimeError:
                    pass
                return fonte
        return None


    def _tracking_para_espacamento(tracking):
        return 1.0 + tracking / AVANCO_MEDIO


    def _largura_visivel(distancia, lente, sensor, proporcao):
        """Largura do quadro 9:16 a 'distancia' da camera: o sensor de 36 mm vai
        para o lado MAIOR (a altura), e a largura e a altura vezes 9/16."""
        altura = distancia * sensor / lente
        return altura * proporcao


    # ---------------------------------------------------------------- materiais

    def _material_texto(nome, cor_hex, forca):
        """Emission x Transparent misturados por um Value 'alfa' (0 = invisivel).
        E o alfa que recebe as chaves do fade - ver cabecalho."""
        mat = bpy.data.materials.new(nome)
        mat.use_nodes = True
        nt = mat.node_tree
        for n in list(nt.nodes):
            nt.nodes.remove(n)
        saida = nt.nodes.new("ShaderNodeOutputMaterial")
        saida.location = (400, 0)
        mistura = nt.nodes.new("ShaderNodeMixShader")
        mistura.location = (200, 0)
        transp = nt.nodes.new("ShaderNodeBsdfTransparent")
        transp.location = (0, 80)
        emis = nt.nodes.new("ShaderNodeEmission")
        emis.location = (0, -80)
        emis.inputs["Color"].default_value = cor_linear(cor_hex)
        emis.inputs["Strength"].default_value = forca
        alfa = nt.nodes.new("ShaderNodeValue")
        alfa.name = "alfa"
        alfa.label = "alfa"
        alfa.location = (0, 220)
        alfa.outputs[0].default_value = 1.0
        nt.links.new(alfa.outputs[0], mistura.inputs["Fac"])
        nt.links.new(transp.outputs[0], mistura.inputs[1])
        nt.links.new(emis.outputs[0], mistura.inputs[2])
        nt.links.new(mistura.outputs[0], saida.inputs["Surface"])
        _transparente(mat)
        return mat


    def _material_logo(nome, imagem):
        """Cor da imagem em Emission (nao depende de luz), alfa da imagem vezes o
        Value 'alfa' do fade."""
        mat = bpy.data.materials.new(nome)
        mat.use_nodes = True
        nt = mat.node_tree
        for n in list(nt.nodes):
            nt.nodes.remove(n)
        saida = nt.nodes.new("ShaderNodeOutputMaterial")
        saida.location = (600, 0)
        mistura = nt.nodes.new("ShaderNodeMixShader")
        mistura.location = (400, 0)
        transp = nt.nodes.new("ShaderNodeBsdfTransparent")
        transp.location = (200, 80)
        emis = nt.nodes.new("ShaderNodeEmission")
        emis.location = (200, -80)
        emis.inputs["Strength"].default_value = 1.0
        tex = nt.nodes.new("ShaderNodeTexImage")
        tex.location = (-200, 0)
        tex.image = imagem
        tex.interpolation = "Cubic"
        tex.extension = "CLIP"
        alfa = nt.nodes.new("ShaderNodeValue")
        alfa.name = "alfa"
        alfa.label = "alfa"
        alfa.location = (-200, 260)
        alfa.outputs[0].default_value = 1.0
        produto = nt.nodes.new("ShaderNodeMath")
        produto.operation = "MULTIPLY"
        produto.location = (100, 260)
        nt.links.new(tex.outputs["Alpha"], produto.inputs[0])
        nt.links.new(alfa.outputs[0], produto.inputs[1])
        nt.links.new(tex.outputs["Color"], emis.inputs["Color"])
        nt.links.new(produto.outputs[0], mistura.inputs["Fac"])
        nt.links.new(transp.outputs[0], mistura.inputs[1])
        nt.links.new(emis.outputs[0], mistura.inputs[2])
        nt.links.new(mistura.outputs[0], saida.inputs["Surface"])
        _transparente(mat)
        return mat


    def _transparente(mat):
        try:
            mat.surface_render_method = "BLENDED"   # 4.2+
        except AttributeError:
            mat.blend_method = "BLEND"              # 4.1 e antes
        try:
            mat.show_transparent_back = False
        except AttributeError:
            pass
        try:
            mat.shadow_method = "NONE"              # sumiu no 4.2; existe antes
        except AttributeError:
            pass


    def _carregar_logo(caminho):
        if os.path.exists(caminho):
            img = bpy.data.images.load(caminho, check_existing=True)
            return img, False
        # Sem o PNG, um quadrado cobre com furo, para a cartela nao ficar sem nada.
        import numpy as np
        img = bpy.data.images.new("cartela.logo_provisoria", 256, 256, alpha=True)
        px = np.zeros((256, 256, 4), dtype=np.float32)
        px[32:224, 32:224] = (0.58, 0.13, 0.014, 1.0)
        px[96:160, 96:160] = (0.0, 0.0, 0.0, 0.0)
        img.pixels.foreach_set(px.ravel())
        img.pack()
        return img, True


    def _medir_conteudo(img):
        """Fracao (largura, altura) da imagem ocupada pelos pixels opacos e o
        centro deles, para o plano da logo ser dimensionado pelo DESENHO e nao
        pela margem transparente do PNG."""
        import numpy as np
        w, h = img.size
        if w == 0 or h == 0:
            return (1.0, 1.0), (0.5, 0.5)
        px = np.empty(w * h * 4, dtype=np.float32)
        img.pixels.foreach_get(px)
        alfa = px.reshape(h, w, 4)[:, :, 3]
        ys, xs = np.where(alfa > 0.5)
        if len(xs) == 0:
            return (1.0, 1.0), (0.5, 0.5)
        x0, x1, y0, y1 = xs.min(), xs.max() + 1, ys.min(), ys.max() + 1
        return ((x1 - x0) / w, (y1 - y0) / h), ((x0 + x1) / (2.0 * w), (y0 + y1) / (2.0 * h))


    # ---------------------------------------------------------------- construir

    def _linha(nome, texto, fonte, tamanho, espacamento, p, col, raiz):
        curva = bpy.data.curves.new(nome, "FONT")
        curva.body = texto
        if fonte is not None:
            curva.font = fonte
        curva.size = tamanho
        curva.space_character = espacamento
        curva.align_x = "CENTER"
        # Origem na linha de base: a posicao Y do objeto E a linha de base, e a
        # subida de 3 cm da entrada nao depende da altura dos glifos.
        curva.align_y = "TOP_BASELINE"
        curva.extrude = p["extrusao"]
        curva.bevel_depth = p["chanfro"]
        curva.bevel_resolution = 1
        curva.resolution_u = p["resolucao"]
        curva.fill_mode = "BOTH"
        obj = bpy.data.objects.new(nome, curva)
        col.objects.link(obj)
        obj.parent = raiz
        return obj


    def _largura(obj):
        """Largura em X do objeto avaliado, no espaco local (o texto e plano)."""
        xs = [v[0] for v in obj.bound_box]
        return max(xs) - min(xs)


    def construir_cartela(cena, colecao_pai=None, params=None):
        """Cria 'cartela.raiz' (Empty, convencao de camera: +Z para quem olha, +Y
        para cima), 'cartela.linha.1..4' (TEXT, emissao) e 'cartela.logo' (plano
        com alfa) na sub-colecao 'cartela'. Devolve os objetos e as medidas."""
        p = dict(PARAMS_PADRAO)
        if params:
            p.update(params)
        limpar_colecao(NOME)
        col = _colecao(cena, colecao_pai, NOME)

        largura_max = p["largura_max"]
        if largura_max is None:
            largura_max = p["fracao_largura"] * _largura_visivel(
                p["distancia"], p["lente"], p["sensor"], p["proporcao"])

        raiz = bpy.data.objects.new(NOME + ".raiz", None)
        raiz.empty_display_type = "PLAIN_AXES"
        raiz.empty_display_size = 0.1
        raiz.location = p["posicao"]
        raiz.rotation_euler = p["rotacao"]
        col.objects.link(raiz)

        fonte_fina = _carregar_fonte(p["fonte_fina"], FONTES_FINAS)
        fonte_forte = _carregar_fonte(p["fonte_forte"], FONTES_FORTES)
        esp_fim = _tracking_para_espacamento(p["tracking"])

        # 1) linhas em tamanho RELATIVO, para medir.
        linhas = []
        for i, texto in enumerate(p["textos"]):
            fonte = fonte_forte if p["fortes"][i] else fonte_fina
            linhas.append(_linha("%s.linha.%d" % (NOME, i + 1), texto, fonte,
                                 p["tamanhos"][i], esp_fim, p, col, raiz))
        bpy.context.view_layer.update()
        larguras_rel = [_largura(o) for o in linhas]
        mais_larga = max(larguras_rel)
        # 2) escala unica que poe a mais larga em largura_max.
        escala = largura_max / mais_larga if mais_larga > 0 else 1.0
        tamanhos = [t * escala for t in p["tamanhos"]]
        for obj, tam in zip(linhas, tamanhos):
            obj.data.size = tam

        # 3) empilhar: linha 1 no topo, descendo pelas entrelinhas; depois centrar
        # o conjunto (logo incluida) na origem da raiz.
        bases = [0.0]
        for i in range(1, len(linhas)):
            bases.append(bases[-1] - p["entrelinhas"][i - 1] * tamanhos[i - 1])
        # Altura de caixa alta ~0,72 em: topo do bloco de texto acima da base 1.
        topo_texto = bases[0] + 0.72 * tamanhos[0]
        # Descendente da ultima linha (~0,22 em) e o fundo do bloco.
        fundo = bases[-1] - 0.22 * tamanhos[-1]

        logo = None
        imagem = None
        altura_logo = 0.0
        if p["com_logo"]:
            imagem, provisoria = _carregar_logo(_caminho_asset(p["logo"]))
            (fx, fy), (cx, cy) = _medir_conteudo(imagem)
            largura_desenho = p["largura_logo"] * largura_max
            lado = largura_desenho / max(fx, 1e-6)      # plano inteiro (com margem)
            altura_desenho = lado * fy
            altura_logo = altura_desenho + p["espaco_logo"] * tamanhos[0]
            malha = bpy.data.meshes.new(NOME + ".logo")
            h = lado / 2.0
            malha.from_pydata([(-h, -h, 0), (h, -h, 0), (h, h, 0), (-h, h, 0)], [], [(0, 1, 2, 3)])
            uv = malha.uv_layers.new(name="UVMap")
            for l, co in zip(uv.data, ((0, 0), (1, 0), (1, 1), (0, 1))):
                l.uv = co
            malha.update()
            malha.materials.append(_material_logo(NOME + ".logo", imagem))
            logo = bpy.data.objects.new(NOME + ".logo", malha)
            col.objects.link(logo)
            logo.parent = raiz
            # Centro do DESENHO (nao do PNG) fica em cima da linha 1 com a folga.
            # Pixel (0,0) do Blender e o canto inferior esquerdo; cy cresce p/ cima.
            centro_y = topo_texto + p["espaco_logo"] * tamanhos[0] + altura_desenho / 2.0
            logo.location = (-(cx - 0.5) * lado, centro_y - (cy - 0.5) * lado, 0.0)

        topo = topo_texto + altura_logo
        deslocamento = -(topo + fundo) / 2.0     # centra o bloco em Y = 0
        for obj, base in zip(linhas, bases):
            obj.location = (0.0, base + deslocamento, 0.0)
        if logo is not None:
            logo.location.y += deslocamento

        # materiais
        for i, obj in enumerate(linhas):
            destaque = (i + 1) == p["linha_destaque"]
            mat = _material_texto(
                "%s.linha.%d" % (NOME, i + 1),
                p["cor_destaque"] if destaque else p["cor_texto"],
                p["forca_destaque"] if destaque else p["forca_texto"])
            obj.data.materials.append(mat)

        bpy.context.view_layer.update()
        larguras = [_largura(o) for o in linhas]
        # Tracking inicial por linha: o pedido, limitado pelo que cabe no quadro.
        # A largura e ~proporcional ao space_character, entao a linha de largura
        # L (assentada, esp_fim) chega a borda quando esp = esp_fim * quadro / L.
        largura_quadro = largura_max / p["fracao_largura"]
        esp_ini_pedido = _tracking_para_espacamento(p["tracking_inicial"])
        esp_iniciais = [
            min(esp_ini_pedido, esp_fim * p["folga_borda"] * largura_quadro / max(l, 1e-6))
            for l in larguras
        ]
        return {
            "raiz": raiz,
            "linhas": linhas,
            "logo": logo,
            "imagem_logo": imagem,
            "tamanhos": tamanhos,
            "larguras": larguras,
            "largura": max(larguras),
            "altura": topo - fundo,
            "largura_max": largura_max,
            "distancia": p["distancia"],
            "espacamento_final": esp_fim,
            "espacamentos_iniciais": esp_iniciais,
            "fonte_fina": fonte_fina.name if fonte_fina else "Bfont",
            "fonte_forte": fonte_forte.name if fonte_forte else "Bfont",
        }


    def posicionar_cartela(objs, camera, distancia=None):
        """Poe a raiz a 'distancia' em frente a camera, de cara para ela, com o
        'para cima' da camera. So mexe na raiz; a camera e apenas lida."""
        if distancia is None:
            distancia = objs["distancia"]
        objs["raiz"].matrix_world = camera.matrix_world @ Matrix.Translation(Vector((0.0, 0.0, -distancia)))


    # ---------------------------------------------------------------- animacao

    def _no_alfa(obj):
        for mat in obj.data.materials:
            if mat is not None and mat.use_nodes:
                no = mat.node_tree.nodes.get("alfa")
                if no is not None:
                    return no.outputs[0], mat.node_tree
        return None, None


    def _chave_alfa(obj, quadro, valor):
        saida, _ = _no_alfa(obj)
        if saida is None:
            return
        saida.default_value = valor
        saida.keyframe_insert("default_value", frame=quadro)


    def _suavizar(dono, q_ini, q_fim, easing, interpolacao="BEZIER"):
        """Bezier + easing so nas chaves deste intervalo, para nao alterar a
        animacao de outro beat. 'dono' e qualquer ID com animation_data."""
        ad = getattr(dono, "animation_data", None)
        if ad is None or ad.action is None:
            return
        for fc in ad.action.fcurves:
            for kp in fc.keyframe_points:
                if q_ini - 0.5 <= kp.co.x <= q_fim + 0.5:
                    kp.interpolation = interpolacao
                    kp.easing = easing
                    if interpolacao == "BEZIER":
                        kp.handle_left_type = "AUTO_CLAMPED"
                        kp.handle_right_type = "AUTO_CLAMPED"
            fc.update()


    def _elementos(objs):
        """Ordem de entrada: a logo primeiro, depois as linhas de cima p/ baixo."""
        seq = []
        if objs.get("logo") is not None:
            seq.append(objs["logo"])
        seq.extend(objs["linhas"])
        return seq


    def _repouso(obj):
        """Posicao assentada do elemento: guardada no proprio objeto na primeira
        animacao, para a saida e uma segunda entrada nao acumularem deslocamento."""
        if "cartela_repouso" not in obj:
            obj["cartela_repouso"] = list(obj.location)
        return Vector(obj["cartela_repouso"])


    def animar_cartela(objs, q_ini, q_fim, easing="EASE_IN_OUT", subida=0.03,
                       fracao_elemento=0.55):
        """Entrada escalonada: a logo primeiro, depois as linhas 1..4. Cada
        elemento gasta 'fracao_elemento' do intervalo fazendo fade (alfa 0 -> 1),
        subindo 'subida' m e fechando o tracking; os inicios se distribuem para o
        ultimo terminar exatamente em q_fim - tudo assentado ate la."""
        seq = _elementos(objs)
        n = len(seq)
        total = float(q_fim - q_ini)
        dur = max(1.0, total * fracao_elemento)
        passo = (total - dur) / (n - 1) if n > 1 else 0.0
        esp_fim = objs["espacamento_final"]
        for i, obj in enumerate(seq):
            a = int(round(q_ini + i * passo))
            b = int(round(a + dur)) if i < n - 1 else q_fim
            repouso = _repouso(obj)
            esp_ini = esp_fim
            if obj.type == "FONT":
                esp_ini = objs["espacamentos_iniciais"][objs["linhas"].index(obj)]
            # Invisivel ate a sua vez: chave no quadro anterior ao inicio, para o
            # elemento nao "aparecer" interpolando desde uma chave de outro beat.
            for quadro, alfa, deslocamento, esp in ((a - 1, 0.0, -subida, esp_ini),
                                                    (a, 0.0, -subida, esp_ini),
                                                    (b, 1.0, 0.0, esp_fim)):
                _chave_alfa(obj, quadro, alfa)
                obj.location = repouso + Vector((0.0, deslocamento, 0.0))
                obj.keyframe_insert("location", frame=quadro)
                if obj.type == "FONT":
                    obj.data.space_character = esp
                    obj.data.keyframe_insert("space_character", frame=quadro)
            _suavizar(obj, a, b, easing)
            _suavizar(obj.data, a, b, easing)
            _, nt = _no_alfa(obj)
            _suavizar(nt, a, b, easing)


    def animar_cartela_saida(objs, q_ini, q_fim, easing="EASE_IN_OUT", deslocamento=0.02):
        """Fade out de tudo junto (alfa 1 -> 0), com uma leve continuacao da
        subida para nao parecer que a cartela 'apagou'."""
        for obj in _elementos(objs):
            repouso = _repouso(obj)
            for quadro, alfa, dy in ((q_ini, 1.0, 0.0), (q_fim, 0.0, deslocamento)):
                _chave_alfa(obj, quadro, alfa)
                obj.location = repouso + Vector((0.0, dy, 0.0))
                obj.keyframe_insert("location", frame=quadro)
            _suavizar(obj, q_ini, q_fim, easing)
            _, nt = _no_alfa(obj)
            _suavizar(nt, q_ini, q_fim, easing)
    return locals()


mod_cartela = _registrar_modulo('mod_cartela', _modulo_cartela())


# ============================================================================
# MODULO mod_coreografia (scripts/mod_coreografia.py), inteiro, dentro de uma funcao-namespace
# ============================================================================
def _modulo_coreografia():
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
    # - A CAIXA DESCE E SOME no beat 2 (em vez de o U1 pousar ao lado dela). O
    #   motivo e o beat 3: a camera orbita 180 graus ate a traseira para ver o
    #   cabo entrar numa tomada a 12 cm do chao. Com a caixa de 0,8 m atras do
    #   U1 (ou ao lado), ela entraria entre a camera e a tomada em parte da
    #   orbita. Com o U1 sozinho na origem, a orbita e os closes do beat 5 tem
    #   360 graus livres, e o rig de luzes (centrado na origem) continua certo.
    #   No beat 6 a caixa volta pelo chao enquanto o U1 flutua acima dela - e o
    #   mesmo truque ao contrario, e a ordem (U1 sobe, caixa sobe, U1 desce,
    #   espuma volta, tampa fecha) e a que nao atravessa nada: conferir_colisoes
    #   mede isso quadro a quadro.
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
        },
        3: {
            "orbita": (0.00, 0.48),         # frente -> traseira pelo lado +X
            "cabo": (0.19, 0.71),
            "botao": (0.77, 0.98),
        },
        4: {
            "orbita": (0.00, 0.56),         # traseira -> frente pelo lado -X
            "boot": 0.58,                   # boot de ~0,8 s, corte seco para a UI
            "ui": 0.84,
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
            "sobe_para_logo": (0.00, 0.244),
            "mergulho": (0.244, 0.444),     # quadro da travessia = fim do mergulho
            "cartela": (0.456, 0.833),
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
        # Deslocamento da cartela para cima no quadro (m a 2 m): tira a linha 4
        # da zona de legendas do Reels e a logo do brilho do horizonte.
        "cartela_subida": 0.13,
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


    def _chave_rim_especular(objs, quadro_, valor):
        """specular_factor do rim com chave constante: 0,0 nos planos largos, 0,5
        nos planos do produto (que esconde o ponto de espelho no chao).

        Medido no quadro 1 (chao vazio): o reflexo do painel do rim no chao e uma
        barra branca com rastro ate o pe do quadro; esconder o rim ou zerar o
        especular do chao a apaga, e 0,2 e 0,05 so a escurecem um pouco - e um
        painel de 350 W espelhado em Fresnel rasante, ~100x acima do branco, e
        so o zero resolve. As outras tres luzes espelham fora do quadro.
        """
        dados = objs["ambiente"]["luzes"]["rim"].data
        dados.specular_factor = valor
        try:
            dados.keyframe_insert("specular_factor", frame=quadro_)
        except (RuntimeError, TypeError):
            # Versao sem a propriedade animavel: fica o valor dos planos largos.
            dados.specular_factor = 0.0
            return
        for fc in dados.animation_data.action.fcurves:
            if fc.data_path == "specular_factor":
                for kp in fc.keyframe_points:
                    kp.interpolation = "CONSTANT"
                fc.update()


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


    def _u1_real(cena, col_pai, p):
        """Modelo do cliente por nome (objeto ou colecao). Devolve o mesmo dict
        que construir_u1, com 'real': True, ou None se o nome nao existe.

        O que faz: cria (ou reaproveita) o Empty 'u1.raiz', parenteia nele os
        objetos de topo do modelo mantendo a pose, aplica 'u1_rotacao_z', mede o
        bounding box avaliado e move a raiz para o modelo ficar centrado em XY com
        a base em z = 0 - a mesma pose do substituto, que e o que a caixa, o cabo
        e a camera esperam. Os pontos de tela/tomada/botao vem de params (nas
        coordenadas originais do arquivo dele, levadas pela mesma matriz) ou de
        uma heuristica pelo bounding box, documentada em _pontos_heuristicos.
        """
        nome = p["u1_nome"]
        fontes = []
        col_modelo = bpy.data.collections.get(nome)
        if col_modelo is not None:
            todos = set(o.name for o in col_modelo.all_objects)
            fontes = [o for o in col_modelo.all_objects if o.parent is None or o.parent.name not in todos]
            col = col_modelo
        elif nome in bpy.data.objects:
            fontes = [bpy.data.objects[nome]]
            col = bpy.data.collections.get("u1")
            if col is None:
                col = bpy.data.collections.new("u1")
                col_pai.children.link(col)
        else:
            return None

        raiz = bpy.data.objects.get("u1.raiz")
        if raiz is None or raiz.type != "EMPTY":
            raiz = bpy.data.objects.new("u1.raiz", None)
            raiz.empty_display_type = "ARROWS"
            raiz.empty_display_size = 0.2
            col.objects.link(raiz)
        raiz.parent = None
        raiz.matrix_world = Matrix.Identity(4)
        if raiz.animation_data:
            raiz.animation_data_clear()
        # matrix_world de objeto recem-criado ou recem-movido so e valido depois
        # de uma avaliacao (medido: sem isto a rotacao do bloco de teste saiu 0).
        bpy.context.view_layer.update()
        fontes = [o for o in fontes if o is not raiz]
        originais = {obj: obj.matrix_world.copy() for obj in fontes}
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


    # ---------------------------------------------------------------- construir

    def construir_tudo(params=None):
        """Colecao ANUNCIO com ambiente, caixa, U1 (substituto ou real), cabo,
        cartela e camera. Devolve o dict que coreografar e configurar_render usam."""
        p = dict(PARAMS_PADRAO)
        if params:
            p.update(params)
        cena = bpy.context.scene
        col = _colecao_raiz(cena)

        # Rim com especular 0,5 (padrao do modulo: 0,6) nos beats 3-5; nos
        # planos largos (beats 1, 2, 6, 7) a coreografia CHAVEIA o specular_factor
        # do rim para 0,05 (ver _beat1/_beat3/_beat6): o reflexo dele no chao cai
        # no eixo da camera e saia como uma barra branca no horizonte com um
        # rastro ate o pe do quadro - no quadro 1, antes de a caixa emergir, era a
        # imagem inteira. Medido: 0,4 e 0,2 so encolhem a barra (e Fresnel
        # rasante, nao intensidade). As outras tres luzes espelham fora do
        # quadro; o recorte da aresta vem do difuso, que fica.
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
            pu1 = {"imagem_boot": _asset(p, "tela_boot.png"), "imagem_ui": _asset(p, "tela_ui.png")}
            pu1.update(p["u1"])
            u1 = mod_u1.construir_u1(cena, col, pu1)
            u1["real"] = False

        pcaixa = {"cor": p["cor_caixa"], "logo": _asset(p, "logo_engineprint.png"), "u1": tuple(u1["dimensoes"])}
        pcaixa.update(p["caixa"])
        caixa = mod_caixa.construir_caixa(cena, col, pcaixa)
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

        # Forca 1,8/2,2: com 1,0 o "branco" media 0,82 sRGB no render (pendencia
        # do modulo cartela); bloco mais compacto (logo 0,24, entrelinha 3 menor)
        # para a linha 4 subir acima da faixa de legendas do Reels.
        pcart = {"logo": _asset(p, "logo_engineprint.png"), "forca_texto": 1.8, "forca_destaque": 2.2,
                 "largura_logo": 0.24, "entrelinhas": (1.30, 1.45, 1.55)}
        pcart.update(p["cartela"])
        cartela = mod_cartela.construir_cartela(cena, col, pcart)

        cam, alvo = mod_ambiente.criar_camera(cena, col, params=p["camera"])
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
            "_chaves_camera": {},
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
        # A lente so tem chave nos cortes: entre eles precisa segurar, nao rampar.
        ad = objs["camera"].data.animation_data
        if ad and ad.action:
            for fc in ad.action.fcurves:
                if fc.data_path == "lens":
                    for kp in fc.keyframe_points:
                        kp.interpolation = "CONSTANT"
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
        return {
            "cabecotes": Vector((0.0, 0.25 * P, 0.80 * A)),
            "porta": Vector((0.35 * L, -P / 2.0, 0.35 * A)),
            "mesa": Vector((0.0, 0.0, 0.21 * A)),
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
        profundidade = caixa["topo_tampa_z"] + 0.25    # some inteira sob o chao
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
        # centro da caixa que sobe.
        # A lente PRECISA de chave aqui: a fcurve extrapola a primeira chave para
        # tras, e sem esta os beats 1-4 saiam com os 60 mm da primeira foto do
        # beat 5 (medido: caixa 1,7x maior que o calculado).
        _chave_camera(objs, q_ini, -90.0, 2.2, 1.0, (0.0, 0.0, 0.30), lente=35.0)
        _chave_camera(objs, q_fim, -90.0, 2.6, 1.2, (0.0, 0.0, 0.45))
        _chave_rim_especular(objs, q_ini, 0.0)


    def _beat2(objs, fator):
        """Tampa sai, espuma explode, U1 sobe, caixa afunda no chao, U1 pousa."""
        r = ROTEIRO[2]
        q_ini, q_fim = quadros_do_beat(2, fator)
        q = lambda fr: q_em(2, fr, fator)  # noqa: E731
        caixa, u1, amb = objs["caixa"], objs["u1"], objs["ambiente"]

        mod_caixa.animar_tampa(caixa, q(r["tampa"][0]), q(r["tampa"][1]), abrir=True, lado=1.0)
        # Fora do quadro a tampa ficaria flutuando a 1,6 m do lado: esconder ate
        # o beat 6 - a chave de volta e gravada la.
        objs["_q_tampa_some"] = q(r["tampa"][1]) + 1

        mod_caixa.animar_espuma(caixa, q(r["espuma"][0]), q(r["espuma"][1]))

        raiz = u1["raiz"]
        z_alto = caixa["exterior_corpo"][2] + objs["params"]["folga_u1"]
        objs["z_alto_u1"] = z_alto
        z0 = u1["z_na_caixa"]
        _chave(raiz, q(r["u1_sobe"][0]), (0.0, 0.0, z0))
        _chave(raiz, q(r["u1_sobe"][1]), (0.0, 0.0, z_alto))
        _chave(raiz, q(r["u1_desce"][0]), (0.0, 0.0, z_alto))
        _chave(raiz, q(r["u1_desce"][1]), (0.0, 0.0, 0.0))
        _interpolar(raiz, q(r["u1_sobe"][0]), q_fim)

        corpo = caixa["corpo"]
        _chave(corpo, q(r["caixa_desce"][0]), (0.0, 0.0, 0.0))
        _chave(corpo, q(r["caixa_desce"][1]), (0.0, 0.0, -objs["profundidade_caixa"]))
        _interpolar(corpo, q(r["caixa_desce"][0]), q(r["caixa_desce"][1]))

        # Camera acompanha o U1 subindo (alvo sobe com ele) e comeca a derivar
        # para +X, de onde a orbita do beat 3 parte.
        _chave_camera(objs, q(r["u1_sobe"][1]), -84.0, 2.5, 1.6, (0.0, 0.0, z_alto + 0.30))
        _chave_camera(objs, q_fim, -75.0, 2.3, 1.1, (0.0, 0.0, 0.37))
        mod_ambiente.animar_rig(amb, q_ini, q_fim, 0.0, 15.0)


    def _beat3(objs, fator):
        """Orbita ate a traseira; cabo entra e encaixa; botao afunda, LED acende."""
        r = ROTEIRO[3]
        q_ini, q_fim = quadros_do_beat(3, fator)
        q = lambda fr: q_em(3, fr, fator)  # noqa: E731
        u1, cabo, amb, cena = objs["u1"], objs["cabo"], objs["ambiente"], objs["cena"]

        q_orb = q(r["orbita"][1])
        # Da orbita em diante o produto esconde o reflexo do rim no chao.
        _chave_rim_especular(objs, q_ini, 0.5)
        _chave_camera(objs, q_orb, 105.0, 1.5, 0.60, (0.15, 0.15, 0.30))
        _chave_camera(objs, q_fim, 100.0, 1.25, 0.42, (0.20, 0.20, 0.18))
        # Rim atras do produto do ponto de vista da camera: rig = azimute + 90.
        mod_ambiente.animar_rig(amb, q_ini, q_orb, 15.0, 195.0)
        mod_ambiente.animar_rig(amb, q_orb, q_fim, 195.0, 190.0)

        # Tomada no mundo com o U1 ja no chao (a raiz esta na identidade aqui).
        cena.frame_set(q_ini)
        bpy.context.view_layer.update()
        ponto = mod_u1.ponto_no_mundo(u1, "posicao_tomada", "ponto")
        direcao = mod_u1.ponto_no_mundo(u1, "posicao_tomada", "direcao")
        normal = -direcao
        lateral = normal.cross(Vector((0, 0, 1))).normalized() * cabo.get("lado", 1.0)
        # Origem fora do quadro (a 16 graus de meio-campo horizontal, 1,3 m atras
        # e 0,9 m para o lado esta fora em qualquer ponto da orbita), no chao.
        origem = ponto + normal * 1.3 + lateral * 0.9
        origem.z = 0.02
        q_cabo = (q(r["cabo"][0]), q(r["cabo"][1]))
        mod_cabo.animar_conexao(cabo, ponto, direcao, q_cabo[0], q_cabo[1],
                                origem=origem, z_chao=0.0, penetracao=-mod_cabo.BICO[4])
        objs["_q_cabo"] = q_cabo

        if u1.get("botao") is not None:
            mod_u1.animar_botao(u1, q(r["botao"][0]), q(r["botao"][1]))
        else:
            print("[coreografia] modelo real sem 'u1_botao_objeto': botao/LED nao animados")


    def _beat4(objs, fator):
        """Orbita de volta pela frente e dolly ate a tela; boot rapido e UI."""
        r = ROTEIRO[4]
        q_ini, q_fim = quadros_do_beat(4, fator)
        q = lambda fr: q_em(4, fr, fator)  # noqa: E731
        u1, amb, cena = objs["u1"], objs["ambiente"], objs["cena"]

        cena.frame_set(q_ini)
        bpy.context.view_layer.update()
        tela = mod_u1.ponto_no_mundo(u1, "posicao_tela", "centro")
        normal = mod_u1.ponto_no_mundo(u1, "posicao_tela", "normal")
        # Tela de 0,104 m a ~69% da largura do quadro: 0,26 m a 35 mm.
        pos_fim = tela + normal * 0.26 + Vector((0.02, 0.0, 0.015))
        az_fim, r_fim, z_fim = _cil(pos_fim)
        az_fim += 360.0          # continua girando no mesmo sentido (105 -> 292)
        q_orb = q(r["orbita"][1])
        _chave_camera(objs, q(0.30), 180.0, 1.9, 0.80, (0.0, 0.0, 0.42))
        _chave_camera(objs, q_orb, 250.0, 1.35, 0.72, (tela + Vector((-0.08, 0.0, -0.10))))
        # Ultima chave em q_fim-1, CONSTANT: o corte da primeira foto e em q_fim,
        # e duas chaves no mesmo quadro fariam a foto sobrescrever o close da
        # tela (medido: o quadro 350 saiu apontando para os cabecotes).
        _chave_camera(objs, q_fim - 1, az_fim, r_fim, z_fim, tela, interp="CONSTANT")
        mod_ambiente.animar_rig(amb, q_ini, q_orb, 190.0, 340.0)
        mod_ambiente.animar_rig(amb, q_orb, q_fim - 1, 340.0, az_fim + 90.0)

        if u1.get("tela") is not None:
            mod_u1.animar_tela(u1, q(r["boot"]), q(r["ui"]), q_fim)
        else:
            print("[coreografia] modelo real sem 'u1_tela_objeto': tela nao animada")


    def _beat5(objs, fator):
        """Tres fotos: cortes secos com flash, closes ancorados no canto inferior
        direito, cada um com um drift lento e a luz mudando de angulo."""
        q_ini, q_fim = quadros_do_beat(5, fator)
        cena, amb, cam = objs["cena"], objs["ambiente"], objs["camera"]
        cena.frame_set(q_ini)
        bpy.context.view_layer.update()
        s = _sujeitos_fotos(objs)
        cortes = [q_em(5, fr, fator) for fr in ROTEIRO[5]["fotos"]] + [q_fim]

        # (sujeito, camera no inicio, deslocamento do drift, lente, rig relativo, energia key, energia rim)
        fotos = [
            (s["cabecotes"], s["cabecotes"] + Vector((-0.22, -0.38, 0.30)), Vector((0.03, 0.02, -0.02)), 60.0, +45.0, 300.0, 550.0),
            (s["porta"], s["porta"] + Vector((0.38, -0.52, 0.18)), Vector((-0.02, 0.02, 0.01)), 50.0, -50.0, 260.0, 650.0),
            (s["mesa"], s["mesa"] + Vector((-0.12, -0.16, 0.86)), Vector((0.02, 0.02, -0.03)), 50.0, +70.0, 240.0, 800.0),
        ]
        luzes = amb["luzes"]
        padrao_key = luzes["key"].data.energy
        padrao_rim = luzes["rim"].data.energy
        rig_luz = amb["rig"]
        # Antes do primeiro corte as energias precisam de chave com o valor
        # padrao, senao a primeira chave extrapola para tras e muda os beats 1-4.
        for luz, val in ((luzes["key"], padrao_key), (luzes["rim"], padrao_rim)):
            luz.data.energy = val
            luz.data.keyframe_insert("energy", frame=cortes[0] - 1)
        objs["_chaves_rig_luz"] = {}
        for i, (sujeito, pos, drift, lente, rig_rel, e_key, e_rim) in enumerate(fotos):
            q_a, q_b = cortes[i], cortes[i + 1] - 1
            for q_, p_ in ((q_a, pos), (q_b, pos + drift)):
                az, raio, z = _cil(p_)
                alvo = _enquadrar(p_, sujeito, lente)
                _chave_camera(objs, q_, az, raio, z, alvo, foco=sujeito, lente=lente,
                              interp="LINEAR" if q_ == q_a else "CONSTANT")
            mod_ambiente.animar_flash(amb, cam, q_a, forca=1.0)
            # Luz da foto: rig girado em relacao a camera (key mais lateral) e
            # rim mais forte; tudo em chave constante, e um corte.
            az_cam = _cil(pos)[0]
            objs["_chaves_rig_luz"][q_a] = az_cam + 90.0 + rig_rel
            for luz, val in ((luzes["key"], e_key), (luzes["rim"], e_rim)):
                luz.data.energy = val
                luz.data.keyframe_insert("energy", frame=q_a)
        # De volta ao padrao no corte do beat 6.
        for luz, val in ((luzes["key"], padrao_key), (luzes["rim"], padrao_rim)):
            luz.data.energy = val
            luz.data.keyframe_insert("energy", frame=q_fim)
            for fc in luz.data.animation_data.action.fcurves:
                for kp in fc.keyframe_points:
                    kp.interpolation = "CONSTANT"
        objs["_q_rig_luz_padrao"] = q_fim


    def _beat6(objs, fator):
        """Corte ao plano geral: U1 sobe, caixa volta pelo chao, U1 entra, espuma
        volta, tampa fecha; camera sobe."""
        r = ROTEIRO[6]
        q_ini, q_fim = quadros_do_beat(6, fator)
        q = lambda fr: q_em(6, fr, fator)  # noqa: E731
        caixa, u1, amb, cabo = objs["caixa"], objs["u1"], objs["ambiente"], objs["cabo"]

        _chave_camera(objs, q_ini, -80.0, 3.0, 1.3, (0.0, 0.0, 0.55), lente=35.0)
        _chave_camera(objs, q_fim, -84.0, 2.8, 2.3, (0.0, 0.0, 0.70))
        mod_ambiente.animar_rig(amb, q_ini, q_fim, 10.0, 6.0)
        _chave_rim_especular(objs, q_ini, 0.0)

        raiz = u1["raiz"]
        z_alto = objs["z_alto_u1"]
        _chave(raiz, q(r["u1_sobe"][0]), (0.0, 0.0, 0.0))
        _chave(raiz, q(r["u1_sobe"][1]), (0.0, 0.0, z_alto))
        _chave(raiz, q(r["u1_desce"][0]), (0.0, 0.0, z_alto))
        _chave(raiz, q(r["u1_desce"][1]), (0.0, 0.0, u1["z_na_caixa"]))
        _interpolar(raiz, q_ini, q_fim)

        corpo = caixa["corpo"]
        _chave(corpo, q(r["caixa_sobe"][0]), (0.0, 0.0, -objs["profundidade_caixa"]))
        _chave(corpo, q(r["caixa_sobe"][1]), (0.0, 0.0, 0.0))
        _interpolar(corpo, q(r["caixa_sobe"][0]), q(r["caixa_sobe"][1]))

        mod_caixa.animar_espuma_voltar(caixa, q(r["espuma"][0]), q(r["espuma"][1]))
        mod_caixa.animar_tampa(caixa, q(r["tampa"][0]), q(r["tampa"][1]), abrir=False, lado=1.0)
        _esconder_entre([caixa["tampa"]], objs["_q_tampa_some"], q(r["tampa"][0]) - 1)

        # Cabo: visivel so do inicio do voo ate o corte do beat 6 (o modulo nao
        # acompanha o U1 subindo, e o plugue no chao apareceria nos beats 1-2).
        visiveis = _esconder_entre(list(cabo["colecao"].all_objects), 1, objs["_q_cabo"][0])
        for obj in visiveis:
            _chave_visivel(obj, q_ini, False)
        # Tela apaga no corte: a maquina volta para a caixa desligada.
        mat = u1.get("materiais", {}).get("tela")
        no = mat.node_tree.nodes.get("ligada") if mat and mat.use_nodes else None
        if no is not None:
            no.outputs[0].default_value = 0.0
            no.outputs[0].keyframe_insert("default_value", frame=q_ini)


    def _beat7(objs, fator):
        """Camera sobe para o eixo da logo, mergulha ate atravessar a tampa;
        corte para a cartela, parented na camera, que entra e fica."""
        r = ROTEIRO[7]
        q_ini, q_fim = quadros_do_beat(7, fator)
        q = lambda fr: q_em(7, fr, fator)  # noqa: E731
        caixa, cartela, cena, cam, amb = objs["caixa"], objs["cartela"], objs["cena"], objs["camera"], objs["ambiente"]
        p = objs["params"]

        cena.frame_set(q_ini)
        bpy.context.view_layer.update()
        logo = caixa["tampa"].matrix_world @ caixa["centro_logo_local"]
        normal = (caixa["tampa"].matrix_world.to_3x3() @ caixa["normal_logo"]).normalized()
        q_topo, q_t = q(r["sobe_para_logo"][1]), q(r["mergulho"][1])
        objs["q_travessia"] = q_t
        # O alvo fica 1 m abaixo da camera, 4 mm para +Y: e o que define o "para
        # cima" do quadro (+Y = logo em pe) sem o Track To degenerar na vertical.
        # O foco fica na logo o mergulho inteiro.
        alto = logo + normal * 1.8
        _chave_camera(objs, q_topo, -90.0, 0.0, alto.z, (0.0, 0.004, alto.z - 1.0), foco=logo,
                      interp="QUAD", easing="EASE_IN")
        baixo = logo + normal * 0.12
        _chave_camera(objs, q_t - 1, -90.0, 0.0, baixo.z, (0.0, 0.004, baixo.z - 1.0), foco=logo,
                      interp="CONSTANT")
        mod_ambiente.animar_rig(amb, q_ini, q_topo, 6.0, 0.0)

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
        mod_cartela.posicionar_cartela(cartela, cam, cartela["distancia"])
        raiz = cartela["raiz"]
        raiz.parent = cam
        raiz.matrix_parent_inverse = Matrix.Identity(4)
        raiz.matrix_basis = Matrix.Translation((0.0, p["cartela_subida"], -cartela["distancia"]))
        bpy.context.view_layer.update()
        foco_cartela = raiz.matrix_world.translation.copy()
        objs["foco"].location = foco_cartela
        objs["foco"].keyframe_insert("location", frame=q_t)
        deriva = 0.12
        _chave_camera(objs, q_fim, 90.0, raio0 + deriva, z_cam, (0.0, raio0 + deriva + dist_alvo, alvo_z),
                      foco=foco_cartela + Vector((0.0, deriva, 0.0)), interp="LINEAR")

        elementos = list(cartela["linhas"]) + ([cartela["logo"]] if cartela.get("logo") else [])
        _esconder_entre(elementos, 1, q_t)
        mod_cartela.animar_cartela(cartela, q_t + 1, q(r["cartela"][1]))


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
        for n in (2, 6):
            a, b = quadros_do_beat(n, fator)
            for f in range(a, b + 1, passo):
                cena.frame_set(f)
                zu = u1["raiz"].matrix_world.translation.z
                zc = caixa["corpo"].matrix_world.translation.z
                fundo = zc + parede
                if zu < fundo - 1e-4:
                    piores["u1_abaixo_do_fundo_m"] = max(piores["u1_abaixo_do_fundo_m"], fundo - zu)
                dentro = 0
                for esp in caixa["espumas"]:
                    p_ = esp.matrix_world.translation
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
                sobre = abs(tm.translation.x) < ext_t[0] / 2.0 + mx.x and abs(tm.translation.y) < ext_t[1] / 2.0 + mx.y
                deslocada = abs(tm.translation.x) > 0.005 or abs(tm.translation.y) > 0.005
                if sobre and not tampa.hide_render:
                    topo_u1 = zu + mx.z
                    if topo_u1 > teto + 1e-3 and zu + mn.z < teto:
                        piores["u1_x_tampa"] += 1
                    elif deslocada and base < topo_u1 - 1e-3 and abs(tm.translation.x) < mx.x + ext_t[0] / 2.0 - 0.02:
                        piores["u1_x_tampa"] += 1
        print("[coreografia] colisoes: U1 abaixo do fundo da caixa = %.4f m; espumas dentro do U1 (pior quadro %s) = %d; quadros com tampa x U1 = %d"
              % (piores["u1_abaixo_do_fundo_m"], piores["quadro_pior"], piores["espumas_no_u1"], piores["u1_x_tampa"]))
        cena.frame_set(1)
        return piores
    return locals()


mod_coreografia = _registrar_modulo('mod_coreografia', _modulo_coreografia())


# ============================================================================
# ASSETS (PNG em base64): logo, tela de boot, interface. Gravados na pasta
# temporaria na hora de rodar e passados como caminho absoluto aos modulos.
# ============================================================================
_ASSETS = {
    'logo_engineprint.png': (
        "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"
        "FmOqLjbJL2be9G//zcySfvvwBvW7+1dW/lbQCRdccMFxzx5ObjAT5PU/LFt3/Emf7z9uOm++azYBuBMAQH1sAACopWzxP+qV"
        "jwnm7964QV12ww03PClYVOqii7Zv073eNea8vMI8XmrR+Ud+zvPGYTNb+rj0lj54752fu1uwsM4666yT1NLG28znfl6d86fO"
        "eddk3hyvP/213AkAoB42AABU1uLf+S/Mm68eWTuydOFtt33iccHC2LFjx4tXE/mZRKtfMJ/yqen3qy72YsiTj+aVHFSJfPDQ"
        "icd+9ODttx8SLIwtW7a8NJHlz5rP+/R5On+1aH4dAEAtbAAAqKTsd/7rHGeST/RXk2V94W033vglwVzbtm3nG9YkuUaJ+nHz"
        "8Nj+96ZxHqXIK0mSZLC51tn8U6qn/mBV1n7zvrvvflQw1zZvvuBUUclnE61f3srxqzzPnQAAKmMDAECpOfqd/+K8kidkTV98"
        "yy03PiCYKxdccNVxx53w1E+aRX//r/mfHXpO+rmnX/fVOY+qLpbJV8/XMbd5rVeU9D64caO6fmVlZVUwV84855yze3rp0+bL"
        "k/uP5/U8ZBMAQFVsAAAoNIe/81+c1/pQT8nlN9984+cErfemN735NLWk3ms+uJ8yn+MLZnkFLqSred63wPOUetx88bvJ6tJv"
        "7du38g1B65255fw39UQ+aT6/EyZ1/k85zyYAgFJsAACImtff+S/Lm38+IkpffevNN35C0DpbtmzZcNLJf/9tWqtrzIf1I2XP"
        "r7poI09+SvlVJerGRMkH9t5zx2cFrbR58xveqpX+mPlyQ//xvIxfFfJsAgAoxAYAgKCy3/n3HxcdQ/k6xwnlterJu2656cbf"
        "FbTC9u3bX76me9eYoekfm4/nJen3Rz0PyJOfdd78/0d7qvdBvfbch/fs2fOUoBXO2vyGdyqlf7s/DMxo/Jl0nk0AAFFsAADI"
        "WZjf+a+WvfbWT934LwWzot508Zt39vpX+5VcYh73qn6OVRdr5Mm3IP8d05/+UU+p/7h79x1fEMzM2VvO7/f3/2v6uO74kT5/"
        "DvJsAgAIYgMAgGPhfue/Wv53b7n5T98l6xfsMA1bt17+95aXj/xcouRdStQr5/z8IU++Tm63JPLBp59+4k8efvjhw4JpUWdt"
        "Pv+3zcfwzrqfY53zpk158/+v/342AQB42AAAMLSov/NfKa/kE3/zjVOu3r//d44IJuaii3b8sO6p/n/C723mfT9mlM+v6qKL"
        "PPl25+VvzaP/Z7mX/NY999zz3wQT0/+7Iole/pjp59/awfOPOwEAONgAALCu7Hf+6xznNW+++Fxy5NnLb7/99kOCxrzxiitO"
        "PPbQ4bf3F/7mjf7Btp4HKfLr/5379SP5yeeNxPS7t/T/U4J79txxi3AnUqPOOOOME3objvuk+TTe1ObxZ6J50dd//2u/j00A"
        "AOvYAADQqd/5r5B/4MjGpYs/c/31TwjGsm3bJa/XSt4jOvlH5g3flL7vfel7X+dzbCrvL9bIj56vg3yl/H81n8qHltTq7+3e"
        "vftbgrGcccYPvaS3YfVWJers9HtdOw/ZBADgYwMA6LiO/s5/cV7Jl5Ij6sLbb//Trwpq2blz58bVVfXjiU6uMW/sBY1dwZpC"
        "PqSred63GedFnjf//Ccq0R/cs+fuewW1nXXWG14pPf058z6+cpLn/1zlE7MJcDqbAEDXsQEAdFinf+e/3OPJkr7w9k9+8hFB"
        "qYsuuvTVeim5Rmn5GfPwZPvfqi56YsiT73Re9AMm9oElSf7z7t27nxWU2rLlgtev6eTPzPs2/M+Jdmz8CubWj9wJAHQeGwBA"
        "R5X9zr//uOgYytc5tjXf66knk9Vkx2233bRXkNM/h/7u7w5dJkpfk2i9XVknTp33fdTPkTz5tuSn8TcDzDOfNP/y4Z4s9f9W"
        "wF8Kgs465w1vNG/YLebdPLHO59BX5fkLkedOAKDT2AAAOojf+a/lWaWTK2+55abbBeu2XnLJKUtH9DvNlz9v/vc9sedVXWyR"
        "J9+VfB0F+f43PqO0+uD3fu9Lb2QRl9m8+bzLEpGPm/dtY/q9up9D+vyFz2thEwDoKDYAgI7hd/5Hyq9KIm+/9dYbPyYdtn37"
        "zgvXtFxjvrzS/G859BzOH/KLnm/X69ZfM1O539mwpH/7nnvu+aZ02Flb3vDT5tL275v3qVf3fazz/i9Unk0AoJPYAAA6hN/5"
        "Hytv/p/6xdtuufE3pUO2br3ypKUNh3+6f5u/eQdOq5KpuughT558Y/kj/cWcLPU+uG/3nZ+Xjjl7yxv+J/PO/Wr/a86fmnk2"
        "AYDOYQMA6JDLrnjrn5jDVfb3OnvlY8S8+eLf3nrLJ98vC67/R/3UUvIvtFb/g2n2cYvwOabIT/e/c09+6vkvakl+a+Ny7w/u"
        "vvvup2WxqbM3v+E/mPa+t+z9G/Xnpgt5NgGAbmEDAOgAfue/2bz57kcOP/v0z62srKzKArn22mt7d9yz51Il6t2m8Ttkfa1R"
        "/X0cTiYHX4uM9jvT4+b9xRL5mf/OOvlZ5JU8I1p9dEn1/uOePXc+LAtm69aty99+6tk/NO282v5+184Du98cK88mANAZbAAA"
        "C47f+Z9M3jy6/YWbjr3STJbm/j/LtXXr5X9v6Zi1dyot79Kiv3cWV6CmkQ/p7PnL+9a1/F0m9UG9dvjj+/fvPyJz7oILLjju"
        "+cPJDYP/+shEz/8u5ROdXP+601/DJgCw4NgAABYYv/M/4bzWe58/dnnHyg03PClzaNu2N/8DUb13m5HgbebhMaO+f1UXLeTJ"
        "k595/r+b//0/Si9/aN++O74qc+iss846SZY23mYmsOfZ3+/q+NX0+ZNoMZsA3AkALDI2AIAFFbvynw7y/uOiYyhf57jg+UeU"
        "PubC2277xOMyB7Zv336CVstv14n0F/6vr9LuUd9H8uQXJb9ofzPA6C/ubtJKPrh/zz2flvWbmtpvy5YtL01k+bOmfaeXtX/U"
        "z508dwIAi44NAGAB8Tv/087LV5VWZhPgxi9JS73pzW8+rad77zPnxE+Zjv9E+99GbX9ssVG1HPLkFy1fR2vyok2/pX5r7fAx"
        "v3/gwEpr72bavPmCU7VK+ov/l8c2Q6pIn0++OG/OCzYBgAXFBgCwYPid/5nln1hSvYtvueXGB6QlzNWyDSed/JK3KlHXmNf3"
        "o+NcOUqRJ7/o+Q6/b8+aK78fE+nd8YITjvnYysrKc9ISZ55zztk9vfRp8zpPHuV9qPP+kc/y5nj9676fTQBg0bABACwQfud/"
        "5vlDSuvLb7vt5s/JDF188RUv02r1GvOqfs508qfEnjdq+6suOsiTJz+fedNvfMt89/eVXvqt++67669khs7ccv6beqI/aV7V"
        "Cf3HfH7TzXMnALB42AAAFkTZ7/zXOZIfPZ8kyZEltXT1rbfe+AmZLrV9+yXbzAzt3T2lLjevZ6mJ9qfm4XMYvhHkF/2/c09+"
        "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"
        "cy36UfPtR/rHnvQe6fVW/3L37t3PypgGGwEjTf4vuOCC455bW3qt0qunKVGneis3jAAAEABJREFUm0sKp5tNh9PMP51uXvfx"
        "fH7dyBeV01e3nKLjpMuvUt+CtW954xH9TnP8VQHQGdwBAHTItdde27tr976/NqP/KTKCKpM98ouXrzPZJz9Svv+Nr5h/fFRp"
        "/UiixRx7jywtHXnULPL/WubQmRdc8N1La+o0pfubAsnpZrpxmtm8ON20/+Wm/bXmHpw/7ek3xinHNu6iu4l6qO8o82l+5eAD"
        "+15lvkwEQCewAQB0yLYdl7zFjPY3FE3mRrmSQL6b+dSo5SxSPkkSZ7Jdktc60Q+ar1Z6IisbNug77rrrrr+TDrjgggtefHhN"
        "fkSvyVbzLm01s5DXm2/3/MXKqOct+enly36OquarHEPmub7Y+z6r9pktuSsO3r/vkwKgE9gAADrk4u2X3GamE9ulptikkfxi"
        "56tM8slXyidm4+1gFxf8ZdY3BA6bDQFxNwT6/8bP7Wx/7sctxxb6OYrV1wTqq13frQ8+sG+nAOgENgCAjtixY8epa3rpL2R9"
        "rpCfzI1yJYE8eV8X8hVyiRZ9vzILfnOle2VtbeNd+/f/2bcFpX7AbAgct74hkGw1XdVW8z6+3ryfPc679uSLyumrW07RsUiT"
        "9VSpb1rtm0S7KtSnl3vJK/fv3/8VAbDw2AAAOmLb9kt+wxz+xzqZKpM98uSLch3JJ2Y4vd8kV7QyC/7nWfA3pb8hcMzhtR/p"
        "rW8G9O8SyO4QiOHntrn8OOX0NbXopr7J12ei//bA/fveLwAWHhsAQAfs3Llz45E19U0zOXiBP5kb5UoC+W7mU/3HNX/nfTHz"
        "PfUX5tu/t6GX/NHdd9/9dcHEnX32G18mvbW3a0l+Vol6bfr9Lp1/9vOnmQ+1I/S4iWPINOqzHzdZX+x9n3b7Sup76pmnvvWS"
        "L33pS88LgIXGBgDQARdtv/Qfm6H+96o+PzZpJL+YeV2wGJhkvXOa/zvzTv1R0ks+svfuu+8TzMxZ5557rujeTyst/9A8fBE/"
        "983lxymnL7SY9etpUpXF8yTqq7vIH7c++/Ek6jPF/MyB+/d9WAAstJ4AWHg9Fb/1P51U2Ed/UlM0uZhVPpQjP1renlxWnTTP"
        "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"
        "U+hYtIisWp+vbPE5jfZVWSQ3Wd+8ti8R4T8JCEwRGwDAlGzdeskp5vC2/tf+4JgKTWZigyl58lXzKpFfFQAo0+tdq3XxlVz/"
        "sc1+HOufivqzcerzjVpf6DjJ9lHfureee+65pwiAqah+zw+AWi666KKTtd64SSl9gtbJJq3kp8xg957QYBkSG1TJk6+ZX9mz"
        "+843CQBU8Pqzzvmc6Ti2Vu1/ihbhfXXKGace6pvv+sy/fkC0/MHSUvLM4cNLhzZsWH1m7969TwiAxrEBgE7bsmXLhuNOOWXT"
        "8UeObDIPzUJ9eVN/sW6WT+br3iZZ0pukfzSPzf/Mv8kmcz31BG2+Ns9fz5ifIvMc8z9lvk7M95Xqf3+5X37VxVoMefLj5nsi"
        "b969+87bBAAqOPPMzTu06t1aZ5E4qlEXpW2vb9xFd5P1Trr8KdS3ar56xnz1jDke0ko/o7Q8o5Uyj/UhU+PRrxPzteo9Y57/"
        "jMkd6j8n6T+31zvUS/Qza2vJM73ehkPPbUyeOf2lL33quuuuWxOgo9gAwFy46qqrlv7mb46ceNxxz29aW1s2C+zk6FX1nlmc"
        "r5nFeU9tGizKTzCjxaaeWYQnWk4wY8dg0W4W9GbBbgYF81ifcPSYmMe9Df6iapSd7WnnU+T8aRjfAAAQAElEQVSVJEniTDrI"
        "O8+//9577jin/20BgGrU6884Z6/qyTllizz7cax/b+IYqi/3oidc3yK2L1T+Ir2fJe07kpjNgp7ZXDCbCmYjwRyT5Bkznzxk"
        "/vkZ84T1zYQls5lgYmZTQT9jnj/YZJD1jYU1s7HQWzPPWVo99Pzzxz3zmtf8/afZWMA8YAMAjdu2bdsLk2OP3XTMaq+/yM4W"
        "6utX1dWmnlmgJ+br/iL96KK9t75o1+tXzo9eaZfBlXZR61fdX9wv1++8i9iDQJqZx3xocCQ/Wr6OhcmLvnLPPXf+qQBADa8/"
        "65y3mBHsBmlIlcVeqmgRN4n6mtCG9jVZfpX6pv1+ptrfPn14fTNBmU0C7dyxcMgMys+YAs3mgqzfpdDfYEj/zWy49TcYnlFr"
        "+tDS0tIzSdLfWFg7dPjwCc/s3/9nT8v630oEmsEGAErt2HHZL0hPTk+vqJsuyFw5lxOS9Svq5gp7kl5p1y+uu1NrG2Wnt6v5"
        "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"
        "s0lXVpG4x7QWpdkAANB6pv87ye7Hjx7SxczRZ2RXgMXrz735gr1I9Pt3bW92e4vW0OJ0OF8Rd7Hn5YZ5Z/EoVn+ei4u7WMvm"
        "A8Pxz8s5m/tpXuvc0R4OnOHHX2wr72KAtwrMhhm7ff7Rm484b5A7TmXty47u/Mif79jzFqu9g2fa+fDT3fF6MGGT9APNDaPD"
        "t9FvX7Z4d9vnvsHZfDE7Ou1T1ma+k/deeHpepXmVvV5ns0tl7dLaf0H25oWV1/ZpZ+Wt9wEowgYAylmduLXGDXfm1jF2xT/r"
        "RCXXifrF2J1p6Mq/P3jkrsz7x1D5IrnOPivf7vy9wUO5V3TtUT565d852pOdbNKirElB7Mq/2IP94Kiyj2t4dN9Gd9KQfZ5Z"
        "PW75KlC+ewyNeSp3vviDdviDCJfvX5mPnm7D88I/ofwrSWVX/t2j+zaLc8zOD/88EdHi3yYYLt//HL16hrXIKQIALWe6u1Ps"
        "fly8cdsfF9xxw+7/B/13aIAZVJR1l+HNefv2bFW4yXq0pGyR6d4BcLS69JiLuwOR3T6xx93huyHD92f4+rzxN9c+bzwfbirY"
        "i8Ms5xzFHseyccffhB4evXFtWIA1/7DveLTvGNBOXgbvez6f/cPRZ2rrGHy6FLRTlDtOO2+nn9OR9rlvsHPlX9nzvsA80f5g"
        "BjnnaLdvkEvPK7d9bjvtF+RsDtg/N6H8cP4DFGMDAOV0oDMXEWexa3XqzhXXwOAw7NzF7kwlMFiI2Islt3wRZ4ffKV/nOulh"
        "O+xR2Ovs16WPxZ0UOIt/7e5826N7/sq/cvtip3xvMHfKl8DkQESVTBKCkwbtXYFwyhdn8uGWr63y3EHMLV8qlC+58yN/vtjl"
        "588X8QdN/3yxBm3rA3fPR3GvJISu/Hsvb/i5+Z+fWOeHeJ+fSMEk1CnfmoTkyhe7/Ff/yVWyJADQUn9y1VVLprs6ddhhuh31"
        "sL9WVv/nzwfScWDYf1tPcOYfIuJeoRVxN9HdzVev2x68PHcx6Nwmrr0r//m4Mw6nOad9Yo/vIvbme5rTofF32D6x3w2JX/lX"
        "kt+tttqnrMdanMW/PX7Zx1z7rPmHfQeA2750HiJW+/L5tEVqUKP1dKcc+8q2+O0U7S1+/fa5749/J2DoDXau/Is48z3/YoF4"
        "L0useYSIfV5Z7Rzks2mWfcX/6FHb7499Ion74+Tnh/Mn+/UBBdgAQDm7711/POx9rE7PGsQkWwy5j7POM+vidKR88TrV7AnO"
        "lVZ/8Tws3z3KcOxIB0kJdNqSL1/E2wHWWTnK6mztzZC0fJHh6xuyJysi7qRFsslB7Mp//oqxdsYwsQdjyd5Pd3C2y7c+4Fz5"
        "yirfH8Ts8p3TQexBXweeoNwPxD1f7EFS3PPDHnzduYA16ObK9z9PexB2P08VOD+suYbz+VnvuPP5DR/rUPkqUL41GcmVL1b5"
        "8oJTX/Kq7xEAaKlTX/HQ95juapPd/7rjiwyPzmJIsn47HWfc+YW48wPx5g/ibsrnrmynA5aIszhyNm+9RWP+yrgfV844LNbi"
        "3h5vsyvkko2noUW/UoH2SfpuuON77vVpsVbB+fY57RKpdeXfaV92zG1GDF6osl+wdi8yZO2z5juSz6evNx2vw+2zFr/Dt9Md"
        "b8Ua53PtC7zB/vnkz8/883H4srx8+g2nfcq76DB4nnLfMLHvALDnN8qb99h5Z34z/EEBirEBgHLDTs4dDNJFb/qEar/zL24n"
        "Hy0/O9qDztGn60j5+U7aLV+cUbzK7/w7j+3yh5sdw1E+X37uaJc3GKSGx2wn3Z8UZOW7g6A9dvlHu3xn0pAr33qDcuXnBzG3"
        "fD9efOU/d74EPr/hYO6fLypyvtgnlPcKQ5+fW777eXrNz32eYpXnH7PyRar8zr+KnS92K+zy+l8kS68UAGip5WT5letfDBel"
        "IsNFzXBcsPtzb75gLxJzEwb7YZY7+vTA4nQ4n5BhPuVe8Zfg4thePMsw7g0QTn/uj7dZtc7mvtM+92gPN87woNy8277BM3XW"
        "wGwc89vnbnIcbZe22pcrwGtfdoxd+c/aJ7n5ixrUlG3KZ9XYeffOBvHap/2X57RLtJv353OhN9i58q/FeX25iwX2y/HbmZ5X"
        "g6SzSNfWZpdyNy/cF2RvXljvj7ZPOysv7s/P0SewtEM5zhKUK+rsrMHHGRwkfuVfwvFI+W6n7A4eXueuJH/Ubj2SjSGiJNTp"
        "S0H57hVd++jNErzJigTKzyYtzuRF20exZhH2YC+5yYKIP4SI+Dv0+fK9SUiwfPfolu9/XO4VnuAH4JwvXvkqHWzjn5871wmc"
        "UPYg6h+D5ftHu3z383QH98B5kvs8vfIlf8ydL2Idvc9PK3W6AEBLJcna6W5HKu64IP644S4KRYc21dNVkN3d2/MLe7NVO4vA"
        "NOccB7n1o72YtReRYh/tuD1OZuOn2z6vncP2ifX6vPFXsrxVSzaaDTcVsrzKr4Kz9g1fZn5TI37l32LPB1Rgc8Vu5+CFDo/+"
        "vCVbtQ7a5R6DT5dsEZxvn3KnKyrfTjsfvrMhrViG59PwvFL2uB6YJ6pAO+1/sNs3yKXnlds+r53WC3I2B4Kni5W35znDeUoi"
        "QBk2AFDO66XDv5PvDRJW5+lu5KuCwUIC5VuDtVWPs/iyJg252/ntUdjr7Nd55WeDrQpPLrzJSdGV3OLyiyYvOlB+ZJLgPXYn"
        "C+7j0ORjuFOeK98dxAKnQaR8yQ2K2v4AnPMlVL53vgxfr+TPF+/9DZ6PYl0JsAdjUcHzwxmLQ+WL/3kqdxLpn4/e+RGalGr3"
        "jBH7yoz3/rIBAKC1euublFlHmvbXw0WT1b8P+/v1pDc+OLmsf8xt1kc30UW8bvto3l/02nlrESnhuLiLUXsxnrbPHt9F7M33"
        "NKe98ddtn3fMbSp4m/fivT/D9ll5LeLeAZAfx8Lty8bfKlf+s/aJN3/JBlg1qNF6ulOOfWXbWf06L0+746nOt9MZ/532WUdr"
        "fHY29e35Xzo+59oXegPsxfignYN8Nu3K5jvit1PsTQ+rvdbH7V/5t9uZ5fqPWdqhHGcJqrEXpaLcRaRkiyH3cdZ5Dvu69HHa"
        "t4tkX1iTBPsJ9mJcxFs8D8vTTvkyHDvSQdIvX4bfd6/kircDbJfvTk5y5Q/Ls983keiVf8kmB+Er/0q0P8hr7U4SdOjKvPLG"
        "Jrv89P3OylfD8pVVvj+IZeU7H/+wfAmWnw2uofJVoHztxa2KhnOB/OeXPSFwvtiDsPV5qsD54Q7O4kwOxJ4kiHVlwZ5cOeWr"
        "3PkRPl+cMyb7/HLlCxsAAFrLdFOnu+OLDI9uPyziXLkUf34xyImIWIta50rtcDzJ+t+jT7cGFBFnQHY2V51Fozj5cFw547D9"
        "+uzx1r0ybo2v/qI/2L5B8eljbW0Ce4ty8RbrbvtkOL4plc8744v7Bnnty45lV/7TeYtS+bzY8x1R9tOt6U42Xg/Ph1z7Bu+U"
        "jrVPO/Mrp32BN9g/n/z5mXs+2u3LvwH59nkXHdLnDU8s67wfJPLtE2c+5MxnIj8/R18QdwCgHBsAqGY46O49iJkAABAASURB"
        "VPQfZItHUfbidtCJidv52YOE07ennbG1aHLKF2uwDpafv9KaK98axf1NglD5yn6cK384akj0yr9ztMv3BnNxF9/+lX8JDPL2"
        "2OUf8+VLpHzrDcqVn72/2VHcQdqJ2+VLpPzsFebLzwa9/JX/fHHuN6zzxinf/fzy5WefX24O5X2eYpXnH7PyRar8zn9WkbhH"
        "sT5Huzz/8dHy2AAA0F49OT1dhNmLpaw/9+YL9iLR79+1vdkd2BQVb3E6XLyKu9jzcsO8s3i0+9tcXNzFWjYfGI5/Xs7Z3E/z"
        "WueO9nDgDD/+Ylt5FwOG8xHr5eXa5x+9+YjzBrnjVPDKvzM/8uc79rzFaq9ki9Q0F366O14PJmySfqC5YXT4NvrtyxbvuSv/"
        "1hvsXPnXkm+fsjbznbz3wtPzSrI7BtLX62x2qaxdWvsvyN68sPLaPu2svFXfsBzn56ePpR3KcZagGnsrUvzBQQKDQ9YJ2p2o"
        "X4zdmYau/PuDR+7KvH8MlS+S6+yz8u3O3xs8lHtF1x7lo1f+naNyJgNKuTvpziaKU741+FnH4eBpHd230Z00SDpoWPW45atA"
        "+e4xNOYpa9DPyo98wM4kIFS+f2U+8PKGL9M+kcQ9DyVwVKHy/aP7NotzzM4P/zwR0eLfPhou3/8cvXpEiid9Tvnq5fynAAG0"
        "0UPv/oFNpqN6uT8uuOOG3f8P+u/QACOD/KDs2Oa8tvKqcJP1aEnZItO9A+BodekxF3cHIrt9YvfX2Xg8HOeGr88bf3Pt88Zz"
        "axPYvrLtvkDlvjxxxx1/Ezr36w/OG+TOP+w7Hu07BrSTl8H7ns9n/3D0mdo6Bp8uBe0U5Y7Tztvp53Skfe4b7Fz5V/a8LzBP"
        "tD+YQc452u0b5NLzym2f2077BTmbA/bPTSjvzXOG8yHnBOQOAJRjAwDVWJ26c8U1MDj4O+FOn+n0/jIcrOwnOFdcA52zc6U1"
        "7fyGcRXt7I+2QyR3RVd7i3/t7nzbnWv+yr+y13Je+d5g7pQvgcmBiCqZJAQnDdq7AuGUL87kwy1fW+W5g5hbvlQoX3LnR/58"
        "scvPny/iD5r++WIN2tYH7p6PYl0JsAdjUcHzw5pj5D4/sc4P8T4/kYJJqFO+NQnJlS+B8kWCd4oc/dyWTvt7p50qANA2x6rv"
        "M/3UUjYuWP25ZPOBdBwY9t/WE4b9/YB7hVbE3UR3N1+9bvto3l/0WovF3JX/fNwZh9Oc0z6xx3cRe/M9zenQ+Dtsn9jvhsSv"
        "/Fs5O+9vWljjkL0JcLRd7jHXPmv+Yd8B4LYvnYeI1b58XoaLVLGOWT47xi4eDHLKPvrtc98f/07A0BvsXPkXceZ7/sUC8V6W"
        "WPMIEfu8sto5yGfTLKt9g4S23x/7RBJx5j1+fjh/yk1gdO4hSztUwVmCauxBTLLFkPs46zyzLkq7fXu/LH+R5D3BudLqL56H"
        "5btHGY4d6SApgU5b8uWLeDvAOitHKat8nS9/8L4o531Ky/cH9WwTw54U6NDOe/p+e+Ur67Fd/nATxnp/s/JFhh9Arnxlle8P"
        "Ynb5Yu9RiD3o68AT3J1p73wRa5AU9/ywB193LmANurny/c/THoTdz1MFzg9rruF8ftY77nx+w8c6VL4KlG9NRnLli1W+FJSf"
        "fZ5Jb41fAwDQOonWr7THBWcxJFm/nY4zzniRdeciw0W7O87bm/K5K9vpgCXWUcTdXPUWjfkr435cOeOwWIt7e7zN+m/JxtPQ"
        "ol+pQPskfTfc8T33+rRYq+B8+5x2idS68u+0LzvmNiMGL1TZL1i7Fxmy9lnzHcnn09ebjtfh9lmL3+Hb6Y63Yo3zufYF3mD/"
        "fPLnZ/75OHxZXj79htM+5V10GDxPuW+Y2HcA2PMb5c177Lwzv7FfmJUX6yF3AKAKNgBQjb2olGzRlXXu3pVcEfdx+g2x/kFr"
        "q1PNdm6dxbhTfr6TdssXZxTXTvkSKd96bJc/3OwYjvL58nNHu7zBIDU8Zjvp/qQgK98dBO2xyz/a5TuThlz51huUKz8/iLnl"
        "+3G7fImUnz0OfX7Dwdw/X1TkfLFPKO8Vhj4/t3z38/San/s8xSrPP2bli7PYd8svnpTmPke7PP+xU372eeqEPwQIoIUSdbq9"
        "Vh32w2LNF+xFYm7CYD8MbIqKtzj1NluHHauXG+a9xbG9eJZh3BsgnP7cH2+zap3Nfad97tEebpzhQbl5t32DZ+qsgdk45rfP"
        "3eQ42i5ttS9XgNe+7Bi78p+1T3LzFzWoKduUz6qx8+6dDeK1T/svz2mXaDfvz+dCb7Bz5V+L8/pyFwvsl+O3Mz2vBklnka6t"
        "zS7lbl64L8jevLDeH22fdlZe3J+f/ITJ+rYMqmFphwo4S1CNPzjYnefgKLlBwhu7Qp2p1ym7g4fXuSvJH7Vbj2RjiCgJdfpS"
        "UL5Vj3aP3izBm6xIoPxs0hK6kute+bfL19b7fVRo0pCV700etFuPMwkJlu8e3fL9j0sNP0cd/oCdY658lQ628c/PnesETih7"
        "EPWPwfL9o12++3m6g3vgPMl9nl75kj/mzhexjt7nl7tt0z8eLZcNAACtkyg5PT9uuItC0aFN9XQVY3f39vwitBkqw8VvrmMd"
        "5NaP9mLWXkSKfbTj9jiZjZ/DcU/s/jobjYbj3PD1eeOvZHmrlmw0G24qZHmVXwVn7Ru+zPymRvzKv8WeD6jA5ordzsELHR79"
        "eYu1KNWDmuxj8OmSLYLz7VPudEXl22nnw3c2iDNdcK78K3tcD8wTVaCd9j/Y7Rvk0vPKbZ/XTusFOZsDwdPFytvznOwHxDkB"
        "/W8DVbABgFJ6/XYib5CwOk+njxx2phIYLGTYmdujinvFNbA4tyYNudv57VHY6+zXeeVng60KTy68zrXoSm5x+UWTFx0oPzJJ"
        "8B67kwX3cWjyYS8m3fLdQcwtXwrKl9ygqO0PwHocLt87X4avV/Lni/f+5stPB9NsUFXWMXR+OGNxqHzxP0/lTiL989E7P0KT"
        "Uu2eMWJfmXE/T28S53+eR8t/pQBAyyitTrf792F/f/Rfxem/B/38sL9Py7DG+WwRGtpEF/G67aN5f9Fr561FpITj4i5G7X5Y"
        "RLzxMHt9bk5746/bPu+Y21TwNu/Fe3+G7bPyWiS3eSz+vCrUvmz8rXLlP2ufePOXbIBVgxqtpzvl2Fe2ndWv8/K0O57qfDud"
        "8d9pn3W0xmdnU9+e/6Xjc659oTfAXowP2jnIZ8N0Nt8Rv51ib3pY7bU+bv/Kv93OLOfPl+xmK/vtBAotC1BCmX0i93fwrMXL"
        "oPcZ9nXp47RvF8m+GHZW7hPsxbiIt3gelqed8mU4djirS6t8GX7fvZIr3g6wXb47OcmVPyzPYk9WguWLMylwr/wrq/zs6EwS"
        "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"
        "kOl5vEv3UOdp6J8sa4Y81/FOtSyMqJ/jmX8pJ1jOXvBhfIrRSxksbfwKOY1xDvC6T8W4r+0U45wymEiNzP9Az/2EdmJqBpLp"
        "ONiUIAfAz0B6mX9Qh+COD29r4Xz07ZV1JSoODUFs26GVsy3zr/ed2Ddq362+VsGppDLiSe4bjWbmv+gHRa/Wf1IL2+6fqlek"
        "fKUDdb9o/TJ8DScjnpzMf2vfwNELSe0bOJl/VLbuGzO5sW8AKDVg9w2ND4je1wurMjt6BbRM0NAr5TwUikBuNMUO+s0iP/NP"
        "G7WlF6i/9s4/wAOeYekh9IOX+VfrCmrfCLos5VuVly0Wm7d86H/9qscgEAicFUgIBAJ7Hu/8nkOXbT64uK3783rv+2IMlA8c"
        "44BtO2nuqnKH/Ech28lk1Aj+bDuuxV/3l4w7SGN8x/y1k5R2k/mvmZzyNfffVkexwmsNyd5U9+Z3rH+y+vqZS7/ju838e3fi"
        "UX0dkrPBUH0t1jGA5K7LZncc9ixfrs4c/O5YqHlUxn2aGB8t0FjzvbFu6WUFgf115x+YnN7pCRgF75ud7J9ybYG+Hum+pyjG"
        "2Df0yrSe4BgDstQvZ+LOf3PfsFhrMLTy1e+a+2ZSPg4OO/LJBkb76wbfRyEbEOtqettX+Zp6T3+9zf/P8/H0rMNH37B3FUYg"
        "ENhGvAEQCOxx3PqCQxd0zv/v5m3nnzIR/WkujILtD4ZnnflUFRynvPKVZc0EM39QZmj4oi+hywTvV/gNf5FB0/w1ZDvr3vnX"
        "7bng73N17mTmPxtbV3fcZv4r/VCtH+7ihNTMPSCjFTIzCFEtEb0d94G+EoL6B0e+mtHbfeY/GzmdD6R8lPECpJGuM5BVIBqn"
        "ft1ren33uDgxaSLznyrd6muZuax3xkkssUwrXRl3yH3auvMvOpTq+CBretqnrC+GdUH0Yl3xx31r++/Of1/29AmNfZO8zH82"
        "7Tcz/0bPaf1A9O6+W1EM+6bKh4Z8Wr/09HJ7Yjrzz+uRmyF9qfbdqbrzb/VfHbiEkX0DuW9yhiOflTOLbSEz/0U+vW94nMy+"
        "o+CnkK8vad9VtaXlIz0o9IJY1nWgm3qF1pXd9hi9859HM/9Cvq64fnFw+bsfPHrjyL+/GQgE9gISAoHAnsXrno+Nw1dd81sp"
        "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"
        "7bjz+4VYqSf0vFiVn7v43kRnoNrVdNh7L7pROszuV8rsykX6OWwVN1nF+M/3vyN+dzj1xrtBCDkQuAOAkANi96fqnn/6oV+L"
        "e5P/IAUtNfjJk3OrfJoHUE7Kc7r6mhUfmT7EQvp4Y3OFvQatyKvPIv1RERCDnZy+UyPiOvfMv83PRd6PeXKnlf88yLN2hT3y"
        "9cjP/Av7bAhRPqBH+Y/CrrBHvnpn/mHdsY4nv8SgF3qHyThIj1H7mQsk6mnT7q29PXs8tn+p0Knk0phW+IlsP/oXhOKV3FLN"
        "NNuleoOO09qZf1WgkOsH0dqLOIVULiH8E+kICTB9vcnt+J3531w39gGVuAme8h+L/G3/YRV/qYzr/kHYu3G3thj7l+wfKv7Z"
        "/mVjH7TdtPKvezLb/7rKfypf2YCCjZuiXwFqZ/7L/i9XnNxRVMQNdNxE0a9A9Ctw+hVZ3mQP4aeNG+snoPqVUMSd6AdF3OVq"
        "t/6JflD1C6pZJztU+xXRrsqwR/PMf+xV/jfxAhM3clVG2OdyinYRysX6MupacZfLKe1z9TovfN0ev/3DF+38mnCRELJPGEyE"
        "HBBvf8rl1wy/Vz8mvyuGbubHvcQb7JlfZ/l0RDnoqD8+O7/9p+/nlwZf0d9GOAs7WAih8KbbXlhU7YvqaufkPt7OoZnAfpX/"
        "NMhGmPav78b6bijbS4+dvZ0/eu2yblcmWyqZPXY6g4p/rnmlvO2Pdft6RnscizP/nc1Ch1nRwLpQk5feuHEKoJXxRjMK7fbb"
        "7EbitL1NSFZnjp85/ukbXf1DR2BNv7a+OO+zn/N1pR1Mv1hV3oM681++v5p56O4W3X50Iu7K7PxF+g5Lld88/yovPMZrL3nu"
        "9U8HIWTfcAcAIQfAO556+XOxN/k3yj+QVr7zb5pR/tNsOrqDPv/Mf1AKpJ7DauVEp28+o6ZE19LPP+Zl+mOqMn3APfOPuvKf"
        "00U52NAZQyv/WknJtRgn7YXSAEj7oGsLoab8j4NBiCvyYkR6XCheqr5HA32do/y37Nf5AWqHA0rlP/sH1y75qdqvN2bzlb2x"
        "JpW99A9CuUrtfp1wqfzHPMaU5Uz17vs5FkdXu664SeVfxLkokKifVDAV1mryMcbxWO/qwWD8E1ccozP/Y38FcYWIG5i4EQ3t"
        "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"
        "PiYHsbV8MZHffpV/61mff/Uadf1rJ1i/7T4d6n41qu9Qz/zH1mtvOIRW9sWLbpbzWJz5b1BvhpMvoJKd6G9648YP3HR7IkN0"
        "xQ1qYTavf5DNpRk3XgGcG9Uz//KxaoLavqMbwVT/MLt/aSeH5o6iSccdP7ub5XT/0DYP3d2pvt3Xr5SlFYuvsUf5z5bei+vz"
        "r/7Cp5vR5Avk5J+QcxDuACDkHGL3J3h3J8DwQ5l2Aux9Pw5W9q76N7TrzL+rKHjp20mYHiUFqxAIpSEPEqJMQCkNhfK/Lqge"
        "+48VMdrLG2rVQiuDY/p6cCjtc32I4q1zVv7p69SZ/1jUT75Rns3d1E/hH4x/ebCr/QNqZ/7jpkCzzvzDV/7H+s0ZqxcMubMB"
        "wm7WmX9RcaVCB/jKP0SBcsGUvXE7+SXrKVSUf1ldwv1jc+Z/7wEdd+vbdvKY46ZU/ksmFUwTd9W4sf2LaP96x01Q9ipubP8C"
        "EzeFf8LPVAOA7GdDET+O8h913DTP/Kv4kf2L7D6i8S87fNBn/mXc6Tmk07+g0r+U3Q3szghf+ReBWusXPD+jbtaALqe0h+of"
        "POXftCuYuFF2U8o/VP8y+8w/fOUfpTmCaZjF7zMAteOm1i84dmMt8Mw/IceTAELIOcfuT/K4EwBb0HPmvxw8TWAnSWE/yn9W"
        "ctJtWf7y8TxoyU/o5IvHi2/QLp9+fL/K//l45l+VsHygkqxsBzFPdmS+TSp+imuXI9K+knr209jrBxTH68w/pppFmgQ0XkCT"
        "reKmiDt9u1F8L5BayTfiLqCnn5iMm8m405Noae/7531R9qP97lTKJ/1zw6Idd36/ECv1hJ4Xq/LLi9a9zVJnoNrVdNjD7f+q"
        "pcPsfqXMrlykn8NWcdOo0Olm1BE3nPwTcs7CHQCEnIPs/iSPOwH2Prtjgaiuc8/86x0AlcGGvB/z5E4r/9GOIaEUBmmPfD3y"
        "M/9GocMmv1w+oEf5j8KusEe+no9n/pHKOWaIYvKS/BR2o39BKF7pfapmKv0D+pV/WeHZkdwuTT2l1yyVS+TJl6wQIQGaajuG"
        "Z/431419QCVugqf8l2P4qvIPfS2sJ+NubTH2L9k/VPyz/cvGPmi7aeVf92SmG4Or/KfyteKu7FdQ9CtR9YPZv80XEcpubP/b"
        "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"
        "0ttWHhvK/2Gc+YfeqqCVcTG5LpR/GOU+2gLFYvFDKupwlP/sn90iIOpJbQjoOfOfqlvvAACw7Zn/YudA4BEAQgghy4U7AAgh"
        "hCyX3X8FQCnbcpIJofx7Z/7hKv/A4Z35LxRxqYyLcu7/zH/oP/Mvpf59n/lP1S2ueVFl3AEw58y/3Tkw/Af/GUBCCCGLhQsA"
        "hBBCFktYxfeHEzu/s1oNk8OdsPuvAmBnuK7Sdfh6B/kz1lvn9q5CyV8Nk8ydQgl3rjBfA1nxH69hZ7i/2rtiuML5nK6eTxDp"
        "7uYzFDQO5e+52oT2ptA7kH8CYfB78Hf4wr1CXGHqa896PL4QRUG1X6P/a/+Gz4NlTFeIRRKneiO8L9Tn4X9vACGEELJQAggh"
        "hBBCCCGEEHLs4Q4AQgghhBBCCCFkAXABgBBCCCGEEEIIWQBcACCEEEIIIYQQQhYAFwAIIYQQQgghhJAFwAUAQgghhBBCCCFk"
        "AXABgBBCCCGEEEIIWQBcACCEEEIIIYQQQhYAFwAIIYQQQgghhJAFwAUAQgghhBBCCCFkAXABgBBCCCGEEEIIWQBcACCEEEII"
        "IYQQQhYAFwAIIYQQQgghhJAFwAUAQgghhBBCCCFkAXABgBBCCCGEEEIIWQBcACCEEEIIIYQQQhYAFwAIIYQQQgghhJAFwAUA"
        "QgghhBBCCCFkAXABgBBCCCGEEEIIWQBcACCEEEIIIYQQQhYAFwAIIYQQQgghhJAFwAUAQgghhBBCCCFkAXABgBBCCCGEEEII"
        "WQBcACCEEEIIIYQQQhYAFwAIIYQQQgghhJAFwAUAQgghhBBCCCFkAXABgBBCCCGEEEIIWQBcACCEEEIIIYQQQhYAFwAIIYQQ"
        "QgghhJAFwAUAQgghhBBCCCFkAXABgBBCCCGEEEIIWQBcACCEEEIIIYQQQhYAFwAIIYQQQgghhJAFwAUAQgghhBBCCCFkAXAB"
        "gBBCCCGEEEIIWQBcACCEEEIIIYQQQhYAFwAIIYQQQgghhJAFwAUAQgghhBBCCCFkAXABgBBCCCGEEEIIWQBcACCEEEIIIYQQ"
        "QhYAFwAIIYQQQgghhJAFwAUAQgghhBBCCCFkAXABgBBCCCGEEEIIWQBcACCEEEIIIYQQQhYAFwAIIYQQQgghhJAFwAUAQggh"
        "hBBCCCFkAXABgBBCCCGEEEIIWQBcACCEEEIIIYQQQhYAFwAIIYQQQgghhJAFwAUAQgghhBBCCCFkAXABgBBCCCGEEEIIWQBc"
        "ACCEEEIIIYQQQhYAFwAIIYQQQgghhJAFwAUAQgghhBBCCCFkAXABgBBCCCGEEEIIWQBcACCEEEIIIYQQQhYAFwAIIYQQQggh"
        "hJAFwAUAQgghhBBCCCFkAXABgBBCCCGEEEIIWQBcACCEEEIIIYQQQhYAFwAIIYQQQgghhJAFwAUAQgghhBBCCCFkAXABgBBC"
        "CCGEEEIIWQBcACCEEEIIIYQQQhYAFwAIIYQQQgghhJAFwAUAQgghhBBCCCFkAXABgBBCCCGEEEIIWQBcACCEEEIIIYQQQhYA"
        "FwAIIYQQQgghhJAFwAUAQgghhBBCCCFkAXABgBBCCCGEEEIIWQBcACCEEEIIIYQQQhYAFwAIIYQQQgghhJAFwAUAQgghhBBC"
        "CCFkAXABgBBCCCGEEEIIWQBcACCEEEIIIYQQQhYAFwAIIYQQQgghhJAFwAUAQgghhBBCCCFkAXABgBBCCCGEEEIIWQBcACCE"
        "EEIIIYQQQhYAFwAIIYQQQgghhJAFwAUAQgghhBBCCCFkAXABgBBCCCGEEEIIWQBcACCEEEIIIYQQQhYAFwAIIYQQQgghhJAF"
        "wAUAQgghhBBCCCFkAXABgBBCCCGEEEIIWQBcACCEEEIIIYQQQhYAFwAIIYQQQgghhJAFwAUAQgghhBBCCCFkAXABgBBCCCGE"
        "EEIIWQBcACCEEEIIIYQQQhYAFwAIIYQQQgghhJAFwAUAQgghhBBCCCFkAXABgBBCCCGEEEIIWQBcACCEEEIIIYQQQhYAFwAI"
        "IYQQQgghhJAFwAUAQgghhBBCCCFkAXABgBBCCCGEEEIIWQBcACCEEEIIIYQQQhYAFwAIIYQQQgghhJAFwAUAQgghhBBCCCFk"
        "AXABgBBCCCGEEEIIWQBcACCEEEIIIYQQQhYAFwAIIYQQQgghhJAFwAUAQgghhBBCCCFkAXABgBBCCCGEEEIIWQBcACCEEEII"
        "IYQQQhYAFwAIIYQQQgghhJAFwAUAQgghhBBCCCFkAXABgBBCCCGEEEIIWQBcACCEEEIIIYQQQhYAFwAIIYQQQgghhJAFwAUA"
        "QgghhBBCCCFkAXABgBBCCCGEEEIIWQBcACCEEEIIIYQQQhYAFwAIIYQQQgghhJAFwAUAQgghhBBCCCFkAXABgBBCCCGEEEII"
        "WQBcACCEEEIIIYQQQhYAFwAIIYQQQgghhJAFwAUAQgghhBBCCCFkAXABgBBCCCGEEEIIWQBcACCEEEIIIYQQQhYAFwAIIYQQ"
        "QgghhJAFwAUAQgghhBBCCCFkAXABgBBCCCGEEEIIWQBcACCEEEIIIYQQQhYAFwAIIYQQQgghhJAFwAUAQgghhBBCCCFkAXAB"
        "gBBCCCGEEEIIWQBcACCEEEIIIYQQQhYAFwAIIYQQQgghhJAFwAUAQgghhBBCCCFkAXABgBBCCCGEEEIIWQBcACCEEEIIIYQQ"
        "QhYAFwAIIYQQQgghhJAF8P8DAAD//3Ur4/gAAAAGSURBVAMAbM1T0qeum6kAAAAASUVORK5CYII="
    ),
    'tela_boot.png': (
        "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"
        "g2tfffmrX37+2aeffXF7yfU/g17SnZ4zKJYKunYKudUraufZHG2/ei235nyzeIXmy5Y65BlK42jZ2speaydqTTY22TzNYwqt"
        "xpSOWRCkINCi7e88vPaFUbLcsBX3jnw3m+qQ1boRtbu9V8/uXz/tCEjSjZdO3Js/PamfrjRHlzE/ZeSEZSTx8teyn3Sm7SyU"
        "juzPz/S6cW/hQvr9aaDTpJMsfFD2jicDFPOzZ85kNtNG10+1CHVeMArsUXfTjl+zN5dd/JkzymVj/Pte3E6OuQBpkE3JHP++"
        "eOyefLd9mrmb+KkxzU6+/d2tzWtbm1r+4oZlmtWqaZpHp3Wos3NO0FaDIqHneuHOsYeuaa/bX3Y+aKDNzek7KTS5lbVSyShl"
        "F5yXjLkQ60bltPNOevFsj/2ofi/bFR8ta7Qvvr84sUve0GDuzWTzNhY95sDm5CS5ldVSqTS8xL40v6OsThWeZaZNfu1qEHqV"
        "0c5zN7QXzambKhilyX69eqMHZ20clvZi9YDhqFI2QlXQXiw5UFJHDZwSPAcI9Pmxr04FZWeDbqpMXDStmlktq0pUDuxYqrPz"
        "QbOomdaDF8sTfZYiLZJb3XDqnmOdPLx8MpXg/jGvtD+cyTZ8g8ft3J5mqng2y+z77jPmVtbtet2xzPJ7mPCoF816I7CM0Vdq"
        "BN795rjTczk17jJZz3rZj1NfO+XzGNkWbW/pkQU70OcAQxznUTbh4sHt3/1WjY1+XiiWrezs4dzpRNNvOB/qlna5lY177bjV"
        "sN9HnbWT05rOz91dHmjtA+4O5i/dasWdwHkvdc6m5PihW56+lVLNs4+fQ1kofL+RJl37YVs0CMAe9HmXxfruta1GfaMeRt5o"
        "hnKhWvdrLevh7vveScqt1cLQrUzSrIZ1oyBodoZ32Ov19ibjDyuXv+40R7duOkGq9TXRhtPg5q5P7MVRGGRj/kmSzL3j/Pqd"
        "dscrn2aJ6szBga8LVT/wOubtpfcfnG9pqp73tIc//eMeSZ7PBwL9kRi8flp3vHInGM0KKJStcjHafc8zW1eqrje7WKLlVmub"
        "P/Ceebo2HLhY/jL1A5e8az+2fLnmTS+HV+fyTPP29sJ3rJ/t5GIah15U9EaXnugVN6i3zJtLxqFHe7v69BVs7jI48W+DIY6P"
        "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"
        "BBoAhCLQACAUgQYAoQg0AAhFoAFAKAINAEIRaAAQikADgFAEGgCEItAAIBSBBgChCDQACEWgAUAoAg0AQhFoABCKQAOAUAQa"
        "AIQi0AAgFIEGAKEINAAIRaABQCgCDQBCEWgAEIpAA4BQBBoAhCLQACAUgQYAoQj0Ab///X/evu394he/0IDz7O9///sf/vBf"
        "f/zjf2vnxz//+c+//nXvb3/7Pw0Tn2iYUF3+y1/+8vOf/1wDzr9//OPdr39dffcu1c6Pf/3rX7u7//vdd99pGPoPDRNqp+NP"
        "f/qTBnwU/vzn/zlfdVb2999S53nsQQOAUIxBA4BQBBoAhCLQACAUgQYAoQg0AAhFoAFAKAINAEIRaAAQikADgFAEGgCEItAA"
        "IBSBBgChCDQACEWgAUAoAg0AQhFoABCKQAOAUAQaAIQi0AAgFIEGAKEINAAIRaABQCgCDQBCEWgAEIpAA4BQBBoAhCLQACAU"
        "gQYAoQg0AAhFoAFAKAINAEIRaAAQikADgFAEGgCEItAAIBSBBgChCDQACEWgAUAoAg0AQhFoABCKQAOAUAQaAIQi0AAg1P8D"
        "AAD///pX39UAAAAGSURBVAMA7YBHNO3OxHwAAAAASUVORK5CYII="
    ),
    'tela_ui.png': (
        "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"
    ),
}


def _gravar_assets():
    pasta = _os.path.join(_tempfile.gettempdir(), "anuncio_u1_assets")
    _os.makedirs(pasta, exist_ok=True)
    for nome, b64 in _ASSETS.items():
        caminho = _os.path.join(pasta, nome)
        with open(caminho, "wb") as f:
            f.write(_base64.b64decode(b64))
    return pasta


def _limpar_cena_de_fabrica():
    """So na cena padrao do Blender (Cube, Light, Camera e nada mais): tira os
    tres para nao aparecerem no anuncio. Qualquer outra cena fica intacta."""
    import bpy
    nomes = sorted(o.name for o in bpy.data.objects)
    if nomes == ["Camera", "Cube", "Light"]:
        for n in nomes:
            bpy.data.objects.remove(bpy.data.objects[n], do_unlink=True)
        print("[anuncio] cena de fabrica: Cube/Light/Camera removidos")


def main():
    import bpy
    _limpar_cena_de_fabrica()
    pasta_assets = _gravar_assets()
    params = {
        "u1_nome": U1_NOME,
        "u1_rotacao_z": U1_ROTACAO_Z,
        "u1_tela": U1_TELA,
        "u1_tomada": U1_TOMADA,
        "u1_botao": U1_BOTAO,
        "u1_tela_objeto": U1_TELA_OBJETO,
        "u1_botao_objeto": U1_BOTAO_OBJETO,
        "u1_led_objeto": U1_LED_OBJETO,
        "duracao_s": float(DURACAO_S),
        "cor_caixa": COR_CAIXA,
        "pasta_assets": pasta_assets,
    }
    objs = mod_coreografia.construir_tudo(params)
    mod_coreografia.coreografar(objs)
    mod_coreografia.conferir_colisoes(objs, passo=3)
    largura, altura = RESOLUCAO
    pasta_saida = _os.path.dirname(bpy.data.filepath) if bpy.data.filepath else _os.path.expanduser("~")
    mod_coreografia.configurar_render(objs, largura, altura, AMOSTRAS, video=False,
                                      caminho_saida=_os.path.join(pasta_saida, "anuncio_u1_quadros", "quadro_"))
    print("[anuncio] pronto: %d quadros, camera '%s', saida em %s" % (
        objs["cena"].frame_end, objs["camera"].name, objs["cena"].render.filepath))
    if SALVAR_BLEND:
        caminho = _os.path.join(pasta_saida, "anuncio_u1.blend")
        try:
            bpy.ops.wm.save_as_mainfile(filepath=caminho, copy=True)
            print("[anuncio] .blend gravado em", caminho)
        except RuntimeError as e:
            print("[anuncio] nao foi possivel gravar o .blend:", e)
    return objs


if __name__ == "__main__":
    main()
