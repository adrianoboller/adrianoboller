# Update PENDENCIAS and verify the counts
# 28/08 15:46

p='docs/PENDENCIAS.md'
s=open(p).read()
a='''| ☑️ | 67 | **Botão e menu Tabelas** para gerir as tabelas do banco'''
novas = '''| ☑️ | 91 | **Operações básicas de union, inner join e as outras do diagrama** | as sete figuras (`interna`, `esquerda`, `direita`, `completa`, `so_esquerda`, `so_direita`, `so_dos_lados`) mais `UNION` e `UNION ALL`. Na tela se escolhe **clicando no desenho de Venn**, com o SQL equivalente escrito embaixo. Chave composta, e nulo que não casa com nulo, como no SQL |
| ☑️ | 67 | **Botão e menu Tabelas** para gerir as tabelas do banco'''
assert a in s; s=s.replace(a,novas,1)
a='''**80 feitos · 4 parciais · 6 planejados**, de 90 pedidos.'''
b='''**81 feitos · 4 parciais · 6 planejados**, de 91 pedidos.'''
assert a in s; s=s.replace(a,b,1)
a='''dezessete
correções de defeito — três delas de perda silenciosa de dado, e três achadas
**rodando** o que tinha acabado de ser escrito (o percentual de disco que
dividia pelo total, o assunto de e-mail com acento cru no cabeçalho, e o
decimal que a grade arredondava).'''
b='''dezoito
correções de defeito — três delas de perda silenciosa de dado, e quatro
achadas **rodando** o que tinha acabado de ser escrito (o percentual de disco
que dividia pelo total, o assunto de e-mail com acento cru no cabeçalho, o
decimal que a grade arredondava, e o `criar_tabela` que gravava
`filial.clientes.reg` na raiz do banco e devolvia uma tabela que nenhuma outra
operação conseguia abrir).'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)
