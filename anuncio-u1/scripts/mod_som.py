# Som do anuncio Snapmaker U1: trilha sintetizada + efeitos sincronizados por
# beat, mixados no MP4 pelo VSE do Blender (Revisao 2, item 4 da ESPECIFICACAO).
#
# TRILHA PROVISORIA SINTETIZADA. Um anuncio de verdade usa musica licenciada:
# ponha o arquivo em assets/trilha_externa.wav (ou passe trilha_externa=...) e
# ela substitui o pad daqui, no mesmo nivel (-18 dBFS RMS). O WAV pode ser mono
# ou estereo, 8/16/24/32 bits; se nao for 48 kHz e reamostrado por
# interpolacao linear (numpy, sem scipy) - serve para previa; para o final,
# entregue em 48 kHz que o arquivo entra sem tocar.
#
# Tudo e sintese com numpy a 48 kHz estereo, gravada em WAV 16-bit pelo modulo
# 'wave'. Sem asset de audio, sem pip: numpy vem com o Blender.
#
# BLOCOS DE SINTESE (cada um devolve (sinal, antecipacao_s); sinal mono (n,) ou
# estereo (n, 2), pico 1,0; antecipacao = quantos segundos ANTES da cue o sinal
# comeca - so o swell usa, porque nele a cue marca o apice, nao o inicio):
#   whoosh          ruido branco por passa-banda com varredura de frequencia
#                   (STFT com mascara gaussiana em log-frequencia) e envelope
#                   assimetrico; pan animado vem da cue
#   impacto_sub     seno 62 -> 40 Hz com decaimento + transiente
#   pop_espuma      rajadas curtas de ruido em cluster aleatorio (semente fixa)
#   rasgo_fita      graos de ruido com taxa e nivel crescentes, 0,3 s
#   clique_plugue   transiente + corpo em 1 kHz + "tock" em 320 Hz
#   chime_ligar     dois senos em quinta (C5+G5), 0,6 s, com sub
#   tique_boot      3 ticks curtos
#   ding_ui         seno com harmonicos (um inarmonico), 0,4 s
#   obturador       clique duplo + ruido curto de cortina
#   swell           ruido filtrado crescendo 1,5 s, estereo descorrelacionado
#   baque_surdo     sub + ruido curto passa-baixa
#
# CUE SHEET e DADO (CUE_SHEET, abaixo): (beat, fracao_do_beat, efeito,
# ganho_db, pan). Vira segundos pela tabela de beats da especificacao com a
# MESMA conversao do mod_coreografia (quadro = round(t*fps*fator); fracao
# interpola em quadros), e o som cai em (quadro - 1)/fps - o instante em que
# aquele quadro aparece. As fracoes copiam o ROTEIRO da coreografia (revisao
# 3, 25 s): o plugue encaixa em 0,71 do beat 3, o LED acende no MEIO do curso
# do botao (0,77..0,98 -> 0,875), o boot comeca em 0,52 do beat 4 e a UI corta
# em 0,74, as fotos cortam em 0, 1/3 e 2/3 do beat 5 (as tres com flash), a
# foto C NAO corta para o beat 6 - abre num pull-back ate 'u1_desce' (0,48),
# que aqui e um whoosh leve e nao um obturador -, a tampa termina de fechar
# em 1,0 do beat 6 e a travessia (centro do topo da caixa; a logo nasce na
# cartela) e o fim do mergulho, 0,42 do beat 7. Os whooshes seguem os trechos
# do obturador visual (_obturador da coreografia): b1 0-0,80, b2 espuma ->
# u1_sobe, as duas orbitas ate 0,48, b6 ate 0,70 e b7 ate a travessia.
#
# NIVEIS: trilha -18 dBFS RMS; efeitos normalizados a -6 dBFS de pico; mix
# (trilha + efeitos) por limitador soft-clip e normalizada a -1 dBFS de pico.
# No VSE as duas faixas entram separadas e o Blender soma sem limitador: por
# isso gerar_stems devolve 'ganho_vse' (<= 1), o volume que montar_no_vse poe
# nas duas faixas para a soma ficar abaixo de -1 dBFS sem clipar.
#
# ---------------------------------------------------------------------------
# INTEGRACAO NO ARQUIVO UNICO (o que montar.py precisa fazer) - quem coordena:
#
# 1. MODULOS de montar.py: acrescentar "mod_som" (qualquer posicao: nao importa
#    outro modulo; so numpy, math, wave, os, e bpy dentro de montar_no_vse). O montar.py indenta o modulo numa funcao-namespace: os
#    'import' de topo daqui funcionam la dentro, e 'import numpy as np' passa
#    a rodar na hora do _registrar_modulo - o Blender traz numpy, ok.
#
# 2. Bloco PARAMETROS: COM_SOM = True e TRILHA_EXTERNA = "" (caminho absoluto
#    de um WAV licenciado, ou "" para o pad sintetizado).
#
# 3. No main() do RODAPE, a chamada de configurar_render passa a pedir MP4
#    (o audio so existe no container FFMPEG; em PNG por quadro nao ha onde por
#    som), e o som entra DEPOIS dela, nesta ordem:
#
#        mod_coreografia.configurar_render(
#            objs, largura, altura, AMOSTRAS, video=COM_SOM,
#            caminho_saida=_os.path.join(pasta_saida, "anuncio_u1.mp4") if COM_SOM
#            else _os.path.join(pasta_saida, "anuncio_u1_quadros", "quadro_"))
#        if COM_SOM:
#            pasta_som = _os.path.join(_tempfile.gettempdir(), "anuncio_u1_som")
#            stems = mod_som.gerar_stems(
#                pasta_som, fps=30, beats=mod_coreografia.BEATS,
#                fator=mod_coreografia.fator_duracao(DURACAO_S),
#                trilha_externa=TRILHA_EXTERNA or None)
#            mod_som.montar_no_vse(objs["cena"], stems, mod_coreografia.BEATS, fps=30)
#
#    A ORDEM IMPORTA: mod_ambiente.configurar_render(video=True) escreve
#    ffmpeg.audio_codec = "NONE"; montar_no_vse escreve "AAC" a 192 kbps e
#    48 kHz. Chamado antes, o render sai mudo. Os WAV vao para a pasta
#    temporaria do sistema (como os PNG dos assets); montar_no_vse empacota
#    os sons no .blend (Sound.pack), entao o anuncio_u1.blend gravado no fim
#    continua tocando depois da limpeza do %TEMP%.
#
# 4. Feito isso, Render > Render Animation do cliente ja sai
#    anuncio_u1.mp4 com H.264 + AAC: nao ha passo separado de mixagem. Faixas
#    so de som no VSE NAO trocam o render para o sequencer (o Blender so usa o
#    sequencer quando ha faixa de imagem/cena) - a cena 3D renderiza e o audio
#    e somado na saida; video_com_som.py prova isso com PROVA_3D=1.
#
# 5. Prova por medida (fora do Blender, no python que traz numpy):
#        python3 scripts/teste_som.py   -> assets/som_*.wav + saida/som_*.png
#    Os assets/som_*.wav sao SAIDA do teste (11,5 MB): NAO entram na tupla
#    ASSETS de montar.py - gerar_stems os refaz em ~2 s no Blender do cliente.
#    Previa em video com som: bash scripts/previa.sh scripts/video_com_som.py
#    (PASTA_SEQ= pasta dos quadros de 2 em 2; FATOR= 1.0 para 25 s);
#    SO_CRUZAR=1 confere q_em da coreografia = quadro de cada cue, sem video.
# ---------------------------------------------------------------------------

