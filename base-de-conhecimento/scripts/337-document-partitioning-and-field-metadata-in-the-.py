# Document partitioning and field metadata in the manual
# 28/08 11:48

import pathlib
p = pathlib.Path('MANUAL.txt')
s = p.read_text()
v = '''    A PAGINACAO SE ESCOLHE NA CRIACAO E NAO MUDA DEPOIS. Ela e o divisor que
    transforma o rowid em endereco; trocar mais tarde mudaria o endereco de
    cada registro ja gravado. E nao existe "sem teto": o sufixo tem largura
    fixa, entao com tres digitos cabem 999 volumes. Teto deixado em zero vira
    o maior que couber.
'''
n = '''    A PARTICAO SE ESCOLHE NA CRIACAO E NAO MUDA DEPOIS. Ela e o que transforma
    o rowid em endereco; trocar mais tarde mudaria o endereco de cada registro
    ja gravado. E nao existe "sem teto": o sufixo tem largura fixa, entao com
    tres digitos cabem 999 volumes. Teto deixado em zero vira o maior que
    couber.

    DUAS REGRAS DE CORTE

    Por faixa de quantidade, o volume corta a cada N registros e o endereco
    sai de uma divisao:

        volume = (rowid-1) / registros_por_arquivo + 1

    Por periodo -- mensal, bimestral, semestral ou anual --, o volume corta
    quando o periodo de uma coluna de data vira, OU quando o volume enche, o
    que vier primeiro. A coluna tem de ser Date ou DateTime e obrigatoria.

    Os blocos comecam sempre em janeiro: bimestre e jan-fev, mar-abr, ...;
    semestre e jan-jun e jul-dez.

    A LINHA ATRASADA NAO VOLTA. Um lancamento de janeiro digitado em marco
    entra no volume de MARCO. A ordem de digitacao e sagrada: voltar seria
    escrever no meio de um arquivo ja fechado. Por isso o periodo de um volume
    e "o periodo em que ele abriu", e um volume pode conter linhas de periodos
    anteriores que chegaram depois. Quem quiser todos os lancamentos de
    janeiro usa o indice pela data -- e para isso que ele existe.

    CADASTRO DE CAMPOS

    Cada campo tem, alem do nome e do tipo:

        id          UUID v7 sorteado na criacao, NUNCA reaproveitado. E por
                    ele que uma tela ou um relatorio apontam para a coluna,
                    para que renomear o campo nao quebre nada
        caption     o rotulo de tela; vazio significa "use o nome"
        descricao   para que o campo serve
        mascara     o PICTURE do Clarion(R): @N-11.2, @D6, @P###-####P
        tamanho     sai do tipo; para Str e a largura declarada

    E o papel nas chaves -- primaria, estrangeira, composta -- e DERIVADO dos
    indices e das chaves estrangeiras, nao gravado no campo. Marcar "primaria"
    no proprio campo criaria uma segunda verdade ao lado do indice.

    CATALOGO DO SISTEMA

    SysTables lista as tabelas do banco com registros, chaves, particao e
    volumes. SysColumns e o dicionario de dados: cada campo com caption,
    descricao, mascara, tipo, tamanho e o papel na chave.
'''
assert s.count(v) == 1
p.write_text(s.replace(v, n))
print('MANUAL: particao e cadastro de campos')
