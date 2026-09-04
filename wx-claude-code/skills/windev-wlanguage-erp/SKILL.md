---
name: windev-wlanguage-erp
description: "ERP em WINDEV, WEBDEV, WINDEV Mobile e WLanguage: dados, UI, APIs, testes, desempenho e migração, por versão WX exata."
origem: erp-skills-pack (pesquisa em skills.sh, 2026-09-04); descricao encurtada de proposito
---

# ERP com WINDEV, WEBDEV, WINDEV Mobile e WLanguage

Produza mudanças compatíveis com o produto e a versão realmente usados, preservando invariantes do ERP e o estilo do projeto. Na ajuda oficial, o idioma aparece frequentemente como “WLangage”; use o nome do projeto ou do usuário sem inferir diferença técnica.

## Regra de verificação WX

Antes de escrever código, obtenha e registre:

- produto exato: WINDEV, WEBDEV, WINDEV Mobile ou combinação;
- versão WX, edição, número de build/atualização e idioma do IDE;
- configuração do projeto, alvo de geração e plataformas de execução;
- versão do servidor de aplicação, HFSQL, conectores e drivers envolvidos.

Para cada função, tipo, propriedade, constante, editor ou recurso proposto, consulte sua página na ajuda oficial da PC SOFT com a versão WX alvo selecionada. Leia especialmente as matrizes `Disponibilité` e `Version minimum requise`; confirme produtos, plataformas, modo de execução, limitações e exemplos. Registre URL, versão exibida e data da consulta.

Quando houver banco externo, use também a documentação oficial da versão implantada do banco e do driver/conector para tipos, SQL, transações, isolamento, bloqueios, erros e limites. A ajuda PC SOFT comprova o acesso pelo produto WX; ela não substitui a especificação do mecanismo externo. Confirme o comportamento com teste no banco real.

Se a documentação da versão alvo não confirmar uma capacidade, escreva `NÃO VERIFICADO NA VERSÃO WX` e não forneça sintaxe executável. Use pseudocódigo explicitamente rotulado quando ele ainda ajudar. Não converta exemplos de outra versão, produto ou plataforma em código supostamente válido.

## Escopo e exclusões

Esta skill cobre engenharia e revisão de módulos ERP, modelo e acesso a dados, regras transacionais, consultas, procedimentos, classes, interfaces desktop/web/mobile, integrações, segurança, testes, desempenho, migração e documentação.

Não:

- invente nomes de APIs, parâmetros, constantes, palavras reservadas ou extensões de arquivo;
- presuma que código WINDEV executa igual em WEBDEV ou Mobile, ou que código servidor executa no navegador;
- prometa suporte a banco, conector, sistema operacional, navegador ou dispositivo sem confirmação na matriz oficial da versão;
- altere análise/esquema, converta projeto, atualize servidor ou migre dados de produção sem backup verificado, ensaio e rollback;
- redesenhe regras contábeis, fiscais, financeiras ou de estoque sem validação do responsável do domínio;
- faça refatoração ampla em projeto existente quando uma mudança compatível e localizada atender ao pedido.

## Entradas mínimas

Solicite:

