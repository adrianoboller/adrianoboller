# Segurança do servidor

Quatro camadas, da rede para o dado: **política**, **token**, **login**,
**permissão**. Este documento cobre a primeira e a forma como a credencial
viaja. O cadastro e as permissões estão em [`USUARIOS.md`](USUARIOS.md).

---

## 1. Base64 não é criptografia

Vale dizer isso antes de tudo, porque a confusão é comum e cara.

```bash
$ echo 'YWRyaWFubzpzZW5oYTEyMw==' | base64 -d
adriano:senha123
```

Quem captura o pacote decodifica com um comando. Base64 é **codificação**, não
cifra. O `login` aceita `usuario_b64` e `senha_b64`, e isso resolve coisas
reais — a senha some do `grep` casual, some do olho de quem passa atrás da
cadeira, e senha com aspas ou barra invertida atravessa o JSON sem escape.

Mas **não protege a senha na rede**. Se você achar que protege, pode acabar
expondo a porta 5000 fora do túnel — que é justamente o erro que ela convida a
cometer.

## 2. O que protege de verdade: desafio-resposta

Aqui a senha **nunca sai da máquina do cliente**.

```text
cliente                                          servidor
   |  {"op":"desafio","usuario":"adriano"}          |
   | -------------------------------------------->  |
   |  {"sal":..., "iteracoes":210000, "nonce":...}  |
   | <--------------------------------------------  |
   |                                                |
   | dk    = pbkdf2(senha, sal, iteracoes)          |
   | prova = hmac(dk, nonce + nonce_cliente + user) |
   |                                                |
   |  {"op":"login","prova":..., "nonce_cliente":..}  |
   | -------------------------------------------->  |
   |                                       refaz a conta com o dk guardado
```

O nonce do servidor é sorteado a cada desafio e **vale uma vez só**: gravar o
diálogo e repeti-lo depois não autentica ninguém. O nonce do cliente entra
junto para que o servidor também não escolha sozinho o que será assinado. E a
prova amarra o login, então não dá para apresentá-la como se fosse de outro
usuário.

O protocolo não tem nada de proprietário — um cliente Python com `hashlib` e
`hmac` da biblioteca padrão autentica:

```python
d  = pede({"op":"desafio","usuario":"adriano"})["resultado"]
nc = os.urandom(16).hex()
dk = hashlib.pbkdf2_hmac("sha256", senha.encode(), bytes.fromhex(d["sal"]),
                         d["iteracoes"], 32)
msg   = f'{d["nonce"]},{nc},adriano'.encode()
prova = hmac.new(dk, msg, hashlib.sha256).hexdigest()
pede({"op":"login","usuario":"adriano","nonce_cliente":nc,"prova":prova})
```

### O que ele não resolve

- **O resto do tráfego continua em claro.** Protege a credencial, não os
  dados. Para os dados, túnel — IPSec, WireGuard.
- **Quem lê o `config.json` consegue autenticar**, porque o que está guardado
  lá é exatamente a chave usada na prova. É menor do que parece: esse mesmo
  arquivo tem o token de serviço e aponta para os dados. Quem o lê já ganhou.

### As três formas de login, da mais segura para a menos

| Forma | Campos | A senha na rede |
|---|---|---|
| Desafio-resposta | `prova` + `nonce_cliente` | **não trafega** |
| Base64 | `senha_b64` | trafega, codificada |
| Texto puro | `senha` | trafega, legível |

---

## 3. Política: o que ninguém pede

```json
"seguranca": {
  "comandos_proibidos": ["excluir", "reindexar"],
  "bases_proibidas": ["financeiro"],
  "tentativas_ate_bloquear": 5,
  "janela_minutos": 10,
  "bloqueio_minutos": 60,
  "blacklist": "blacklist.json"
}
```

Isto vale para **todo mundo, root incluso**, e é conferido **antes** do token.
Não é permissão de usuário — é o que este servidor não faz por esta porta.

Pedir um comando proibido **bloqueia o IP na hora**. Não há por que dar cinco
chances a quem pediu exatamente aquilo que o arquivo diz que ninguém pede.

### Duas gravidades

| | O que é | O que acontece |
|---|---|---|
| **Grave** | comando proibido, base proibida | bloqueia na hora |
| **Leve** | token errado, senha errada, IP fora da lista | conta na janela; bloqueia no limite |

Errar a senha uma vez é humano. Errar oito vezes em dois minutos, não.

---

## 4. `blacklist.json`

```json
{
  "atualizado_em": "2026-08-27 19:30:17,323",
  "bloqueios": [
    {
      "ip": "127.0.0.1",
      "desde": "2026-08-27 19:30:17,323",
      "desde_ms": 1787858929577,
      "ate": "2026-08-27 20:30:17,323",
      "ate_ms": 1787862529577,
      "motivo": "token invalido",
      "comando": "ping",
      "tentativas": 3,
      "firewall": false
    }
  ]
}
```

IP bloqueado tem a conexão recusada **antes de qualquer outra coisa** — antes
do token, antes do login. `bloqueio_minutos: 0` bloqueia até alguém desfazer.

```bash
phxsqld --bloqueios              # quem está de fora, e por quê
phxsqld --desbloquear 203.0.113.9
```

