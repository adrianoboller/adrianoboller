# Update the manual pagination section
# 28/08 20:00

import pathlib
p = pathlib.Path("MANUAL.txt")
s = p.read_text()

antigo = """PAGINACAO: CURSOR, E NAO POSICAO

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
"""
novo = """PAGINACAO: ANDA POR CURSOR, SALTA POR POSICAO

    Sao duas coisas diferentes, e as duas sao baratas.

    ANDAR -- "depois" leva o rowid onde a pagina anterior parou; "antes" faz o
    caminho de volta:

        {"op":"varrer","database":"loja","tabela":"clientes",
         "max":200,"depois":4000}

    SALTAR -- "pular" e o OFFSET do SQL, e e o que a caixa "ir para a pagina"
    da grade manda:

        {"op":"varrer","database":"loja","tabela":"clientes",
         "max":200,"pular":100000}

    PELO NUMERO DE ORDEM -- "desde_rownum" comeca na linha de numero N,
    inclusive. E o cursor de quem guardou o numero de ordem em vez do rowid.

    A resposta traz o par pronto para pedir a proxima:

        cursor_inicio   o primeiro rowid desta pagina
        cursor_fim      o ultimo -- e o que vai no "depois" seguinte
        rownum_inicio   o numero de ordem da primeira linha
        rownum_fim      o da ultima -- e o que vai no "desde_rownum" seguinte
        ha_mais         tem pagina depois desta?
        ha_antes        tem pagina antes?
        registros       quantas linhas ha no .reg, do CABECALHO
        visiveis        quantas ESTA visao enxerga -- a conta de "de quantas"
        marcadas        quantas estao marcadas como excluidas
        modo            "cursor", "posicao", "rownum" ou "indice"
        salto           so no modo posicao: "bisseccao" ou "passo"

    POR QUE ANDAR E BARATO. Continuar depois do rowid um milhao e uma conta, e
    nao uma procura, porque aqui a ordem logica E a ordem fisica:

        offset = data_offset + (rowid - 1) x slot_size

    POR QUE SALTAR TAMBEM E. Se ninguem apagou de vez e ninguem marcou, a
    POSICAO de uma linha na lista e o "rownum" dela menos um -- e ai o inicio
    da pagina sai de uma bisseccao de vinte leituras em vez de "pular" passos.
    O motor confere as duas condicoes no cabecalho, em tempo constante, e diz
    em "salto" qual caminho pagou:

        bisseccao   a posicao e o numero de ordem: busca binaria
        passo       a tabela tem buraco, ou e alfanumerica, ou a visao e a das
                    excluidas -- ai anda ate a posicao, uma linha por vez

    Os dois devolvem a MESMA pagina. O que muda e o preco. Medido numa tabela
    de 200.000 linhas, pelo protocolo, pedindo 200 linhas:

        pular       bisseccao   passo
        200              7 ms    6 ms
        20.000           7 ms   18 ms
        100.000          6 ms   72 ms
        199.800          6 ms  131 ms

    A bisseccao e PLANA -- e os 6 ms dela sao decodificar e serializar as 200
    linhas, e nao achar o comeco. O passo cresce com a profundidade, e cresce
    com a tabela.

    "ha_mais" sai de UMA leitura alem do teto, e nao de contar a tabela.
    "visiveis" sai de dois contadores do cabecalho -- e por isso "pagina 3 de
    40" voltou a existir sem custar varredura.

    O CAMPO rownum. Toda tabela tem a coluna de sistema "rownum": o numero de
    ordem de chegada da linha. O motor preenche, nunca reaproveita numero, e
    alterar nao renumera. Na grade ele e a coluna "n" da esquerda.

    Ele existe por causa da particao alfanumerica, onde o rowid deixa de
    crescer com a chegada. Fora dela, rowid e rownum andam juntos -- e e
    justamente essa coincidencia que o salto por bisseccao usa.
"""
assert antigo in s
s = s.replace(antigo, novo)
p.write_text(s)
print("ok")
