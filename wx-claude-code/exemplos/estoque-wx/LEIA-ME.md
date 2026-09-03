# Projeto de exemplo ESTOQUE (WINDEV 2025)

Um sistema pequeno e completo o bastante para exercitar o plugin de ponta a
ponta: cadastro de clientes, produtos e depósitos, venda com itens, baixa de
estoque transacional, títulos parcelados, comissão mensal. Serve de tutorial,
de teste de regressão do plugin e de material de demonstração.

Todo o conteúdo é sintético. Nenhum dado real de pessoa ou empresa.

## O que tem

```text
inputs/
  banco.sql                     DDL da análise (7 tabelas, 1 view, FKs)
  estoque-codigo.pdf            só o código WLanguage (4 páginas)
  estoque-interfaces.pdf        só as janelas, controles e relatórios (2 páginas)
  estoque-queries.pdf           só as queries, parâmetros e onde são usadas (2 páginas)
  estoque-completo.pdf          a documentação inteira (9 páginas)
  screenshots/                  4 janelas em 4 estados, com screenshots.json (tela, estado, plataforma)
  dados-de-amostra/
    amostra.sql                 dados sintéticos
    resultados-esperados.json   10 casos de golden master, capturados do legado
questionario.json               as respostas do bloco 0 e das letras A–L deste projeto
fontes/                         HTML de onde os PDFs e screenshots são gerados
gerar.mjs                       regenera os PDFs e screenshots (Chromium via Playwright)
```

## Regras de negócio que ele contém (para o G2 achar)

| ID sugerido | Regra | Onde está |
| --- | --- | --- |
| BR-001 | desconto máximo 15 % para cliente comum e 25 % para especial | `CalculaDesconto` |
| BR-002 | não vende sem saldo no depósito escolhido | `ValidaEstoque` |
| BR-003 | juros de 2 % ao mês pro rata die a partir do dia seguinte ao vencimento | `CalculaJurosAtraso` |
| BR-004 | parcelas iguais a cada 30 dias, diferença de arredondamento na última | `GeraTitulos` |
| BR-005 | CPF validado pelos dois dígitos e único | `ValidaCPF`, saída de `EDT_CPF` |
| BR-006 | baixa de estoque é tudo ou nada | `BaixaEstoque` (transação) |
| BR-007 | total acima do limite de crédito pede confirmação só para cliente comum | clique em `BTN_Fechar` |
| QRY-003 | comissão de 3 % só sobre vendas fechadas | `QRY_ComissaoMensal` |

Há uma **divergência plantada** para o G2 encontrar: o PDF de interfaces diz
que `WIN_ListaVendas` tem o botão «Cancelar venda» que chama
`EstornaEstoque`, e essa procedure não aparece no PDF de código. É um
`GAP-*` legítimo, não um erro do exemplo.

## Como usar

```bash
cd exemplos/estoque-wx
python3 ../../skills/conversao-wx/scripts/aplicar_questionario.py --questionario questionario.json --project-root . --plugin-root ../..
python3 ../../skills/conversao-wx/scripts/wx_preflight.py --manifest .wx-migration/wx-inputs.manifest.json --allowed-evidence-root ./inputs --workspace-root . --output .wx-migration/preflight
```

Resultado medido do G0 sobre este exemplo: `CONDITIONAL`, classe `FORENSIC`,
zero erros bloqueantes (não há projeto WX nativo nem baseline executável, e o
pré-flight diz isso). Em seguida:

```bash
python3 ../../skills/conversao-wx/scripts/extrair_pdf.py --manifest .wx-migration/wx-inputs.manifest.json --allowed-evidence-root ./inputs --output .wx-migration/evidence/pdf-text
python3 ../../skills/conversao-wx/scripts/golden.py capturar --casos inputs/dados-de-amostra/resultados-esperados.json --saida .wx-migration/tests/golden-master/casos.json
```

Ou, dentro do Claude Code com o plugin: `/wx-claude-code:converter inventario .`

## Regenerar os anexos

```bash
NODE_PATH=<pasta do node_modules com o playwright> node gerar.mjs
```
