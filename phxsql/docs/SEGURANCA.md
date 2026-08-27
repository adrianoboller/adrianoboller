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

## 6. O que ainda não tem

- **Sem TLS.** O tráfego (fora a credencial no desafio-resposta) vai em claro.
  A porta 5000 pertence dentro de VPN ou IPSec.
- **Sem troca de senha pelo protocolo.** Muda no `config.json` e reinicia.
- **Sem bloqueio por faixa.** É IP a IP; `/24` inteiro exige o firewall.
- **As tentativas leves vivem em memória.** Reiniciar o servidor zera o
  contador; os bloqueios já gravados, não.
