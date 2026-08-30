# Update manual and changelog
# 27/08 22:43

s = open('CHANGELOG.md').read()
novo = '''## 0.4.0 — 2026-08-27

### Adicionado

- **Painel.** A primeira tela depois do login: o servidor inteiro em gráficos
  — bancos, registros, usuários, conexões, acessos, recusados, IPs bloqueados
  e tabelas em RAM nos números do topo; operações por hora nas últimas 24 h;
  operações mais pedidas; usuários por nível; maiores tabelas; de onde vêm os
  acessos; e quem mais usou.

  Tudo de **uma** chamada — a operação `painel` agrega no servidor. Dez
  chamadas deixariam a tela dez vezes mais lenta só pela ida e volta. E o
  painel conta **só o que quem está olhando poderia abrir**: base sem
  permissão de leitura não entra na conta.

  Os gráficos são SVG escrito à mão — barras, área e anel —, como o resto do
  projeto. Usam `currentColor` e os tokens do tema, então trocam de cor com o
  sol/lua sem uma linha a mais.

- **O phx-grid v0.8.0 na aba Conteúdo.** O grid do ecossistema Phoenix, ES5
  estrito e sem dependência. Arrastar um cabeçalho para a faixa de cima
  **agrupa**, com contagem e agregados por grupo; vários níveis empilham e as
  pastilhas reordenam arrastando. Vieram junto a busca global e a paginação.

  As colunas saem do **esquema** da tabela, não de uma lista escrita à mão —
  tabela nova aparece certa sem tocar na página. E o grid segue o tema do
  console.

- **Comparação medida com o MySQL(R)**, 10.000.000 de registros, em
  `bancada/`. Tudo para ser refeito: `python3 bancada/medir.py 10000000`.

- **Espelho `.bkp`** (`"espelho": true`): toda escrita no `.reg` vai também
  para um irmão, e a leitura tenta o espelho quando o CRC falha. `phxsql
  reparar` conserta nos dois sentidos e **conta** o que não teve salvação.

- **Três portas de replicação:** `envio` e `retorno` separadas, validadas
  contra a porta de dados, a da web e uma contra a outra.

- `phxsqld --pagina` escreve o Centro de Controle num arquivo — da **mesma**
  função que serve o navegador.

### Corrigido

- **Ligar o espelho apagava a cópia boa.** `espelhar()` copiava o `.reg` por
  cima do `.bkp` existente: estragar o principal e religar o espelho destruía
  a segunda chance. Um teste pegou. Agora só semeia o que ainda não existe.

- **Erro de medição na bancada.** A primeira versão mandava ao MySQL(R) um
  único `WHERE id IN (…)` e ao PhxSql vinte mil buscas separadas — 41× a
  favor do MySQL(R) pela *forma da pergunta*, não pelo motor. Corrigido para
  uma instrução por operação dos dois lados; o SELECT pontual passou de 41×
  perdendo para 3,4×, e o UPDATE de perdendo para empate.

- **Gráficos desproporcionais.** O `viewBox` de 620 dentro de cartões de
  ~370 px encolhia o desenho inteiro em 0,6 — texto de 12 px virava 7 px.
  Cada gráfico passa a nascer com a largura do cartão que o recebe.

- **Colisão de nome entre o relay e o backup**: o campo do servidor remoto se
  chamava `destino`, e `destino` já era o diretório do backup. Renomeado.

### Sabido

O que a medição diz e ninguém deve esconder: **a inserção é o nosso buraco**
— 3.685 linhas/s contra 95.301 do MySQL(R), e é CPU, não disco. Continuam
faltando triggers, stored procedures, jobs, transporte de replicação,
start/stop pela interface, transações e TLS.

**280 testes**, clippy limpo, zero dependências externas.

---

'''
s = s.replace('## 0.3.0 — 2026-08-27', novo + '## 0.3.0 — 2026-08-27')
open('CHANGELOG.md','w').write(s)
