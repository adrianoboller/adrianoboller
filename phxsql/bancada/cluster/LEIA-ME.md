# Bancada do cluster — eleição e promoção automática (pedido 126)

```bash
cargo build --release
python3 bancada/cluster/provar.py [diretorio]     # padrão /tmp/phx-cluster
```

Sobe **três** `phxsqld` próprios em 127.0.0.1:5310–5312 e um **SMTP falso**
em 5316 que captura tudo que os nós mandam. Nenhum `pkill`: cada servidor é
morto **pelo PID que o script guardou** — o demo em 5199/5599 nunca é tocado.

Configuração usada: janela de inatividade **4 s**, pulso **1 s**, aviso por
e-mail a cada **0.1 min (6 s)** — o campo aceita fração justamente para a
repetição ser provável em segundos de teste. Prioridades: no2=2, no3=1, para
a eleição ter um vencedor previsível quando as posições empatam.

## O resultado esperado, escrito ANTES de rodar

| passo | o que se faz | o que TEM de acontecer |
|---|---|---|
| (a) | sobe no1 (source), no2, no3 | `cluster_estado` responde nos três com o MESMO master (no1, 5310), época 0, três vivos |
| (b) | 3.000 linhas no master | as duas réplicas alcançam; retrato SHA-256 idêntico nos três; escrever no no2 devolve o erro `REDIRECIONA 127.0.0.1:5310` (nome `REDIRECIONA`, código 4003) |
| (c) | **mata o no1** pelo PID | em até ~janela+2 pulsos (≈6 s) o no2 se promove (prioridade maior no empate de posição); o no2 aceita escrita; o no3 passa a segui-lo e o `REDIRECIONA` dele aponta 5311 |
| (d) | espera ~14 s | o SMTP falso capturou: **exatamente 1** e-mail de promoção; e-mails de degradação citando o no1 caído, **repetidos** (≥2 do mesmo nó, a cada 6 s) |
| (f) | **sobe o no1 de volta** | ele se vê destronado pela época maior no pulso e **se rebaixa sozinho** a réplica; alcança as escritas feitas na ausência |
| (e) | mata no1 e no2: só o no3 vivo (1 de 3) | por 3× a janela o no3 **NÃO se promove** — degradado dizendo `NAO promovo`, escrita recusada, e-mail de "sem maioria" capturado. É o teste de proteção da bancada |
| — | sobe no1 e no2 de volta | o cluster sara sozinho: no2 volta master (papel persistido, época 1), retratos idênticos nos três |
| (g) | mata tudo; sobe 4 servidores SEM bloco `cluster` (1 source + 3 réplicas com origens fixas) | replicação funciona como hoje; `cluster_estado` responde erro claro ("não está em cluster"); **nenhum** e-mail chega ao SMTP falso |

A última linha é `RESULTADO <json>`, para não ter de adivinhar nada.

## O defeito reposto (prova real da bateria)

O teste unitário `cluster::testes::sem_maioria_visivel_nao_promove` protege a
conferência de maioria em `vencedor()` (`cluster.rs`). Prova real feita:
removida a conferência (`if vivos.len() * 2 <= total_configurado`), o teste
**falhou** — e só ele — e voltou a passar com a conferência restaurada. O
passo (e) desta bancada prova a mesma coisa pelo soquete, contra o sistema
operacional.

## A última corrida

Os números da última corrida ficam em `resultados.json`, gravado pelo
próprio `provar.py` — número digitado à mão envelhece calado.
