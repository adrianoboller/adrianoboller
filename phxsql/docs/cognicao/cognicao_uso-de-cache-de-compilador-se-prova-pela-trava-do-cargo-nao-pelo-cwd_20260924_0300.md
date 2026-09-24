# O uso de um cache de compilador se prova pela trava do cargo, não pelo `cwd`

Pedidos 317 e 389. Papel D (zelador), conserto pelo papel A em 24/09/2026.

## 1. O que aconteceu

O dono pediu «zelador mais atenção». Medido antes de mexer: `./zelador.sh --ver`
prometia liberar **0 MiB** com o disco em **2.462 MiB livres**, porque dizia
«alguém trabalha aqui, não toco no target (8.999 MiB)». O `target/debug/incremental`
sozinho tinha **3.033 MiB**. Cortado à mão, com a trava do cargo segurada durante o
`rm` e nenhum processo com `cwd` ou descritor dentro: disco a **5.473 MiB**.

## 2. O que eu concluí primeiro, e estava errado

**Primeiro:** que o conserto era o 389 — o zelador se acha pelo `cwd` do próprio
`cd`, excluir a linhagem resolveria. Excluí, e a árvore continuou «em uso»: o PID
que segurava era o **shell de uma frente vizinha**, um `until [ -s … ]; do sleep 10`
de 17 minutos esperando o próprio resultado. Nesta máquina há sempre um desses. O
`cwd` responde «alguém PODE compilar aqui», nunca «alguém está compilando».

**Segundo:** que um vigia a cada 30 min bastava. A primeira corrida do vigia foi
**recusada**: o portão `esta-medindo.sh` conta `cargo` vivo como medição em curso,
e havia um `cargo test`, o provador de guardas e um servidor falso de teste de
tela. Com cinco frentes compilando, a corrida comum quase nunca passa.

**Terceiro, na prova:** o mutante que tira só a conferência da trava reprovava com
a frase «apagou o incremental de quem compila» — e o incremental estava **intacto**,
porque o `rm` roda dentro de um segundo `flock -n`. A prova acusava pelo veredito e
anunciava um dano que não houve.

## 3. O que a medição disse

- O incremental voltou de 0 a **1.126 MiB** em cerca de 15 minutos de frentes
  compilando, e o disco caiu de 5.473 para **4.168 MiB** no mesmo intervalo.
- Prova `bancada/zelador/prova-do-zelador.sh`: **9/9 verde** no zelador novo.
  Vermelho nos quatro mutantes: linhagem que volta a contar **1** falha (o caso da
  árvore ociosa acusa o próprio PID); corte removido **7**; conferência da trava
  removida **2** (dado sobrevive pela segunda camada, e a prova agora diz isso);
  as duas camadas removidas **6**, com o incremental **apagado** no meio de um
  `cargo build` de verdade.
- O caso 8 prova a premissa contra o SO: um `cargo build` real com `build.rs` que
  dorme 20 s segura `target/debug/.cargo-lock`, e o `flock -n` do zelador recusa.
- O maior consumidor que sobra não é cache: são **27 binários de teste de
  integração de ~68 MiB cada, 1.906 MiB**, um por arquivo de `tests/`.

## 4. A regra

**Cache de compilador se apaga segurando a trava do próprio compilador — o `cwd`
diz quem pode compilar, a trava diz quem está compilando. E prova de proteção em
camadas mede o dano separado do veredito.**

## 5. Como está guardado hoje

- `zelador.sh`: linhagem excluída do `em_uso` (389); corte do `incremental` de cada
  perfil sob `flock -n` do `.cargo-lock`, quando frio (6 h) ou abaixo de
  **4.096 MiB** (317); trava que não existe **não** se cria para conferir.
- `./zelador.sh --vigiar 30`: olha o disco a cada 2 min; abaixo do piso chama
  `--mesmo-assim` (no máximo a cada 5 min), acima faz a corrida comum a cada 30.
- `comunicacao.sh` acusa vigia parado como problema, pelo **argv** e não pelo texto.
- **Buraco que fica:** o vigia vive enquanto o contêiner vive; contêiner novo nasce
  sem ele até alguém rodar o comando que o batimento imprime. E o 317(a) — o provador
  apagar o próprio cache ao fim da rodada — continua aberto.
