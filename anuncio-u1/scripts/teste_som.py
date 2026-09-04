# Prova do mod_som por MEDIDA, fora do Blender (precisa de numpy: use o python
# do Blender, que o traz):
#
#   /caminho/do/blender/4.2/python/bin/python3.11 scripts/teste_som.py
#
# O que prova:
#   1. gera assets/som_trilha.wav, som_efeitos.wav, som_mix.wav (25 s, 30 fps)
#      e imprime a cue sheet resolvida: segundo, quadro (30 fps), quadro da
#      previa (15 fps), efeito, ganho, pan;
#   2. mede cada stem e a mix LENDO O ARQUIVO gravado: duracao, pico, RMS
#      (trilha -18 dBFS RMS, efeitos -6 dBFS pico, mix < -1 dBFS pico);
#   3. para cada cue, acha o onset no stem de efeitos (energia antes x depois
#      da cue; no swell, o apice) e confere que cai no quadro pedido;
#   4. preset de 15 s (fator 0,6) -> 15,0 s, e trilha externa a 44,1 kHz mono
#      -> reamostrada, 25 s, -18 dBFS;
#   5. desenha saida/som_forma_de_onda.png (3 pistas, linhas de beat, cues
#      numeradas, regua de quadros) e zooms do beat 2 e do plugue/chime.
#      PNG escrito a mao (zlib + struct): nao ha PIL aqui, e o teste nao deve
#      precisar do Blender para rodar. QUEM RODA ABRE OS PNG E OLHA.
#
# O teste falha (SystemExit 1) se qualquer medida sair do alvo.

import math
import os
import struct
import sys
import tempfile
import zlib

import numpy as np

AQUI = os.path.dirname(os.path.abspath(__file__))
RAIZ = os.path.dirname(AQUI)
sys.path.insert(0, AQUI)
import mod_som  # noqa: E402

ASSETS = os.path.join(RAIZ, "assets")
SAIDA = os.path.join(RAIZ, "saida")
# Os stems do preset de 15 s e da trilha externa (23 MB) vao para a pasta
# temporaria, nao para saida/: sao prova, nao entrega.
SCRATCH = os.environ.get("SCRATCH") or os.path.join(tempfile.gettempdir(), "som_teste")
FPS = 30
FPS_PREVIA = 15
falhas = []


def conferir(condicao, texto):
    print(("   ok  " if condicao else "   FALHA ") + texto)
    if not condicao:
        falhas.append(texto)


# ---------------------------------------------------------------- PNG a mao

FONTE = {   # 3x5, linhas de cima para baixo, bit 2 = esquerda
    "0": (7, 5, 5, 5, 7), "1": (2, 6, 2, 2, 7), "2": (7, 1, 7, 4, 7), "3": (7, 1, 7, 1, 7),
    "4": (5, 5, 7, 1, 1), "5": (7, 4, 7, 1, 7), "6": (7, 4, 7, 5, 7), "7": (7, 1, 1, 1, 1),
    "8": (7, 5, 7, 5, 7), "9": (7, 5, 7, 1, 7), ".": (0, 0, 0, 0, 2), "-": (0, 0, 7, 0, 0),
    " ": (0, 0, 0, 0, 0), ":": (0, 2, 0, 2, 0),
    "a": (0, 7, 1, 7, 7), "b": (4, 4, 7, 5, 7), "e": (7, 4, 7, 4, 7), "f": (7, 4, 7, 4, 4),
    "h": (4, 4, 7, 5, 5), "i": (2, 0, 2, 2, 2), "l": (4, 4, 4, 4, 7), "m": (0, 7, 7, 5, 5),
    "o": (0, 7, 5, 5, 7), "q": (7, 5, 7, 1, 1), "r": (0, 7, 4, 4, 4), "s": (7, 4, 7, 1, 7),
    "t": (2, 7, 2, 2, 3), "x": (0, 5, 2, 2, 5), "d": (1, 1, 7, 5, 7), "u": (0, 5, 5, 5, 7),
    "c": (0, 7, 4, 4, 7), "p": (7, 5, 7, 4, 4), "n": (0, 7, 5, 5, 5), "g": (7, 5, 7, 1, 7),
    "v": (0, 5, 5, 5, 2), "w": (0, 5, 5, 7, 7), "z": (7, 1, 2, 4, 7), "k": (4, 5, 6, 5, 5),
}


