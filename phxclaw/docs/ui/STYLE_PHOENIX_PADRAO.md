# Style Phoenix Padrão

Referência visual definida pelo dono em 01/10/2026: as telas do console web do PhxSql
(pacote `telas-svg.zip`, commit `6fc8244e` do PhxSql, 79 telas × 2 temas). Os SVGs são
**capturas** (PNG dentro de `<image>`), então fontes, tokens e ícones foram lidos no fonte que as
gera: `phxsql/crates/phxsql-server/ui/index.html` (1,2 MB com os módulos).

## O que define o padrão

| Elemento | Como é | Onde está |
|---|---|---|
| Estrutura | cabeçalho (logo, versão, ajuda, tema, usuário, Sair) → barra de menus com mnemônico sublinhado (Arquivo, Banco, Tabelas…) → barra de ferramentas com ícone + rótulo, agrupada por divisores → árvore lateral fixável (Navegação, Bancos, Administração) → área de trabalho com título, subtítulo em mono e abas | todas as telas |
| Multitela | 1 a 4 regiões lado a lado, cada uma com abas próprias, «soltar numa janela» | `92-ver-quatro-regioes` |
| Grade | caixa «arraste uma coluna para agrupar», busca global, linha de filtro por coluna com operador, chips SUM no cabeçalho, seleção por checkbox, números à direita, moeda formatada | `02-tabela-conteudo` |
| Painel | cartões de número grande com rótulo em caixa alta, medidores circulares, barras de E/S, selo de estado («SAUDÁVEL»), série por hora | `06-arquivo-painel` |
| Assistente | passos numerados em pílula (1 Tabelas → 2 Campos → 3 Resultado) | `17-banco-tabela-dinamica` |
| Login | cartão central, logo, seis idiomas com bandeira, campos com rótulo em caixa alta, botão primário largo | `00-login` |
| Fontes | **Exo 2** (interface) e **IBM Plex Mono** (dados, código, subtítulos), fallback Helvetica/Arial e ui-monospace | `index.html:114`, `:19` |
| Cores | tokens em dois temas: escuro `--fundo #010418`, `--painel #0a1122`, laranja `#ff8a1c`; claro em papel quente `#f7f5f2`, laranja `#c63c0a` | `:root` e `:root[data-tema="claro"]` |
| Cores de ação | `--acao-incluir` verde, `--acao-alterar` amarelo, `--acao-marcar` rosa, `--acao-excluir` vermelho, `--acao-consultar` azul, em contorno | tokens |
| Ícones | SVG em linha, só traço (estilo Lucide), coloridos por categoria | barra de ferramentas |
| Cantos | 4 a 8 px, pílula de 99 px nos selos | CSS |

## Medido

- **Contraste dos tokens** contra o fundo, calculado (WCAG): escuro — texto 15,65:1, laranja 8,63,
  ações 6,84 a 12,81; claro — texto 16,96, laranja 4,76, ações 4,86 a 6,41. Todos ≥ 4,5:1.
- **Tamanhos de fonte**: 15 tamanhos distintos entre 9 e 17 px, sem escala; os mais usados são
  12,5 / 11 / 10,5 / 11,5 / 12 px. 52 declarações abaixo de 11 px.
- **Traço dos ícones**: 11 espessuras diferentes (1 a 3,2), a maioria entre 1,5 e 1,7.
- **Cores fixas fora dos tokens**: 44 hexadecimais literais no `index.html`.

## O que é qualidade e se mantém

- Hierarquia clara e familiar para quem vem do WinDev/ERP desktop: menu com mnemônico, barra de
  ferramentas com rótulo, árvore, abas, grade com agrupamento — o usuário reconhece sem treino.
- Dois temas desenhados (não um invertido): o claro em papel quente, com o laranja escurecido para
  manter contraste — decisão já registrada na marca.
- Dado em mono e rótulo em sans: separa o que é valor do que é interface.
- Estado com forma, não só cor: selo «SAUDÁVEL», chips SUM, pílulas de passo.
- Densidade alta e útil na multitela: quatro ferramentas abertas ao mesmo tempo.

## O que as capturas mostram de defeito

1. **A fonte da marca não aparece nas capturas.** Exo 2 e IBM Plex Mono vêm do Google Fonts; o
   servidor de captura estava sem rede, e as telas saíram em Helvetica/Arial e DejaVu Mono. Um
   console de banco que precisa rodar offline não pode depender de CDN para a fonte — as duas
   devem ir embutidas (como o PhxClaw já faz com a Exo 2).
2. **Texto pequeno demais**: rótulos da barra de ferramentas, cabeçalhos da grade e notas em 9 a
   11 px; na multitela, a leitura fica difícil.
3. **Barra de ferramentas em arco-íris**: 27 ícones em ~10 cores competem entre si; a cor deveria
   marcar o tipo de ação (as cinco cores de ação) e o resto ficar neutro.
4. **Data em formato errado no filtro**: a coluna mostra 02/02/2025 e o filtro pede `mm/dd/yyyy`
   (campo de data nativo do navegador em inglês) numa tela em português.
5. **Linhas da grade fortes demais no escuro**: separadores quase brancos entre as linhas.
6. **Botão primário cheio no login** («Entrar» laranja preenchido) contra a regra «contorno, nunca
   fundo cheio» — aceitável como única ação primária da tela, mas precisa ficar escrito como exceção.
7. **Estilos de botão misturados**: «ESCOLHER» em mono caixa alta em pílula; «Nova tabela…» em
   contorno verde; «Redesenhar» em contorno azul; «Pausar» cheio; três famílias numa só tela.
8. **Telas vazias**: a tabela dinâmica deixa 60% da área em branco no primeiro passo.
9. **Diálogos nativos do navegador** (`prompt`, `confirm`) para criar, duplicar e excluir — fogem
   do estilo e não se testam nem se traduzem.

## Como o PhxClaw adota o padrão

A sprint de qualificação da UI do PhxClaw (SP000027) passa a usar este documento como alvo de marca:
os mesmos nomes de token (`--fundo`, `--painel`, `--laranja`, `--acao-*`), Exo 2 + IBM Plex Mono
**locais**, os dois temas, ícones de traço uniforme e as cinco cores de ação só onde há ação. Os
nove defeitos acima não se copiam.
