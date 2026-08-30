# Update HFSQL.md 3.5
# 29/08 00:03

import pathlib
p = pathlib.Path("docs/HFSQL.md")
s = p.read_text()
alvo = '''Aqui não há detecção nenhuma: a segunda gravação vence em silêncio. O `.reg` já
guarda uma **versão por registro** que incrementa a cada alteração — a peça
está no formato, falta usá-la: recusar a gravação quando a versão que o cliente
leu não é mais a atual.

**É o item mais barato desta lista com o maior ganho de correção.**'''
novo = '''**Feito na 0.17.0**, e sem mudar formato: o `.reg` já guardava uma **versão por
registro** desde a v1, que sobe a cada regravação. A ficha lê a linha e guarda a
versão; o «Salvar» manda a versão de volta; o servidor recusa com o erro 3004
(`CONFLITO`) quando ela não é mais a atual. Conferir custa 24 bytes de leitura.

A janela mostra as três colunas do PDF e vai um passo além dele: **já vem
marcado quem mexeu em cada coluna**. A que você digitou fica com o seu valor, a
que só o outro mudou fica com o dele — duas pessoas que editaram campos
diferentes da mesma linha saem daí com os dois trabalhos preservados, sem ter de
escolher nada. Marcar tudo como «o meu» por omissão desfaria em silêncio o
trabalho do outro nas colunas que eu nem toquei, que é o mesmo estrago de antes
com mais cliques.

Três decisões que valem registro:

- **Não é trava.** Travar a linha na leitura resolveria o mesmo problema e
  criaria dois piores: a linha fica presa quando alguém fecha o navegador com a
  ficha aberta, e duas sessões que travam em ordem trocada se abraçam. O
  contador não prende nada — só recusa a segunda gravação.
- **A conferência é pedida, não imposta.** Quem manda `"versao"` ganha a
  garantia; quem não manda continua com a última gravação vencendo. Imposta,
  todo cliente escrito antes da 0.17.0 pararia de gravar de um dia para o
  outro — e o que ele receberia não seria proteção, seria um erro que ele não
  sabe tratar. A interface web manda sempre, porque é ali que existe gente e a
  janela de minutos entre abrir a ficha e clicar em salvar.
- **Excluída de vez também é conflito**, e não «não encontrado»: quem leu a
  linha há um minuto precisa saber que ela foi apagada, e não que o rowid nunca
  existiu.'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)
p.write_text(s)
print("ok")