def texto(img, x, y, s, cor, escala=2):
    for ch in s:
        g = FONTE.get(ch, (7, 7, 7, 7, 7))
        for li, bits in enumerate(g):
            for co in range(3):
                if bits & (4 >> co):
                    x0, y0 = x + co * escala, y + li * escala
                    img[y0:y0 + escala, x0:x0 + escala] = cor
        x += 4 * escala


def gravar_png(caminho, img):
    """RGB uint8 (h, w, 3) -> PNG sem filtro, so zlib + struct."""
    h, w = img.shape[:2]
    linhas = np.concatenate([np.zeros((h, 1), np.uint8), img.reshape(h, w * 3)], axis=1)

    def bloco(tipo, dados):
        return struct.pack(">I", len(dados)) + tipo + dados + struct.pack(">I", zlib.crc32(tipo + dados) & 0xffffffff)

    with open(caminho, "wb") as f:
        f.write(b"\x89PNG\r\n\x1a\n" + bloco(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0))
                + bloco(b"IDAT", zlib.compress(linhas.tobytes(), 6)) + bloco(b"IEND", b""))
    return caminho


def desenhar(caminho, stems, medidas, cues, t_ini, t_fim, largura=1800, titulo=""):
    """Tres pistas (trilha, efeitos, mix) entre t_ini e t_fim, min/max por
    coluna, linhas de beat, marcadores de cue numerados e regua de quadros."""
    fundo, grade, cor_beat, cor_cue, cor_onset, branco = ((8, 10, 24), (30, 34, 60), (255, 120, 40),
                                                          (80, 220, 120), (255, 80, 200), (235, 235, 240))
    cores = {"trilha": (110, 150, 255), "efeitos": (255, 200, 80), "mix": (200, 200, 210)}
    alt_pista, topo, esq, dir_ = 220, 40, 60, 20
    altura = topo + 3 * alt_pista + 60
    img = np.zeros((altura, largura, 3), np.uint8)
    img[:] = fundo
    lw = largura - esq - dir_
    px_por_s = lw / float(t_fim - t_ini)

    def x_de(t):
        return int(esq + (t - t_ini) * px_por_s)

    texto(img, esq, 8, titulo, branco, 3)
    for i, nome in enumerate(("trilha", "efeitos", "mix")):
        y0 = topo + i * alt_pista
        ym = y0 + alt_pista // 2
        img[y0 + 10:y0 + alt_pista - 10, esq:esq + lw] = (14, 16, 34)
        img[ym, esq:esq + lw] = grade
        dados, taxa = mod_som.ler_wav(stems[nome])
        a, b = int(t_ini * taxa), min(int(t_fim * taxa), len(dados))
        mono = dados[a:b].mean(axis=1)
        n_col = lw
        idx = np.linspace(0, len(mono), n_col + 1).astype(int)
        meia = (alt_pista - 24) / 2.0
        for c in range(n_col):
            seg = mono[idx[c]:max(idx[c + 1], idx[c] + 1)]
            if not len(seg):
                continue
            lo, hi = float(seg.min()), float(seg.max())
            img[int(ym - hi * meia):int(ym - lo * meia) + 1, esq + c] = cores[nome]
        m = medidas[nome]
        texto(img, esq + 6, y0 + 12, "%s  pico %.1f dbfs  rms %.1f dbfs  %.2f s" % (
            nome, m["pico_dbfs"], m["rms_dbfs"], m["duracao_s"]), cores[nome], 2)
    # beats, na MESMA convencao das cues (instante em que o quadro aparece,
    # (quadro-1)/fps): em segundos da especificacao a linha do b2 ficava em
    # 2,500 s e a cue em 2,467 s, e parecia cue adiantada.
    for bt in mod_som.BEATS:
        t = mod_som.instante(mod_som.BEATS, bt["n"], 0.0, FPS)
        if t_ini <= t <= t_fim:
            x = x_de(t)
            img[topo:topo + 3 * alt_pista, x] = cor_beat
            texto(img, x + 3, topo - 14, "b%d" % bt["n"], cor_beat, 2)
    x = x_de(mod_som.instante(mod_som.BEATS, 7, 1.0, FPS))
    if esq <= x < largura:
        img[topo:topo + 3 * alt_pista, x] = cor_beat
    # cues (numeradas na ordem do tempo) na pista de efeitos, com o onset medido
    y_ef = topo + alt_pista
    for i, cue in enumerate(cues):
        if not (t_ini <= cue["t"] <= t_fim):
            continue
        x = x_de(cue["t"])
        img[y_ef + 10:y_ef + alt_pista - 10, x] = cor_cue
        img[y_ef + 10:y_ef + 16, max(x - 3, 0):x + 4] = cor_cue
        texto(img, x + 3, y_ef + alt_pista - 34 - (i % 3) * 12, "%d" % (i + 1), cor_cue, 2)
        if cue.get("t_medido") is not None:
            xm = x_de(cue["t_medido"])
            img[y_ef + alt_pista - 22:y_ef + alt_pista - 10, xm] = cor_onset
    # regua de quadros (30 fps): tick a cada 15 quadros, numero a cada 30
    y_r = topo + 3 * alt_pista + 6
    q_a, q_b = int(math.floor(t_ini * FPS)) + 1, int(math.ceil(t_fim * FPS)) + 1
    for q in range(q_a, q_b + 1):
        t = (q - 1) / float(FPS)
        if not (t_ini <= t <= t_fim):
            continue
        x = x_de(t)
        if (q - 1) % 30 == 0:
            img[y_r:y_r + 10, x] = branco
            texto(img, x + 2, y_r + 12, "%d" % q, branco, 2)   # so o numero: 'q' em 3x5 le-se como '9'
        elif (q - 1) % 15 == 0 or px_por_s > 200:
            img[y_r:y_r + 5, x] = grade
    texto(img, esq, altura - 14, "verde: cue  rosa: onset medido  laranja: beat  regua: numero do quadro a 30 fps", branco, 2)
    return gravar_png(caminho, img)


