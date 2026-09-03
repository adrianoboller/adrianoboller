# Fluxo de conversão

## Modos

- `inventario`: G0–G1. Não gera arquitetura nem código.
- `plano`: G0–G3. Produz especificação, ADRs, estimativa por ondas e piloto proposto.
- `piloto`: G0–G4. Implementa uma fatia vertical representativa.
- `completo`: G0–G7. Exige aprovação entre gates e nunca começa como reescrita integral.

## G1 — normalização das evidências

1. Preserve originais e hashes.
2. Extraia PDFs com layout; use OCR somente quando necessário e sinalize texto OCR.
3. Execute `query_wlanguage_help.py --verify` no corpus bundled e registre suas quarentenas/lacunas; use `build_help_index.py` somente para overrides específicos da release.
4. Catalogue cada tela, controle, evento, procedure, classe, query, tabela, relatório e integração.
5. Gere localizadores estáveis: arquivo + página, JSON Pointer, linha SQL, nome legado ou região da imagem.
6. Produza matriz `presente | parcial | ausente | ilegível | conflitante`.

Leia [wlanguage-semantics.md](wlanguage-semantics.md) antes de interpretar código.
Use [official-sources.md](official-sources.md) para confirmar a release e cobrir lacunas; registre divergências entre corpus, Help específico, código e comportamento observado sem escolher silenciosamente uma fonte.

## G2 — especificação comportamental

Reconstrua comportamento, não sintaxe:

- gatilho, pré-condições, entradas, transformações, saídas e efeitos colaterais;
- validações, mensagens, permissões, auditoria e exceções;
- estados da tela e fluxo de navegação;
- queries, parâmetros, ordenação, paginação, nulidade e precisão;
- transações, locks, idempotência, retries e comportamento em falha;
- integrações, contratos, timeouts e operação offline;
- relatórios, exportações e tarefas agendadas.

Para cada regra, cite evidência. Se duas fontes discordarem, crie `GAP-*` e uma pergunta; não escolha a versão conveniente.

## G3 — arquitetura-alvo

Produza ADRs para:

- linguagem, frameworks e versões suportadas;
- separação domínio/aplicação/infraestrutura/UI;
- modelo de dados e estratégia de compatibilidade;
- autenticação, autorização, segredos e LGPD;
- observabilidade, configuração, deployment e rollback;
- testes, golden masters e critérios de equivalência;
- estratégia incremental: strangler, módulos paralelos ou substituição controlada.

Inclua threat model, privacidade, capacidade, observabilidade e operabilidade já em G3 e no piloto. Defina matriz real de navegador/SO/dispositivo/DPI/tema/locale e WCAG quando aplicável.

Mapeie cada dependência WX a `equivalente`, `adaptar`, `substituir`, `encapsular` ou `remover com aprovação`.

## G4 — piloto vertical

Escolha uma fatia que contenha UI, regra, query, persistência e ao menos uma condição de erro. Evite a tela mais simples e o núcleo mais crítico. O piloto deve comprovar:

- execução reproduzível;
- dados de teste fixos;
- resultado legado capturado;
- resultado novo comparado;
- diferença visual/funcional explicada;
- esforço real usado para recalibrar o plano.

O contrato de equivalência do piloto declara build/configuração do legado, ambiente, dataset, tolerâncias e dimensões verificadas: regras, dados/efeitos colaterais, UI, permissões, relatórios, integrações, desempenho e recuperação. Bugs conhecidos, vulnerabilidades ou comportamento ilegal não são preservados sem decisão explícita.

Falha no piloto retorna a G2 ou G3; não escale a implementação.

## G5–G7 — implementação, endurecimento e corte

Implemente por ondas pequenas. Cada módulo passa por testes unitários, integração, contrato, banco e fluxo de usuário antes do próximo. Mantenha migrações de banco versionadas, reversíveis quando possível e ensaiadas em cópia anonimizada.

Antes do corte, execute perfil de qualidade/volume, mapeamento campo a campo, deduplicação, quarentena de rejeitados, cargas idempotentes/retomáveis, checkpoints, CDC/delta quando necessário, reseed, freeze, contagens, checksums por faixa/tenant/período, invariantes, consultas críticas, permissões, desempenho, backup/restore e rollback ou forward-fix. O aceite humano registra limites conhecidos e plano de suporte.
