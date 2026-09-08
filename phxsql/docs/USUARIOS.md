# Usuários e permissões

O cadastro mora no `config.json`, com nome completo, login, senha, e-mail,
telefone, a marca de supervisor e o poder sobre cada base — exatamente como
pedido. Com uma diferença que vale explicar.

## A senha é guardada como hash, não como senha

Um `config.json` vai para backup, para o Git, para o anexo de um chamado de
suporte. Um hash nesses lugares é um aborrecimento; uma senha é um incidente.

```
pbkdf2-sha256$210000$<sal em hex>$<hash em hex>
              ^        ^            ^
              |        |            derivado da senha com o sal
              |        16 bytes, único por senha
              iterações (o custo)
```

PBKDF2-HMAC-SHA256 com 210.000 iterações, que é a recomendação da OWASP. As
três primitivas — SHA-256, HMAC e PBKDF2 — foram escritas neste projeto para
não quebrar a regra de zero dependências, e são **conferidas contra os vetores
oficiais** nos testes: FIPS 180-4 para o SHA-256 (incluindo o clássico de um
milhão de letras `a`), RFC 4231 para o HMAC e os vetores usuais de PBKDF2.

O número de iterações viaja dentro da própria linha, então aumentar o custo no
futuro não invalida as senhas já cadastradas.

### Como gerar

```bash
echo -n 'a senha de verdade' | phxsqld --senha
"senha_hash": "pbkdf2-sha256$210000$7570c880...$becbc17c..."
```

Use o cano (`echo -n | phxsqld --senha`) e não o argumento: assim a senha não
fica no histórico do shell nem aparece num `ps`. Se você chamar `phxsqld
--senha` sem nada, ele pergunta — mas aí a senha aparece na tela.

Senha em texto puro no campo `"senha"` **funciona**, para não travar quem está
começando, mas o servidor grita no arranque:

```
AVISO: usuario legado esta com a SENHA EM TEXTO PURO no config.json.
       Troque por senha_hash: phxsqld --senha
```

## O cadastro

```json
"root": {
  "id": 1,
  "nome": "Administrador do sistema",
  "login": "root",
  "senha_hash": "pbkdf2-sha256$...",
  "email": "root@empresa.com.br",
  "telefone": ""
},

"usuarios": [
  {
    "id": 3,
    "nome": "Maria Operadora",
    "login": "maria",
    "senha_hash": "pbkdf2-sha256$...",
    "email": "maria@empresa.com.br",
    "telefone": "+55 47 98888-0000",
    "supervisor": false,
    "ativo": true,
    "bases": {
      "*": { "ler": true },
      "Z": {
        "ler": true, "inserir": true, "alterar": true, "excluir": false,
        "criar": false, "reindexar": false, "diario": true,
        "verificar": true, "administrar": false, "replicar": false,
        "tabelas": { "folha": {} }
      }
    }
  }
]
```

| Campo | Para que serve |
|---|---|
| `id` | Vai para o `.log` da tabela como autor da operação. Omitido, sai do CRC-32 do login. |
| `nome` | Nome completo, para relatório e tela. |
| `login` | O que se digita no `login`. Único, e não pode colidir com o root. |
| `senha_hash` | O hash. Nunca a senha. |
| `email`, `telefone` | Contato. |
| `supervisor` | Pode tudo, em toda base. |
| `ativo` | `false` bloqueia o login e zera o poder, sem apagar o cadastro. |
| `bases` | O poder, base por base — e, dentro de cada base, tabela por tabela. |

O **root é sempre supervisor e sempre ativo**, diga o que disser o arquivo.

## As dez atividades

| Atividade | Cobre |
|---|---|
| `ler` | `bancos`, `tabelas`, `esquema`, `ler`, `varrer`, `buscar`, `sistabelas`, `siscolunas`, `pivotar`, `sequencias` |
| `inserir` | `inserir` |
| `alterar` | `atualizar` |
| `excluir` | `excluir` |
| `criar` | `criar_database`, `criar_schema`, `criar_tabela`, `duplicar_tabela`, `copiar_tabela` |
| `reindexar` | `reindexar` |
| `diario` | `diario` |
| `verificar` | `verificar` |
| `administrar` | `acessos`, `ips`, `config`, `usuarios`, `excluir_tabela`, `ajustar_sequencia` |
| `replicar` | `posicao`, `replicar` |

> **`copiar_tabela` confere a permissão no DESTINO.** O portão geral confere
> contra o database do campo `database` — que aqui é a *origem*. Colar exige
> `criar` no banco de destino, conferido à parte: sem isso, quem pode ler um
> banco e não pode criar no outro conseguiria escrever onde não devia.

