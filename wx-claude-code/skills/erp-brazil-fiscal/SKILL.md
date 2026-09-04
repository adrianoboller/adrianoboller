---
name: erp-brazil-fiscal
description: "Módulo fiscal brasileiro de ERP: tributação, NF-e, NFC-e, CT-e, NFS-e, SPED, com fontes oficiais e vigências."
metadata:
  short-description: ERP fiscal brasileiro rastreável e versionado
---

# ERP fiscal brasileiro

Construa software fiscal comprovável, parametrizado por vigência e rastreável
até fontes oficiais. Trate regras fiscais, códigos, leiautes, endpoints e prazos
como dados externos mutáveis, nunca como conhecimento permanente do modelo.

Esta skill orienta engenharia de software. Ela não presta consultoria tributária
nem certifica conformidade legal. Decisões materiais devem ser aprovadas pelo
responsável fiscal ou contábil da empresa.

## Barreiras obrigatórias

Antes de propor regra, cálculo, código fiscal, XML, endpoint ou fluxo operacional,
registre explicitamente os itens abaixo. Quando um item realmente não incidir no
escopo, registre **N/A**, a justificativa e a evidência que sustenta a exclusão:

1. **UF** do estabelecimento e, quando aplicável, origem e destino da operação.
2. **Município** e inscrição municipal quando houver serviço ou NFS-e.
3. **Regime tributário**, incluindo exceções ou regimes especiais e sua vigência.
4. **Data e hora de vigência** da operação ou período de apuração, com fuso.
5. **Documento ou obrigação** no escopo: NF-e, NFC-e, CT-e, NFS-e ou módulo SPED.
6. **Ambiente**: desenvolvimento local, produção restrita/homologação ou produção.

Se qualquer item estiver ausente e não houver **N/A** justificado, pare a
determinação fiscal e faça perguntas objetivas. É permitido discutir arquitetura
genérica, mas marque-a como independente de regra fiscal. Não preencha lacunas
por memória ou suposição.

Depois do conjunto mínimo, confirme também, conforme o caso:

- empresa, estabelecimento, CNPJ, IE, IM e CNAE relevantes;
- papel da empresa na operação, origem, destino, destinatário e consumidor final;
- produto, mercadoria, serviço, transporte e classificações fornecidas;
- finalidade da emissão e documento referenciado;
- credenciamento, autoridade autorizadora e produto municipal habilitado;
- versão do leiaute, esquema, nota técnica e data de implantação em cada ambiente;
- certificado aceito, procurações e permissões, sem solicitar a chave privada;
- obrigação acessória, perfil, periodicidade e período de referência;
- responsável fiscal que aprovará a interpretação e a entrada em produção.

Quando o usuário disser “atual”, converta isso em uma data e hora explícitas e
faça nova consulta. Para documento histórico, use a regra vigente na data do fato,
não a regra mais recente.

## Proporcionalidade e aplicabilidade

Aplique controles, testes e artefatos na proporção do risco e do escopo solicitado.
Uma revisão de cálculo não exige implementar transmissão; uma integração NF-e não
torna SPED ou NFS-e automaticamente aplicáveis. Para cada seção desta skill:

- classifique cada requisito como aplicável ou **N/A**;
- justifique **N/A** por escopo, documento, jurisdição, ambiente e fonte;
- não use **N/A** para ocultar entrada desconhecida, risco não analisado ou trabalho
  necessário ainda pendente;
- aprofunde requisitos quando houver emissão real, efeito fiscal, produção,
  segredo, dado pessoal, múltiplas empresas ou obrigação acessória;
- mantenha sempre as barreiras de não invenção, evidência oficial, isolamento e
  preservação de artefatos fiscais que efetivamente existirem.

Registre essa matriz de aplicabilidade no contexto ou na entrega. Critérios não
aplicáveis não bloqueiam conclusão quando o **N/A** estiver fundamentado e aprovado
pelo responsável adequado.

## Política de evidência

Nunca invente nem estime:

