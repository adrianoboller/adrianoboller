---
name: erp-multi-company
description: "Isolamento multiempresa, multifilial e multiestabelecimento em ERP: contexto ativo, intercompany, autorização por escopo."
origem: erp-skills-pack (pesquisa em skills.sh, 2026-09-04); descricao encurtada de proposito
---

# ERP multiempresa

Construa isolamento verificável entre organizações sem impedir compartilhamentos de dados que tenham sido autorizados de forma explícita. Trate vazamento entre empresas como falha crítica de segurança e integridade.

## Resultado esperado

Entregue uma solução em que:

- grupo econômico, tenant, empresa, filial e estabelecimento tenham significados documentados e identificadores estáveis;
- cada operação seja executada em um contexto ativo validado no servidor;
- dados, permissões e artefatos derivados permaneçam dentro do escopo autorizado;
- cadastros compartilhados e locais tenham propriedade, precedência e governança definidas;
- operações intercompany gerem lançamentos correlacionados e reconciliáveis;
- testes negativos demonstrem que um contexto não lê nem altera recursos de outro.

## Escopo

Inclua quando aplicável:

- grupos econômicos, tenants, empresas jurídicas, filiais, unidades gerenciais e estabelecimentos fiscais;
- seleção, troca, propagação e expiração do contexto ativo;
- isolamento em banco, cache, busca, filas, eventos, arquivos, logs, relatórios, exportações, backups e telemetria;
- cadastros globais, por grupo, por empresa, por filial ou por estabelecimento;
- RBAC com escopo e ABAC para condições dependentes do recurso e do contexto;
- operações entre empresas, inclusive pares a pagar/receber, transferências e conciliação;
- trilha de auditoria e testes de autorização horizontal e vertical.

## Fora do escopo

Não invente:

- enquadramento societário, tributário ou contábil;
- regras legais para documentos fiscais, preços de transferência ou consolidação;
- uma equivalência automática entre `tenant`, grupo econômico e empresa;
- uma arquitetura de banco única para todos os projetos;
- compartilhamento de dados apenas porque entidades pertencem ao mesmo grupo.

Encaminhe regras fiscais para a skill fiscal, lançamentos para a skill contábil e entrega confiável de eventos para a skill de integração. Registre dependências sem duplicar essas regras aqui.

## Entradas necessárias

Antes de projetar, obtenha ou marque como pendente:

- hierarquia organizacional e significado legal/operacional de cada nível;
- relação entre tenant e empresa, inclusive se um usuário pode acessar vários tenants;
- matriz de usuários, papéis, atributos e escopos permitidos;
- classificação de cada agregado ou cadastro por proprietário e nível de compartilhamento;
- matriz de escopo por recurso, indicando quais dimensões (`tenant_id`, `company_id`, `branch_id`, `establishment_id` ou outras) são obrigatórias em leitura, escrita e relacionamento;
- operações intercompany previstas e sistemas participantes;
- requisitos de residência, retenção, criptografia, auditoria, exportação e restauração;
- banco, cache, broker, mecanismo de busca, armazenamento de arquivos e ferramentas de relatório;
- volumes, concorrência, disponibilidade e estratégia de migração do legado.

Não esconda lacunas com valores presumidos. Separe requisito confirmado, hipótese e decisão proposta.

## Modelo organizacional

Defina os conceitos no vocabulário do projeto. Como ponto de partida, avalie:

| Conceito | Responsabilidade típica | Regra de modelagem |
|---|---|---|
| Tenant | Fronteira de segurança, contrato ou implantação | Não o confunda com pessoa jurídica sem decisão explícita |
| Grupo econômico | Relação ou consolidação entre empresas | Não concede acesso automaticamente |
| Empresa | Pessoa jurídica ou entidade contábil | Possui identificadores legais e políticas próprias |
| Filial/unidade | Estrutura administrativa ou operacional | Pode ou não coincidir com estabelecimento fiscal |
| Estabelecimento | Unidade relevante para operação fiscal/local | Modele conforme requisito oficial aplicável |