# ---------------------------------------------------------------- onset

TRANSIENTES = ("impacto", "rasgo_fita", "pop_espuma", "clique_plugue", "chime_ligar", "tique_boot",
               "ding_ui", "obturador", "baque_surdo")


def onset_e_pico(bloco, taxa):
    """No bloco ISOLADO: primeira amostra a -40 dB do proprio pico, e o
    apice do envelope RMS de 20 ms. RMS suavizado, nao a amostra maxima:
    num bloco de ruido a amostra maxima cai ao acaso dentro do trecho em
    que o envelope esta perto do topo (medido: -45 ms no swell)."""
    env = np.abs(bloco).max(axis=1)
    pico = float(env.max())
    i_on = int(np.argmax(env >= pico * 10 ** (-40 / 20.0)))
    k = int(0.02 * taxa)
    # potencia media dos DOIS canais: no swell sao ruidos independentes, e a
    # media divide a variancia da estimativa por dois
    rms = np.sqrt(np.convolve((bloco ** 2).mean(axis=1), np.ones(k) / k, "same"))
    return i_on / float(taxa), int(np.argmax(rms)) / float(taxa)


def atraso_por_correlacao(stem, bloco, ini, taxa, margem_s=2.0 / FPS):
    """Lag (amostras) em que o bloco isolado casa melhor com o stem completo,
    procurado em +-margem em volta de onde ele foi colocado. 0 = esta la.
    Robusto a outros efeitos por cima: e o que o onset por energia nao era."""
    m = int(margem_s * taxa)
    a, b = max(ini - m, 0), min(ini + len(bloco) + m, len(stem))
    x = stem[a:b].mean(axis=1)
    y = bloco.mean(axis=1)
    n = 1 << int(np.ceil(np.log2(len(x) + len(y))))
    corr = np.fft.irfft(np.fft.rfft(x, n) * np.conj(np.fft.rfft(y, n)), n)
    lags = np.arange(-m - 5, m + 6)
    valores = corr[(lags + (ini - a)) % n]
    return int(lags[int(np.argmax(valores))])


# ---------------------------------------------------------------- 1. stems de referencia

