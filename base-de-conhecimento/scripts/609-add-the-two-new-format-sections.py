# Add the two new format sections
# 28/08 18:03

import io, re
p='docs/FORMATO.md'
s=io.open(p,encoding='utf-8').read()

velho='''cheia, coluna obrigatória em branco — **não** gera evento: o diário registra o
que aconteceu, não o que foi tentado.

---

## 5. Paginação de tabelas grandes'''

novo='''cheia, coluna obrigatória em branco — **não** gera evento: o diário registra o
que aconteceu, não o que foi tentado.

---

## 5. `.trash` — a linha inteira, antes de sumir

Só quem tem `administrar` lê este arquivo.

### A ordem é o recurso

A linha é gravada aqui e **o arquivo é sincronizado** antes de o slot do `.reg`
ser liberado. Se a máquina cair no meio, o pior caso é a linha aparecer nos
dois lugares — o que se resolve olhando —, e **nunca em nenhum**. A ordem
inversa (liberar e depois guardar) tem uma janela em que o registro não existe
em lugar nenhum, e essa janela não tem conserto depois.

Entre perder e duplicar, o motor duplica.

### Por que não é um `.reg` paralelo

Um `.reg` guarda *payload* de largura fixa, e as colunas `Bin`/`Memo` moram
nele como **ponteiro** para o `.bin`/`.memo`. Copiar só o *payload* para um
`.reg` paralelo guardaria os ponteiros — que apontam para blocos que a própria
exclusão acabou de liberar, e que a próxima inserção pode reaproveitar. A foto
voltaria sendo a foto de outra linha.

Por isso o registro daqui é de **tamanho variável**: o *payload* byte a byte,
mais o **conteúdo** de cada coluna externa logo em seguida.

### Cabeçalho do arquivo (64 bytes)

Mesmo desenho do `.log`: assinatura, versão, volume, quantidade, fim e CRC-32.

### Registro (56 bytes de cabeçalho + payload + externos)

| Offset | Bytes | Campo |
|---:|---:|---|
| 0 | 8 | carimbo em milissegundos desde 1970-01-01T00:00:00Z |
| 8 | 1 | *flags* (reservado) |
| 9 | 1 | quantas colunas externas a linha tem |
| 10 | 2 | *reservado* |
| 12 | 8 | rowid que a linha tinha |
| 20 | 4 | usuário que excluiu (0 = não informado) |
| 24 | 4 | tamanho do *payload* |
| 28 | 16 | UUID **v7** deste descarte |
| 44 | 4 | tamanho total do registro |
| 48 | 4 | *reservado* |
| 52 | 4 | CRC-32 de tudo, menos estes 4 bytes |
| 56 | n | o *payload* do slot, byte a byte como estava no `.reg` |
| … | … | por externo: `(coluna u16)(tamanho u32)(bytes)` |

O **tamanho total** está no cabeçalho de propósito: quem percorre o arquivo
avança por ele sem somar os externos um a um, e um registro que se declara
maior que o volume é recusado em vez de arrastar a leitura para dentro do
registro seguinte.

O CRC cobre o *payload* e os anexos, e não só o cabeçalho: o `.trash` só vale
como prova de que a linha era assim se adulterar o conteúdo for detectado.

O rowid guardado é **memória de onde a linha estava**, não promessa de para
onde ela volta: o `.reg` não reaproveita slot, nem por restauração.

### Esvaziar

`esvaziar_lixeira` apaga os volumes e recomeça do volume 1. Daqui não volta —
e por isso o expurgo é registrado no `.reason` **antes** de o dado sair, e a
operação exige motivo escrito mesmo numa tabela que não exige motivo para
excluir.

---

## 6. `.reason` — por que cada linha foi excluída

Só quem tem `administrar` lê este arquivo.

O `.log` já diz que houve uma exclusão no rowid tal, no instante tal. O que ele
não diz — e não tem onde dizer, porque o evento dele tem 36 bytes fixos — é
**por quê**. Este arquivo guarda a frase, a identidade do registro e o usuário,
e **sobrevive ao registro**: a linha pode sumir do `.reg` e do `.trash`, e o
motivo continua aqui.

### Cabeçalho do arquivo (64 bytes)

Mesmo desenho do `.log`.

### Registro (48 bytes de cabeçalho + dois textos)

| Offset | Bytes | Campo |
|---:|---:|---|
| 0 | 8 | carimbo em milissegundos |
| 8 | 1 | tipo: `1` suave, `2` física, `3` restauração, `4` expurgo |
| 9 | 1 | *flags* (reservado) |
| 10 | 2 | tamanho do texto do motivo |
| 12 | 8 | rowid |
| 20 | 4 | usuário (0 = não informado) |
| 24 | 16 | UUID **v7** deste evento |
| 40 | 2 | tamanho do texto da identidade |
| 42 | 2 | *reservado* |
| 44 | 4 | CRC-32 do cabeçalho (menos estes 4 bytes) e dos dois textos |
| 48 | n | motivo, UTF-8 |
| … | m | identidade, UTF-8 |

O **UUID é v7 do próprio evento**: ele identifica *esta* exclusão, e como o v7
leva o relógio nos primeiros 48 bits, ordenar por ele é ordenar por quando
aconteceu.

A **identidade** é o valor que identifica a linha na tabela — a chave primária,
senão a primeira coluna `Uuid` ou `Sequence` —, já em texto. Está aqui porque
quem lê o motivo seis meses depois não tem mais o esquema daquela linha na
cabeça, e "rowid 4173" não diz nada.

O CRC cobre os dois textos. Se cobrisse só o cabeçalho, trocar *fraude* por
*engano* passaria sem ser notado — e o arquivo existe justamente para isso não
poder acontecer.

Tetos: **2000 bytes** de motivo e **512** de identidade, cortados no limite de
caractere para nunca gravar UTF-8 inválido.

### O motivo obrigatório

É uma escolha da tabela, gravada no esquema (v4) e feita na criação. Marcada,
o motor **recusa** qualquer exclusão sem uma frase escrita — antes de qualquer
gravação. Vale para tabela cujo apagamento alguém vai ter de justificar
depois; numa tabela de rascunho, obrigar só ensina todo mundo a digitar um
ponto.

---

## 7. Paginação de tabelas grandes'''
assert velho in s
s=s.replace(velho,novo,1)

# renumera as secoes seguintes
for velho_n, novo_n in [("## 6. Hierarquia","## 8. Hierarquia"),
                        ("## 7. Reindex","## 9. Reindex"),
                        ("## 8. Identificadores","## 10. Identificadores"),
                        ("## 9. Limites","## 11. Limites"),
                        ("## 10. O que este formato ainda não faz","## 12. O que este formato ainda não faz")]:
    assert velho_n in s, velho_n
    s=s.replace(velho_n, novo_n, 1)
s=s.replace("Ver a seção 5.","Ver a seção 7.",1)
io.open(p,'w',encoding='utf-8').write(s)
