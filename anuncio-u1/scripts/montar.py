# Monta o ARQUIVO UNICO do anuncio: scripts/anuncio_u1.py.
#
# Concatena, nesta ordem, mod_ambiente, mod_caixa, mod_u1, mod_cabo,
# mod_cartela e mod_coreografia, mais os PNGs de assets/ em base64 e um
# main(). O cliente cola o resultado na aba Scripting do Blender e roda.
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
# POR QUE os PNGs vao em base64: na aba Scripting nao existe __file__ nem
# pasta assets/. O main grava os tres arquivos na pasta temporaria do sistema
# e passa os caminhos absolutos aos modulos (todos aceitam caminho absoluto).
#
# Uso: python3 scripts/montar.py   (Python comum; nao precisa do Blender)

import base64
import os
import textwrap

AQUI = os.path.dirname(os.path.abspath(__file__))
RAIZ = os.path.dirname(AQUI)
MODULOS = ("mod_ambiente", "mod_caixa", "mod_u1", "mod_cabo", "mod_cartela", "mod_coreografia")
ASSETS = ("logo_engineprint.png", "tela_boot.png", "tela_ui.png")
DESTINO = os.path.join(AQUI, "anuncio_u1.py")

CABECALHO = '''# ============================================================================
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
'''

RODAPE = '''

# ============================================================================
# ASSETS (PNG em base64): logo, tela de boot, interface. Gravados na pasta
# temporaria na hora de rodar e passados como caminho absoluto aos modulos.
# ============================================================================
_ASSETS = {
__ASSETS__}


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
    # Marcador em vez de %-format: o rodape tem prints com %d/%s proprios.
    partes.append(RODAPE.replace("__ASSETS__", "".join(linhas_assets)))
    texto = "".join(partes)
    with open(DESTINO, "w", encoding="utf-8") as f:
        f.write(texto)
    compile(texto, DESTINO, "exec")     # erro de sintaxe aparece aqui, nao no Blender
    print("[montar] %s: %d linhas, %.0f kB" % (DESTINO, texto.count("\n"), len(texto.encode("utf-8")) / 1024.0))


if __name__ == "__main__":
    montar()
