---
name: erp-accounting
description: "Núcleo contábil de ERP: razão, partidas dobradas, plano de contas, competência, conciliação, estorno, fechamento, multimoeda."
origem: erp-skills-pack (pesquisa em skills.sh, 2026-09-04); descricao encurtada de proposito
---

# ERP Accounting

## Objetivo

Entregar um núcleo contábil íntegro, rastreável, reprocessável e conciliável. Trate o razão como registro financeiro controlado, não como uma coleção de saldos editáveis.

Esta skill orienta engenharia de software. Ela não presta consultoria contábil, jurídica, tributária ou de auditoria e não substitui a aprovação de políticas e lançamentos por profissional da contabilidade legalmente habilitado.

## Escopo

Inclua quando solicitado:

- diário, razão, balancete e saldos por período;
- motor de contabilização por partidas dobradas;
- plano de contas e dimensões gerenciais;
- reconhecimento por competência e visão de caixa;
- integrações dos subrazões com o razão geral;
- conciliação bancária e contábil;
- estorno, transferência, complementação e lançamento extemporâneo;
- períodos, fechamento, reabertura e transporte de saldos;
- moeda funcional, moedas de transação, conversão e arredondamento;
- trilha de auditoria, controles de acesso e evidências;
- migração de saldos e lançamentos contábeis.

## Fora do escopo

Não assuma responsabilidade por:

- emissão ou autorização de documentos fiscais, SPED, cálculo de tributos ou obrigações acessórias;
- regras detalhadas de estoque, custo, folha, faturamento, tesouraria ou ativos fixos; esses módulos apenas originam eventos para contabilização;
- definição autônoma de políticas contábeis, materialidade, contas, taxas de câmbio, vida útil, provisões ou critérios de reconhecimento;
- assinatura, autenticação ou transmissão oficial de livros e demonstrações;
- lançamento direto em produção sem autorização explícita.

Encaminhe regras fiscais para uma skill fiscal, regras de estoque para uma skill de inventário e isolamento societário para uma skill multiempresa. Mantenha nesta skill os contratos contábeis dessas integrações.

## Regra normativa

Antes de codificar comportamento dependente de norma:

1. Identifique país, tipo e porte da entidade, finalidade do relatório, regulador setorial e estrutura contábil aplicável.
2. Consulte a versão vigente nas fontes oficiais do CFC e do CPC; confira revisões, erratas, vigência e eventual revogação.
3. Registre no artefato de decisão a URL, o documento, a revisão, a data da consulta e a interpretação aprovada pelo responsável contábil.
4. Modele regras voláteis com vigência, versão e configuração; não enterre critérios normativos em condicionais espalhadas pelo código.
5. Diante de conflito, lacuna ou interpretação ambígua, exponha a dúvida e solicite decisão do contador responsável. Não invente a regra.

Considere as referências ao final como pontos de partida, não como cópia congelada da norma. Normas, revisões e orientações podem mudar.

## Entradas necessárias

Obtenha ou marque explicitamente como pendente:

- entidades, filiais, livros e razão afetados;
- estrutura contábil e regulador aplicáveis;
- período fiscal, calendário, fuso horário e política de corte;
- moeda funcional, moeda de apresentação e moedas de transação;
- escala decimal e política de arredondamento por moeda e tipo de operação;
- plano de contas atual, histórico de versões, natureza e finalidade de cada conta;
- dimensões obrigatórias: centro de custo, projeto, unidade, contrato, parceiro ou outras;
- catálogo de eventos dos módulos de origem e respectiva chave idempotente;
- matriz evento -> débito/crédito e suas condições de vigência;
- políticas de reconhecimento, liquidação, câmbio, retificação e fechamento;
- bancos, contas, formatos de extrato e regras de conciliação;
- papéis, alçadas, segregação de funções e aprovações;
- relatórios, integrações, volumes, requisitos de desempenho e retenção;
- fonte, qualidade e data de corte dos dados a migrar.
- sistema-alvo, repositório ou conector autorizado, stack, banco e versões;
- fronteira transacional real, baseline de build/testes e ambiente de execução;
- autorização explícita para qualquer mutação, especialmente em produção.

Se uma entrada essencial faltar, produza uma lista objetiva de decisões pendentes. Use dados fictícios em protótipos e testes; não transforme suposições em regra de produção.

Para efetivar ou corrigir lançamentos, entradas técnicas, contexto contábil, documento/evento original, período, moedas/taxas, regra aprovada e autorização são bloqueadores. Sem sistema-alvo e autorização identificados, entregue somente especificação, proposta ou revisão; não afirme que contabilização ou estorno foi executado.

