# Write the 0.14.0 changelog and bump version
# 28/08 19:08

import io
p='CHANGELOG.md'
s=io.open(p,encoding='utf-8').read()
marca='## 0.13.0 — 2026-08-28'
novo = '''## 0.14.0 — 2026-08-28

Paginação por **cursor**, a coluna de sistema **`rownum`**, e a partição
**alfanumérica** — `Clientes_A.reg` até `Clientes_Outros.reg` — com o descritor
`.pag` ao lado.

### Corrigido

- **O servidor nunca ligava `TCP_NODELAY` nas conexões que aceita** — só o
  cliente DbLink ligava. O Nagle segurava cada resposta por até 40 ms
  esperando mais bytes para encher um pacote, e nunca vinham: a resposta tinha
  acabado. Medido na porta de dados com 20.000 linhas: **1 ms de servidor e
  44 ms de relógio**. Depois: **1,3 ms**.

  Trinta e três vezes, numa opção de soquete de uma linha, e valia para **toda**
  operação do protocolo e para todo clique da tela. Achado medindo o relógio
  contra o `ms` que a própria resposta declara — ler o código não acharia, não
  há nada errado escrito.

- **O `varrer` lia a tabela inteira para devolver uma página.** `varrer_com`
  decodifica cada linha **com os anexos** do `.bin` e do `.memo`, monta tudo em
  memória, e só então o servidor jogava fora tudo menos as primeiras `max`.

  Medido com o exemplo `custo-da-pagina`, a mesma página de 200 linhas:

  | linhas na tabela | antes | pelo cursor |
  |---:|---:|---:|
  | 100.000 | 181 ms | não mensurável |
  | 400.000 | 749 ms | não mensurável |
  | 800.000 | **3.176 ms** | não mensurável |

  O custo crescia com a **tabela**, e não com a página — pior que o
  `LIMIT`/`OFFSET` de qualquer motor, porque o `OFFSET` ao menos não carrega o
  blob.

- **A grade da tela listava os baldes como tabelas separadas.** O catálogo só
  sabia tirar sufixo **numérico**, então `clientes_A.reg`, `clientes_B.reg` e
  companhia apareciam na árvore como se fossem 37 tabelas. Agora o sufixo de
  letra conta como volume — mas **só quando o `_A` está ao lado**, porque uma
  tabela que por acaso se chame `dados_X` continua sendo ela mesma.

- **Os arquivos externos saíam com sufixo de letra.** O `.log`, o `.bin`, o
  `.memo`, o `.trash` e o `.reason` não se partem por letra: rolam por tamanho.
  Um `clientes_B.log` se leria como «o diário do balde B», e o diário é da
  tabela inteira. Achado olhando o `ls` do diretório depois de criar a tabela
  pela tela.

### Adicionado

- **Paginação por cursor no protocolo e na grade.** `depois` e `antes` levam o
  rowid onde a página parou; a resposta devolve `cursor_inicio`, `cursor_fim`,
  `ha_mais` e `ha_antes`. `pular` continua como modo de compatibilidade, e a
  resposta declara qual dos dois foi usado em `modo`.

  `ha_mais` sai de **uma** leitura além do teto, e não de contar a tabela:
  contar para mostrar «página 3 de 40» é o item mais caro da tela numa tabela
  grande, e é o que ninguém lê.

  Dentro do navegador, 20 páginas encadeadas numa tabela de 20.000 linhas:
  **4,0 ms de média, 4,9 ms a pior**, sem crescer com a profundidade. Por
  posição no mesmo ponto: **16,1 ms**.

- **Coluna de sistema `rownum`** — o número de ordem de chegada da linha, em
  toda tabela. O motor preenche; não se escreve à mão e não se ajusta. **Nunca
  reaproveita número**: se reaproveitasse, uma linha nova apareceria *atrás* de
  um cursor parado e a paginação passaria a pular registro sem avisar. Alterar
  não renumera.

- **`rowid_do_rownum`: a bissecção.** O `rownum` cresce com o `rowid`, porque o
  `.reg` guarda as linhas na ordem de chegada — então achar a linha de número
  500.000 num milhão custa **vinte leituras**, sem índice nenhum a manter.

- **Partição alfanumérica.** 37 volumes fixos — `A`..`Z`, `0`..`9`, `Outros` —
  e a linha vai para o arquivo da letra dela. O rowid é atribuído como
  `(balde − 1) × registros_por_arquivo + slot`, que é a **inversa exata** da
  conta que `localizar` já fazia: nenhum caminho de leitura mudou, o `.ndx` não
  mudou, o espelho não mudou.

  Acento cai na letra sem acento; vazio e o que não for letra nem algarismo vão
  para `Outros`; o balde que nunca recebeu linha não ganha arquivo.

- **`.pag`, o descritor de partição**, em JSON indentado ao lado da tabela.
  Diz o modo, a coluna de referência, a conta do endereço por extenso, e o que
  cada balde tem. **Gerado, nunca lido pelo motor** — a verdade continua no
  bloco de esquema e nos cabeçalhos dos volumes. Apagar não quebra a tabela.

### Mudado

- **Esquema `PSCH` v4 → v5** (a coluna `rownum`) e **`.reg` v2 → v3** (o
  contador do `rownum` nos bytes 92..100, e os slots do balde em 100..108).

- **`total` saiu da resposta do `varrer`.** Produzi-lo exigia exatamente a
  varredura que esta versão removeu. No lugar entrou `registros`, que sai do
  cabeçalho e não custa nada. Cliente que lia `total` precisa trocar.

- **Junção e união não devolvem `rownum`**, pela mesma razão de não devolverem
  `softdeleted`: dois números de ordem, de tabelas diferentes, não paginam
  coisa nenhuma.

### Sabido

- **Alterar a coluna de referência de uma tabela alfanumérica é recusado.**
  Mudaria o arquivo em que a linha mora, e com ele o rowid — que é a identidade
  dela em todo índice. O caminho é excluir e inserir de novo, e a mensagem diz
  isso.

- **O teto passa a ser por letra.** Num cadastro brasileiro o `_S` enche muito
  antes do `_K`, e quem enche primeiro derruba a inserção daquela letra com as
  outras 36 ainda com espaço. É a conta a fazer ao dimensionar.

- **O cursor é o rowid, e por índice ele não vale.** O índice devolve rowid na
  ordem da *chave*, e «continuar depois do rowid X» não quer dizer nada ali —
  o próximo da chave pode ter rowid menor. Por índice a paginação é por
  posição, e a resposta declara isso.

- **Não há salto para «a página 500».** O cursor sabe ir e voltar uma página;
  ir direto para a milésima exigiria contar, que é justamente o que foi
  removido. Quem precisa de um ponto específico usa `rownum` com a bissecção.

- **Uma tabela chamada `dados_X` e o balde X de uma tabela `dados` se escrevem
  igual.** A presença do `_A` separa os dois casos, mas criar as duas no mesmo
  diretório continua sendo uma colisão de nome que o motor não recusa.

---

'''
assert marca in s
s = s.replace(marca, novo + marca, 1)
io.open(p,'w',encoding='utf-8').write(s)