import math
import os
import wave

import numpy as np

NOME = "som"
TAXA = 48000                 # Hz: o AAC do Blender mixa a 48 kHz; gerar nela evita reamostrar
FPS_REFERENCIA = 30.0
DURACAO_REFERENCIA = 25.0    # revisao 3: 750 quadros; presets 20 s = 0,8 e 15 s = 0,6
SEMENTE = 20260904           # espuma e rasgo sao aleatorios, mas iguais a cada render

# Espelho da tabela da coreografia (mod_coreografia.BEATS, revisao 3, 25 s).
# Fica aqui porque este modulo nao pode importar a coreografia (ela importa
# bpy no topo e o teste roda fora do Blender); gerar_stems aceita beats= com
# a tabela real e video_com_som.py confere que as duas sao iguais.
BEATS = (
    {"n": 1, "nome": "caixa_sobe", "t_ini": 0.0, "t_fim": 2.8},
    {"n": 2, "nome": "abre", "t_ini": 2.8, "t_fim": 7.2},
    {"n": 3, "nome": "traseira", "t_ini": 7.2, "t_fim": 11.2},
    {"n": 4, "nome": "tela", "t_ini": 11.2, "t_fim": 15.2},
    {"n": 5, "nome": "fotos", "t_ini": 15.2, "t_fim": 18.2},
    {"n": 6, "nome": "volta", "t_ini": 18.2, "t_fim": 20.8},
    {"n": 7, "nome": "cartela", "t_ini": 20.8, "t_fim": 25.0},
)

# (beat, fracao_do_beat, efeito, ganho_db, pan). pan: -1 esquerda .. +1 direita;
# uma dupla (ini, fim) anima o pan ao longo do efeito (whoosh acompanha a
# camera: no beat 3 a orbita passa pelo lado +X, o mundo desliza para a
# esquerda, o som vai direita -> esquerda; no beat 4 volta pelo lado -X).
CUE_SHEET = (
    (1, 0.000, "whoosh_grave", -3.0, (-0.5, 0.5)),      # caixa sobe girando (obturador visual 0-0,80)
    (1, 1.000, "impacto", -14.0, 0.0),                  # ...e assenta no ar
    (2, 0.000, "rasgo_fita", -4.0, 0.15),               # abas abrem (0,00-0,27): a fita rasga na emenda
    (2, 0.200, "pop_espuma", -6.0, 0.0),                # espuma explode (0,20-0,72)
    (2, 0.420, "whoosh_revelacao", -2.0, 0.0),          # U1 sobe da caixa (0,42-0,62)
    (2, 1.000, "impacto", -16.0, 0.0),                  # U1 desce e assenta no ar (0,84-1,00)
    (3, 0.000, "whoosh_orbita", -6.0, (0.8, -0.8)),     # orbita frente -> traseira (0,00-0,48)
    (3, 0.710, "clique_plugue", -4.0, 0.35),            # plugue encaixa (fim do arco do cabo)
    (3, 0.875, "chime_ligar", 0.0, 0.0),                # LED acende no meio do curso do botao (0,77-0,98)
    (4, 0.000, "whoosh_orbita", -6.0, (-0.8, 0.8)),     # orbita traseira -> frente (0,00-0,48)
    (4, 0.520, "tique_boot", -8.0, 0.2),                # tela de boot acende
    (4, 0.740, "ding_ui", -4.0, 0.2),                   # corte seco para a UI
    (5, 0.000, "obturador", -2.0, -0.3),                # foto A (corte + flash)
    (5, 1.0 / 3.0, "obturador", -2.0, 0.3),             # foto B
    (5, 2.0 / 3.0, "obturador", -2.0, 0.0),             # foto C
    (6, 0.000, "whoosh_pullback", -10.0, 0.0),          # foto C abre em pull-back ate u1_desce (0,48), sem corte
    (6, 0.480, "whoosh_descida", -4.0, 0.0),            # U1 desce na caixa (0,48-0,72)
    (6, 1.000, "baque_surdo", -1.0, 0.0),               # abas fecham (0,82-1,00)
    (7, 0.420, "impacto", 0.0, 0.0),                    # sub na travessia do topo da caixa (fim do mergulho)
    (7, 0.420, "swell", -3.0, 0.0),                     # apice na travessia; comeca no inicio do beat (1,75 s)
)

