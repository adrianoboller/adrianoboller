---
name: erp-inventory
description: "Subledger de estoque de ERP: movimentações, reservas, depósitos, lotes, séries, custo médio ou PEPS, inventário."
metadata:
  short-description: Estoque de ERP rastreável e reconciliável
---

# ERP Inventory

Construa o estoque como um subledger transacional: cada saldo e custo deve ser explicável por movimentos imutáveis, reproduzível por recálculo e conciliável com contabilidade, documentos fiscais e inventário físico.

## Limites da skill

Inclua:

- entradas, saídas, devoluções, transferências, ajustes, perdas e produção quando afetarem estoque;
- saldo físico, saldo escriturado, reservas, disponibilidade, bloqueio, quarentena e trânsito;
- empresa, filial, depósito, endereço, proprietário, lote, série e validade;
- unidade-base, unidades comerciais e conversões;
- custo médio ponderado móvel ou periódico e PEPS/FIFO, conforme política aprovada;
- contagem, reconciliação, fechamento, auditoria e integração por API/eventos.

Não decida nesta skill:

- política contábil, tributária ou método de custeio sem aprovação do responsável contábil/fiscal;
- emissão, assinatura ou autorização de documento fiscal;
- razão contábil completo, contas a pagar/receber, MRP, planejamento de fábrica ou roteirização de armazém;
- tratamento de ativo imobilizado, instrumentos financeiros, ativo biológico no ponto da colheita, commodities mensuradas por exceção ou estoque público sem carregar a norma específica.

Quando esses temas aparecerem, delimite a interface e encaminhe a decisão ao domínio responsável. Não trate esta skill como parecer contábil ou fiscal.

## Entradas obrigatórias

Antes de projetar ou alterar regras, confirme o que afetar a decisão:

1. Empresa, filiais, estabelecimentos fiscais, proprietários dos bens e isolamento entre tenants.
2. Tipos de item: revenda, matéria-prima, produto em processo, acabado, embalagem, consumo, loteado, serializado ou serviço em elaboração.
3. Política de estoque negativo, reservas, validade, quarentena, corte e período fechado. Para reserva multi-depósito, confirme divisão entre depósitos, atendimento parcial, prioridade, expiração, realocação e compensação.
4. Método de valoração autorizado, escopo em que se aplica, frequência do custo médio e precisão monetária.
5. Unidade-base por item, conversões permitidas, casas decimais e regra de arredondamento.
6. Depósitos, endereços, estoque em trânsito, bens próprios em terceiros e bens de terceiros em poder da empresa.
7. Fontes de movimento, documentos de origem, eventos contábeis/fiscais e garantias de idempotência.
8. Obrigações vigentes por regime, atividade, UF e período, inclusive EFD ICMS/IPI, blocos H e K quando aplicáveis.
9. Volumetria, concorrência esperada, necessidade de rastreio, RTO/RPO e estratégia de migração, quando houver dados ou operação existente.
10. Topologia real: processos/serviços, bancos e schemas, filas, jobs, réplicas, integrações, proprietários do dado e a fronteira ACID efetivamente disponível.

Se uma política ausente puder alterar custo, saldo legal, fechamento ou escrituração, pare e solicite a decisão. Registre hipóteses não bloqueantes de modo explícito.

## Portões bloqueadores antes de mutações

Antes de alterar código, schema, configuração, contrato ou dados, faça descoberta somente leitura e satisfaça estes dois portões:

1. **Topologia e fronteira transacional:** mapeie onde cada parte de movimento, reserva, projeção, custo e integração é persistida; identifique chamadas síncronas/assíncronas, triggers, jobs e quem é a fonte autoritativa. Declare o que cabe em uma transação local e o que cruza banco, serviço ou fila. Para cada operação multi-recurso, escolha atomicidade local ou máquina de estados com compensação/reconciliação. Se a fronteira real ou os estados de falha não puderem ser provados, não faça a mutação.
2. **Reserva multi-depósito:** defina se um pedido pode ser dividido, se atendimento parcial é aceito, qual depósito/item/lote tem prioridade, como empates são resolvidos, quando a reserva expira ou é realocada e o que compensar após sucesso parcial. Defina também se a operação é tudo-ou-nada ou convergente por etapas. Se qualquer semântica afetada pelo pedido estiver indefinida, não altere reserva nem disponibilidade.

