# `grep -c` sem casamento mata a prova — justamente no caso bom

## 1. O que aconteceu

`phxvpn/provas/mfa/rodar.sh` (`set -euo pipefail`) contava vazamentos de
senha e segredo nos logs com `grep -c … | awk`. Os cinco casos da conexão
OpenVPN passaram, e o roteiro **parou calado**: sem «PROVA OK», sem
«REPROVADA», sem `resultados.json`.

## 2. O que eu concluí primeiro, e estava errado

Que o `kill` do openvpn no começo do RED tinha falhado: a última linha na tela
era `kill: (32386): No such process`. Troquei a busca do PID (de `comm` para a
linha de comando com o caminho do conf) — e a prova parou no mesmo lugar, agora
**sem** mensagem nenhuma.

## 3. O que a medição disse

O `grep -c` devolve **1 quando não casa nada** — e «zero vazamentos» é
exatamente o caso em que ele não casa. Com `pipefail`, o 1 vira o status do
cano, e o `set -e` encerra o roteiro no caso que mais importava passar. Com
`{ grep -c … || true; }`: 5/5 casos, RED 2/2, «PROVA OK», 0 vazamentos.

## 4. A regra

**Em roteiro com `set -e` e `pipefail`, todo `grep` cujo «não achei» é o
resultado BOM leva `|| true` dentro do cano.** Prova que morre sem dizer onde
é pior que prova que reprova.

## 5. Como está guardado hoje

Consertado no `rodar.sh` (linha do `VAZOU`). Não há conferidor que ache o
mesmo padrão nos outros roteiros da pasta `phxvpn/`; o `prova-openvpn.sh` usa
`grep -q` dentro de `||`, que não tem o problema.

## Estado

- **Estado:** INFRUTÍFERO
- **Evidência:** `phxvpn/provas/mfa/rodar.sh`, `commit:0e5352c`
- **Causa:** `grep -c` sem casamento sai com 1; sob `set -e` e `pipefail` isso mata o roteiro justo no caso bom, sem dizer onde.
- **Prevenção:** todo `grep` cujo «não achei» é o resultado bom leva `|| true` dentro do cano.
- **Validado em:** 24/09/2026
