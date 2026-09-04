# Previa em VIDEO COM SOM: junta os quadros da previa (PASTA_SEQ, padrao
# saida/previa_seq/quadro_*.png: 1 a cada 2 da coreografia, tocando a 15 fps;
# 25 s = 750 quadros -> 375 PNG) com os stems do mod_som num MP4 H.264 + AAC
# pelo VSE, e PROVA que o arquivo tem video e audio.
#
# Roda no Blender headless (xvfb + llvmpipe, o mesmo caminho da previa):
#   bash scripts/previa.sh scripts/video_com_som.py
#   PROVA_3D=1 bash scripts/previa.sh scripts/video_com_som.py   (+ prova 5)
#   SO_CRUZAR=1 <blender> -b -P scripts/video_com_som.py          (so a prova 4)
# Ambiente: PASTA_SEQ (pasta dos PNG), FATOR (1.0 = 25 s; 0.8 = 20 s; 0.6 =
# 15 s - o mesmo fator_duracao da coreografia com que os quadros foram
# renderizados), NOME_VIDEO (padrao previa_com_som.mp4 na pasta saida/).
#
# SINCRONIA: o quadro k da previa e o 2k-1 da coreografia e aparece em
# (k-1)/15 = (2k-2)/30 s - o mesmo instante em que o 2k-1 apareceria a 30 fps.
# O audio e gerado com fps=30 em tempo real e entra no quadro 1: os segundos
# de som casam com os PNG a 15 fps sem escalar nada. O fps da cena e fixado
# ANTES das faixas de som, porque o comprimento delas em quadros e calculado
# na criacao.
#
# PROVAS (impressas; as imagens ficam em saida/som_quadro_*.png):
#   1. cabecalho do MP4 lido a mao (atomos moov/mvhd/trak/mdia/hdlr/mdhd/
#      stsd): duracao total, uma trilha 'vide' (avc1) e uma 'soun' (mp4a),
#      taxa e duracao de cada uma. Nao ha ffprobe no container.
#   2. o audio DO MP4 decodificado pelo proprio Blender (aud): taxa, canais,
#      duracao; correlacao com som_mix.wav (lag tem de ser 0) e cada clique
#      do obturador (bloco isolado, mesma semente) correlacionado com o
#      audio do MP4 em volta da cue (os tres cortes do beat 5) - prova de
#      sincronia DENTRO do arquivo, nao so de presenca. Correlacao, e nao
#      onset por energia: no audio mixado o pad da trilha tem energia
#      comparavel a do clique em 3 ms, e o detector de energia disparava no
#      comeco da janela (-59 ms medidos, com lag real 0,0000 s).
#   3. tres quadros extraidos do MP4 (inicio, foto A com flash, fim) -
#      QUEM RODA ABRE E OLHA.
#   4. cruzamento com a coreografia real: mod_coreografia.BEATS == mod_som.BEATS
#      e q_em igual em todas as cues (o modulo e de outro agente: se nao
#      importar, avisa e segue).
#   5. PROVA_3D=1: cena 3D (Workbench) + faixas SO de som no VSE -> o render
#      continua sendo a cena 3D, nao o sequencer, e o MP4 sai com audio. E
#      exatamente o caminho do cliente (Render Animation com o som no VSE).

import os
import struct
import sys

import bpy
import numpy as np

AQUI = os.path.dirname(os.path.abspath(__file__))
RAIZ = os.path.dirname(AQUI)
sys.path.insert(0, AQUI)
import mod_som  # noqa: E402