Não presuma que uma transação se estende a outro banco, serviço ou fila. Registre a decisão e os cenários de falha antes da implementação.

## Proporcionalidade e uso de N/A

Monte uma matriz curta `requisito | aplicável | justificativa | evidência alternativa` para a mudança. Aplique estes critérios:

- **Outbox/inbox:** exigidas quando há publicação assíncrona ou efeito externo que não participa da transação local, salvo padrão equivalente cuja recuperação esteja demonstrada. Em operação inteiramente local e síncrona, podem ser `N/A`; atomicidade, idempotência e rollback continuam obrigatórios.
- **Custeio:** implemente e teste o método aprovado para o escopo. Teste ambos somente se o produto oferecer ambos, houver troca/migração de método ou o pedido tocar o motor comum. Marcar o outro método `N/A` não autoriza mistura nem quebra da política versionada.
- **Migração/cutover:** obrigatórios ao alterar sistema com saldos, movimentos ou camadas existentes. Em implantação realmente nova e sem dados, podem ser `N/A`, mantendo testes de inicialização e reconciliação do saldo zero.
- **UI:** obrigatória apenas quando o pedido incluir superfície visual. Sem UI, marque `N/A` e prove consistência nos canais existentes, como domínio, API, evento ou relatório.

`N/A` exige razão verificável e não pode ser usado por conveniência. Nunca o use para eliminar uma invariante atingida pela mudança; substitua o artefato ou teste por evidência proporcional equivalente.

## Modelo mínimo

Modele, adapte ou mapeie equivalentes para:

- `Item`, `UnidadeMedida` e `ConversaoUnidade` versionada;
- `Empresa`, `Filial`, `Deposito` e `Localizacao` hierárquica;
- `Lote`, `NumeroSerie` e estados de qualidade/validade;
- `MovimentoEstoque` e suas linhas, com razão, origem, destino e documento correlato;
- `Reserva`/`Alocacao` com ciclo de vida explícito;
- `CamadaCusto` para PEPS ou estado do custo médio para o escopo definido;
- `SaldoProjetado`, sempre reconstruível a partir do ledger;
- `SessaoInventario`, contagem, recontagem, divergência e ajuste aprovado;
- `LancamentoIntegracao` e chave de idempotência; `EventoOutbox`/`Inbox` quando houver fronteira assíncrona ou externa.

Separe quantidade, propriedade, posse, status de qualidade e valor. Não represente todos esses conceitos em um único campo `saldo`.

## Fluxo de trabalho

1. Descreva casos de uso, catálogo de movimentos, estados e responsáveis. Diferencie data do fato, data de lançamento e data contábil/fiscal.
2. Defina a granularidade do saldo e do custo. A chave costuma incluir tenant, empresa/estabelecimento, item, depósito/localização e, quando aplicável, proprietário, lote, série e estado.
3. Especifique invariantes, fórmulas de disponibilidade, política de arredondamento, fechamento e comportamento de cada movimento.
4. Modele o ledger imutável e as projeções. Um movimento só afeta saldo quando `POSTADO`; rascunho, rejeição ou cancelamento não afetam quantidade.
5. Implemente a postagem na fronteira ACID comprovada: validar, controlar concorrência, gravar movimento e atualizar projeção/custo. Grave outbox na mesma transação somente quando aplicável. Use o menor agregado de bloqueio e uma ordem determinística; não bloqueie tabelas inteiras por padrão.
6. Faça reversão compensatória referenciando o original. Nunca apague ou edite silenciosamente um movimento postado.
7. Integre contabilidade e fiscal por contratos versionados e identificadores estáveis. Reprocessamento não pode duplicar movimento nem lançamento.
8. Concilie ledger, projeções, custo, inventário, documentos de origem e módulos consumidores.
9. Execute os testes aplicáveis — determinísticos, concorrentes, de propriedade, integração, migração e fechamento — e justifique cada `N/A` antes de declarar conclusão.

## Invariantes obrigatórias

