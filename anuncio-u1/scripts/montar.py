# Monta o ARQUIVO UNICO do anuncio: scripts/anuncio_u1.py.
#
# Concatena, nesta ordem, mod_ambiente, mod_caixa, mod_u1, mod_cabo,
# mod_cartela, mod_som e mod_coreografia, mais os PNGs pequenos de assets/
# em base64 e um main(). O cliente cola o resultado na aba Scripting do
# Blender e roda.
#
# POR QUE nao e uma concatenacao crua: os modulos repetem nomes de topo
# (NOME, PARAMS_PADRAO, limpar_colecao, _colecao, _suavizar com assinaturas
# diferentes...). Colados num so namespace, o ultimo venceria e o primeiro
# quebraria. Cada modulo entra INTEIRO e legivel, mas indentado dentro de uma
# funcao que devolve locals(); o resultado vira um modulo em sys.modules com
# o nome original, e os "import mod_x" da coreografia continuam funcionando
# sem tocar em disco. Funcoes aninhadas enxergam os nomes do modulo por
# closure, inclusive os definidos depois delas - a mesma semantica de modulo.
#
# POR QUE ha dois tipos de asset:
# - Os PEQUENOS (logo e as duas telas, ~140 kB) vao em base64: na aba
#   Scripting nao existe __file__ nem pasta assets/, e o main grava os tres
#   na pasta temporaria do sistema e passa caminhos absolutos aos modulos.
# - Os GRANDES (a impressora da Meshy, 25,8 MB, e as sete texturas da caixa,
#   ~6,6 MB) ficam EXTERNOS numa pasta assets/ ao lado do .blend do cliente:
#   o GLB passa de qualquer criterio de embutir (28 MB em base64 dentro de
#   um texto do Blender). O main() acha a pasta sozinho (ver _pasta_assets
#   no RODAPE) e passa os caminhos ABSOLUTOS aos modulos pelos params que
#   eles ja aceitam ('arquivo_impressora' do u1; 'texturas' e 'etiqueta' da
#   caixa). Tem de ser absoluto: o _caminho_asset dos modulos resolve nome
#   relativo por __file__, e na aba Scripting __file__ e '<pasta do
#   .blend>/<nome do texto>' - a pasta-mae do .blend nao e onde o cliente pos
#   os assets. A lista ASSETS_EXTERNOS e a UNICA fonte: vai escrita no
#   arquivo unico (o main exige exatamente ela) e empacotar.py monta o zip
#   com exatamente ela.
#
# POR QUE o som entra depois do render: mod_ambiente.configurar_render
# (video=True) escreve ffmpeg.audio_codec = "NONE"; montar_no_vse escreve
# "AAC". Chamado antes, o render sai mudo (cabecalho do mod_som).
#
# Uso: python3 scripts/montar.py   (Python comum; nao precisa do Blender)
#      python3 scripts/empacotar.py  remonta e gera saida/anuncio_u1_pacote.zip

import base64
import os
import textwrap

AQUI = os.path.dirname(os.path.abspath(__file__))
RAIZ = os.path.dirname(AQUI)
# mod_som antes da coreografia so por ordem de leitura: nao importa nenhum
# outro modulo (numpy, wave, os, math; bpy so dentro de montar_no_vse).
MODULOS = ("mod_ambiente", "mod_caixa", "mod_u1", "mod_cabo", "mod_cartela", "mod_som", "mod_coreografia")
# Embutidos em base64 (pequenos).
ASSETS = ("logo_engineprint.png", "tela_boot.png", "tela_ui.png")
# Externos: viajam em assets/ ao lado do .blend do cliente. Exatamente estes,
# nem mais (4k, .bak, previas) nem menos: o main() recusa rodar sem qualquer
# um deles e empacotar.py zipa so estes.
ASSETS_EXTERNOS = (
    "impressora_limpa.glb",          # mod_u1: a impressora da Meshy, limpa (393.991 tris, 3 texturas dentro)
    "caixa_cor_2k.png",              # mod_caixa: bake da Meshy para a geometria limpa, 2048^2
    "caixa_normal_2k.png",
    "caixa_rugosidade_2k.png",
    "caixa_etiqueta_cor.png",        # mod_caixa: etiqueta pendurada (malha nos bytes de um PNG + 3 texturas 1024^2)
    "caixa_etiqueta_normal.png",
    "caixa_etiqueta_rugosidade.png",
    "caixa_etiqueta_malha.png",
)
NOME_SCRIPT = "anuncio_u1.py"
DESTINO = os.path.join(AQUI, NOME_SCRIPT)

