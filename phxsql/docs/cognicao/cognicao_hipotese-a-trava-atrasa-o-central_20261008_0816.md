# Hipótese morta: «a trava global atrasa o central» explicava o p95 de 10.719 ms

**Estado:** INFRUTÍFERO

**Causa:** A cauda vinha de classificar a latência pela fase do commit: vendas cometidas no último segundo antes do `SIGKILL` ainda não tinham sido puxadas e só chegaram ~20 s depois, entrando no «antes». A trava não era a causa.

**Prevenção:** Classifique a latência de quem atravessa uma queda pela hora da chegada e publique a janela da queda como número à parte; antes de acusar a trava, separe a cauda por hora de chegada.

## 1. O que aconteceu

Pedido 678, `bancada/caixa-offline/medir.py`, 20 caixas, 10 s por fase (08/10/2026, 08:16): a chegada ao central das vendas cometidas antes da queda deu p95 de 10.719 ms (mediana 613 ms). Origem: `cognicao_latencia-de-chegada-misturada-com-a-janela-da-queda_20261008_0816.md`.

## 2. O que eu concluí primeiro, e estava errado

Que o central se atrasava com 20 origens chegando juntas (trava global segurando a aplicação, fila crescendo), a hipótese que o parecer do 325 (lacuna L1) deixava pronta.

## 3. O que a medição disse

Separadas pela hora da CHEGADA (depois da morte do processo, `antes_pega_pela_queda`), em 5 voltas: o p95 do «antes» caiu de 10.719 ms para 1.015–1.025 ms, e as vendas puxadas depois do religar foram 32–48 por volta, todas entre 20,2 e 22,6 s. A trava aparece em outro lugar: no `varrer` do central, de 1–3 ms (ocioso) para 251–595 ms no alcance. Número em `bancada/caixa-offline/resultados.json`. Ressalva: a corrida de 08:16 rodou com load alto, e uma segunda corrida que reproduza o 1.015–1.025 ainda não foi feita.

## 4. A regra

Antes de acusar a trava (ou a fila) por uma cauda, separe as amostras pela hora de chegada.

## 5. Como está guardado hoje

`medir.py` (`t_morte` e a fase `antes_pega_pela_queda`). O aprendizado vivo continua no arquivo da latência (PENDENTE, à espera de uma segunda corrida). Nenhuma guarda impede outra bancada de queda de repetir a mistura.
