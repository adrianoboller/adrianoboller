# Qualidade gráfica e funcional para ERP: as oito subperguntas da letra F

Um ERP não é um site. Quem usa fica oito horas na tela, digita com o
teclado, vive dentro de grids, imprime e trabalha com número. Paleta e
tipografia são o começo; o que decide a qualidade final é o que vem abaixo.
Cada subpergunta alimenta um arquivo (`PRODUCT.md` ou `DESIGN.md`) e um
comando do Impeccable, e por isso não é pergunta decorativa: sem resposta o
comando roda no escuro.

| # | Subpergunta | Vai para | Comando do Impeccable que consome | Regra que nasce |
| --- | --- | --- | --- | --- |
| F1 | **Quem opera e por quanto tempo?** perfil (balcão, financeiro, chão de fábrica), horas por dia, ambiente (luz, ruído), tela típica (1366×768, 1920×1080, tablet) | `PRODUCT.md` | `init` (modo *Operate*), `adapt` | densidade, tamanho mínimo de fonte, contraste alvo |
| F2 | **Teclado ou mouse?** atalhos do WINDEV a preservar (F2 novo, F5 salvar, Esc), ordem de tabulação, Enter avança campo? | `DESIGN.md` › interação | `harden`, `polish` | mapa de atalhos, foco visível, tab order igual à do legado |
| F3 | **Grids** volume por tela (centenas ou milhares de linhas), colunas fixas, ordenação e filtro por coluna, edição na célula, totais no rodapé, exportar (XLSX, CSV), impressão da grade | `DESIGN.md` › grids | `shape`, `layout`, `audit` | virtualização, identidade de linha, exportação sem fórmula |
| F4 | **Formulários** validação inline ou ao salvar, mensagens do legado a manter, campos obrigatórios marcados como, máscaras (CPF, CNPJ, moeda, data), autocompletar | `DESIGN.md` › formulários | `harden`, `clarify` | texto de erro do legado preservado, máscara por tipo |
| F5 | **Números, datas e moeda** locale (pt-BR), casas decimais por tipo, negativo em vermelho ou entre parênteses, alinhamento à direita, fuso | `DESIGN.md` › formatos | `typeset`, `harden` | `tabular-nums`, formatação por tipo, nunca arredondar na tela o que o banco não arredonda |
| F6 | **Relatórios e impressão** quais telas imprimem, papel (A4, bobina), cabeçalho e rodapé, PDF, etiquetas | `DESIGN.md` › impressão | `layout`, `harden` | CSS de impressão, quebra de página, totais por quebra |
| F7 | **Estados e erros** o que a tela mostra vazia, carregando, sem permissão, offline, com erro do servidor; confirmação em ações destrutivas | `DESIGN.md` › estados | `onboard`, `harden`, `critique` | um estado por situação, sem tela em branco |
| F8 | **Acessibilidade e conformidade** WCAG AA obrigatório? leitor de tela? daltonismo? tamanho de toque em tablet? | `DESIGN.md` › acessibilidade | `audit`, `adapt` | contraste medido 4,5:1, foco, rótulos, alvo de toque 44 px |

## A tela modelo (F0)

Antes de qualquer subpergunta, o wizard pede a captura da **tela principal** do
projeto WX, e de um cadastro típico se houver, para servir de modelo visual.
Registra também o que dela deve ser preservado (posição da barra de botões,
ordem dos campos, totais) e o que pode mudar (fonte, espaçamento, cores). A
captura só entra depois de aberta, como todo anexo. No `DESIGN.md` ela vira a
seção «Tela modelo», e o `/impeccable critique` de cada tela nova compara com
ela antes do `polish`: parecido onde o usuário pediu para preservar, diferente
só onde ele pediu para mudar.

## As cinco subperguntas dos botões e do fundo (F9 a F13)

Botão é onde o usuário do ERP mais percebe diferença entre o sistema velho e
o novo. Por isso o vocabulário, a posição, o ícone e a cor são perguntados
um a um, e a resposta é gravada como tabela por ação, que os agentes seguem
letra por letra.

