# Add table rights to MANUAL
# 29/08 00:34

import pathlib
p = pathlib.Path("MANUAL.txt")
s = p.read_text()
alvo = '''    NEGA POR OMISSAO: atividade que nao aparece vale false. Base listada
    manda (o "*" nao completa o que faltou). Sem a base e sem "*", nega tudo.

14.4 Os tres portoes'''
novo = '''    NEGA POR OMISSAO: atividade que nao aparece vale false. Base listada
    manda (o "*" nao completa o que faltou). Sem a base e sem "*", nega tudo.

14.3.1 O direito desce ate a TABELA

    Ate a 0.17.0 a permissao parava na base: quem lia a base lia todas as
    tabelas dela. A folha de pagamento e a tabela de clientes moram no mesmo
    banco porque o negocio e um so, e o direito de ler as duas nao e o mesmo.

    Dentro do objeto da base, "tabelas" escreve a regra de cada uma:

      "bases": {
        "Z": {
          "ler": true, "inserir": true, "alterar": true,
          "tabelas": {
            "folha":    { },
            "clientes": { "ler":true, "inserir":true, "alterar":true }
          }
        }
      }

    A REGRA DA TABELA SUBSTITUI A DA BASE naquela tabela -- nao soma nem
    corta, do mesmo jeito que a base ja fazia com o "*". E o que permite as
    duas coisas que a pratica pede:

      "*": { "ler":true, "tabelas": { "folha": {} } }
          tira a folha de quem le o banco inteiro

      "Z": { "tabelas": { "clientes": { "ler":true } } }
          da clientes a quem nao le o banco nenhum

    A ordem, do mais especifico para o mais geral: supervisor; a regra desta
    tabela nesta base; o "*" de tabela nesta base; a regra desta tabela na
    base "*"; o "*" de tabela na base "*"; e so entao a regra da base.

    Operacao que nao fala de tabela (bancos, criar_database, sistema) cai
    direto na regra da base. UM config.json SEM "tabelas" SE COMPORTA
    EXATAMENTE COMO ANTES.

    A arvore e o catalogo (tabelas, sistabelas, siscolunas) listam so o que
    da para abrir: o nome de uma tabela ja conta parte da historia.

    O que ainda nao desce e o direito por COLUNA.

14.4 Os tres portoes'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)

s = s.replace('''    bases       o poder base por base; "*" vale para as nao listadas''',
'''    bases       o poder base por base; "*" vale para as nao listadas.
                Dentro de cada base, "tabelas" desce ao nivel da tabela (14.3.1)''', 1)
p.write_text(s)
print("ok")