> **Por que `excluir_tabela` pede `administrar` e não `excluir`.** Poder excluir
> uma *linha* não é poder excluir a *tabela*: a primeira operação perde um
> registro, a segunda apaga o `.reg`, o `.ndx`, o `.bin`, o `.memo`, o `.log` e
> o espelho de uma vez, com todos os volumes de cada um. Não há desfazer nem
> lixeira, então a permissão é a mais alta. O servidor ainda exige o nome da
> tabela repetido no campo `confirmar`.

### Três regras que decidem tudo

1. **Nega por omissão.** Atividade que não aparece na base vale `false`.
2. **A base listada manda.** Se `"Z"` está lá, vale o que está em `"Z"` — o
   `"*"` não completa o que faltou. Uma base listada vazia (`"W": {}`) nega tudo.
3. **Sem a base e sem `"*"`, nega tudo.**

Operação desconhecida exige `administrar` — o padrão é negar, não deixar passar.

## O direito no nível da tabela

Até a 0.17.0 a permissão parava na base: quem lia a base lia **todas** as
tabelas dela. A folha de pagamento e a tabela de clientes moram no mesmo banco
porque o negócio é um só, e o direito de ler as duas não é o mesmo direito.

Dentro do objeto da base, `"tabelas"` escreve a regra de cada uma:

```json
"bases": {
  "Z": {
    "ler": true, "inserir": true, "alterar": true,
    "tabelas": {
      "folha":    { },
      "clientes": { "ler": true, "inserir": true, "alterar": true }
    }
  }
}
```

Nesse exemplo a Maria lê e grava tudo em `Z`, **menos** `folha`, onde não pode
nada.

### A regra da tabela SUBSTITUI a da base

Não soma, não corta: substitui — a mesma coisa que a base já fazia com o `"*"`.
É o que permite as **duas** coisas que a prática pede:

```json
// tirar uma tabela de quem lê o banco inteiro
"*": { "ler": true, "tabelas": { "folha": {} } }

// dar uma tabela a quem não lê o banco nenhum
"Z": { "tabelas": { "clientes": { "ler": true } } }
```

O segundo caso é o que uma regra de *interseção* não resolveria: se a tabela só
pudesse restringir, nunca daria para conceder uma tabela a quem não tem a base.

### A ordem, do mais específico para o mais geral

1. supervisor — pode tudo, em toda tabela;
2. a regra desta tabela nesta base;
3. a regra `"*"` de tabela nesta base;
4. a regra desta tabela na base `"*"`;
5. a regra `"*"` de tabela na base `"*"`;
6. e só então a regra da **base** — que por sua vez cai em `"*"` e no nível.

Operação que não fala de tabela — `bancos`, `criar_database`, `sistema` — cai
direto na regra da base, como sempre foi. **Um `config.json` sem `"tabelas"` se
comporta exatamente como antes**, e há teste que falha se deixar de se comportar.

### O que a tela mostra, e o que ela esconde

A árvore e o catálogo (`tabelas`, `sistabelas`, `siscolunas`) listam **só o que
dá para abrir**. Não é enfeite: sem isso, quem perdeu o direito a `folha`
continuaria vendo o nome dela na árvore, e o nome de uma tabela já conta parte
da história.

### As portas dos fundos que precisaram de conferência própria

O portão de permissão é **um só**, e ele lê o campo `"tabela"` do pedido. Três
operações não têm esse campo:

- **`juntar`** — as tabelas estão em `a.tabela` e `b.tabela`;
- **`unir`** — as tabelas estão numa **lista** em `"tabelas"`;
- **`pivotar`** — a tabela de fatos está no campo de sempre, mas as de
  **consulta** moram cada uma num `tabela` dentro de um item de `juntar`
  aninhado, e o portão não desce até ali.

Sem conferência própria, bastaria pedir a tabela negada como o lado B de uma
junção. As três conferem cada tabela do pedido, e há teste para cada uma.

E a mesma varredura teve de ser refeita quando o direito desceu à coluna: quem
nomeia tabela onde o portão não olha é quem escapa da guarda nova também. A
função `direito_coluna::tabelas_do_pedido` é onde essa lista mora hoje — as
três acima, mais o `destino` de `duplicar_tabela`, `copiar_tabela` e
`renomear_tabela`, que é para onde a coluna negada iria sem regra nenhuma.

### E abaixo dela, a coluna

O direito não para mais na tabela: dentro do objeto dela, `colunas` escreve
`ler` e `alterar` por coluna, com a **mesma** ordem de precedência de cima.

```json
"folha": {
  "ler": true, "inserir": true, "alterar": true,
  "colunas": { "salario": { "ler": false, "alterar": false } }
}
```

