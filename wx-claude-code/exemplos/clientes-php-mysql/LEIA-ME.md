# Projeto de exemplo CLIENTES (PHP + MySQL → Rust + MySQL + React)

O **primeiro projeto** feito com o plugin de ponta a ponta, do zelador à
entrega, e gravado em `docs/video/wx-claude-code-video-primeiro.mp4`. É o
menor caso que exercita tudo: **uma tabela**, um CRUD PHP procedural com
mysqli, e o destino em três partes — regras e API em Rust (`mysql` +
`tiny_http`), a tela em React 19 + Vite, o MySQL do legado por baixo.

Todo o conteúdo é sintético. Nenhum dado real de pessoa ou empresa.

## O que tem

```text
inputs/
  banco.sql                       DDL da tabela clientes (MySQL 8 / MariaDB 10.11)
  legado-php/                     config.php, db.php, clientes.php e o capturador do golden
  dados-de-amostra/               resultados-esperados.json, capturado rodando o PRÓPRIO legado
  marca/                          logotipo, para o questionário não faltar anexo
gerar-questionario.py             deriva o questionario.json do exemplo ESTOQUE (mesmas 60 perguntas)
questionario.json                 as respostas: produtos php, H = rust (tiny_http + mysql), I = React
destino/
  Cargo.toml, src/                regras.rs (BR-001..003, cada uma citando clientes.php#linha),
                                  repo.rs (mysql; 1062 → «e-mail ja cadastrado»), main.rs (golden | servir)
  web/                            App.tsx, App.css e tela.mjs (Playwright exercitando a tela de verdade)
  golden-master/                  casos.json e comparacao.json (5/5)
  .wx-migration/                  traceability.csv (10 linhas) e as evidências EVID-0001..0005
```

## O que ele ensinou

- **O golden master tem de sair do legado rodando**, não da leitura do código:
  foi assim que o salto do auto-increment no e-mail repetido entrou como caso,
  e o Rust teve de reproduzi-lo.
- **Interface só se prova exercitando.** Os botões vazavam da tabela
  (`display:flex` direto no `<td>`); ler o código não mostrava. O conserto
  venceu a prova UI-001, a prova foi refeita (EVID-0005), e o grafo aprendeu a
  não acusar como vencida uma prova que outra, do mesmo assunto e com o hash
  de hoje, superou.
- **`interface escolher` gravava só na raiz**, e o conversor lê a cópia em
  `.wx-migration/`. Hoje grava nas duas que existirem.
- **O zip do cliente vinha sem `marketplace.json`**, e `claude plugin list`
  saía vazio. O empacotador o escreve com a versão sincronizada.

## Para reproduzir o destino

```bash
mysql -u root < inputs/banco.sql                 # cria o banco loja e o usuário loja
export LOJA_DB_PASS=loja123
cd destino && cargo build --release
python3 <plugin>/skills/conversao-wx/scripts/golden.py comparar \
  --golden golden-master/casos.json --comando "target/release/clientes-rs golden"
./target/release/clientes-rs servir 8080 &
cd web && npm install && npm run build && npx vite preview --port 4173 & node tela.mjs
```