Use IDs internos imutáveis. Trate CNPJ, códigos humanos e nomes como atributos alteráveis ou chaves de negócio, não como substitutos universais da identidade técnica.

## Invariantes obrigatórios

1. Negue por padrão. Autorize no servidor cada ação sobre cada recurso, inclusive leitura, pesquisa, exportação e download.
2. Não aceite `tenant_id`, `company_id`, `branch_id` ou papel enviado pelo cliente como prova de autorização.
3. Faça cada registro privado apontar de forma não nula para todas as dimensões exigidas por sua matriz de escopo. A obrigatoriedade de empresa, filial ou estabelecimento não pode depender de convenção informal.
4. Impeça referências cruzadas inválidas com chaves únicas e estrangeiras compostas pelo escopo, não apenas com filtros da aplicação.
5. Falhe de modo fechado quando o contexto estiver ausente, inválido, expirado ou ambíguo.
6. Preserve o contexto em transações, jobs, eventos e callbacks; revalide-o no consumidor.
7. Nunca reutilize cache, arquivo temporário, conexão contextualizada ou relatório entre escopos sem limpeza e nova validação.
8. Toda exceção administrativa ou acesso transversal deve ser explícita, mínima, temporária, justificada e auditada.
9. Uma alteração de vínculo ou permissão deve invalidar sessões, caches de autorização e trabalhos ainda não autorizados, conforme a política definida.
10. Compartilhamento deve ser opt-in, possuir proprietário e permitir saber quem pode ler, alterar e revogar.
11. Duas empresas dentro do mesmo tenant continuam sendo fronteiras distintas para todo recurso classificado por empresa; compartilhar `tenant_id` nunca autoriza omitir ou ignorar `company_id`.

## Contexto ativo

Modele um contexto mínimo com:

- identidade autenticada e identidade efetiva, quando houver representação autorizada;
- `tenant_id`, `company_id` e, se aplicável, `branch_id`/`establishment_id`;
- permissões e atributos relevantes, obtidos de fonte confiável;
- instante de emissão/expiração, origem e `correlation_id`;
- finalidade ou modo especial, como consolidação ou suporte emergencial.

Derive o contexto de sessão ou token validado e confirme que os vínculos continuam ativos. A troca de empresa deve ser uma ação explícita, autorizada e auditada. Não mantenha simultaneamente dois contextos implícitos na mesma unidade de trabalho. Operações transversais devem declarar a lista exata de escopos e aplicar autorização a cada item.

## Matriz de escopo por recurso

Crie uma matriz normativa antes de implementar filtros. Para cada agregado, tabela, endpoint, evento, relatório, cache e arquivo, registre:

- dimensão proprietária e dimensões obrigatórias: tenant, grupo, empresa, filial, estabelecimento ou combinação delas;
- se leitura, criação, alteração, exclusão e relacionamento usam o mesmo escopo;
- se o recurso é local, herdado, compartilhado por referência ou transversal por autorização explícita;
- chaves, FKs, índices, políticas e verificações que materializam a decisão;
- comportamento quando uma dimensão obrigatória estiver ausente, que deve ser falha fechada;
- exceções, responsável, vigência e teste que prova a exceção.

Não classifique automaticamente todos os recursos apenas por tenant. Se duas empresas A e B pertencerem ao mesmo tenant, um recurso classificado por empresa deve exigir `company_id` em identidade, consulta, unicidade, relacionamento, cache, evento e autorização. Uma consulta com apenas `tenant_id` deve falhar ou ficar restrita a um caso transversal explicitamente autorizado pela matriz.

## Estratégia de isolamento

Escolha a topologia com um ADR, considerando risco, custo operacional, volume, restauração seletiva e exigências contratuais:

- banco por tenant ou empresa: maior separação e restauração independente, com maior custo operacional;
- schema por tenant: separação lógica forte, porém com migrações e pools mais complexos;
- tabelas compartilhadas com coluna de escopo: operação mais simples, mas exige controles consistentes em todas as consultas;
- híbrida: adequada quando classes de clientes ou dados exigem níveis distintos de isolamento.