CABECALHO = '''# ============================================================================
# ANUNCIO SNAPMAKER U1 - EnginePrint - arquivo unico para o Blender (4.2+)
# ============================================================================
#
# O QUE E: um script que monta a cena inteira do anuncio (caixa de papelao
# da Meshy, impressora da Meshy, espuma, cabo, luzes, camera, cartela),
# coreografa os 750 quadros (25 s a 30 fps; 20 e 15 s sao presets), gera a
# trilha e os efeitos e configura o render (EEVEE Next, 1080x1920 vertical,
# AgX, H.264 + AAC). O que sai do Render Animation e o MP4 final, com som.
#
# PASSO A PASSO
#
#   1. Salve um .blend numa pasta sua (File > Save; pode ser a cena vazia de
#      fabrica). E ao lado DESSE arquivo que o script procura os assets.
#   2. Copie a pasta assets/ do pacote (anuncio_u1_pacote.zip) para a MESMA
#      pasta do .blend. Ela tem a impressora (impressora_limpa.glb, 25,8 MB) e
#      as sete texturas da caixa (caixa_*.png). Fica assim:
#          minha_pasta/
#              cena.blend
#              assets/impressora_limpa.glb
#              assets/caixa_cor_2k.png ... caixa_etiqueta_malha.png
#   3. Aba Scripting > New > cole este arquivo inteiro (ou Text > Open e
#      aponte para anuncio_u1.py).
#   4. Ajuste o bloco PARAMETROS logo abaixo, se quiser (o padrao ja e o
#      anuncio aprovado: 25 s, com som, caixa some por baixo).
#   5. Run Script (Alt+P). Leva um a dois minutos: importa a impressora,
#      monta a colecao ANUNCIO, escreve os sete beats, sintetiza o som
#      (~2 s), poe as faixas no VSE e grava anuncio_u1.blend ao lado do seu
#      .blend. O que o script achou e decidiu sai no console (Window >
#      Toggle System Console no Windows).
#   6. Render > Render Animation. Sai anuncio_u1.mp4 (H.264 + AAC) ao lado
#      do seu .blend. Na RTX 4050, a 1080x1920 com 64 amostras, conte de 30 a
#      60 minutos. Nao ha passo de mixagem: o audio ja vai no MP4.
#
# Se a pasta assets/ nao for encontrada o script para na hora, ANTES de
# mexer na cena, e imprime onde procurou e o que esperava achar. Ele procura,
# nesta ordem: PASTA_ASSETS (se preenchida), 'assets' ao lado do .blend
# salvo, 'assets' na pasta de trabalho do Blender, 'assets' ao lado do texto
# anuncio_u1.py (se aberto de um arquivo) e ao lado do proprio script (se
# rodado por blender -P).
#
# PARAMETROS (bloco abaixo)
#
#   PASTA_ASSETS     ""     caminho da pasta assets/; "" = procurar (acima)
#   COM_SOM          True   trilha + efeitos no VSE e MP4 com AAC; False =
#                           PNG por quadro em anuncio_u1_quadros/, mudo
#   TRILHA_EXTERNA   ""     WAV licenciado que substitui a trilha sintetizada
#                           (caminho absoluto; ou assets/trilha_externa.wav).
#                           A trilha daqui e PROVISORIA, sintetizada: um
#                           anuncio de verdade usa musica licenciada.
#   DURACAO_S        25     25 (padrao), 20 ou 15 (15 fica frenetico)
#   CAIXA_SOME       True   caixa some por baixo no beat 2 e volta no 6;
#                           False = o U1 para no ar na frente dela
#   ESPUMA_SOME_NOS_CLOSES True  flocos que sobraram somem nos beats 3-5
#   ESCONDER_RESTO   False  True tira do render objetos SEUS fora de ANUNCIO
#   COR_CAIXA        "clara" so por compatibilidade: a cor vem do bake
#   RESOLUCAO        (1080, 1920)  9:16 vertical
#   AMOSTRAS         64     amostras do EEVEE no render final
#   SALVAR_BLEND     True   grava anuncio_u1.blend ao lado do seu .blend
#   U1_NOME e os U1_*  ""   so se quiser trocar a impressora da Meshy por um
#                           modelo seu ja na cena: nome do objeto/colecao,
#                           rotacao para a frente apontar a -Y, pontos da
#                           tela/tomada/botao e objetos que acendem/afundam
#
# NAO HA CHAO: os objetos flutuam num vazio com o fundo em gradiente
# preto/rose mesclado. A caixa sobe de fora do quadro por baixo, some por
# baixo no beat 2 e volta por baixo no beat 6; o U1 para no ar. A caixa NAO
# tem logo: a logo EnginePrint aparece pela primeira vez na cartela.
#
# Rodar de novo nao duplica nada: cada modulo apaga a propria colecao antes,
# as faixas de som anteriores saem do VSE e o SEU modelo (U1_NOME) volta a
# pose original antes de ser medido de novo.
# ============================================================================

# ---------------------------- PARAMETROS -----------------------------------
PASTA_ASSETS = ""            # "" = procurar 'assets' ao lado do .blend, na pasta de trabalho, ao lado do texto
COM_SOM = True               # True: trilha + efeitos no VSE, MP4 com AAC; False: PNG por quadro, mudo
TRILHA_EXTERNA = ""          # caminho absoluto de um WAV licenciado, ou "" (trilha sintetizada provisoria)
DURACAO_S = 25               # 25 (padrao), 20 ou 15 (presets; 15 fica frenetico)
CAIXA_SOME = True            # True: caixa some por baixo no beat 2; False: U1 para no ar na frente dela
ESPUMA_SOME_NOS_CLOSES = True  # True: os flocos de espuma que sobraram em volta somem nos beats 3-5 (fade de escala)
ESCONDER_RESTO = False       # True: objetos SEUS fora de ANUNCIO saem do render (False devolve)
COR_CAIXA = "clara"          # so por compatibilidade: a cor da caixa vem do bake da Meshy
RESOLUCAO = (1080, 1920)     # 9:16 vertical
AMOSTRAS = 64                # amostras do EEVEE no render final
SALVAR_BLEND = True          # grava anuncio_u1.blend ao lado do seu .blend
U1_NOME = ""                 # "" = impressora da Meshy (assets/); ou nome do objeto/colecao de um modelo seu
U1_ROTACAO_Z = 0.0           # graus, para a frente do seu modelo apontar para -Y
U1_TELA = None               # (x, y, z) do centro da tela no seu arquivo, ou None
U1_TOMADA = None             # (x, y, z) da tomada IEC, ou None
U1_BOTAO = None              # (x, y, z) do botao liga/desliga, ou None
U1_TELA_OBJETO = ""          # nome do objeto da tela (para acender), ou ""
U1_BOTAO_OBJETO = ""         # nome do objeto do botao (para afundar), ou ""
U1_LED_OBJETO = ""           # nome de um objeto com LED (Emission), ou ""
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
'''

