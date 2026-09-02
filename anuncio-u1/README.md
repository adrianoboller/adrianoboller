# Anúncio cinemático — Snapmaker U1

Comercial de produto em 3D, feito no Blender por script. O modelo do U1 fica
na máquina do Adriano; o container que escreve os scripts não a enxerga. Por
isso a cena **não viaja**: o script é que vai até ela.

## Divisão do trabalho

| Onde | O quê |
|---|---|
| Container (Blender 4.2.5 headless, 4 núcleos) | escreve os scripts, roda, corrige, renderiza prévias em baixa resolução (Cycles CPU) |
| PC do Adriano (i7, RTX 4050, 32 GB) | abre o `.blend` e renderiza o filme final (EEVEE Next) |

## Decidido

- **Formato:** 9:16 vertical, 15 s — Reels / TikTok / Shorts.
- **Motor:** EEVEE Next. O filme inteiro sai em 15–40 min na 4050, o que
  permite rodada de correção no mesmo dia. Em Cycles cada ajuste custaria uma
  noite.
- **Modelo:** já aberto no Blender do Adriano, precisa de remodelagem.

## Falta

- **O storyboard.** É o que trava tudo: sem ele não dá para saber quantos
  planos modelar nem em que nível de detalhe. Peça que só aparece desfocada ao
  fundo não merece o mesmo trabalho de um close de 3 segundos no bico.
- **O diagnóstico da cena** — rodar `scripts/01_diagnostico.py` (abaixo).

## Como rodar o diagnóstico

No Blender, com o arquivo do U1 aberto:

1. Aba **Scripting** → **Novo**
2. Cola o conteúdo de `scripts/01_diagnostico.py`
3. **Executar** (Alt+P)
4. O caminho do relatório aparece no fim do Console

Gera `u1_diagnostico.txt` (legível, é esse que interessa) e
`u1_diagnostico.json` (completo) na pasta do `.blend`.

Ele responde as perguntas que decidem o anúncio:

- **O U1 está separado em peças ou é uma malha só?** Peça separada permite
  close individual e troca de cabeçote animada. Malha única, não.
- **A malha é triangulada?** Cara de STL/CAD exportado: sem aresta de apoio, o
  chanfro serrilha em close. Confirma a remodelagem.
- **A escala bate com o produto real?** Compara a extensão da cena com os
  584 × 499 × 730 mm oficiais, eixo por eixo. Escala errada estraga
  profundidade de campo, chanfro e luz em watts de uma vez — e nada disso
  aparece olhando o viewport.
- Tem UV, tem material, tem luz, quantos polígonos, versão do Blender.

## Ficha técnica do U1 (referência para a modelagem)

| | |
|---|---|
| Dimensões externas | 584 × 499 × 730 mm |
| Peso | 18,2 kg |
| Volume de impressão | 270 × 270 × 270 mm |
| Cabeçotes | 4, troca em 5 s |
| Bico | aço inox 0,4 mm, até 300 °C |
| Mesa | chapa de aço flexível com superfície PEI, até 100 °C |
| Movimento | CoreXY, 500 mm/s, 20.000 mm/s² |
| Tela | 3,5", 320 × 480, sensível ao toque |
| Estrutura | quadro de metal, painéis compostos, hastes de fibra de carbono |

Fonte: <https://www.snapmaker.com/snapmaker-u1/specs>

Materialmente, o U1 vive de **reflexo anisotrópico e chanfro** — metal
escovado, painel composto, fibra de carbono, chapa PEI. É o que a luz do
anúncio tem de servir.

## Cuidado com a folha de marca

A comunicação da Snapmaker sobre o U1 traz números de desempenho comparativos.
Nenhuma afirmação de desempenho entra no anúncio sem fonte na ficha oficial —
a mesma regra que o PhxSql aplica ao *ACID compliant* da própria folha de
marca.
