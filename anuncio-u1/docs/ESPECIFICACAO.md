# Especificação — anúncio Snapmaker U1 (EnginePrint)

Documento que todos os módulos seguem. Quem constrói um módulo lê isto
inteiro antes de escrever a primeira linha.

## O que estamos fazendo

Anúncio de produto em 3D, **9:16 vertical**, para Reels/TikTok/Shorts, do
Snapmaker U1 vendido pela **EnginePrint** (engineprint.com.br). Estilo
**Apple**: limpo, suave, luz de estúdio, fundo em gradiente preto → rosé‑branco,
movimento de câmera contínuo e "smooth". Nada de mãos: cabo conecta e botão
aperta "como mágica".

Motor final: **EEVEE Next** na RTX 4050 do cliente. Prévia aqui: o mesmo
EEVEE Next por software (`scripts/previa.sh`), ~6 s/quadro em 540×960.

O modelo real do U1 está no Blender do cliente e **não viaja até aqui**. O
entregável é UM arquivo Python que ele cola na aba Scripting e roda sobre a
cena dele. Aqui construímos e validamos tudo com um **U1 substituto**
paramétrico nas dimensões reais; o arquivo final aceita o modelo dele por
nome e usa o substituto quando não encontra.

## Storyboard (do cliente, em beats)

| # | Beat | O que acontece |
|---|---|---|
| 1 | Caixa sobe | Caixa de alta qualidade com a logo **impressa** (plana, não 3D) no **topo da tampa** sobe de baixo para cima girando em velocidade média‑rápida |
| 2 | Abre | Tampa abre; **espuminhas** voam junto; o U1 aparece e sai da caixa |
| 3 | Traseira | Câmera vira para a **traseira** do U1; cabo de energia **conecta**; botão **liga** — sem mãos |
| 4 | Tela | Câmera volta à frente, **foca na tela**; tela de **boot** (carrega bem rápido) → **interface do U1** |
| 5 | "Fotos" | Câmera "tira fotos": ângulos **bem perto**, iluminação cinemática, **sem mostrar o corpo inteiro**, produto ancorado no **canto inferior direito** do quadro |
| 6 | Volta | U1 entra de volta na caixa; tampa fecha |
| 7 | Cartela | Câmera foca no topo da caixa, **alinhada e centrada na logo**, aproxima até **atravessar** a logo; aparece o texto: **EnginePrint** / **qualidade excepcional** / **13 unidades restantes** / **compre em engineprint.com.br** |

Fundo: gradiente **preto mesclado com rosé‑branco**, "bem profissional estilo
Apple".

## Linha do tempo

30 fps. Duração de referência **20 s = 600 quadros**. O cliente pediu 15 s;
sete beats em 15 s ficam frenéticos e a cartela com quatro linhas fica
ilegível em 2 s — por isso a linha do tempo é **parametrizada por beat** e
existe o preset de 15 s (fator 0,75). A prévia sai em 20 s; ele decide.

| # | Beat | Início (s) | Fim (s) | Quadros (20 s) |
|---|---|---|---|---|
| 1 | Caixa sobe girando | 0,0 | 2,5 | 1–75 |
| 2 | Tampa abre, espuma, U1 emerge | 2,5 | 5,5 | 75–165 |
| 3 | Órbita p/ trás, cabo, botão | 5,5 | 9,0 | 165–270 |
| 4 | Frente, dolly na tela, boot → UI | 9,0 | 12,0 | 270–360 |
| 5 | Três "fotos" com flash | 12,0 | 15,0 | 360–450 |
| 6 | U1 desce, tampa fecha | 15,0 | 17,0 | 450–510 |
| 7 | Câmera na logo, atravessa, cartela | 17,0 | 20,0 | 510–600 |

A tabela vive em `mod_coreografia.py` como dado, não como número solto.

## Convenções de cena (obrigatórias)

- **Unidades:** metros, `scale_length = 1.0`. **30 fps.** Render 1080×1920.
- **Eixos:** Z para cima. A **frente** do produto e da caixa aponta para **−Y**
  (câmera em −Y olhando +Y vê a frente). Chão em **z = 0**.
- **Origem:** centro da base da caixa em (0, 0, 0).
- **U1:** 0,584 (X, largura) × 0,499 (Y, profundidade) × 0,730 (Z, altura) m.
- **Caixa:** rígida, **tampa solta** (estilo caixa de iPhone, não aba de
  papelão). Interior = U1 + folga de espuma: **0,66 × 0,58 × 0,80 m**; parede
  8 mm; tampa com 0,12 m de altura, encaixe com 2 mm de folga.
- **Coleção raiz:** `ANUNCIO`; cada módulo cria a sua sub‑coleção com o nome
  do módulo (`caixa`, `u1`, `ambiente`, `cabo`, `cartela`, `camera`).
- **Nomes:** `<modulo>.<peca>` — `caixa.corpo`, `caixa.tampa`, `caixa.logo`,
  `caixa.espuma.001`…, `u1.corpo`, `u1.tela`, `u1.botao`, `u1.tomada`,
  `u1.cabecote.1`…`4`, `u1.mesa`, `u1.porta`, `cabo.curva`, `cabo.plugue`,
  `cartela.linha.1`…`4`, `camera.principal`, `camera.alvo`.
- **Idempotente:** rodar duas vezes não duplica nada — todo módulo chama
  `limpar_colecao(nome)` antes de construir.
- **Só `bpy`, `bmesh`, `math`, `mathutils`, `random` (com semente fixa).**
  Nada de pip. Tem de rodar no Blender **4.2 e posteriores**; diferença de API
  entre versões vai em `try/except` com comentário do porquê.