## Fluxo de trabalho

### 1. Delimitar o contexto contábil

- Diferencie razão geral, subrazões, contabilidade gerencial, visão de caixa e relatórios estatutários.
- Desenhe a fronteira de cada módulo. O módulo de origem mantém o fato operacional; o razão mantém o efeito contábil e a referência à origem.
- Defina a unidade de contabilização: entidade, livro, período, lote, lançamento e linha.
- Registre decisões relevantes em ADRs e políticas versionadas.

### 2. Modelar eventos antes de telas

- Catalogue eventos econômicos, estados, origem, momento de reconhecimento e cancelamento.
- Para cada evento, documente pré-condições, contas, dimensões, moeda, histórico e regra de reversão.
- Use uma chave idempotente estável para a identidade do fato econômico, como `source_system + entity_id + event_type + source_event_id`; nunca inclua a versão da regra nessa identidade.
- Guarde `posting_rule_version` como atributo do processamento e do lançamento. Repetir o mesmo evento depois de alterar a regra deve recuperar o resultado original, não gerar outro lançamento.
- Se uma nova versão precisar corrigir eventos já contabilizados, use fluxo explícito de reprocessamento/retificação que referencie o lançamento anterior e possua identidade própria; não transforme replay em novo fato econômico.
- Separe evento recebido, proposta de contabilização, lançamento validado e lançamento efetivado.
- Não faça a tela ou o relatório decidir débitos e créditos.

### 3. Projetar o modelo persistente

Separe, no mínimo:

- cabeçalho do lançamento;
- linhas de débito/crédito;
- documento ou evento de origem;
- plano de contas versionado;
- dimensões contábeis;
- períodos e estados de fechamento;
- lotes de contabilização;
- taxas de câmbio, par de moedas, direção/convenção, histórico, relacionamento entre versões e procedência;
- vínculos de retificação e reversão;
- itens e vínculos de conciliação;
- aprovações e trilha de auditoria.

Use chaves técnicas imutáveis. Códigos exibidos ao usuário podem mudar, mas não devem romper referências históricas. Defina restrições no banco e no domínio para as invariantes críticas; validação apenas na interface não basta.

### 4. Implementar o motor de contabilização

- Transforme eventos de origem em propostas determinísticas de lançamento.
- Valide vigência da regra, período, contas, dimensões, moeda e equilíbrio antes da efetivação.
- Grave lançamento, linhas, chave idempotente e auditoria em uma única transação atômica.
- Publique efeitos posteriores somente após o commit, preferencialmente por outbox transacional.
- Em retry, retorne o resultado já produzido para a mesma chave; não duplique o lançamento.
- Torne a regra de contabilização versionada e reproduzível para explicar como qualquer lançamento foi formado.

### 5. Conciliar e fechar

- Importe evidências externas sem destruir o conteúdo original.
- Proponha correspondências; exija aprovação para exceções e lançamentos criados por regra automática conforme a alçada definida.
- Execute checklist de subrazões, reconciliações, cortes, ajustes, balancete e exceções antes do fechamento.
- Gere evidência do fechamento e bloqueie alterações retroativas conforme o estado do período.

### 6. Verificar e entregar

- Execute testes de invariantes, integração, concorrência, migração e relatórios.
- Reconcilie subrazões, razão e demonstrativos com conjuntos de dados conhecidos.
- Entregue evidências reproduzíveis, decisões pendentes e riscos residuais.
- Obtenha aceite técnico e validação do contador responsável antes de declarar prontidão contábil.

## Invariantes obrigatórias

### Partidas dobradas

- Todo lançamento efetivado possui cabeçalho, identificador unívoco e pelo menos duas linhas válidas.
- Cada linha representa débito ou crédito, nunca ambos; valores são não negativos e linhas sem efeito são rejeitadas.
- A soma dos débitos é igual à soma dos créditos na moeda funcional, após a política de arredondamento aplicável.
- O equilíbrio é validado por lançamento e livro; lotes equilibrados não podem mascarar lançamentos individuais desequilibrados.
- A efetivação é atômica: cabeçalho, linhas e auditoria persistem juntos ou nada persiste.
- Um lançamento efetivado é imutável. Correções geram novos registros vinculados ao original.
- Cada lançamento guarda entidade, livro, datas relevantes, histórico que expresse a essência econômica, origem, evidência, usuário/processo e versão da regra.
- A mesma chave idempotente não pode efetivar dois lançamentos.
- Alterar a versão da regra de contabilização não altera a identidade idempotente do evento que já foi processado.

