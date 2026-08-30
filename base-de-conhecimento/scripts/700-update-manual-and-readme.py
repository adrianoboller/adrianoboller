# Update manual and README
# 28/08 19:07

import io
p='MANUAL.txt'
s=io.open(p,encoding='utf-8').read()
velho='''    .reg + .ndx + .bin + .memo + .log + .trash + .reason = cadastroClientes'''
novo='''    .reg + .ndx + .bin + .memo + .log + .trash + .reason = cadastroClientes

E mais um arquivo de texto ao lado, o .pag: um JSON que descreve como a tabela
esta partida, para quem esta do lado de fora descobrir isso sem abrir o .reg.
Ele e GERADO e nunca lido pelo motor -- apagar nao quebra a tabela.'''
assert velho in s
s=s.replace(velho,novo,1)

velho2='''    varrer          database, tabela, [indice], [max], [visao]'''
novo2='''    varrer          database, tabela, [indice], [max], [visao],
                    [depois], [antes], [pular]'''
assert velho2 in s
s=s.replace(velho2,novo2,1)

velho3='''AS DUAS EXCLUSOES'''
novo3='''PAGINACAO: CURSOR, E NAO POSICAO

    A tela e o protocolo paginam por CURSOR. "depois" leva o rowid onde a
    pagina anterior parou; "antes" faz o caminho de volta:

        {"op":"varrer","database":"loja","tabela":"clientes",
         "max":200,"depois":4000}

    A resposta traz o par pronto para pedir a proxima:

        cursor_inicio   o primeiro rowid desta pagina
        cursor_fim      o ultimo -- e o que vai no "depois" seguinte
        ha_mais         tem pagina depois desta?
        ha_antes        tem pagina antes?
        registros       quantas linhas a tabela tem, do CABECALHO
        modo            "cursor" ou "posicao"

    POR QUE ISTO IMPORTA. Pular ate a posicao um milhao custa um milhao de
    passos; continuar depois do rowid um milhao custa uma conta, porque aqui
    a ordem logica E a ordem fisica:

        offset = data_offset + (rowid - 1) x slot_size

    Medido numa tabela de 20.000 linhas, dentro do navegador: 4,0 ms de media
    por pagina pelo cursor, sem crescer com a profundidade. Por posicao no
    mesmo ponto, 16,1 ms -- e crescendo.

    "pular" continua existindo para tela pequena e para cliente ja escrito. E
    o modo de compatibilidade, e a resposta declara isso em "modo".

    "ha_mais" sai de UMA leitura alem do teto, e nao de contar a tabela.
    Contar para mostrar "pagina 3 de 40" seria o item mais caro da tela numa
    tabela grande -- e e o que ninguem le.

    O CAMPO rownum. Toda tabela tem a coluna de sistema "rownum": o numero de
    ordem de chegada da linha. O motor preenche, nunca reaproveita numero, e
    alterar nao renumera. Na grade ele e a coluna "n" da esquerda.

    Ele existe por causa da particao alfanumerica, onde o rowid deixa de
    crescer com a chegada. Fora dela, rowid e rownum andam juntos.

AS DUAS EXCLUSOES'''
assert velho3 in s
s=s.replace(velho3,novo3,1)

velho4='''OPERACOES

    ping                                     versao, papel, conexoes'''
novo4='''PARTICAO ALFANUMERICA

    Um arquivo por letra inicial de uma coluna de referencia:

        Clientes_A.reg   Clientes_0.reg
        Clientes_B.reg   Clientes_1.reg     Clientes_Outros.reg
        ...              ...
        Clientes_Z.reg   Clientes_9.reg

    Sao 37 volumes fixos: A-Z, 0-9 e Outros. Escolhe-se na criacao da tabela,
    em Nova tabela -> Particao -> alfanumerica, com a coluna de referencia e o
    teto POR LETRA.

    Pelo protocolo:

        {"op":"criar_tabela","database":"loja","tabela":"clientes",
         "colunas":[...],
         "particao":"letra","particao_coluna":"nome",
         "registros_por_arquivo":1000000}

    TRES COISAS PARA SABER ANTES DE ESCOLHER:

    1. O TETO E POR LETRA, e nao da tabela. Num cadastro brasileiro o _S tem
       dez vezes o _K: quem enche primeiro derruba a insercao daquela letra
       com as outras 36 ainda com espaco.

    2. A ORDEM DE DIGITACAO MUDA DE CAMPO. O rowid passa a dizer em que
       ARQUIVO a linha esta, e nao quando ela chegou -- duas linhas digitadas
       em seguida caem em arquivos diferentes. A ordem de chegada fica no
       rownum, e a leitura sai em ordem alfabetica de balde.

    3. ALTERAR A COLUNA DE REFERENCIA E RECUSADO. Mudar "Silva" para
       "Andrade" mudaria o arquivo em que a linha mora, e com ele o rowid, que
       e a identidade dela em todo indice. Para mudar: exclua e insira de novo.

    Acento cai na letra sem acento (Avila e Avila com A-crase vao para o _A).
    Vazio e o que nao for letra nem algarismo vao para Outros. O balde que
    nunca recebeu linha NAO ganha arquivo.

    So o .reg se parte por letra. O .bin, o .memo, o .log, o .trash e o
    .reason rolam por tamanho e continuam com sufixo numerico -- um
    Clientes_B.log se leria como "o diario do balde B", e o diario e da
    tabela inteira.

OPERACOES

    ping                                     versao, papel, conexoes'''
assert velho4 in s
s=s.replace(velho4,novo4,1)
s=s.replace('''PhxSql - sete arquivos, uma tabela.''','''PhxSql - sete arquivos, uma tabela.''',1)
io.open(p,'w',encoding='utf-8').write(s)
print('manual ok')
