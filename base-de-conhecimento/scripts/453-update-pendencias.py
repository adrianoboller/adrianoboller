# Update PENDENCIAS
# 28/08 15:09

p='docs/PENDENCIAS.md'
s=open(p).read()
a='''| ☑️ | 67 | **Botão e menu Tabelas** para gerir as tabelas do banco'''
novas = '''| ☑️ | 84 | **Botão DbLink na barra** e **definições do DbLink** no menu Configurações | cadastro com apelido, endereço, credencial e teto; a senha nunca sai em JSON. Nasce **somente-leitura** |
| ☑️ | 85 | **Conectar em banco de fora e ver as tabelas na grade tipo Janus(R)** — MySQL(R) primeiro | protocolo do MySQL(R) escrito à mão, só `std`; testado contra um MySQL(R) 8.0.46 de verdade. A grade é a **mesma** das tabelas daqui |
| ◐ | 86 | **Depois testar com PostgreSQL(R) e outros** | a definição já pode ser guardada e o cadastro reconhece o motor; o **cliente ainda não existe**. Os tijolos estão prontos: o SCRAM-SHA-256 do PostgreSQL(R) usa SHA-256, HMAC e PBKDF2, que o projeto já tem |
| ☑️ | 87 | **Monitor de espaço em disco no dashboard** | uma barra por caminho que o servidor usa — o `base`, o destino do backup e o que estiver em `alertas.caminhos`. A conta é sobre `usado+livre`, como a do `df` |
| ☑️ | 88 | **Definir no config o local de armazenamento** (`C:\\database`, `D:\\database`) | é o campo `base`, e sempre aceitou caminho absoluto. O que faltava era a tela mostrar o caminho **já resolvido**: relativo vale a partir de onde o servidor foi iniciado, e subir por outro caminho passa a ver outro banco |
| ☑️ | 89 | **Alerta de falta de espaço por e-mail**, configurado no config | seção `alertas`, com dois limites no OU e silêncio entre avisos. Cliente SMTP escrito aqui — **sem TLS**, serve para relé interno |
| ☑️ | 90 | **Monitores de placa de rede, CPU, memória e HDs no dashboard** | do `/proc`, com taxa entre duas amostras; renovam sozinhos a cada quatro segundos. **Só no Linux** — fora dele a tela diz que não sabe medir, em vez de mostrar zero |
| ☑️ | 67 | **Botão e menu Tabelas** para gerir as tabelas do banco'''
assert a in s; s=s.replace(a,novas,1)

a='''**74 feitos · 3 parciais · 6 planejados**, de 83 pedidos.'''
b='''**80 feitos · 4 parciais · 6 planejados**, de 90 pedidos.'''
assert a in s; s=s.replace(a,b,1)

a='''Fora do que você pediu, entraram por medição: o CRC slice-by-8, o `descer` sem
reler a folha, a conferência de unicidade sem descida dupla, e catorze correções
de defeito — três delas de perda silenciosa de dado.'''
b='''Fora do que você pediu, entraram por medição: o CRC slice-by-8, o `descer` sem
reler a folha, a conferência de unicidade sem descida dupla, e dezessete
correções de defeito — três delas de perda silenciosa de dado, e três achadas
**rodando** o que tinha acabado de ser escrito (o percentual de disco que
dividia pelo total, o assunto de e-mail com acento cru no cabeçalho, e o
decimal que a grade arredondava).'''
assert a in s; s=s.replace(a,b,1)

# a lista de parciais ganha o PostgreSQL
a='''| 3 | **Chave estrangeira** com CASCADE / RESTRICT / SET NULL |'''
b='''| 4 | **DbLink para PostgreSQL(R) e outros** | o cadastro reconhece o motor, guarda a definição e a tela mostra «sem cliente» em vez de fingir que conecta | o **cliente**. O caminho é curto: a autenticação `scram-sha-256` do PostgreSQL(R) se monta com SHA-256, HMAC e PBKDF2, que já estão escritos aqui, e o protocolo de consulta simples (`Q` → `T`/`D`/`C`) é menor que o do MySQL(R) |
| 3 | **Chave estrangeira** com CASCADE / RESTRICT / SET NULL |'''
assert a in s; s=s.replace(a,b,1)
a='''As duas primeiras são os dois ◐ da tabela lá em cima. A terceira **não é um
pedido seu** — é um buraco achado na revisão, dentro de um pedido marcado
feito, e fica aqui para não sumir de vista.'''
b='''As três primeiras são os ◐ da tabela lá em cima. A que trata de chave
estrangeira **não é um pedido seu** — é um buraco achado na revisão, dentro de
um pedido marcado feito, e fica aqui para não sumir de vista.'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)
print('ok')
