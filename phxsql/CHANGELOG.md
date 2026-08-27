# Changelog

Tudo que mudou no PhxSql, do mais novo para o mais antigo.

Formato: cada versão traz **Corrigido** primeiro — defeito é o que o leitor
precisa achar rápido —, depois **Adicionado**, **Mudado** e **Sabido**. A
seção *Sabido* lista o que ainda não funciona, para ninguém descobrir sozinho.

Os números são **medidos**, nunca estimados.

---

## 0.2.0 — 2026-08-27

### Corrigido

- **Sondagem de travessia de diretório não contava violação.** Nome de
  database, tabela ou schema com `..` ou barra já era recusado pelo motor,
  mas era recusado **calado**: não contava tentativa e não gerava bloqueio.
  Auditado com seis sondagens seguidas (`../../../etc`, `/etc`, `C:\dados`,
  byte nulo, quebra de linha): seis recusas, seis linhas no `acessos.log`,
  **zero bloqueios**. Quem sondasse podia tentar a noite inteira.

  Agora é violação grave, na mesma classe de comando proibido: **bloqueia na
  primeira tentativa** e cria a regra de firewall. Conferido contra servidor
  de verdade — uma sondagem, um bloqueio, uma regra.

  A separação está em `catalogo::nome_hostil`, deliberadamente distinta de
  `validar_nome`: `"minha tabela!"` é um nome ruim (alguém errou, recusar
  basta); `"../../etc/passwd"` não é nome nenhum.

- **Colisão de nome entre o relay e o backup.** O campo que escolhe o servidor
  remoto se chamava `destino` — e `destino` já era o diretório do backup.
  Resultado: todo pedido de backup ia parar no relay e voltava com "esta
  interface não fala com outro servidor". Renomeado para `servidor`. Achado
  ligando as duas peças, não lendo o código.

- **`fe_de_bytes` do Ed25519 lia sete bytes onde precisa de oito.** O pedaço
  do meio perdia o bit 152. Passou despercebido no teste do ponto base — que
  tem esse bit em zero — e só apareceu quando os vetores da RFC 8032 rodaram.
  É exatamente por isso que a regra "criptografia se confere contra vetor
  oficial" existe.

- **Duas cores presas ao tema escuro.** O gradiente da tela de entrada e a
  tinta do botão eram literais. No tema claro o botão ficava com tinta quase
  preta sobre vermelho escuro. Viraram token.

### Adicionado

- **Tabela em memória e `SelectMemory`.** A tabela inteira em RAM, com
  consulta que não toca em disco. Filtros (`=`, `!=`, `<`, `<=`, `>`, `>=`,
  `contem`, `comeca`, `termina`, `nulo`, `nao_nulo`), ordenação múltipla,
  projeção de colunas, `pular` e teto. Filtro de igualdade numa coluna
  mapeada evita a varredura, e a resposta diz qual mapa usou.

  **Medido** (`cargo run --release --example memoria`, 50.000 linhas, a mesma
  pergunta pelos dois caminhos):

  | caminho | tempo | linhas examinadas |
  |---|---:|---:|
  | varrendo o `.reg` | 55.878 µs | 50.000 |
  | `SelectMemory` | 641 µs | 8.333 |

  **87×.** Carga para a RAM: 53 ms, 2.205 KB de valores. O exemplo confere as
  duas respostas linha por linha antes de imprimir o número.

  Nada entra em memória sozinho, e toda escrita atualiza a cópia residente
  **dentro da mesma trava** do disco — não existe janela em que os dois
  discordem.

- **Chave assimétrica Ed25519 como segundo fator.** Escrito do zero, mais o
  SHA-512 que ele exige. Conferido contra os quatro vetores da RFC 8032
  seção 7.1, o vetor de 1023 bytes, e os quatro do FIPS 180-4 para o SHA-512.

  E a prova que vale mais: um cliente de teste que assina com a implementação
  **de referência** da RFC (Python puro, independente desta) gerou a mesma
  chave pública e teve a assinatura aceita pelo servidor.

  `phxsqld --gerar-chave` imprime o par uma vez. `"chave_publica"` no usuário
  do `config.json` passa a exigir assinatura no login, sobre o **mesmo**
  desafio da senha — então a assinatura também vale uma vez só.

