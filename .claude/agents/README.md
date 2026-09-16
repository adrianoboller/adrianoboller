# Agentes e subagentes do PhxSql

Os **dez papéis pétreos** (`phxsql/CLAUDE.md`) são a lei. Estes arquivos os
tornam **invocáveis** — cada um é um agente que o orquestrador convoca quando o
trabalho toca o domínio dele. A cláusula não obriga abrir dez por tarefa;
obriga que **nenhum papel fique sem dono quando o trabalho toca o domínio
dele**, e que a dispensa seja registrada.

## O que virou agente, e o que não

| Papel | Agente | Por quê |
|---|---|---|
| A — Orquestrador | *(esta sessão)* | é quem convoca; não se delega a si mesmo |
| B — Engenheiro | `engenheiro` | implementa e responde pelos portões |
| C — DBA | `dba` | formato em disco, chave, índice, integridade, migração |
| D — Zelador | *(script `phxsql/zelador.sh`)* | roda por script, não por subagente; **não mata processo** |
| E — Designer | `designer` | tela; interface só se prova exercitando |
| F — Prova real | `prova-real` | o teste FALHA com o defeito reposto e passa com o conserto |
| G — QA | `qa` | catracas e catálogo de guardas; catraca só desce |
| H — Documentação | `documentacao` | número visível sai de gerador, nunca de memória |
| H — Documentação | `tradutor` | agente multilíngua pétreo (GOV-1, pedido #110): lote coerente na `FABRICA_TELA`, catraca `TETO_ROTULOS_E_CRASE` sempre baixando |
| I — Versionador/Backup | *(scripts `backup.sh`; integrador comita)* | pacote por script, nunca à mão |
| J — Pesquisador | `pesquisador` | traz o que os outros fazem, **medido contra o nosso gargalo** |

E os **acréscimos medidos** do comparativo com o Phoenix Cast
(`PHOENIX_EQUIPE.xlsx`, 16/09/2026):

| Novo | Agente | Lacuna que fecha |
|---|---|---|
| SEC | `seguranca` | revisor adversário de segurança — o único gap real; antes era ad hoc |
| RES-subagentes | `pesquisa-motor`, `pesquisa-bancada` | o J deixa de ser um só e coordena subagentes por domínio |
| RES-rede | `pesquisa-rede` | transporte P2P, gossip e anti-entropia — o pilar do e-mail P2P é domínio novo (16/09/2026) |

**Recusado por escopo, não por mérito:** a camada de domínio do Phoenix Cast
(conectores, redes sociais, anúncios, marketplaces, atribuição, React) serve a
um **SaaS de marketing** — o PhxSql é o motor embaixo desse produto, não o
produto. Reabre quando esse produto for construído.

## Os três pilares e a cobertura de papéis — 16/09/2026

O produto passou a ser três (`phxsql/docs/VISAO.md`): a **base de dados**, um
**e-mail P2P** e uma **blockchain de mini-contratos sigilosos**. A pergunta «faltam
agentes?» tem resposta medida: **nenhum papel novo falta** — os dez cobrem os
três pilares. O que os pilares novos abrem é **profundidade de domínio**, e ela
entra como **subagente**, não como papel: `pesquisa-rede` para o transporte P2P;
e o sigilo de contrato é contrato de `seguranca` + `pesquisa-motor` (cripto e
norma) com o `dba` sobre o formato da cadeia. Abrir papel de topo para isso seria
o exagero que a cláusula avisa; o dono de cada domínio já existe.

## Modelo por agente: o NÍVEL, nunca o nome

Esta casa é proibida de gravar identificador de modelo em artefato versionado.
Por isso **nenhum arquivo aqui traz o campo `model:`** — o agente herda o
modelo, e o orquestrador aplica o **tier** na hora de convocar (na conversa,
não no arquivo). O tier de cada papel e o motivo estão em
`phxsql/docs/MODELOS.md`: projeto e risco (motor, formato, cripto, concorrência,
segurança) no modelo forte; mecânico e verificável (varredura, medição
roteirizada, tradução) no mais leve que ainda faça direito.

## As ferramentas por agente não são enfeite

Revisor não escreve: `dba`, `qa`, `seguranca` e os `pesquisa-*` têm só leitura
e busca — um revisor que pode editar deixa de ser revisor. `prova-real`,
`engenheiro`, `designer`, `documentacao` e `tradutor` escrevem, porque o
entregável deles é código, teste, tela ou documento. **Só o integrador
comita**, por caminho explícito — nenhum agente empurra para o `origin`
sozinho.