SAIDA = os.path.join(RAIZ, "saida")
ASSETS = os.path.join(RAIZ, "assets")
FPS_PREVIA = 15
FPS_COREO = 30
FATOR = float(os.environ.get("FATOR", "1.0"))
PASTA_SEQ = os.environ.get("PASTA_SEQ") or os.path.join(SAIDA, "previa_seq")
NOME_MP4 = os.environ.get("NOME_VIDEO", "previa_com_som.mp4")
# O que a coreografia produz com esse fator: frame_end, duracao e quantos PNG
# de 2 em 2 (quadros impares 1, 3, ..., ate frame_end).
Q_FIM = mod_som.quadro(mod_som.BEATS[-1]["t_fim"], FPS_COREO, FATOR)
DURACAO = mod_som.duracao_total(mod_som.BEATS, FPS_COREO, FATOR)
N_PNG = (Q_FIM + 1) // 2
falhas = []


def conferir(condicao, texto):
    print(("   ok  " if condicao else "   FALHA ") + texto)
    if not condicao:
        falhas.append(texto)


def faixas_do_vse(cena):
    if cena.sequence_editor is None:
        cena.sequence_editor_create()
    faixas = getattr(cena.sequence_editor, "sequences", None)   # 4.4+: strips
    return faixas if faixas is not None else cena.sequence_editor.strips


def saida_ffmpeg(cena, largura, altura, fps, caminho):
    r = cena.render
    r.fps, r.fps_base = fps, 1.0
    r.resolution_x, r.resolution_y, r.resolution_percentage = largura, altura, 100
    r.image_settings.file_format = "FFMPEG"
    r.ffmpeg.format = "MPEG4"
    r.ffmpeg.codec = "H264"
    r.ffmpeg.constant_rate_factor = "HIGH"
    r.ffmpeg.ffmpeg_preset = "GOOD"
    r.ffmpeg.gopsize = fps
    r.filepath = caminho


# ---------------------------------------------------------------- leitura do MP4 a mao

def _atomos(dados, ini, fim):
    p = ini
    while p + 8 <= fim:
        tam, tipo = struct.unpack(">I4s", dados[p:p + 8])
        cab = 8
        if tam == 1:
            tam = struct.unpack(">Q", dados[p + 8:p + 16])[0]
            cab = 16
        elif tam == 0:
            tam = fim - p
        if tam < cab:
            break
        yield tipo, p + cab, p + tam
        p += tam


def _tempo(dados, a):
    """mvhd/mdhd: (timescale, duration) nas versoes 0 (32 bits) e 1 (64)."""
    if dados[a] == 1:
        return struct.unpack(">IQ", dados[a + 20:a + 32])
    return struct.unpack(">II", dados[a + 12:a + 20])


def ler_mp4(caminho):
    with open(caminho, "rb") as f:
        dados = f.read()
    info = {"bytes": len(dados), "trilhas": []}
    for tipo, a, b in _atomos(dados, 0, len(dados)):
        if tipo != b"moov":
            continue
        for t2, a2, b2 in _atomos(dados, a, b):
            if t2 == b"mvhd":
                ts, dur = _tempo(dados, a2)
                info["duracao_s"] = dur / float(ts)
            elif t2 == b"trak":
                tr = {}
                for t3, a3, b3 in _atomos(dados, a2, b2):
                    if t3 != b"mdia":
                        continue
                    for t4, a4, b4 in _atomos(dados, a3, b3):
                        if t4 == b"hdlr":
                            tr["tipo"] = dados[a4 + 8:a4 + 12].decode("ascii", "replace")
                        elif t4 == b"mdhd":
                            ts, dur = _tempo(dados, a4)
                            tr["taxa"], tr["duracao_s"] = ts, dur / float(ts)
                        elif t4 == b"minf":
                            for t5, a5, b5 in _atomos(dados, a4, b4):
                                if t5 != b"stbl":
                                    continue
                                for t6, a6, b6 in _atomos(dados, a5, b5):
                                    if t6 == b"stsd":
                                        tr["codec"] = dados[a6 + 12:a6 + 16].decode("ascii", "replace")
                info["trilhas"].append(tr)
    return info


def audio_do_mp4(caminho):
    """Decodifica o audio do MP4 com o aud do Blender -> (float (n, c), taxa)."""
    import aud
    som = aud.Sound.file(caminho)
    taxa, canais = som.specs
    dados = np.asarray(som.data(), dtype=float)
    if dados.ndim == 1:
        dados = dados[:, None]
    return dados, int(taxa)