A coluna negada sai da resposta de quem devolve linha, a escrita nela é
recusada nomeando-a, e o `atualizar` que a omite **preserva o valor gravado**
— porque quem não lê a coluna manda a linha sem ela, e o motor a zeraria em
silêncio. As operações que devolvem linha por um caminho que a peneira não
percorre (`exportar`, `juntar`, `diario`, `backup`, …) **recusam a tabela**:
recusar é mais seguro que vazar. Tudo isso, com a lista medida operação a
operação, está na §15 do [`SEGURANCA.md`](SEGURANCA.md).

## Os três portões de um pedido

```
pedido ──► token ──► login ──► permissão ──► executa
           (rede)   (identidade)  (poder)
```

**Portão 1 — o token.** Continua sendo exigido em todo pedido. Ele é a chave da
porta da rede, não a identidade de ninguém.

**Portão 2 — o login.** Havendo cadastro, o token sozinho não basta: é preciso
`login` antes de qualquer operação. **Sem cadastro nenhum, o token continua
dando poder total** — que é o comportamento anterior. Ou seja: cadastrar
usuários só aperta a segurança, nunca afrouxa.

```json
{"token":"...","op":"login","usuario":"maria","senha":"..."}
```

A autenticação acontece **uma vez por conexão**, não por pedido. PBKDF2 com
210.000 iterações custa da ordem de 100 ms de propósito — irrelevante uma vez,
inviável a cada pedido. A identidade fica na conexão até ela fechar.

Login errado e usuário inexistente devolvem a **mesma** mensagem, e o caso do
usuário inexistente ainda gasta o tempo de um PBKDF2, para que os dois não se
distingam pelo relógio.

**Portão 3 — a permissão**, sobre a base daquele pedido:

```
acesso negado: carlos nao tem permissao de inserir em Z
```

## O rastro

O login aparece nos **dois** registros:

```
acessos.log   "op":"inserir","usuario":"carlos","autenticado":true,"ok":false
Tabela.log    2026-08-27 19:07:19,936  inclusao  rowid 6  versao 1  usuario 3
```

O `acessos.log` guarda o login e registra **toda** tentativa, inclusive as
negadas. O `.log` da tabela guarda o `id` numérico de quem alterou o dado — o
campo já existia no formato desde o início e agora carrega sentido.

## Criar, alterar e excluir pelo protocolo

Até a 0.18 o cadastro só se escrevia no `config.json`, e o servidor
reiniciava — que é o custo que faz o administrador dar a conta do vizinho em
vez de criar uma nova. O pedido 221 abriu a porta.

```json
{"op":"usuario_criar","login":"carlos","senha":"a-senha-do-carlos",
 "nome":"Carlos Consulta","email":"carlos@empresa.com.br",
 "nivel":"leitor","bases":{"loja":{"ler":true,"verificar":true}}}

{"op":"usuario_alterar","login":"carlos","senha":"a-senha-nova"}
{"op":"usuario_alterar","login":"carlos","ativo":false}
{"op":"usuario_excluir","login":"carlos"}
```

E em SQL, para quem chega por um driver:

```sql
CREATE USER carlos PASSWORD 'a-senha-do-carlos';
ALTER  USER carlos PASSWORD 'a-senha-nova';
DROP   USER carlos;
```

### A senha vira hash antes de a estrutura existir

Ela chega em claro no pedido — o fio já é cifrado, ver `CIFRA-DO-FIO.md` — e
o PBKDF2 acontece dentro de `objeto_do_usuario`, que é a **única** porta por
onde ela entra. Não há instante em que a árvore que vai ao disco, ou a que
volta na resposta, carregue a senha.

Os três lugares onde ela vazaria calada, e o que fecha cada um:

| Onde | O que fecha |
|---|---|
| `config.json` | o campo gravado é `senha_hash`; e o `"senha"` em texto puro que o formato ainda aceita **sai junto** quando alguém troca a senha — senão o arquivo ficaria com a nova cifrada e a velha em claro ao lado |
| resposta do protocolo | volta a **ficha**, que nunca traz senha nem hash |
| `acessos.log` | guarda a operação e o login de quem pediu, nunca o corpo |
| Profiler | tapa `senha` por **análise** da árvore, em qualquer profundidade — e o texto SQL, que não tem campo para tapar, volta redigido pelo mesmo método |

Mandar `senha_hash` pronto é **recusado**: hash pronto escolheria o próprio
custo, e «PBKDF2 com uma volta» tem a mesma cara de «PBKDF2 com 210.000».

### Aplicação a quente

O cadastro vivo é trocado e uma **geração** anda. O próximo `login` já aceita
quem acabou de nascer; e quem foi excluído ou desativado perde a ficha no
**pedido seguinte** da própria conexão, com o `faça login` de sempre.

