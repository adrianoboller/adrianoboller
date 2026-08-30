# Add #124 to CHANGELOG
# 29/08 00:34

import pathlib
p = pathlib.Path("CHANGELOG.md")
s = p.read_text()
alvo = '''- **`--example ordem-da-chave`**, que mede quanto a ordem das chaves custa. Foi
  ele que reprovou a hipótese do pedido 113 antes de ela virar código.

### Mudado'''
novo = '''- **`--example ordem-da-chave`**, que mede quanto a ordem das chaves custa. Foi
  ele que reprovou a hipótese do pedido 113 antes de ela virar código.

- **Direito no nível da tabela** (pedido 124), o primeiro item da lista que a
  leitura do HFSQL(R) apontou como faltando. Até aqui a permissão parava na
  base: quem lia a base lia **todas** as tabelas dela — e a folha de pagamento
  e a tabela de clientes moram no mesmo banco porque o negócio é um só.

  Dentro do objeto da base, `"tabelas"` escreve a regra de cada tabela, e ela
  **substitui** a da base ali — a mesma coisa que a base já fazia com o `"*"`.
  Substituir, e não interceder, é o que permite as duas coisas que a prática
  pede: **tirar** `folha` de quem lê o banco inteiro, e **dar** `clientes` a
  quem não lê o banco nenhum. Uma regra de interseção resolveria só a primeira.

  O portão continua sendo **um só** — espalhado por quarenta operações, a que
  alguém esquecesse de conferir viraria a porta dos fundos, e ninguém acharia
  isso por leitura. Duas operações precisaram de conferência própria porque não
  têm o campo `"tabela"` que o portão lê: **`juntar`**, cujas tabelas moram em
  `a.tabela` e `b.tabela`, e **`unir`**, cuja lista de tabelas está em
  `"tabelas"`. Sem isso bastaria pedir a tabela negada como o lado B de uma
  junção — há um teste com esse nome.

  A árvore e o catálogo (`tabelas`, `sistabelas`, `siscolunas`) passaram a
  listar **só o que dá para abrir**: o nome de uma tabela já conta parte da
  história, e descobrir a recusa só ao clicar é pior do que não ver.

  9 testes, e o que mais importa deles é `sem_regra_de_tabela_nada_muda`: um
  `config.json` escrito antes desta versão continua se comportando igual.

### Mudado'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)
p.write_text(s)
print("ok")