def lag_por_correlacao(x, y, taxa, ini=0, margem_s=0.3):
    """Lag (s) em que y (bloco) casa melhor com x (audio completo), procurado
    em +-margem em volta da amostra 'ini' onde y deveria estar. Positivo = y
    esta atrasado no audio. Robusto ao que toca por cima: e o que o onset
    por energia nao era."""
    m = int(margem_s * taxa)
    a, b = max(ini - m, 0), min(ini + len(y) + m, len(x))
    xx = x[a:b]
    n = 1 << int(np.ceil(np.log2(len(xx) + len(y))))
    corr = np.fft.irfft(np.fft.rfft(xx, n) * np.conj(np.fft.rfft(y, n)), n)
    lags = np.arange(-m, m + 1)
    v = corr[(lags + (ini - a)) % n]
    return lags[int(np.argmax(v))] / float(taxa)


def extrair_quadro(caminho_mp4, quadro, caminho_png, largura, altura, fps):
    """Novo VSE so com o filme, renderiza UM quadro em PNG."""
    cena = bpy.context.scene
    faixas = faixas_do_vse(cena)
    for s in list(faixas):
        faixas.remove(s)
    faixas.new_movie("filme", caminho_mp4, 1, 1)
    r = cena.render
    r.fps, r.fps_base = fps, 1.0
    r.resolution_x, r.resolution_y, r.resolution_percentage = largura, altura, 100
    r.image_settings.file_format = "PNG"
    r.image_settings.color_mode = "RGB"
    r.filepath = caminho_png
    cena.frame_set(quadro)
    bpy.ops.render.render(write_still=True)
    return caminho_png


def media_do_png(caminho):
    img = bpy.data.images.load(caminho)
    px = np.array(img.pixels[:], dtype=float).reshape(-1, img.channels)[:, :3]
    bpy.data.images.remove(img)
    return float(px.mean()), float(px.std())


def cruzar_com_coreografia(cues):
    """Prova 4: a tabela e as fracoes daqui contra a coreografia REAL (modulo
    de outro agente, importa bpy: so roda dentro do Blender)."""
    print("\n== 4. cruzamento com mod_coreografia")
    try:
        import mod_coreografia
        iguais = tuple(dict(b) for b in mod_coreografia.BEATS) == tuple(dict(b) for b in mod_som.BEATS)
        print("   mod_coreografia: DURACAO_REFERENCIA %.1f, fator_duracao(25) = %.2f, frame_end %d" % (
            mod_coreografia.DURACAO_REFERENCIA, mod_coreografia.fator_duracao(25.0),
            mod_coreografia.quadro(mod_coreografia.BEATS[-1]["t_fim"], FATOR)))
        conferir(iguais, "mod_coreografia.BEATS == mod_som.BEATS")
        conferir(abs(mod_coreografia.fator_duracao(mod_som.DURACAO_REFERENCIA) - 1.0) < 1e-9,
                 "DURACAO_REFERENCIA igual (%.1f s)" % mod_som.DURACAO_REFERENCIA)
        dif = []
        for c in cues:
            q_c = mod_coreografia.q_em(c["beat"], c["fracao"], FATOR)
            print("   %-18s beat %d fracao %.3f  som q%3d  coreografia q%3d  %s" % (
                c["efeito"], c["beat"], c["fracao"], c["quadro"], q_c, "ok" if q_c == c["quadro"] else "DIFERENTE"))
            if q_c != c["quadro"]:
                dif.append((c["efeito"], c["quadro"], q_c))
        conferir(not dif, "q_em da coreografia = quadro da cue em todas as %d cues %s" % (len(cues), dif or ""))
        r = mod_coreografia.ROTEIRO
        esperado = {
            ("rasgo_fita", 2): r[2]["tampa"][0], ("pop_espuma", 2): r[2]["espuma"][0],
            ("whoosh_revelacao", 2): r[2]["u1_sobe"][0], ("clique_plugue", 3): r[3]["cabo"][1],
            ("chime_ligar", 3): (r[3]["botao"][0] + r[3]["botao"][1]) / 2.0,
            ("tique_boot", 4): r[4]["boot"], ("ding_ui", 4): r[4]["ui"],
            ("whoosh_descida", 6): r[6]["u1_desce"][0], ("baque_surdo", 6): r[6]["tampa"][1],
            ("impacto", 7): r[7]["mergulho"][1], ("swell", 7): r[7]["mergulho"][1],
        }
        fora = [(e, b, c["fracao"], fr) for (e, b), fr in esperado.items()
                for c in cues if c["efeito"] == e and c["beat"] == b and abs(c["fracao"] - fr) > 1e-6]
        conferir(not fora, "fracoes das cues = fracoes do ROTEIRO da coreografia %s" % (fora or ""))
        fotos = sorted(c["fracao"] for c in cues if c["efeito"] == "obturador")
        conferir(fotos == sorted(r[5]["fotos"]), "um obturador por corte de foto do ROTEIRO[5] (%d)" % len(fotos))
        return True
    except Exception as e:  # noqa: BLE001 - o modulo esta sendo editado em paralelo
        print("   (mod_coreografia nao importou: %s: %s - cruzamento pulado)" % (type(e).__name__, e))
        falhas.append("cruzamento com mod_coreografia nao rodou")
        return False


