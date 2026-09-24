# Trocar o fio não reenvia o que já saiu pelo fio velho

**Descoberta:** 24/09/2026, 04:12, frente F2 do phxvpn (P2P por TCP e proxy).

## 1. O que aconteceu

O nó P2P ganhou um segundo fio até o repasse (`phxvpn/src/fio.rs`): UDP e,
sem confirmação do REGISTRO em 10 s, TCP. Na prova em netns com UDP
bloqueado (`phxvpn/provas/tcp/rodar.sh`), o `auto` levou **15,0 s** até o
primeiro ping pelo túnel, e o caso só-proxy (TCP desde o início) **5,3 s**.

## 2. O que eu concluí primeiro, e estava errado

Que o tempo do `auto` seria «reserva + um aperto», uns 10–11 s, e que o
proxy seria quase instantâneo, porque o TCP já nasce escolhido. Eu olhei só
para o fio: o REGISTRO já era refeito na hora em que o fio mudava
(`pedir_registro`), e achei que isso bastava.

## 3. O que a medição disse

O INICIO do Noise tinha saído **pelo fio velho** (o UDP que a rede engolia,
ou o TCP que ainda não tinha conectado) e ficou pendente; o nó só o repete a
cada `REPETIR_APERTO` = 5 s. Então o `auto` pagou 10 s de reserva + até 5 s
de espera do reenvio (15,0 s), e o proxy pagou o reenvio inteiro (5,3 s). Com
um contador de «fio novo» (`FioRepasse::geracao`) que o tique olha para
largar o aperto pendente e refazer na hora — depois do REGISTRO, pelo mesmo
fio —, os números viraram **11,0 s** e **1,1 s** (duas corridas seguidas: 11,1
e 1,0; 11,0 e 1,1).

## 4. A regra

Quando o caminho muda, refaça **tudo** o que está pendente no caminho velho,
não só o que o próprio caminho controla: estado de aperto, fila e
temporizador de reenvio moram fora do fio e não sabem que ele trocou.

## 5. Como está guardado hoje

Pelo número da prova (`provas/tcp/resultados.json`, `primeiro_ping_s`), não
por teste unitário: nenhum teste reprova se o contador sumir — os pacotes
ainda passam, só 5 s depois. É um buraco consciente; a bancada de TCP é o
guarda, e só se roda à mão (root, netns).

## Estado

- **Estado:** FRUTÍFERO
- **Evidência:** `phxvpn/provas/tcp/resultados.json`, `commit:8b35d60`
- **Validado em:** 24/09/2026