# Variantes dos blocos: nome da cue -> (gerador, parametros). Duracoes dos
# whooshes = duracao do movimento na referencia de 25 s (escalam com o fator).
EFEITOS = {
    "whoosh_grave": ("whoosh", dict(dur=2.2, f_ini=90.0, f_fim=420.0, largura=1.4, apice=0.55)),
    "whoosh_revelacao": ("whoosh", dict(dur=1.1, f_ini=300.0, f_fim=3200.0, largura=1.1, apice=0.60)),
    "whoosh_orbita": ("whoosh", dict(dur=1.9, f_ini=500.0, f_fim=1400.0, largura=1.0, apice=0.45)),
    "whoosh_pullback": ("whoosh", dict(dur=1.25, f_ini=1400.0, f_fim=350.0, largura=1.1, apice=0.40)),
    "whoosh_descida": ("whoosh", dict(dur=0.7, f_ini=1800.0, f_fim=220.0, largura=1.0, apice=0.35)),
    "impacto": ("impacto_sub", {}),
    "pop_espuma": ("pop_espuma", dict(dur=2.0)),
    "rasgo_fita": ("rasgo_fita", {}),
    "clique_plugue": ("clique_plugue", {}),
    "chime_ligar": ("chime_ligar", {}),
    "tique_boot": ("tique_boot", {}),
    "ding_ui": ("ding_ui", {}),
    "obturador": ("obturador", {}),
    "swell": ("swell", dict(dur=1.75)),
    "baque_surdo": ("baque_surdo", {}),
}

# Trilha: progressao I-V-vi-IV em Fa maior, uma troca por marco do roteiro
# (beat, fracao, raiz do pulso de sub, notas do acorde). Vozes proximas para
# a troca nao "pular"; a ultima e o IV, que fica aberto, sem resolver - o que
# se quer numa cartela.
ACORDES = (
    (1, 0.0, "F2", ("F3", "A3", "C4")),
    (3, 0.0, "C2", ("E3", "G3", "C4")),
    (5, 0.0, "D2", ("D3", "F3", "A3")),
    (7, 0.0, "Bb1", ("D3", "F3", "Bb3")),
)
BPM = 80                     # pulso implicito: um sub curto na raiz a cada tempo
BRILHO = (7, 0.42, 1.2)      # oitava acima entra na travessia (a logo nasce do preto), em 1,2 s: a trilha "abre"
# Ganho da trilha ao longo dos beats: sobe na revelacao (beat 2, quando o U1
# emerge), abre na cartela e cai a zero no ultimo quadro para nao cortar seco.
DINAMICA = (
    (1, 0.000, 0.55),
    (2, 0.200, 0.55),
    (2, 0.620, 1.00),
    (6, 1.000, 1.00),
    (7, 0.420, 1.35),
    (7, 0.900, 1.35),
    (7, 1.000, 0.00),
)

NIVEL_TRILHA_DBFS = -18.0    # RMS
NIVEL_EFEITOS_DBFS = -6.0    # pico
NIVEL_MIX_DBFS = -1.0        # pico, depois do limitador
CANAIS_VSE = {"trilha": 2, "efeitos": 3}   # canal 1 fica para a imagem


# ---------------------------------------------------------------- tempo

def fator_duracao(duracao_s):
    return float(duracao_s) / DURACAO_REFERENCIA


def quadro(t, fps=FPS_REFERENCIA, fator=1.0):
    """Segundo (na referencia de DURACAO_REFERENCIA) -> quadro. Igual ao mod_coreografia."""
    return max(1, int(round(t * fps * fator)))


def quadros_do_beat(beats, n, fps=FPS_REFERENCIA, fator=1.0):
    b = beats[n - 1]
    return quadro(b["t_ini"], fps, fator), quadro(b["t_fim"], fps, fator)


def q_em(beats, n, fracao, fps=FPS_REFERENCIA, fator=1.0):
    """Quadro na fracao 'fracao' do beat n. Igual ao mod_coreografia.q_em."""
    a, b = quadros_do_beat(beats, n, fps, fator)
    return int(round(a + fracao * (b - a)))


def instante(beats, n, fracao, fps=FPS_REFERENCIA, fator=1.0):
    """Segundo em que o quadro da fracao aparece (quadro 1 = 0,0 s)."""
    return (q_em(beats, n, fracao, fps, fator) - 1) / float(fps)


def duracao_total(beats, fps=FPS_REFERENCIA, fator=1.0):
    """frame_end / fps: o video vai do quadro 1 ao quadro(t_fim do ultimo beat)."""
    return quadro(beats[-1]["t_fim"], fps, fator) / float(fps)


def cue_sheet_resolvida(beats=None, fps=FPS_REFERENCIA, fator=1.0, cue_sheet=None):
    """A cue sheet em segundos e quadros: lista de dicts, na ordem do tempo."""
    beats = tuple(beats or BEATS)
    cues = []
    for beat, fracao, efeito, ganho_db, pan in (cue_sheet or CUE_SHEET):
        q = q_em(beats, beat, fracao, fps, fator)
        cues.append({"beat": beat, "fracao": fracao, "efeito": efeito, "ganho_db": ganho_db,
                     "pan": pan, "quadro": q, "t": (q - 1) / float(fps)})
    cues.sort(key=lambda c: (c["t"], c["efeito"]))
    return cues


# ---------------------------------------------------------------- utilidades de sinal

def _t(n):
    return np.arange(n) / float(TAXA)


def _normalizar_pico(s, alvo=1.0):
    pico = float(np.max(np.abs(s))) if len(s) else 0.0
    return s * (alvo / pico) if pico > 0 else s


def _dbfs(x):
    return 20.0 * math.log10(x) if x > 0 else -float("inf")


def _rampa(t, a, b):
    """0 antes de a, 1 depois de b, cosseno no meio."""
    if b <= a:
        return (t >= a).astype(float)
    x = np.clip((t - a) / (b - a), 0.0, 1.0)
    return 0.5 - 0.5 * np.cos(np.pi * x)