- alíquota, base, benefício, redução, crédito, retenção ou arredondamento;
- CFOP, CST, CSOSN, NCM, CEST, NBS, código de serviço ou classificação tributária;
- código ou significado de rejeição, evento, prazo ou sequência permitida;
- tag, cardinalidade, namespace, XSD, assinatura, QR Code ou conteúdo de XML;
- URL, hostname, operação, versão, certificado TLS ou endpoint de web service/API;
- regra de numeração, série, cancelamento, inutilização ou contingência;
- obrigação, leiaute, registro, bloco, campo ou regra de validação do SPED.

Para cada decisão fiscal ou técnica:

1. Consulte a documentação oficial aplicável à jurisdição, documento, ambiente e
   vigência.
2. Leia a fonte primária; resultado de busca, resumo, blog, fórum, fornecedor e
   memória do modelo não são evidência normativa.
3. Registre autoridade, título, URL, versão/revisão, seção, publicação, vigência,
   ambiente e data/hora da consulta.
4. Relacione a regra implementada ao identificador dessa fonte.
5. Preserve versões substituídas para reproduzir documentos e apurações antigos.
6. Se a fonte estiver indisponível, ambígua ou conflitante, declare bloqueio,
   registre a divergência e peça o documento oficial ou decisão qualificada.

Não escolha sozinho entre interpretações legais conflitantes. Apresente o conflito
com impacto, fontes e vigências ao responsável fiscal. Uma aprovação humana não
dispensa os testes técnicos.

## Fontes oficiais primárias

Antes de determinar qualquer regra fiscal, leia
[references/official-sources.md](references/official-sources.md) e consulte as
seções correspondentes ao documento, obrigação, UF e município do escopo.

Use como autoridades: Receita Federal e SPED para tributos e obrigações federais;
CONFAZ, portais nacionais de DF-e e SEFAZ competente para ICMS e documentos
estaduais; Portal Nacional da NFS-e e administração tributária municipal para
ISSQN/NFS-e; ITI e manual específico para ICP-Brasil e assinatura. Revalide fonte,
versão, vigência e ambiente a cada determinação. Fonte nacional não substitui a
regra territorial aplicável.

## Registro de fontes

Mantenha um catálogo versionado e legível por máquina. Cada registro deve conter:

- **source_id** imutável;
- órgão/autoridade e jurisdição;
- documento ou obrigação coberta;
- título, URL oficial e seção utilizada;
- versão, revisão e identificador do ato ou nota;
- data de publicação;
- início e fim de vigência jurídica, quando disponíveis;
- início da implantação em homologação e em produção;
- data/hora UTC da consulta;
- estado: proposta, vigente, futura, substituída, revogada ou não confirmada;
- hash do artefato baixado quando a integridade do arquivo for relevante;
- regras, testes, configurações e decisões que dependem da fonte.

Não sobrescreva histórico. Uma correção cria nova versão e relaciona a antecessora.
Marque separadamente vigência jurídica, versão técnica e disponibilidade por
ambiente; elas podem começar em datas diferentes.

## Arquitetura fiscal

Use um núcleo fiscal separado da interface, vendas, estoque e financeiro:

- **Contexto fiscal**: resolve empresa, estabelecimento, jurisdição, regime,
  operação, competência e data de vigência.
- **Catálogo versionado**: mantém classificações e parâmetros com fonte,
  aprovação, início e fim de validade.
- **Motor de determinação**: produz resultado explicável sem efeitos colaterais.
- **Construtor de documento**: converte a decisão aprovada no leiaute oficial.
- **Validador**: aplica esquema oficial e validações internas rastreáveis.
- **Assinador**: acessa a chave por interface segura sem expor o segredo.
- **Adaptador autorizador**: um por documento, autoridade e ambiente.
- **Máquina de estados**: registra cada transição fiscal e evento oficial.
- **Outbox/inbox**: desacopla transação local de transmissão externa.
- **Reconciliador**: resolve respostas tardias, timeouts e divergências.
- **Exportador SPED**: transforma livros e fatos imutáveis no leiaute da competência.

Não acople tela ou pedido diretamente ao web service fiscal. Não misture regra de
tributação, serialização XML, transporte e interpretação de retorno na mesma
rotina. Um modelo canônico interno é útil, mas não deve apagar dados exigidos por
um documento ou jurisdição.

## Tributação parametrizada e versionada

Cada regra deve possuir, no mínimo:

