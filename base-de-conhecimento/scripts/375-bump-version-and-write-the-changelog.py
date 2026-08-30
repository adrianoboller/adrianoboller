# Bump version and write the changelog
# 28/08 13:36

import pathlib
p = pathlib.Path('Cargo.toml'); s = p.read_text()
p.write_text(s.replace('version = "0.8.0"', 'version = "0.9.0"'))

ENTRADA = '''## 0.9.0 — 2026-08-28

Duas peças de análise: o agrupamento da grade chega ao nível do Janus GridEX(R)
e do DevExpress(R), e a **tabela dinâmica** ganha assistente e um motor de
tabulação cruzada no servidor.

### Adicionado — tabela dinâmica

- **Operação `pivotar`**, que cruza uma tabela por dois eixos e resume as
  células. A agregação acontece **no servidor**, e é o ponto: um pivot resume —
  cem mil linhas viram uma grade de vinte por doze —, e trazer as cem mil para
  o navegador somar seria pagar o transporte do que vai ser jogado fora.

- **Junção por tabela de consulta.** Cruzar «vendas pela cidade do cliente»
  exige a cidade, que mora na outra tabela. A forma ingênua — uma busca no
  índice por linha de venda — custaria uma descida na árvore por linha. Aqui a
  tabela de consulta é lida **uma vez** para um mapa em memória e o cruzamento
  vira acesso direto: é o *hash join*, e para a forma de dado que um pivot cruza
  (muitos fatos, poucas dimensões) ele é a escolha certa. Teto de 500.000 linhas
  por tabela de consulta, dito no erro quando estoura.

- **Seis resumos**: soma, média, contagem, mínimo, máximo e valores distintos.
  Contagem é o único que dispensa campo de valor.

- **Granularidade de data**: cada valor, por dia, mês, trimestre ou ano. Cruzar
  venda por dia daria uma coluna por dia do ano; o que se quer é por mês ou
  trimestre, e isso é escolha de quem monta, não propriedade do dado. Os rótulos
  saem em ordem lexicográfica crescente (`2026-01`, `2026-T1`), então ordenar
  texto já ordena tempo.

- **Assistente de três passos** na interface (botão *Pivot*, `Alt+7`): quais
  tabelas entram — com as junções propostas a partir das chaves estrangeiras
  declaradas —, que campo vai em cada eixo (arrastando), e o resultado com total
  por linha, por coluna e geral. Mais «copiar como CSV» e «ver o pedido», que
  mostra o JSON equivalente pela porta 5000.

### Adicionado — agrupamento da grade

- **Ordem por nível**: a seta na pastilha inverte crescente/decrescente daquele
  nível. Agrupar por mês quase sempre quer o mais recente em cima. A direção é
  guardada por *campo* e não por posição, então arrastar a pastilha para outro
  lugar não vira a ordem de quem ficou no lugar dela.
- **Rodapé por grupo**, com o total alinhado **na coluna** e não numa tira de
  texto — é assim que se compara um total com os valores acima. Num grupo de
  trinta linhas o cabeçalho já rolou para fora da tela quando o total interessa.
- **Total geral** da grade, sobre o conjunto filtrado inteiro: ele não muda ao
  virar de página, porque um rodapé que muda ao virar de página não é total de
  nada.
- **Expandir tudo / recolher tudo**, e um botão que liga e desliga o rodapé por
  grupo.

### Corrigido

- **`Sequence` aparecia como campo de texto** na paleta do pivot. É um contador.

### Sabido

- O pivot lê até 5.000.000 de linhas por cruzamento. Acima disso o número
  devolvido seria de uma amostra, e amostra sem aviso é pior que recusa.
- A junção é por igualdade de uma coluna com a chave primária da tabela de
  consulta (ou a coluna nomeada em `chave`). Não há junção por faixa nem
  composta.
- Célula vazia quer dizer «nenhuma linha caiu ali», não zero — e os dois são
  informações diferentes.

---

'''
p = pathlib.Path('CHANGELOG.md'); s = p.read_text()
v = '## 0.8.0 — 2026-08-28'
assert s.count(v) == 1
p.write_text(s.replace(v, ENTRADA + v, 1))

p = pathlib.Path('README.md'); s = p.read_text()
trocas = [
 ('O motor de armazenamento está completo e testado: **355 testes**, sem nenhuma',
  'O motor de armazenamento está completo e testado: **367 testes**, sem nenhuma'),
 ('| phx-grid na aba Conteúdo: agrupamento por arrastar | pronto |',
  '| phx-grid: agrupamento por arrastar, ordem por nível, rodapé e total geral | pronto |\n'
  '| Tabela dinâmica com assistente — cruzamento somado no servidor | pronto |'),
]
for v, n in trocas:
    assert s.count(v) == 1, v[:50]
    s = s.replace(v, n)
p.write_text(s)
print('CHANGELOG 0.9.0 e README')