| ID | Regra | Evidência esperada |
|---|---|---|
| INV-01 | Todo efeito em quantidade ou valor nasce de movimento postado; saldo projetado não é fonte única da verdade. | Saldo pode ser reconstruído e comparado ao cache. |
| INV-02 | Movimento postado é imutável; correção ocorre por estorno/reversão ou ajuste vinculado. | Cadeia original-reversão e ator/motivo preservados. |
| INV-03 | Nenhuma operação atravessa tenant ou empresa por erro de chave. Transferência intercompany são duas operações comerciais/contábeis correlacionadas, não uma transferência interna. | Testes de isolamento e autorização por empresa. |
| INV-04 | Uma transferência interna conserva quantidade e valor no escopo consolidado. Saída e entrada são atômicas ou passam por localização de trânsito conciliável. | Duas pernas correlacionadas e diferença consolidada zero, salvo perda formal. |
| INV-05 | `disponivel` tem fórmula declarada. Por padrão: disponível = físico postado − reservas duras − bloqueado/quarentena; entradas previstas não contam sem política explícita. | Mesma fórmula em todos os canais aplicáveis: domínio, API, UI, eventos, relatórios e testes. |
| INV-06 | Reserva não reduz estoque físico. Consumir, cancelar ou expirar reserva altera sua disponibilidade uma única vez. | Máquina de estados e testes de repetição. |
| INV-07 | Série identifica no máximo uma unidade disponível por item/tenant e não pode ocupar dois locais simultaneamente. | Restrição de unicidade e histórico completo. |
| INV-08 | Lote, validade, proprietário e qualidade acompanham o movimento. Item vencido ou bloqueado não é alocado sem autorização explícita e auditada. | Validações de alocação e trilha de exceção. |
| INV-09 | Cada item possui unidade-base. Fatores são positivos, versionados, imutáveis após uso e convertem por decimal exato dentro da tolerância documentada. | Testes ida-volta e histórico de vigência. |
| INV-10 | Quantidades e valores usam decimal/fixed-point, nunca ponto flutuante binário. Arredonde somente nos limites definidos e preserve precisão interna. | Casos de arredondamento e soma de parcelas. |
| INV-11 | O método de valoração é único no escopo aprovado para estoques de natureza e uso semelhantes. Não misture custo médio e PEPS no mesmo fluxo nem troque método silenciosamente. | Política versionada, vigência e ADR/aprovação. |
| INV-12 | Localização geográfica ou regra fiscal, isoladamente, não justifica método de valoração diferente. Natureza ou uso distinto exige justificativa contábil documentada. | Matriz item/uso/método revisada. |
| INV-13 | Tributos recuperáveis, descontos, frete, seguro e custos de transformação entram ou saem do custo conforme política contábil/fiscal vigente; não codifique a decisão por suposição. | Composição do custo explicável por componente. |
| INV-14 | Estoque negativo é bloqueado por padrão. Se autorizado, use custo provisório explícito, limite, alerta e recusteio/reconciliação obrigatórios. | Sem valor definitivo inventado para saída sem camada. |
| INV-15 | Período fechado não é reescrito. Fato retroativo segue política aprovada de reabertura ou ajuste no período aberto, preservando o original. | Testes de corte e trilha de aprovação. |
| INV-16 | A mesma chave de idempotência no mesmo tenant/origem produz o mesmo resultado; payload divergente com chave repetida falha. | Restrição única e testes de retry/duplicidade. |
| INV-17 | Concorrência não permite dupla baixa, dupla alocação ou saldo incoerente. | Teste simultâneo com uma unidade: apenas uma operação vence. |
| INV-18 | Ledger, projeção, camadas de custo, razão contábil e escrituração fiscal possuem identificadores correlacionáveis. | Relatório de reconciliação ponta a ponta. |
| INV-19 | Reserva multi-depósito obedece à política declarada de divisão, parcialidade, prioridade e tudo-ou-nada/convergência. Falha parcial não deixa reserva órfã nem duplica alocação. | Testes sincronizados por depósito e prova de compensação/recovery. |
| INV-20 | Atomicidade só é alegada dentro da fronteira transacional comprovada. Fora dela, estados intermediários são explícitos, idempotentes, recuperáveis e reconciliáveis. | Mapa de topologia, injeção de falhas e retomada convergente. |

