---
name: erp-approval-workflows
description: "Fluxos de aprovação de ERP: alçadas, etapas, delegação, segregação de funções, expiração e trilha auditável."
origem: erp-skills-pack (pesquisa em skills.sh, 2026-09-04); descricao encurtada de proposito
---

# Fluxos de aprovação para ERP

Modele aprovação como uma máquina de estados versionada, autorizada e auditável. Não a trate como um campo booleano nem como uma sequência controlada pela interface.

## Resultado esperado

Entregue um fluxo no qual:

- alçadas selecionem deterministicamente quem deve decidir;
- estados, transições, guardas e efeitos estejam explícitos;
- etapas seriais, paralelas, por unanimidade, qualquer aprovador ou quórum tenham semântica definida;
- rejeição, devolução, cancelamento, delegação, expiração e escalonamento sejam recuperáveis;
- ninguém concentre funções incompatíveis nem aprove a própria solicitação sem regra excepcional explícita;
- retries e decisões concorrentes não dupliquem efeitos nem corrompam o estado;
- a trilha permita reconstruir quem fez o quê, sobre qual versão e com qual autoridade.

## Escopo

Inclua quando aplicável:

- solicitações de compra, pedidos, descontos, pagamentos, crédito, cadastro de fornecedor, despesas, férias, contratos e lançamentos sensíveis;
- regras de alçada por valor, moeda, empresa, centro de custo, categoria, risco, projeto ou atributo do documento;
- aprovação serial, paralela, `todos`, `qualquer um`, quórum e combinações por estágio;
- decisão humana ou automática baseada em política explícita;
- rejeição, devolução para correção, retirada, cancelamento, expiração, reabertura e nova submissão;
- substituição temporária, delegação, escalonamento e tratamento de ausência;
- segregação estática e dinâmica de funções;
- autorização, idempotência, concorrência, notificações, outbox e auditoria.

## Fora do escopo

Não presuma nem declare:

- validade jurídica de assinatura eletrônica, carimbo do tempo ou certificado;
- conformidade fiscal, trabalhista, bancária ou contábil sem requisito oficial aplicável;
- que aprovação substitui autenticação, autorização de recurso ou lançamento contábil;
- que uma mensagem de e-mail, clique de interface ou status recebido do cliente é prova suficiente de decisão;
- que IA pode decidir operação material sem política, autoridade, explicabilidade e supervisão definidas.

Encaminhe contabilização, regra fiscal, isolamento multiempresa e confiabilidade de integrações às skills especializadas. Mantenha aqui apenas os contratos entre esses domínios.

## Entradas necessárias

Obtenha ou marque como pendente:

- tipos de objeto aprovável e efeitos liberados por sua aprovação;
- estados atuais, ações possíveis e responsáveis por cada ação;
- matriz de alçadas, limites, moedas, condições, exceções e vigências;
- hierarquia de empresa, filial, centro de custo, departamento e projeto;
- papéis incompatíveis e casos em que quatro olhos ou quórum são obrigatórios;
- política para autoaprovação, substituição, delegação, ausência e contas de serviço;
- significado de rejeitar, devolver, cancelar, retirar, expirar, reabrir e ressubmeter;
- dados materiais cuja alteração invalida decisões anteriores;
- prazos, calendário útil, fuso horário, lembretes e níveis de escalonamento;
- canais de notificação e comportamento quando estiverem indisponíveis;
- requisitos de retenção, privacidade, assinatura, auditoria e evidência;
- banco, mensageria, integrações e limites transacionais.

Não complete lacunas críticas com convenções silenciosas. Registre requisito confirmado, hipótese e decisão proposta.

## Modelo mínimo

Separe os conceitos abaixo, mesmo que a implementação use outros nomes:

| Entidade | Conteúdo mínimo | Invariante |
|---|---|---|
| Definição do fluxo | ID, versão, escopo, vigência, estágios e regras | Versões publicadas não mudam retroativamente |
| Instância | Objeto, revisão, definição aplicada, estado e versão concorrente | Aponta para uma revisão imutável do objeto |
| Estágio | Ordem, modo serial/paralelo, regra de conclusão e prazo | Só inicia quando sua guarda for satisfeita |
| Atribuição | Aprovador elegível, papel, escopo e delegação | Não concede privilégio além da alçada original |
| Decisão | Ator, ação, instante, motivo e snapshot avaliado | É acrescentada; não sobrescrita |
| Evento de prazo | vencimento, lembrete, escalonamento e tentativa | Execução idempotente |
| Efeito/outbox | comando liberado após transição | Persiste atomicamente com a mudança de estado |

