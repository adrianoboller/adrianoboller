# Anúncio cinemático — Snapmaker U1 (EnginePrint)

Comercial de produto em 3D, **9:16 vertical, 20 s a 30 fps**, feito no Blender
por script. Tudo — caixa, U1 substituto, espuma, cabo, luz, câmera, tela,
cartela e a coreografia dos sete beats — nasce de **um arquivo Python** que
você cola na aba Scripting e roda. Nada para importar, nada para baixar.

## Como rodar no seu Blender (4.2 ou mais novo)

1. Abra o Blender com a sua cena (a do U1, ou uma vazia).
2. Aba **Scripting** → **New** → cole o conteúdo de `scripts/anuncio_u1.py`.
3. Ajuste o bloco **PARÂMETROS** no topo (abaixo).
4. **Run Script**. Leva alguns segundos: monta a coleção `ANUNCIO`,
   coreografa 600 quadros, configura o render e grava `anuncio_u1.blend` ao
   lado do seu arquivo.
5. **Render → Render Animation.** Na RTX 4050, com EEVEE Next a 1080×1920 e
   64 amostras, conte de 20 a 40 minutos. O motion blur está ligado; o vidro
   usa raytracing.

Rodar de novo não duplica nada: cada módulo apaga a própria coleção antes,
e o seu modelo volta à pose original antes de ser medido de novo.

## Parâmetros que você decide

| Parâmetro | Padrão | O que faz |
|---|---|---|
| `U1_NOME` | `""` | Nome do **objeto ou coleção** do seu U1. Vazio usa o substituto. |
| `U1_ROTACAO_Z` | `0` | Graus para a frente do seu modelo apontar para −Y. |
| `U1_TELA`, `U1_TOMADA`, `U1_BOTAO` | vazios | XYZ nas coordenadas originais do seu arquivo. Vazios saem de heurística pelo envelope. |
| `U1_TELA_OBJETO`, `U1_BOTAO_OBJETO` | vazios | Objetos do seu modelo que acendem e afundam. A tela precisa de material com Emission. |
| `DURACAO_S` | `20` | `15` é o preset frenético (só conferido por número, não por render). |
| `CAIXA_SOME` | `True` | A caixa afunda pelo chão no beat 2 e volta no 6. `False`: o U1 pousa na frente dela. |
| `ESPUMA_SOME_NOS_CLOSES` | `True` | Os flocos somem do chão nos beats 3 a 5, com fade. |
| `ESCONDER_RESTO` | `False` | `True` tira do render objetos seus fora de `ANUNCIO`. O script avisa quais estão visíveis. |
| `COR_CAIXA` | `"clara"` | `"escura"` existe mas não foi renderizada; a logo escura some sobre ela. |
| `RESOLUCAO`, `AMOSTRAS` | `(1080, 1920)`, `64` | Render final. |
| `SALVAR_BLEND` | `True` | Grava `anuncio_u1.blend` ao lado do seu `.blend`. |

## O que é provisório e o que você ainda me deve

- **Logo.** `assets/logo_engineprint.png` é uma **provisória** desenhada por
  mim a partir da imagem que veio colada na conversa. Mande a oficial em PNG
  com fundo transparente, ou SVG. Ela entra pelo mesmo nome e eu remonto o
  arquivo único.
- **Tela do U1.** Boot e interface são aproximações em HTML
  (`assets/tela_ui_fonte.html`) da tela real, com o wordmark em texto. Se
  tiver capturas oficiais, mande.
- **Seu modelo.** Tudo foi validado com o **substituto** paramétrico nas
  medidas reais. Rode `scripts/01_diagnostico.py` na sua cena e me mande o
  `u1_diagnostico.txt` para eu ajustar tela, tomada e botão ao seu modelo.

## Decisões de direção que ficaram comigo (e você pode virar)

- **20 s em vez de 15.** Sete beats em 15 s ficam frenéticos e a cartela de
  quatro linhas não se lê em 2 s. O preset de 15 existe.
- **Caixa some pelo chão** no beat 2 (`CAIXA_SOME`). O storyboard só diz "o
  U1 sai da caixa". Com `False` ele pousa na frente dela.
- **Linha 3 da cartela em cobre**, a cor da logo. `linha_destaque=None`
  no módulo tira.
- **Travessia pelo centro da logo**, que é vazado: na tela o corte vai
  branco → preto → logo. Mirar num dente daria engrenagem → preto → logo.
- **Momento da revelação** (beat 2): o U1 branco recorta contra o rosé
  escurecido por 1,2 s. A lateral não aparece nessa câmera; girá-la alguns
  graus é a outra alavanca.

## Pipeline daqui (para quem continuar)

- `docs/ESPECIFICACAO.md` — o contrato: eixos, unidades, nomes, API, paleta,
  linha do tempo por beat.
- `scripts/mod_*.py` — um módulo por peça; `scripts/teste_*.py` prova cada um
  renderizando e **olhando** o PNG.
- `scripts/montar.py` — concatena os módulos e embute os PNGs em base64 no
  `anuncio_u1.py`. Mexeu num módulo, roda de novo.
- `scripts/previa.sh` — EEVEE Next **sem GPU** (Xvfb + llvmpipe). Cubo: ~6 s
  por quadro; cena completa: 27 a 40 s.
- `scripts/lotes.sh` + `MODO=video scripts/teste_coreografia.py` — prévia
  em vídeo: 300 quadros (1 a cada 2) a 360×640 em lotes de 14, depois o MP4 a
  15 fps pelo ffmpeg do Blender.
- `docs/REVISAO-RODADA-1.md` — o que os revisores mediram na primeira
  passagem; as rodadas 2 e 3 estão nas mensagens de commit.

## Ficha técnica do U1 (referência)

584 × 499 × 730 mm, 18,2 kg, volume 270³, 4 cabeçotes, bico 0,4 mm até
300 °C, mesa PEI até 100 °C, CoreXY 500 mm/s, tela 3,5" 480×320. Corpo
**branco** com aro e moldura da porta pretos, porta de vidro na frente,
painel traseiro transparente — confirmado no guia rápido oficial.
Fonte: <https://www.snapmaker.com/snapmaker-u1/specs>