Não escolha apenas por conveniência do ORM. Documente ameaças, falhas esperadas e mecanismo de restauração por escopo.

### Banco de dados

- Inclua o escopo nos índices, unicidades e relacionamentos. Exemplo conceitual: uma numeração única por empresa exige unicidade de `(tenant_id, company_id, numero)`, não de `numero` global.
- Centralize a aplicação do contexto e proíba consultas sem escopo nas rotas comuns.
- Restrinja credenciais da aplicação pelo menor privilégio; contas de manutenção não devem servir requisições comuns.
- Avalie RLS como defesa adicional quando o banco oferecer suporte. Ela não substitui autorização de negócio.
- No PostgreSQL, se usar RLS, habilite e avalie `FORCE ROW LEVEL SECURITY`, use papel de aplicação que não seja proprietário nem tenha `BYPASSRLS`, configure contexto com duração transacional (`SET LOCAL`) e limpe/encerre a transação antes de devolver a conexão ao pool.
- Teste separadamente operações de importação, `COPY`, migração, backup, restauração, replicação e rotinas executadas com privilégios elevados.

### Cache e busca

- Inclua ambiente, tenant, empresa e demais dimensões necessárias nas chaves e tags de invalidação.
- Não armazene respostas autorizadas apenas por ID de recurso; o mesmo ID pode existir ou ser solicitado em outro contexto.
- Propague o escopo para índices de busca, filtros de consulta, sugestões, contagens e snippets.
- Invalide o cache ao remover vínculos, alterar papéis ou mudar a visibilidade de um cadastro.

### Filas, eventos e jobs

- Transporte identificadores de escopo, versão do contrato, autor do comando, `correlation_id` e chave de idempotência.
- Trate o envelope como dado não confiável até validar o vínculo entre escopo, recurso e operação.
- Não use um consumidor privilegiado para ignorar a autorização que existia na origem.
- Separe ou particione quando necessário filas, DLQs, métricas e reprocessamentos; reprocessar não pode trocar o escopo.
- Jobs agendados devem enumerar escopos autorizados e abrir uma unidade de trabalho isolada para cada um.

### Arquivos, relatórios e observabilidade

- Namespaceie caminhos, buckets, objetos e temporários por escopo. Valide a autorização antes de emitir URL temporária e limite validade e operação permitida.
- Aplique o mesmo filtro a consulta, totalizadores, drill-down, impressão, e-mail e exportação.
- Não permita que logs, traces, nomes de métricas ou mensagens de erro exponham dados sensíveis. Registre IDs de escopo suficientes para investigação e restrinja o acesso aos logs.
- Verifique isolamento em cubos, data lakes, réplicas de leitura, snapshots e arquivos de backup. Defina como restaurar uma empresa sem sobrescrever outra.

## Cadastros compartilhados e locais

Para cada cadastro, registre:

- proprietário: plataforma, tenant, grupo, empresa ou estabelecimento;
- leitores, editores e aprovadores;
- estratégia: global imutável, compartilhado por referência, cópia controlada, herança com override ou exclusivamente local;
- precedência entre valor herdado e local;
- vigência, versionamento, desativação, deduplicação e impacto da revogação;
- campos compartilháveis e campos que devem permanecer locais.

Não use um booleano genérico `global`. Uma matriz de propriedade evita que preço, limite de crédito, condição fiscal ou dado pessoal seja compartilhado por engano. Ao copiar um cadastro, registre origem e momento da cópia; não crie sincronização bidirecional implícita.

## RBAC, ABAC e acesso transversal

- Escopo o vínculo do papel: `aprovador` na empresa A não implica `aprovador` na empresa B.
- Use RBAC para responsabilidades estáveis e ABAC para atributos como empresa, estabelecimento, centro de custo, propriedade do recurso, valor e estado.
- Avalie sujeito, ação, recurso e contexto em cada solicitação. Não autorize apenas a rota ou o botão.
- Modele explicitamente auditores, consolidação corporativa, suporte e contas de serviço.
- Para acesso emergencial, exija motivo, prazo, autorização apropriada e revisão posterior; nunca conceda acesso oculto e permanente.

