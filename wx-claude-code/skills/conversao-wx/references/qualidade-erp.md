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

Depois das oito, a letra F fecha com as três de antes: paleta e marca, tema
(claro, escuro, ambos), preservar ou redesenhar o visual do WX.

## O que muda no fluxo

1. `aplicar_questionario.py` grava as respostas em `PRODUCT.md` (F1) e em
   seções próprias do `DESIGN.md` (F2 a F8), além da paleta.
2. `/wx-claude-code:estilo-telas` roda `init` já com o `PRODUCT.md`
   preenchido, e depois um comando por seção respondida: `shape` para grids,
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