Vincule a instância a `subject_type`, `subject_id` e `subject_revision` ou hash equivalente. Uma aprovação só vale para os dados apresentados ao aprovador.

## Estados e transições

Defina uma máquina de estados explícita. Um conjunto típico, a adaptar, é:

- `RASCUNHO`: ainda editável e não submetido;
- `SUBMETIDO`: snapshot fechado e validações iniciais executadas;
- `PENDENTE`: há decisões exigidas;
- `PARCIALMENTE_APROVADO`: parte das decisões paralelas foi registrada;
- `APROVADO`: todas as condições de liberação foram satisfeitas;
- `REJEITADO`: política de rejeição encerrou a instância;
- `DEVOLVIDO`: requer correção e uma nova revisão;
- `CANCELADO`: retirada autorizada sem fingir aprovação ou rejeição;
- `EXPIRADO`: prazo terminal atingido segundo a política.

Para cada transição, especifique:

- estado de origem e destino;
- comando e ator autorizado;
- guardas, inclusive versão esperada, alçada e segregação de funções;
- campos obrigatórios, como justificativa;
- eventos, notificações e efeitos gerados;
- comportamento idempotente e resposta a estado já alterado;
- se é reversível e, em caso positivo, por qual transição compensatória.

Valide estado e sequência no servidor em toda solicitação. Rejeite transições ausentes, fora de ordem ou aplicadas à revisão errada. Não apague um estado terminal: estorno, revogação ou nova submissão deve criar evento e, quando apropriado, nova instância/revisão.

## Invariantes obrigatórios

1. Negue por padrão e valide sujeito, ação, objeto e contexto em cada comando.
2. Uma decisão vincula-se à revisão exata do documento, da política e da alçada avaliadas.
3. Alteração material após submissão invalida ou reinicia as aprovações segundo regra explícita.
4. Publicar nova configuração não modifica silenciosamente instâncias em andamento.
5. Estado aprovado somente é alcançado pelo motor após satisfazer todas as regras; clientes e integrações não gravam esse estado diretamente.
6. O solicitante não aprova a própria solicitação salvo exceção documentada, autorizada e auditada.
7. Delegação não amplia valor, empresa, ação, duração ou demais atributos. A autoridade efetiva é a interseção aplicável entre autoridade própria do delegante, autoridade própria do delegado, escopo delegado e política/contexto da instância.
8. Repetir o mesmo comando produz o mesmo resultado observável e não repete efeitos.
9. Uma decisão aceita é persistida na mesma transação que atualiza o estado e registra o evento/outbox correspondente.
10. Eventos de auditoria não são editados nem excluídos pelo fluxo operacional.
11. Notificação informa; não constitui a fonte de verdade da aprovação.
12. Falha, timeout ou ausência de aprovador nunca resulta em aprovação implícita.

## Alçadas

Modele regras com limites e precedência inequívocos:

- valor mínimo inclusivo/exclusivo e valor máximo inclusivo/exclusivo;
- moeda da regra e taxa/data usada quando houver conversão;
- empresa, estabelecimento, centro de custo, categoria, projeto, risco e tipo de operação;
- papel ou pessoa elegível, número de aprovadores e ordem dos estágios;
- vigência da regra e versão aplicada;
- fallback quando não houver aprovador elegível.

Teste valores imediatamente abaixo, exatamente no limite e imediatamente acima. Normalize moeda e arredondamento antes de selecionar a regra. Defina política contra fracionamento de operações para contornar alçada; quando relevante, avalie total do pedido, grupo de documentos relacionados ou janela temporal.

Se mais de uma regra casar, use precedência documentada ou falhe com erro de configuração. Não escolha arbitrariamente o primeiro registro retornado pelo banco.

## Aprovação serial e paralela

Para cada estágio, escolha uma semântica:

- `todos`: conclui quando todos os aprovadores exigidos aprovarem;
- `qualquer um`: conclui na primeira aprovação válida e encerra atribuições restantes;
- `quórum`: conclui ao atingir quantidade ou proporção definida;
- serial: abre o próximo estágio apenas após concluir o atual;
- paralela: abre atribuições simultaneamente, com regra determinística para decisões conflitantes.

