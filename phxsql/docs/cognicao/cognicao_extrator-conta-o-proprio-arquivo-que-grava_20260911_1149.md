# Extrator conta o próprio arquivo que ele acabou de gravar

## O que aconteceu

Pedido 156: amarrar `docs/tecnologias/extrair.py` ao
`docs/dossie/portao-dos-geradores.py`, fazendo-o **gravar** no lugar (em vez de
só imprimir) e entrando no `PLANO` do portão em modo `exato`.

Depois da primeira `--gravar` (que trocou seções inteiras de prosa digitada
por blocos compactos regenerados, encolhendo `docs/TECNOLOGIAS.md`), rodei o
portão para confirmar VERDE — e ele acusou **VERMELHO**, citando
`docs/TECNOLOGIAS.md` com uma única linha divergindo:

```
-| Markdown (documentacao tecnica) | `docs/` (...) | 284 | 71798 |
+| Markdown (documentacao tecnica) | `docs/` (...) | 284 | 71650 |
```

Rodar `--gravar` **uma segunda vez** (sem tocar em mais nada) resolveu: o
portão voltou a VERDE e continuou VERDE nas rodadas seguintes, inclusive na
prova do defeito reposto (editar um número à mão e rodar de novo).

## O que eu concluí primeiro, e estava errado

Ao ver o VERMELHO logo após implementar `--gravar`, a primeira hipótese foi
que eu tinha errado a fronteira de alguma das 16 regiões `<!-- GERADO -->`
(um fechador mal posicionado engolindo ou cortando conteúdo, fazendo o texto
regravado divergir de si mesmo) — ou um bug no `PADRAO_REGIAO.sub()`. Cheguei
a reler as 16 fronteiras contra o diff antes de perceber que o problema não
estava em nenhuma região específica: estava em uma única célula de tabela,
sempre a mesma, e o valor sempre se aproximava do estável a cada rodada extra
— sinal de convergência, não de fronteira errada.

## O que a medição disse

`bloco_outras_linguagens()` mede `contar_arquivos(["*.md"], RAIZ / "docs")`
— **recursivo**, sem excluir nada — e isso inclui o próprio
`docs/TECNOLOGIAS.md`, que é exatamente o arquivo que `gravar()` está
reescrevendo. A sequência exata:

1. `TECNOLOGIAS.md` tinha um tamanho grande (seções com prosa digitada à mão
   ao lado do bloco colado, ver `cognicao` desta mesma rodada nos passos 1–2
   do pedido). O gerador leu esse tamanho, calculou **71.798** linhas totais
   de Markdown em `docs/`, e gravou o arquivo — que agora ficou **menor**
   (507 linhas, contra o que tinha antes), porque a prosa saiu.
2. Rodar o portão chama o gerador de novo, agora lendo o `TECNOLOGIAS.md` já
   **encolhido** — o total de `docs/` mede **71.650** (148 linhas a menos,
   batendo com a diferença de tamanho do próprio arquivo). Esse número
   diverge do que está publicado (71.798) → VELHO.
3. Rodar `--gravar` de verdade uma segunda vez escreve 71.650 no arquivo. O
   arquivo continua com o mesmo número de linhas (507 → 507, porque trocar o
   texto de uma célula não muda a contagem de linhas do arquivo) — então uma
   terceira leitura mede o mesmo total, **converge**, e o portão fica VERDE.

Ou seja: **uma** rodada extra bastou porque a mudança de tamanho foi
**estrutural e única** (prosa removida, formato compactado); depois que a
estrutura para de mudar, o número da célula para de realimentar o próprio
tamanho do arquivo.

## A regra

Depois de qualquer mudança que altere a contagem de LINHAS de uma região
`<!-- GERADO -->` do `TECNOLOGIAS.md` (não só o valor dentro de uma célula),
rode `python3 docs/tecnologias/extrair.py` **duas vezes seguidas** antes de
confiar no portão — a primeira rodada nunca converge quando o próprio
tamanho do arquivo mudou, porque `bloco_outras_linguagens()` conta
`docs/TECNOLOGIAS.md` dentro do total de Markdown de `docs/`, e esse total
só se estabiliza depois que o arquivo para de mudar de tamanho.

## Como está guardado hoje

Só neste arquivo. Nem o `extrair.py` nem o `portao-dos-geradores.py` sabem
disso — o portão compara só ANTES/DEPOIS de uma única chamada, então uma
mudança estrutural sempre aparenta VERMELHO na primeira conferência seguinte,
mesmo com o gerador funcionando certo. Não é bug do portão nem do extrator:
é a consequência matemática de um gerador contar o próprio arquivo-alvo. O
buraco fica registrado aqui para a próxima vez que uma seção do
`TECNOLOGIAS.md` crescer ou encolher de novo (nova região, parágrafo
removido, etc.): o VERMELHO da primeira rodada não é motivo de alarme —
rode `--gravar` de novo antes de investigar fronteira ou regex.

Achado a parte, fora do escopo desta rodada, registrado mas não corrigido
aqui: o texto que `bloco_outras_linguagens()` imprime («não recursivo em
`dossie/`, `design/`, `video/`») não bate com a implementação —
`contar_arquivos(["*.md"], RAIZ / "docs")` não recebe `excluir` nenhum, e o
`rglob` É recursivo. Medido: há 4 arquivos `.md` reais dentro dessas três
pastas (`docs/dossie/LEIA-ME.md`, `docs/dossie/figuras/LEIA-ME.md`,
`docs/design/LEIA-ME.md`, `docs/video/LEIA-ME.md`), todos contados apesar do
texto dizer que não seriam. Fica como sugestão de tarefa separada.
