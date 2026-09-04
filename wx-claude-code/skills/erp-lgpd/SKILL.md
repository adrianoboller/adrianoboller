---
name: erp-lgpd
description: "Privacidade e LGPD em ERP: dados pessoais, retenção, direitos do titular, terceiros, incidentes e evidências técnicas."
origem: erp-skills-pack (pesquisa em skills.sh, 2026-09-04); descricao encurtada de proposito
---

# Engenharia de privacidade LGPD para ERP

Conduza a análise como trabalho de engenharia de privacidade e *privacy by design*. Produza controles implementáveis, riscos rastreáveis e evidências verificáveis. Não declare que o ERP ou a organização está “em conformidade” apenas porque uma lista foi preenchida.

## Limites obrigatórios

- Não substitua o jurídico, o encarregado/DPO, a governança de dados nem a autoridade competente.
- Não invente finalidade, hipótese legal, prazo de retenção, obrigação de comunicação, mecanismo de transferência internacional ou papel de agente de tratamento. Use somente decisões documentadas pela organização e fontes oficiais vigentes.
- Quando houver lacuna ou interpretação ambígua, marque `PENDENTE JURÍDICO/DPO`, explique a decisão necessária e suspenda a conclusão sobre esse ponto. Não transforme uma suposição em requisito aprovado.
- Não trate consentimento como opção padrão. Avalie-o somente quando a organização o indicar como hipótese aplicável e o jurídico/DPO confirmar essa escolha.
- Não presuma que criptografia, consentimento ou contrato corrige coleta excessiva, finalidade incompatível ou ausência de hipótese legal.
- Não elimine dados, altere produção ou notifique titulares/ANPD durante uma revisão sem autorização expressa e procedimento aprovado. A autorização de eliminação deve identificar o solicitante e seu papel, o caso/ticket aprovado, os dados, o escopo, o ambiente, o aprovador e o responsável pela execução.
- Em toda conclusão jurídica ou regulatória, registre título da fonte oficial, URL, dispositivo ou seção relevante, versão quando disponível e data da consulta no formato `AAAA-MM-DD`.

## Contexto mínimo antes de analisar ou implementar

Solicite e registre, no mínimo:

