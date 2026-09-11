# Bancada do Registro do Windows — latência de I/O por ponto

Responde, com número, à pergunta do dono: **a velocidade de I/O de leitura e
escrita do Registro do Windows contra o PhxSql, o SQLite, o MySQL e o
MariaDB.** Aqui está o *como medir*; os quatro bancos já estão medidos em
`bancada/comparacao/` (um milhão de linhas, 08/09/2026).

## O que é, e o que ela mede

O Registro (as *hives* por trás do `regedit` — `SYSTEM`, `SOFTWARE`,
`NTUSER.DAT`…) é a base de configuração do Windows: um armazém **chave→valor
hierárquico, mapeado em memória, com descarga preguiçosa**. Não é motor
relacional — sem SQL, sem índice por coluna, sem junção, sem varredura por
faixa, sem tabela de milhões de linhas. O único trabalho que ele faz **igual**
a um banco é **ler um valor por chave** e **gravar um valor por chave**, com o
dado pequeno. É só nesse chão que a comparação é honesta — e é o que
`bench.c` mede.

Três latências, no mesmo valor pequeno (`us/op`):

| fase | o que faz | por que importa |
|---|---|---|
| **escrita preguiçosa** | `RegSetValueEx` e volta | é o padrão do Registro: durabilidade **adiada**, a colmeia só vai ao disco na descarga preguiçosa do CM (a cada segundos) ou no `RegFlushKey` |
| **escrita durável** | `RegSetValueEx` + `RegFlushKey` **a cada chave** | reescreve a colmeia inteira; é o **único** jeito de comparar de igual para igual com um banco que dá `fsync` por commit |
| **leitura** | `RegQueryValueEx` | colmeia quente, em RAM |

## A regra que a bancada existe para não deixar quebrar

**Bancada compara trabalho igual, não só pergunta igual** (a lei está em
`../LEIA-ME.md`). Aqui isso tem duas faces:

1. **Mesmo trabalho.** Ponto por chave, valor pequeno, dos dois lados. O
   Registro não faz o resto; medir o resto seria comparar coisas diferentes.
2. **Mesmo regime de durabilidade.** É o erro fácil desta comparação: a
   escrita preguiçosa do Registro (~µs, adia o disco) *parece* ganhar de um
   `fsync` por commit — mas não estão fazendo a mesma coisa. Para ser justo,
   ou se compara **Registro preguiçoso × PhxSql `por_lote`** (os dois adiam),
   ou **Registro durável × banco com `fsync` por operação** (os dois pagam o
   disco na hora). Cruzar os regimes publica um número que mente a favor de
   quem adia.

## Como rodar — num Windows DE VERDADE

**Não rode sob `wine`.** O `wine` reimplementa o Registro; o número seria dele,
não do Configuration Manager do kernel. «O que depende do sistema operacional
se prova contra o sistema operacional.»

1. **Compile** (cruzado, daqui no Linux): `./compilar.sh` gera
   `bench-registro.exe` e confere que ele importa só DLLs do Windows
   (`ADVAPI32`, `KERNEL32`, `msvcrt`) — zero runtime do mingw. Ou compile no
   próprio Windows com qualquer compilador C, ligando `advapi32`.
2. **Leve o `.exe`** para a máquina Windows e rode no `cmd`/PowerShell:

   ```
   bench-registro.exe            :: 20000 operações, valor de 64 bytes
   bench-registro.exe 50000 128  :: N e tamanho do valor à escolha
   ```

   Ele cria tudo sob `HKCU\Software\PhxSqlBench` (por usuário, **não** precisa
   de administrador) e **apaga a subárvore inteira no fim** — não deixa lixo na
   colmeia, do mesmo modo que a bateria da casa não deixa diretório solto em
   `/tmp`.
3. **Rode 3 vezes** e fique com a mediana e a faixa min–máx, como as outras
   bancadas — a máquina carregada no meio de uma corrida é ruído, não custo.

## A outra metade: os bytes que vão ao disco

`bench.c` mede o **relógio por operação**. Os **bytes que de fato vão ao
disco** um programa de espaço de usuário não enxerga sozinho — e são metade da
resposta de «I/O». Meça-os **ao lado**, na mesma corrida:

- **Process Monitor** (Sysinternals): filtre por `Operation is RegSetValue` e
  por `WriteFile` no arquivo da colmeia (`...\NTUSER.DAT` e o `.LOG`). A coluna
  *Detail* traz o tamanho de cada escrita; a diferença entre a fase preguiçosa
  e a durável aparece aqui em bytes, não só em microssegundos.
- **ETW** (`wpr -start`/`-stop`, ou `xperf`): os contadores de disco por
  processo dão o mesmo, roteirizável. É o análogo do nosso `strace` contando
  `fsync` e toques de página **por dentro**, em vez de citar «~20».

## Prova real, nos dois sentidos

O `bench.c` grava um padrão que depende do índice da chave e **lê de volta
conferindo cada uma**. Se um valor não bater — trocado, truncado, sumido —, ele
imprime `verificacao: FALHOU` e sai com código ≠ 0: não publica número de uma
corrida que não provou o que gravou. É o mesmo crivo do `.fts` e da
`bancada/comparacao`, que recusam publicar quando o lido não bate com o
escrito.

## Onde o número entra depois de medido

Rodou no Windows? Guarde a saída (as três linhas `*_us_op` + `verificacao`,
com **data e nome da máquina**, que o próprio `.exe` imprime) num
`resultados.json` aqui, e aí ele deixa de ser estimativa: entra ao lado das
medianas dos quatro bancos de `bancada/comparacao/um-milhao.json` para desenhar
o comparativo — com o Registro finalmente **medido**, e não raciocinado. Até
lá, a estimativa (e o porquê de ser estimativa) é a que foi entregue ao dono
como página; **número citado é número que não se mede**, e este ainda não foi.

## Arquivos

- `bench.c` — a micro-bancada (C, zero dependência, só `advapi32`/`kernel32`).
- `compilar.sh` — cross-compila daqui e confere as DLLs importadas.
- `bench-registro.exe` — **derivado**, gerado pelo `compilar.sh`, fora do git.
