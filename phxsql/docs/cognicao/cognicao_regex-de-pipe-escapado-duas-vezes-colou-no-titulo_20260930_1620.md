# Pipe escapado duas vezes numa regex: três fechamentos colaram no título

**Estado:** INFRUTÍFERO
**Causa:** para achar a linha do pedido no `PENDENCIAS.md`, escrevi `r"^\\| [^|]+ \\| 350 \\|.*$"` num script Python dentro de um heredoc de shell. Numa string crua, `\\|` vira `\\` seguido de `|`: uma barra literal **ou** o resto do padrão. O ramo vazio casou o começo do arquivo, e o `re.sub` colou o fechamento na linha 1. Isso aconteceu três vezes (pedidos 350, 255 e 575), e os três continuaram ☐.
**Prevenção:**
- O `pagina-dos-pedidos.py` recusa, com o motivo, estado de pedido grudado fora do começo da linha, e também título que não esteja na linha 1 (`ESTADO_GRUDADO`). A guarda foi provada contra o arquivo quebrado do commit `163cdeff`, que ela recusa, e contra o consertado, que ela aceita.
- Nos scripts de edição, o padrão sai de uma string crua de **uma** barra (`r"^\| "`). Depois de escrever, confere-se o estado relido da própria linha. Não se confia no fato de o script não ter dado erro.

## O que aconteceu

Rodada de 30/09/2026, fechando o 350, o 255 e o 575. Cada fechamento rodou um script `python3 - <<'EOF'` que trocava o estado e colava a nota no fim da linha do pedido. Os três scripts saíram sem erro, e os portões ficaram verdes.

## O que eu concluí primeiro, e estava errado

Que o pedido estava fechado porque o script terminou sem exceção. Relatei ao dono «350 ☑️ … falta 16,0%». Quem desmentiu foi o próprio número: fechar o 575 não mexeu na porcentagem, e só então li o estado da linha.

## O que a medição disse

| | antes do conserto | depois |
|---|---|---|
| linha 1 do `PENDENCIAS.md` | `\| ☑️ \|\| ☑️ \|\| ☑️ \|# Tudo que…` e as três notas | `# Tudo que foi pedido…` |
| 255, 350, 575 | ☐ | ☑️ |
| falta da versão | 16,2% | 15,6% |

## A regra

Script de edição que **não confere o que escreveu** é o mesmo defeito do gerador que imprime êxito tendo feito menos, só que do lado de quem edita. E o leitor tinha a obrigação de recusar: estado de pedido fora do começo da linha nunca é legítimo.