def _env_assimetrico(n, apice=0.4, pot_sobe=2.0, pot_desce=1.5):
    """Envelope que sobe ate 'apice' (fracao) e cai mais devagar: um whoosh."""
    x = np.linspace(0.0, 1.0, n)
    sobe = (x / max(apice, 1e-6)) ** pot_sobe
    desce = ((1.0 - x) / max(1.0 - apice, 1e-6)) ** pot_desce
    return np.where(x < apice, sobe, desce)


def _filtrar(s, f_baixa=None, f_alta=None, transicao=0.5):
    """Passa-faixa por FFT num bloco so, bordas em cosseno de 'transicao'
    oitavas. Serve para eventos curtos e filtros fixos; a varredura no tempo
    e _ruido_passa_banda."""
    n = len(s)
    esp = np.fft.rfft(s)
    f = np.fft.rfftfreq(n, 1.0 / TAXA)
    lf = np.log2(np.maximum(f, 1e-3))
    m = np.ones_like(f)
    if f_baixa:
        x = np.clip((lf - math.log2(f_baixa)) / transicao + 0.5, 0.0, 1.0)
        m *= 0.5 - 0.5 * np.cos(np.pi * x)
    if f_alta:
        x = np.clip((math.log2(f_alta) - lf) / transicao + 0.5, 0.0, 1.0)
        m *= 0.5 - 0.5 * np.cos(np.pi * x)
    return np.fft.irfft(esp * m, n)