RODAPE = '''

# ============================================================================
# ASSETS EMBUTIDOS (PNG em base64): logo, tela de boot, interface. Gravados
# na pasta temporaria na hora de rodar e passados como caminho absoluto aos
# modulos.
# ============================================================================
_ASSETS = {
__ASSETS__}

# Os que viajam como arquivo, em assets/ ao lado do .blend. O main() exige
# exatamente esta lista (e empacotar.py zipa exatamente ela).
_NOME_SCRIPT = __NOME_SCRIPT__
_ASSETS_EXTERNOS = (
__ASSETS_EXTERNOS__)


def _gravar_assets():
    pasta = _os.path.join(_tempfile.gettempdir(), "anuncio_u1_assets")
    _os.makedirs(pasta, exist_ok=True)
    for nome, b64 in _ASSETS.items():
        caminho = _os.path.join(pasta, nome)
        with open(caminho, "wb") as f:
            f.write(_base64.b64decode(b64))
    return pasta


def _pasta_assets():
    """A pasta com os assets externos, completa. PASTA_ASSETS preenchida vale
    sozinha (quem apontou, apontou); vazia, procura ao lado do .blend salvo,
    na pasta de trabalho, ao lado do texto anuncio_u1.py (se veio de um
    arquivo) e ao lado do proprio script (blender -P). Uma pasta que existe
    mas esta incompleta nao serve: o erro diz o que falta em cada uma."""
    import bpy
    candidatas = []
    if PASTA_ASSETS:
        candidatas.append(_os.path.abspath(_os.path.expanduser(PASTA_ASSETS)))
    else:
        if bpy.data.filepath:
            candidatas.append(_os.path.join(_os.path.dirname(bpy.data.filepath), "assets"))
        candidatas.append(_os.path.join(_os.getcwd(), "assets"))
        for texto in bpy.data.texts:
            if texto.filepath and (texto.name == _NOME_SCRIPT or _os.path.basename(texto.filepath) == _NOME_SCRIPT):
                candidatas.append(_os.path.join(_os.path.dirname(_os.path.abspath(texto.filepath)), "assets"))
        try:
            candidatas.append(_os.path.join(_os.path.dirname(_os.path.abspath(__file__)), "assets"))
        except NameError:
            pass    # exec do texto sem __file__: os outros candidatos valem
    tentadas = []
    for pasta in candidatas:
        if pasta in [t[0] for t in tentadas]:
            continue
        faltam = [n for n in _ASSETS_EXTERNOS if not _os.path.isfile(_os.path.join(pasta, n))]
        if not faltam:
            print("[anuncio] assets externos em", pasta)
            return pasta
        tentadas.append((pasta, faltam))
    linhas = []
    for pasta, faltam in tentadas:
        if not _os.path.isdir(pasta):
            linhas.append("  %s  (pasta nao existe)" % pasta)
        else:
            linhas.append("  %s  (faltam: %s)" % (pasta, ", ".join(faltam)))
    raise RuntimeError(
        "[anuncio] nao achei a pasta assets/ com os arquivos externos. Copie a pasta assets/ do "
        "pacote para AO LADO do seu .blend (salve o .blend antes) ou preencha PASTA_ASSETS.\\n"
        "Procurei em:\\n%s\\nArquivos esperados: %s" % ("\\n".join(linhas), ", ".join(_ASSETS_EXTERNOS)))


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
    # Antes de tocar na cena: sem os assets nao ha o que montar, e o erro tem
    # de chegar com a cena exatamente como estava.
    pasta_externa = _pasta_assets()
    ext = lambda nome: _os.path.join(pasta_externa, nome)   # noqa: E731
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
        "caixa_some": bool(CAIXA_SOME),
        "espuma_some_nos_closes": bool(ESPUMA_SOME_NOS_CLOSES),
        # Logo e telas: da pasta temporaria (embutidas).
        "pasta_assets": pasta_assets,
        # Impressora e caixa: da pasta externa, por caminho ABSOLUTO (ver o
        # cabecalho de montar.py: nome relativo cairia na pasta errada).
        "u1": {"arquivo_impressora": ext("impressora_limpa.glb")},
        "caixa": {
            "resolucao_texturas": "2k",
            "texturas": {"cor": ext("caixa_cor_2k.png"), "normal": ext("caixa_normal_2k.png"),
                         "rugosidade": ext("caixa_rugosidade_2k.png")},
            "etiqueta": {"malha": ext("caixa_etiqueta_malha.png"), "cor": ext("caixa_etiqueta_cor.png"),
                         "normal": ext("caixa_etiqueta_normal.png"), "rugosidade": ext("caixa_etiqueta_rugosidade.png")},
            "com_logo": False,      # revisao 3: o topo e papelao e fita, nada mais
        },
    }
    objs = mod_coreografia.construir_tudo(params)
    # O que e seu e ficou visivel fora de ANUNCIO renderiza junto: avisa (e,
    # com ESCONDER_RESTO, esconde - marcando, para a proxima rodada devolver).
    mod_coreografia.avisar_objetos_de_fora(objs, esconder=bool(ESCONDER_RESTO))
    mod_coreografia.coreografar(objs)
    mod_coreografia.conferir_colisoes(objs, passo=3)
    largura, altura = RESOLUCAO
    pasta_saida = _os.path.dirname(bpy.data.filepath) if bpy.data.filepath else _os.path.expanduser("~")
    fps = int(mod_coreografia.FPS)
    # Render ANTES do som: configurar_render(video=True) zera o codec de
    # audio, e montar_no_vse o liga (AAC). Na ordem inversa o MP4 sai mudo.
    mod_coreografia.configurar_render(
        objs, largura, altura, AMOSTRAS, video=bool(COM_SOM),
        caminho_saida=_os.path.join(pasta_saida, "anuncio_u1.mp4") if COM_SOM
        else _os.path.join(pasta_saida, "anuncio_u1_quadros", "quadro_"))
    if COM_SOM:
        trilha = TRILHA_EXTERNA or None
        if trilha is None and _os.path.isfile(ext("trilha_externa.wav")):
            trilha = ext("trilha_externa.wav")     # o lugar natural para o WAV licenciado do cliente
        pasta_som = _os.path.join(_tempfile.gettempdir(), "anuncio_u1_som")
        stems = mod_som.gerar_stems(pasta_som, fps=fps, beats=mod_coreografia.BEATS,
                                    fator=mod_coreografia.fator_duracao(DURACAO_S), trilha_externa=trilha)
        # Empacota os WAV no .blend: a pasta temporaria some na limpeza.
        mod_som.montar_no_vse(objs["cena"], stems, mod_coreografia.BEATS, fps=fps)
    print("[anuncio] pronto: %d quadros, camera '%s', %s, saida em %s" % (
        objs["cena"].frame_end, objs["camera"].name, "com som (AAC)" if COM_SOM else "sem som",
        objs["cena"].render.filepath))
    if SALVAR_BLEND:
        caminho = _os.path.join(pasta_saida, "anuncio_u1.blend")
        # Logo e telas vem da pasta temporaria: empacotar, senao o .blend
        # aponta para %TEMP% e as imagens somem na limpeza.
        empacotadas = mod_coreografia.preparar_para_salvar()
        print("[anuncio] imagens empacotadas no .blend:", ", ".join(empacotadas) or "nenhuma (ja estavam)")
        try:
            bpy.ops.wm.save_as_mainfile(filepath=caminho, copy=True)
            print("[anuncio] .blend gravado em", caminho)
        except RuntimeError as e:
            print("[anuncio] nao foi possivel gravar o .blend:", e)
    return objs


if __name__ == "__main__":
    main()
'''


