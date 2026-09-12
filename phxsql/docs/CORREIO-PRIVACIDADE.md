# CORREIO — privacidade: apagar é apagar, e não há rastro

> Regras do dono, 12/09/2026:
> - **Prioridade da mensagem** — já existe (`prioridade`: alta/media/baixa).
> - **Assunto pesquisável** — já existe (`assunto_ct` E2E, busca no cliente).
> - **Não existe rascunho.**
> - **Uma vez excluído da lixeira, o e-mail não fica em lugar nenhum.**
> - **Não existe log dos e-mails.**
> - **Não existe engenharia reversa das mensagens.**

As três últimas descrevem a mesma coisa por ângulos diferentes: o correio **não
deixa rastro forense**. Isto é o **oposto** do padrão do motor — e é uma exceção
declarada, não um esquecimento.

## 0. A pétrea que este bloco contraria (e por que pode)

O PhxSql, por padrão e de propósito, é **forense**: ao excluir, guarda a linha
inteira no `.trash` e escreve o motivo no `.reason` (o diário das exclusões),
para **restaurar** e para **auditar**. Isso é certo para dado de negócio — órfã
que ninguém vê é pior que órfã que dá erro.

O e-mail é o caso onde **guardar é o defeito**. Um sistema fim-a-fim promete que
a mensagem some quando o dono manda sumir; um `.trash` com a linha, ou um
`.reason` com metadado, ou um log, **quebram essa promessa**. Então a tabela de
e-mail (e a de anexo) é uma **exceção declarada** ao modelo forense: nasce
marcada **sem diário, sem lixeira física, sem log**.

A exceção **não** afrouxa a regra geral — ela vale só para as tabelas do correio,
onde a promessa de privacidade é o produto. Toda outra tabela continua forense.

## 1. Não existe rascunho

Compor **não persiste** até enviar. Não há estado `rascunho`, não há pasta de
rascunhos, não há linha meio-escrita gravada em disco. O que está sendo digitado
vive só na memória do cliente; fechou sem enviar, não sobrou nada. *(Some do
cliente a pasta «Rascunhos» e o botão «Salvar rascunho».)*

## 2. Excluir da lixeira é purga — não fica em lugar nenhum

O ciclo do e-mail tem **dois** passos, e só o segundo é irreversível:

1. **Mover para a lixeira** (`estado = na_lixeira`) — exclusão suave, **ainda
   volta**. É a marca rosa da casa.
2. **Esvaziar a lixeira** — **purga**: o e-mail sai do disco e **não volta**.

Na purga, ao contrário de toda outra tabela:

- **não** se copia a linha para o `.trash`;
- **não** se escreve no `.reason`;
- **não** se deixa log;
- os **bytes do blob são zerados** no `.reg` (crypto-erase do ciphertext).

E a pétrea que continua de pé: **a ordem de digitação é sagrada — o `.reg` não
reaproveita slot excluído.** A purga **zera o conteúdo**, não reusa o slot: o
lugar continua queimado (o `rownum` nunca volta), mas o que estava lá vira zero.
Purga apaga **dado**, não **ordem**.

## 3. Não existe log dos e-mails

O servidor **não mantém log** de e-mail — nem de conteúdo (que ele nunca vê),
nem de metadado de entrega, nem de exclusão. O que ele precisa para funcionar
(rotear, cobrar, moderar) vive na **linha viva**; sumiu a linha, sumiu tudo.
Não há arquivo `.log`/`.txt` guardando «fulano mandou para beltrano às tal
hora».

## 4. Não existe engenharia reversa

A garantia é, **primeiro, criptográfica** e só **depois** operacional:

- **Criptográfica (a forte):** o corpo e o assunto são **E2E** — o servidor
  nunca teve a chave. Mesmo um byte que sobrasse em qualquer lugar (um bloco de
  disco não sobrescrito, um backup antigo) é **ciphertext sem chave**: não se
  reverte. É o mesmo alicerce do SHA-256/ChaCha desta casa.
- **Operacional (o reforço):** a purga zera os bytes vivos e não deixa `.trash`/
  `.reason`/log, então nem o ciphertext fica para trás na parte que o motor
  controla.

As duas juntas fecham «não há engenharia reversa»: não há texto para reverter
(E2E) **e** não há blob para tentar (purga).

## 5. Reconciliação com o resto do PhxSql

- **Replicação.** O que trafega para as réplicas é **ciphertext** (o blob E2E),
  nunca texto. A purga **propaga** como uma operação de purga: a réplica também
  zera e não guarda diário. O diário de replicação do correio **não retém** a
  linha depois de aplicada.
- **PITR / restaurar.** O ponto do PITR é reaplicar o diário vivo; como o e-mail
  **não tem** `.reason`, um e-mail purgado **não volta** por PITR — que é
  exatamente o pedido.
- **Backup (`.bundle`).** Um backup tirado **antes** da purga contém o blob —
  mas **ciphertext sem a chave do usuário**, logo inútil para reverter a
  mensagem. A garantia de «sem engenharia reversa» **não depende** de reescrever
  backups; ela é criptográfica. Se o dono quiser além disso que a purga também
  reescreva backups históricos, é um pedido **mais pesado** e separado — e eu
  **não** recomendo, porque troca uma garantia forte (cripto) por uma frágil
  (rasurar arquivos imutáveis).

## 6. O que falta você decidir (formato/motor — entra cedo)

1. **O modo «purga» por tabela** no PSCH/motor: a tabela de e‑mail e a de anexo
   nascem com `sem_diario = true`, `sem_lixeira = true`, `sem_log = true`, e o
   `excluir` físico **zera os bytes** em vez de copiar para `.trash`. É mudança
   de caminho de exclusão — decisão de formato, com você.
2. **Confirmar** que a purga propaga na replicação como purga (a réplica zera e
   não guarda diário).
3. **Backups pré‑purga:** manter como estão (recomendado — é ciphertext sem
   chave) ou reescrever (não recomendado).

Prova real, quando entrar o modo purga: um teste que **falha** se, depois de
esvaziar a lixeira, sobrar linha no `.trash`, entrada no `.reason`, ou bytes não
zerados no `.reg` — e passa quando a purga é total. (Nos dois sentidos, como
manda a casa.)