Não armazene somente o saldo. Calcule ou materialize saldos a partir de movimentos íntegros, com mecanismo verificável de reconstrução.

### Plano de contas

- Versione o plano e use intervalos de vigência sem sobreposição inválida.
- Diferencie contas sintéticas de contas analíticas e permita lançamento apenas nas contas configuradas como lançáveis.
- Impeça ciclos, pais inexistentes e hierarquias quebradas.
- Registre natureza, classe, finalidade, moeda ou restrições dimensionais necessárias, sem presumir um modelo universal.
- A ITG 2000 não impõe um plano único; o detalhamento deve refletir complexidade, usuários e requisitos aplicáveis. Não invente uma taxonomia genérica como obrigação normativa.
- Não reutilize identificador interno de conta encerrada para outro significado.
- Alterações de código, descrição ou agrupamento preservam a leitura histórica.
- Conta inativa continua consultável, mas não recebe novos lançamentos fora de exceção autorizada e auditada.
- Mapeamentos para demonstrativos e eventos possuem vigência e responsável pela aprovação.

### Competência e caixa

- Para escrituração contábil sujeita às NBC gerais, trate competência como base do reconhecimento, conforme confirmação do responsável contábil e da norma vigente.
- Mantenha separadas: data do fato econômico, data contábil, data de efetivação, competência e data de liquidação.
- Gere visão de caixa e fluxo de caixa a partir de recebimentos e pagamentos vinculados; não substitua silenciosamente a data de reconhecimento pela data financeira.
- Apropriações, diferimentos, provisões, liquidações e reversões precisam de evento, política, competência e rastreabilidade próprios.
- Se outro regime for legalmente aplicável a relatório específico, mantenha-o identificado em livro ou visão separada e exija fundamento e aprovação. Não misture bases no mesmo saldo sem identificação.
- Teste cortes de mês e ano, pagamentos antecipados, recebimentos parciais e liquidações posteriores.

### Conciliação

- Preserve arquivo, hash, conta, período e identificador externo do extrato importado; impeça importação duplicada.
- Suporte correspondência um-para-um, um-para-muitos e muitos-para-um quando autorizada.
- Valor, data, descrição e tolerância de correspondência são regras explícitas, versionadas e testadas.
- Um item não pode ser consumido acima de seu valor disponível nem conciliado duas vezes.
- Diferenças, tarifas e juros não geram ajustes ocultos; criam proposta de lançamento com regra e aprovação identificáveis.
- Permita estados não conciliado, parcialmente conciliado, conciliado e desfeito, com histórico completo.
- Desfazer conciliação não apaga lançamento nem evidência; registra motivo, ator e vínculo com a ação anterior.
- O total conciliado, o saldo contábil e o saldo externo devem ser demonstráveis por data de corte.

### Retificações e estornos

- Nunca exclua nem sobrescreva lançamento efetivado.
- Implemente as formas previstas na escrituração aplicável: estorno, transferência e complementação, sempre como novos lançamentos.
- Estorno total replica os valores em sentido inverso e referencia o lançamento original.
- Transferência corrige a conta indevida preservando a explicação e o vínculo com a origem.
- Complementação aumenta ou reduz o efeito anterior sem esconder o valor originalmente registrado.
- Guarde tipo, motivo, data, usuário, aprovador, lançamento original e evidência.
- Lançamento extemporâneo informa a data efetiva do fato e a justificativa do atraso, sem falsificar a cronologia do registro.
- Um lançamento já totalmente estornado não pode ser estornado novamente sem fluxo de exceção explícito.
- Estorno parcial mantém saldo reversível por lançamento, linha, componente e moeda. A soma das reversões não pode exceder o original; valide e atualize esse limite atomicamente para impedir que comandos concorrentes o ultrapassem.

#### Gate para pedidos de estorno operacional

Quando o pedido for “estornar venda”, “cancelar compra” ou equivalente, pare antes de tratá-lo como um simples lançamento inverso e classifique os efeitos envolvidos:

- razão contábil;
- contas a receber ou a pagar e caixa;
- estoque, reserva, lote, série, expedição ou devolução;
- documento fiscal, autorização, evento fiscal e obrigação acessória;
- comissões, contratos, crédito, fidelidade e outros domínios afetados.

