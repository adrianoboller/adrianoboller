# Bump to 0.8.0 and write the changelog
# 28/08 11:50

import pathlib
p = pathlib.Path('Cargo.toml'); s = p.read_text()
p.write_text(s.replace('version = "0.7.0"', 'version = "0.8.0"'))

ENTRADA = '''## 0.8.0 — 2026-08-28

**Duas mudanças de formato**, e as duas entram agora porque não há dado em
produção: o campo ganhou identidade e metadados, e o volume aprendeu a cortar
pelo calendário. Junto vem a gestão do banco inteiro — catálogo, configurações,
diretivas e copiar/colar.

### Corrigido

- **Um `onclick` no `#painel` vazava para a tela seguinte.** A gestão do banco
  pendurou o clique no próprio painel, e o `folha()` troca o *conteúdo* do
  painel, não o *elemento* — o tratador sobrevivia à troca de tela e disparava
  na próxima. Clicar em «Configurações e diretivas» abria SysColumns. Corrigido
  em dois lugares: o tratador foi para o container das operações, e o `folha()`
  passou a limpar o `onclick` do painel por garantia.

- **O botão primário ocupava a linha inteira** numa barra de ações. O `.botao`
  nasceu com `width:100%` para o cartão de entrada, onde é o único da linha.

- **A tela de partições calculava por divisão**, que é a conta certa para a
  partição por faixa e errada para a por período: quatro meses apareciam como um
  volume só. Agora lê as fronteiras que o `esquema` devolve.

### Adicionado — formato

- **Esquema `PSCH` versão 3.** Cada coluna passa a carregar `id`, `caption`,
  `descricao` e `mascara`, e cada índice um bit de **primário**. A leitura ainda
  aceita a versão 2: tabela gravada antes abre, ganha um `id` v7 sorteado na
  hora e os textos vazios.

  O `id` é um UUID v7 **nunca reaproveitado**, e existe para que renomear a
  coluna não quebre nada: uma tela ou um relatório apontam para ele, e renomear
  troca só o `nome`. Os metadados moram no `.reg`, com o resto do esquema, pela
  mesma razão que o esquema mora ali — um dicionário externo se perde, se
  desatualiza, e obriga quem copia os cinco arquivos a copiar um sexto.

- **Chave primária de verdade.** Até aqui só havia «índice único», e chave
  primária é mais: é a identidade da linha. Só um índice pode ser primário, ele
  é sempre único, e nenhuma coluna dele aceita nulo — uma identidade nula não
  identifica. As três conferências acontecem no `Schema::new`.

  O papel de uma coluna — primária, estrangeira, composta — **não é gravado na
  coluna**: sai dos índices e das chaves estrangeiras, que são a verdade. Marcar
  no próprio campo criaria uma segunda verdade que divergiria no primeiro
  `ALTER`.

- **Partição por período: mensal, bimestral, semestral e anual.** O volume corta
  quando o período de uma coluna de data vira — ou quando enche, o que vier
  primeiro, porque `registros_por_arquivo` continua sendo teto.

  O endereço não pode sair de divisão quando o corte depende do calendário: dois
  meses rendem quantidades diferentes. Então **cada volume grava no próprio
  cabeçalho** o rowid em que começou e o período em que abriu, e a tabela de
  fronteiras se remonta lendo esses cabeçalhos na abertura. Achar o volume de um
  rowid vira uma busca binária num vetor de dezenas de posições, em vez de uma
  divisão. Sem arquivo extra e sem bloco que cresce.

  **A linha atrasada não volta**: um lançamento de janeiro digitado em março
  entra no volume de março. Voltar significaria escrever no meio de um arquivo
  já fechado, quebrando de uma vez a ordem de digitação e o endereço contíguo.
  Por isso o período de um volume é *o período em que ele abriu*.

- `Paginacao::com_max_arquivos` e `com_modo`; `Periodo` com `chave`,
  `primeiro_mes` e `rotulo`.

### Adicionado — protocolo

- **`copiar_tabela`**, que atravessa databases e schemas. A permissão de criar é
  conferida **no destino**, à parte: sem isso, quem pode ler um banco e não pode
  criar no outro conseguiria escrever onde não devia.
- **`sistabelas`** e **`siscolunas`** (também `systables` e `syscolumns`): o
  catálogo em forma de dado.
- `criar_tabela` aceita `caption`, `descricao`, `mascara` e `id` por coluna,
  `primario` por índice, e `particao` + `particao_coluna`.
- `esquema` devolve os metadados, o papel de cada coluna nas chaves, o modo de
  partição e a **tabela de fronteiras dos volumes**.

### Adicionado — interface

- **Gerir banco** (`Alt+6`), com 15 itens: tabelas, SysTables, SysColumns,
  copiar tabela, configurações, diretivas, editor de menu, conexões, arquivos
  bloqueados, transações, backup/restauração — e, apagados dizendo o que falta,
  triggers, procedures, jobs e modo exclusivo.
- **Configurações gerais do servidor, do banco e dos usuários**, cada uma com
  sua tela, mais **diretivas de acesso ao banco** com os seis portões na ordem
  em que fecham.
- **Copiar e colar tabela** entre bancos, com área de transferência.
- **Cadastro de campos** com id, nome, caption, tipo, tamanho, máscara,
  obrigatoriedade e descrição, e a chave primária escolhida por rádio.
- **Tabela particionada** com grade que mostra como o volume vai cortar, antes
  de gravar — porque depois não muda.
- **Configurações e diretivas da tabela**: a geometria decidida na criação, os
  índices e chaves, os volumes no disco, e o que a tabela herda do servidor.
- **Editor de menu**: troca o nome exibido de qualquer item. Fica no navegador
  de quem mexeu, não no servidor — é preferência de quem opera, não política do
  banco.

### Sabido

- **As telas de configuração leem, não gravam.** Gravar o `config.json` pela
  porta web significaria que uma sessão roubada abre o firewall, esvazia a lista
  de comandos proibidos e cria um supervisor. Criar e alterar usuário pela web
  tem o mesmo problema, com credencial no meio. As telas dizem qual campo mexer.
- **Triggers, procedures, jobs e modo exclusivo continuam não existindo.** As
  telas mostram o que falta e de que dependem; elas não os implementam.
- **Restaurar backup ainda não existe.** Copiar de volta é decidir o que fazer
  com o que está lá, e isso precisa de desenho.
- Mudar a partição de uma tabela existente continua sendo criar outra e copiar
  as linhas — o que refaz os rowids, que é exatamente o motivo de não ser
  automático.

---

'''
p = pathlib.Path('CHANGELOG.md'); s = p.read_text()
v = '## 0.7.0 — 2026-08-28'
assert s.count(v) == 1
p.write_text(s.replace(v, ENTRADA + v, 1))
print('ok')
