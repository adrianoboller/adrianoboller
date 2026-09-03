# Regras do projeto de conversão WX

Este arquivo deve ser copiado para a raiz do repositório de destino.

## Objetivo

Reconstruir o comportamento comprovado do projeto WINDEV, WEBDEV ou WINDEV Mobile na tecnologia-alvo definida pelo usuário. Não fazer tradução linha a linha nem inventar regras ausentes.

## Regras obrigatórias

1. Trate os anexos originais como somente leitura. Grave todo material gerado em `.wx-migration/` e o novo produto nos diretórios definidos pela arquitetura aprovada.
2. Antes de escrever código, execute `/wx-claude-code:converter` no plugin ou `/wx-claude-code:questionario` e conclua o Gate G0 de intake.
3. Não considere um arquivo “fornecido” sem comprovar que o Claude Code consegue lê-lo localmente.
4. O corpus WLanguage 12k incluído é uma fonte técnica auxiliar e degradada; ele não é fonte de regras de negócio nem prova suficiente de compatibilidade.
5. Toda regra, consulta, tela, integração e comportamento migrado deve possuir origem localizável e teste de equivalência.
6. Registre incertezas em `.wx-migration/gaps.md`. Nunca transforme suposição em requisito silenciosamente.
7. Diante de conflito entre evidências, interrompa apenas o item afetado, registre o conflito e peça decisão humana.
8. Preserve precisão numérica, nulidade, datas, fusos, collations, transações, bloqueios, concorrência, permissões e regras fiscais.
9. Não exponha senhas, tokens, certificados, dados pessoais ou dados de produção. Use placeholders e dados anonimizados.
10. Não altere o legado para fazê-lo concordar com a migração. A equivalência é demonstrada por testes, snapshots, consultas e critérios aceitos.

## Hierarquia de evidências

Use esta ordem para detectar conflitos, sem resolver divergências automaticamente:

1. Decisão humana registrada e critério de aceite aprovado.
2. Comportamento observável do sistema legado e resultados reproduzíveis.
3. Código, queries, schema SQL, triggers e integrações do legado.
4. Regras de negócio documentadas.
5. Telas, relatórios, imagens e fluxos.
6. Help WLanguage e documentação externa, apenas para semântica técnica.

## Identificadores de rastreabilidade

Use `BR-` para regras, `QRY-` para consultas, `DB-` para banco, `UI-` para interface, `INT-` para integrações, `RPT-` para relatórios e `NFR-` para requisitos não funcionais. Use `SRC-` para fontes, `DEC-` para decisões, `GAP-` para lacunas e `TST-` para testes; estes quatro são referências auxiliares, não IDs principais da matriz.

## Condição de conclusão

Uma funcionalidade só está concluída quando possui evidência de origem, implementação, teste automatizado ou roteiro reproduzível, resultado comparativo e risco residual registrado.

Qualquer afirmação de equivalência vale somente para o escopo, build, configuração, ambiente, dataset e tolerâncias registrados. Este processo não certifica conformidade LGPD ou jurídica.

## Estilo de resposta

- Dê a resposta na primeira frase.
- Use frases curtas e um assunto por parágrafo.
- Descreva o problema em uma linha.
- Apresente a solução em passos numerados.
- Explique cada termo técnico na primeira vez que aparecer, em uma frase.
- Se faltar informação necessária para executar, pergunte antes de agir.