## Movimentações e disponibilidade

Crie um catálogo explícito: recebimento de compra, devolução de cliente, retorno, produção, consumo, venda, devolução a fornecedor, perda, ajuste, transferência e posse de terceiros. Para cada tipo, determine:

- dimensões obrigatórias, sinal, propriedade e local de origem/destino;
- estado inicial e transições permitidas;
- autorização, documento de origem e razão do ajuste;
- efeito em físico, disponível, reserva, custo, contabilidade e fiscal;
- evento emitido e comportamento de estorno.

Não confunda:

- `on_hand` físico com disponível para promessa;
- reserva suave com alocação dura;
- propriedade com posse;
- PEPS contábil com estratégia operacional de separação; FEFO por validade pode coexistir com o método contábil aprovado;
- data de ocorrência com ordem de processamento.

Estados úteis de reserva: `SOLICITADA`, `ALOCADA`, `SEPARADA`, `CONSUMIDA`, `CANCELADA` e `EXPIRADA`. Defina transições idempotentes e proíba retorno informal a estados anteriores.

Para reserva multi-depósito, especifique antes de implementar:

- **divisão:** um pedido deve sair de um único depósito ou pode gerar várias alocações; determine máximo de divisões e restrições de empresa/filial;
- **parcialidade:** quantidade insuficiente rejeita tudo, mantém pendência/backorder ou confirma parte; a resposta deve expor reservado, faltante e estado final;
- **prioridade:** estabeleça uma ordenação estável e auditável por regra comercial, depósito, lote/FEFO, prazo e desempate; retry deve repetir a mesma decisão enquanto as entradas não mudarem;
- **concorrência:** revalide cada candidato sob lock/versão e impeça que duas reservas consumam a mesma disponibilidade;
- **compensação:** em uma fronteira ACID, falha desfaz todas as alocações. Entre fronteiras, um coordenador persiste progresso e libera somente parcelas criadas pela operação, de modo idempotente, até convergir ou abrir incidente;
- **efeitos posteriores:** cancelamento, expiração, troca de depósito, separação parcial e falha de expedição devem liberar ou realocar quantidades exatamente uma vez.

Não simule tudo-ou-nada distribuído com rollback local. Não gere movimento compensatório de estoque para reserva que nunca foi postada; compense o estado da própria reserva.

## Depósitos, endereços, lotes e séries

- Modele depósito e localização separadamente; valide capacidade, estado e compatibilidade somente quando forem requisitos reais.
- Use localização de trânsito quando expedição e recebimento da transferência não forem simultâneos.
- Preserve lote do fornecedor, lote interno, fabricação, validade e estado de inspeção sem sobrescrever histórico.
- Defina regra de seleção operacional — manual, FIFO físico ou FEFO — separada da valoração contábil.
- Em devolução, tente vincular série/lote e custo ao movimento original. Sem vínculo, aplique política documentada e marque exceção.
- Estoque próprio em terceiros e de terceiros em poder da empresa deve permanecer separado e reconciliável; não some como propriedade econômica sem base aprovada.

## Unidades e conversões

Escolha uma unidade de controle por item. Converta unidades de compra, venda, produção e inventário para essa base, mantendo a quantidade informada no documento e a convertida.

- Não encadeie conversões mutáveis; converta por fator canônico versionado.
- Não reutilize o mesmo código de unidade para volumes diferentes.
- Defina se a quantidade admite fração; série normalmente exige inteiro.
- Preserve precisão suficiente para impedir deriva acumulada e registre resíduos de arredondamento.
- Mudança de embalagem ou fator cria nova versão/código com vigência; não altera movimentos históricos.
- Quando aplicável ao SPED, valide a unidade de inventário do item e o fator do registro 0220 contra a versão oficial vigente do Guia Prático.

## Custeio

Obtenha aprovação formal do método e sua vigência. A NBC TG 16/CPC 16 orienta, em seu escopo, identificação específica para itens não intercambiáveis/projetos específicos e PEPS ou custo médio ponderado para os demais. Confirme a redação e as revisões vigentes antes de implementar.

### Custo médio ponderado