1. Objetivo, módulos, usuários, empresas/*tenants*, invariantes de negócio e critérios de aceite.
2. Projeto existente ou *greenfield*; repositório/artefatos, branch, convenções, análise/modelo, configurações e baseline de compilação/testes.
3. Produto(s), versão WX/build, alvo(s), sistemas operacionais, navegadores e dispositivos.
4. Banco e versão: HFSQL Classic ou Client/Server, ou banco externo; topologia, volume, crescimento, conexão, conector/driver/licença e restrições de rede.
5. Regras de transação, concorrência, isolamento, bloqueio, consistência, precisão decimal, datas/fusos e auditoria.
6. Telas/páginas móveis ou web, padrões visuais, acessibilidade, offline/sincronização e requisitos de impressão/relatórios.
7. APIs/webservices, contratos, autenticação, timeouts, idempotência, terceiros e ambientes.
8. Segurança, privacidade, perfis, segredos, logs, retenção, implantação, RPO/RTO e metas de desempenho.

Para correção ou diagnóstico, obtenha também:

- mensagem de erro completa e *stack trace*, quando houver;
- passo a passo mínimo e reproduzível, incluindo dados de entrada não sensíveis;
- resultado esperado e resultado observado;
- ambiente, produto, configuração, plataforma, banco/conector e versões conhecidas;
- logs e capturas sanitizados, sem credenciais, tokens ou dados pessoais desnecessários;
- última mudança conhecida em código, esquema, configuração, build, runtime ou infraestrutura.

Se a versão WX ou o produto estiver ausente, não gere código WLanguage nem instruções específicas de IDE, função ou configuração. Isso não bloqueia o diagnóstico: classifique sintomas, compare esperado/observado, proponha verificações independentes de versão, identifique evidências faltantes e entregue hipóteses claramente rotuladas, sem inventar sintaxe ou capacidades.

## Escolher o modo de trabalho

### Projeto existente

1. Preserve uma baseline reproduzível antes da mudança: build, testes, alertas conhecidos, versões e configuração.
2. Inspecione estrutura do projeto, análise, configurações, dependências, consultas, coleções de procedimentos, classes, janelas/páginas, relatórios, webservices e testes existentes.
3. Localize convenções de nomes, tratamento de erro, transação, logging, segurança e acesso a dados. Siga-as salvo quando forem a causa comprovada do problema.
4. Trace chamadas e impacto em módulos consumidores antes de alterar assinatura, consulta, esquema ou componente compartilhado.
5. Prefira o menor delta coerente; se refatoração for necessária, separe-a da mudança funcional e mantenha comportamento coberto por testes.
6. Execute auditoria, build e testes na mesma configuração usada pela entrega. Registre diferenças entre ambiente local e destino.

### Projeto greenfield

1. Defina produtos/alvos, topologia, modelo de implantação, fonte de verdade, banco, conectividade e operação offline antes de estruturar o código.
2. Separe regras de domínio, casos de uso, persistência, integrações e UI em limites adequados ao tamanho do ERP, sem criar abstrações sem uso.
3. Estabeleça convenções para erros, transações, logs, configuração, segredos, testes, versionamento de contratos e migrações.
4. Crie uma fatia vertical pequena e executável para provar build, conexão, transação, UI, segurança, implantação e observabilidade no alvo real.
5. Expanda somente após medir a fatia e confirmar a compatibilidade na versão WX.

## Fluxo de engenharia

### 1. Mapear arquitetura e impacto

- Identifique pontos de entrada, eventos da UI, procedimentos, classes, consultas, tabelas/arquivos, serviços externos e relatórios envolvidos.
- Mapeie dependências e direção de chamadas. Evite regra crítica duplicada em eventos de múltiplas telas.
- Declare onde validação, autorização, transação, persistência e auditoria ocorrem.
- Para mudanças compartilhadas entre produtos, mantenha uma matriz por WINDEV, WEBDEV e Mobile; não marque como comum sem compilar e testar em cada alvo.

### 2. Projetar HFSQL ou banco externo

- Confirme na ajuda da versão alvo o modo HFSQL, o acesso externo, o conector/driver, a plataforma, o licenciamento e as diferenças documentadas.
- Modele chaves, unicidade, integridade, nulidade, precisão numérica, datas/fusos, volumes e arquivamento a partir das regras do ERP.
- Use consultas parametrizadas ou recursos oficiais equivalentes; nunca concatene entrada do usuário em SQL.
- Se usar o editor de consultas, mantenha nomes e parâmetros claros, examine SQL gerado quando relevante e teste no mecanismo real.
- Se usar SQL específico do banco, isole o dialeto, registre a versão e mantenha testes por mecanismo suportado.
- Planeje índices a partir de filtros, junções, ordenações e medições. Não crie índice por intuição nem assuma que estatísticas estão atualizadas.
- Evite leitura linha a linha e ida repetida ao servidor quando uma consulta em conjunto resolver; confirme impacto com dados representativos.

### 3. Garantir transações e concorrência

- Liste operações que devem confirmar ou falhar juntas e mantenha a transação curta, sem interação com usuário ou chamada de rede no meio.
- Consulte a documentação da versão para início, confirmação, cancelamento, suporte pelo conector e restrições. Só então use os elementos WLanguage exatos.
- Faça todo caminho de erro encerrar a transação de forma explícita e preserve o erro original para diagnóstico seguro.
- Defina estratégia contra atualização perdida: restrição, versão, bloqueio ou comando atômico conforme o banco e a regra. Não use apenas “ler, alterar, gravar” sob concorrência.
- Trate bloqueio, deadlock, timeout e desconexão. Retry só é aceitável quando a operação for idempotente e limitado.
- Não suponha que semântica, isolamento ou bloqueio de HFSQL seja igual em banco externo, conector, Webservice ou plataforma diferente.
- Teste duas ou mais sessões concorrentes no mecanismo real e valide saldo, sequência, unicidade e auditoria.

### 4. Organizar consultas, procedimentos e classes

- Dê a cada consulta um contrato de parâmetros, colunas, cardinalidade, ordenação, nulidade e erros; evite depender de ordem implícita.
- Centralize operações reutilizáveis em coleções de procedimentos ou classes somente quando a estrutura for suportada pela versão e melhorar coesão.
- Mantenha procedimentos pequenos, com entradas/saídas e efeitos explícitos. Não dependa desnecessariamente de estado global ou do contexto visual.
- Use classes para comportamento e invariantes coesos; não transforme toda tabela ou controle de UI em classe automaticamente.
- Em procedimentos e consultas armazenados no servidor HFSQL, confirme o contexto e os elementos WLanguage permitidos; não presuma acesso a classes, janelas, páginas ou estado do cliente.
- Mantenha regra de negócio fora de manipuladores visuais quando precisar ser reutilizada ou testada.
- Trate retorno, erro e recurso aberto/fechado em todos os caminhos. Use o mecanismo oficial confirmado para a versão.
- Em exemplos, copie a sintaxe da página oficial relevante e adapte somente após compilar. Se não puder compilar, declare a limitação.

### 5. Construir UI por produto

Para WINDEV desktop:

- preserve padrões de janelas, navegação, atalhos, foco, validação e relatórios do projeto;
- mantenha operações longas fora do fluxo que bloqueia a interface quando houver mecanismo confirmado e seguro na versão;
- teste resolução, escala, teclado, múltiplas janelas e erros de conectividade no alvo.

Para WEBDEV:

- diferencie rigorosamente código servidor e navegador, ciclo de requisição, sessão e dados enviados ao cliente;
- aplique autorização no servidor, independentemente de controles ocultos ou desabilitados na página;
- teste navegação, expiração de sessão, múltiplas abas, retorno do navegador, responsividade, acessibilidade e principais navegadores suportados;
- confirme o modo de site e implantação antes de usar qualquer recurso dependente dele.

Para WINDEV Mobile:

- modele conectividade intermitente, repetição segura, conflitos de sincronização, ciclo de vida e armazenamento local;
- solicite apenas permissões de dispositivo necessárias e trate recusa/revogação;
- teste em sistemas e dispositivos reais da matriz, inclusive rotação, segundo plano, retomada, pouco espaço e rede ruim;
- trate GO/simulador como feedback de desenvolvimento, não como prova de comportamento no aparelho real;
- não presuma paridade Android/iOS ou desktop sem confirmação da ajuda e teste.

### 6. Integrar REST e webservices

- Confirme se o projeto cria, publica, importa ou consome REST/SOAP e quais recursos existem no produto/versão alvo.
- Defina contrato, versão, autenticação/autorização, conteúdo, limites, códigos de erro, paginação, precisão, datas/fusos e idempotência.
- Valide payload antes de aplicar regra ou abrir transação. Nunca confie em autorização feita apenas pelo cliente.
- Configure timeout, cancelamento e retry limitado; não repita automaticamente operação não idempotente.
- Proteja credenciais fora do código e dos artefatos de distribuição; restrinja logs de cabeçalhos e corpos.
- Teste contrato e falhas com o serviço real ou simulador fiel. Se usar importação OpenAPI ou editor REST, confirme disponibilidade na versão.

### 7. Aplicar segurança

- Autentique e autorize cada operação no lado confiável; aplique menor privilégio e segregação entre empresas/*tenants*.
- Parametrize consultas e valide tipo, tamanho, formato e domínio das entradas.
- Não grave senhas, tokens, chaves ou dados sensíveis em código, repositório, mensagens de erro, trace ou log.
- Use canais protegidos, armazenamento de segredo e recursos criptográficos somente conforme suporte e orientação da versão/plataforma.
- Proteja sessões web, endpoints, arquivos de configuração, diretórios de dados, backups e contas do banco.
- Registre ações privilegiadas e alterações críticas com ator, data, entidade e correlação, sem vazar dados além do necessário.
- Revise dependências, runtimes, servidor de aplicação e banco para atualizações suportadas; não atualize automaticamente em produção.

### 8. Testar e medir

- Use o gerenciador de testes automáticos e os recursos de auditoria disponíveis na versão alvo; confirme na ajuda antes de prescrever passos do IDE.
- Separe testes de regra, persistência, contrato, UI e ponta a ponta. Mantenha dados determinísticos e isolamento entre execuções.
- Cubra sucesso, validação, permissão negada, nulidade/limites, erro do banco, rollback, concorrência, timeout, resposta inválida e repetição.
- Compile e execute todas as configurações e plataformas afetadas, não apenas a configuração ativa do desenvolvedor.
- Meça com o analisador de desempenho oficial quando disponível e na versão correta. Use volume e latência representativos.
- Analise consultas lentas, plano/`EXPLAIN` quando suportado, cardinalidade, tráfego, índices/estatísticas, memória, inicialização, renderização e chamadas repetidas antes de otimizar.
- Compare baseline e resultado com a mesma carga; não aceite “parece mais rápido”.

### 9. Migrar versão, esquema ou dados

- Separe, quando possível, atualização WX, mudança de esquema e alteração funcional para reduzir causas de falha.
- Preserve projeto e dados em fonte controlada/backup testado. Faça conversão ou migração primeiro em cópia isolada.
- Leia notas e incompatibilidades da versão de origem e destino; inventarie erros, avisos, mudanças de comportamento e componentes externos.
- Não presuma *round-trip* entre builds: depois que um elemento for convertido ou salvo em atualização mais recente, sua reabertura em build anterior pode não ser suportada. Confirme nas notas oficiais antes de abrir o único artefato.
- Planeje evolução de esquema compatível com implantação, transformação de dados, validação, duração, bloqueios e espaço.
- Ensaie com cópia representativa, meça, reconcilie contagens/valores e teste rollback ou *roll-forward*.
- Não abra/converta de modo irreversível a única cópia do projeto nem execute migração destrutiva sem aprovação.
- Depois da migração, compile todas as configurações, execute a suíte e teste integrações, relatórios e implantação no alvo.

### 10. Documentar para manutenção

Registre versão WX/build, produtos/alvos, componentes e conectores; decisões de arquitetura; análise/esquema; contratos; transações/concorrência; configuração e segredos por referência; procedimento de build/deploy; migrações; testes; baseline de desempenho; observabilidade; rollback; limitações e links oficiais consultados.

## Invariantes

- Todo código específico foi verificado para o produto, plataforma e versão WX alvo, ou está marcado como pseudocódigo/não verificado.
- Projeto existente mantém baseline e convenções, exceto mudança intencional, justificada e testada.
- Regra crítica não depende exclusivamente da UI nem de autorização no cliente.
- Operação atômica confirma integralmente ou cancela; chamada remota não mantém transação aberta.
- Concorrência não perde atualização, duplica efeito nem viola unicidade/saldo aprovado.
- Entrada não é concatenada em SQL; segredos não aparecem no código ou logs.
- Tipos numéricos, nulidade, datas e fusos preservam a semântica do domínio e do banco real.
- Cada configuração afetada compila e passa seus testes no alvo suportado.
- Migração tem backup restaurável, ensaio, reconciliação e caminho de retorno.
- Compatibilidade entre WINDEV, WEBDEV, Mobile, HFSQL e bancos externos nunca é presumida.

## Artefatos da entrega

Entregue, conforme o escopo:

1. ficha de contexto e matriz produto × versão WX × build × plataforma × banco/conector;
2. baseline do projeto existente ou decisões de fundação do greenfield;
3. mapa de impacto e desenho de componentes/fluxos;
4. alterações de análise/esquema, consultas, procedimentos, classes, UI e integrações;
5. matriz de transações, concorrência, erros e idempotência;
6. código compilável na versão alvo, ou pseudocódigo claramente identificado;
7. testes automáticos/manuais e relatório por configuração/plataforma;
8. baseline e comparação de desempenho;
9. plano de migração, implantação, reconciliação e rollback;
10. documentação e registro de fontes oficiais, versão exibida e data de consulta.

## Testes e critérios de aceite

Classifique cada superfície como `AFETADA`, `NÃO AFETADA` ou `NÃO APLICÁVEL`, com justificativa. Execute os testes da superfície alterada, suas dependências e regressões de domínio; uma correção localizada não exige artificialmente UI, REST, migração ou desempenho quando não os toca.

Teste, conforme o escopo:

- build limpo e suíte nas configurações afetadas;
- regra de domínio em limites, nulos, precisão, datas e permissões;
- commit, cancelamento, erro intermediário, queda e concorrência real;
- consulta no HFSQL ou banco externo/versionamento realmente suportado;
- UI no desktop, navegador ou dispositivo da matriz, inclusive falhas e acessibilidade;
- REST/webservice com sucesso, autenticação, contrato inválido, timeout, duplicação e indisponibilidade;
- migração a partir de cópia representativa, reconciliação e restauração;
- desempenho com volume acordado e comparação à baseline;
- auditoria de segredos, SQL parametrizado, autorização e isolamento por empresa/*tenant*.

Aceite somente quando não houver recurso afetado não verificado na versão alvo; todas as configurações impactadas compilarem; testes aplicáveis de domínio, transação, concorrência, integração e segurança passarem; metas de desempenho acordadas forem atendidas quando aplicáveis; migração e rollback tiverem sido ensaiados quando houver migração; e documentação permitir reprodução por outra pessoa.

Se faltarem metas quantitativas ou plataformas, proponha-as para validação e marque-as como hipótese, não como requisito aprovado.

## Ajuda oficial da PC SOFT

Use somente a ajuda oficial e selecione a versão WX alvo antes de aplicar conteúdo. As páginas podem exibir a documentação mais recente por padrão; registre versão exibida e data de consulta.

- [Ajuda de WINDEV, WEBDEV e WINDEV Mobile](https://doc.pcsoft.fr/fr-FR/)
- [WLangage — visão geral](https://doc.pcsoft.fr/fr-FR/?9000196=)
- [Mudanças de comportamento entre versões](https://doc.pcsoft.fr/fr-FR/?9500013=)
- [Atualizações do WINDEV 2026 — exemplo de notas por build](https://doc.pcsoft.fr/fr-FR/?9000050=)
- [Manipular transações por programação](https://doc.pcsoft.fr/fr-FR/?3044336=)
- [Transações no HFSQL Client/Server](https://doc.pcsoft.fr/fr-FR/?3044337=)
- [Modos de isolamento de transações Client/Server](https://doc.pcsoft.fr/fr-FR/?1000017316=)
- [Acesso a bancos de dados — resumo](https://doc.pcsoft.fr/fr-FR/?3044202=)
- [Tipo Connexion para bases de dados](https://doc.pcsoft.fr/fr-FR/?1514073=)
- [HFSQL Client/Server — recomendações](https://doc.pcsoft.fr/fr-FR/?1000017310=)
- [Gestão de direitos no HFSQL Client/Server](https://doc.pcsoft.fr/fr-FR/?3044333=)
- [Editor de consultas](https://doc.pcsoft.fr/fr-FR/?2032063=)
- [Uso de consulta parametrizada](https://doc.pcsoft.fr/fr-FR/?2032032=)
- [Procedimentos e consultas armazenados no HFSQL Client/Server](https://doc.pcsoft.fr/fr-FR/?3044360=)
- [Coleções de procedimentos](https://doc.pcsoft.fr/fr-FR/?1513003=)
- [Programação orientada a objetos — apresentação](https://doc.pcsoft.fr/fr-FR/?6010009=)
- [Janelas WINDEV](https://doc.pcsoft.fr/fr-FR/?1010025=)
- [Páginas WEBDEV](https://doc.pcsoft.fr/fr-FR/?1012018=)
- [Código servidor e código navegador no WEBDEV](https://doc.pcsoft.fr/fr-FR/?2013002=)
- [Particularidades de aplicações Android](https://doc.pcsoft.fr/fr-FR/?9000108=)
- [Teste de aplicação Mobile](https://doc.pcsoft.fr/fr-FR/?2019019=)
- [Webservice REST — descrição e características](https://doc.pcsoft.fr/fr-FR/?1410090999=)
- [Importação de API OpenAPI](https://doc.pcsoft.fr/fr-FR/?1410087809=)
- [Testes automáticos](https://doc.pcsoft.fr/fr-FR/?1410087527=)
- [Testes automáticos de procedimentos e classes](https://doc.pcsoft.fr/fr-FR/?2019027=)
- [Analisador de desempenho](https://doc.pcsoft.fr/fr-FR/?2030035=)
- [Otimização da execução de uma consulta](https://doc.pcsoft.fr/fr-FR/?2032021=)
- [Auditoria estática](https://doc.pcsoft.fr/fr-FR/?1014501=)
- [Auditoria dinâmica](https://doc.pcsoft.fr/fr-FR/?1014502=)
- [Auditorias de segurança — recurso cuja versão mínima deve ser conferida](https://doc.pcsoft.fr/fr-FR/?9000234=)
- [Responsive Web Design em páginas WEBDEV](https://doc.pcsoft.fr/fr-FR/?9000165=)
- [Modificação automática dos arquivos de dados](https://doc.pcsoft.fr/fr-FR/?3044195=)
- [Sincronização da análise com bancos externos](https://doc.pcsoft.fr/fr-FR/?2011024=)
- [GDS — mudança de versão dos projetos](https://doc.pcsoft.fr/fr-FR/?2038027=)
