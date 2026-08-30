# Write the 0.13.0 changelog and bump version
# 28/08 18:06

import io
p='CHANGELOG.md'
s=io.open(p,encoding='utf-8').read()
marca='## 0.12.0 — 2026-08-28'
novo = '''## 0.13.0 — 2026-08-28

**Excluir deixou de ser uma coisa só.** Toda tabela ganhou a coluna de sistema
`softdeleted`, e dois arquivos novos entraram: o `.trash`, com a linha inteira
antes de ela sumir, e o `.reason`, com o porquê de cada exclusão. Os dois são
de quem administra.

### Corrigido

- **A grade de dados estava com os valores desalinhados do cabeçalho.** Cada
  célula era montada como `<td>${celulaValor(...)}</td>`, e `celulaValor` já
  devolve o `<td>` inteiro — o navegador fecha o primeiro e abre outro, então
  **cada valor ganhava uma célula vazia na frente**. A linha saía com o dobro
  de células do cabeçalho, e todo dado aparecia uma coluna à direita do nome
  dele. Achado abrindo a página no Chromium e contando as células do DOM, não
  lendo o código: o defeito estava em duas telas, a principal inclusive.

- **Um `atualizar` de rotina ressuscitava linha marcada como excluída.** O
  servidor monta a linha inteira a partir do JSON, e a coluna de sistema
  ausente virava `false` — sem erro, sem aviso, e a linha reaparecia na lista.
  Agora, quando o pedido não fala da coluna, ela **mantém o que a linha já
  tinha**. Achado escrevendo o teste, antes de existir na tela.

- **A lixeira dizia «0 anexos» para linha que tinha anexo.** A listagem não
  carrega os anexos de propósito — um memo de megabytes vezes trezentas linhas
  vira uma resposta que ninguém usa —, e o contador saía do vetor vazio em vez
  do cabeçalho do registro. Quem investigasse concluiria que a foto nunca
  existiu, que é o oposto do que o `.trash` serve para provar. Agora o contador
  vem do cabeçalho, o campo externo aparece como «anexo · não carregado» em vez
  de `NULL`, e há um botão que traz aquela linha inteira.

### Adicionado

- **Exclusão suave, e ela é o padrão.** `excluir` marca a linha: ela some das
  listas e continua inteira no `.reg`, com os anexos, e `restaurar` desfaz. A
  física acontece com `"fisico": true`.

  O padrão é o reversível porque **o irreversível não pode ser escolhido por
  omissão**: um cliente que manda `excluir` sem dizer mais nada está pedindo
  «tira isto da minha lista», e é isso que ele recebe.

- **O `.trash`: a linha inteira, antes de sumir.** Gravada e **sincronizada
  antes** de o slot do `.reg` ser liberado. Guardar depois de liberar teria uma
  janela em que a linha não existe em lugar nenhum, e uma queda dentro dela não
  tem conserto; guardar antes tem a janela oposta, que se resolve olhando.
  Entre perder e duplicar, o motor duplica. Há teste que fecha a tabela **sem
  sincronizar** e reabre, para provar que a garantia não depende de um
  `sincronizar` posterior.

  Guarda o *payload* byte a byte **mais o conteúdo dos anexos** — e não os
  ponteiros. Os blocos do `.bin` são liberados na exclusão e podem ser
  reaproveitados pela próxima inserção: com ponteiros, a foto voltaria sendo a
  de outra linha. Há teste que exclui, insere vinte linhas por cima e confere
  que a foto que volta ainda é a certa.

- **O `.reason`: quem, quando e por quê.** O `.log` diz que houve uma exclusão
  no rowid tal; o que ele não tem onde dizer — o evento dele tem 36 bytes
  fixos — é o motivo. Guarda a frase, a identidade da linha (a chave primária,
  em texto, porque «rowid 4173» não diz nada seis meses depois), o usuário e um
  UUID v7 do próprio evento. **Sobrevive à linha**: o expurgo da lixeira é
  registrado aqui antes de o dado sair.

- **Motivo obrigatório por tabela**, escolhido na criação. Marcado, o motor
  recusa qualquer exclusão sem frase escrita, antes de qualquer gravação.

- **Os três arquivos do administrador.** `lixeira` e `motivos` exigem
  `administrar`; o `.log` mantém a permissão `diario`, que já existe e que só
  um administrador concede. A razão está no conteúdo: quem só tem `ler` perdeu
  o direito àquela linha no instante em que ela foi excluída, e a lixeira
  devolveria o direito por outra porta.

- **Na tela:** o botão Excluir abre um diálogo com os dois modos e o campo do
  motivo — e não um `confirm()`, que só sabe perguntar sim ou não. A grade
  ganhou o par «ativas / excluídas», com botão de restaurar em cada linha
  marcada. Lixeira e Motivos têm tela própria, no menu Tabelas e no botão novo
  da barra. A coluna de sistema **não** vira campo de formulário: oferecer um
  `select` com «verdadeiro / falso» convidaria a excluir digitando, sem motivo
  registrado.

### Mudado

- **Esquema `PSCH` v3 → v4.** A v4 acrescenta a coluna de sistema e o byte do
  motivo obrigatório. Tabela gravada na v3 **continua abrindo e lendo
  exatamente como está** — ela só não tem exclusão suave, e a mensagem de erro
  diz isso em vez de ler lixo.

  A coluna entra em `Schema::new`, que é o caminho de criar; a leitura do disco
  usa outro caminho, que não acrescenta nada. Se acrescentasse, cada linha de
  uma tabela v3 passaria a ser lida com os *offsets* deslocados — e
  **silenciosamente**, porque o CRC do slot continuaria batendo: os bytes
  seriam os mesmos, só a interpretação mudaria. Há teste que trava isso.

- **A coluna entra no fim da lista**, para que os *offsets* das colunas do
  usuário não mudem de lugar. `inserir` com N−1 valores preenche `false`;
  `atualizar` com N−1 mantém o que a linha tinha.

- **`varrer` ganhou `visao`**: `ativas` (padrão), `excluidas`, `todas`. Sem o
  filtro por padrão, marcar não faria nada.

- **Junção e união não devolvem a coluna de sistema.** Uma junção traria duas —
  `c.softdeleted` e `p.softdeleted` —, e as duas seriam falso em toda linha,
  porque a junção só lê linha ativa.

### Sabido

- **A lixeira não devolve a linha para o `.reg`.** Ela guarda, mostra e deixa
  baixar; restaurar de lá exige reinserir, e a linha volta com **outro rowid** —
  o `.reg` não reaproveita slot, nem por restauração. Quem quer volta pelo mesmo
  rowid usa a exclusão suave, que é para isso.

- **O `.trash` e o `.reason` não são cifrados nem compactados.** Compactar
  arquivo append-only exige rotacionar e reescrever, e cifrar exige uma cifra
  de bloco que o projeto ainda não tem: há SHA-256, HMAC e PBKDF2 escritos aqui,
  mas nenhum AES. Enquanto isso, quem tem acesso ao disco lê os dois — a
  proteção é a permissão do sistema de arquivos, e não o formato.

- **A listagem da lixeira carrega o resultado inteiro na memória**, como a
  exportação. Serve para investigar; não serve para varrer uma lixeira de
  milhões de linhas.

- **Filtrar por visão num caminho de índice custa uma leitura por linha.** O
  índice devolve rowid e a marca está no registro. É o preço de pedir
  ordenado; a varredura direta não paga nada.

---

'''
assert marca in s
s = s.replace(marca, novo + marca, 1)
io.open(p,'w',encoding='utf-8').write(s)
