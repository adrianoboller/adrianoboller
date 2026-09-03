# Pontos críticos de WLanguage

Use o corpus bundled como índice técnico local e qualquer Help específico da release como override. O corpus possui lacunas conhecidas e metadados amplos de versão; não trate a presença de uma página como prova de compatibilidade. Confirme símbolos críticos pela release, comportamento do legado e documentação oficial. Nunca substitua evidência por memória do modelo.

## Contexto de execução

Antes de mapear um trecho, determine:

- produto: WINDEV, WEBDEV ou WINDEV Mobile;
- evento: inicialização, declaração, entrada, saída, clique, alteração, timer, fechamento ou outro;
- lado de execução em WEBDEV: servidor ou navegador;
- ciclo de vida e estado compartilhado: projeto, janela/página, sessão, request, thread ou dispositivo;
- modo síncrono/assíncrono e efeitos colaterais.

Código idêntico em eventos diferentes pode ter semântica diferente.

## Tipos e operadores

Verifique na versão fornecida:

- conversões implícitas, variantes, strings Unicode/ANSI e buffers;
- datas, horas, durações, fusos e valores inválidos/vazios;
- decimais, moedas, arredondamento, divisão e overflow;
- arrays, associative arrays, structures, classes, referências e cópias;
- `Null`, vazio, zero e valor padrão;
- exceções, erros retornados, `ErrorInfo` e comportamento de funções que falham.

Não traduza operadores até existir teste de borda.

## Dados e queries

Mapeie chamadas `H*`, fontes de dados, queries parametrizadas, filtros, navegação, buffers de registro, locks, transações e conexão. Preserve:

- ordenação e collation;
- distinção entre ausência e `NULL`;
- leitura concorrente e bloqueios;
- identidade/sequências;
- triggers, constraints e defaults;
- commit, rollback e recuperação após falha;
- paginação e limites.

Compare o SQL exportado com queries embutidas e código que as monta dinamicamente.

## Interface

Extraia propriedades e eventos de cada controle, ordem de tabulação, anchors, visibilidade, habilitação, máscaras, validação, mensagens, atalhos, menus e bindings. Screenshots mostram aparência, não necessariamente comportamento ou acessibilidade.

## Especificidades por produto

- WEBDEV: sessão, request, cookies, código servidor/navegador, AJAX, upload, cache e publicação.
- Mobile: offline, sincronização, permissões, ciclo de vida, armazenamento seguro, câmera, GPS, push e restrições das lojas.
- Desktop: múltiplas janelas/monitores, impressão, arquivos locais, serviços, COM/DLL e integrações do sistema operacional.

## Recursos frequentemente omitidos em PDF

Procure explicitamente: código global, procedures internas, eventos de controles, queries do editor, análise de dados, relatórios, estados, estilos, traduções, componentes externos, configurações de projeto e geração. Se não houver exportação desses itens, crie lacunas; não suponha defaults.
