# Duvidei do guarda, e o guarda tinha razão

- **Quando:** 2026-09-02, 22:00
- **Onde:** o agente de comunicação, contra um pacote de backup que eu fiz
- **Custo:** quase, o backup ficando desatualizado com eu achando que estava em
  dia — que é o pior estado de um backup

## O que aconteceu

O agente de comunicação avisou **backup ATRASADO**: o pacote mais novo parava
num commit velho. Eu tinha gerado um pacote cinco minutos antes, e a minha
primeira reação foi *«o aviso está errado»*.

Estava certo. Eu tinha feito `git bundle create` **à mão**, de dentro de
`phxsql/`, e o `backup.sh` grava no diretório de cima. O meu pacote existia,
mas num lugar onde nada o procura — e o conferidor, olhando o lugar certo,
achou o pacote anterior.

Quebrei uma pétrea que eu mesmo tinha citado duas horas antes: *«pacote gerado
por script, nunca montado à mão — pacote feito à mão é pacote que ninguém
consegue refazer igual»*. E o «igual» aqui incluía **onde ele fica**.

## O que eu concluí primeiro, e estava errado

Que o alarme era falso, porque eu me lembrava de ter feito o backup. Lembrar de
ter feito não é o mesmo que ter feito **do jeito que o resto do sistema
espera** — e a memória é justamente o que o medidor existe para substituir.

## O que a medição disse

O pacote à mão estava em `phxsql/`; os quatro anteriores, e o que o
`comunicacao.sh` procura, em `/home/user/adrianoboller/`. Rodado o `backup.sh`
de verdade, ele **restaurou e comparou a árvore** — o critério não é «o clone
não deu erro», é o SHA do `tree` bater dos dois lados:

    PACOTE PROVADO: phxsql-20260902-2200.bundle
      18M, 343 commits, arvore eddfd07d
      ponta: a3c6f1b

## A regra

**Quando o medidor contradiz a sua lembrança, a lembrança é a suspeita.** É
para isso que ele existe. O reflexo de conferir o instrumento primeiro é o
mesmo que faz alguém desligar um alarme que toca — e um alarme que se aprende
a ignorar é pior que alarme nenhum, porque dá a sensação de cobertura.

Este caso é o espelho de outro de hoje, e os dois juntos formam o par: de manhã
o instrumento **mediu a si mesmo** e eu quase reportei defeito inexistente; à
noite o instrumento estava certo e eu quase o descartei. A conclusão não é
«confie no medidor» nem «desconfie»: é **vá ver de qual dos dois é o erro,
antes de escrever a frase**.

## Como está guardado hoje

Nada mudou no código — a regra já existia e o guarda já funcionava. O que ficou
é este registro, e uma observação que vale a rodada: **o agente de comunicação
que o dono pediu hoje acusou um defeito de quem o construiu, três horas depois
de nascer.** Guarda que só pega erro dos outros não está sendo testado.
