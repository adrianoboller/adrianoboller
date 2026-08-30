# A bancada contra o SQLite(R)

A pergunta que a abriu foi do dono: *«como o PhxSql mobile pode ser melhor que
o SQLite(R) e o HFSQL(R) no celular?»*. A resposta em prosa está em
`docs/MOBILE.md`; **os números estão aqui**, e são refazíveis.

```bash
cargo build --release
cargo build --release --examples -p phxsql-store   # a regra do binário velho
python3 bancada/sqlite/medir.py                    # ~12 min, portas 7450-7455
python3 bancada/sqlite/medir.py 200000 --rodadas 5 --partes ad   # só A e D
```

O SQLite(R) vem na biblioteca padrão do Python — não há o que instalar, e o
módulo `sqlite3` é extensão em C, não Python interpretado.

| Arquivo | O que é |
|---|---|
| `medir.py` | as cinco bancadas |
| `resultados.json` | a última medição completa, crua, com a carga da máquina junto |

## As cinco bancadas, e por que são cinco

Este par exige mais separação que o do MySQL(R), porque as duas coisas
comparadas **não têm a mesma forma**: o SQLite(R) é biblioteca em processo e o
PhxSql de hoje é servidor por soquete. Medir os dois juntos e chamar o
resultado de «o motor» seria comparar chamada de função com ida e volta de
rede — o mesmo erro de forma que já mentiu 41× nesta casa, com outra roupa.

| | O que mede | Por que existe |
|---|---|---|
| **A** | `carga` (biblioteca) × `sqlite3` (biblioteca) | é a comparação limpa dos dois motores, e a que vale para o celular — no aparelho o PhxSql seria biblioteca embutida, não daemon |
| **B** | as mesmas fases pelo soquete | quanto custa a forma de hoje |
| **C** | durabilidade casada nos três regimes | o SQLite(R) sincroniza por transação; o PhxSql tem `por_operacao`, `por_lote` e `sistema`. Agrupar de um lado e não do outro faz o número mentir |
| **D** | o piso do transporte, decomposto em três | «quanto disso é do soquete» só se responde medindo o soquete sozinho |
| **E** | o custo de uma chamada a mais | nasceu de um número que não fechava — ver abaixo |

## As quatro regras da casa, aplicadas a este par

1. **Mesmos dados.** O `linha(i)` deste script é a tradução literal do
   `linha(i)` de `crates/phxsql-store/examples/carga.rs`. Sem sorteio.
2. **Mesmo esquema.** Cinco colunas, uma busca por `id` e uma por `cidade`.
3. **Mesma forma de pergunta.** Uma instrução por operação dos dois lados.
4. **Mesma quantidade de trabalho.** A varredura lê a faixa inteira e soma o
   valor dos dois lados — e a prova é a **soma**: os dois devolvem
   `2.502.600.000` centavos em 25.000 linhas, por dois códigos sem uma linha
   em comum. Divergiu, a bancada **reprova em vez de publicar**.

## As sete armadilhas que esta bancada pagou

Ficam escritas porque cada uma virou uma linha de código, e porque a próxima
bancada desta casa vai encontrar as mesmas.

**1. O esquema do SQLite(R) não tem tradução única.** «Chave primária em `id`»
vira `id INTEGER PRIMARY KEY` (o `id` *é* o rowid: duas estruturas) ou
`id INTEGER NOT NULL` mais `UNIQUE INDEX` (três). A primeira favorece o
SQLite(R), a segunda o penaliza, e nenhuma é «a certa» — então rodam **as
duas**, e o leitor vê o tamanho da escolha em vez de herdá-la.

**2. O relógio de fora media o `fork`.** As fases do PhxSql rodam em processo
separado, e subir um processo custa ~6 ms nesta máquina — contra 4 ms que a
fase de 20.000 buscas leva inteira. Medida por fora, ela apareceria como 10 ms,
e o outro lado, que é chamada de função, não pagaria nada disso. Hoje o tempo
sai da linha `RESULTADO` que o próprio `carga` cronometra.

**3. Exclusão suave contra `DELETE` seria bit contra remoção.** O `excluir` do
protocolo marca um bit por padrão; o `DELETE` do SQLite(R) tira a linha e
conserta os índices. A bancada pede `"fisico": true`.