- Declare se é móvel por entrada ou periódico; não alterne sem migração aprovada.
- Na entrada, derive o novo custo da quantidade/valor remanescente mais o custo elegível da entrada. Trate valor adicional posterior como ajuste rastreável.
- Saídas usam o custo vigente na ordem temporal definida. Movimentos retroativos exigem recálculo desde o primeiro período aberto afetado ou ajuste aprovado.
- Devoluções referenciam o custo original quando a política exigir; não use automaticamente o custo corrente.

### PEPS/FIFO

- Cada entrada cria camada com quantidade remanescente, custo unitário, moeda/data e origem.
- Saída consome as camadas elegíveis mais antigas usando um desempate determinístico.
- Transferência interna carrega as camadas/custo; não cria ganho nem nova valoração.
- Retorno vinculado recompõe a origem conforme política. Sem vínculo, gere camada explicitamente classificada.
- Nunca permita quantidade negativa de camada sem política provisória e rotina comprovada de recusteio.

### Regras comuns

- Mensure e teste o menor entre custo e valor realizável líquido quando a norma aplicável exigir, incluindo redução e reversão limitada ao valor original.
- Separe custo operacional estimado de custo contábil reconhecido.
- Mudança de método requer justificativa, aprovação, data de corte, tratamento de saldos/camadas, efeitos contábeis e testes de migração.
- Gere memória de cálculo por movimento: quantidade, componentes, camadas consumidas, custo unitário, arredondamento e versão da política.

## Concorrência, idempotência e integrações

- Use transação ACID local para validar e postar movimento e atualizar projeções/custo. Se houver publicação assíncrona, grave a outbox na mesma transação; sem essa fronteira, use o mecanismo proporcional registrado na matriz de aplicabilidade.
- Controle concorrência no agregado mínimo relevante por bloqueio de linha, versão otimista ou serialização equivalente. Revalide saldo após adquirir o controle.
- Defina uma chave canônica e uma ordem global de aquisição para item, depósito, localização, lote e série. Operações multi-chave e multi-depósito devem ordenar o conjunto inteiro antes do primeiro lock.
- Trate deadlock/erro transitório com rollback integral e retry limitado, idempotente e com jitter. Não repita automaticamente insuficiência de saldo, conflito de política ou erro de validação.
- Use chave idempotente composta por tenant, sistema de origem, tipo e identificador externo. Armazene hash do comando e resposta original.
- Para consumo assíncrono, use inbox deduplicada e outbox transacional. Não prometa “exactly once” entre sistemas sem prova fim a fim.
- Eventos devem informar `event_id`, `movement_id`, tenant/empresa, versão do schema, correlação, causalidade, instante do fato e da publicação.
- Cancelamento fiscal ou contábil gera comando compensatório; não apaga o estoque já consumido por outros processos.
- Injete falhas depois de cada etapa persistente. Dentro da transação, nenhuma etapa parcial pode sobreviver; fora dela, a retomada ou compensação deve convergir sem duplicar nem perder quantidade/valor.

## Inventário físico e reconciliação

1. Defina corte por data/hora e escopo; use congelamento operacional ou snapshot com movimentos durante a contagem.
2. Gere lista cega quando apropriado e registre contador, dispositivo, horário, unidade, lote, série e local.
3. Aplique tolerâncias, recontagem e aprovação por alçada. O autor da contagem não deve autoaprovar ajuste sensível.
4. Poste divergência por movimento de ajuste com causa; nunca sobrescreva saldo.
5. Concilie quantidade e valor por item/local/lote/série, incluindo trânsito, terceiros, bloqueios e reservas.
6. Refaça a projeção a partir do ledger e compare com o cache. Divergência deve gerar incidente, não correção automática silenciosa.
7. Produza evidência de corte, contagens, recontagens, aprovações, ajustes e reconciliação final.

Para EFD ICMS/IPI aplicável, confronte o inventário do bloco H e o estoque/produção do bloco K com movimentos e documentos. O saldo do K200 deve ser derivável de estoque inicial + entradas/produção/movimentações − saídas/consumo/movimentações, conforme o guia oficial vigente.

## Auditoria e segurança

Registre em cada ação: tenant/empresa, ator ou serviço, função, razão, documento, correlação, idempotência, valores anteriores quando necessários, fato/postagem, IP/dispositivo quando autorizado e versão da regra. Proteja a trilha contra alteração e aplique retenção definida.