Pelo protocolo, `{"op":"bloqueios"}` e `{"op":"desbloquear","ip":"..."}` —
ambos exigem `administrar`.

**O servidor relê o arquivo quando ele muda.** O `--desbloquear` roda em outro
processo, e sem isso o servidor continuaria barrando um IP que já saiu da
lista. Custa um `stat` por conexão.

---

## 5. A regra de firewall

```json
"firewall": {
  "ligado": false,
  "bloquear":    ["/usr/sbin/iptables", "-I", "INPUT", "-s", "{ip}", "-j", "DROP"],
  "desbloquear": ["/usr/sbin/iptables", "-D", "INPUT", "-s", "{ip}", "-j", "DROP"]
}
```

**O bloqueio nunca depende disto.** Um IP na lista é recusado dentro do
servidor, sem firewall, sem root, sem poder falhar. A regra é um extra que
tira o tráfego antes de ele chegar ao processo.

Três cuidados, e eles não são decorativos:

1. **Desligado por padrão.** Ligar é decisão consciente.
2. **Sem shell.** O comando vem como lista de argumentos e é executado direto,
   sem `sh -c`. Um daemon de rede que monta linha de comando com texto vindo de
   fora é uma porta dos fundos.
3. **O IP é validado como endereço** antes de entrar no lugar do `{ip}`. Há
   teste que recusa `"; rm -rf /"`, `"10.0.0.1 && reboot"` e `"$(whoami)"`.

Se o comando falhar, o bloqueio **continua valendo** dentro do servidor e a
falha vira aviso no log. Firewall quebrado não vira porta aberta.

Para o `iptables` funcionar, o `phxsqld` precisa rodar como root ou ter
`CAP_NET_ADMIN` — o que é um aumento de privilégio real. Pense se compensa:
recusar a conexão dentro do processo já resolve quase tudo, e não pede
privilégio nenhum.

---

## 6. A porta da interface web

O Centro de Controle escuta numa porta separada da 5000, e essa separação é
deliberada: quem fala HTTP não é quem fala JSON Lines, e um firewall pode
tratar cada uma do seu jeito. O servidor recusa subir com as duas no mesmo
endereço.

Ela **vem desligada**, e quando ligada escuta só em `127.0.0.1`. Abrir para a
rede é uma decisão de quem administra, não um padrão herdado.

### Os mesmos portões

O `POST /api` passa exatamente pelos quatro portões da porta 5000 — política,
token, login, permissão — porque é o mesmo `despachar`. A interface não tem um
caminho privilegiado: quem não pode inserir recebe a mesma recusa, tenha
clicado num botão ou aberto um soquete.

E a lista de bloqueio é do **servidor**, não da porta: cinco tokens errados
pelo navegador bloqueiam também a 5000, e `phxsqld --desbloquear` solta as
duas. Todo pedido pela web entra no `acessos.log` com IP, data e hora.

### O que ela não faz

Não serve arquivo do disco, não lista diretório e não interpreta caminho. Há
três rotas — `GET /`, `GET /saude`, `POST /api` — e nenhuma toca o sistema de
arquivos. Não há `..` para explorar porque não há diretório para escapar. A
página está embutida no binário com `include_str!`.

O cabeçalho de toda resposta traz `no-store`, `nosniff`, `X-Frame-Options:
DENY`, `Referrer-Policy: no-referrer` e uma CSP com `default-src 'none'`. A
única folga é no HTML, para a fonte da marca; as respostas de dados não abrem
exceção para host nenhum.

Tetos de tamanho: 16 KB de cabeçalho, 4 MB de corpo. Pedido maior é recusado
antes de virar memória.

### A senha, de novo

Em `127.0.0.1` e em `https://` o navegador oferece `crypto.subtle`, e a página
deriva a prova com PBKDF2 ali mesmo: manda a prova, não a senha — é a seção 2
deste documento, do lado do navegador. Fora de contexto seguro `crypto.subtle`
não existe, a página cai em Base64 e **avisa na tela**, com todas as letras.
Base64 é a seção 1: esconde de quem olha, não de quem captura.

### A sessão

A porta 5000 autentica uma vez por conexão. HTTP não tem conexão que dure,
então o login devolve um identificador de 48 caracteres hexadecimais (24 bytes
de `/dev/urandom`), repetido pelo navegador em `X-Sessao`. O prazo conta a
partir do último clique. `sair` derruba a sessão no servidor, não só na tela.

A sessão também carrega o desafio em aberto — é o que permite o
desafio-resposta por HTTP, já que o nonce precisa sobreviver de um pedido para
o outro. Ele continua valendo **uma vez só**: sai da sessão no login, dando
certo ou errado.

## 7. O que ainda não tem

- **Sem TLS.** O tráfego (fora a credencial no desafio-resposta) vai em claro.
  A porta 5000 pertence dentro de VPN ou IPSec, e a porta da interface web
  também — em `http://` para outra máquina o próprio navegador desliga a
  cifra do login.
- **Sem troca de senha pelo protocolo.** Muda no `config.json` e reinicia.
- **Sem bloqueio por faixa.** É IP a IP; `/24` inteiro exige o firewall.
- **As tentativas leves vivem em memória.** Reiniciar o servidor zera o
  contador; os bloqueios já gravados, não.