A conexão não é derrubada, e isso é decisão: um soquete cortado chegaria na
aplicação do outro lado como falha de **rede**, por uma decisão de
**cadastro** — e a diferença aparece exatamente no cliente mais antigo, que é
quem menos sabe se recuperar.

O custo de quem nunca mexe no cadastro é **zero**: um `load(Relaxed)` compara
duas gerações, e quando elas batem não há trava, busca nem `String`. É o
portão que vem antes do trabalho.

### O que elas recusam

- deixar o servidor sem **nenhum administrador ativo** — e a conferência é
  sobre o cadastro que **sobrou**, não sobre o que o pedido parecia querer;
- tirar de si mesmo o poder de administrar, ou apagar a própria conta
  (trocar a **própria senha** continua podendo — a guarda barra a escalada,
  não o trabalho);
- criar ou promover **supervisor** sem ser supervisor: administrar *uma* base
  não dá o poder de criar quem manda em *todas*;
- mexer no **root** — ele é a porta de entrada de quando o cadastro sai
  errado, e uma operação de protocolo que trocasse a senha dele seria a porta
  e a chave no mesmo molho;
- **login que nunca autenticaria**: vazio, com espaço na ponta (o `login`
  apara antes de comparar, então essa conta não entra nunca) ou com caractere
  de controle. Espaço **no meio** continua valendo, porque o `login` aceita —
  recusar aqui seria esta camada inventando uma regra que o autenticador não
  tem.

### O portão é o mesmo, e a gravação também

As três exigem `administrar`, pelo **mesmo** `exigir_administrar` que o
`config_gravar` usa — uma cópia em cada operação seria a porta dos fundos de
sempre, a que alguém esquece de atualizar.

E gravam pelo mesmo caminho do `config_gravar`: valida a árvore inteira
antes, troca o campo **no texto** (comentário, ordem e espaçamento ficam),
escreve num temporário que herda as permissões do original e troca com
`rename`. Duas gravações do `config.json` com caminhos diferentes seriam dois
jeitos de o arquivo ficar pela metade, e só um deles estaria provado.

O que **não** mudou: `usuarios` continua fora de `CAMPOS_EDITAVEIS`, então a
tela de configuração continua sem gravar o cadastro pelo formulário genérico.
A porta nova é própria, com portão e guardas próprias.

### Provado onde

- `crates/phxsql-server/src/usuarios.rs` — as regras de cadastro, sem disco.
- `crates/phxsql-server/src/servidor.rs`, `testes_cadastro_de_usuarios` — o
  caminho inteiro, incluindo o teste do comportamento **velho**
  (`sem_mexer_no_cadastro_a_sessao_nunca_e_relida`).
- `bancada/usuarios/provar.py` — contra o motor **vivo**, pelo soquete: o
  login numa conexão nova contra o mesmo processo, o `acessos.log` escrito
  pelo laço de conexão e o `perfil.txt` do Profiler ligado.

## Conferir o cadastro

```bash
phxsqld --usuarios

login      nome                      supervisor ativo   poder por base
root       Administrador do sistema  sim        sim     (supervisor: tudo em toda base)
maria      Maria Operadora           nao        sim     *=ler  Z=ler+inserir+alterar+diario+verificar
carlos     Carlos Consulta           nao        sim     Z=ler+verificar
```

Pelo protocolo, `{"op":"usuarios"}` devolve o mesmo — e **nunca** devolve senha
nem hash; há um teste que falha se algum dia devolver.

## O que ainda não tem

- **Sem bloqueio por tentativas.** O `acessos.log` registra as falhas, mas
  ninguém é barrado automaticamente. Use `fail2ban` sobre o log, ou
  `ips_permitidos`.
- **Sem grupos ou papéis.** O poder é por usuário. Com muitos usuários iguais,
  isso incomoda — e aí entram papéis.
- ~~**Sem direito por COLUNA.**~~ Passou a existir: dentro do objeto da
  tabela, um objeto `colunas` com `ler` e `alterar` por coluna. A regra
  inteira, a lista medida das operações e o que ela recusa estão na §15 do
  [`SEGURANCA.md`](SEGURANCA.md); o resumo de uso está no `MANUAL.txt` 14.3.2.
  **Sem `colunas` no cadastro, nada muda.**
- **Senha trafega em claro** no `login`, como todo o resto do protocolo. A
  porta 5000 pertence dentro de VPN ou IPSec — e o mesmo vale para o
  `usuario_criar`: a senha vai no pedido, e o que a protege no fio é a cifra
  do fio (`CIFRA-DO-FIO.md`), não o protocolo. O desafio-resposta resolve o
  `login`; para criar não há como, porque o servidor precisa da senha para
  derivar o hash.
- **A tela ainda não cria usuário.** A porta existe no protocolo; a aba de
  Usuários da interface continua só lendo — ver a nota nela.