| # | Subpergunta | O que se registra | Comando que consome |
| --- | --- | --- | --- |
| F9 | **Vocabulário dos botões**: imperativo (INCLUIR, ALTERAR, EXCLUIR, GRAVAR, SELECIONAR REGISTRO, VOLTAR, CANCELAR, DUPLICAR) ou substantivo (Inclusão, Alteração, Exclusão, Gravação, Selecionar, Abortar, Cancelar)? Maiúsculas ou capitalizado? Texto exato das mensagens (confirmar exclusão, gravado, excluído, cancelado) | um rótulo por ação, as mensagens padrão | `harden`, `polish` |
| F10 | **Posição**: a barra fica acima, abaixo, à direita ou à esquerda da grade? E dos campos? Alinhada a que lado? Em que ordem? Onde ficam gravar e cancelar? | posição por barra, ordem fixa, igual em todas as telas | `layout`, `shape` |
| F11 | **Ícones**: usar? qual biblioteca (Lucide, Material, Font Awesome, os do WINDEV)? com ou sem texto? tamanho? um ícone por ação | tabela ação → ícone | `polish` |
| F12 | **Cores das ações**: uma cor por ação, contorno ou preenchido? O padrão do plugin é verde inclui e grava, amarelo altera, vermelho exclui, azul seleciona, cinza volta e cancela, sempre contorno e preenchimento só no *hover* | tabela ação → cor, com contraste a medir | `colorize`, `audit` |
| F13 | **Fundo das telas**: cor lisa (hexadecimal ou rgb), textura ou imagem? Cor do tema escuro? Opacidade da textura? | tipo, cores, textura e a regra de não baixar o contraste | `colorize`, `adapt` |

As oito ações são fixas (incluir, alterar, excluir, gravar, selecionar,
voltar, cancelar, duplicar) para que a tabela seja comparável entre telas; o
usuário escolhe o rótulo de cada uma, e o agente não «melhora» texto de
botão. Mensagem que já exista no código do legado tem precedência.

Depois das treze, a letra F fecha com as três de antes: paleta e marca, tema
(claro, escuro, ambos), preservar ou redesenhar o visual do WX.

## O que muda no fluxo

1. `aplicar_questionario.py` grava as respostas em `PRODUCT.md` (F1) e em
   seções próprias do `DESIGN.md` (F2 a F8), além da paleta.
2. `/wx-claude-code:estilo-telas` roda `init` já com o `PRODUCT.md`
   preenchido, e depois um comando por seção respondida (inclusive as tabelas de botões e o fundo): `shape` para grids,
   `harden` para formulários e estados, `typeset` para formatos, `layout`
   para impressão, `audit` para acessibilidade. Seção sem resposta não gera
   comando, gera pergunta.
3. O papel E (designer) e o `design-quality-specialist` usam essas seções
   como critério de aceite de cada tela: uma tela está pronta quando passa
   por `polish` e `audit` **e** atende as seções do `DESIGN.md` que a
   afetam.
4. O `grid-migration-specialist` lê F3 antes de propor a grade de destino.

## Outras skills de qualidade que o plugin encaminha

- **Dataviz** (skill do Claude Code, quando disponível na sessão): para
  dashboards e gráficos de ERP, com paleta acessível e escala honesta. O
  designer a carrega quando a tela tem gráfico; o plugin não a vendoriza.
- **Impeccable `critique`**: revisão heurística com nota, para a reunião de
  aceite da tela com o responsável de negócio.
- **Golden master visual**: screenshots do legado (letra C e a pasta
  `screenshots/`) ao lado da tela nova, no mesmo estado, como evidência do
  `fidelity.ui`.

## O que não muda

A cor vem do usuário ou da marca; sem resposta não há `DESIGN.md`, há
pergunta. Texto de interface não muda a caixa do dado gravado. Componente
novo se abre no navegador e se olha; ler o CSS não prova a tela.
