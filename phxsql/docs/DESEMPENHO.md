# Desempenho da escrita: onde dói, e o que as propostas comprariam

Documento de **medição**, não de opinião. Cada número aqui sai de um programa
que está no repositório e que qualquer pessoa roda de novo.

---

## 1. A resposta curta

> **83,5% do tempo de uma inserção está no `.ndx`.** O arquivo de dados — a
> parte que as propostas de WAL, MemTable e LSM querem substituir — já é
> *append-only* sequencial e custa 16,5%.

Isso muda o alvo. A receita clássica para acelerar escrita («tire o `fsync` do
caminho crítico») foi escrita para motores cujo gargalo é o `fsync`. O do PhxSql
não é, e há medida para isso: na bancada de 10 milhões de linhas, o processo
gastou **870 s de CPU para 884 s de relógio (98%) e leu 0,0 MiB**. Ele passou o
tempo inteiro *calculando*, não esperando disco.

---

## 2. Onde o tempo vai, fator por fator

```bash
cargo run --release --example onde-doi -- 200000
```

Mesma tabela, mesmas linhas, esquemas diferentes. A conta de cada parcela sai
da subtração:

| Esquema | linhas/s | µs por linha |
|---|---:|---:|
| só `.reg` (sem índice nenhum) | **136.338** | 7,3 |
| + 1 índice comum | 46.433 | 21,5 |
| + o mesmo índice, agora único | 32.639 | 30,6 |
| + 2 índices (a forma da bancada) | **22.516** | 44,4 |

| Parcela | µs | % |
|---|---:|---:|
| `.reg` + `.log` | 7,3 | **16,5%** |
| primeiro índice | 14,2 | 32,0% |
| conferir a chave única | 9,1 | 20,5% |
| segundo índice | 13,8 | 31,0% |
| **total** | **44,4** | 100% |

**Seis vezes.** Uma tabela sem índice insere a 136 mil linhas/s; a mesma tabela
com dois índices, a 22,5 mil. O heap não é o problema.

E piora com o tamanho, o que confirma o diagnóstico: na carga de 10 milhões, o
primeiro milhão entrou a 16.051/s e o décimo a 9.311/s — **42% mais devagar no
fim**. Taxa que cai conforme a tabela cresce, com o disco parado, é assinatura
de estrutura de índice: a B+tree reescrita nó a nó, uma linha por vez.

### Uma conta que não fecha, e vale investigar

O mesmo medidor estima, pelo `strace`, ~41 chamadas de sistema e ~20 toques de
página por linha, e mede o CRC-32 de uma página de 4 KiB em 2,36 µs. Vinte
toques dariam ~47 µs só de CRC — **mais que os 44,4 µs medidos no total**.

A conta não fecha, e isso é informação: ou nem todo toque recalcula o CRC, ou
as páginas quentes não são revisitadas tantas vezes quanto o `strace` sugere.
Está registrado aqui como **pista aberta**, não como conclusão. É o próximo
lugar a instrumentar.

---

## 3. As dez propostas, uma a uma

A avaliação abaixo é da arquitetura LSM/WAL clássica (RocksDB, InnoDB tunado)
aplicada ao PhxSql. **Ela é uma boa receita — para o problema que ela descreve.**
Cinco itens já existem aqui, dois miram um gargalo que o PhxSql não tem, um
quebraria o formato, e **dois são reais**.

