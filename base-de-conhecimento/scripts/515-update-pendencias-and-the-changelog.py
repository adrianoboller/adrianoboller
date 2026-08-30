# Update PENDENCIAS and the changelog
# 28/08 16:43

p='docs/PENDENCIAS.md'
s=open(p).read()
a='''| ☑️ | 67 | **Botão e menu Tabelas** para gerir as tabelas do banco'''
b='''| ☑️ | 92 | **Revisar o help do MySQL(R) e do MariaDB(R) e ver o que melhorar** | comparado contra os dois help embutidos rodando (705 e 833 tópicos). Entraram: erro com **código estável**, `sessoes` (PROCESSLIST), `encerrar_sessao` (KILL), `estatisticas` com percentis/histograma/mais-lentas/por-tabela, `checksum` e tempo no ar. O que ficou fora está em `docs/COMPARACAO.md` **com o motivo** |
| ☑️ | 67 | **Botão e menu Tabelas** para gerir as tabelas do banco'''
assert a in s; s=s.replace(a,b,1)
a='''**81 feitos · 4 parciais · 6 planejados**, de 91 pedidos.'''
b='''**82 feitos · 4 parciais · 6 planejados**, de 92 pedidos.'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)
