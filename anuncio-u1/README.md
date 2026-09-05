# Anúncio cinemático — Snapmaker U1 (EnginePrint)

Comercial de produto em 3D, **9:16 vertical, 25 s a 30 fps, com som**, feito
no Blender por script, com **a caixa e a impressora enviadas pelo Adriano**
(modelos Meshy, limpos e remodelados) num vazio sem chão com gradiente
preto e rosé mesclado. Estilo de movimento do @nzj.3d: entradas com
overshoot curto, câmera calma e sempre perto.

## Como rodar no seu Blender (4.2 ou mais novo)

O entregável é `saida/anuncio_u1_pacote.zip` (regenerável por
`python3 scripts/empacotar.py`):

```
anuncio_u1.py
assets/
  impressora_limpa.glb          sua impressora, limpa (25,8 MB)
  caixa_cor_2k.png              sua caixa, textura transferida por bake
  caixa_normal_2k.png
  caixa_rugosidade_2k.png
  caixa_etiqueta_cor.png        a etiqueta pendurada, peça 3D própria
  caixa_etiqueta_normal.png
  caixa_etiqueta_rugosidade.png
  caixa_etiqueta_malha.png      a malha da etiqueta, como bytes
```

1. Descompacte. **Salve um `.blend`** na mesma pasta do `anuncio_u1.py`
   (a pasta `assets/` precisa ficar ao lado do `.blend`).
2. Aba **Scripting** → **Open** (ou **New** e cole) `anuncio_u1.py`.
3. Ajuste o bloco **PARÂMETROS** no topo, se quiser.
4. **Run Script.** Alguns segundos: monta a coleção `ANUNCIO`, coreografa
   750 quadros, gera o som, configura o render e grava `anuncio_u1.blend`.
5. **Render → Render Animation.** Sai `anuncio_u1.mp4`, 1080×1920, H.264
   com **AAC**. Na RTX 4050, EEVEE Next a 64 amostras, conte de 30 a 50
   minutos.

Se o script não achar `assets/`, ele para **antes de tocar na cena** e lista
onde procurou e o que falta. Rodar de novo não duplica nada.

## Parâmetros

| Parâmetro | Padrão | O que faz |
|---|---|---|
| `PASTA_ASSETS` | `""` | Vazio procura `assets/` ao lado do `.blend`; ou aponte o caminho. |
| `DURACAO_S` | `25` | Presets 15 / 20 / 25. O tempo extra vai para a revelação, a tela e a cartela. |
| `COM_SOM` | `True` | Trilha e efeitos sintetizados no próprio Blender, mixados no MP4. |
| `TRILHA_EXTERNA` | `""` | Caminho de um WAV seu (48 kHz) para substituir a trilha sintetizada; `assets/trilha_externa.wav` também é aceito sozinho. |
| `U1_NOME` | `""` | Vazio usa a sua impressora Meshy. Nome de objeto/coleção troca por outro modelo seu. |
| `U1_ROTACAO_Z`, `U1_TELA`, `U1_TOMADA`, `U1_BOTAO` | | Só para modelo trocado por `U1_NOME`. |
| `CAIXA_SOME` | `True` | A caixa desce para fora do quadro no beat 2 e volta no 6. |
| `ESPUMA_SOME_NOS_CLOSES` | `True` | Flocos somem com fade nos beats 3 a 5. |
| `ESCONDER_RESTO` | `False` | `True` tira do render objetos seus fora de `ANUNCIO`. |
| `RESOLUCAO`, `AMOSTRAS` | `(1080, 1920)`, `64` | Render final. |
| `SALVAR_BLEND` | `True` | Grava `anuncio_u1.blend` ao lado. |

## O que é provisório

- **Logo.** `logo_engineprint.png` (embutida) é uma provisória desenhada a
  partir da imagem colada na conversa. Aparece **só na cartela**; a caixa não
  tem logo, a pedido. Mande a oficial em PNG transparente ou SVG.
- **Trilha.** Sintetizada, para provar timing e clima. Um anúncio de verdade
  usa música licenciada: `TRILHA_EXTERNA`.
- **Tela da impressora.** Boot e interface são aproximações em HTML.

## Decisões de direção (todas reversíveis por parâmetro ou pedido)

- **A impressora é a Meshy** que você mandou, por sua decisão depois do
  aviso de que não é um U1 (é branca com tubos de filamento e bobinas). Se o
  arquivo real da Snapmaker chegar, entra por `U1_NOME`.
- **Caixa:** geometria limpa de 14 faces e 4 abas com a textura da sua caixa
  transferida por bake; ícones esticam 1,25× na vertical para caber a
  impressora. A etiqueta pendurada é peça 3D.
- **Sem chão:** tudo flutua; a caixa entra e sai por baixo do quadro; a
  espuma sobe pela boca e sai do quadro; o cabo pende.
- **Beat 7:** a câmera mergulha no centro do topo da caixa (fita) e a logo
  nasce na cartela, agora com cinco linhas: a segunda é **Snapmaker U1**.
- **Revisão 4:** a caixa entra pairando (giro de 90° em todo o beat, no
  máximo 1,5° por quadro, com balanço de 2,5 cm que continua depois de
  parar); a espuma é packing peanut creme‑amarelo em S, 96 flocos; o cabo
  vem reto na horizontal, alinhado com a tomada; "Snapmaker U1" aparece
  como legenda fina ao lado da impressora na revelação e na cartela.
- **Linha "13 unidades restantes" em cobre**, cor da logo.

## Pipeline daqui (para quem continuar)

- `docs/ESPECIFICACAO.md` — o contrato, com as Revisões 2 e 3 no fim.
- `scripts/mod_*.py` + `scripts/teste_*.py` — um módulo por peça, provado
  renderizando e **olhando** o PNG. `mod_som.py` sintetiza o áudio e sua
  cue sheet lê as frações do `ROTEIRO` da coreografia.
- `scripts/limpar_impressora.py`, `scripts/bake_caixa.py` — rodam uma vez;
  produzem os assets externos a partir dos GLB Meshy (no scratchpad, fora do
  repositório; o `.glb` original usa `EXT_meshopt_compression`, que o Blender
  não importa; descomprima com `npx @gltf-transform/cli copy`).
- `scripts/montar.py` → `anuncio_u1.py`; `scripts/empacotar.py` → zip, com
  prova de autossuficiência (`--provar`).
- `scripts/previa.sh` — EEVEE Next sem GPU (Xvfb + llvmpipe), 27 a 40 s por
  quadro na cena completa. `scripts/lotes.sh` + `scripts/video_com_som.py` —
  prévia em vídeo com áudio (375 quadros de 750, 15 fps).
- `docs/REVISAO-RODADA-1.md` — a primeira revisão medida; as demais estão
  nas mensagens de commit.

## Ficha técnica do U1 (referência de medidas)

584 × 499 × 730 mm, volume 270³, 4 cabeçotes, tela 3,5" 480×320.
Fonte: <https://www.snapmaker.com/snapmaker-u1/specs>