## Intercompany

Modele uma operação intercompany como uma correlação entre efeitos pertencentes a entidades distintas:

- atribua um ID imutável comum e mantenha IDs locais em cada empresa;
- preserve empresa emissora, empresa destinatária, moeda, datas, status e referências;
- gere pares ou pernas determinísticos e reconciliáveis; não altere silenciosamente apenas um lado;
- defina quando cada lado pode confirmar, rejeitar, cancelar, estornar e reconciliar;
- aplique idempotência para impedir duplicação por retry;
- use transação atômica quando os efeitos estiverem na mesma fronteira transacional; entre sistemas ou bancos, use outbox/eventos e estados intermediários recuperáveis;
- registre diferenças, compensações e intervenção manual sem apagar o histórico.

Solicite validação especializada para tributos, câmbio, contabilização e preço de transferência.

## Ramo inicial: vazamento ativo ou suspeito

Antes do fluxo normal, verifique se há indício de acesso entre escopos, exportação indevida, cache contaminado ou propagação incorreta. Se houver:

1. Acione o plano de resposta a incidente e contenha o vetor sem apagar, sobrescrever ou reorganizar evidências.
2. Preserve logs, eventos, snapshots, configurações, horários e cadeia de custódia conforme o plano; não execute limpeza destrutiva para “corrigir” o sintoma.
3. Revogue ou restrinja sessões, credenciais, permissões, URLs temporárias e caches afetados conforme a autoridade e o plano aprovados. Evite revogações globais indiscriminadas quando ampliarem o dano operacional sem necessidade.
4. Determine tenants, empresas, pessoas, recursos, canais e intervalo temporal potencialmente atingidos; diferencie alcance confirmado de alcance ainda sob investigação.
5. Corrija e valide primeiro em ambiente controlado. Preserve uma forma segura de rollback e não reabra o acesso antes dos testes negativos relevantes.
6. Se dados pessoais estiverem ou puderem estar envolvidos, acione `erp-lgpd` e os responsáveis jurídico, privacidade e segurança para avaliar obrigações e comunicações. Não faça conclusão jurídica ou notificação externa sem essa avaliação.
7. Registre decisões, responsáveis, evidências e horário de cada ação. Depois da contenção, execute análise de causa, reconciliação e prevenção de recorrência.

## Fluxo de trabalho

1. Levante a hierarquia e diferencie fronteira jurídica, operacional e de segurança.
2. Classifique agregados e cadastros por proprietário, visibilidade e dimensões obrigatórias na matriz de escopo.
3. Escolha a topologia de isolamento e documente a decisão e seus riscos.
4. Defina o contexto ativo, como ele nasce, troca, propaga, expira e é revogado.
5. Modele chaves, relacionamentos e autorização para impedir referências entre escopos.
6. Aplique o contexto a banco, cache, busca, filas, arquivos, relatórios e observabilidade.
7. Modele compartilhamentos e operações intercompany com estados recuperáveis.
8. Produza a matriz RBAC/ABAC e valide segregação e acessos transversais.
9. Implemente telemetria e auditoria sem registrar segredos ou dados pessoais desnecessários.
10. Execute testes de vazamento, migração, concorrência, falha e restauração antes da liberação.

Em legado, inventarie consultas, tabelas e integrações sem escopo. Faça backfill validado, imponha restrições gradualmente e mantenha rollback. Não habilite compartilhamento ou RLS em produção sem ensaio com dados representativos e contas de privilégio real.

## Artefatos

Crie ou atualize somente os artefatos aplicáveis, adaptando os caminhos ao repositório. Marque como `N/A` cada item omitido e registre uma justificativa verificável; “não implementado” ou “não solicitado” não bastam quando o risco existe. Artefatos de controles que protegem uma fronteira realmente presente não podem ser dispensados sem decisão e aceite de risco explícitos.

