# Bump version and write the changelog entry
# 28/08 11:06

import pathlib
p = pathlib.Path('Cargo.toml')
s = p.read_text()
v = 'version = "0.6.0"'
assert s.count(v) == 1
p.write_text(s.replace(v, 'version = "0.7.0"'))

ENTRADA = '''## 0.7.0 — 2026-08-28

A tela ganha **gestão de tabelas**. Criar, duplicar, reparar e excluir tabela
passam a existir no protocolo — três operações que a interface pedia e o
servidor não tinha.

### Corrigido

- **Um servidor `somente_leitura` teria deixado apagar tabela.** As três
  operações novas entraram no despacho e ficaram fora de `OPS_ESCRITA`, a lista
  que o modo somente-leitura consulta. `criar_tabela` e `excluir_tabela`
  passariam num servidor marcado como só de leitura. Como a lista é escrita à
  mão, o conserto veio com um teste que a percorre.

- **`criar_schema` estava prometido em dois lugares e não existia.** Aparecia na
  tabela de permissões do `docs/USUARIOS.md` e em `OPS_ESCRITA`; pedir pela rede
  respondia «operacao desconhecida». A biblioteca já sabia criar a pasta —
  faltava a operação. Agora existe, e a tela de nova tabela tem o campo.

- **A largura do sufixo entrava depois do teto de volumes.** `Paginacao::nova`
  confere o teto contra os três dígitos do padrão, então pedir 9.999 volumes era
  recusado *antes* de o quarto dígito existir. Entrou `Paginacao::com_max_arquivos`,
  e a ordem passou a ser largura primeiro, teto depois.

- **«Sem teto» não existe, e o padrão fingia que sim.** O sufixo tem largura
  fixa: com três dígitos o volume 1000 não teria nome de arquivo. Teto omitido
  agora vira o maior que cabe no sufixo — 999 com três dígitos —, em vez de zero,
  que o validador recusava com uma mensagem que não ajudava quem preencheu a tela.

- **A árvore roubava a tela de quem pintasse depois dela.** `montarArvore`
  terminava sempre clicando no Painel; criar uma tabela redesenhava a árvore,
  voltava para a grade — e meio segundo depois o painel chegava por cima. Quem
  vai pintar a própria tela passa `montarArvore(false)`.

### Adicionado

- **Operação `criar_tabela`**, com colunas, índices, schema e paginação. O tipo
  da coluna aceita as três formas que aparecem na prática — `Int8`,
  `Decimal(15,2)` e a forma que o próprio `esquema` devolve —, e a razão é uma
  só: o que a leitura do esquema **devolve** tem de voltar como entrada, senão
  duplicar uma tabela exigiria traduzir cada tipo à mão. As colunas do índice
  vão por **nome**, não por posição: posição muda quando alguém reordena.

- **Operação `duplicar_tabela`**, que copia os cinco arquivos byte a byte. A
  cópia nasce com os **mesmos rowids e a mesma ordem de digitação** — o que uma
  reinserção linha a linha não daria.

- **Operação `excluir_tabela`**, que apaga os cinco arquivos e o espelho
  `.bkp`, todos os volumes de cada um. Exige a permissão `administrar`, não
  `excluir` — poder perder uma linha não é poder perder a tabela — e o nome da
  tabela repetido no campo `confirmar`. A conferência de qual arquivo pertence
  a qual tabela exige o sufixo todo em algarismos: sem isso, excluir `precos`
  levaria `precos_historico` junto.

- **Operação `criar_schema`**, a pasta dentro do database.

- **Botão e menu «Tabelas»**, com as oito operações sobre a tabela escolhida:
  estrutura, editar conteúdo, partições, duplicar, reparar tabela, reparar
  índice, nova tabela e excluir. `Alt+5` abre a grade.

- **Tela de partições**, que mostra em que volume cada faixa de rowid cai e com
  que nome de arquivo. As faixas são **conta, não busca** —
  `volume = (rowid−1) ÷ por_arquivo + 1` —, e a tela diz por que não dá para
  editá-las depois: mudar o divisor mudaria o endereço de cada registro já
  gravado.

- **Tela de nova tabela**, com colunas e índices montados linha a linha, os 21
  tipos com o que cada um custa em bytes, e schema opcional.

- **Gestão de transações no menu Ferramentas.** A tela mostra a **ausência**:
  não há `BEGIN`, `COMMIT` nem `ROLLBACK`, então ela não traz lista de
  transações abertas — uma lista vazia daria a entender que o mecanismo existe e
  está parado. Lista o que de fato existe e o que falta, na ordem.

- `digitos` e `bytes_por_arquivo` na resposta de `esquema`, sem os quais não dá
  para escrever o nome do volume: `_1` e `_001` são arquivos diferentes.

### Mudado

- O menu **Tabela** virou **Tabelas** e absorveu a gestão. Dois menus vizinhos
  com nomes quase iguais obrigariam a adivinhar em qual está cada operação.

- Novo menu **Ferramentas**, espelho da barra pelo teclado.

- A ferramenta *Transações* deixa de ser um botão apagado.

### Sabido

- **Continua sem transações.** A tela nova diz isso; ela não as implementa.
- A **CLI ainda não cria tabela** — só o protocolo e a interface.
- `buscar` e `desbloquear` continuam sem tela.

---

'''
p = pathlib.Path('CHANGELOG.md')
s = p.read_text()
v = '## 0.6.0 — 2026-08-28'
assert s.count(v) == 1
p.write_text(s.replace(v, ENTRADA + v, 1))
print('ok')
