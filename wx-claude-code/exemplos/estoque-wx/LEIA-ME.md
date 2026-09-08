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

## O destino: Rust + Axum + PostgreSQL 16, tela em React 19

`destino/` é o resultado da conversão feita com o plugin a partir **só dos
PDFs** (não há projeto WINDEV nativo), gravada em
`docs/video/wx-claude-code-video-windev.mp4`:

```text
destino/
  Cargo.toml, src/           regras.rs (BR-001, 003, 004, 005, cada uma citando estoque-codigo.pdf e a página),
                             repo.rs (BR-002, 006, 007, QRY-001, 003, 005 sobre o PostgreSQL), main.rs (golden | servir)
  database/                  esquema-postgresql.sql (DB-001, traduzido do banco.sql) e amostra-postgresql.sql
  web/                       WIN_Venda em React (App.tsx), tela.mjs (Playwright: 16 conferências)
  golden-master/             casos.json (10, capturados da amostra) e comparacao.json (10/10)
  .wx-migration/             traceability.csv (17 linhas), evidências EVID-0001..0003, DEC-001
```

O grafo fecha com **uma** lacuna, e ela é a linha UI-003, bloqueada pelo GAP
plantado: `EstornaEstoque` continua sem código para converter, e a condição de
desbloqueio está escrita na própria linha.

### O que ele ensinou

- **`currency` é ponto fixo; f64 não é.** As regras trabalham em centavos
  inteiros, e os juros pro rata die são `valor_cent × 2 × dias / 3000` com
  arredondamento inteiro, não `valor × 0.02 / 30 × dias` em f64.
- **O tipo do parâmetro é do banco, não da linguagem.** `CURRENT_DATE + $2`
  caiu com «operator is not unique: date + unknown», e `$1::date` com uma
  string caiu com «error serializing parameter 0». O `tokio-postgres` infere o
  tipo do parâmetro pelo contexto do SQL; o cast explícito (`$2::int`,
  `$1::text::date`) é o que fecha a conta. O Display do erro diz só «db
  error»; a mensagem do servidor está em `as_db_error()`.
- **A venda inteira numa transação (DEC-001).** O legado gravava `VENDA` e
  `ITEMVENDA` fora da transação e só a baixa dentro: sobrava venda `A` órfã
  quando a baixa falhava. O comportamento visível é o mesmo; o órfão some.
- **Olhar a tela achou o que o teste não media.** A pergunta «Fechar mesmo
  assim?» saía espremida na primeira coluna da grade (120 px): faltava
  `grid-column: 1/-1`. As 14 conferências passavam.
- **Uma linha de matriz por arquivo de código, ou o grafo acusa.** `main.tsx`
  e `vite.config.ts` entraram como UI-002 e NFR-001, com o teste da tela.
- **A matriz feita à mão não passava no validador do próprio plugin.** O C-GATE
  do vídeo passo a passo acusou: linhas «verified» sem `test_result_ref`,
  `target_commit`, `expected` e `actual`; `GAP-001` como id, que a matriz não
  aceita — gap é uma linha de tela **bloqueada**, com a condição de desbloqueio
  nas notas. Corrigida, o validador diz VALID e o C-GATE aprova a CONST-0005.

### Para reproduzir

```bash
apt-get install postgresql && service postgresql start
su postgres -c "psql -c \"CREATE USER estoque WITH PASSWORD 'estoque123'\" -c 'CREATE DATABASE estoque OWNER estoque'"
export PGPASSWORD=estoque123 ESTOQUE_DB_PASS=estoque123
psql -h localhost -U estoque -d estoque -f destino/database/esquema-postgresql.sql
psql -h localhost -U estoque -d estoque -f destino/database/amostra-postgresql.sql
cd destino && cargo build --release && cargo test
python3 ../../../skills/conversao-wx/scripts/golden.py comparar --golden golden-master/casos.json --comando "target/release/estoque-rs golden"
./target/release/estoque-rs servir 8081 &
cd web && npm install && npx vite build && (npx vite preview --port 4174 &) && node tela.mjs
```

## Regenerar os anexos

```bash
NODE_PATH=<pasta do node_modules com o playwright> node gerar.mjs
```
