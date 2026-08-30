# Document version field in FORMATO.md
# 29/08 00:04

import pathlib
p = pathlib.Path("docs/FORMATO.md")
s = p.read_text()
alvo = '''### Payload

```
[bitmap de nulos: ceil(n_colunas / 8) bytes][coluna 0][coluna 1]...
```'''
novo = '''### A versão do registro, e para que ela serve

Os oito bytes em `8..16` valem 1 quando a linha nasce e sobem **um a cada
regravação** — alteração de rotina e exclusão suave incluídas, porque as duas
regravam o slot.

O campo estava no formato desde a v1 sem ninguém usar. A partir da 0.17.0 ele é
a **janela de conflito de escrita**: o cliente lê a linha e guarda a versão, e
manda a versão de volta no `atualizar`; o servidor recusa com o erro 3004
(`CONFLITO`) quando ela não é mais a atual. Sem isso, duas pessoas com a mesma
ficha aberta terminavam com a segunda gravação apagando o trabalho da primeira
sem erro e sem registro.

Conferir custa **24 bytes** de leitura, e não a linha: quem pergunta se pode
gravar não precisa do conteúdo, e uma tabela com memo de megabytes cobraria o
arquivo externo inteiro por uma pergunta de oito bytes.

Ela **não é uma trava**. Travar a linha na leitura prenderia o registro toda vez
que alguém fechasse o navegador com a ficha aberta, e duas sessões que travam em
ordem trocada se abraçariam. O contador não prende nada — só recusa a segunda
gravação quando ela chegou depois de alguém ter mudado a linha.

### Payload

```
[bitmap de nulos: ceil(n_colunas / 8) bytes][coluna 0][coluna 1]...
```'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)
p.write_text(s)
print("ok")
