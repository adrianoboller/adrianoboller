# A queda no meio da janela da exclusão

Uma prova só, e ela existe porque um `#[test]` não conseguiria dá-la.

```bash
cargo build --release
python3 bancada/exclusao/prova-da-queda.py
```

Também roda pela bateria única: `python3 provar.py --so queda-na-exclusao`.

## O que ela prova

`recursos.exclusao_na_janela` (`docs/DESEMPENHO.md` §4.12) tira o `fsync` de
dentro de cada exclusão física. Isso abre um intervalo entre o `excluir`
responder OK e o `.trash` estar no disco — e a pergunta que o sprint mandou
responder **antes** de uma linha de código é o que exatamente se perde numa
queda dentro dele.

O script sobe um `phxsqld` de verdade na **porta 7100**, com a janela
configurada para **não fechar durante a corrida inteira** (`lote_operacoes` em
um milhão, `lote_milissegundos` em uma hora). Manda 150 exclusões físicas pelo
soquete, todas respondendo OK, mata o processo com **`SIGKILL`** e reabre a
base. Depois confere, **linha a linha**, em qual dos quatro estados cada uma
ficou:

| estado | o que quer dizer |
|---|---|
| só no `.reg` | a exclusão não aconteceu — nada se perde |
| só no `.trash` | aconteceu, e é reversível |
| nos dois | duplicada — o lado que a casa escolheu de propósito |
| **em nenhum** | **o caso que mata o sprint** |

E confere também o outro lado, que é o que ninguém lembra de olhar: se alguma
linha que **ninguém mandou excluir** sumiu.

Roda duas vezes — primeiro o **controle**, com o comportamento de sempre. Se o
controle não passasse, a rodada seguinte não estaria provando nada sobre o
modo novo.

## Por que `SIGKILL`, e por que não bastava um teste

O `phxsqld` trata o `SIGTERM`: ele fecha a janela, sincroniza e sai limpo — que
é o contrário do que se quer aqui. `SIGKILL` não é entregue ao programa; o
núcleo derruba o processo onde ele estiver.

E é a mesma lição do `BULKINSERT`: *teste unitário não prova queda de conexão —
soquete prova*. Quem fecha uma `Table` num teste executa `Drop`, libera
descritores e volta ao teste. Nada disso é uma queda.

## O que ela NÃO prova, e está dito

**Queda de energia.** Nenhum processo em espaço de usuário consegue provocar
uma. O caso a caso dela — inclusive o **quarto caso**, que existe e que o
sprint dizia não existir — está escrito em `docs/DESEMPENHO.md` §4.12, e foi
por causa dele que o interruptor entrou **pedido** em vez de virar padrão.

## Portas e processos

Porta **7100**, e só ela. Mata **apenas** os PIDs que ele mesmo criou, pelo
PID — nunca `pkill`. A base de trabalho é `.base-da-prova/` aqui ao lado, e ela
é apagada no fim.