Esta skill pode estornar somente o efeito no razão. Ela não cancela documento fiscal, desfaz recebimento, executa reembolso, reabre título, devolve item ao estoque nem revoga outros efeitos operacionais. Em resultado isolado, declare precisamente “efeito contábil estornado”; nunca alegue “venda estornada integralmente”.

Para estorno integral, exija coordenação explícita entre os domínios responsáveis, com identificador de correlação, idempotência por operação, ordem/compensações definidas, estados parciais observáveis e reconciliação final. Não prometa atomicidade distribuída sem mecanismo que realmente a garanta. Registre por domínio: solicitado, concluído, falhou, compensado ou pendente de ação manual.

### Fechamento

- Controle períodos por entidade e livro, com estados explícitos como aberto, em fechamento, fechado e reaberto.
- Bloqueie contabilização em período fechado no serviço e no banco quando tecnicamente possível; não dependa da tela.
- Exija checklist configurável: subrazões contabilizados, exceções tratadas, conciliações, ajustes, câmbio, balancete e aprovações.
- Gere instantâneo ou hash verificável do balancete, usuário, horário, regras e pendências aceitas no momento do fechamento.
- Reabertura requer permissão segregada, justificativa, aprovação e auditoria. Ela não remove a evidência do fechamento anterior.
- Lançamentos de encerramento e transporte de saldos usam contas e políticas aprovadas, são reproduzíveis e não alteram o histórico.
- Defina explicitamente o efeito de períodos reabertos sobre relatórios já emitidos.

### Moedas e arredondamento

- Diferencie moeda da transação, moeda funcional e, quando aplicável, moeda de apresentação.
- Nunca armazene “uma taxa” sem semântica. Guarde moeda-base, moeda-cotada, direção explícita — por exemplo, `1 BASE = x COTADA` —, convenção de multiplicar/dividir, valor, tipo, fonte, data/hora de observação, início/fim de vigência e valor funcional resultante.
- Preserve o histórico imutável das taxas usadas e o relacionamento entre taxa original, correção, substituição e inversão. Um lançamento deve apontar para a taxa exata aplicada, não apenas para a cotação atualmente vigente.
- Valide que o par e a direção convertem da moeda de origem para a moeda de destino esperada. Não trate automaticamente uma taxa como sua inversa; quando a inversão for permitida, registre sua derivação, precisão e arredondamento.
- Use decimal exato ou inteiros em unidade mínima com escala explícita; nunca use ponto flutuante binário para valores contábeis.
- Centralize política de escala, arredondamento e tolerância, com versão e vigência. Não presuma duas casas para toda moeda ou operação.
- Defina a granularidade do cálculo e arredondamento — item, componente, linha ou total —, distribuição de resíduos, desempate e limite autorizado; não envie toda diferença para uma conta de arredondamento por tolerância implícita.
- Após conversão, o lançamento continua equilibrado na moeda funcional.
- Diferença de arredondamento usa linha e conta aprovadas; não é descartada nem distribuída silenciosamente.
- Remensuração, conversão e reconhecimento de variação cambial seguem política aprovada e norma vigente; taxas históricas, médias e de fechamento não são intercambiáveis por conveniência técnica.
- Classifique, por conta e componente, quais saldos são monetários e elegíveis à remensuração segundo política aprovada; não aplique a mesma taxa indistintamente a recebível, receita, tributo e custo.
- Reprocessar com a mesma regra e taxa registrada deve produzir o mesmo resultado.

### Auditoria e acesso

- Registre criação, validação, aprovação, efetivação, retificação, conciliação, fechamento e reabertura.
- A auditoria identifica ator humano ou serviço, instante, ação, antes/depois aplicável, origem, correlação e motivo.
- Separe, quando exigido, quem configura regras, aprova, efetiva, concilia e reabre período.
- Usuário não altera entidade, livro ou dimensão para contornar autorização.
- Dados contábeis e evidências respeitam retenção, privacidade e acesso mínimo, definidos pela organização e pela legislação aplicável.

## Artefatos esperados

Produza somente os artefatos necessários ao escopo e ao risco da tarefa. Em implementação completa do núcleo, considere todos os itens abaixo; em revisão pontual, correção ou protótipo, entregue o subconjunto relevante e marque os demais como `N/A` com justificativa, sem criar arquivos vazios. Adapte nomes ao repositório, mantendo o conteúdo equivalente quando aplicável:

