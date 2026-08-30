# Add 0.12.0 changelog entry and bump version
# 28/08 17:18

import io
p='CHANGELOG.md'
s=io.open(p,encoding='utf-8').read()
marca='## 0.11.0 — 2026-08-28'
novo = '''## 0.12.0 — 2026-08-28

A tabela sai em **sete formatos**, com o XLSX e o DOCX escritos aqui, e o
espelho `.bkp` entra no fluxograma de onde estava faltando.

### Corrigido

- **O cabeçalho da planilha saía com a cor da zebra e sem negrito.** Ele
  apontava para o estilo de índice 1, que é o «texto listrado». O Excel(R) não
  reclama de índice errado — ele obedece. Os índices do `cellXfs` agora têm
  nome (`estilo::CABECALHO`, `estilo::DATA_ZEBRA`, …) e há teste que confere a
  correspondência, porque número solto ali já custou caro uma vez.

- **A tabela do DOCX estava sem o `w:tblGrid`, que é obrigatório.** O Word(R)
  tolera a falta, então o defeito passaria despercebido até alguém abrir o
  arquivo noutro programa; o python-docx recusou o documento inteiro.

- **O `.bkp` não aparecia na seção 7 do dossiê** — justamente a que desenha o
  fluxo de gravação. Quem lia via cinco arquivos sendo escritos e concluía que
  o espelho era cópia feita depois. Não é: ele é escrito **no mesmo instante**
  que o principal, no mesmo offset. A figura ganhou a caixa do espelho e a da
  janela de durabilidade, que também faltava. Achado pelo Adriano lendo o
  dossiê.

### Adicionado

- **Exportar em CSV, TXT, JSON, XML, HTML, XLSX e DOCX.** Botão na barra e
  item no menu. Os dois formatos do Office são ZIP de XML, e o projeto já
  escreve ZIP com DEFLATE desde o backup: o que parecia exigir biblioteca são
  os mesmos tijolos que já estavam aqui. **Nenhuma crate entrou.**

- **A planilha sai formatada**, não crua: cabeçalho pintado, zebra nas linhas,
  painel congelado abaixo do cabeçalho, autofiltro em todas as colunas e
  largura medida das 500 primeiras linhas. O documento sai em paisagem, com o
  cabeçalho repetindo a cada página.

- **Data em planilha sai como número com formato**, e não como texto. Texto
  que parece data não ordena, não filtra por período e não entra em conta. A
  diferença entre a época do Excel(R) e a nossa é de 25.569 dias, e é só isso.

- **O HTML exportado leva filtro embutido** e não busca nada na rede: abre em
  máquina sem internet e continua funcionando.

- **`docs/MULTILINK.md`** — por que o pacote MULTILINK não dá para ligar por
  `.rlib` e qual é o caminho que funciona.

### Mudado

- **`FORMATO.md`, `MANUAL.txt` e `README.md`** passaram a dizer que a tabela é
  de cinco arquivos **mais um sexto opcional**, com a descrição de quando o
  `.bkp` é escrito, quando é lido e o que `reparar` faz nos dois sentidos.

### Sabido

- **O MULTILINK não entra por `.rlib`.** O pacote traz só binários — os fontes
  que o manifesto promete não estão nele —, e o `.rlib` foi compilado pelo
  rustc 1.98 contra o 1.94 daqui: **provado rodando o linkador** (E0514), não
  suposto. O formato do `.rlib` não é estável entre versões do compilador,
  então igualar resolveria hoje e quebraria na próxima atualização de qualquer
  um dos lados. Fora isso, um `.rlib` é dependência externa — a regra que
  sustenta o projeto —, não há fachada C que contorne, e o licenciamento é por
  máquina com prazo: linkar faria o servidor de dados inteiro passar a exigir
  licença válida para subir. O caminho é **falar por protocolo**, como o DbLink
  já faz.

- **A exportação carrega o resultado inteiro na memória** antes de escrever.
  Serve para o que uma pessoa abre no Excel(R); não serve para despejar uma
  tabela de dez milhões de linhas.

- **O DOCX não pagina coluna demais.** Em paisagem cabem umas doze colunas
  legíveis; acima disso a tabela aperta. Para tabela larga, XLSX.

---

'''
assert marca in s
s = s.replace(marca, novo + marca, 1)
io.open(p,'w',encoding='utf-8').write(s)