Defina se uma rejeição encerra todo o fluxo, apenas o estágio ou exige quórum de rejeição. Determine o tratamento de abstenção, aprovador indisponível, empate, remoção de usuário e alteração organizacional. Ao concluir ou cancelar um estágio, marque convites restantes como encerrados; não os deixe capazes de alterar o resultado.

## Rejeição, devolução e cancelamento

Não trate essas ações como sinônimos:

- rejeição expressa decisão desfavorável e normalmente encerra a revisão;
- devolução solicita correção, preserva histórico e exige nova revisão antes de ressubmeter;
- cancelamento ou retirada interrompe a solicitação por ator autorizado;
- expiração decorre de prazo e política, não de decisão humana;
- revogação de aprovação já concluída exige transição excepcional, motivo, autoridade e efeitos compensatórios.

Defina comentários obrigatórios, quem pode executar cada ação e quais efeitos precisam ser revertidos. Uma edição após devolução não pode reutilizar decisões vinculadas ao snapshot antigo, exceto para campos declarados não materiais e cobertos por regra explícita.

## Delegação, substituição e ausência

- Registre delegante, delegado, motivo, início, fim, escopos, tipos de operação e limites.
- Defina **autoridade própria do delegante** como as permissões e alçadas ativas que ele possui diretamente, sem contar a delegação avaliada nem autoridade recebida por outra delegação.
- Defina **autoridade própria do delegado** como as permissões e alçadas ativas que o ator delegado possui diretamente, fora da delegação avaliada.
- Defina **escopo delegado** como o subconjunto explícito concedido pelo delegante: ações, tipos de objeto, valores, moedas, empresas, centros de custo, atributos e intervalo de vigência.
- Calcule **autoridade efetiva** como `autoridade própria do delegante ∩ autoridade própria do delegado ∩ escopo delegado ∩ política e contexto da instância`. Use interseção em cada dimensão; nunca união, maior limite ou precedência permissiva.
- Trate a resolução completa da autoridade efetiva como gate bloqueador. Antes de aceitar a decisão ou executar qualquer mutação de estado, efeito ou outbox, carregue fontes autoritativas, valide vigência/revogação, resolva todas as dimensões e segregações e confirme que a ação cabe integralmente na interseção. Dado ausente, ambíguo, inconsistente ou fora do limite resulta em negação sem mutação de negócio.
- Faça a resolução dentro da unidade de consistência da decisão ou proteja-a contra mudança concorrente com versão/lock apropriado. Cache não pode substituir a verificação atual quando houver revogação ou mudança de alçada.
- Preserve na trilha a identidade real, a identidade representada e a regra de delegação usada.
- Detecte ciclos, sobreposições e delegação para pessoa com conflito de função.
- Defina se atribuições já abertas migram, permanecem ou são reavaliadas.
- Expire automaticamente a delegação e invalide caches de autorização.

Substituição automática por ausência deve seguir política aprovada. Não escolha um subordinado ou administrador apenas porque está disponível.

## Expiração e escalonamento

Defina prazo por estágio, calendário, feriados, fuso horário e instante de referência. Registre tempos em UTC e apresente-os no fuso autorizado.

- Lembretes não alteram o estado.
- Escalonamento adiciona ou troca aprovador somente conforme política e sem ampliar alçada indevidamente.
- Expiração não aprova automaticamente.
- O scheduler usa chave idempotente por instância, estágio e evento de prazo.
- Retries têm backoff, limite e destino para falha persistente; operadores podem reprocessar sem duplicar decisões.
- Uma ação no instante do vencimento é resolvida por regra transacional determinística, não pela ordem casual de duas requisições.

## Segregação de funções

Modele conflitos estáticos e dinâmicos. Exemplos a validar com o negócio:

- solicitante versus aprovador;
- criador/alterador de fornecedor versus aprovador do cadastro ou pagamento;
- lançador versus liberador de pagamento;
- administrador da regra versus aprovador de instância governada por essa regra;
- executor versus reconciliador ou auditor da mesma operação.

Aplique a restrição ao usuário real e à identidade representada/delegada. Contas administrativas não ficam isentas. Exceções emergenciais exigem aprovação adicional, prazo, justificativa e revisão posterior.

