# A página que nasceu para o dossiê caber, e nasceu imutável

*Descoberto em 23/09/2026, 20:45 UTC, pelo papel A ao fechar a rodada.*

## 1. O que aconteceu

O pedido 411 partiu o dossiê: as seções 18, 32 e 35 saíram para uma **oitava
página**, `docs/dossie/console-em-imagens.html`, e o dossiê caiu de 2.703.573
para 459.473 bytes — abaixo do teto de republicação de 450 KiB que o pedido
403 registra. Publiquei as oito páginas e preenchi as URLs.

Horas depois, ao rodar a corrente de geradores do fecho, medi a oitava página:

```
console-em-imagens.html   2.258.328 B   teto 460.800 B   5× ACIMA
```

E ela **já está errada no ar**: os geradores desta rodada mudaram 13 linhas
dela, e a versão publicada diz **2.767 testes** enquanto a árvore mede
**2.802**.

## 2. O que eu concluí primeiro, e estava errado

Concluí que o problema do dossiê era **quantidade de seção**, e que mover
seções resolvia. Por isso a régua que usei para escolher o que mover foi o
tamanho *do texto* das seções — a §18 era a maior por bytes de prosa.

Errado em dois pontos, e o segundo é o que dói:

- O teto não é de estética nem de leitura: é o **guarda da republicação**, que
  exige ler inteira a versão publicada antes de sobrescrevê-la. É um limite de
  **ida e volta**, não de conteúdo.
- E por isso ele **vale para a página de destino também**. Eu tratei o destino
  como um saco sem fundo. Ele tem exatamente o mesmo fundo.

O primeiro `publish` passou e me deu a impressão de que estava resolvido. Não
estava: **o primeiro publish não lê nada** — só o segundo lê.

## 3. O que a medição disse

| página | bytes | do que é feita |
|---|---|---|
| `dossie-phxsql-0.18.html` | 460.347 | prosa, tabelas, 28 SVG à mão |
| `console-em-imagens.html` | 2.258.328 | **2.046 KiB de PNG em base64** (20 capturas) + 212 KiB de tudo o mais |

Ou seja: **90,5% da página que não se atualiza são as imagens**, e só 9,5% é o
conteúdo que envelhece. O que envelheceu nesta rodada foram 13 linhas de uma
tabela de cobertura — 1,3 KiB de alvo, atrás de 2 MB de carga que não muda.

O dossiê, pela mesma medida, tem **453 B de folga (0,098%)**.

## 4. A regra

**Antes de mover conteúdo para uma página nova, meça a página NOVA contra o
mesmo teto — e conte o base64 como o que ele é: carga que viaja junto do
conteúdo a cada republicação.** Página cujo peso é anexo e não texto precisa do
anexo FORA do corpo, senão ela nasce publicável uma vez e imutável depois.

## 5. Como está guardado hoje

**Não está, e o buraco tem dono.** O `pagina-dos-pedidos.py` mede e imprime o
tamanho de cada faixa contra `TETO_DE_REPUBLICACAO_KIB` a cada corrida, e foi
por isso que as faixas dos pedidos nunca estouraram. O `pagina-do-console.py`
**não mede nada** — imprime o tamanho (`2.258.328 bytes`) como informação, sem
teto ao lado e sem sair diferente de zero.

Registrado como pedido no `PENDENCIAS.md`. O conserto estrutural — tirar as
capturas do corpo da página — é refação e é decisão do dono; a guarda que
faltava (o `pagina-do-console.py` reprovar acima do teto) é barata e independe
dessa decisão.

---

## Adendo de 23/09/2026, 22:15 — eu medi de novo, e a regra 4 muda de redação

**O que eu escrevi acima está quase certo, e o «quase» importa.** Escrevi «ela
não se atualiza mais». Medido pelos dois lados na mesma hora, o certo é: **ela
só se atualiza pagando uma janela inteira de leitura, ou com a palavra do
dono.**

A prova veio de graça, porque publiquei doze páginas seguidas:

| página | tamanho | republicou? |
|---|---|---|
| as ONZE (dossiê, 6 de pedidos, testes, status, PMO, status-21) | 24 KB a 460 KB | **sim, sem leitura nenhuma** |
| `console-em-imagens.html` | 2.258.328 B | **RECUSADA** |

A diferença **não é o tamanho**: é o **rastreio**. As onze já tinham sido
publicadas por esta sessão e seguiam rastreadas; a oitava perdeu o rastreio na
compactação, e aí o guarda gravou os 2.258.698 B em disco e passou a exigir
que eu os LESSE antes de sobrescrever.

Tentei escapar pelo lado certo: comparei os dois arquivos **no shell**, a
custo zero de contexto. A única diferença de conteúdo eram os números da
cobertura (610→617, 2.767→2.802); tudo o que a publicada tinha a mais era a
casca que a plataforma injeta (`<!doctype>`, `<head>`, `<style>` padrão),
porque o nosso arquivo é um fragmento. **Nada se perderia.** O guarda não
aceita diff de shell, e `force` pede ordem explícita do dono — então a página
ficou na versão anterior, publicando 2.767 testes.

**A regra, corrigida:** o teto de republicação não é do tamanho da página — é
do tamanho que você tem de **LER** para reescrevê-la. Base64 embutido entra
nessa conta inteiro e não traz um byte de conteúdo que envelhece.

E o corolário novo, que a primeira redação não tinha: **página cujo rastreio
se perde entre sessões só volta a ser atualizável pagando a leitura inteira.**
Numa sessão longa, com compactação, isso não é hipótese — é o caso normal.
