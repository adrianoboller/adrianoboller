# Document the alphanumeric partition
# 28/08 19:06

import io
p='docs/FORMATO.md'
s=io.open(p,encoding='utf-8').read()
velho='''### Duas regras de corte

| `modo` | quando o volume corta |
|---|---|
| `PorQuantidade` | a cada `registros_por_arquivo` linhas |
| `PorPeriodo { coluna, periodo }` | quando o período da coluna de data vira — **ou** quando o volume enche |
'''
novo='''### Três regras de corte

| `modo` | quando o volume corta | sufixo |
|---|---|---|
| `PorQuantidade` | a cada `registros_por_arquivo` linhas | `_001` |
| `PorPeriodo { coluna, periodo }` | quando o período da coluna de data vira — **ou** quando o volume enche | `_001` |
| `PorLetra { coluna }` | **não corta**: são 37 volumes fixos, e a linha vai para o da letra dela | `_A`, `_0`, `_Outros` |
'''
assert velho in s
s=s.replace(velho,novo,1)

velho2='''### Na partição por período, o endereço sai de uma busca binária'''
novo2='''### A partição alfanumérica

```
cadastroClientes_A.reg   cadastroClientes_0.reg
cadastroClientes_B.reg   cadastroClientes_1.reg      cadastroClientes_Outros.reg
…                        …
cadastroClientes_Z.reg   cadastroClientes_9.reg
```

São **37 volumes**, sempre os mesmos, nesta ordem: `A`..`Z` (1..26), `0`..`9`
(27..36) e `Outros` (37). A ordem é o formato — mudar a lista mudaria o
endereço de toda linha já gravada.

O volume sai da **primeira letra** de uma coluna de referência, e o valor dela
vira texto pela mesma função que o `.reason` usa — então número também
particiona, e `12345` cai no `_1`. Três decisões:

- **Acento cai na letra sem acento.** «Ávila» vai para o `_A`. Um balde `_Á`
  separado faria «Avila» e «Ávila» — a mesma pessoa digitada por duas pessoas —
  pararem em arquivos diferentes. A tabela de dobra é escrita à mão e cobre o
  português, o espanhol e o alemão; o que não cobrir cai em `Outros`, que é um
  lugar visível e não um erro escondido.
- **Vazio vai para `Outros`,** e não para `A`. Nome em branco não começa com A;
  juntá-lo com os Andrades esconderia o problema no maior balde.
- **Maiúscula e minúscula são o mesmo balde.** O contrário faria a mesma
  consulta achar ou não achar conforme como foi digitada.

A coluna de referência tem de ser **obrigatória** e **não externa**: o valor de
um `Bin`/`Memo` mora fora do slot, e o balde precisa ser decidido *antes* de a
linha ser gravada — ler o `.memo` para saber em que arquivo gravar seria a
ordem invertida.

#### O endereço continua sendo a mesma conta

O rowid é **atribuído** assim:

```
rowid = (balde - 1) × registros_por_arquivo + slot_no_balde
```

que é a inversa exata da conta de `localizar`. Por isso **nenhum caminho de
leitura mudou**: `localizar` continua devolvendo (volume, offset) por divisão,
o `.ndx` continua guardando rowid sem saber que balde existe, e o espelho
`.bkp` também não muda.

Cada volume guarda no próprio cabeçalho (bytes 100..108) quantos slots já usou.
Fica no volume, e não num arquivo separado, pela mesma razão da fronteira do
período: um arquivo separado seria uma segunda verdade.

O `slot_count` do volume 1 deixa de ser "quantos slots" e passa a ser a **marca
d'água** — o maior rowid que já existiu. Entre o fim do `_A` e o começo do `_B`
há `registros_por_arquivo` menos os usados de puro vazio, então a varredura anda
**por balde**: dentro do balde vai até `usados`, e no fim salta direto para o
início do próximo.

#### A ordem de digitação muda de campo

O que se perde é o rowid ser crescente na ordem de chegada: com os baldes, o
rowid diz em que **arquivo** a linha está, e não quando ela chegou. Dentro de
cada volume a ordem continua sendo a de digitação, e slot excluído continua sem
ser reaproveitado.

A ordem global fica na coluna de sistema `rownum`. **Sem ela este modo seria uma
quebra da regra da casa; com ela, é uma troca de campo.** A leitura sai em ordem
alfabética de balde — que é a ordem do arquivo.

#### O teto passa a ser por letra

`registros_por_arquivo` é o teto **de cada balde**, e não da tabela. Num
cadastro brasileiro o `_S` costuma ter dez vezes o `_K`: quem enche primeiro
derruba a inserção daquela letra com as outras 36 ainda com espaço, e o erro
diz **qual** balde encheu — «tabela cheia» com 3% de ocupação seria uma
mensagem que não ajuda ninguém.

#### O que é recusado

**Alterar a coluna de referência.** Mudar «Silva» para «Andrade» mudaria o
arquivo em que a linha mora, e com ele o rowid — que é a identidade dela em
todo índice. Mover não é opção; deixar a linha no balde errado também não,
porque aí o `_S` deixa de conter os S. Então a alteração é recusada, com o
caminho escrito na mensagem: exclua e insira de novo, e a linha nova nasce no
balde certo com outro rowid.

#### Só o `.reg` leva a letra

O `.bin`, o `.memo`, o `.log`, o `.trash` e o `.reason` rolam por **tamanho**, e
continuam com o sufixo numérico: um `Clientes_B.log` se leria como «o diário do
balde B», e o diário é da tabela inteira.

### Na partição por período, o endereço sai de uma busca binária'''
assert velho2 in s
s=s.replace(velho2,novo2,1)
io.open(p,'w',encoding='utf-8').write(s)