- glossário e diagrama da hierarquia organizacional;
- matriz de propriedade e compartilhamento de dados;
- matriz de escopo por recurso e operação;
- matriz RBAC/ABAC por ação, recurso e escopo;
- ADR da estratégia de isolamento e do uso ou não de RLS;
- modelo de contexto e contrato de propagação para eventos/jobs;
- esquema, constraints, migrações e políticas RLS, quando escolhidas;
- especificação de intercompany e reconciliação;
- modelo de ameaças com caminhos de vazamento;
- plano de migração, rollback, restauração seletiva e resposta a incidente;
- suíte automatizada de testes de isolamento e relatório de evidências.

Cada artefato aplicável deve distinguir decisão, hipótese, pendência, responsável e critério de aceite. Dimensione o detalhe conforme número de fronteiras, canais e impacto, sem criar documentos vazios para cumprir checklist.

## Testes obrigatórios

Selecione os testes proporcionalmente às fronteiras e canais realmente existentes. Para cada caso abaixo não aplicável, registre `N/A` e a razão técnica; não omita um canal existente nem aceite `N/A` apenas porque ainda não há teste preparado. Cubra ao menos os itens aplicáveis:

- leitura, alteração, exclusão e enumeração horizontal com ID de outro tenant/empresa;
- duas empresas do mesmo tenant, comprovando que `tenant_id` correto com `company_id` ausente ou de outra empresa falha para recursos classificados por empresa;
- ausência e troca indevida de cada dimensão obrigatória definida na matriz, inclusive filial e estabelecimento;
- escalada vertical e papel válido usado fora de seu escopo;
- contexto ausente, adulterado, expirado, revogado ou trocado durante a sessão;
- FKs e unicidades tentando relacionar entidades de escopos diferentes;
- reutilização de conexão do pool após `commit`, `rollback`, erro e timeout;
- chaves e invalidação de cache, índice de busca, autocomplete e agregações;
- fila, retry, DLQ, reprocessamento e job concorrente com escopos distintos;
- relatório, drill-down, impressão, exportação e URL temporária de arquivo;
- acesso de auditor, suporte, conta de serviço, proprietário de tabela e superusuário controlado;
- cadastros herdados, override local, revogação e cópia;
- intercompany duplicada, parcial, fora de ordem, cancelada e reconciliada;
- backup e restauração seletiva sem contaminar outros escopos;
- testes generativos ou parametrizados que troquem aleatoriamente sujeito, recurso e escopo.

Teste o resultado e o efeito colateral: resposta vazia não basta se logs, contagens, tempo, cache ou eventos revelarem a existência do recurso.

## Critérios de conclusão

Considere a tarefa concluída somente quando:

- a fronteira de tenant e a hierarquia organizacional estiverem documentadas sem ambiguidades conhecidas;
- a matriz de escopo definir dimensões obrigatórias por recurso e impedir que empresas do mesmo tenant percam sua fronteira;
- todo agregado privado possuir escopo e integridade referencial compatível;
- todos os canais síncronos e assíncronos aplicarem autorização e isolamento;
- compartilhamentos estiverem explícitos, governados e revogáveis;
- intercompany puder ser rastreada, repetida com segurança e reconciliada;
- acessos transversais forem mínimos, temporários quando possível e auditáveis;
- testes negativos demonstrarem ausência de vazamento entre empresas, inclusive em cache, busca, arquivos e relatórios;
- migração, rollback e restauração tiverem evidência executável;
- nenhuma alegação de segurança depender apenas da interface ou de revisão manual.

## Referências oficiais de segurança

- [OWASP — Multi-Tenant Security Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Multi_Tenant_Security_Cheat_Sheet.html)
- [OWASP — Authorization Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Authorization_Cheat_Sheet.html)
- [PostgreSQL — Row Security Policies](https://www.postgresql.org/docs/current/ddl-rowsecurity.html)
- [PostgreSQL — CREATE POLICY](https://www.postgresql.org/docs/current/sql-createpolicy.html)
- [NIST SP 800-207 — Zero Trust Architecture](https://csrc.nist.gov/pubs/sp/800/207/final)
