# Document rownum in the format
# 28/08 19:06

import io
p='docs/FORMATO.md'
s=io.open(p,encoding='utf-8').read()
velho='''### O bloco de esquema (`PSCH`, versão 4)

O bloco começa com `PSCH` e a versão. A **3** acrescentou os metadados de
coluna, o marcador de chave primária e o modo de partição. A **4** acrescentou
a coluna de sistema `softdeleted` e um byte no fim, com o sinal de *motivo
obrigatório*. A leitura ainda aceita a 2: tabela gravada antes abre
normalmente, ganha um `id` v7 sorteado na hora e os textos vazios.
**Escrever, só na 4.**'''
novo='''### O bloco de esquema (`PSCH`, versão 5)

O bloco começa com `PSCH` e a versão. A **3** acrescentou os metadados de
coluna, o marcador de chave primária e o modo de partição. A **4** acrescentou
a coluna de sistema `softdeleted` e um byte no fim, com o sinal de *motivo
obrigatório*. A **5** acrescentou a coluna de sistema `rownum`. A leitura ainda
aceita a 2: tabela gravada antes abre normalmente, ganha um `id` v7 sorteado na
hora e os textos vazios. **Escrever, só na 5.**'''
assert velho in s
s=s.replace(velho,novo,1)

# a secao da coluna de sistema ganha a irma
velho2='''Declarar `softdeleted` à mão é permitido — quem recria uma tabela precisa —,
mas só como `Bool` não nula. Com outro tipo, o esquema é recusado: seria uma
coluna comum com nome reservado, e o motor passaria a marcar exclusão num
campo que o usuário lê como texto. Nulo também é recusado: seria um terceiro
estado entre excluída e não excluída.'''
novo2='''Declarar `softdeleted` à mão é permitido — quem recria uma tabela precisa —,
mas só como `Bool` não nula. Com outro tipo, o esquema é recusado: seria uma
coluna comum com nome reservado, e o motor passaria a marcar exclusão num
campo que o usuário lê como texto. Nulo também é recusado: seria um terceiro
estado entre excluída e não excluída.

### A coluna de sistema `rownum`

`UInt8` não nula, e ela entra **depois** da `softdeleted` — coluna de sistema
nova sempre no fim, senão uma tabela gravada na versão anterior teria os
*offsets* deslocados ao ser relida.

É o **número de ordem de chegada** da linha. O motor preenche; não se escreve à
mão e não se ajusta — um valor escolhido seria uma ordem inventada. Nunca
reaproveita número, nem depois de exclusão: se reaproveitasse, uma linha nova
apareceria **atrás** de um cursor parado numa página, e a paginação passaria a
pular registro sem avisar. Alterar a linha não renumera.

O contador vive nos bytes 92..100 do cabeçalho do volume 1 e vai ao disco no
`sincronizar`, como os outros.

**Por que ela existe, se já há o `rowid`.** O `rowid` é a *posição física*.
Enquanto o volume sai de divisão, posição e ordem de chegada são a mesma coisa
e o rowid serve de cursor sozinho. Na **partição alfanumérica** não são: a
linha vai para o volume da letra dela, e duas linhas digitadas em seguida caem
em arquivos diferentes com rowids que não se comparam. O `rownum` é o que
sobra de monotônico.

**Ela não é `Sequence`.** Uma tabela só pode ter uma coluna `Sequence` — o
contador do `.reg` é único —, e reservar essa única vaga para o motor tiraria
do usuário um tipo que é dele. O `rownum` tem contador próprio.

**Como ela pagina sem índice.** O `rownum` cresce com o `rowid`, porque o
`.reg` guarda as linhas na ordem de chegada. Uma sequência crescente num
arquivo de acesso aleatório se procura por **bissecção**: achar a linha de
número 500.000 num milhão custa vinte leituras, sem índice nenhum a manter. É
o mesmo motivo de o endereço sair de uma conta — a ordem lógica é a ordem
física.'''
assert velho2 in s
s=s.replace(velho2,novo2,1)
io.open(p,'w',encoding='utf-8').write(s)