DURACAO = mod_som.duracao_total(mod_som.BEATS, FPS)
Q_FIM = mod_som.quadro(mod_som.BEATS[-1]["t_fim"], FPS)
print("== 1. stems (%.0f s = %d quadros, 30 fps) em %s" % (DURACAO, Q_FIM, ASSETS))
stems = mod_som.gerar_stems(ASSETS, fps=FPS)
cues = stems["cues"]
print("\n   cue sheet resolvida (%d cues):" % len(cues))
print("   %-3s %-8s %-6s %-9s %-18s %-7s %s" % ("#", "segundo", "q30", "q previa", "efeito", "ganho", "pan"))
for i, c in enumerate(cues):
    q_previa = (c["quadro"] + 1) // 2
    print("   %-3d %-8.3f %-6d %-9d %-18s %-+7.1f %s" % (i + 1, c["t"], c["quadro"], q_previa, c["efeito"],
                                                        c["ganho_db"], c["pan"]))

# ---------------------------------------------------------------- 2. medidas

print("\n== 2. medidas lidas dos WAV")
medidas = {}
for nome in ("trilha", "efeitos", "mix"):
    m = mod_som.medir(stems[nome])
    medidas[nome] = m
    print("   %-8s %7.3f s  %d Hz  pico %7.2f dBFS  rms %7.2f dBFS  (%s)" % (
        nome, m["duracao_s"], m["taxa"], m["pico_dbfs"], m["rms_dbfs"], stems[nome]))
conferir(Q_FIM == 750 and abs(DURACAO - 25.0) < 1e-9, "referencia: 750 quadros = 25,000 s")
for nome in ("trilha", "efeitos", "mix"):
    conferir(abs(medidas[nome]["duracao_s"] - DURACAO) < 1e-3, "%s dura %.3f s" % (nome, DURACAO))
conferir(abs(medidas["trilha"]["rms_dbfs"] + 18.0) < 0.1, "trilha RMS = -18 dBFS (+-0,1)")
conferir(abs(medidas["efeitos"]["pico_dbfs"] + 6.0) < 0.1, "efeitos pico = -6 dBFS (+-0,1)")
conferir(medidas["mix"]["pico_dbfs"] < -1.0, "mix pico < -1 dBFS (nao clipa)")
conferir(medidas["trilha"]["pico_dbfs"] < 0.0 and medidas["efeitos"]["pico_dbfs"] < 0.0, "stems nao clipam")
soma_pico = 20 * math.log10(10 ** (medidas["trilha"]["pico_dbfs"] / 20) + 10 ** (medidas["efeitos"]["pico_dbfs"] / 20))
print("   soma dos picos (pior caso trilha+efeitos): %.2f dBFS; ganho no VSE %.3f (%.2f dB)" % (
    soma_pico, stems["ganho_vse"], 20 * math.log10(stems["ganho_vse"])))
conferir(stems["ganho_vse"] <= 1.0, "ganho no VSE <= 1")

# ---------------------------------------------------------------- 2b. escuta por numero

print("\n== 2b. por beat: RMS da trilha e dos efeitos, centroide espectral da trilha (Hz)")
print("   (a DINAMICA manda a trilha subir na revelacao - beat 2 - e abrir na cartela - beat 7)")
d_tr, _ = mod_som.ler_wav(stems["trilha"])
d_ef, _ = mod_som.ler_wav(stems["efeitos"])
niveis = {}
for bt in mod_som.BEATS:
    a = int(mod_som.instante(mod_som.BEATS, bt["n"], 0.0, FPS) * mod_som.TAXA)
    b = int(mod_som.instante(mod_som.BEATS, bt["n"], 1.0, FPS) * mod_som.TAXA)
    tr, ef = d_tr[a:b].mean(axis=1), d_ef[a:b].mean(axis=1)
    esp = np.abs(np.fft.rfft(tr * np.hanning(len(tr))))
    f = np.fft.rfftfreq(len(tr), 1.0 / mod_som.TAXA)
    centroide = float((esp * f).sum() / max(esp.sum(), 1e-12))
    rms_tr = 20 * np.log10(max(np.sqrt(np.mean(tr ** 2)), 1e-9))
    rms_ef = 20 * np.log10(max(np.sqrt(np.mean(ef ** 2)), 1e-9))
    niveis[bt["n"]] = rms_tr
    print("   b%d %-11s %5.1f-%5.1f s  trilha %6.1f dBFS  efeitos %6.1f dBFS  centroide %6.0f Hz" % (
        bt["n"], bt["nome"], bt["t_ini"], bt["t_fim"], rms_tr, rms_ef, centroide))
