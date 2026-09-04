# Conferidor que discorda de conferidor não acusa — acalma

**Descoberto em 04/09/2026, 00:58**, ao entregar o pacote de fontes e rodar
`./empacotar.sh conferir` por hábito.

## 1. O que aconteceu

O empacotador reprovou **três dos oito pacotes** — `conhecimento`, `dossie` e
`kit`, todos de 30/08/2026 — e reprovou de um jeito que não parece corrupção:

```
== phxsql-0.18.0-dossie
9 arquivos no manifesto, 2994544 bytes no pacote

18 DIVERGENCIA(S):
  A MAIS  CHANGELOG.md -- nao esta no manifesto
  ...
  FALTA   ./CHANGELOG.md -- esta no manifesto e nao no pacote
```

Cada arquivo aparecia **duas vezes**: uma «a mais» e uma «faltando». Nove
arquivos, dezoito divergências. Os pacotes estavam **intactos** — o hash de
cada arquivo batia. O que não batia era a **grafia do caminho**: o manifesto
gravava `./CHANGELOG.md` e o `conferir-pacote`, que caminha a árvore e monta o
caminho relativo sem prefixo, procurava `CHANGELOG.md`.

A causa é a pétrea do irmão em forma de shell. A receita que grava o manifesto
existia em **quatro** lugares do `empacotar.sh`; o conserto (`sed 's|^\./||'`)
entrou em **um** — o `manifesto()`, que serve `fontes`, `linux`, `windows` e os
dois ARM — e os três irmãos (`dossie()`, `conhecimento()`, `kit()`) continuaram
com a receita velha, `find . -print0 | xargs -0 sha256sum`.

## 2. O que eu concluí primeiro, e estava errado

Quando o resumo da sessão registrou «três pacotes falham na verificação», eu
anotei como **pacote corrompido** e adiei: zip velho, provavelmente montado
antes de alguma mudança, para investigar depois.

Errado nos dois pedaços. Não estava corrompido — cada byte batia — e não era
velho: o `kit` reproduz o defeito hoje, com o `empacotar.sh` de hoje, se ninguém
tocar na receita. E o mais importante: eu tratei o veredito do conferidor como
*ruído do artefato* quando ele era *defeito da ferramenta*. Conferidor que
reprova é para se acreditar até se medir; foi a segunda ferramenta, passando
verde, que me deu permissão para não olhar.

## 3. O que a medição disse

O pacote traz **duas** maneiras de se conferir, e o `COMECE-AQUI.txt` oferece as
duas:

| ferramenta | veredito no `dossie` de 30/08 |
|---|---|
| `phxsql conferir-pacote <dir>` | **18 divergências** — reprova |
| `sha256sum -c MANIFESTO.sha256` | **OK** em todas as 9 linhas — passa |

O `sha256sum -c` aceita o `./` porque ele *abre pelo caminho escrito no
arquivo*; o `conferir-pacote` *caminha a árvore e compara chaves*. As duas
respostas estavam certas para a pergunta que cada uma faz — e discordavam.

Medido depois do conserto, com a receita única: **8 pacotes, 8 ÍNTEGROS**;
`fontes` com 551 arquivos, `conhecimento` com 1.348, `kit` com 11.

E a medição que explica os cinco dias: a guarda que pega isso **existia** —
`./empacotar.sh conferir` — e **não estava na bateria**. Rodava só quando
alguém a digitava. Digitei em 04/09 porque estava entregando um pacote; em
30/08, quem montou os oito não digitou.

## 4. A regra

**Quando duas ferramentas conferem a mesma coisa, a que passa verde é a
suspeita, não o álibi.** Discordância entre conferidores é defeito de
ferramenta até se provar o contrário — e a lenta de acusar é justamente a que
faz ninguém olhar para a outra.

E o corolário do alcance, que é o aprendizado novo sobre a pétrea do irmão:
**guarda que só roda quando alguém a digita não é guarda, é lembrança.** Ela
pegou o defeito na primeira vez que alguém a chamou — cinco dias depois.

## 5. Como está guardado hoje

- **A receita mora num lugar só.** `dossie()`, `conhecimento()` e `kit()`
  chamam `fecha()`, que chama `manifesto()`. O comentário do `manifesto()`
  conta por que o `sed` existe e o que a falta dele fez.
- **A prova real está na bateria**, parte `pacote`:
  `bancada/pacote/provar-manifesto.py`. Ela prova nos **dois sentidos** — a
  receita de hoje sai ÍNTEGRA, e a receita antiga, **reposta ali de propósito**,
  reprova com exatamente 2 divergências por arquivo, que é a assinatura do
  defeito. Sem esse segundo sentido, um conferidor que dissesse ÍNTEGRO para
  tudo passaria verde.
- **Ela também trava a volta do irmão**: falha se qualquer linha fora do
  `manifesto()` gravar o `MANIFESTO.sha256`, e falha se `xargs -0 sha256sum`
  reaparecer no `empacotar.sh`. Conferido repondo o defeito no `dossie()`: a
  prova reprovou nomeando a linha 635, e voltou a passar ao desfazer.
- **O `empacotar.sh` ganhou `manifesto <dir>`** para a bancada chamar a receita
  de verdade em vez de copiar o `find` para dentro do teste — copiar receita foi
  exatamente o defeito que a prova existe para pegar.
- **O buraco que fica:** `./empacotar.sh conferir` continua fora da bateria, e
  de propósito — `pacotes/` está no `.gitignore`, então numa árvore recém
  clonada ele não teria zip nenhum para conferir e viraria um pulo permanente.
  Quem monta pacote ainda tem de rodá-lo à mão. A parte `pacote` cobre a
  **receita**; ela não confere os zips que estão no disco.