- identificador e versão imutáveis;
- empresa/filial ou escopo de compartilhamento explicitamente aprovado;
- jurisdição, regime, documento e tipo de operação;
- critérios de entrada e resultado;
- início/fim de validade e prioridade sem sobreposição ambígua;
- fonte oficial e seção;
- autor, revisor, aprovação e datas;
- conjunto de testes e impacto esperado.

Use aritmética decimal exata. Escala, arredondamento, ordem de cálculo e tolerância
devem vir da fonte aplicável. Guarde entradas, regra resolvida, passos do cálculo,
resultado e versão para explicação e reprocessamento. Não use ponto flutuante
binário para valores fiscais.

Mudança futura não altera documentos passados. Publique nova versão com ativação
pela vigência, execute comparação em homologação e exija revisão em quatro olhos.
Não permita edição direta em produção sem trilha e aprovação.

## Ciclo de documentos fiscais eletrônicos

Modele estados conforme a documentação oficial do documento. Preserve ao menos a
distinção entre intenção local, validado, assinado, enfileirado, transmitido,
resultado desconhecido, autorizado, rejeitado e eventos posteriores. Não trate
HTTP 200, recebimento de lote, impressão de auxiliar ou timeout como autorização.
A resposta oficial aplicável é a fonte do estado fiscal.

Fluxo-base:

1. Crie a intenção fiscal e uma identidade estável para o comando interno.
2. Resolva regras pelo contexto e pela data do fato.
3. Registre a memória de cálculo explicável.
4. Aloque série e número atomicamente no escopo definido pela fonte oficial.
5. Gere o conteúdo e valide contra o esquema oficial correto.
6. Assine somente após a validação e não altere os bytes assinados.
7. Calcule hash, grave o artefato e publique a transmissão via outbox.
8. Persista requisição, resposta, protocolo, timestamps, ambiente e correlação.
9. Interprete códigos usando a documentação da versão aplicável.
10. Em resultado desconhecido, consulte/reconcilie antes de retransmitir.
11. Armazene XML e protocolo no formato processado exigido, sem reconstruí-los.
12. Propague efeitos para estoque, financeiro e contabilidade apenas a partir de
    estados explicitamente aceitos pelo domínio e pela regra oficial.

Use restrições únicas para impedir duplicidade por empresa, estabelecimento,
documento, modelo, série, número, ambiente e identificadores oficiais pertinentes.
Nunca reutilize número por simples exclusão local. Registre lacunas e processe
inutilização somente quando o documento e a autoridade a permitirem.

### Eventos e correções

Autorização, rejeição, cancelamento, inutilização, carta/evento de correção e
demais eventos não são intercambiáveis. Para cada ação:

- confirme se existe para o documento e a jurisdição;
- valide prazo, sequência, autoria, justificativa e pré-condições oficiais;
- gere e assine o evento no leiaute correto;
- preserve o documento autorizado e anexe o protocolo do evento;
- torne o comando interno deduplicável e reconciliável, sem presumir suporte
  idempotente do sistema externo;
- impeça “correção” por atualização direta do XML ou registro autorizado.

### Contingência

Não implemente uma contingência genérica. Verifique modalidade, disponibilidade,
prazo, impressão, marcações, assinatura, transmissão posterior e reconciliação
para o documento, UF, versão e ambiente exatos.

Ao ativar contingência, registre motivo, responsável, início, modo, documentos
afetados e evidência operacional. Após restabelecimento, transmita no prazo
oficial, consulte estados antes de repetir, reconcilie todos os documentos e
encerre formalmente o incidente. Um modo de contingência não pode vazar para
outra empresa, filial, documento ou ambiente.

## Idempotência, filas e reconciliação

Todo comando interno capaz de causar efeito fiscal deve receber uma identidade
estável antes da primeira tentativa. Grave pedido fiscal e evento de outbox na
mesma transação local. O consumidor deve registrar inbox/deduplicação antes de
aplicar efeitos repetíveis.