def _indentar(fonte):
    linhas = []
    for linha in fonte.splitlines():
        linhas.append(("    " + linha) if linha.strip() else "")
    return "\n".join(linhas)


def montar():
    partes = [CABECALHO]
    for nome in MODULOS:
        with open(os.path.join(AQUI, nome + ".py"), encoding="utf-8") as f:
            fonte = f.read()
        partes.append("\n\n# %s\n# MODULO %s (scripts/%s.py), inteiro, dentro de uma funcao-namespace\n# %s\n"
                      % ("=" * 76, nome, nome, "=" * 76))
        partes.append("def _modulo_%s():\n%s\n    return locals()\n\n\n%s = _registrar_modulo(%r, _modulo_%s())\n"
                      % (nome[4:], _indentar(fonte), nome, nome, nome[4:]))
    linhas_assets = []
    for nome in ASSETS:
        with open(os.path.join(RAIZ, "assets", nome), "rb") as f:
            b64 = base64.b64encode(f.read()).decode("ascii")
        corpo = "\n".join('        "%s"' % t for t in textwrap.wrap(b64, 96))
        linhas_assets.append('    %r: (\n%s\n    ),\n' % (nome, corpo))
    for nome in ASSETS_EXTERNOS:
        # Conferido aqui para o arquivo unico nunca exigir o que o projeto nao tem.
        if not os.path.isfile(os.path.join(RAIZ, "assets", nome)):
            raise SystemExit("[montar] asset externo ausente em assets/: %s" % nome)
    externos = "".join("    %r,\n" % n for n in ASSETS_EXTERNOS)
    # Marcadores em vez de %-format: o rodape tem prints com %d/%s proprios.
    rodape = (RODAPE.replace("__ASSETS__", "".join(linhas_assets))
              .replace("__ASSETS_EXTERNOS__", externos)
              .replace("__NOME_SCRIPT__", repr(NOME_SCRIPT)))
    partes.append(rodape)
    texto = "".join(partes)
    with open(DESTINO, "w", encoding="utf-8") as f:
        f.write(texto)
    compile(texto, DESTINO, "exec")     # erro de sintaxe aparece aqui, nao no Blender
    print("[montar] %s: %d linhas, %.0f kB" % (DESTINO, texto.count("\n"), len(texto.encode("utf-8")) / 1024.0))
    return DESTINO


if __name__ == "__main__":
    montar()