- escopo contábil e matriz de responsabilidades;
- glossário e modelo de domínio;
- diagrama de dados e dicionário de campos;
- plano de contas versionado e regras de validação;
- catálogo de eventos e matriz evento -> lançamento;
- especificação do motor de contabilização e idempotência;
- política de moedas, taxas e arredondamento;
- regras e fluxo de conciliação;
- calendário, checklist de fechamento e política de reabertura;
- matriz de papéis, alçadas e segregação de funções;
- contratos de API/eventos e tratamento de erros;
- migrações de banco, índices e restrições;
- plano de migração, reconciliação e rollback;
- cenários de teste, massas conhecidas e evidências de execução;
- registro das fontes oficiais consultadas, versões e aprovações;
- manual operacional curto para contabilização, conciliação, fechamento e recuperação de falhas.

## Testes mínimos proporcionais ao escopo

Execute as categorias que alcancem o comportamento alterado e suas invariantes. Marque uma categoria como `N/A` apenas com justificativa verificável — por exemplo, tarefa exclusivamente documental sem código executável — e registre a evidência alternativa usada. Uma alteração no motor, persistência, fechamento ou moeda não pode dispensar seus testes críticos apenas por ser pequena.

### Unidade e propriedades

- Gere lançamentos de múltiplas linhas e prove sempre `total_debitos = total_creditos`.
- Rejeite linha com débito e crédito simultâneos, conta não lançável, dimensão obrigatória ausente e período inválido.
- Teste valores mínimos, máximos, negativos, zero, escalas variadas e limites de precisão.
- Prove determinismo da mesma entrada, regra e taxa.
- Processe o mesmo evento antes e depois de trocar a versão da regra; confirme que o replay retorna o lançamento original e não duplica o fato econômico. Teste separadamente o fluxo autorizado de reprocessamento/retificação entre versões.
- Prove que retificações mantêm o original e formam o efeito líquido esperado.

### Integração e concorrência

- Simule falha entre cabeçalho, linhas e auditoria; confirme rollback total.
- Envie o mesmo evento simultaneamente; confirme uma única contabilização.
- Envie estornos parciais concorrentes, com chaves distintas, contra o mesmo original; confirme que o total reverso nunca excede o saldo reversível e que uma segunda reversão total falha.
- Teste fechamento concorrente com efetivação e reabertura concorrente.
- Confirme bloqueios e isolamento entre entidades, livros e períodos.
- Reconstrua saldos a partir dos lançamentos e compare com materializações.

### Competência, caixa e corte

- Cubra reconhecimento e liquidação em períodos diferentes.
- Cubra antecipação, parcelamento, liquidação parcial, apropriação e lançamento extemporâneo.
- Compare razão por competência, posição financeira e visão de caixa sem dupla contagem.

### Conciliação

- Cubra duplicidade de extrato, correspondências 1:1, 1:N e N:1, tolerâncias, parcialidade e desfazimento.
- Demonstre saldo inicial + movimentos = saldo final para extrato e razão.
- Confirme que diferenças exigem tratamento explícito.

### Moedas

- Cubra moeda sem casas decimais, escalas superiores, taxas inversas, datas ausentes e arredondamento de várias linhas.
- Cubra pares em ambas as direções, convenções de multiplicar/dividir, tentativa de usar par incompatível e consulta histórica após substituição da taxa.
- Confirme equilíbrio funcional, preservação do valor original e contabilização explícita das diferenças.
- Teste remensuração e reversão com taxas controladas por fixtures aprovadas.
- Use fixtures com oráculos em moeda original e funcional para reconhecimento, remensuração, liquidação e reversão; confirme que apenas contas/componentes elegíveis são remensurados.
- Em venda com mercadoria, prove que a taxa comercial não altera o custo informado pelo subledger de estoque e que o resíduo do estorno segue a política do lançamento relacionado.

### Estorno interdomínios

- Para cenário de venda, confirme que o comando contábil não altera recebível/caixa, estoque nem documento fiscal por efeito colateral.
- Simule sucesso contábil e falha em outro domínio; confirme estado parcial observável, retry idempotente e reconciliação, sem declaração falsa de estorno integral.

### Fechamento e auditoria

- Confirme que período fechado recusa lançamentos comuns por API, job e acesso direto suportado.
- Confirme reabertura autorizada, rejeição de usuário sem alçada e preservação das evidências.
- Rastreie uma linha do relatório até o lançamento, evento e documento de origem, e no sentido inverso.

### Migração e relatórios

- Reconcilie saldos de abertura, débitos, créditos e saldos finais por conta, entidade e período.
- Compare balancete e amostras de diário/razão com a origem.
- Registre diferenças, decisão, responsável e aceite; não ajuste dados silenciosamente para “fechar”.