- **Sistema de backup com manifesto conferível.** Cópia mais um `backup.json`
  com o SHA-256 de cada arquivo, e um comando que lê tudo de volta e confere.
  Acha arquivo que sumiu, arquivo que mudou (mesmo do mesmo tamanho) e
  arquivo que apareceu sem estar no manifesto.

  ```
  phxsql backup <base> <destino>        com o servidor parado
  phxsql conferir-backup <destino>      sai com erro se não bater
  {"op":"backup","destino":"..."}       com o servidor no ar, sob a trava
  ```

- **Alternador de tema, sol ☀️ e lua 🌙.** Paleta clara completa, começando no
  que o sistema pede e lembrando a escolha por navegador. O vermelhão
  escurece para `#c63c0a` no claro, por contraste — a mesma adaptação que o
  dossiê já fazia.

- **Campos de conexão no login:** servidor (IP ou DNS), porta, usuário, senha,
  chave privada e database. A porta que aparece é a que o servidor
  **realmente** escuta, lida do `/saude`.

- **Console para mais de um servidor.** Apontar o login para outro endereço
  abre uma conexão para ele, mantida viva pela sessão. `web.servidores`
  começa **vazio** — interface que fala com qualquer endereço é proxy aberto
  de saída.

- **`replicacao.escuta`:** o socket onde o *source* serve os eventos, separado
  da porta de dados. O config recusa colisão com a porta de dados e com a
  da web.

### Mudado

- Nomes de bancos de terceiros na documentação passam a levar **(R)**.
  Exceções deliberadas: nomes de pacote (`rusqlite` é identificador, não
  marca) e citações literais de texto alheio.
- `memoria_carregar`, `memoria` e `SelectMemory` pedem permissão de **ler**,
  não de administrar: é o mesmo dado do disco por outro caminho.
- O arranque avisa alto quando o papel de replicação não é `isolado`, porque
  o transporte de eventos ainda não existe.

### Sabido — o que ainda não funciona

- **Replicação não transporta evento.** A configuração entra e valida; o
  desenho está em `docs/REPLICACAO.md`; o `.log` v2 com imagem da linha é o
  próximo passo. Hoje o papel é só um rótulo.
- **Start/stop do serviço de dados pela interface** não existe. Parar a porta
  5000 sem derrubar o processo exige mexer no laço de aceitação, e prefiro
  fazer isso inteiro a fazer pela metade.
- **Sem transações**, logo sem o A nem o I do ACID.
- **Sem TLS.** O tráfego vai em claro; a credencial não, quando se usa
  desafio-resposta ou chave.
- **Sem compactação**, sem camada SQL, sem MCP, sem ODBC.
- `crypto.subtle` com Ed25519 é recente; navegador sem suporte não assina, e
  a página diz isso em vez de fingir.

**254 testes**, clippy limpo, zero dependências externas.

---

## 0.1.0 — 2026-08-27

Primeira versão que roda ponta a ponta.

### Adicionado

- **Os cinco arquivos:** `.reg` (registros na ordem de digitação, CRC por
  registro, esquema embutido), `.ndx` (B+tree com divisão de páginas, chave
  composta, ASC/DESC/NOCASE/único), `.bin` e `.memo` (blocos com CRC e
  contabilidade de espaço morto), `.log` (diário datado das três operações).
- **Ordem de digitação como garantia:** slot excluído nunca é reaproveitado.
- **Paginação em volumes** `_001`, `_002`, … com abertura preguiçosa. O
  volume sai da aritmética do rowid, então o índice não paga nada por ela.
- **Hierarquia** database → schema → tabela, em diretórios.
- **Chave estrangeira** no esquema, com CASCADE / RESTRICT / SET NULL.
- **Reindex:** recria o `.ndx` do zero a partir do `.reg`.
- **Servidor TCP na porta 5000**, protocolo JSON Lines, `config.json`.
- **Log de acessos** por IP, com data e hora ao milissegundo — inclusive das
  tentativas recusadas.
- **Cadastro de usuários** com senha em PBKDF2-HMAC-SHA256 de 210.000
  iterações, e permissão por base em dez atividades.
- **Login por desafio-resposta** (a senha não trafega) e por Base64.
- **Política, blacklist e gancho de firewall:** comando proibido bloqueia o
  IP na hora; token e senha errados contam tentativa.
- **Centro de Controle:** interface web embutida no binário, servida pelo
  próprio `phxsqld`.
- **Linha de comando** com nove comandos, e compilados para Linux e Windows.

### Sabido

Zero dependências externas — só a `std`. JSON, CRC-32, SHA-256, HMAC, PBKDF2
e Base64 escritos aqui, cada um conferido contra vetor oficial.