def _ruido_passa_banda(n, f_centro, largura_oitavas, rng, bloco=2048, salto=512):
    """Ruido branco por passa-banda cuja frequencia central varia no tempo
    (f_centro: escalar ou array de n valores, Hz). STFT com janela Hann,
    sobreposicao 4x e mascara gaussiana em log2(f) - a largura em oitavas e
    o que o ouvido percebe como "abertura" do whoosh. So numpy: um filtro
    IIR variante no tempo exigiria laco por amostra em Python."""
    f_centro = np.broadcast_to(np.asarray(f_centro, dtype=float), (n,))
    ruido = rng.standard_normal(n + bloco)
    janela = np.hanning(bloco)
    lf = np.log2(np.maximum(np.fft.rfftfreq(bloco, 1.0 / TAXA), 1.0))
    sigma = max(largura_oitavas, 0.05) / 2.0
    saida = np.zeros(n + bloco)
    for ini in range(0, n, salto):
        fc = f_centro[min(ini + bloco // 2, n - 1)]
        mascara = np.exp(-0.5 * ((lf - math.log2(fc)) / sigma) ** 2)
        esp = np.fft.rfft(ruido[ini:ini + bloco] * janela)
        saida[ini:ini + bloco] += np.fft.irfft(esp * mascara, bloco) * janela
    return saida[:n]


def _para_estereo(sinal, pan):
    """Mono + pan (numero ou (ini, fim) animado) -> estereo em potencia
    constante. Estereo + pan -> balanco. Pan 0 nao muda nada."""
    n = len(sinal)
    if isinstance(pan, (tuple, list)):
        p = np.linspace(float(pan[0]), float(pan[1]), n)
    else:
        p = np.full(n, float(pan))
    ang = (np.clip(p, -1.0, 1.0) + 1.0) * math.pi / 4.0
    if sinal.ndim == 1:
        return np.stack([sinal * np.cos(ang), sinal * np.sin(ang)], axis=1)
    return np.stack([sinal[:, 0] * np.cos(ang) * math.sqrt(2.0),
                     sinal[:, 1] * np.sin(ang) * math.sqrt(2.0)], axis=1)


def _somar_em(destino, sinal, ini):
    """Soma 'sinal' (n, 2) em 'destino' a partir da amostra ini, recortando
    o que cai fora (antes do zero ou depois do fim)."""
    n = len(sinal)
    a, b = max(ini, 0), min(ini + n, len(destino))
    if b > a:
        destino[a:b] += sinal[a - ini:b - ini]


def _freq(nome):
    """Nome de nota (C4, Bb1, F#3) -> Hz, la 440."""
    semitons = {"C": 0, "D": 2, "E": 4, "F": 5, "G": 7, "A": 9, "B": 11}
    letra, resto = nome[0], nome[1:]
    alt = 0
    while resto and resto[0] in "#b":
        alt += 1 if resto[0] == "#" else -1
        resto = resto[1:]
    midi = 12 * (int(resto) + 1) + semitons[letra] + alt
    return 440.0 * 2.0 ** ((midi - 69) / 12.0)


# ---------------------------------------------------------------- blocos de efeito

def whoosh(fator, rng, dur=1.0, f_ini=300.0, f_fim=2000.0, largura=1.0, apice=0.4):
    n = int(dur * fator * TAXA)
    x = np.linspace(0.0, 1.0, n)
    f_c = f_ini * (f_fim / f_ini) ** x            # varredura exponencial: o ouvido e logaritmico
    s = _ruido_passa_banda(n, f_c, largura, rng) * _env_assimetrico(n, apice)
    return _normalizar_pico(s), 0.0


def impacto_sub(fator, rng, f_ini=62.0, f_fim=40.0, dur=1.3):
    n = int(dur * TAXA)
    t = _t(n)
    f = f_fim + (f_ini - f_fim) * np.exp(-t / 0.12)   # o glide para baixo e o que da "peso"
    sub = np.sin(2.0 * np.pi * np.cumsum(f) / TAXA) * np.exp(-t / 0.30)
    m = int(0.006 * TAXA)
    trans = _filtrar(rng.standard_normal(m) * np.exp(-_t(m) / 0.0015), None, 2500.0)
    s = sub.copy()
    s[:m] += 0.6 * _normalizar_pico(trans)
    return _normalizar_pico(s), 0.0


def pop_espuma(fator, rng, dur=1.4, n_pops=26):
    D = dur * fator
    n = int(D * TAXA)
    saida = np.zeros((n, 2))
    # potencia > 1 concentra os pops no comeco: explode e depois rareia
    tempos = np.sort(rng.random(n_pops) ** 1.8) * D * 0.92
    for t0 in tempos:
        d = rng.uniform(0.004, 0.012)
        m = int(d * TAXA)
        g = rng.standard_normal(m) * np.exp(-np.arange(m) / (TAXA * d * 0.25))
        g = _filtrar(g, rng.uniform(700.0, 1800.0), rng.uniform(2500.0, 5200.0))
        g = _normalizar_pico(g) * rng.uniform(0.35, 1.0)
        _somar_em(saida, _para_estereo(g, rng.uniform(-0.9, 0.9)), int(t0 * TAXA))
    return _normalizar_pico(saida), 0.0


def rasgo_fita(fator, rng, dur=0.3):
    D = dur * fator
    n = int(D * TAXA)
    saida = np.zeros(n)
    t = 0.0
    while t < D:
        taxa_graos = 60.0 + 900.0 * (t / D) ** 2      # a fita descola cada vez mais rapido
        m = int(rng.uniform(0.0015, 0.004) * TAXA)
        g = rng.standard_normal(m) * np.hanning(m) * (0.25 + 0.75 * (t / D) ** 1.5)
        ini = int(t * TAXA)
        fim = min(ini + m, n)
        saida[ini:fim] += g[:fim - ini]
        t += rng.exponential(1.0 / taxa_graos)
    return _normalizar_pico(_filtrar(saida, 1200.0, 9000.0)), 0.0


def clique_plugue(fator, rng):
    n = int(0.09 * TAXA)
    t = _t(n)
    trans = _normalizar_pico(_filtrar(rng.standard_normal(n) * np.exp(-t / 0.0015), 1500.0, 9000.0))
    corpo = np.sin(2.0 * np.pi * 1000.0 * t) * np.exp(-t / 0.012)
    peso = np.sin(2.0 * np.pi * 320.0 * t) * np.exp(-t / 0.02)   # o "tock" do plastico encaixando
    return _normalizar_pico(0.9 * trans + 0.8 * corpo + 0.5 * peso), 0.0


def chime_ligar(fator, rng, f=523.25, dur=0.6):
    """C5 + G5: quinta justa, que cabe no Fa maior da trilha (C e o V)."""
    n = int(dur * TAXA)
    t = _t(n)
    env = (1.0 - np.exp(-t / 0.012)) * np.exp(-t / 0.22)
    s = (np.sin(2.0 * np.pi * f * t) + 0.8 * np.sin(2.0 * np.pi * f * 1.5 * t + 0.3)
         + 0.15 * np.sin(2.0 * np.pi * f * 2.0 * t)) * env
    sub = np.sin(2.0 * np.pi * 52.0 * t) * np.exp(-t / 0.25) * (1.0 - np.exp(-t / 0.01))
    return _normalizar_pico(0.7 * _normalizar_pico(s) + 0.8 * sub), 0.0


def tique_boot(fator, rng, n_ticks=3, passo=0.09):
    n = int((passo * n_ticks + 0.05) * TAXA)
    saida = np.zeros(n)
    m = int(0.012 * TAXA)
    t = _t(m)
    for i in range(n_ticks):
        tk = _normalizar_pico(_filtrar(rng.standard_normal(m) * np.exp(-t / 0.0012), 2000.0, 8000.0))
        tk = tk + 0.6 * np.sin(2.0 * np.pi * 2400.0 * t) * np.exp(-t / 0.004)
        ini = int(i * passo * TAXA)
        saida[ini:ini + m] += tk
    return _normalizar_pico(saida), 0.0


def ding_ui(fator, rng, f=880.0, dur=0.4):
    n = int(dur * TAXA)
    t = _t(n)
    # (multiplo, amplitude, tau): o 4,16 inarmonico e o que soa "vidro", nao "orgao"
    parciais = ((1.0, 1.0, 0.13), (2.0, 0.45, 0.09), (3.0, 0.2, 0.06), (4.16, 0.1, 0.045))
    s = sum(a * np.sin(2.0 * np.pi * f * mult * t) * np.exp(-t / tau) for mult, a, tau in parciais)
    return _normalizar_pico(s * (1.0 - np.exp(-t / 0.004))), 0.0


def obturador(fator, rng):
    n = int(0.16 * TAXA)
    saida = np.zeros(n)

    def clique(ini_s, amp, f_corpo):
        m = int(0.02 * TAXA)
        t = _t(m)
        c = _normalizar_pico(_filtrar(rng.standard_normal(m) * np.exp(-t / 0.0012), 1800.0, 10000.0))
        c = c + 0.5 * np.sin(2.0 * np.pi * f_corpo * t) * np.exp(-t / 0.005)
        ini = int(ini_s * TAXA)
        saida[ini:ini + m] += amp * c

    clique(0.0, 1.0, 1600.0)                          # espelho sobe
    m = int(0.045 * TAXA)
    cortina = _normalizar_pico(_filtrar(rng.standard_normal(m) * np.exp(-_t(m) / 0.02), 3000.0, 9000.0))
    ini = int(0.008 * TAXA)
    saida[ini:ini + m] += 0.35 * cortina              # cortina corre
    clique(0.065, 0.8, 1300.0)                        # cortina fecha
    return _normalizar_pico(saida), 0.0


def swell(fator, rng, dur=1.5, queda=0.35):
    """Cresce por 'dur' ate o apice e cai em 'queda'. A antecipacao devolvida
    e 'dur': a cue marca o apice."""
    D = dur * fator
    n_sobe, n_desce = int(D * TAXA), int(queda * TAXA)
    n = n_sobe + n_desce
    x = np.linspace(0.0, 1.0, n_sobe)
    f_c = np.concatenate([250.0 * (3000.0 / 250.0) ** x, np.full(n_desce, 3000.0)])
    env = np.concatenate([x ** 2.2, np.exp(-np.arange(n_desce) / (TAXA * 0.09))])
    saida = np.zeros((n, 2))
    for c in range(2):   # ruido independente por canal: o swell abre em largura, nao so em volume
        saida[:, c] = _ruido_passa_banda(n, f_c, 1.6, rng) * env
    return _normalizar_pico(saida), D


def baque_surdo(fator, rng):
    n = int(0.9 * TAXA)
    t = _t(n)
    f = 44.0 + 30.0 * np.exp(-t / 0.05)
    s = np.sin(2.0 * np.pi * np.cumsum(f) / TAXA) * np.exp(-t / 0.22)
    m = int(0.03 * TAXA)
    corpo = _normalizar_pico(_filtrar(rng.standard_normal(m) * np.exp(-np.arange(m) / (TAXA * 0.008)), None, 700.0))
    s[:m] += 0.9 * corpo
    return _normalizar_pico(s), 0.0


GERADORES = {
    "whoosh": whoosh, "impacto_sub": impacto_sub, "pop_espuma": pop_espuma,
    "rasgo_fita": rasgo_fita, "clique_plugue": clique_plugue, "chime_ligar": chime_ligar,
    "tique_boot": tique_boot, "ding_ui": ding_ui, "obturador": obturador,
    "swell": swell, "baque_surdo": baque_surdo,
}


# ---------------------------------------------------------------- trilha

def trilha_pad(duracao, beats, fps, fator, rng):
    """Pad de acordes (3 notas, 3 osciladores desafinados por nota, harmonicos
    leves), pulso de sub a cada tempo de 80 BPM, oitava de brilho na cartela e
    a DINAMICA por beat. Devolve estereo (n, 2), sem nivel definido."""
    n = int(round(duracao * TAXA))
    t = _t(n)
    xf = 0.8    # crossfade entre acordes, s
    inicios = [instante(beats, b, fr, fps, fator) for b, fr, _, _ in ACORDES]
    limites = inicios[1:] + [duracao + xf]
    t_brilho = instante(beats, BRILHO[0], BRILHO[1], fps, fator)
    brilho = _rampa(t, t_brilho, t_brilho + BRILHO[2])
    pad = np.zeros((n, 2))
    pulso = np.zeros(n)
    passo = 60.0 / BPM
    m_p = int(0.30 * TAXA)
    t_p = _t(m_p)
    env_p = (1.0 - np.exp(-t_p / 0.005)) * np.exp(-t_p / 0.08)
    for (b, fr, raiz, notas), t_ini, t_fim in zip(ACORDES, inicios, limites):
        g = _rampa(t, t_ini - xf / 2.0, t_ini + xf / 2.0) * (1.0 - _rampa(t, t_fim - xf / 2.0, t_fim + xf / 2.0))
        if not np.any(g > 0):
            continue
        for nome in notas:
            f0 = _freq(nome)
            for k, cents in enumerate((-6.0, 0.0, 6.0)):
                # +-6 cents batem a ~0,6-0,9 Hz nestas alturas: o "respirar" do pad
                fase = 2.0 * np.pi * f0 * 2.0 ** (cents / 1200.0) * t + rng.uniform(0.0, 2.0 * np.pi)
                voz = np.sin(fase) + 0.2 * np.sin(2.0 * fase) + 0.07 * np.sin(3.0 * fase)
                # cada oscilador desafinado vai mais para um lado: largura sem reverb
                lado = (-0.5, 0.0, 0.5)[k]
                pad[:, 0] += voz * g * (1.0 - 0.5 * lado)
                pad[:, 1] += voz * g * (1.0 + 0.5 * lado)
                pad[:, 0] += 0.35 * np.sin(2.0 * fase) * g * brilho
                pad[:, 1] += 0.35 * np.sin(2.0 * fase) * g * brilho
        # pulso na raiz do acorde vigente, a cada tempo
        f_raiz = _freq(raiz)
        k0 = int(math.ceil(max(t_ini, 0.0) / passo))
        while k0 * passo < min(t_fim, duracao):
            ini = int(k0 * passo * TAXA)
            fim = min(ini + m_p, n)
            pulso[ini:fim] += (np.sin(2.0 * np.pi * f_raiz * t_p) * env_p)[:fim - ini]
            k0 += 1
    pad = _normalizar_pico(pad)
    pulso = _normalizar_pico(pulso) * 0.22
    som = pad + np.stack([pulso, pulso], axis=1)
    tempos = [instante(beats, b, fr, fps, fator) for b, fr, _ in DINAMICA]
    ganho = np.interp(t, tempos, [g for _, _, g in DINAMICA])
    ganho *= _rampa(t, 0.0, 0.25)                    # entrada sem clique
    ganho *= 1.0 + 0.05 * np.sin(2.0 * np.pi * 0.15 * t)   # respiracao lenta
    return som * ganho[:, None]


def _reamostrar(dados, taxa_origem, taxa_destino=TAXA):
    """Interpolacao linear por canal (sem scipy). Boa para previa; para o
    final, entregar em 48 kHz."""
    n_dest = int(round(len(dados) * taxa_destino / float(taxa_origem)))
    x_orig = np.arange(len(dados)) / float(taxa_origem)
    x_dest = np.arange(n_dest) / float(taxa_destino)
    return np.stack([np.interp(x_dest, x_orig, dados[:, c]) for c in range(dados.shape[1])], axis=1)


def ler_wav(caminho):
    """WAV PCM 8/16/24/32 bits, mono ou estereo -> (float (n, 2) em [-1, 1], taxa)."""
    with wave.open(caminho, "rb") as w:
        canais, largura, taxa, n = w.getnchannels(), w.getsampwidth(), w.getframerate(), w.getnframes()
        bruto = w.readframes(n)
    if largura == 1:
        dados = (np.frombuffer(bruto, dtype=np.uint8).astype(float) - 128.0) / 128.0
    elif largura == 2:
        dados = np.frombuffer(bruto, dtype="<i2").astype(float) / 32768.0
    elif largura == 3:
        b = np.frombuffer(bruto, dtype=np.uint8).reshape(-1, 3).astype(np.int32)
        inteiro = b[:, 0] | (b[:, 1] << 8) | (b[:, 2] << 16)
        inteiro = np.where(inteiro >= 1 << 23, inteiro - (1 << 24), inteiro)
        dados = inteiro.astype(float) / float(1 << 23)
    elif largura == 4:
        dados = np.frombuffer(bruto, dtype="<i4").astype(float) / float(1 << 31)
    else:
        raise ValueError("WAV com %d bytes por amostra nao e suportado" % largura)
    dados = dados.reshape(-1, canais)
    if canais == 1:
        dados = np.repeat(dados, 2, axis=1)
    elif canais > 2:
        dados = dados[:, :2]
    return dados, taxa


def trilha_externa_carregada(caminho, duracao):
    """Carrega o WAV do cliente no formato interno: 48 kHz, estereo, com a
    duracao do video (corta ou completa com silencio) e fade de saida."""
    dados, taxa = ler_wav(caminho)
    if taxa != TAXA:
        print("[som] AVISO: trilha externa a %d Hz reamostrada para %d Hz por interpolacao linear; "
              "para o final, entregue em 48 kHz" % (taxa, TAXA))
        dados = _reamostrar(dados, taxa)
    n = int(round(duracao * TAXA))
    if len(dados) < n:
        print("[som] AVISO: trilha externa tem %.2f s, o video tem %.2f s; o resto fica em silencio"
              % (len(dados) / float(TAXA), duracao))
        dados = np.concatenate([dados, np.zeros((n - len(dados), 2))])
    dados = dados[:n]
    t = _t(n)
    return dados * (1.0 - _rampa(t, duracao - 0.5, duracao))[:, None]


# ---------------------------------------------------------------- WAV e medidas

def gravar_wav(caminho, dados):
    """Estereo float -> WAV PCM 16-bit, 48 kHz."""
    dados = np.asarray(dados, dtype=float)
    if dados.ndim == 1:
        dados = np.stack([dados, dados], axis=1)
    inteiros = np.clip(np.round(dados * 32767.0), -32768, 32767).astype("<i2")
    with wave.open(caminho, "wb") as w:
        w.setnchannels(2)
        w.setsampwidth(2)
        w.setframerate(TAXA)
        w.writeframes(inteiros.tobytes())
    return caminho


def medir(caminho):
    """Duracao, pico e RMS (dBFS) lidos DO ARQUIVO gravado, nao do array."""
    dados, taxa = ler_wav(caminho)
    pico = float(np.max(np.abs(dados))) if len(dados) else 0.0
    rms = float(np.sqrt(np.mean(dados ** 2))) if len(dados) else 0.0
    return {"duracao_s": len(dados) / float(taxa), "taxa": taxa, "amostras": len(dados),
            "pico_dbfs": _dbfs(pico), "rms_dbfs": _dbfs(rms)}


def _limitador(s, limiar=0.7):
    """Soft clip: linear ate 'limiar', tanh acima. Depois normaliza-se o pico;
    o que o limitador faz e tirar as pontas onde trilha e efeito coincidem."""
    a = np.abs(s)
    acima = limiar + (1.0 - limiar) * np.tanh((a - limiar) / (1.0 - limiar))
    return np.sign(s) * np.where(a > limiar, acima, a)


# ---------------------------------------------------------------- API

def _rng_da_cue(cue):
    """Gerador aleatorio proprio de cada cue, semeado pelo que a identifica
    (beat, fracao, efeito) e nao pela posicao na lista: acrescentar ou tirar
    uma cue nao muda o som das outras, e o teste consegue sintetizar uma
    cue isolada identica a que esta no stem."""
    return np.random.default_rng([SEMENTE, cue["beat"], int(round(cue["fracao"] * 1e6)),
                                  sum(ord(c) for c in cue["efeito"])])


def sintetizar_cue(cue, fator=1.0):
    """Uma cue resolvida -> (sinal estereo com ganho e pan, amostra inicial).
    Registra na cue 't_colocado' e 'antecipacao' (s). E o unico caminho de
    sintese de efeito: gerar_stems e o teste passam por aqui."""
    gerador, kw = EFEITOS[cue["efeito"]]
    sinal, antecipacao = GERADORES[gerador](fator=fator, rng=_rng_da_cue(cue), **kw)
    estereo = _para_estereo(sinal, cue["pan"]) * 10.0 ** (cue["ganho_db"] / 20.0)
    ini = int(round((cue["t"] - antecipacao) * TAXA))
    cue["antecipacao"] = antecipacao
    cue["t_colocado"] = ini / float(TAXA)
    return estereo, ini


def gerar_stems(pasta, fps=30, beats=None, fator=1.0, trilha_externa=None, cue_sheet=None):
    """Sintetiza e grava som_trilha.wav, som_efeitos.wav e som_mix.wav em
    'pasta'. Devolve {'trilha', 'efeitos', 'mix': caminhos, 'ganho_vse': float,
    'duracao_s': float, 'cues': lista}. 'beats' e a tabela da coreografia
    (dicts com n, t_ini, t_fim); 'fator' e o da duracao (20 s = 0,8; 15 s = 0,6).
    'trilha_externa': WAV que substitui o pad; None procura
    <pasta>/trilha_externa.wav e assets/trilha_externa.wav ao lado do modulo."""
    beats = tuple(beats or BEATS)
    fps = float(fps)
    os.makedirs(pasta, exist_ok=True)
    duracao = duracao_total(beats, fps, fator)
    n = int(round(duracao * TAXA))
    rng = np.random.default_rng([SEMENTE, 0])      # o da trilha; cada cue tem o seu
    cues = cue_sheet_resolvida(beats, fps, fator, cue_sheet)

    efeitos = np.zeros((n, 2))
    for cue in cues:
        estereo, ini = sintetizar_cue(cue, fator)
        _somar_em(efeitos, estereo, ini)
    efeitos = _normalizar_pico(efeitos, 10.0 ** (NIVEL_EFEITOS_DBFS / 20.0))

    if trilha_externa is None:
        candidatos = [os.path.join(pasta, "trilha_externa.wav")]
        try:
            candidatos.append(os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
                                           "assets", "trilha_externa.wav"))
        except NameError:
            pass    # aba Scripting: sem __file__; so o caminho explicito vale
        trilha_externa = next((c for c in candidatos if os.path.exists(c)), None)
    if trilha_externa:
        print("[som] trilha EXTERNA:", trilha_externa)
        trilha = trilha_externa_carregada(trilha_externa, duracao)
        origem_trilha = trilha_externa
    else:
        trilha = trilha_pad(duracao, beats, fps, fator, rng)
        origem_trilha = "pad sintetizado (provisorio)"
    rms = float(np.sqrt(np.mean(trilha ** 2)))
    if rms > 0:
        trilha *= 10.0 ** (NIVEL_TRILHA_DBFS / 20.0) / rms

    soma = trilha + efeitos
    pico_soma = float(np.max(np.abs(soma)))
    alvo_mix = 10.0 ** (NIVEL_MIX_DBFS / 20.0) * 0.999    # 0,999: pico estritamente abaixo de -1 dBFS
    ganho_vse = min(1.0, alvo_mix / pico_soma) if pico_soma > 0 else 1.0
    mix = _normalizar_pico(_limitador(soma), alvo_mix)

    caminhos = {
        "trilha": gravar_wav(os.path.join(pasta, "som_trilha.wav"), trilha),
        "efeitos": gravar_wav(os.path.join(pasta, "som_efeitos.wav"), efeitos),
        "mix": gravar_wav(os.path.join(pasta, "som_mix.wav"), mix),
    }
    caminhos.update({"ganho_vse": ganho_vse, "duracao_s": duracao, "cues": cues,
                     "origem_trilha": origem_trilha})
    print("[som] %.2f s, %d cues, trilha: %s, soma trilha+efeitos pico %.2f dBFS -> ganho no VSE %.3f"
          % (duracao, len(cues), origem_trilha, _dbfs(pico_soma), ganho_vse))
    return caminhos


def _faixas_do_vse(cena):
    if cena.sequence_editor is None:
        cena.sequence_editor_create()
    # 4.4+ renomeou sequences -> strips; no 4.2 'sequences' existe e esta
    # VAZIA - testar por None, nao por verdade (medido pela coreografia).
    faixas = getattr(cena.sequence_editor, "sequences", None)
    if faixas is None:
        faixas = cena.sequence_editor.strips
    return faixas


def montar_no_vse(cena, stems, beats=None, fps=30, empacotar=True):
    """Poe trilha e efeitos no VSE (canais 2 e 3, a partir do quadro 1), liga
    AAC 192 kbps / 48 kHz / estereo na saida FFMPEG e confere o comprimento.
    Idempotente: faixas 'som.*' anteriores saem antes. 'fps' e o da CENA (o
    da previa e 15; o do cliente, 30): decide o comprimento em quadros da
    faixa. Devolve {'trilha': strip, 'efeitos': strip}."""
    import bpy
    fps = float(fps)
    if abs(cena.render.fps / cena.render.fps_base - fps) > 1e-6:
        # O comprimento da faixa de som e calculado com o fps da cena NA HORA
        # de criar; mudar depois deixa a faixa com o tamanho errado.
        print("[som] AVISO: cena a %.3f fps, montar_no_vse pediu %.3f; ajustando a cena antes das faixas"
              % (cena.render.fps / cena.render.fps_base, fps))
        cena.render.fps = int(round(fps))
        cena.render.fps_base = 1.0
    faixas = _faixas_do_vse(cena)
    for s in [s for s in faixas if s.name.startswith("som.")]:
        faixas.remove(s)
    for som in [s for s in bpy.data.sounds if s.name.startswith("som_") and s.users == 0]:
        bpy.data.sounds.remove(som)
    criadas = {}
    for nome, canal in CANAIS_VSE.items():
        caminho = stems.get(nome)
        if not caminho or not os.path.exists(caminho):
            print("[som] AVISO: stem '%s' nao encontrado, faixa nao criada" % nome)
            continue
        faixa = faixas.new_sound("som." + nome, caminho, canal, 1)
        faixa.volume = float(stems.get("ganho_vse", 1.0))
        try:
            faixa.show_waveform = True
        except AttributeError:
            pass
        if empacotar:
            try:
                faixa.sound.pack()     # o .blend gravado nao pode depender do %TEMP%
            except (AttributeError, RuntimeError) as e:
                print("[som] nao empacotou %s: %s" % (nome, e))
        criadas[nome] = faixa
        esperado = int(round(float(stems.get("duracao_s", 0.0)) * fps))
        dur = faixa.frame_final_duration
        estado = "ok" if abs(dur - esperado) <= 1 else "DIFERENTE do esperado %d" % esperado
        print("[som] faixa %s: canal %d, quadros 1..%d (%s), volume %.3f, %s"
              % (faixa.name, canal, dur, estado, faixa.volume, caminho))
    r = cena.render
    r.use_sequencer = True                        # faixas so de som nao trocam o render para o VSE
    r.ffmpeg.audio_codec = "AAC"
    r.ffmpeg.audio_bitrate = 192
    r.ffmpeg.audio_mixrate = TAXA
    try:
        r.ffmpeg.audio_channels = "STEREO"
    except (AttributeError, TypeError):
        pass
    if r.image_settings.file_format != "FFMPEG":
        print("[som] AVISO: a saida esta em %s; o audio so entra no container FFMPEG "
              "(configurar_render(video=True))" % r.image_settings.file_format)
    # As cues em quadros DESTA cena (a previa toca a 15 fps: quadro 2k-1 da
    # coreografia vira o k da previa), para conferir na timeline do VSE.
    for cue in stems.get("cues", []):
        print("[som]   %6.3f s  quadro %3d  %-18s %+5.1f dB" % (
            cue["t"], int(round(cue["t"] * fps)) + 1, cue["efeito"], cue["ganho_db"]))
    return criadas