## Critérios de conclusão

Avalie estes critérios somente dentro do escopo contratado. Itens fora do escopo podem ser `N/A` se houver justificativa e aceite; invariantes tocadas pela mudança não podem ser dispensadas. Considere o trabalho concluído somente quando os critérios aplicáveis estiverem atendidos:

- escopo, estrutura contábil, políticas e responsáveis estão documentados;
- contador habilitado aprovou plano de contas, matriz de contabilização e políticas dependentes de julgamento;
- invariantes críticas são impostas pelo domínio e por restrições transacionais adequadas;
- nenhum valor monetário usa ponto flutuante binário;
- lançamentos efetivados são imutáveis, balanceados, idempotentes e rastreáveis à origem;
- retificações, conciliações, fechamento e reabertura deixam trilha completa;
- competência e caixa não são confundidos e os cortes foram testados;
- conversões preservam valor original, par/direção, convenção, taxa exata, histórico, fonte e resultado funcional;
- pedidos de estorno operacional tiveram seus domínios classificados e o resultado não amplia indevidamente o alcance do estorno contábil;
- subrazões, razão, balancete e relatórios conciliam nas massas de aceite;
- migração possui totais de controle, relatório de diferenças e plano de reversão;
- testes automatizados e testes de aceite foram executados com evidência recente;
- fontes oficiais foram verificadas na data da entrega e decisões normativas estão versionadas;
- riscos residuais e pendências estão explícitos, sem alegação indevida de conformidade profissional.

## Referências oficiais brasileiras

Consulte sempre a versão vigente. Verificação inicial destas páginas: 2026-09-04.

- [CFC — Normas Brasileiras de Contabilidade](https://cfc.org.br/tecnica/normas-brasileiras-de-contabilidade/): índice oficial, revisões, erratas e vigência.
- [CFC — Normas específicas](https://cfc.org.br/tecnica/normas-brasileiras-de-contabilidade/normas-especificas/): catálogo que aponta a ITG 2000 (R1) e demais interpretações de escrituração.
- [CFC — ITG 2000 (R1), Escrituração Contábil](https://www1.cfc.org.br/sisweb/SRE/docs/ITG2000%28R1%29.pdf): formalidades, conteúdo mínimo, documentação e formas de retificação.
- [CFC — Regime de caixa e de competência](https://cfc.org.br/tecnica/perguntas-frequentes/regime-de-caixa-e-de-competencia/): orientação oficial sobre a base de competência.
- [CFC — Normas simplificadas para PMEs](https://cfc.org.br/tecnica/normas-brasileiras-de-contabilidade/normas-simplificadas-para-pmes/): identifique a norma aplicável conforme porte e enquadramento, sem presumir uma única estrutura.
- [CPC — Pronunciamentos emitidos](https://www.cpc.org.br/CPC/Documentos-Emitidos/Pronunciamentos): índice oficial e indicação de documentos revogados.
- [CPC 00 (R2) — Estrutura Conceitual para Relatório Financeiro](https://www.cpc.org.br/CPC/Documentos-Emitidos/Pronunciamentos/Pronunciamento?Id=80): fundamentos de reconhecimento, mensuração e apresentação.
- [CPC 02 (R2) — Efeitos das mudanças nas taxas de câmbio e conversão](https://www.cpc.org.br/CPC/Documentos-Emitidos/Pronunciamentos/Pronunciamento?Id=9): referência para moeda funcional, transações em moeda estrangeira e conversão.
- [CPC 03 (R2) — Demonstração dos Fluxos de Caixa](https://www.cpc.org.br/CPC/Documentos-Emitidos/Pronunciamentos/Pronunciamento?Id=34): referência para classificação e apresentação de fluxos de caixa.
- [CPC 23 — Políticas Contábeis, Mudança de Estimativa e Retificação de Erro](https://www.cpc.org.br/CPC/Documentos-Emitidos/Pronunciamentos/Pronunciamento?Id=54): diferencie alteração de política, estimativa e correção de erro.
- [CPC 26 (R1) — Apresentação das Demonstrações Contábeis](https://www.cpc.org.br/CPC/Documentos-Emitidos/Pronunciamentos/Pronunciamento?Id=57): apresentação, competência, consistência, comparabilidade e materialidade.

Não copie requisitos de memória nem use blogs como autoridade normativa. Para setores regulados, consulte também o órgão oficial competente e registre a prevalência definida pelo responsável contábil.
