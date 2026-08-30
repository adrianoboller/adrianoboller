# Bump version and write the changelog
# 28/08 15:07

p='CHANGELOG.md'
s=open(p).read()
novo = '''## 0.11.0 — 2026-08-28

Os monitores da máquina no painel, o aviso de disco por e-mail, e o
**DbLink** — o banco de fora aparecendo na mesma grade que os daqui.

### Corrigido

- **O percentual de disco dividia pelo tamanho errado.** A conta era
  `usado / total`, e o certo é `usado / (usado + livre)`, como a do `df`.
  Reserva de sistema de arquivos e cota não estão à disposição de ninguém, e
  contá-las como livres faz um disco cheio parecer vazio. Na máquina onde isto
  foi medido o `df` dizia **55% usado** e a conta antiga dava **8%** — com 8%,
  um alerta de «menos de 10% livre» nunca dispararia e o disco encheria calado.
  Achado rodando o servidor, não lendo o código.

- **O e-mail do alerta não atravessava relé de sete bits.** O assunto levava o
  «ç» de «espaço» cru no cabeçalho, e cabeçalho de e-mail é ASCII por
  definição (RFC 5322); o corpo ia em UTF-8 cru declarado como 7 bits, e um
  relé sem `8BITMIME` tem licença para cortar o oitavo bit. Agora o assunto sai
  em palavra codificada da RFC 2047 e o corpo em base64. Conferido decodificando
  o que um relé de verdade recebeu, com um leitor independente.

- **`.botao.perigo` pintava vermelho sobre laranja.** A regra trocava a borda e
  a cor do texto mas não apagava o fundo do `.botao`, e o botão de excluir
  ficava ilegível — na tela de usuários, que já era assim, e na nova de DbLink.

### Adicionado

- **Monitores da máquina no painel:** CPU, memória, placas de rede, discos
  físicos e espaço livre de cada caminho que o servidor usa. Tudo do `/proc`,
  que o núcleo publica em texto; o espaço livre do `df`, porque exige
  `statvfs`, que não está na `std`. Nenhuma crate entrou. Os monitores renovam
  sozinhos a cada quatro segundos, e a primeira leitura **se declara primeira**:
  `/proc` traz contador desde o arranque, e taxa precisa de dois instantes.

- **Aviso de disco apertado, por e-mail.** Dois limites no OU — percentual e
  piso em MB —, porque cada um sozinho erra de um lado: 10% de 8 TB não são
  aperto, e 1 GB livre num disco de 20 GB são. O cliente SMTP é escrito aqui,
  com a `std`.

- **DbLink.** Botão na barra, definições no menu Configurações, e o protocolo
  do MySQL(R) escrito à mão. As tabelas do banco de fora na lista, o conteúdo
  na **mesma grade** das tabelas daqui — agrupar, buscar, totalizar e paginar
  valem igual. Testado contra um MySQL(R) 8.0.46 de verdade.

- **SHA-1**, conferido contra os vetores do FIPS 180-4. Entrou por causa do
  `mysql_native_password` e só por isso: não é usado em lugar nenhum do formato
  do PhxSql — senha continua em PBKDF2-HMAC-SHA256, integridade em CRC-32 e
  SHA-256. Quem define o protocolo é o outro lado.

- **`alertas` e `dblink` no `config.json`**, e o caminho do `base` **já
  resolvido** na tela de configuração: caminho relativo vale a partir de onde o
  servidor foi iniciado, e subir por outro caminho passa a ver outro banco.

### Sabido

- **Não há TLS em lugar nenhum** — nem no SMTP nem no DbLink. A `std` não traz
  TLS e o projeto não aceita crate. O e-mail serve para relé interno na porta
  25; o DbLink, para rede interna ou túnel. A senha não viaja em texto nos dois
  casos, mas o **dado devolvido pelo DbLink viaja**.

- **Do `caching_sha2_password` só o caminho rápido**, que vale quando o
  servidor já tem a senha em cache. O completo exige TLS ou a chave RSA. Quando
  o servidor pede o completo, o erro diz isso e as duas saídas.

- **PostgreSQL(R) ainda não conecta.** A definição já pode ser guardada; o
  cliente não existe.

- **O monitor de CPU, memória e rede só existe no Linux.** Fora dele a tela diz
  que não sabe medir, em vez de mostrar zero. O espaço em disco continua
  valendo, porque vem do `df`.

---

'''
a='## 0.10.0 — 2026-08-28'
assert a in s
s=s.replace(a, novo+a, 1)
open(p,'w').write(s)
print('ok')