if os.environ.get("SO_CRUZAR"):
    # So a prova 4, sem quadros nem render: a cue sheet resolvida contra a
    # coreografia real. E o que se roda quando os quadros novos ainda nao existem.
    cues = mod_som.cue_sheet_resolvida(mod_som.BEATS, FPS_COREO, FATOR)
    cruzar_com_coreografia(cues)
    print("\n== resultado:", "TUDO OK" if not falhas else "%d FALHA(S): %s" % (len(falhas), "; ".join(falhas)))
    sys.exit(1 if falhas else 0)


# ================================================================ montagem

bpy.ops.wm.read_factory_settings(use_empty=True)
cena = bpy.context.scene
pasta_seq = PASTA_SEQ
quadros = sorted(f for f in os.listdir(pasta_seq) if f.startswith("quadro_") and f.endswith(".png"))
print("[video] %d quadros em %s (%s .. %s); fator %.2f -> frame_end %d, %.1f s, %d PNG esperados" % (
    len(quadros), pasta_seq, quadros[0], quadros[-1], FATOR, Q_FIM, DURACAO, N_PNG))
conferir(len(quadros) == N_PNG, "%d quadros na sequencia (1 a cada 2 de %d)" % (N_PNG, Q_FIM))
if len(quadros) != N_PNG:
    print("[video] a pasta nao bate com o fator: abortando antes de gerar um MP4 fora de sincronia")
    sys.exit(1)

caminho_mp4 = os.path.join(SAIDA, NOME_MP4)
saida_ffmpeg(cena, 360, 640, FPS_PREVIA, caminho_mp4)   # fps ANTES das faixas de som
cena.frame_start, cena.frame_end = 1, len(quadros)
cena.view_settings.view_transform = "Standard"          # PNGs ja vem com AgX aplicado
faixas = faixas_do_vse(cena)
faixa = faixas.new_image("previa", os.path.join(pasta_seq, quadros[0]), 1, 1)
for f in quadros[1:]:
    faixa.elements.append(f)

print("[video] stems em", ASSETS)
stems = mod_som.gerar_stems(ASSETS, fps=FPS_COREO, beats=mod_som.BEATS, fator=FATOR)
criadas = mod_som.montar_no_vse(cena, stems, mod_som.BEATS, fps=FPS_PREVIA)
conferir(set(criadas) == {"trilha", "efeitos"}, "faixas trilha e efeitos criadas no VSE")
for nome, s in criadas.items():
    conferir(s.frame_final_duration == N_PNG, "faixa %s tem %d quadros a 15 fps (%d)" % (nome, N_PNG, s.frame_final_duration))