## Concorrência e idempotência

- Exija chave de idempotência por comando externo e armazene resultado e escopo associados.
- Use versão otimista ou atualização condicional por estado e versão esperados.
- Garanta unicidade para uma decisão final por atribuição e revisão, sem impedir que eventos históricos sejam acrescentados.
- Grave decisão, nova versão da instância, encerramento de atribuições e outbox em uma única transação local.
- Se efeitos externos não forem atômicos, publique via outbox e torne o consumidor idempotente; não mantenha transação de banco aberta aguardando rede.
- Em corrida entre aprovação, rejeição, cancelamento e expiração, apenas uma transição terminal pode vencer. As demais retornam conflito ou o resultado idempotente definido.
- Reavalie autorização e segregação dentro da unidade transacional, não apenas antes dela.

Não use bloqueio global do fluxo. Restrinja lock à instância ou agregado necessário e monitore contenção e deadlocks.

## Trilha resistente a adulteração

Registre, no mínimo:

- ID do evento, instante confiável em UTC, ator real e identidade efetiva;
- empresa/tenant, origem, IP/dispositivo quando permitido, sessão e `correlation_id`;
- objeto, revisão/hash, definição e versão da regra;
- estado anterior, comando, decisão, estado resultante e motivo;
- alçada, estágio, atribuição e delegação utilizadas;
- chave de idempotência e IDs dos efeitos/outbox.

Use armazenamento append-only com privilégios separados do fluxo operacional. Acrescente detecção de alteração e exclusão, como encadeamento de hashes, assinatura/lacre, armazenamento imutável ou cópia em repositório somente de escrita, conforme risco. Um hash guardado apenas ao lado do registro não protege contra um administrador que reescreva registro e hash; ancore ou replique a evidência fora da mesma fronteira quando essa ameaça fizer parte do modelo.

Registre e monitore o acesso à trilha. Não grave senha, token, segredo ou conteúdo pessoal desnecessário. Defina retenção, descarte autorizado, exportação e tratamento de correções de dados sem apagar a evidência operacional exigida.

## Fluxo de trabalho

1. Identifique o objeto, a revisão aprovada e o efeito de negócio que será liberado.
2. Levante alçadas, papéis, conflitos, prazos, exceções e significados das ações.
3. Desenhe estados, transições, guardas e estados terminais.
4. Modele definições versionadas e congele a versão na submissão.
5. Especifique estágios seriais/paralelos e regras de todos, qualquer um ou quórum.
6. Modele rejeição, devolução, cancelamento, alteração material e ressubmissão.
7. Aplique autorização, segregação, delegação e fallback sem privilégio implícito.
8. Implemente transições atômicas, idempotência, outbox e resolução de concorrência.
9. Implemente prazos, escalonamento e notificações como processos recuperáveis.
10. Produza auditoria resistente a alteração e execute testes de regras, falhas e ataques.

Antes de migrar um fluxo existente, reconcilie estados órfãos, aprovações sem autor, regras conflitantes e documentos alterados após aprovação. Não marque registros legados como aprovados por inferência silenciosa.

## Artefatos

Crie ou atualize somente os artefatos aplicáveis, adaptando nomes ao repositório. Marque como `N/A` cada item omitido e registre justificativa verificável; “não implementado” ou “não solicitado” não bastam quando o risco existe. Controles exigidos por uma transição ou delegação realmente presente não podem ser dispensados sem decisão e aceite de risco explícitos.

- glossário de ações e estados;
- diagrama/tabela da máquina de estados e catálogo de transições;
- matriz de alçadas com limites, precedência, vigência e fallback;
- matriz de segregação de funções e exceções;
- especificação da resolução de delegação, com as quatro entradas da interseção, fontes autoritativas e condições de bloqueio;
- modelo de dados de definição, instância, atribuição, decisão, delegação e auditoria;
- contrato de API/comandos com versão esperada e chave de idempotência;
- contratos de eventos e outbox;
- política de prazos, calendário, escalonamento e reprocessamento;
- modelo de ameaças e estratégia de evidência resistente a adulteração;
- plano de migração, rollback e reconciliação;
- suíte de testes automatizados e relatório de evidências.