1. Organização, controlador responsável, módulo/processo do ERP, ambiente, jurisdições e objetivo da mudança ou revisão.
2. Fluxo completo: origem, coleta, validação, armazenamento, uso, inferências, consultas, relatórios, exportações, compartilhamentos, integrações, arquivamento e descarte.
3. Categorias de titulares e de dados; destaque dados sensíveis, crianças/adolescentes, biometria, geolocalização, credenciais, dados financeiros e combinações que elevem o risco.
4. Finalidade específica de cada operação, hipótese legal candidata, responsável pela decisão e evidência de aprovação jurídica/DPO. Não aceite “cumprir a LGPD” ou “melhorar o serviço” como finalidade suficientemente específica.
5. Papéis e participantes: controlador, controladores conjuntos quando avaliados, operadores, suboperadores, destinatários, provedores de nuvem e países envolvidos.
6. Volume, frequência, escala, decisões automatizadas, monitoramento, usuários internos, perfis privilegiados e segregação entre empresas/filiais/*tenants*.
7. Regras aprovadas de retenção, obrigações legais/contratuais, bloqueios de descarte, processo de direitos dos titulares e histórico de incidentes.
8. Arquitetura e controles existentes: bancos, filas, arquivos, caches, busca, logs, backups, réplicas, *data lakes*, ambientes não produtivos, autenticação, autorização, chaves e monitoramento.

Se faltarem itens que mudem a conclusão, faça perguntas antes de prosseguir. Se ainda for útil, entregue apenas uma análise preliminar, identificando cada suposição, sua consequência e quem deve validá-la.

## Método de trabalho

### 1. Inventariar as operações de tratamento

Crie uma linha por operação material, não apenas por tabela de banco. Para cada linha, registre:

- identificador, processo/módulo e proprietário;
- titular e categorias de dados, inclusive derivados e metadados;
- origem, finalidade, hipótese legal informada e evidência de aprovação;
- sistemas, tabelas/campos, APIs, relatórios, arquivos, filas, logs e backups envolvidos;
- usuários/perfis com acesso, destinatários, operadores, suboperadores e países;
- retenção, gatilho de início, descarte, bloqueio legal e método de atendimento aos direitos;
- riscos, controles, evidências, pendências e data da última revisão.

Use o modelo de ROPA da ANPD apenas quando seu escopo for adequado; o modelo publicado para agentes de tratamento de pequeno porte não deve ser apresentado como universal.

### 2. Classificar e minimizar

- Classifique dados pessoais, sensíveis, pseudonimizados, anonimizados e não pessoais no contexto real. Considere que pseudonimização reduz risco, mas normalmente não retira o dado do regime de dados pessoais.
- Para cada campo e evento, teste necessidade, proporcionalidade e compatibilidade com a finalidade aprovada.
- Remova coletas “para uso futuro”, duplicações e cópias desnecessárias. Prefira valores agregados, faixas, indicadores ou consulta sob demanda quando cumprirem a finalidade.
- Verifique se relatórios, buscas, telas, notificações, URLs, métricas, telemetria e exportações revelam mais dados do que o necessário.

### 3. Validar finalidade e hipótese legal sem inventar

- Associe cada operação a uma finalidade específica e a uma hipótese legal fornecida e aprovada pela organização.
- Se a hipótese não estiver documentada, registre a lacuna e encaminhe ao jurídico/DPO. Não escolha a hipótese “mais conveniente”.
- Para legítimo interesse, quando cogitado, exija avaliação documentada e validação jurídica/DPO, incluindo finalidade, necessidade, balanceamento, expectativas do titular, salvaguardas e possibilidade de oposição. Consulte a orientação vigente da ANPD.
- Quando consentimento for a hipótese confirmada, verifique granularidade, liberdade, informação, destaque quando exigido, prova de obtenção, versionamento do aviso, revogação simples e propagação da revogação. Não use caixas pré-marcadas nem vincule finalidades independentes.

### 4. Modelar arquitetura e acessos

- Desenhe o fluxo entre interface, API, serviços, banco, integrações e terceiros; marque fronteiras de confiança, países e cópias persistentes.
- Aplique menor privilégio, negação por padrão, segregação de funções e acesso por necessidade. Diferencie permissão de tela, endpoint, objeto, campo e linha.
- Isole empresas, filiais e *tenants*. Teste referências diretas a objetos, filtros manipulados, relatórios compartilhados, URLs assinadas e rotas administrativas.
- Proteja contas privilegiadas com autenticação forte, aprovação quando adequada, acesso temporário e trilha de auditoria resistente a alteração.
- Minimize segredos e dados pessoais em tokens, cabeçalhos, parâmetros, mensagens de erro e eventos. Defina criptografia, gestão de chaves e rotação conforme o risco, sem tratá-las como substitutas de legalidade ou minimização.

### 5. Definir retenção e descarte ponta a ponta

- Mantenha matriz por categoria/finalidade com gatilho de início, período aprovado, justificativa e fonte, bloqueios legais, proprietário e método de descarte.
- Não invente prazos. Quando houver conflito entre direito do titular e obrigação de guarda, encaminhe ao jurídico/DPO e preserve o dado de forma restrita até decisão.
- Propague exclusão, anonimização ou bloqueio para tabelas, anexos, índices de busca, caches, filas, réplicas, exportações e sistemas integrados.
- Para backups imutáveis, documente expiração, acesso restrito, impedimento de uso operacional e procedimento para que dados descartados não sejam reintroduzidos após restauração.
- Valide o descarte com evidência técnica; não considere apenas uma marca lógica como exclusão definitiva sem justificar o desenho.
- Nunca declare `ELIMINAÇÃO COMPLETA` enquanto existir backup, réplica, exportação ou outra cópia residual, nem quando algum componente do escopo não tiver sido verificado. Informe um status honesto por componente, como `ELIMINADO DO AMBIENTE ATIVO`, `RESIDUAL RESTRITO ATÉ EXPIRAÇÃO` ou `NÃO VERIFICADO`, com evidência, prazo e responsável.
- Enquanto houver cópia residual, impeça uso operacional e restrinja acesso. Se houver restauração, reaplique o descarte/bloqueio aprovado antes de liberar os dados restaurados para uso e registre essa verificação.

### 6. Operacionalizar direitos dos titulares

- Defina canal, autenticação proporcional ao risco, triagem, busca em todos os sistemas, aprovação, resposta, registro e recurso/escalonamento.
- Mapeie acesso/confirmação, correção, anonimização/bloqueio/eliminação quando cabíveis, portabilidade quando regulamentada e aplicável, informação sobre compartilhamento e consentimento, revogação, oposição e revisão de decisões automatizadas conforme o caso.
- Evite coletar dados excessivos para validar identidade e impeça que um solicitante acesse dados de outro titular.
- Não fixe prazo de resposta de memória. Consulte a LGPD e a regulamentação oficial vigentes na data do caso e registre a fonte usada.
- Teste o fluxo de ponta a ponta, incluindo anexos, dados derivados, integrações, arquivos históricos e exceções aprovadas.

### 7. Tratar anonimização e pseudonimização corretamente

- Documente técnica, objetivo, conjunto de dados, informações auxiliares, quem mantém a chave e risco de reidentificação.
- Separe chaves/tabelas de correspondência, restrinja acesso e monitore junções que possam recompor identidades.
- Para afirmar anonimização, avalie reidentificação razoavelmente possível considerando meios próprios ou de terceiros, contexto, custo e evolução técnica. Registre método e teste; se a reversão permanecer viável, trate como dado pessoal.
- Reavalie ao combinar bases, liberar exportações ou mudar a finalidade.

### 8. Controlar logs, backups e ambientes não produtivos

- Defina lista permitida de campos em logs e telemetria. Mascare ou remova documentos, contatos, conteúdo livre, tokens, credenciais, dados sensíveis e corpos de requisição/resposta quando não forem indispensáveis.
- Restrinja acesso aos logs, proteja integridade, monitore consultas, defina retenção e teste a redação de dados.
- Não copie produção para desenvolvimento, teste, demonstração ou suporte sem autorização, necessidade documentada e controles equivalentes. Prefira dados sintéticos; quando dados reais forem indispensáveis, minimize, pseudonimize/anonymize conforme o objetivo e limite tempo/acesso.
- Inclua snapshots, réplicas, arquivos temporários e dumps na governança. Teste restauração segura e o tratamento de dados cuja retenção já expirou.

### 9. Governar operadores, suboperadores e transferências internacionais

- Inventarie instruções, finalidade, categorias de dados, localização, medidas de segurança, subcontratações, auditoria, assistência a direitos/incidentes e devolução ou eliminação ao término.
- Verifique tecnicamente se integrações enviam apenas dados autorizados e se mudanças de fornecedor ou região criam novo fluxo.
- Não deduza o papel de controlador ou operador apenas do contrato; registre os fatos e encaminhe divergências ao jurídico/DPO.
- Para transferência internacional, identifique origem/destino, importador, suboperadores, países, armazenamento e acesso remoto. Exija que o jurídico/DPO confirme o mecanismo previsto na regulamentação vigente da ANPD; não escolha nem redija o mecanismo por conta própria.

### 10. Avaliar necessidade de RIPD

Acione o jurídico/DPO e avalie RIPD antes da implementação quando o tratamento puder gerar alto risco, especialmente em escala, uso de dados sensíveis ou de crianças/adolescentes, biometria, monitoramento sistemático, combinação de bases, tecnologia inovadora, perfilamento ou decisões automatizadas com efeitos relevantes.

Quando aplicável, documente descrição do tratamento, necessidade/proporcionalidade, agentes e fluxos, riscos aos direitos e liberdades, medidas de mitigação, risco residual, responsáveis, aprovações e plano de revisão. Trate o RIPD como documento vivo. Consulte a orientação atual da ANPD; não use este sinalizador como decisão jurídica definitiva.

### 11. Preparar resposta a incidentes

- Preserve evidências e linha do tempo; registre descoberta, contenção, dados/titulares potencialmente afetados, sistemas, agentes, países e medidas tomadas.
- Avalie severidade e risco aos titulares com o time de resposta, segurança, jurídico, controlador e encarregado/DPO.
- O controlador deve decidir comunicações à ANPD e aos titulares com base na norma vigente e no caso concreto. Não prometa nem omita comunicação apenas por uma regra codificada no ERP.
- Mantenha contatos, critérios de escalonamento, modelos aprovados, relógio do incidente e evidência das decisões. Teste o plano em exercício de mesa.

## Evidências e testes mínimos

Associe cada controle a requisito, responsável, evidência e teste reproduzível. Inclua, conforme o escopo:

- testes negativos de autenticação e autorização, inclusive acesso entre *tenants*, IDOR/BOLA, campo, linha, exportação e administração;
- revisão de RBAC/ABAC, segregação de funções e contas privilegiadas;
- varredura de dados pessoais e segredos em logs, métricas, erros, filas, caches, URLs e ambientes não produtivos;
- teste de coleta mínima e bloqueio de campos não necessários;
- prova de obtenção, versionamento e revogação do consentimento, somente quando aplicável;
- exercício de solicitação de titular de ponta a ponta, incluindo integrações e validação de identidade;
- teste de retenção, descarte, bloqueio legal e restauração de backup sem reintrodução indevida;
- avaliação de pseudonimização/anonimização e tentativa controlada de reidentificação;
- inventário e teste contratual/técnico de operadores, suboperadores e transferências;
- exercício de incidente, restauração e cadeia de custódia;
- revisão de migrações para impedir ampliação de finalidade, permissões ou cópias.

Não aceite como evidência apenas “o sistema suporta”. Prefira configuração exportada, consulta reproduzível, captura com dados protegidos, log de teste, resultado automatizado, ticket aprovado, contrato/registro vigente ou decisão formal, sempre com data e responsável.

## Formato da entrega

Entregue:

1. escopo, contexto confirmado, exclusões e suposições;
2. diagrama ou tabela de fluxo e inventário das operações;
3. matriz de achados com `CONFORME`, `LACUNA`, `PENDENTE JURÍDICO/DPO` ou `NÃO APLICÁVEL`;
4. para cada achado: requisito/fonte, fato observado, evidência, risco ao titular e ao negócio, recomendação técnica, responsável, prioridade, prazo e teste de aceite;
5. decisões jurídicas pendentes separadas de tarefas de engenharia;
6. registro das fontes oficiais: título, URL, seção/dispositivo, versão e data de consulta;
7. riscos residuais, aprovações e data sugerida para revisão.

Se não houver evidência suficiente, registre “não verificado”; não converta ausência de evidência em conformidade.

## Fontes oficiais de partida

Antes de usar qualquer fonte, confirme que continua vigente, procure atos posteriores na página oficial e registre a data da consulta. Em conflito, lacuna ou ambiguidade, interrompa a conclusão e consulte jurídico/DPO.

- [Lei nº 13.709/2018 — LGPD, texto compilado (Planalto)](https://www.planalto.gov.br/ccivil_03/_ato2015-2018/2018/lei/L13709compilado.htm)
- [Regulamentações da ANPD — índice e situação dos atos normativos](https://www.gov.br/anpd/pt-br/acesso-a-informacao/institucional/atos-normativos/regulamentacoes_anpd)
- [Direitos dos titulares de dados pessoais — ANPD](https://www.gov.br/anpd/pt-br/assuntos/titular-de-dados-1/direito-dos-titulares)
- [Comunicação de incidente de segurança — ANPD](https://www.gov.br/anpd/pt-br/assuntos/comunicacao-de-incidentes-de-seguranca-cis)
- [Relatório de Impacto à Proteção de Dados Pessoais — ANPD](https://www.gov.br/anpd/pt-br/canais_atendimento/agente-de-tratamento/relatorio-de-impacto-a-protecao-de-dados-pessoais-ripd)
- [Guia sobre agentes de tratamento e encarregado — ANPD](https://www.gov.br/anpd/pt-br/centrais-de-conteudo/materiais-educativos-e-publicacoes/guia-orientativo-para-definicoes-dos-agentes-de-tratamento-de-dados-pessoais-e-do-encarregado)
- [Guia sobre legítimo interesse — ANPD](https://www.gov.br/anpd/pt-br/centrais-de-conteudo/materiais-educativos-e-publicacoes/guia_orientativo_hipoteses_legais_tratamento_de_dados_pessoais_legitimo_interesse)
- [Guia de segurança da informação para agentes de pequeno porte — ANPD](https://www.gov.br/anpd/pt-br/centrais-de-conteudo/materiais-educativos-e-publicacoes/guia-orientativo-sobre-seguranca-da-informacao-para-agentes-de-tratamento-de-pequeno-porte)
- [Modelo de registro das operações de tratamento para agentes de pequeno porte — ANPD](https://www.gov.br/anpd/pt-br/documentos-e-publicacoes/modelo_de_ropa_para_atpp-3.xlsx)