Não presuma que SEFAZ, ambiente nacional, município ou provedor aceite uma chave
de idempotência enviada pelo ERP. Só atribua garantia idempotente ao sistema
externo quando o protocolo oficial aplicável documentar expressamente o mecanismo,
seu escopo e sua semântica. Sem esse suporte, a proteção depende da identidade
interna, dos identificadores fiscais oficiais, de restrições únicas, do artefato
assinado preservado e de consulta/reconciliação antes de retransmitir.

### Gate antes de retry

Bloqueie qualquer retry ou reprocessamento até registrar e conferir:

1. semântica fiscal oficial do código/retorno na versão aplicável: rejeição ou
   erro funcional, processamento assíncrono, falha transitória de transporte,
   indisponibilidade, falha permanente ou resultado desconhecido;
2. empresa e estabelecimento exatos;
3. ambiente exato;
4. tipo de documento e, quando aplicável, modelo, série, número, chave, recibo,
   protocolo e identificador de evento;
5. artefato transmitido, sua versão e hash; para conteúdo assinado, confirme que
   os bytes permanecem idênticos;
6. estado local atual, outbox/inbox, correlação e efeitos já aplicados;
7. histórico completo de tentativas, requisições, respostas e timestamps;
8. para resultado desconhecido, consulta oficial e reconciliação executadas ou
   impossibilidade documentada.

Erro funcional, validação de negócio ou rejeição fiscal não entra em mecanismo
genérico de retry. Corrija a causa e siga o fluxo oficial correspondente; quando
isso exigir novo artefato ou nova tentativa fiscal, crie um comando auditável em
vez de reenviar cegamente. Somente falha transitória confirmada pode seguir uma
política automática de retry. Resultado desconhecido exige consulta/reconciliação,
não retransmissão especulativa.

Classifique falhas em:

- **negócio, validação ou rejeição fiscal**: não repetir automaticamente nem
  colocar em retry genérico; corrigir a causa e aplicar o fluxo oficial;
- **transitória confirmada**: repetir com backoff, jitter e limite documentado;
- **resultado desconhecido**: consultar a autoridade antes de qualquer reenvio;
- **permanente técnica**: bloquear, alertar e encaminhar para intervenção;
- **segurança**: interromper, preservar evidência e acionar o runbook.

Respeite limites e orientações oficiais. Use fila de quarentena após o limite de
tentativas. Tenha circuit breaker e pausa manual por empresa/documento/ambiente,
sem descartar mensagens. O painel deve mostrar idade da fila, tentativas, último
erro, próximo passo e responsável, sempre com dados sensíveis reduzidos.

Execute reconciliação periódica e sob demanda entre:

- intenções locais, outbox, inbox e máquina de estados;
- respostas/protocolos oficiais e documentos armazenados;
- numeração emitida, autorizada, cancelada, inutilizada e pendente;
- documentos fiscais, estoque, financeiro, contabilidade e livros;
- arquivos SPED, recibos e dados-fonte da competência.

Não “conserte” divergência silenciosamente. Gere caso auditável, preserve antes e
depois e exija aprovação conforme materialidade.

## SPED

Não presuma quais módulos obrigam uma empresa. Determine obrigação, perfil,
periodicidade, leiaute e competência com fonte oficial e validação fiscal.
EFD ICMS/IPI, EFD-Contribuições, ECD, ECF, EFD-Reinf e outros módulos entram
somente quando confirmados no escopo.

Para cada arquivo:

1. Fixe empresa, período, obrigação, perfil, versão do leiaute e fontes.
2. Mapeie cada registro e campo ao dado de origem e à regra que o produziu.
3. Gere de livros e fatos imutáveis; não dependa de totais digitados sem origem.
4. Valide estrutura, cardinalidade, códigos, totais e cruzamentos documentados.
5. Use o programa validador oficial quando aplicável e guarde a evidência.
6. Reconcilie com DF-e, estoque, financeiro, contabilidade e declarações correlatas.
7. Preserve arquivo entregue, assinatura, recibo, hash, logs e versão do gerador.
8. Trate retificação ou substituição pelo processo oficial, nunca sobrescrevendo
   a entrega anterior.

O mesmo conjunto de entradas, regras e versão deve reproduzir o mesmo arquivo.
Qualquer ajuste manual deve possuir autor, motivo, fonte, aprovação e trilha.

## Certificados, segredos e dados