conferir(niveis[3] > niveis[1] + 3.0, "trilha sobe na revelacao: beat 3 %.1f dB acima do beat 1" % (niveis[3] - niveis[1]))
conferir(niveis[7] > niveis[6] + 1.0, "trilha abre na cartela: beat 7 %.1f dB acima do beat 6" % (niveis[7] - niveis[6]))

# ---------------------------------------------------------------- 3. cada cue no stem

print("\n== 3. cada cue: bloco isolado (mesma semente) correlacionado com o stem de efeitos")
print("   lag = amostras de diferenca entre onde o bloco esta no stem e onde a cue manda (0 = exato);")
print("   onset = -40 dB do pico do bloco isolado; nos transientes tem de cair a menos de meio quadro da cue")
dados_ef, taxa = mod_som.ler_wav(stems["efeitos"])
pior_lag, pior_trans, pior_swell = 0, 0.0, 0.0
for i, c in enumerate(cues):
    bloco, ini = mod_som.sintetizar_cue(dict(c), 1.0)
    t_on, t_pk = onset_e_pico(bloco, mod_som.TAXA)
    lag = atraso_por_correlacao(dados_ef, bloco, ini, taxa)
    t_onset, t_pico = c["t_colocado"] + t_on, c["t_colocado"] + t_pk
    c["t_medido"] = t_onset if c["efeito"] != "swell" else t_pico
    d_on = (t_onset - c["t"]) * 1000.0
    pior_lag = max(pior_lag, abs(lag))
    if c["efeito"] in TRANSIENTES:
        pior_trans = max(pior_trans, abs(d_on))
    if c["efeito"] == "swell":
        pior_swell = max(pior_swell, abs(t_pico - c["t"]) * 1000.0)
    print("   %2d %-18s cue %7.3f s (q%3d)  colocado %7.3f  onset %+7.1f ms (q%3d)  pico %+8.1f ms  lag %+d" % (
        i + 1, c["efeito"], c["t"], c["quadro"], c["t_colocado"], d_on, int(round(t_onset * FPS)) + 1,
        (t_pico - c["t"]) * 1000.0, lag))
# +-1 amostra (0,02 ms): a correlacao de um sub de 40-60 Hz e plana nessa
# escala e o ruido do swell por cima decide o argmax (medido: -1 no impacto
# da travessia, com t_colocado exato). Um quadro tem 1.600 amostras.
conferir(pior_lag <= 1, "todo bloco esta no stem onde a cue manda, a +-1 amostra (pior lag %d)" % pior_lag)
conferir(pior_trans <= 1000.0 / FPS / 2.0, "transientes soam a menos de meio quadro da cue (pior %.1f ms)" % pior_trans)
# Swell: a prova de sincronia e a colocacao (lag 0 e colocado = cue - dur,
# acima); o argmax do RMS de um RUIDO perto de um apice quase plano e limitado
# pelo proprio ruido - banda de ~3,5 kHz no apice x janela de 20 ms = ~140
# amostras efetivas, ~8% de flutuacao do RMS, e o envelope x^2,2 so muda 5%
# em 40 ms (medido: -11 ms com 1,5 s, -39 ms com 1,75 s). Tolerancia de 1,5
# quadro: confere que o apice esta na travessia e nao no comeco do bloco.
conferir(pior_swell <= 1.5 * 1000.0 / FPS, "apice medido do swell a menos de 1,5 quadro da travessia (%.1f ms)" % pior_swell)
conferir(all(c["t_colocado"] + c["antecipacao"] - c["t"] < 1.0 / mod_som.TAXA + 1e-9 for c in cues),
         "colocacao = cue - antecipacao, a menos de uma amostra")

# ---------------------------------------------------------------- 4. preset 15 s e trilha externa

FATOR_15 = mod_som.fator_duracao(15.0)     # PRESETS da coreografia: 15 s = 0,6
print("\n== 4. preset de 15 s (fator %.2f) e trilha externa, em %s" % (FATOR_15, SCRATCH))
os.makedirs(SCRATCH, exist_ok=True)
p15 = os.path.join(SCRATCH, "preset15")
s15 = mod_som.gerar_stems(p15, fps=FPS, fator=FATOR_15)
m15 = mod_som.medir(s15["mix"])
q_trav_15 = mod_som.q_em(mod_som.BEATS, 7, 0.42, FPS, FATOR_15)
print("   15 s: mix %.3f s, pico %.2f dBFS, %d cues, travessia em %.3f s (q%d; q_em = %d)" % (
    m15["duracao_s"], m15["pico_dbfs"], len(s15["cues"]), s15["cues"][-1]["t"], s15["cues"][-1]["quadro"], q_trav_15))
