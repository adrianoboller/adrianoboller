# Update changelog and run final checks
# 28/08 15:54

p='CHANGELOG.md'
s=open(p).read()
a='''### Sabido

- **Não há TLS em lugar nenhum**'''
b='''- **As sete junções do diagrama**, mais `UNION` e `UNION ALL`. Na tela se
  escolhe **clicando no desenho de Venn**, com o SQL equivalente escrito
  embaixo de cada um. Chave composta, teto que se declara, e as três armadilhas
  do SQL respeitadas: nulo não casa com nulo, família errada é recusada na
  entrada em vez de devolver zero linhas parecendo resposta, e decimal casa por
  valor e não por escala.

- **`criar_tabela` com nome qualificado.** *(corrigido)* `filial.clientes`
  gravava cinco arquivos chamados `filial.clientes.reg` na **raiz** do banco.
  Toda leitura separa o ponto em schema e tabela desde sempre; só a criação não
  separava. A tabela nascia inalcançável e o servidor respondia «criada».

### Sabido

- **Não há TLS em lugar nenhum**'''
assert a in s; s=s.replace(a,b,1)
a='''- **PostgreSQL(R) ainda não conecta.**'''
b='''- **Junção é de duas tabelas por vez, e só por igualdade.** `ON a.x > b.y` não
  existe: o *hash join* casa por igualdade. `WHERE` sobre o resultado da junção
  também não — a tela filtra depois, na grade.

- **PostgreSQL(R) ainda não conecta.**'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)
print('ok')