- Autorize por empresa, depósito, operação e limite de ajuste.
- Exija justificativa e aprovação para estoque negativo, desbloqueio, validade vencida, reabertura, recusteio e ajustes materiais.
- Monitore, quando aplicável, falhas de outbox/inbox; sempre monitore saldo negativo indevido, reserva órfã, série duplicada, lote vencido, custo ausente, divergência de projeção e integração não conciliada.

## Artefatos esperados

Produza apenas os artefatos pedidos ou necessários ao escopo, escolhendo entre:

- matriz de aplicabilidade com cada `N/A` justificado e sua evidência alternativa;
- mapa de contexto, glossário e catálogo de movimentos/estados;
- política de disponibilidade, unidades, estoque negativo, fechamento e custeio;
- modelo de dados/ERD, constraints, índices e migrações reversíveis;
- contratos OpenAPI/AsyncAPI e matriz de idempotência/erros;
- matriz de efeitos por movimento: quantidade, reserva, custo, contábil e fiscal;
- ADR da granularidade, concorrência, método de custeio e estratégia de retroatividade;
- plano de migração/cutover com saldo inicial, camadas de custo e reconciliação;
- relatório de inventário/reconciliação, trilha de auditoria e runbook de reparo;
- suíte de testes com massas numéricas reproduzíveis.

Em projeto existente, preserve convenções, identifique lacunas e faça a menor mudança segura. Não reestruture módulos fora do pedido.

## Testes mínimos aplicáveis

Automatize os casos cuja capacidade exista ou seja atingida pela mudança. Registre os demais na matriz de aplicabilidade; uma operação multi-depósito, por exemplo, não pode marcar seus testes concorrentes como `N/A`.

1. **Conservação:** sequência aleatória de movimentos mantém saldo calculado igual ao ledger; transferência conserva quantidade/valor total.
2. **Idempotência:** dez retries do mesmo comando geram um único movimento; mesma chave com payload diferente falha.
3. **Concorrência sincronizada de uma chave:** use barreira/latch para duas transações disputarem a última unidade no mesmo ponto; apenas uma baixa ou reserva vence.
4. **Multi-chave/multi-depósito sincronizado:** pause requisições concorrentes antes da aquisição, libere-as juntas e dispute conjuntos sobrepostos de itens/depósitos em ordens de entrada opostas. Verifique ordem canônica de locks, ausência de dupla alocação e resultado tudo-ou-nada ou convergente conforme a política.
5. **Deadlock e retry:** force ou simule ciclo de locks; confirme rollback integral do perdedor, retry somente do erro transitório, limite/jitter e um único efeito final pela idempotência.
6. **Falha intermediária local:** injete erro após gravar movimento e antes de projeção, custo ou outbox. Confirme que a transação desfaz tudo e que retry produz exatamente um resultado completo.
7. **Falha entre fronteiras:** interrompa depois da primeira parcela/depósito ou antes/depois da publicação. Confirme recuperação pelo coordenador, retomada ou compensação idempotente, nenhuma reserva órfã e reconciliação final.
8. **Reservas multi-depósito:** cubra divisão permitida e proibida, atendimento total e parcial, falta, prioridade/desempate, expiração, realocação, cancelamento e compensação. Reservar reduz disponível, não físico.
9. **Unidades:** conversão ida-volta respeita tolerância; fator zero, negativo, incompatível ou alterado retroativamente falha.
10. **Série/lote:** série duplicada ou em dois locais falha; lote vencido/bloqueado exige exceção autorizada.
11. **Médio, quando aplicável:** 10 unidades a 10 + 10 a 14 resultam em média 12; saída de 5 custa 60 e deixa valor 180, conforme precisão definida.
12. **PEPS, quando aplicável:** 10 a 10 + 10 a 14; saída de 12 custa 128 e deixa camada de 8 a 14, valor 112.
13. **Mudança/migração de custeio, quando aplicável:** data de corte, saldos/camadas, histórico, estornos e recálculo permanecem explicáveis antes e depois.
14. **Devolução/estorno:** vínculo original preserva quantidade, custo, lote/série e trilha sem apagar o fato.
15. **Retroatividade:** lançamento em período fechado é rejeitado ou vira ajuste conforme política; recálculo em período aberto é determinístico.
16. **Inventário:** snapshot, movimentos durante contagem, recontagem e ajuste fecham na quantidade aprovada.
17. **Isolamento:** consulta ou comando de uma empresa não lê nem altera outra.
18. **Integração assíncrona, quando aplicável:** falha antes/depois do commit e da publicação é recuperada por outbox/inbox ou padrão equivalente; consumidores deduplicam.
19. **Reconciliação:** projeção reconstruída, camadas/custo e módulos contábil/fiscal aplicáveis fecham por identificador e total.
20. **Carga e recuperação:** volume esperado, timeout, reinício e replay não corrompem saldo, reserva nem custo.