conferir(abs(m15["duracao_s"] - 15.0) < 1e-3, "preset 15 s dura 15,000 s")
conferir(m15["pico_dbfs"] < -1.0, "preset 15 s nao clipa")
conferir(s15["cues"][-1]["quadro"] == q_trav_15, "travessia no preset 15 s cai no q_em(7, 0,42) do fator")

# trilha externa: 44,1 kHz, mono, 16-bit, 12 s (mais curta que o video, de proposito)
ext = os.path.join(SCRATCH, "trilha_externa_44k.wav")
import wave  # noqa: E402
tt = np.arange(int(12.0 * 44100)) / 44100.0
sinal = 0.5 * np.sin(2 * np.pi * 220.0 * tt) * (1 - np.exp(-tt / 0.5))
with wave.open(ext, "wb") as w:
    w.setnchannels(1)
    w.setsampwidth(2)
    w.setframerate(44100)
    w.writeframes(np.round(sinal * 32767).astype("<i2").tobytes())
pext = os.path.join(SCRATCH, "externa")
sext = mod_som.gerar_stems(pext, fps=FPS, trilha_externa=ext)
mext = mod_som.medir(sext["trilha"])
print("   externa: trilha %.3f s a %d Hz, pico %.2f dBFS, rms %.2f dBFS, origem %s" % (
    mext["duracao_s"], mext["taxa"], mext["pico_dbfs"], mext["rms_dbfs"], sext["origem_trilha"]))
conferir(mext["taxa"] == 48000 and abs(mext["duracao_s"] - DURACAO) < 1e-3, "trilha externa reamostrada a 48 kHz e com %.0f s" % DURACAO)
conferir(abs(mext["rms_dbfs"] + 18.0) < 0.1, "trilha externa normalizada a -18 dBFS RMS")
conferir(sext["origem_trilha"] == ext, "gerar_stems registrou a origem externa")
# a frequencia sobreviveu a reamostragem? conta cruzamentos por zero no 2o segundo
d_ext, _ = mod_som.ler_wav(sext["trilha"])
seg = d_ext[48000:96000, 0]
cruz = int(np.sum((seg[:-1] < 0) & (seg[1:] >= 0)))
print("   externa: %d ciclos/s medidos no 2o segundo (esperado 220)" % cruz)
conferir(abs(cruz - 220) <= 1, "reamostragem preservou a frequencia (220 Hz)")

# ---------------------------------------------------------------- 5. graficos

print("\n== 5. graficos")
# Janelas dos zooms saem dos beats, nao de numero solto: o beat 2 inteiro
# (com 0,1 s de folga) e do plugue (3/0,71) ate a orbita do beat 4 (4/0,00).
t2a, t2b = mod_som.instante(mod_som.BEATS, 2, 0.0, FPS) - 0.1, mod_som.instante(mod_som.BEATS, 2, 1.0, FPS) + 0.1
t3a, t3b = mod_som.instante(mod_som.BEATS, 3, 0.71, FPS) - 0.15, mod_som.instante(mod_som.BEATS, 4, 0.0, FPS) + 0.5
png1 = desenhar(os.path.join(SAIDA, "som_forma_de_onda.png"), stems, medidas, cues, 0.0, DURACAO,
                titulo="som do anuncio u1 - %.0f s a 30 fps - trilha / efeitos / mix" % DURACAO)
png2 = desenhar(os.path.join(SAIDA, "som_zoom_beat2.png"), stems, medidas, cues, t2a, t2b,
                titulo="zoom beat 2 - rasgo, espuma, whoosh de revelacao, u1 assenta")
png3 = desenhar(os.path.join(SAIDA, "som_zoom_beat3.png"), stems, medidas, cues, t3a, t3b,
                titulo="zoom beat 3 - plugue, chime de ligar, orbita do beat 4")
print("   ", png1)
print("   ", png2)
print("   ", png3)

print("\n== resultado:", "TUDO OK" if not falhas else "%d FALHA(S): %s" % (len(falhas), "; ".join(falhas)))
sys.exit(1 if falhas else 0)
