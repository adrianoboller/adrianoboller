# O batimento de 15 minutos expira sozinho — e é a expiração que o conserta

- **Quando:** 2026-09-02, 20:11
- **Onde:** o agente de comunicação, pedido pelo dono
- **Custo:** nenhum, porque a falha avisa; teria sido silêncio se não avisasse

## O que aconteceu

O dono pediu aviso **de 15 em 15 minutos**. Duas descobertas de plataforma, nas
duas tentativas:

1. **Gatilho agendado não aceita menos de uma hora.** `*/15 * * * *` é recusado,
   e a recusa nomeia o mínimo — recusa boa, que diz o número em vez de falhar
   torto.
2. **O laço dentro da sessão tem teto de 30 minutos**, e `persistent: true`
   **não** o remove: pedi 60 min, recebi 30, duas vezes.

## O que eu concluí primeiro, e estava errado

Que `persistent: true` significava «sem teto». Significa «sem *o meu* teto» — o
da plataforma continua valendo. Anunciei ao dono um batimento contínuo de 15
min antes de ter visto a primeira expiração.

## O que a medição disse

Pedido `timeout_ms: 3600000`; concedido **1800000**, nas duas armadas. O
primeiro batimento nasceu 19:47 e expirou 20:11 — **24 minutos de avisos**, e
depois a notificação de expiração.

## A regra

**Cadência fina se monta em duas camadas, e a de baixo tem de sobreviver à de
cima.** Aqui: o laço de 15 min dentro da sessão, e o gatilho de hora em hora
que sobrevive ao reinício dela e cuja instrução manda **rearmar** o laço.

E o que salva o desenho: **a expiração NOTIFICA**. Não há buraco silencioso —
quando o batimento morre, eu sou acordado por isso mesmo e rearmo. Uma falha
que avisa não é falha, é ciclo. A que mataria o agente seria a que morresse
calada.

## Como está guardado hoje

O gatilho de hora em hora traz a ordem de rearmar no próprio texto, e o
`comunicacao.sh` é o mesmo nas duas camadas — um medidor só, para as duas não
divergirem.

**O buraco que fica:** se a sessão inteira cair, o batimento fino só volta na
próxima hora cheia. É o preço do piso da plataforma, e está escrito em vez de
escondido.
