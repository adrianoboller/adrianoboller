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
        "verificar": true, "administrar": false, "replicar": false
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
| `bases` | O poder, base por base. |

O **root é sempre supervisor e sempre ativo**, diga o que disser o arquivo.

## As dez atividades

| Atividade | Cobre |
|---|---|
| `ler` | `bancos`, `tabelas`, `esquema`, `ler`, `varrer`, `buscar` |
| `inserir` | `inserir` |
| `alterar` | `atualizar` |
| `excluir` | `excluir` |
| `criar` | `criar_database`, `criar_schema`, `criar_tabela`, `duplicar_tabela` |
| `reindexar` | `reindexar` |
| `diario` | `diario` |
| `verificar` | `verificar` |
| `administrar` | `acessos`, `ips`, `config`, `usuarios`, `excluir_tabela` |
| `replicar` | `posicao`, `replicar` |

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

- **Sem troca de senha pelo protocolo.** Muda-se no `config.json` e reinicia.
- **Sem bloqueio por tentativas.** O `acessos.log` registra as falhas, mas
  ninguém é barrado automaticamente. Use `fail2ban` sobre o log, ou
  `ips_permitidos`.
- **Sem grupos ou papéis.** O poder é por usuário. Com muitos usuários iguais,
  isso incomoda — e aí entram papéis.
- **Senha trafega em claro** no `login`, como todo o resto do protocolo. A
  porta 5000 pertence dentro de VPN ou IPSec.