Se o sistema oferecer custo médio e PEPS, execute as duas massas e testes do motor compartilhado. Se somente um método pertencer ao escopo, teste-o profundamente e justifique o outro como `N/A`.

Prefira testes de propriedade para conservação, não negatividade, unicidade e reversibilidade; mantenha exemplos contábeis como testes dourados aprovados pelo responsável.

## Critérios de conclusão

Considere a entrega pronta somente quando:

- todos os movimentos têm semântica, autorização, estorno e efeitos documentados;
- saldo e disponibilidade são determinísticos e iguais em todos os canais aplicáveis — domínio, API, UI, eventos, relatório e recálculo;
- retries e concorrência não duplicam nem ultrapassam estoque/reserva;
- unidade, lote, série, validade, propriedade e localização são rastreáveis de ponta a ponta;
- cada método de custo aplicável está aprovado, isolado por escopo, explicável e reconciliado, sem mistura silenciosa;
- inventário não sobrescreve histórico e fecha com ajustes aprovados;
- períodos fechados permanecem imutáveis;
- contabilidade, fiscal e integrações aplicáveis conciliam por movimento e total;
- testes, métricas, alertas, rollback e runbook aplicáveis foram executados com evidência recente; migração/cutover foi comprovada quando existiam dados ou operação anterior;
- todo `N/A` tem justificativa verificável e não remove evidência de invariante afetada;
- dúvidas normativas pendentes estão registradas e aprovadas antes da produção.

## Fontes oficiais

Consulte novamente antes de qualquer decisão normativa e registre versão, vigência, UF/regime e data de acesso:

- [CFC — Normas Brasileiras de Contabilidade completas](https://cfc.org.br/tecnica/normas-brasileiras-de-contabilidade/normas-completas/): fonte para a NBC TG 16 e revisões em vigor.
- [CFC — NBC TG 16 (R2), Estoques](https://www1.cfc.org.br/sisweb/SRE/docs/NBCTG16%28R2%29.pdf): mensuração, composição do custo, critérios de valoração, valor realizável líquido, resultado e divulgação. A página oficial do CFC registra alterações posteriores; não trate o PDF isolado como prova de vigência integral.
- [CFC — Revisão NBC 31](https://www2.cfc.org.br/sisweb/sre/detalhes_sre.aspx?codigo=2025%2FREVIS%C3%83ONBC31): publicada em 25/02/2026 e indicada pelo CFC como alteração, entre outras, da NBC TG 16.
- [CPC — CPC 16 (R1), Estoques](https://www.cpc.org.br/CPC/Documentos-Emitidos/Pronunciamentos/Pronunciamento?Id=47): pronunciamento correlato à IAS 2 e aprovações regulatórias.
- [SPED — Guia Prático EFD-ICMS/IPI 3.2.3](https://www.gov.br/sped/pt-br/assuntos/escrituracoes-digitais/efd-icms-ipi/manuais-e-documentos-tecnicos/guia-pratico-efd-versao-3-2-3.pdf/%40%40display-file/file): versão consultada, atualizada em 06/05/2026; contém unidades/conversões (registro 0220), inventário (bloco H) e produção/estoque (bloco K). Verifique se há versão posterior no portal oficial.

Não copie regras fiscais de blogs, respostas informais ou memória do modelo. Use atos, manuais e documentação oficial vigentes e submeta interpretações materiais ao responsável contábil/fiscal.
