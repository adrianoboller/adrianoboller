# Requisitos gráficos — v0.3+

## Requisitos pétreos da UI

- nitidez HiDPI/Retina/4K;
- Dark e Light;
- mouse, trackpad e touch;
- zoom e pan fluidos;
- seleção persistente;
- labels progressivos para reduzir poluição visual;
- destaque do contexto selecionado;
- exportação em alta qualidade;
- nenhum diagrama deve depender de hover para a função principal;
- responsividade em desktop, tablet e mobile.

## MindSet

- SVG vetorial;
- curvas Bézier;
- hierarquia visual por profundidade;
- cores por domínio/tipo;
- nós com título + informação secundária;
- seleção sincronizada.

## DER

- cartões de tabela com schema, campos e metadados;
- PK/FK diferenciadas;
- cardinalidades;
- conectores ortogonais no motor ELK;
- minimização de cruzamentos;
- foco por vizinhança para schemas grandes.

## Obsidian

- WebGL quando possível;
- ForceAtlas2;
- clusterização visual por migration;
- nó dimensionado por conectividade;
- labels por importância/zoom;
- câmera e foco na seleção;
- fallback Canvas local.

## Hybrid

- tabela central dominante;
- primeiro anel relacional;
- segundo anel de contexto SQL;
- leitura do centro para fora.