Cada artefato aplicável deve indicar decisões, hipóteses, pendências, responsáveis e critérios de aceite. Dimensione o detalhe conforme quantidade de fluxos, níveis de alçada e impacto, sem gerar documentos vazios apenas para cumprir checklist.

## Testes obrigatórios

Selecione os testes proporcionalmente aos fluxos e mecanismos realmente existentes. Para cada caso abaixo não aplicável, registre `N/A` e a razão técnica; não omita funcionalidade existente nem aceite `N/A` apenas porque ainda não há teste preparado. Cubra ao menos os itens aplicáveis:

- toda transição válida e cada transição inválida por estado, ator ou revisão;
- valor abaixo, no limite e acima de cada alçada; moeda, arredondamento e regras sobrepostas;
- seleção sem regra, com múltiplas regras e fallback indisponível;
- estágios seriais, paralelos, todos, qualquer um, quórum, empate e rejeição concorrente;
- autoaprovação, papéis incompatíveis e tentativa via delegação ou conta administrativa;
- delegação vencida, futura, cíclica, sobreposta, revogada e acima da autoridade original;
- limites imediatamente abaixo, exatamente no limite e imediatamente acima da autoridade própria do delegante, da autoridade própria do delegado e do escopo delegado, variando também moeda, empresa, centro de custo, ação e início/fim da vigência;
- casos em que apenas uma, duas ou três partes da interseção autorizam, confirmando bloqueio total e ausência de mutação/outbox;
- mudança ou revogação concorrente da autoridade do delegante/delegado entre abertura e decisão, comprovando revalidação no gate;
- remoção do aprovador, mudança organizacional e alteração de configuração durante instância ativa;
- edição material e não material após submissão, devolução, ressubmissão e nova revisão;
- aprovação versus rejeição/cancelamento/expiração no mesmo instante;
- duas decisões simultâneas, retry após timeout e comandos com chave de idempotência repetida ou reutilizada em outro objeto;
- falha entre persistir decisão, atualizar estado, gerar outbox e consumir efeito externo;
- scheduler duplicado, atraso, indisponibilidade, mudança de fuso e horário de verão;
- acesso horizontal entre empresas e autorização vertical insuficiente;
- tentativa de gravar estado diretamente, forjar workflow/revisão ou executar transição fora de ordem;
- alteração/exclusão da auditoria, verificação de lacre e acesso indevido aos logs;
- recuperação após falha e reconciliação entre documento, instância, outbox e sistema externo.

Faça testes parametrizados da matriz sujeito × ação × objeto × estado × escopo. Teste efeitos colaterais: uma resposta de conflito não pode gerar e-mail, lançamento ou evento duplicado.

## Critérios de conclusão

Considere a tarefa concluída somente quando:

- estados, transições, guardas e efeitos estiverem documentados e implementados no servidor;
- cada instância estiver vinculada às versões imutáveis do objeto e do fluxo;
- limites e precedência das alçadas forem inequívocos e testados nas bordas;
- semântica de etapas seriais/paralelas e decisões conflitantes estiver definida;
- rejeição, devolução, cancelamento, delegação, expiração e escalonamento forem recuperáveis e auditáveis;
- segregação de funções abranger papéis, usuários reais, delegações, administradores e contas de serviço;
- toda decisão delegada passar pelo gate bloqueador e registrar a interseção resolvida sem ampliar nenhuma de suas quatro entradas;
- concorrência e retry não puderem produzir duas transições terminais nem efeitos duplicados;
- auditoria permitir reconstrução e detectar alteração ou exclusão conforme o modelo de ameaça;
- falhas de notificação ou integração não mudarem silenciosamente o resultado;
- testes negativos e de recuperação tiverem evidência executável;
- nenhuma alegação de conformidade jurídica tiver sido feita sem validação especializada e fonte oficial vigente.

## Referências oficiais de segurança

- [NIST — Separation of Duty](https://csrc.nist.gov/glossary/term/separation_of_duty)
- [NIST SP 800-53 Rev. 5 — Security and Privacy Controls](https://csrc.nist.gov/pubs/sp/800/53/r5/upd1/final)
- [OWASP — Authorization Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Authorization_Cheat_Sheet.html)
- [OWASP — Transaction Authorization Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Transaction_Authorization_Cheat_Sheet.html)
- [OWASP — REST Security Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/REST_Security_Cheat_Sheet.html)
- [OWASP — Logging Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Logging_Cheat_Sheet.html)
