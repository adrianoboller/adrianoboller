---
name: erp-integration-reliability
description: "Integrações de ERP confiáveis: APIs, webhooks, filas, eventos, lotes, idempotência, retry, DLQ e reconciliação."
origem: erp-skills-pack (pesquisa em skills.sh, 2026-09-04); descricao encurtada de proposito
---

# Confiabilidade de integrações de ERP

Trate a integração como um protocolo distribuído sujeito a duplicação, atraso, reordenação, indisponibilidade, resposta perdida e execução parcial. Preserve o efeito de negócio correto, torne falhas recuperáveis e deixe evidência suficiente para operar e reconciliar.

## Escopo e exclusões

Esta skill cobre integrações síncronas e assíncronas por API, webhook, mensageria, eventos, arquivos e lotes; produtores, consumidores, adaptadores e rotinas de reconciliação; e mudanças ou revisões em integrações existentes.

Não:

- defina regras funcionais, contábeis, fiscais, de estoque ou de estorno sem o responsável do domínio;
- escolha produto de fila, banco, protocolo ou garantia de entrega sem conhecer o ambiente;
- afirme “exactly once” com base apenas no broker; descreva as garantias observáveis em cada fronteira e projete efeitos idempotentes;
- faça reprocessamento, correção de dados ou *redrive* em produção sem autorização, simulação e plano de reversão;
- esconda inconsistência com retries infinitos, descarte silencioso ou atualização manual sem trilha.

## Entradas obrigatórias

Antes de propor a solução, obtenha:

1. Processo de negócio, comando/evento, fonte de verdade, efeito esperado e invariantes do domínio.
2. Produtor, consumidor, intermediários, proprietários, ambientes e fronteiras transacionais.
3. Contrato atual, exemplos válidos/inválidos, identificadores de negócio, versão e política de compatibilidade.
4. Semântica disponível de entrega, confirmação, visibilidade, ordenação, retenção, replay e DLQ do transporte real.
5. Volume normal e de pico, tamanho das mensagens, concorrência, latência/SLO, RPO/RTO e janela máxima de replay ou reenvio.
6. Classes de erro conhecidas, limites dos provedores, timeouts, `Retry-After` quando existir e dependências externas.
7. Requisitos de segurança, privacidade, auditoria, segregação por empresa/*tenant* e retenção.
8. Operação existente: métricas, alertas, runbooks, incidentes, reconciliação e procedimento de reprocessamento.

Se uma garantia depender de recurso do broker, banco ou provedor, confirme-a na documentação da versão implantada. Marque suposições e seu impacto; não as apresente como fatos.

Escolha mecanismos proporcionais ao risco e à topologia. Para idempotência, outbox, inbox, fila, retries, DLQ, circuit breaker, ordenação, saga e reconciliação, marque `APLICÁVEL`, `NÃO APLICÁVEL` ou `PENDENTE` com justificativa. Não imponha infraestrutura distribuída a uma operação local atômica nem omita um controle necessário apenas para simplificar o desenho.

## Fluxo de trabalho

### 1. Delimitar a operação distribuída

- Classifique cada interação como comando, evento, consulta, notificação ou transferência em lote.
- Identifique onde começa e termina cada transação local e onde há chamada remota ou confirmação assíncrona.
- Defina fonte de verdade, estado terminal, efeitos externos e quem pode decidir correção ou compensação.
- Modele pelo menos: sucesso, rejeição funcional, falha transitória, falha permanente, timeout, resposta perdida, duplicação, reordenação, indisponibilidade prolongada e recuperação.
- Expresse invariantes mensuráveis, por exemplo “um pedido aceito gera no máximo um título financeiro com o mesmo identificador de negócio”, sem inventar a regra do domínio.

### 2. Definir identidade e idempotência

- Use identificador estável por intenção de negócio, reutilizado em todas as tentativas. Não gere uma nova chave a cada retry.
- Delimite o namespace da chave por operação, origem e empresa/*tenant*; evite colisões entre tipos de comando.
- Associe a chave a uma impressão do conteúdo relevante. Mesma chave e mesmo conteúdo devem reproduzir o resultado; mesma chave com conteúdo incompatível deve gerar conflito explícito.
- Faça a reserva da chave e o efeito local na mesma transação quando possível, usando restrição de unicidade como defesa contra concorrência.
- Armazene estado (`em processamento`, `concluído`, `falhou`), resposta ou referência ao resultado e datas suficientes para retomar após queda.
- Dimensione a retenção da deduplicação para superar a maior janela real de retry, replay, DLQ e reenvio manual. Não expire a prova antes de o evento poder reaparecer.
- Se o efeito estiver fora da transação local, use máquina de estados, idempotência no destino e reconciliação; não confunda “solicitação registrada” com “efeito concluído”.

### 3. Usar outbox e inbox nas fronteiras corretas

Para publicar mudança confirmada no ERP:

- grave o estado de negócio e o registro de outbox atomicamente na mesma transação local;
- atribua ID de evento imutável, tipo, versão, agregado, sequência quando necessária, data e payload ou referência durável;
- mantenha um ciclo explícito `pending` → `claim` → `published` ou `error` (ou estados equivalentes documentados): a criação atômica começa em `pending`; o relay faz `claim` com proprietário e validade; a confirmação do transporte leva a `published`; e a falha registra `error`, tentativa, próximo horário e diagnóstico sanitizado;
- publique por processo retomável, com *lease/claim* seguro para concorrência, recuperação de claim expirado e métricas de atraso;
- marque `published` somente após confirmação do transporte. Considere que uma queda entre confirmação e marcação pode duplicar a publicação; o consumidor continua responsável por deduplicar;
- defina quando `error` volta a `pending`, quando se torna terminal e quem pode reprocessá-lo, preservando histórico e limite de tentativas.
- defina retenção, arquivamento e purga da outbox; só remova um registro após prova de publicação e depois das janelas necessárias de replay, deduplicação, auditoria e reconciliação.

Para consumir:

- registre o ID no inbox e aplique o efeito local na mesma transação sempre que possível;
- confirme a mensagem somente depois do commit;
- ao reencontrar evento concluído, não reaplique o efeito; devolva ou registre o resultado idempotente;
- diferencie evento em processamento abandonado de evento concluído e defina recuperação de *leases* vencidos.
- use transições condicionais e token de *fencing* ou mecanismo equivalente para impedir que um worker atrasado confirme depois que seu *lease* expirou e outro worker retomou o item.

Outbox evita o *dual write* entre banco e publicação; inbox evita repetição de efeito local. Nenhum deles, isoladamente, torna idempotente uma chamada externa.

### 4. Configurar filas, retries e DLQ

- Classifique erros em transitórios, permanentes, funcionais e desconhecidos. Faça retry automático apenas quando a operação for segura e houver chance plausível de recuperação.
- Defina timeout por tentativa, prazo total, máximo de tentativas e orçamento de retry. O prazo interno deve caber no orçamento do chamador.
- Use atraso exponencial limitado com *jitter* e respeite orientação explícita do destino. Evite retries sincronizados e amplificação em cascata.
- Aplique backpressure, limite de concorrência e cotas por destino/*tenant* para proteger ERP, banco e fornecedor.
- Envie mensagens venenosas ou esgotadas para DLQ/quarentena com motivo, tentativa, versão, correlação e referência segura ao payload.
- Trate DLQ como fila de trabalho operacional, não como descarte. Defina alerta, proprietário, triagem, correção, expiração e SLO.
- Proteja DLQ/quarentena com autenticação, menor privilégio, segregação por ambiente e empresa/*tenant*, criptografia adequada, auditoria e redação de segredos/dados desnecessários. Defina retenção e descarte coerentes com o payload original e obrigações aplicáveis.
- Modele a falha ao encaminhar para a DLQ: publicar na DLQ e confirmar/remover da origem são outro *dual write*. Prefira dead-lettering atômico oferecido e comprovado pelo transporte; caso não exista, use registro durável/outbox ou protocolo recuperável. Nunca confirme a origem antes de haver prova durável do encaminhamento; deduplicate a DLQ porque a queda após o encaminhamento e antes da confirmação pode duplicá-la.
- Exija autorização e pré-visualização para *redrive*. Preserve ID original, contagem e trilha; não altere silenciosamente a mensagem apenas para fazê-la passar.

### 5. Conter falhas síncronas

- Configure timeouts de conexão e operação; não dependa de padrões implícitos ou espera ilimitada.
- Use circuit breaker por dependência/operação quando falhas repetidas causariam pressão inútil. Defina abertura, janela, recuperação e comportamento em estado semiaberto com dados reais.
- Isole recursos por *bulkhead* e limite filas internas para impedir que uma integração esgote threads, conexões ou memória das demais.
- Defina fallback somente se preservar a regra de negócio. “Aceitar agora e sincronizar depois” exige estado pendente explícito, fila durável e reconciliação.
- Propague cancelamento e prazo quando suportados; não inicie trabalho que já não pode terminar dentro do SLO.

### 6. Tratar ordenação, deduplicação e eventos atrasados

- Exija ordenação apenas no menor escopo necessário, normalmente por agregado ou entidade de negócio; ordenação global reduz paralelismo e raramente é requisito real.
- Escolha chave de partição coerente com esse escopo e documente efeitos de reparticionamento.
- Inclua versão ou sequência monotônica quando o domínio exigir ordem. Defina comportamento para duplicata, sequência antiga, lacuna e evento futuro.
- Não avance silenciosamente sobre uma lacuna que possa invalidar saldo ou estado. Estacione, busque estado autoritativo ou acione reconciliação conforme a regra aprovada.
- Para mensagem venenosa que bloqueia uma partição ordenada, tome uma decisão explícita e auditável: manter a partição bloqueada enquanto corrige; colocar em quarentena e avançar somente com aprovação do domínio e prova de que a ordem pode ser relaxada; ou reconstruir o estado a partir da fonte autoritativa. Não pule a mensagem silenciosamente.
- Torne atualizações condicionais à versão quando houver risco de evento atrasado sobrescrever estado recente.
- Preserve tombstones/cancelamentos durante toda a janela em que mensagens antigas possam reaparecer.

### 7. Modelar sagas e compensações

- Use saga quando uma operação envolver múltiplas transações locais sem commit atômico comum.
- Registre instância, versão, passo, tentativa, resultado, prazo e estado terminal. Faça passos e compensações idempotentes.
- Escolha orquestração ou coreografia conforme observabilidade, acoplamento e governança do ambiente; documente a decisão.
- Compensação é uma nova ação de negócio, não rollback mágico. Em financeiro ou estoque, pode exigir reversão auditável em vez de exclusão.
- Defina quais falhas usam recuperação progressiva, compensação, espera ou intervenção humana. Não compense automaticamente quando o efeito for irreversível ou a regra não estiver aprovada.
- Evite manter transação de banco aberta durante chamada remota.

### 8. Reconciliar fontes de verdade

- Crie rotina periódica e executável sob demanda que compare identificadores, versões, valores e estados entre sistemas.
- Defina fonte autoritativa por campo/estado, janela de estabilização, tolerâncias, paginação, retomada e tratamento de exclusões.
- Classifique divergências: ausente, duplicado, valor diferente, estado impossível, atraso aceitável ou órfão.
- Gere relatório e fila de correção com evidência. Faça reparo em modo simulação primeiro e exija aprovação para alterações de alto impacto.
- Torne correções idempotentes e registre antes/depois, regra, executor, data e correlação.
- Não use amostragem como única garantia para processos financeiros, fiscais ou de estoque quando a totalidade for requisito do domínio.

### 9. Governar contratos e versões

- Mantenha contrato legível por máquina quando adequado e exemplos de sucesso e erro; inclua nomes, tipos, obrigatoriedade, limites, códigos, semântica de ausente/nulo, moeda/precisão, data/fuso, IDs e regras de repetição.
- Separe versão de envelope e payload quando necessário. Declare compatibilidade para adicionar, remover ou reinterpretar campos e eventos.
- Valide produtores e consumidores em CI com testes de contrato e versões reais. Teste consumidor antigo com produtor novo e o inverso durante a janela suportada.
- Faça leitores tolerarem extensões compatíveis sem ignorar violação de invariantes. Rejeite versão não suportada de modo observável.
- Planeje depreciação com inventário de consumidores, telemetria de uso, prazo e estratégia de rollback.
- Para webhooks, documente autenticação, integridade, proteção contra replay, política de reenvio e resposta esperada.

### 10. Tornar o sistema observável e operável

- Propague `trace_id`, correlação, causação, ID de evento e ID de saga; proteja ou resuma chaves que contenham informação sensível.
- Produza logs estruturados sem segredos ou dados pessoais desnecessários e com transições de estado auditáveis.
- Meça taxa de sucesso/erro, latência, tentativas, duplicatas, mensagens em processamento, lag e idade da mais antiga, DLQ, circuit breaker, reconciliações e correções.
- Alerte por impacto e tendência, não apenas por exceção isolada. Vincule alertas a runbooks com diagnóstico, contenção, replay seguro e escalonamento.
- Disponibilize visão por integração, versão, destino e empresa/*tenant*, sem criar cardinalidade de métricas inviável.

## Invariantes de implementação

- Um mesmo comando/evento não pode produzir o mesmo efeito de negócio duas vezes.
- Estado confirmado e outbox são gravados atomicamente; inbox e efeito local também, quando compartilham o banco.
- A mensagem só é confirmada após o efeito durável ou registro inequívoco de estado pendente.
- Todo retry é limitado, observável e seguro; erros permanentes não ficam em ciclo.
- DLQ, replay e correção preservam identidade, histórico e autorização.
- O encaminhamento à DLQ não cria perda silenciosa no *dual write*, não mistura ambientes/*tenants* e respeita retenção e acesso aprovados.
- Retenção de deduplicação cobre toda a janela de reaparição.
- Ordem é preservada ou recuperada no escopo exigido pelo domínio; lacunas não são ocultadas.
- Toda compensação é autorizada, idempotente e auditável.
- Reconciliação detecta perdas e divergências que a entrega normal não detecta.
- Telemetria e mensagens não expõem segredos nem dados pessoais além do necessário.

## Artefatos da entrega

Entregue, proporcionalmente ao escopo:

1. contexto, fonte de verdade, fronteiras transacionais, SLOs e suposições;
2. diagrama de sequência/estados incluindo falhas e recuperação;
3. contrato versionado e matriz de compatibilidade;
4. desenho de chave idempotente, outbox, inbox, retenção e restrições de unicidade;
5. tabela de erros com timeout, retry, backoff/jitter, limite, DLQ e proprietário;
6. definição de ordenação, saga/compensação e reconciliação;
7. código, migrações e configuração sem segredos;
8. dashboards, alertas, runbook e procedimento de *redrive*;
9. plano de implantação, compatibilidade, rollback e critérios de parada;
10. relatório de testes e riscos residuais.

Inclua no desenho uma matriz de aplicabilidade dos mecanismos. Todo `NÃO APLICÁVEL` deve indicar a fronteira e o motivo verificável.

## Testes de falha obrigatórios

Teste em ambiente controlado, conforme aplicável:

- mesma mensagem antes, durante e depois da primeira conclusão; mesma chave com payload diferente; duas execuções concorrentes;
- commit concluído com resposta perdida e queda em cada janela entre estado, outbox, publicação, inbox, efeito e confirmação;
- duplicação, reordenação, lacuna, atraso além da janela normal e replay de mensagem antiga;
- indisponibilidade, lentidão, timeout, limitação de taxa, resposta inválida e recuperação do destino;
- esgotamento de retries, mensagem venenosa, ida à DLQ, correção e *redrive* múltiplo;
- queda antes e depois de gravar na DLQ e antes de confirmar a origem, comprovando recuperação/deduplicação; isolamento, acesso e expiração da DLQ;
- mensagem venenosa em partição ordenada, validando a política aprovada de bloqueio, avanço ou reconstrução;
- circuit breaker aberto/semiaberto, backpressure e isolamento sob pico;
- falha em cada passo e compensação de saga, inclusive compensação que também falha;
- divergência injetada para provar detecção, relatório e correção idempotente da reconciliação;
- produtor/consumidor nas versões suportadas e rejeição observável de versão incompatível;
- carga e retenção suficientes para validar throughput, lag, armazenamento de deduplicação e SLOs.

## Critérios de aceite

Considere a integração pronta somente quando:

- os invariantes de negócio aprovados passam nos testes de duplicação, concorrência e falha parcial;
- dentro das fronteiras, garantias, carga, condições de falha e janela efetivamente testadas, os eventos injetados e confirmados foram recuperados conforme o contrato e as duplicatas injetadas não repetiram efeito;
- retries, DLQ, circuit breaker, ordenação e retenções possuem valores justificados por medições e limites reais;
- reconciliação encontra as divergências injetadas e o reparo autorizado é reproduzível e auditável;
- contratos permanecem compatíveis na matriz acordada;
- dashboards e alertas detectam atraso, erro, DLQ e inconsistência dentro dos SLOs definidos;
- runbook e *redrive* foram exercitados sem corrupção ou perda de rastreabilidade;
- limites de carga, riscos residuais, responsáveis e plano de rollback estão documentados.

Não extrapole esses resultados para afirmar ausência absoluta de perda ou duplicação. Declare com precisão as fronteiras, hipóteses, garantias do transporte/banco, período, volume e cenários cobertos, além do risco residual fora deles. Mecanismos não aplicáveis devem estar marcados `NÃO APLICÁVEL` com justificativa aceita.

Se o usuário não forneceu metas quantitativas, proponha valores para validação e identifique-os como hipótese; não os transforme em aceite implícito.