| # | Proposta | Estado no PhxSql | Veredito |
|---:|---|---|---|
| 1 | WAL exclusivamente sequencial | O `.reg` **já é** *append-only*: `rowid = slots + 1`, endereço por multiplicação, nenhuma página reescrita | **Aponta para o arquivo errado.** Um WAL existe para transformar escrita aleatória de página em sequencial. Não há escrita aleatória no `.reg` — há no `.ndx` |
| 2 | MemTable em RAM | Existe `TabelaMemoria`/`SelectMemory` (87× medido), mas é cache de **leitura** | **Meia peça, do outro lado.** Como buffer de escrita ajudaria o `.ndx` |
| 3 | Single writer + fila MPSC | O servidor **já** serializa tudo numa trava global única | **Já é assim** — e o roteiro quer o contrário: trava por tabela. O gargalo de concorrência é o excesso de serialização, não a falta |
| 4 | Três modos de durabilidade | Existem, com esses três nomes: `por_operacao`, `por_lote`, `sistema` | **Já existe, e medido:** 1.289 → 18.264 → 24.858 → 26.301 linhas/s (20,4×) |
| 5 | Não atualizar índice secundário na hora | Todos os índices são mantidos dentro da inserção | **REAL, e é o maior.** Ver §4 |
| 6 | UUID v7 ou sequência, nunca v4 | `Uuid` v4/v7 (RFC 9562), `Uuid256` e `Sequence` prontos; o dossiê tem uma seção sobre por que v7 | **Já existe** |
| 7 | Não alterar o arquivo principal no INSERT | O `.reg` só anexa. Sem *double-write*, sem divisão de página no arquivo de dados | **Já é assim** |
| 8 | Segmentos imutáveis, SSTable, compactação | — | **Incompatível.** Ver §5 |
| 9 | Buffers grandes em vez de escritas pequenas | Escreve por slot; o `strace` conta 41 chamadas por linha | **Vale medir** — mas 98% de CPU e 0,0 MiB lidos dizem que o disco não é quem espera |
| 10 | Pré-alocar o WAL | Os volumes crescem conforme escrevem | **Aplicável aos volumes**, ganho provavelmente pequeno pela mesma razão do item 9 |

---

## 4. O item que vale: o índice fora do caminho crítico

É o único da lista que a medição sustenta, e o número é grande:

| Se sair do caminho crítico | µs por linha | ganho |
|---|---:|---:|
| nada (hoje) | 44,4 | — |
| o segundo índice | 30,6 | 1,45× |
| os dois índices e a conferência | 7,3 | **6,1×** |

**Mas há uma linha que não dá para cruzar, e ela é do formato.** A conferência
de unicidade acontece **antes de qualquer escrita**, e não depois — porque o
`.reg` nunca reaproveita slot. Uma inserção recusada *depois* de gravar deixaria
um buraco permanente, e uma tabela que recebe muita chave repetida iria inchando
sem nunca crescer.

Então a proposta se divide em duas, com veredito diferente:

- **Índice NÃO único, adiado** — seguro. A chave entra numa fila e um
  trabalhador de fundo a insere. Nada depende dela para decidir se a linha
  entra. Ganho medido do segundo índice: **1,45×**.
- **Índice ÚNICO, adiado** — não. Ele é a própria decisão de aceitar ou recusar
  a linha. Adiá-lo é aceitar gravar primeiro e descobrir depois, que é
  exatamente o buraco permanente. Fica.

Há um terceiro caminho, que ninguém propôs e que a medição favorece: **manter o
índice no caminho crítico, mas em lote**. A carga em lote já provou o princípio
no nível de cima — 2.715 → 25.985 linhas/s, 9,6× — porque tudo que acontecia
*por linha* passou a acontecer uma vez. A B+tree ainda é reescrita nó a nó
dentro do lote; ordenar as chaves do lote antes de inseri-las atacaria
justamente os 83,5%.

---

## 5. Por que LSM não cabe dentro do motor atual

Segmentos imutáveis com compactação é uma boa arquitetura, e é incompatível com
quatro coisas que **já funcionam aqui** — não por gosto, por dependência:

1. **A ordem de digitação.** É a regra que define o projeto: percorrer o `.reg`
   devolve as linhas na ordem em que foram digitadas. Compactação reordena.
2. **O endereço por conta.** `offset = data_offset + (rowid−1) × slot_size`.
   Numa LSM a linha muda de arquivo quando o segmento é compactado, e o rowid
   deixa de ser endereço.