- Nunca grave chave privada, senha, token ou certificado com segredo em código,
  repositório, configuração aberta, prompt, log, telemetria ou ticket.
- Use cofre de segredos, HSM, keystore do sistema ou mecanismo equivalente,
  conforme o tipo de certificado e a arquitetura aprovada.
- Isole credenciais por empresa e ambiente. Acesso deve seguir menor privilégio,
  segregação de funções, autenticação forte e auditoria.
- Não exporte chave privada quando a política exigir não exportabilidade.
- Valide cadeia ICP-Brasil, finalidade, titularidade, validade e revogação conforme
  as políticas do ITI e o manual do documento.
- Monitore expiração com antecedência e teste renovação sem trocar silenciosamente
  a identidade fiscal.
- Exija TLS válido; nunca desative validação de certificado para “fazer funcionar”.
- Reduza CPF, CNPJ, endereço, XML e conteúdo fiscal em logs e mensagens de erro.
- Criptografe dados sensíveis em trânsito e repouso e controle acesso a XMLs.
- Sincronize relógio e registre timestamps com fuso e origem confiável.
- Determine retenção e descarte por obrigação e legislação vigente; não invente
  um prazo único. Backups devem ser cifrados e restaurações testadas.

## Isolamento, autorização e auditoria

Toda consulta e mutação deve carregar empresa, filial e ambiente no contexto.
Autorize no servidor e negue por padrão. Impedir acesso cruzado deve ser testado
no banco, serviço, fila, armazenamento, cache, pesquisa e observabilidade.

Separe permissões de parametrizar, aprovar, emitir, cancelar, operar contingência,
reconciliar e consultar dados sensíveis. Mudanças críticas exigem quatro olhos.

A trilha deve ser append-only e registrar ator, data/hora, empresa, ambiente,
ação, motivo, estado anterior/posterior, correlação, versão da regra e fonte,
sem armazenar segredos. Não permita que o mesmo operador apague sua própria
evidência.

## Workflow de mudança fiscal

Para cada alteração legal ou técnica:

1. Registre a fonte e classifique proposta, publicada, futura ou vigente.
2. Compare versões de ato, manual, XSD, tabela e endpoint.
3. Produza análise de impacto por empresa, UF, município, documento e ambiente.
4. Crie parâmetros e código compatíveis com a vigência antiga e a nova.
5. Adicione testes de fronteira imediatamente antes e depois da mudança.
6. Valide esquema e contratos em produção restrita/homologação.
7. Execute comparação paralela com casos representativos e anonimizados.
8. Obtenha aprovação técnica e fiscal.
9. Ative por data, empresa e ambiente, com observabilidade e plano de reversão.
10. Registre evidências e acompanhe rejeições, filas e reconciliações.

Reverter software não pode apagar documento, protocolo ou evento fiscal ocorrido.
Se o calendário oficial mudar, crie outra versão; não edite o histórico.

## Testes aplicáveis

Mantenha matriz por documento, UF/município, regime, vigência, versão e ambiente.
Execute os grupos aplicáveis ao escopo e marque os demais como **N/A** justificado.
Quando aplicáveis, inclua:

- unidade: seleção de regra, limites de vigência, precisão e arredondamento;
- propriedade/invariantes: totais e relações apenas quando oficialmente definidos;
- esquema: XML válido e inválido contra o pacote oficial exato;
- assinatura: cadeia, certificado vencido/revogado quando simulável e imutabilidade;
- contrato: requisição/resposta no ambiente oficial de testes;
- ciclo: autorização, rejeições documentadas e eventos do escopo;
- resiliência: timeout antes/depois do envio, duplicidade, resposta tardia,
  indisponibilidade, retry, quarentena e circuit breaker;
- numeração: concorrência, lacunas, série, idempotência e reconciliação;
- contingência: ativação, emissão, impressão, transmissão posterior e encerramento,
  apenas nas modalidades oficialmente suportadas;
- NFS-e: município aderente/não aderente, produto habilitado e versões divergentes;
- SPED: leiaute, programa validador oficial quando aplicável, recibo e cruzamentos;
- segurança: isolamento entre empresas, RBAC, redaction de logs e rotação;
- operação: backup/restauração, renovação de certificado e reprocessamento seguro.