- Motor: `'BLENDER_EEVEE_NEXT'`; se não existir (Blender 5), cair para
  `'BLENDER_EEVEE'`.

## API de módulo

Arquivo `scripts/mod_<nome>.py`. Nada roda no import: só definições.

```python
def construir_<nome>(cena, colecao_pai, params: dict) -> dict:
    """Cria os objetos na sub-colecao <nome> e devolve referencias e medidas."""

def animar_<acao>(objs: dict, quadro_ini: int, quadro_fim: int, **kw) -> None:
    """Insere keyframes SO nos proprios objetos. Easing suave por padrao."""
```

- `params` sempre tem defaults; o dict devolvido traz os objetos por nome
  curto (`{"corpo": obj, "tampa": obj, ...}`) e medidas úteis
  (`"altura_tampa"`, `"interior"`, `"posicao_tela"`, `"posicao_tomada"`…).
- Keyframes: Bézier com `easing='EASE_IN_OUT'` salvo pedido diferente; a
  função aceita `easing=` para a coreografia trocar.
- Um módulo **nunca** toca objeto de outro. Quem integra é
  `mod_coreografia.py`.

Cada módulo vem com `scripts/teste_<nome>.py`, que numa cena vazia constrói
só aquele módulo com câmera e luz simples, e renderiza `saida/previa_<nome>.png`
(mecanismo animado: três quadros, `_ini`, `_meio`, `_fim`). **Quem escreve o
módulo abre o PNG e olha.** Componente novo se prova exercitando — a lição do
projeto é que ler o código não acha o que a imagem mostra.

## Materiais — EEVEE primeiro

- Só **Principled BSDF** e Emission. Nada de OSL, nada de volume pesado.
- Transparência: `mat.surface_render_method = 'BLENDED'` (4.2+); em 4.1 e
  antes é `blend_method`. Guardar com `try/except`.
- Vidro/acrílico: Principled com Transmission; em EEVEE precisa de
  `use_screen_refraction`/raytracing ligado no módulo `ambiente`.
- Gerenciamento de cor: **AgX**, look "AgX - Medium High Contrast".
- Metal escovado: anisotropia ≥ 0,6 com rotação por face (o U1 vive disso).

## Paleta

| Uso | Cor |
|---|---|
| Fundo, ponto escuro | `#050507` |
| Fundo, ponto claro (rosé‑branco) | `#F4E6E4` |
| Caixa (variante clara, padrão) | `#F2EDE6` fosca, toque de papel |
| Caixa (variante escura) | `#141416` fosca |
| U1 corpo | **branco** `#F0F0EE`, painel plástico injetado semi‑fosco, aro superior e moldura da porta pretos — confirmado nas fotos oficiais do guia rápido (a primeira versão desta tabela dizia grafite e estava errada) |
| U1 detalhes | alumínio escovado; hastes de fibra de carbono; mesa PEI dourada texturizada |
| Espuma | branca fosca `#F6F6F4` |
| Cabo | borracha preta `#111111`, plugue IEC C13 |
| Cartela, texto | branco `#FFFFFF` sobre o gradiente; peso fino, tracking largo |

Sem serrilhado no gradiente: aplicar ruído sutil (dither) na cor do fundo.

## Fatos do U1 real (para o substituto)

- Painéis plásticos injetados **brancos** escondem a mecânica; aro superior e moldura da porta pretos; visual limpo e industrial.
- **Porta de vidro na frente**; **painel traseiro transparente**.
- **Tela 3,5" (480×320 paisagem) embutida, nivelada com o painel frontal.**
- CoreXY; hastes de fibra de carbono; 4 cabeçotes com estacionamento.
- Botão liga/desliga e **tomada IEC na traseira**.
- Volume 270³ mm; mesa de aço flexível com PEI.
- Quem modelar o substituto pesquisa fotos antes (site oficial, reviews) e
  anota no cabeçalho do módulo o que confirmou e o que chutou. **Ficha
  oficial:** <https://www.snapmaker.com/snapmaker-u1/specs>.

## Assets

- `assets/logo_engineprint.png` — 1024², RGBA. Hoje é **provisória**
  (gerada em `logo_provisoria.html`); a oficial substitui pelo mesmo nome.
- `assets/tela_boot.png`, `assets/tela_ui.png` — 480×320. Boot: fundo
  preto, wordmark "Snapmaker" em texto, barra de progresso. UI: aproximação da
  tela inicial do U1 (pesquisar capturas; os assets oficiais do cliente
  substituem pelo mesmo nome). Geradas por HTML → PNG com o Chromium em
  `/opt/pw-browsers/chromium-*/chrome-linux/chrome --headless=new --screenshot`.
- Sem PIL no sistema. Imagem procedural se faz em `bpy.data.images` +
  `numpy` (o Blender traz numpy).

## Ferramentas neste container

- Blender 4.2.5: `/tmp/claude-0/-home-user-adrianoboller/857c9466-78dd-51a3-8b8e-9f13227d5fd4/scratchpad/blender/blender`
- Prévia EEVEE sem GPU: `bash scripts/previa.sh scripts/teste_<nome>.py`
- Vídeo: o Blender embute ffmpeg — `render.image_settings.file_format='FFMPEG'`.
- Chromium headless para HTML → PNG (caminho acima).

## Regras de estilo (do projeto)

- Código, comentários e documentação em **português**; identificadores e
  comentários **sem acento**. Comentário explica **por quê**.
- Número visível sai de medição, não de chute.
- Cada módulo escreve **só nos seus arquivos**: `scripts/mod_<nome>.py`,
  `scripts/teste_<nome>.py`, `saida/previa_<nome>*.png`, `assets/<nome>_*`.
  Ninguém commita; o commit é único no fim.