3. **A paginação por cursor e o salto por bissecção.** Os dois saem de graça
   *porque* a ordem lógica é a ordem física. Sem isso, voltam a exigir índice.
4. **A garantia da replicação.** Uma réplica chega aos mesmos rowids sem que
   ninguém os transmita, porque o rowid é `slots + 1` e nada os reordena. Numa
   LSM essa garantia não existe.

A saída correta é a que a própria proposta sugere: **dois motores**, escolhidos
por tabela. Um `PHX-LSM` para log, telemetria e IoT — onde a ordem de digitação
não é sagrada e a escrita massiva é tudo — ao lado do motor atual para o ERP.
Isso é um projeto próprio, não um ajuste.

---

## 6. Comparativo com a concorrência

Bancada de **10 milhões de linhas**, mesma máquina, mesmo trabalho
(`bancada/`). Positivo = PhxSql mais rápido:

| Fase | PhxSql | MySQL(R) | |
|---|---:|---:|---|
| inserir 10.000.000 | 884,3 s | 115,3 s | **0,13×** — o buraco |
| buscar 20.000 por chave | 5,08 s | 2,67 s | 0,53× |
| excluir | 4,85 s | 5,44 s | **1,12×** |
| atualizar | 4,44 s | 6,06 s | **1,36×** |
| varrer faixa | 3,94 s | 18,97 s | **4,82×** |

A leitura sequencial é onde o formato de slot fixo paga: quase 5× o MySQL(R). A
inserção é onde ele cobra.

### Onde o PhxSql já ganha, e por quê

| | Medido | Por quê |
|---|---|---|
| Varrer faixa | 4,8× o MySQL(R) | slot de largura fixa, sem página, sem MVCC |
| Página no meio de 800 mil linhas | 164 µs contra 246 ms (1.500×) | a ordem lógica é a física: achar é uma conta |
| Consulta em memória | 87× o disco | `SelectMemory` com mapas por coluna |
| Resposta do protocolo | 44 ms → 1,3 ms (33×) | `TCP_NODELAY`, que faltava |

### O que mudou nesta versão

| | Antes | Agora | |
|---|---:|---:|---|
| Inserção pela rede, linha a linha vs. lote | 2.715/s | 25.985/s | **9,6×** |
| Página por posição no fim de 200 mil linhas | 131 ms | 6 ms | **22×** |
| Contar as linhas visíveis | varredura inteira | dois campos do cabeçalho | O(1) |
| Replicação | não existia | 4.273 eventos/s por réplica | — |

---

## 7. O que eu faria a seguir, nesta ordem

Pela medição, e não pela moda:

1. **Ordenar as chaves do lote antes de inserir no `.ndx`.** Ataca os 83,5% sem
   mudar formato nem garantia. É onde está o dinheiro.
2. **Fechar a conta do CRC** (§2). Se o CRC realmente domina o caminho de
   página, a saída é CRC incremental por nó em vez de recalcular a página.
3. **Índice não único adiado**, com fila e trabalhador de fundo. 1,45× medido,
   e não custa correção nenhuma.
4. **Buffer de escrita maior**, para baixar as 41 chamadas de sistema por linha.
   Ganho incerto — o disco não é quem espera —, mas é barato de medir.
5. **Trava por tabela.** Não acelera uma inserção; acelera o servidor com muita
   gente. É outro eixo, e o roteiro já o previa.

O que eu **não** faria agora: WAL, MemTable de escrita e group commit. Eles
resolvem o gargalo do InnoDB, e a medição diz que ele não é o nosso.

---

## Como refazer tudo

```bash
cargo run --release --example onde-doi -- 200000       # a tabela do §2
cargo run --release --example custo-do-sync            # os modos de durabilidade
cargo run --release --example custo-da-pagina -- 800000 200
python3 bancada/medir.py 10000000                      # o comparativo do §6
python3 bancada/replicacao/montar.py /tmp/phx-replicacao
python3 bancada/replicacao/medir.py 100000
```
