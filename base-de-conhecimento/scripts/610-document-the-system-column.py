# Document the system column
# 28/08 18:03

import io
p='docs/FORMATO.md'
s=io.open(p,encoding='utf-8').read()

velho='''### O bloco de esquema (`PSCH`, versão 3)

O bloco começa com `PSCH` e a versão. A versão **3** acrescentou os metadados
de coluna, o marcador de chave primária e o modo de partição. A leitura ainda
aceita a 2: tabela gravada antes abre normalmente, ganha um `id` v7 sorteado na
hora e os textos vazios. **Escrever, só na 3.**'''
novo='''### O bloco de esquema (`PSCH`, versão 4)

O bloco começa com `PSCH` e a versão. A **3** acrescentou os metadados de
coluna, o marcador de chave primária e o modo de partição. A **4** acrescentou
a coluna de sistema `softdeleted` e um byte no fim, com o sinal de *motivo
obrigatório*. A leitura ainda aceita a 2: tabela gravada antes abre
normalmente, ganha um `id` v7 sorteado na hora e os textos vazios.
**Escrever, só na 4.**'''
assert velho in s
s=s.replace(velho,novo,1)

velho2='''Por índice, os sinalizadores viraram um byte com dois bits: **único** no bit 0
e **primário** no bit 1.'''
novo2='''Por índice, os sinalizadores viraram um byte com dois bits: **único** no bit 0
e **primário** no bit 1.

### A coluna de sistema `softdeleted`

Toda tabela criada a partir da v4 ganha, **no fim da lista**, uma coluna `Bool`
não nula chamada `softdeleted`. Ela marca a linha como excluída sem apagar
nada: a linha some das listas e continua inteira no `.reg`, e `restaurar`
desfaz.

No fim, e não no começo, por uma razão de formato: assim os *offsets* das
colunas do usuário não mudam de lugar quando ela entra, e quem monta a linha
posicionalmente pode continuar mandando só as colunas que declarou — `inserir`
com N−1 valores preenche `false`, e `atualizar` com N−1 **mantém o que a linha
já tinha**.

A coluna entra na **criação** da tabela. Ler o esquema do disco não acrescenta
nada: a lista de colunas gravada é a verdade inteira. Se a leitura
acrescentasse a coluna, cada linha de uma tabela v3 passaria a ser lida com os
*offsets* deslocados — **silenciosamente**, porque o CRC do slot continuaria
batendo: os bytes seriam os mesmos, só a interpretação mudaria.

Uma tabela anterior à v4 continua legível exatamente como está. Ela só não tem
exclusão suave, e a mensagem de erro diz isso em vez de ler lixo.

Declarar `softdeleted` à mão é permitido — quem recria uma tabela precisa —,
mas só como `Bool` não nula. Com outro tipo, o esquema é recusado: seria uma
coluna comum com nome reservado, e o motor passaria a marcar exclusão num
campo que o usuário lê como texto. Nulo também é recusado: seria um terceiro
estado entre excluída e não excluída.'''
assert velho2 in s
s=s.replace(velho2,novo2,1)
s=s.replace('''desatualiza, e obriga quem copia os cinco arquivos a copiar um sexto.''',
            '''desatualiza, e obriga quem copia os arquivos da tabela a copiar mais um.''',1)
io.open(p,'w',encoding='utf-8').write(s)
print('ok')