Use dados sintéticos e fixtures anonimizadas. Não use certificado, segredo ou dado
real de produção em testes. Não transmita documento real em produção sem
autorização explícita da empresa e procedimento fiscal aprovado.

Homologação não prova conformidade total: alguns ambientes possuem comportamento
ou dados limitados. Registre exatamente o que foi observado, sem generalizar.

## Artefatos aplicáveis

Produza ou atualize somente os artefatos necessários ao escopo. Na matriz de
entrega, marque os demais como **N/A** e justifique. Artefatos possíveis:

- **docs/fiscal/context.md**: entradas, empresas, jurisdições, regimes e vigências;
- **docs/fiscal/source-registry.yaml**: catálogo oficial e histórico de fontes;
- **docs/fiscal/document-matrix.md**: documentos, autoridades, ambientes e versões;
- **docs/fiscal/tax-rule-catalog.yaml**: regras parametrizadas e rastreabilidade;
- **docs/fiscal/state-machines.md**: estados, eventos e invariantes por documento;
- **docs/fiscal/integrations.md**: contratos, endpoints confirmados e segurança;
- **docs/fiscal/sped-mapping.md**: origem de cada registro/campo e reconciliações;
- **docs/fiscal/decisions/**: decisões, conflitos, aprovações e impacto;
- **docs/fiscal/runbooks/**: indisponibilidade, contingência, certificado,
  reconciliação, filas e incidente de segurança;
- **docs/fiscal/test-plan.md** e **docs/fiscal/evidence/**: matriz e provas recentes;
- migrações de banco, esquemas de configuração e trilhas de auditoria;
- painel operacional com filas, disponibilidade, certificados e divergências.

Não gere arquivo “decorativo”. Cada artefato deve possuir responsável, data de
revisão, fontes e consumidores.

## Formato da resposta ao usar esta skill

Comece por:

1. **Escopo confirmado**.
2. **Dados faltantes ou bloqueios**.
3. **Fontes oficiais consultadas**, com versão, vigência e data da consulta.
4. **Decisão e implementação proposta**, separando fato, interpretação e hipótese.
5. **Riscos e validações humanas necessárias**.
6. **Testes e evidências**.

Em código, comente a intenção e referencie o **source_id**, não copie longos
trechos legais. Em revisão, aponte regra sem fonte, vigência ausente, endpoint
fixo, segredo exposto, XML reconstruído, duplicidade possível e estado fiscal
ambíguo como bloqueadores.

## Critérios de conclusão

Só declare o trabalho pronto quando houver evidência recente de que:

- as seis entradas obrigatórias estão registradas ou marcadas **N/A** com
  justificativa válida, e o responsável fiscal aplicável está identificado;
- toda regra, código, prazo, leiaute e endpoint possui fonte oficial e vigência;
- versões antigas e futuras coexistem sem alterar documentos históricos;
- cálculo aplicável é decimal, explicável, reproduzível e aprovado;
- XML aplicável usa XSD correto, assinatura válida e bytes assinados imutáveis;
- autorização e eventos aplicáveis preservam artefato, protocolo, resposta, hash
  e correlação;
- identidade interna, outbox/inbox, gate de retry e resultado desconhecido foram
  testados quando houver integração assíncrona ou mutável;
- numeração, cancelamento, inutilização e contingência aplicáveis seguem o escopo
  oficial;
- NFS-e, quando no escopo, considera adesão, produto e autoridade municipal;
- obrigações SPED, quando no escopo, estão confirmadas, mapeadas, validadas e
  reconciliadas;
- controles aplicáveis de segredos, certificados, RBAC, isolamento e auditoria
  passaram nos testes;
- homologação cobriu a matriz aplicável e suas limitações foram documentadas;
- runbooks, alertas, backup/restauração e reconciliação aplicáveis foram
  exercitados;
- entrada em produção, quando autorizada, tem aprovação técnica/fiscal,
  observabilidade e reversão;
- todos os itens fora do escopo possuem **N/A** justificado, sem mascarar pendência.

“Testes verdes” não significam conformidade legal. Declare apenas o que a
evidência demonstra e informe claramente qualquer item ainda não validado.
