# Remedir o caminho fechado não acha o caminho aberto

**07/09/2026, 08:38 UTC.**

## 1. O que aconteceu

O dono pediu um batimento de comunicação **de 15 em 15 minutos**. Ele foi
montado como `Monitor` dentro da sessão, e o `Monitor` morre: o runtime trunca
o `timeout_ms` em 1.800.000 ms e ignora o `persistent: true`. Resultado
medido: ele cai a cada ~30 minutos.

O gatilho de hora em hora — esse sim durável — carregava a instrução de
**rearmar o Monitor** quando ele sumisse. Então a cada rodada eu rearmava, ele
morria, e eu rearmava de novo. A limitação estava registrada, estava certa, e
mesmo assim o pedido do dono ficou sem cumprir por rodadas seguidas.

## 2. O que eu concluí primeiro, e estava errado

Que a limitação já estava medida e não havia o que remedir. Eu tinha o número
— 30 minutos, o teto do `timeout_ms` — e ele estava **correto**. Por isso
parecia caso encerrado: já medido, já registrado, resta rearmar.

O erro não foi medir mal. Foi medir a pergunta errada. Eu remedia «o Monitor
sobrevive?» — que já tinha resposta, e sempre a mesma — em vez de «existe
outro mecanismo?», que nunca foi perguntada uma única vez. **Limitação exata
sobre o mecanismo A não diz nada sobre a existência do mecanismo B**, e é
justamente por ser exata que ela convence e faz parar de procurar.

Cheguei a oferecer o caminho ao dono («o caminho é o gatilho agendado») e
parei ali, esperando decisão — sem medir se aquele caminho sequer funcionava.
Oferecer um caminho não medido é palpite com cara de plano.

## 3. O que a medição disse

Três medições, todas de 07/09/2026 08:38 UTC, contra o serviço de gatilhos:

| tentativa | resultado |
|---|---|
| `Monitor` com `persistent: true` | cai em ~30 min — teto de `timeout_ms` em 1.800.000 ms |
| `create_trigger` com cron `*/15 * * * *` | **RECUSADO**: «may fire runs as little as 15 minutes apart; the minimum interval is 1 hour» |
| `send_later` com `delay_minutes: 15` | **ACEITO** — `fire_at: 2026-09-07T08:54:00Z`, `trigger_id trig_01PnMzvehFwtV1aYuefg74cU` |

O piso de uma hora é do **cron**, e não do agendador: o tiro único não tem
piso nenhum acima de um minuto. Os dois passam pelo mesmo serviço; a recusa é
da expressão de intervalo, não da frequência de disparo.

Daí sai o desenho que serve: uma **corrente de tiros únicos**, cada elo
rearmando o próximo com `send_later`. Cada elo é guardado pelo servidor, então
sobrevive à sessão — que é exatamente o que o `Monitor` não faz.

O preço, dito porque existe: a corrente é mais frágil que um cron. Elo que
falha ao rearmar quebra a corrente inteira, enquanto um cron volta sozinho no
disparo seguinte. Por isso o gatilho de hora em hora **fica** como piso: ele é
quem refaz a corrente quando ela arrebenta.

## 4. A regra

**Limitação medida fecha um caminho, nunca o assunto — remeça procurando o
mecanismo, não o número que já se tem.**

Se a remedição repete a mesma pergunta e dá a mesma resposta, ela não é
remedição: é a limitação se confirmando enquanto o pedido continua sem
cumprir.

## 5. Como está guardado hoje

- A corrente está **de pé**: primeiro elo em `trig_01PnMzvehFwtV1aYuefg74cU`,
  e a mensagem de cada elo manda forjar o seguinte antes de responder.
- O gatilho de hora em hora deixou de mandar rearmar `Monitor` — agora manda
  refazer a corrente por `send_later`, com o piso de uma hora do cron escrito
  ali para ninguém tentar de novo sem motivo novo.
- **Onde o buraco fica:** não há guarda que acuse a corrente arrebentada. Quem
  percebe é o aviso de hora em hora, ao notar que o elo fino não veio. Uma
  corrente que morre em silêncio entre dois avisos custa até 60 minutos de
  batimento fino, e isso não é medido por ninguém hoje.
