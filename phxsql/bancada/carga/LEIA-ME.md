# Carga pela rede: uma a uma contra `inserir_lote`

```bash
cargo build --release
python3 bancada/carga/medir.py 20000
```

## Por que este script existe

O número da carga em lote (**2.715 → 25.985 linhas/s**) foi medido à mão e ficou
na documentação **sem programa que o refizesse**. Aí o motor mudou — o cache de
páginas do `.ndx` entrou na 0.17.0 — e o número virou história em vez de medida.

É a mesma armadilha do selo do dossiê que passou quatro lançamentos dizendo
0.11.0: **número digitado à mão envelhece calado**.

## O que ele compara

As duas metades fazem **o mesmo trabalho**: as mesmas 20.000 linhas, a mesma
tabela, os mesmos dois índices (um único e um de baixa cardinalidade), o mesmo
servidor. Muda só quantas viagens de rede, quantas aberturas de tabela e
quantos `fsync` a carga custa.

No fim ele **confere a contagem das duas tabelas**. Comparar o tempo de
trabalhos diferentes é o erro que esta bancada já cometeu duas vezes, e nas duas
o número não denunciava nada.

## O que saiu, na 0.17.0

| | linhas/s | |
|---|---:|---|
| uma a uma | 2.659 | uma viagem, uma abertura e um `fsync` por linha |
| lotes de 5.000 | **39.287** | **14,8×** |

O ganho **não é do disco**: é de tudo que acontecia *por linha* passar a
acontecer uma vez por lote. Na 0.16.0 o lote dava 25.985/s; o que mudou entre
uma e outra foram o cache de páginas do `.ndx` e o cabeçalho que parou de
reserializar o esquema por linha (`docs/DESEMPENHO.md` §2 e §2.0).

O lado de uma a uma é o mais instável dos dois — cada linha paga uma viagem de
rede e um `fsync` —, e é por isso que ele é o **controle** e não o resultado:
duas corridas seguidas deram 2.400 e 2.659 linhas/s, enquanto o lote deu 39.038
e 39.287.

O lado de controle — o linha a linha — bate com a medição antiga (2.715 e
2.609), que é como se sabe que os dois números são comparáveis.