**4. E o `fsync` da lixeira, do outro lado da mesma moeda.** A exclusão física
sincroniza o `.trash` **por linha**, e o SQLite(R) recebe as 20.000 dentro de
uma transação. Sem `PHX_EXCLUSAO_NA_JANELA=1` seriam 20.000 `fsync` contra um.

**5. A varredura não faz o mesmo trabalho por dentro.** O `carga` decodifica a
**linha inteira** de cada rowid da faixa; o `sum(valor)` do SQLite(R) toca uma
coluna só. Em vez de discutir, mediu-se: a fase `varrer_todas` soma algo de
cada coluna e obriga o SQLite(R) a materializar a linha toda. A diferença
entre as duas é o tamanho exato dessa vantagem dele — e ela é real, não é
defeito de bancada.

**6. A janela do modo `sistema` estava trocada.** A primeira lista mandava o
SQLite(R) para o ramo do autocommit: 20.000 transações (com o `-journal`
criado e apagado em cada uma) contra 100 janelas do PhxSql. Nenhum dos dois
sincronizava, então o `fsync` não denunciava nada — e o número saiu **1,56× a
nosso favor** por trabalho desigual. É o erro de sempre, agora na coluna da
janela.

**7. O piso do transporte medido com o GIL no meio.** O eco de referência
começou como uma *thread* no mesmo interpretador do cliente. Com a máquina
carregada ele marcou 73,78 µs contra 72,75 µs do `phxsqld`, e a subtração deu
**−1,03 µs**: um servidor que custa menos que nada. Cliente e eco no mesmo
interpretador não rodam ao mesmo tempo, então cada ida e volta pagava uma troca
de contexto que o `phxsqld`, sendo outro processo, não paga. Hoje o eco é
processo. *Medir o piso com uma restrição que o medido não tem não mede piso
nenhum.*

## O que esta bancada não consegue estabilizar, e por isso declara

**As medidas dominadas por `fsync` variam muito nesta máquina**, e não é ruído
de vizinho: é a máquina virtual. A bancada C, medida em dias diferentes com a
carga parecida, deu `por_operacao` entre **1,2× e 1,6×** e `por_lote` entre
**1,8× e 2,8×** — o mesmo código, o mesmo binário, o mesmo `n`. A conclusão
qualitativa («com durabilidade por linha a distância encolhe muito») sobrevive
às duas pontas; o número exato, não.

Isso está dito em vez de escondido atrás de uma mediana, porque a mediana de
duas rodadas de uma medida que varia 2× é um número com aparência de precisão.
Quem quiser fechar essa conta precisa de uma máquina com disco dedicado — não
de mais rodadas aqui.

A bancada A, ao contrário, é estável: as cinco rodadas ficam dentro de poucos
por cento, porque nela o trabalho é longo e nenhum `fsync` manda no relógio.
É por isso que **a tabela principal do `docs/MOBILE.md` é a A**, e não a C.

## A bancada E, e por que ela existe

A E não estava planejada. Ela nasceu de **um número que não fechava**: na C, o
`inserir_lote` em blocos de 200 saiu a 32 µs/linha, e na B, em blocos de 1.000,
a 17,8 µs/linha — a tabela **menor**, com a árvore mais rasa, saiu mais **lenta**
por linha. O piso do transporte não explicava: 35 µs por ida e volta dividido
por 200 linhas dá 0,18 µs/linha.

A explicação plausível era «tem um custo fixo por chamada». Plausível não é
medido — foi o que o mutex do Profiler ensinou. Então a E varre o tamanho do
bloco e deixa a reta dizer quanto é da **linha** e quanto é da **chamada**.

## O que esta bancada NÃO compara

**Transação.** O SQLite(R) tem; o PhxSql não. Parte do custo de escrita dele
paga uma garantia que o PhxSql ainda não oferece — e é por isso que não se
escreve *ACID* sobre o PhxSql em lugar nenhum.

**Lixeira e trilha.** A exclusão física copia a linha para o `.trash` e o
motivo para o `.reason`. O SQLite(R) não faz nada disso: é trabalho **a mais**
do nosso lado, e fica dito em vez de escondido.

**Colunas de sistema.** Todo esquema do PhxSql ganha `softdeleted` e `rownum`
de brinde, 9 bytes por linha, sem equivalente do outro lado.

**Maturidade, SQL e superfície.** Nada aqui mede o que mais separa os dois, e
`docs/MOBILE.md` §5 é o lugar onde isso está escrito por extenso.
