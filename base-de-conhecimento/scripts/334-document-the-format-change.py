# Document the format change
# 28/08 11:47

import pathlib
p = pathlib.Path('docs/FORMATO.md')
s = p.read_text()

# ------------------------------------------------------ cabecalho do volume
v = '''| 76 | 8 | transação (reservado) |
| 84 | 40 | reservado |'''
n = '''| 76 | 8 | `primeiro_rowid` — o primeiro rowid deste volume (só na partição por período) |
| 84 | 8 | `chave_periodo` — o período em que este volume abriu (só na partição por período) |
| 92 | 32 | reservado |'''
assert s.count(v) == 1
s = s.replace(v, n)

# ------------------------------------------------------------- paginacao
v = '''| Parâmetro | Significado |
|---|---|
| `registros_por_arquivo` | quantos registros cabem em cada volume do `.reg` |
| `max_arquivos` | quantos volumes a tabela pode ter |
| `digitos` | largura do sufixo, padrão 3 (`_001`) |
| `bytes_por_arquivo` | tamanho de cada volume dos arquivos externos |

Capacidade da tabela = `registros_por_arquivo × max_arquivos`. Passar disso
devolve erro explícito "tabela cheia", em vez do estouro silencioso de 2 GB
que o TopSpeed(R) dava.

### O endereçamento continua sendo uma conta'''
n = '''| Parâmetro | Significado |
|---|---|
| `registros_por_arquivo` | quantos registros cabem em cada volume do `.reg` |
| `max_arquivos` | quantos volumes a tabela pode ter |
| `digitos` | largura do sufixo, padrão 3 (`_001`) |
| `bytes_por_arquivo` | tamanho de cada volume dos arquivos externos |
| `modo` | **o que faz o volume cortar**: a contagem ou o calendário |

Capacidade da tabela = `registros_por_arquivo × max_arquivos`. Passar disso
devolve erro explícito "tabela cheia", em vez do estouro silencioso de 2 GB
que o TopSpeed(R) dava.

**Não existe "sem teto".** O sufixo tem largura fixa: com três dígitos o volume
1000 simplesmente não teria nome de arquivo. Teto omitido vira o maior que cabe
no sufixo — 999 com três dígitos.

### Duas regras de corte

| `modo` | quando o volume corta |
|---|---|
| `PorQuantidade` | a cada `registros_por_arquivo` linhas |
| `PorPeriodo { coluna, periodo }` | quando o período da coluna de data vira — **ou** quando o volume enche |

O período é `Mensal`, `Bimestral`, `Semestral` ou `Anual`, e os blocos sempre
começam em janeiro: bimestre é jan-fev, mar-abr, …; semestre é jan-jun e
jul-dez. Não há bimestre a começar em fevereiro.

A coluna do período tem de ser `Date` ou `DateTime` **e obrigatória** — sem
data não há período em que a linha caiba. As duas conferências acontecem na
criação do esquema, não na primeira gravação: um esquema que só quebra ao
inserir já nasceu quebrado.

### O endereçamento continua sendo uma conta'''
assert s.count(v) == 1
s = s.replace(v, n)

v = '''- **O `.ndx` não muda em nada.** Ele já guarda rowid, e nenhuma linha do código
  de índice precisa saber que existe volume.

### Arquivos externos'''
n = '''- **O `.ndx` não muda em nada.** Ele já guarda rowid, e nenhuma linha do código
  de índice precisa saber que existe volume.

### Na partição por período, o endereço sai de uma busca binária

O volume não pode sair de divisão quando o corte depende do calendário: dois
meses rendem quantidades diferentes. Então cada volume grava no **próprio
cabeçalho** o rowid em que começou (offset 76) e o período em que abriu
(offset 84), e a tabela de fronteiras é remontada lendo esses cabeçalhos na
abertura — poucos bytes por volume, uma vez.

```
volume = a última fronteira com primeiro_rowid <= rowid   (busca binária)
slot   = rowid - primeiro_rowid[volume] + 1
offset = data_offset + (slot - 1) * slot_size
```

Volume é coisa que se conta em dezenas, não em milhares — cada um guarda
`registros_por_arquivo` linhas —, então a busca binária custa três ou quatro
comparações num vetor que já está na memória.

**Sem arquivo extra e sem bloco que cresce.** A alternativa seria guardar a
tabela de fronteiras num sexto arquivo, ou dentro do bloco de esquema — e o
bloco de esquema é seguido pelos dados, então crescer significaria empurrar a
tabela inteira. O cabeçalho de cada volume já existe e tem lugar sobrando.

### A linha atrasada não volta

Esta é a regra que define o desenho. Um lançamento de **janeiro digitado em
março** entra no volume de março, não no de janeiro.

Voltar significaria escrever no meio de um arquivo já fechado, quebrando ao
mesmo tempo as duas garantias que sustentam o formato: a ordem de digitação e o
endereço contíguo. Por isso o período de um volume é **o período em que ele
abriu**, e um volume pode conter linhas de períodos anteriores que chegaram
depois.

Quem quiser todos os lançamentos de janeiro usa o índice pela data — que é
exatamente para isso que ele existe. A partição por período é uma decisão de
*como o arquivo cresce*, não de *como o dado se consulta*.

Consequência prática: um volume recém-criado e ainda vazio não tem período. O
`.reg` grava `i64::MIN` como sentinela, e a primeira linha **adota** o volume
em vez de cortar um novo — senão a tabela nasceria com um arquivo vazio.

### Arquivos externos'''
assert s.count(v) == 1
s = s.replace(v, n)
p.write_text(s)
print('FORMATO.md: cabecalho e particao por periodo')
