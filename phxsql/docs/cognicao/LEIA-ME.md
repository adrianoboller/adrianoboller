# Cognição: um arquivo por aprendizado, com data e hora

Ordem do dono, 02/09/2026: *«Todo aprendizado novo seu deve virar um
`cognicao_assunto_data_hora.md`.»*

## Por que um arquivo por aprendizado, e não um diário

Porque um diário se lê inteiro ou não se lê. Um arquivo por aprendizado se
**procura**: quem vai mexer no gerador de PDF acha o da crase; quem vai trocar
a trava acha o do `RwLock`. E porque a data e a hora respondem a pergunta que
mais importa quando um aprendizado contradiz outro — **qual dos dois é o mais
novo**.

## O nome

```
cognicao_<assunto>_<AAAAMMDD>_<HHMM>.md
```

O `assunto` é o que se procuraria seis meses depois, em minúscula e com hífen:
`crase-no-template-literal`, e não `bug-js`. A hora é a da **descoberta**, e
não a do commit — é ela que diz o que já se sabia quando outra frente errou o
mesmo na mesma tarde.

## O que cada um carrega

Cinco seções, e a terceira é a que impede o documento de virar anedota:

1. **O que aconteceu** — o fato, com arquivo e número.
2. **O que eu concluí primeiro, e estava errado** — o diagnóstico plausível que
   veio antes do medido. Sem isto o documento ensina só a resposta, e o erro
   volta pelo mesmo caminho.
3. **O que a medição disse** — o número. *Número citado é número que não se
   mede.*
4. **A regra** — uma frase, na forma imperativa.
5. **Como está guardado hoje** — e, quando não está, **onde o buraco ficou**.
   Papel que não está cumprindo aparece como não cumprindo.

## O que NÃO vira cognição nova

Reafirmação de pétrea que já existe. Quando uma pétrea quebra de novo, o
aprendizado novo é o **alcance** dela — «a guarda existe, mas só cobre `ui/`»
—, e é isso que o arquivo registra. Uma terceira cópia da mesma lei não
acrescenta lei: acrescenta lugar onde a lei pode divergir de si mesma.

## E a diferença entre isto e o `CLAUDE.md`

O `CLAUDE.md` é a **lei**: curta, e o que ele diz vale sem discussão. A
cognição é o **processo**: como se descobriu, o que se errou antes, e o número.
Lei sem processo vira dogma que ninguém sabe defender; processo sem lei vira
história que ninguém aplica.
