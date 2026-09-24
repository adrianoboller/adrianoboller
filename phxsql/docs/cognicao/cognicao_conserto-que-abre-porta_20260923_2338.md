# Cognição — um conserto pode abrir o caminho que ele mesmo fecha

**Descoberto em** 23/09/2026, 23:38 · pedido 435 · papel A (integrador), achado SEC

## 1. O que aconteceu

O pedido 435 pedia colapsar duas frases de erro do pulso do cluster — uma
dizia «este nó não tem pino», a outra «a prova não fecha» — porque a primeira
enumerava para um atacante justamente os nós que o pedido 278 ainda não
protege. A frente colapsou as duas num só texto que vai ao fio.

Medido depois de colapsar o texto: o campo `"ms"` da resposta **ainda**
separava os dois casos em **40 de 40** corridas, porque o caminho sem pino
voltava antes do X25519 e do HMAC — o mapa não tinha sumido, tinha mudado de
campo. Consertado com o **pino cego**: o nó sem pino paga o mesmo trabalho
contra `x25519::BASE` antes de recusar. Depois: **1/40, 0/40, 0/40**.

Todos os portões fecharam verdes: 2.820 testes, clippy zero avisos, 81 alvos
(1 não rodou, por artefato trocado por outra frente, não por falha de teste),
prova real nos dois sentidos, guarda catalogada.

## 2. O que eu concluí primeiro, e estava errado

Ao ler o relatório da frente, o veredito parecia fechado: texto colapsado,
relógio igualado, testes verdes. Só ao **ler a derivação da chave** em vez do
relatório é que apareceu o problema: `chave_do_par =
SHA256(ROTULO ‖ x25519::segredo(privada, publica))`. Com `publica = BASE`
(o ponto-base do X25519), o segredo compartilhado **é a própria chave pública
deste nó**, que todo par do cluster já conhece. Isso significa que a prova
contra o pino cego **fecha para qualquer par do cluster** — exatamente o
atacante que o pedido 278 existe para barrar.

O código continuava seguro, mas por um motivo que eu não tinha visto de
início: uma recusa incondicional, `if pino.is_none()`, chamada **depois** do
`conferir`. E o comentário acima dela descrevia essa linha como reforço para
«o dia impossível em que a prova fechasse» — como se fosse cinto extra — e
nenhum teste a travava. Concluir «está seguro porque os portões fecharam
verdes» teria sido o erro: um refatoramento que apagasse aquela linha
específica (por parecer cinto redundante) faria o nó sem pino aceitar forja e
ser marcado como provado, e nenhum teste do pacote acusaria.

## 3. O que a medição disse

| medida | número |
|---|---|
| corridas em que o campo `"ms"` separava os dois casos, com o texto já colapsado | 40 de 40 |
| corridas com o pino cego, primeira medição | 1 de 40 |
| corridas com o pino cego, medições seguintes | 0 de 40, 0 de 40 |
| testes da suíte, no fim | 2.820 passaram, 0 falharam, 81 alvos (1 não rodou, por artefato trocado por outra frente) |
| avisos do clippy | 0 |
| resultado do teste com a linha de recusa incondicional apagada (defeito reposto) | `aceito=true, marcado_provado=true` |

## 4. A regra

**Um conserto pode abrir o caminho que ele mesmo fecha.** O conserto do
relógio (o pino cego) criou um ponto novo onde uma prova forjada passaria —
a recusa incondicional que hoje o impede não estava travada por teste
nenhum, só por comentário. Portão verde não acha isso: **ler o que a função
faz, não o que o comentário diz**, acha.

## 5. Como está guardado hoje

- Teste `cluster::testes::forja_contra_o_pino_cego_nao_entra_nem_marca_provado`,
  que forja pela **mesma** derivação da produção; com a linha de recusa
  apagada, cai com `aceito=true, marcado_provado=true`. O teste também exige
  que a forja **feche** antes de aceitar o veredito — canário contra teste
  verde sem forjar nada.
- Guarda `pino-cego-sem-a-recusa-do-no-sem-pino`, catalogada em
  `bancada/guardas/catalogo.py`, **PROVADA**.
- `docs/SEGURANCA.md` §20 (e §20.4 para a guarda), commit `a272d8f`.
- Guarda irmã do mesmo achado, também catalogada:
  `erro-do-pulso-mapeia-quem-nao-tem-pino`.
