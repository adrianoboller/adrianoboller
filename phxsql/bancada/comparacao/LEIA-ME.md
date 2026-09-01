# Os três no mesmo trabalho

PhxSql × MySQL(R) × SQLite(R), a um milhão de linhas, na mesma máquina e na
mesma rodada. É a terceira bancada de comparação da casa, e existe por um
motivo que as outras duas não resolviam.

```bash
cargo build --release --examples -p phxsql-store   # a regra do binário velho
service mysql start
python3 bancada/comparacao/medir.py                # ~15 min
python3 bancada/comparacao/grafico.py
```

| Arquivo | O que é |
|---|---|
| `medir.py` | a bancada: os três motores, quatro fases, N rodadas |
| `um-milhao.json` | a última medição completa, crua — é dela que o gráfico sai |
| `grafico.py` | desenha o SVG e a página; **recusa** desenhar sem o JSON |
| `comparacao-tres-motores.svg` | o fragmento que entra no dossiê |
| `comparacao-tres-motores.html` | a página de pé, para olhar sozinha |
| `corrida-um-milhao.log` | o registro da corrida que produziu o JSON |

## Por que uma terceira, e não a soma das duas

`bancada/medir.py` mede PhxSql × MySQL(R). `bancada/sqlite/medir.py` mede
PhxSql × SQLite(R). Juntar as duas tabelas dá **três colunas e nenhuma
comparação**: as medidas são de dias diferentes, com cargas diferentes na
máquina, e parte da diferença passa a ser do ambiente em vez do motor. É o
mesmo erro de comparar escalas diferentes, com outra roupa.

Aqui os três correm **intercalados dentro da mesma rodada**, e a dispersão
entre rodadas aparece no gráfico como bigode, em vez de virar uma barra lisa
que afirma uma precisão que o número não tem.

## O que esta bancada achou ao ser montada

**A bancada do MySQL(R) violava a regra 1 — mesmos dados.** Ela grava
`'2024-10-04'` em **toda** linha, enquanto o `carga.rs` e a bancada do
SQLite(R) gravam `20000 + (i % 400)` — o dia variável. Dado diferente, do
mesmo tamanho, e **invisível em qualquer medida de tempo**: nenhum número
ficava estranho por causa disso.

O que o achou foi ter de conferir três motores em vez de dois. E o conserto
não foi só gravar a data certa aqui: nasceu a fase `conferir` do `carga.rs`,
que soma o que existe na tabela e obriga os três a chegarem ao **mesmo
estado** antes de qualquer número de tempo ser publicado.

## A prova de trabalho igual

Três totais, em três marcos, por três códigos sem uma linha em comum:

| | o que pega |
|---|---|
| contagem de linhas | linha que faltou ou sobrou |
| soma de `valor` | o `atualizar` que não atualizou |
| soma de `cadastro` | **dado diferente do mesmo tamanho** — o defeito acima |

Os marcos são **depois de inserir**, **depois de atualizar** e **depois de
excluir**. O do meio não é enfeite: `atualizar` e `excluir` mordem exatamente
os mesmos 20.000 alvos, então no marco final o efeito do `atualizar` já
desapareceu junto com as linhas excluídas. Sem o marco do meio, a fase
`atualizar` não teria prova nenhuma.

Divergiu, a bancada **recusa publicar** e diz qual motor e qual coluna. Isso
está provado nos dois sentidos: repor a data constante da bancada antiga faz
ela reprovar com `cadastro 400.000.000` contra `403.990.000` — e
`400.000.000` é exatamente 20.000 linhas × dia 20.000, que é a assinatura do
defeito.

## As quatro regras da casa, aplicadas a este trio

As regras estão em `bancada/LEIA-ME.md`. O que muda com três motores:

1. **Mesmos dados.** O `linha(i)` do `medir.py` é a tradução literal do
   `linha(i)` do `carga.rs`, agora **inclusive a data**.
2. **Mesmo esquema.** Chave em `id`, índice secundário em `cidade`. O
   SQLite(R) não tem tradução única para isso, então rodam as duas variantes;
   a publicada é a `rowid`, que é a que casa com o InnoDB (chave agrupada) e
   a que **favorece o SQLite(R)**. A outra fica no JSON, em `sqlite_2ind`.
3. **Mesma forma de pergunta.** Uma instrução por operação nas fases
   pontuais, nos três. A carga inicial é a exceção, e ela está nas ressalvas
   **com o nome de quem favorece** — o MySQL(R), que recebe a forma mais
   barata das três.
4. **Mesma quantidade de trabalho.** É o que a fase `conferir` prova.

## A assimetria que não dá para tirar, e por isso é medida

Os três não têm a mesma forma. O SQLite(R) é biblioteca no processo, o PhxSql
aqui também (o `carga`), e o **MySQL(R) é daemon que recebe texto por
soquete** — não existe MySQL(R) embutido nesta máquina.

Então a barra dele carrega transporte e análise de texto que as outras duas
não pagam. Isso não se conserta; o que se faz é **medir**: 20.000 instruções
que não fazem trabalho nenhum (`DO 1;`) pelo mesmo caminho. O número vai para
as ressalvas do JSON e para a página, e o leitor subtrai.

Esconder isso seria publicar uma vitória que é do formato, e essa é a família
de erro que esta casa já cometeu três vezes — duas a favor do outro motor e
uma a favor do nosso.

## Durabilidade

Uma sincronização no fim de cada fase, nos três. Do lado do PhxSql a exclusão
entra na janela (`PHX_EXCLUSAO_NA_JANELA=1`), porque do outro lado as 20.000
exclusões vão dentro de uma transação — um `fsync` para as vinte mil. Sem
isso o PhxSql pagaria vinte mil, e o número mentiria **contra nós**.

Não é o regime de quem grava pedido a pedido. Uma bancada com `commit` por
linha daria outros números, e é a que importa para esse caso.