cruzar_com_coreografia(stems["cues"])
conferir(cena.render.ffmpeg.audio_codec == "AAC" and cena.render.ffmpeg.audio_bitrate == 192,
         "saida FFMPEG com AAC a %d kbps" % cena.render.ffmpeg.audio_bitrate)

print("\n[video] render: %d quadros a %d fps -> %s" % (len(quadros), FPS_PREVIA, caminho_mp4))
bpy.ops.render.render(animation=True)
conferir(os.path.exists(caminho_mp4), "MP4 gravado")

# ---- 1. cabecalho
print("\n== 1. cabecalho do MP4 (lido a mao)")
info = ler_mp4(caminho_mp4)
print("   %d bytes, duracao %.3f s" % (info["bytes"], info.get("duracao_s", -1)))
for tr in info["trilhas"]:
    print("   trilha %-4s codec %-4s taxa %6d  duracao %.3f s" % (tr.get("tipo"), tr.get("codec"), tr.get("taxa", 0),
                                                                    tr.get("duracao_s", -1)))
tipos = {tr.get("tipo"): tr for tr in info["trilhas"]}
conferir(abs(info.get("duracao_s", 0) - DURACAO) < 0.05, "duracao do MP4 = %.1f s (%.3f)" % (DURACAO, info.get("duracao_s", 0)))
conferir("vide" in tipos and tipos["vide"].get("codec") == "avc1", "trilha de video H.264 (avc1)")
conferir("soun" in tipos and tipos["soun"].get("codec") == "mp4a", "trilha de audio AAC (mp4a)")
conferir("soun" in tipos and tipos["soun"].get("taxa") == 48000, "audio a 48 kHz")
conferir("soun" in tipos and abs(tipos["soun"].get("duracao_s", 0) - DURACAO) < 0.1,
         "audio dura %.1f s (%.3f)" % (DURACAO, tipos.get("soun", {}).get("duracao_s", 0)))

