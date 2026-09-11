# Cognição: o anti-spam do correio se prova nos DOIS canais, não só na mensagem

**Descoberta:** 11/09/2026 ~14:52 UTC.

## 1. O que aconteceu

O dono fechou as regras do correio: *"não pode ser possível spam"*, *"não pode
ser contatado sem você aprovar"*, *"alertas 🚨 e avisos 🔔 de prioridade alta,
média e baixa"*, *"não é possível abrir o e-mail sem a senha"*.

Estendi o protótipo `crates/phxsql-core/examples/correio-e2e.rs` e provei as
quatro: `cargo run --example correio-e2e -p phxsql-core` sai **12 checagens, 12
ok, PROVA VERDE**.

## 2. O que eu concluí primeiro, e estava errado

Achei que o gate da mensagem — *"sem relação de confiança aceita, não entrega"*
— já matava o spam. Está certo e **incompleto**: se qualquer um pode disparar
**solicitações de confiança** sem limite, o pedido vira o spam. A caixa de
pedidos inunda, e "não te contatam sem aprovar" não impede que te **atormentem
pedindo** aprovação. O canal de pedido era o vetor que eu não tinha olhado.

## 3. O que a medição disse

Fechei o canal de pedido e provei (12/12):

- **pedido repetido** do mesmo remetente → recusado (não há como empilhar
  pedidos);
- **remetente bloqueado** → nem consegue pedir;
- **teto de pendentes** por conta (constante `TETO_PENDENTES`);
- e a **prioridade é autenticada**: `prioridade` e `tipo` entram no **AAD** do
  AEAD, então rebaixar um alerta 🚨 gravado para "baixa" faz a etiqueta não
  conferir e a leitura recusa. Metadado que decide não pode viajar fora do selo.

## 4. A regra

**Anti-spam de correio se prova nos DOIS canais: o da mensagem (sem confiança
aceita não entrega) E o do pedido (sem repetição, com bloqueio e teto). E todo
metadado que decide — prioridade, remetente, destinatário — vai DENTRO do AAD,
nunca ao lado do selo, senão rebaixa-se um alerta sem quebrar nada.**

## 5. Como está guardado hoje, e onde o buraco ficou

Guardado no exemplo rodável (prova do modelo em memória). Buracos nomeados:

- **Formato em disco e protocolo 8000** — pendentes, decidem-se com o dono
  antes de gravar (papel C).
- **O teto de pendentes é um número escolhido (50), não medido** — quando
  houver uso real, medir e ajustar; número citado é número que não se mede.
- **Bloqueio é por par (dono, remetente)** — falta a política de domínio
  inteiro (bloquear `golpe.phxsql.com.br` de uma vez) e a revogação de
  confiança já concedida, que continuam abertas da cognição do modelo ECDH.
