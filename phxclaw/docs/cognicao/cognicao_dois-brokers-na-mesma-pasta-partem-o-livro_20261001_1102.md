# Dois SecretBroker na mesma pasta partem o livro de evidências

**Estado:** PENDENTE (a evidência existe na árvore, falta o commit onde a prova roda)

**Evidência:** `crates/phxclaw-agent/tests/voz_e_midia.rs::pagos::nenhum_provedor_pago_devolve_a_chave_no_erro_nem_segue_redirecionamento`
falhou com `"invalid record at line 4"` ao abrir a credencial da ElevenLabs pela terceira vez, e
passou com um broker só por pasta no processo (`canais::broker_em`). O mutante M6 (tirar o
reaproveitamento) é a reposição do defeito, medido em cópia isolada.

## O que aconteceu

A ElevenLabs entrou como provedor de dois motores (`speak` e `transcribe`) e de uma ferramenta
(`voice_list`). Cada um pedia a chave pela pasta `elevenlabs/`, e cada pedido abria um
`SecretBroker` novo sobre o mesmo `segredos/evidence.jsonl`. Cada broker encadeia o registro novo a
partir do **último registro que ele mesmo viu**; com três vivos, o segundo a escrever encadeia num
registro que já não é o último, e a próxima abertura do livro recusa a corrente.

## O que eu concluí primeiro, e estava errado

Que abrir o broker é barato e sem efeito, como abrir um arquivo para ler — o `xai` fazia assim e
nunca deu erro. Não deu porque o `x_search` abre **um** broker por montagem. O defeito existe em
todo lugar que abre a mesma pasta duas vezes com os dois vivos: a forja, o MCP e os canais abrem a
pasta deles a cada montagem de agente, e o servidor monta um agente por tarefa — duas tarefas ao
mesmo tempo são dois brokers. Isso é dedução pelo mesmo mecanismo, **não medido** nos irmãos.

## O que a medição disse

1 teste de 15 falhou, sempre na terceira abertura (registro 4). Com o cache por caminho absoluto
dentro do próprio `broker_em`, os 15 passam; o conserto entrou no motor e não no chamador novo, para
alcançar os irmãos (forja, MCP, canais, config) que chamam a mesma função na mesma ordem.

## A regra

Recurso com estado encadeado em disco (livro, cursor, corrente de hash) tem UM dono por processo:
quem abre devolve o mesmo, e o cache mora na função que abre — não em cada chamador.
