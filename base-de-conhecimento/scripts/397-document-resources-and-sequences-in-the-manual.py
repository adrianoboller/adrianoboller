# Document resources and sequences in the manual
# 28/08 14:06

import pathlib
p = pathlib.Path('MANUAL.txt')
s = p.read_text()
v = '''    somente_leitura   recusa qualquer operacao de escrita'''
n = '''    somente_leitura   recusa qualquer operacao de escrita'''
assert s.count(v) == 1

# a secao de recursos no manual
marca = '''OPERACOES'''
assert s.count(marca) >= 1
SECAO = '''RECURSOS DA MAQUINA

    A secao "recursos" do config.json diz o que o servidor pode consumir e --
    o mais importante -- QUANDO o que foi gravado vai de fato para o disco.

    "recursos": {
      "durabilidade": "por_lote",
      "lote_operacoes": 200,
      "lote_milissegundos": 200,
      "cache_paginas": 4096,
      "memoria_max_mb": 0,
      "threads": 0,
      "cpu_percentual": 100,
      "conexoes_max": 64,
      "usuarios_max": 0
    }

    Todos os tetos aceitam ZERO, e zero quer dizer "sem teto imposto aqui" --
    nao "desligado".

    DURABILIDADE. E o campo que mais muda a velocidade da gravacao. Medido com
    20.000 linhas, mesma tabela, mesma maquina:

        sincroniza a cada linha .....  1.289 linhas/s
        a cada 100 .................. 18.264 linhas/s   14,2x
        a cada 1.000 ................ 24.858 linhas/s   19,3x
        so no fim ................... 26.301 linhas/s   20,4x

    Ou seja: 95% do tempo de uma insercao era fsync. Os tres modos:

        por_operacao  fsync depois de cada gravacao. Nao perde nada nem numa
                      queda de energia, e custa 20x
        por_lote      fsync a cada N gravacoes ou T milissegundos, o que vier
                      primeiro. E o padrao
        sistema       nunca chama fsync; o sistema operacional decide. O mais
                      rapido, e o que mais perde numa queda

    O QUE SE ARRISCA, exatamente. Os bytes vao para o sistema operacional em
    TODA gravacao, sempre -- um write direto, sem buffer nosso. Entao outro
    processo que abrir o arquivo ve o dado na hora, sincronizado ou nao. O
    fsync protege de UMA coisa: o computador perder energia antes de o sistema
    descarregar a pagina. Em "por_lote" a janela do que se perde e o que entrou
    nos ultimos `lote_milissegundos`.

    Um relogio de fundo fecha a janela quando ninguem grava. Sem ele, a ultima
    venda do dia as 18h ficaria sem fsync a noite inteira.

    CPU. O "cpu_percentual" nao e uma cota do sistema operacional -- ele nao
    tem como impor isso a um processo. E quantos nucleos o trabalho DIVIDIDO
    usa: 50 em oito nucleos usa quatro. Cortar a divisao pela metade e o unico
    jeito honesto de "usar menos CPU" sem mentir sobre o mecanismo.

    USUARIOS x CONEXOES. Nao sao a mesma coisa: um usuario pode ter varias
    conexoes. "conexoes_max" conta soquetes; "usuarios_max" conta logins
    diferentes ao mesmo tempo, que e o que uma licenca por posto quer contar.

SEQUENCIAS

    A operacao "sequencias" junta, num lugar so, o contador de cada tabela do
    banco: o nome da tabela, qual coluna e a Sequence, o proximo numero que ela
    vai dar e quantos registros existem. O administrador ajusta com
    "ajustar_sequencia" -- para zerar depois de esvaziar a tabela, ou para
    pular uma faixa reservada a outra origem.

    ONDE O NUMERO MORA. Cada tabela guarda o proprio contador no cabecalho do
    .reg dela, e continua assim. A operacao JUNTA os contadores para mostrar;
    nao existe um arquivo "sequences" com uma segunda copia.

    A razao e a mesma que impede gravar "e chave primaria" na coluna: uma
    segunda copia e uma segunda verdade, e as duas divergem no primeiro caminho
    que esquecer de atualizar uma delas. Alem disso um arquivo separado
    custaria uma leitura e uma gravacao a mais por insercao -- justamente na
    operacao que ja e a mais cara.

    BAIXAR O CONTADOR E PERIGOSO, e a resposta avisa: se ja houver numero
    gravado na faixa, a proxima insercao repete, e um indice unico recusa. O
    erro aparece longe de quem causou.

OPERACOES'''
s = s.replace(marca, SECAO, 1)

v = '''    sistabelas      database                 o catalogo de tabelas'''
n = '''    sequencias      database                 o contador de cada tabela
    ajustar_sequencia database, tabela, proxima   zera ou muda o contador
    sistabelas      database                 o catalogo de tabelas'''
assert s.count(v) == 1
s = s.replace(v, n)
v = '''                 sistabelas, siscolunas, pivotar'''
n = '''                 sistabelas, siscolunas, pivotar, sequencias'''
assert s.count(v) == 1
s = s.replace(v, n)
v = '''    administrar  acessos, ips, config, usuarios, excluir_tabela'''
n = '''    administrar  acessos, ips, config, usuarios, excluir_tabela,
                 ajustar_sequencia'''
assert s.count(v) == 1
p.write_text(s.replace(v, n))
print('MANUAL: recursos e sequencias')
