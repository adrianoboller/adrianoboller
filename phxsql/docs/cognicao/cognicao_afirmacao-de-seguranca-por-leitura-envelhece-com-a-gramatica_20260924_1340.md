# Afirmação de segurança feita por leitura envelhece quando o analisador cresce

**Estado:** PENDENTE

A prova virá da fatia F1 do 495: um teste que manda a tautologia do `ARSENAL`
pela op `sql` e confere o que ela devolve, e que falha se a gramática mudar
o resultado sem ninguém perceber.

## O que aconteceu

No modelo de ameaça do pedido 495, o SEC escreveu que a injeção «se fecha por
desenho» porque o tradutor analisa e reserializa, e as classes clássicas
virariam dado ou erro. O integrador repetiu a frase na mensagem do commit
`38fca2a`. O papel J mediu contra o binário atual
(`bancada/seguranca/495/premissa_sec.py`): a tautologia do próprio `ARSENAL`
de `bancada/seguranca/injecao.py` **executa** e devolve **2 de 2** linhas.
Em 07/09 ela era recusada; a gramática cresceu desde então (`OR`, expressão
no `WHERE`), e ninguém voltou a olhar.

## O que eu concluí primeiro, e estava errado

Que «o analisador reserializa» queria dizer «o analisador protege». Não quer.
Um banco não tem como saber se o cliente concatenou o texto que recebeu: uma
tautologia bem formada é SQL válido, e executar SQL válido é o trabalho dele.
A defesa contra injeção é o `?`, na ponta que monta o texto. O banco pode
**detectar e avisar**, não impedir.

## O que a medição disse

- `premissa_sec.py`: a tautologia volta com `ok=true` e 2 linhas, igual ao
  controle «sem WHERE».
- O caso 1 do `injecao.py` julga só por tabelas e linhas antes e depois, e não
  vê leitura. Por isso a tautologia passa nele como PASSOU.

## A regra

Afirmação de segurança sobre o que o analisador recusa se prova por uma
corrida contra o binário, e se refaz toda vez que a gramática cresce. Não se
prova lendo o código.

## Como está guardado hoje

Não está: o buraco é o caso 1 do `injecao.py`, que não mede o que se leu.
Recado para G e SEC. A fatia F1 do 495 é onde a prova vai entrar.
