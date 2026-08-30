# Document the conflict window in MANUAL
# 29/08 00:36

import pathlib
p = pathlib.Path("MANUAL.txt")
s = p.read_text()
s = s.replace('''    ler             database, tabela, rowid''',
              '''    ler             database, tabela, rowid, [com_versao]''', 1)
s = s.replace('''    atualizar       database, tabela, rowid, valores
    excluir         database, tabela, rowid, [motivo], [fisico]
    restaurar       database, tabela, rowid, [motivo]   desfaz a exclusao suave''',
'''    atualizar       database, tabela, rowid, valores, [versao]
    excluir         database, tabela, rowid, [motivo], [fisico], [versao]
    restaurar       database, tabela, rowid, [motivo], [versao]''', 1)

alvo = '''TIPOS NO JSON'''
novo = '''A JANELA DE CONFLITO DE ESCRITA

    O caso: alguem abre a ficha do registro 42 as 9h02, sai para o cafe, volta
    as 9h11 e salva. Entre uma coisa e outra, outra pessoa gravou a mesma
    linha. Sem conferencia, o segundo "salvar" apaga o trabalho do primeiro --
    sem erro, sem registro, sem ninguem perceber ate faltar o dado.

    A peca ja estava no formato: cada slot do .reg guarda uma VERSAO, que sobe
    a cada regravacao. O caminho e:

      1. leia com "com_versao": true

         {"op":"ler","database":"Z","tabela":"Clientes","rowid":42,
          "com_versao":true}

         A resposta muda de forma quando se pede a versao -- ela nao pode
         entrar como mais uma chave dentro da linha, senao viraria uma coluna
         que nao existe no esquema:

         {"rowid":42,"linha":{...},"versao":3}

      2. mande a versao de volta ao gravar

         {"op":"atualizar","database":"Z","tabela":"Clientes","rowid":42,
          "valores":{...},"versao":3}

         A resposta traz a versao NOVA, para quem grava duas vezes seguidas
         nao ter de reler a linha inteira no meio:

         {"rowid":42,"versao":4}

      3. se alguem gravou no meio, a recusa e o erro 3004 CONFLITO

         "conflito de escrita: o registro 42 de Clientes esta na versao 5 e
          voce leu a 3: outra sessao gravou nesse meio-tempo"

    A CONFERENCIA E PEDIDA, NAO IMPOSTA. Quem manda "versao" ganha a garantia;
    quem nao manda continua com a ultima gravacao vencendo, que e como todo
    cliente anterior a 0.17.0 funciona. Zero e o mesmo que ausente: a versao de
    um registro vivo comeca em 1.

    Vale tambem no "excluir" e no "restaurar": excluir uma linha que outra
    pessoa acabou de alterar e a mesma janela.

    EXCLUIDA DE VEZ TAMBEM E CONFLITO, e nao "nao encontrado": quem leu a linha
    ha um minuto precisa saber que ela foi apagada, e nao que o rowid nunca
    existiu.

    NAO E TRAVA. Travar a linha na leitura resolveria o mesmo problema e
    criaria dois piores: a linha fica presa quando alguem fecha o navegador com
    a ficha aberta, e duas sessoes que travam em ordem trocada se abracam.

    NA INTERFACE WEB isso ja acontece sozinho: a ficha guarda a versao ao
    abrir, manda no salvar, e quando ha conflito mostra as tres colunas --
    "valor anterior", "o outro escreveu", "voce escreve" --, ja com a escolha
    marcada em quem MEXEU em cada coluna. Dois que editaram campos diferentes
    da mesma linha saem dali com os dois trabalhos.

TIPOS NO JSON'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)
p.write_text(s)
print("ok")