# ---- 2. audio decodificado
print("\n== 2. audio do MP4 decodificado pelo Blender (aud)")
try:
    dados, taxa = audio_do_mp4(caminho_mp4)
    pico = float(np.abs(dados).max())
    rms = float(np.sqrt(np.mean(dados ** 2)))
    print("   %d Hz, %d canais, %.3f s, pico %.2f dBFS, rms %.2f dBFS" % (
        taxa, dados.shape[1], len(dados) / float(taxa), 20 * np.log10(pico), 20 * np.log10(rms)))
    conferir(dados.shape[1] == 2 and abs(len(dados) / float(taxa) - DURACAO) < 0.1, "estereo, %.1f s decodificados" % DURACAO)
    conferir(-30 < 20 * np.log10(rms) < -5, "nivel decodificado plausivel (nao e silencio)")
    mix, _ = mod_som.ler_wav(stems["mix"])
    lag_mix = lag_por_correlacao(dados.mean(axis=1), mix.mean(axis=1), taxa)
    print("   MP4 x som_mix.wav: lag %+.4f s (0 = o audio do MP4 comeca no quadro 1)" % lag_mix)
    conferir(abs(lag_mix) * 1000 <= 1000.0 / FPS_PREVIA / 2.0, "audio do MP4 alinhado com os stems (lag %+.1f ms)" % (lag_mix * 1000))
    pior = 0.0
    for c in [c for c in stems["cues"] if c["efeito"] == "obturador"]:
        bloco, ini = mod_som.sintetizar_cue(dict(c), FATOR)
        lag = lag_por_correlacao(dados.mean(axis=1), bloco.mean(axis=1), taxa, ini)
        pior = max(pior, abs(lag) * 1000)
        print("   obturador: cue %.3f s (q%d da coreografia, q%d da previa)  no MP4 %.3f s  (%+.1f ms)" % (
            c["t"], c["quadro"], (c["quadro"] + 1) // 2, c["t"] + lag, lag * 1000))
    conferir(pior <= 1000.0 / FPS_PREVIA / 2.0, "cliques do obturador no MP4 a menos de meio quadro da previa da cue (pior %.1f ms)" % pior)
except Exception as e:  # noqa: BLE001
    print("   aud nao decodificou: %s: %s" % (type(e).__name__, e))
    falhas.append("audio do MP4 nao decodificado")

# ---- 3. quadros extraidos
print("\n== 3. quadros extraidos do MP4")
extraidos = {}
# 'foto': o primeiro quadro da previa que ja mostra o corte da foto A (com o
# flash): o quadro k da previa e o 2k-1 da coreografia, entao k = (q_a+2)//2.
q_foto_a = min(c["quadro"] for c in stems["cues"] if c["efeito"] == "obturador")
for nome, q in (("ini", 1), ("foto", (q_foto_a + 2) // 2), ("fim", N_PNG)):
    png = extrair_quadro(caminho_mp4, q, os.path.join(SAIDA, "som_quadro_%s.png" % nome), 360, 640, FPS_PREVIA)
    media, desvio = media_do_png(png)
    extraidos[nome] = (png, media, desvio)
    print("   quadro %3d (%s): %s  media %.3f  desvio %.3f" % (q, nome, png, media, desvio))
conferir(all(d > 0.02 for _, _, d in extraidos.values()), "os tres quadros tem imagem (desvio > 0,02, nao sao chapados)")

# ---- 5. faixas so de som nao trocam o render para o sequencer
if os.environ.get("PROVA_3D"):
    print("\n== 5. cena 3D + VSE so com som -> render e a cena 3D, com audio")
    bpy.ops.wm.read_factory_settings(use_empty=True)
    cena = bpy.context.scene
    bpy.ops.mesh.primitive_cube_add(size=1.0, location=(0, 0, 0))
    cubo = bpy.context.active_object
    mat = bpy.data.materials.new("prova")
    mat.diffuse_color = (0.9, 0.3, 0.1, 1.0)
    cubo.data.materials.append(mat)
    bpy.ops.object.camera_add(location=(3, -3, 2), rotation=(1.1, 0, 0.785))
    cena.camera = bpy.context.active_object
    bpy.ops.object.light_add(type="SUN", location=(0, 0, 5))
    cena.render.engine = "BLENDER_WORKBENCH"
    caminho_3d = os.path.join(SAIDA, "som_prova_3d.mp4")
    saida_ffmpeg(cena, 180, 320, FPS_PREVIA, caminho_3d)
    cena.frame_start, cena.frame_end = 1, 30
    mod_som.montar_no_vse(cena, stems, mod_som.BEATS, fps=FPS_PREVIA)
    conferir(all(s.type == "SOUND" for s in faixas_do_vse(cena)), "VSE so tem faixas de som")
    bpy.ops.render.render(animation=True)
    info3 = ler_mp4(caminho_3d)
    tipos3 = {tr.get("tipo"): tr for tr in info3["trilhas"]}
    print("   %s: %.3f s, trilhas %s" % (caminho_3d, info3.get("duracao_s", -1), sorted(tipos3)))
    conferir("vide" in tipos3 and "soun" in tipos3, "MP4 da cena 3D tem video e audio")
    png = extrair_quadro(caminho_3d, 15, os.path.join(SAIDA, "som_prova_3d.png"), 180, 320, FPS_PREVIA)
    media, desvio = media_do_png(png)
    print("   quadro 15 extraido: %s  media %.3f  desvio %.3f" % (png, media, desvio))
    conferir(desvio > 0.05, "o quadro e a cena 3D (cubo laranja), nao preto do sequencer")
    if os.path.exists(caminho_3d):
        os.remove(caminho_3d)

print("\n== resultado:", "TUDO OK" if not falhas else "%d FALHA(S): %s" % (len(falhas), "; ".join(falhas)))
sys.exit(1 if falhas else 0)
