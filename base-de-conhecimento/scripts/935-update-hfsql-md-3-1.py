# Update HFSQL.md 3.1
# 29/08 00:33

import pathlib
p = pathlib.Path("docs/HFSQL.md")
s = p.read_text()
alvo = '''O HFSQL(R) afina direito por servidor, por banco **e por tabela**, e a lista
dele é granular a ponto de separar «direito de ler as linhas» de «direito de
iniciar uma reindexação». Aqui a permissão para na **base**: quem pode ler a
base lê todas as tabelas dela.

Não é difícil — o portão já existe e é um único ponto —, e é o que separa um
banco de departamento de um banco de empresa. **Deveria ser o próximo item de
segurança.**'''
novo = '''O HFSQL(R) afina direito por servidor, por banco **e por tabela**, e a lista
dele é granular a ponto de separar «direito de ler as linhas» de «direito de
iniciar uma reindexação».

**Feito na 0.17.0.** Dentro do objeto da base, `"tabelas"` escreve a regra de
cada tabela, e ela **substitui** a da base ali — a mesma coisa que a base já
fazia com o `"*"`. É o que permite as duas coisas que a prática pede: tirar
`folha` de quem lê o banco inteiro, e dar `clientes` a quem não lê o banco
nenhum. Uma regra de *interseção* resolveria só a primeira.

O portão continua sendo **um só** — espalhado por quarenta operações, a que
alguém esquecesse de conferir viraria a porta dos fundos. Duas operações
precisaram de conferência própria porque não têm o campo `"tabela"` que o
portão lê: `juntar`, cujas tabelas estão em `a.tabela` e `b.tabela`, e `unir`,
cujas tabelas estão numa lista. Sem isso, bastaria pedir a tabela negada como o
lado B de uma junção.

A árvore e o catálogo passaram a listar só o que dá para abrir: o nome de uma
tabela já conta parte da história.

Detalhes em `docs/USUARIOS.md`. O que ainda não desce é o direito por
**coluna** — esconder o salário dentro de uma tabela que a pessoa pode ler.'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)
s = s.replace('''| Autenticação com direitos granulares por servidor/banco/tabela | por usuário e por base, 9 atividades | a deles chega na **tabela**; aqui para na base |''',
'''| Autenticação com direitos granulares por servidor/banco/tabela | por usuário, por base **e por tabela**, 10 atividades | empatado desde a 0.17.0; o que falta dos dois lados é a **coluna** |''', 1)
p.write_text(s)
print("ok")
