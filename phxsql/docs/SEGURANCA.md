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

---

## 8. A cifra dos diários: ChaCha20-Poly1305

O pedido 101 — cifrar `.log`, `.trash` e `.reason` — ficou parado por uma
frase: *«o projeto não tem cifra de bloco»*. Havia SHA-256, HMAC e PBKDF2, e
nenhum AES. A frase estava certa e a conclusão estava errada: **a cifra que
falta não precisa ser de bloco.**

### Por que ChaCha20, e não AES

AES em software puro, sem a instrução do processador, se escreve com tabelas —
e tabela em cache vaza a chave pelo tempo de acesso. Fugir disso exige
*bitslicing*, que são alguns milhares de linhas para conferir. O ChaCha20 é
soma, XOR e rotação de 32 bits: **tempo constante por construção**, sem tabela
nenhuma. São ~300 linhas, e é a mesma escolha que o TLS 1.3 e o WireGuard
fazem para máquina sem AES-NI. O PhxSql compila para Windows, Linux e ARM sem
saber onde vai rodar.

A implementação está em `crates/phxsql-core/src/cifra.rs` e é conferida contra
**todos** os vetores do RFC 8439 que dá para exercitar: o bloco (§2.3.2), a
cifragem (§2.4.2), o Poly1305 (§2.5.2), a chave de uma vez só (§2.6.2) e o
AEAD com dado associado (§2.8.2). Nada foi aceito por parecer certo.

### O desenho da integração nos três arquivos

Os três são *append-only*, e isso decide quase tudo.

**Cifra-se o corpo, não o cabeçalho.** No `.log` o evento é 44 bytes de
cabeçalho mais um corpo opcional — e é o corpo que carrega a imagem da linha,
que é o dado do cliente. O cabeçalho carrega carimbo, rowid, versão e usuário.
Se o cabeçalho fosse cifrado, **ninguém caminharia pelo arquivo sem a chave**:
é o `tam_imagem` dele que diz onde começa o próximo evento. Reindexar, contar
eventos e pular volume deixariam de funcionar para quem só tem o arquivo.

O cabeçalho em claro não fica solto: ele entra como **dado associado** (AAD) na
etiqueta. Trocar o rowid de um evento, ou mover o corpo do evento 3 para o
evento 7, faz a etiqueta falhar. O que o cabeçalho em claro custa é
*metadado*: quem lê o arquivo sem a chave sabe **que** o rowid 42 mudou às
14h03, e não sabe **para que**. É a troca certa para um diário, e está escrita
aqui para ninguém supor mais do que ela dá.

**O nonce sai da ordem, não de um sorteio por evento.** Repetir o par (chave,
nonce) é o único jeito de quebrar isto sem quebrar a matemática, e num arquivo
que só cresce é fácil errar — basta alguém reabrir o arquivo e recomeçar a
contagem do zero. O tipo `cifra::Sequencia` fecha essa porta.

O número de ordem que entrou é o **offset do registro no volume**. Ele é o
contador que o arquivo já tem, que nunca se reaproveita num arquivo que só
cresce, e que **não precisa ser persistido** — um contador à parte teria de ir
a disco a cada registro, que é exatamente a escrita que o `.log` tirou do
caminho para não atrasar o `.reg`.

E cada volume sorteia o **próprio sal**, logo tem a própria chave: é isso que
deixa o offset — que recomeça em cada volume — ser o número de ordem.

**O único caso em que o offset se repetiria, e o que o cobre.** Uma queda no
meio da escrita deixa um rabo estragado; a cura corta esse rabo pelo CRC, e o
registro seguinte entra no offset que ele ocupava. Os 4 bytes de prefixo do
nonce cobrem isso, e eles saem de lugares diferentes conforme o arquivo:

| arquivo | prefixo do nonce | por quê |
|---|---|---|
| `.log` | 4 bytes sorteados por evento, no campo reservado do cabeçalho | o evento não tem identidade própria |
| `.trash` | os 4 últimos bytes do UUID v7 do descarte | o UUID já está lá e já é único |
| `.reason` | os 4 últimos bytes do UUID v7 do evento | idem |

Nos dois últimos **não há byte novo a gravar**. No `.log` são 4 bytes que já
estavam reservados.

**O que muda em cada arquivo:**

| | hoje | cifrado |
|---|---|---|
| cabeçalho do arquivo | 64 bytes, versão 2 | 128 bytes, versão 3: + flag, + sal de 16 bytes, + iterações do PBKDF2, + prova da chave de 16 bytes |
| registro | cabeçalho + corpo | cabeçalho (claro, vira AAD) + corpo cifrado + etiqueta de 16 bytes |
| custo | — | **+16 bytes por registro com corpo**, e nada nos registros sem corpo |

A `.trash` e o `.reason` seguem o mesmo padrão, com a mesma justificativa: o
que identifica a linha descartada fica legível, o conteúdo dela não.

**A prova da chave.** O cabeçalho da versão 3 leva 16 bytes que são a etiqueta
de uma mensagem vazia. Sem ela, uma senha errada no `config.json` só apareceria
na primeira leitura de corpo — que num diário ainda vazio seria nunca, e quem
digitou errado descobriria dias depois, com o arquivo cheio de registros
gravados com a chave errada e os antigos ilegíveis. Com ela, `abrir` recusa na
hora e diz o que está errado.

**Arquivo velho continua abrindo.** A versão 2 se lê como sempre; a flag de
cifrado é a que decide, e quem não a tem passa direto. Escrever cifrado é uma
decisão do `config.json`, não um padrão novo — a mesma regra da janela de
conflito: guarda nova entra pedida, não imposta.

### Como se liga

```json
"cifra": {
  "ligada": true,
  "senha_env": "PHXSQL_CIFRA",
  "iteracoes": 210000
}
```

Sem a seção, **nada muda**: o cofre nasce desligado e os três arquivos nascem
na versão 2, byte por byte como antes. `senha` no lugar de `senha_env` também
funciona, e é a opção pior pelo mesmo motivo de sempre — `config.json` costuma
ir para o controle de versão, variável de ambiente não. A senha nunca sai na
resposta do protocolo, nem no `Debug` da configuração: os dois têm teste.

O campo é lido em `Config::ler`, e não no servidor, porque a **CLI lê o mesmo
arquivo e abre o mesmo diário** — um campo que só o servidor aplicasse deixaria
`phxsql` sem a chave para ler o que `phxsqld` gravou. Campo de configuração que
só metade do programa lê é a mesma armadilha do campo que ninguém lê.

### Ligar a cifra não cifra o que já existe

Vale para os volumes criados **daqui para a frente**. Um `.log` que já existe
em claro continua em claro e continua abrindo — um arquivo *append-only* não se
reescreve, e não há comando de recifragem.

Isso está escrito aqui porque a surpresa seria pior que a limitação: quem liga
a cifra numa terça precisa saber que o diário de segunda não mudou de lugar. O
caminho para proteger o histórico antigo é o mesmo de sempre para dado que já
vazou de forma: copiar a tabela para uma nova, com a cifra já ligada, e apagar
a velha.

### O que a replicação faz com um `source` cifrado

**Nada muda para a réplica.** A cifra é do arquivo em repouso, e quem tem a
chave — o próprio servidor — lê a imagem da linha como sempre leu. O `posicao`
e o `replicar` continuam funcionando:

- `posicao` conta eventos pelos cabeçalhos dos volumes, que estão em claro;
- `replicar` devolve as imagens **já decifradas**, e elas viajam pela sessão
  autenticada como sempre viajaram.

Consequência que precisa estar escrita: **o dado continua indo em claro no
fio**. A cifra do diário não é TLS e não substitui o túnel da §7. O que ela
protege é o arquivo copiado — e uma réplica que grave o próprio diário cifrado
precisa da sua própria seção `cifra`, com a sua própria senha, porque o sal e a
chave são de cada arquivo.

O caminho rápido da replicação — a `MarcaDoDiario`, que faz o lote seguinte
começar de onde o anterior parou — continua valendo sem mudança, e isso não é
coincidência: o número de ordem do nonce é o offset, que a marca já carrega.
Um contador de eventos teria de ser recontado a cada lote.

### O que a chave protege, e o que não protege

A chave vem de `cifra::chave_de_senha`, que é o PBKDF2 que já existia. A senha
mora no `config.json` ou numa variável de ambiente — o servidor precisa
**apresentar** a chave para ler o diário, então não dá para guardar só o hash,
igual à senha do relé de e-mail e à do DbLink.

Isso protege **o arquivo copiado**: disco levado, *backup* vazado, cópia numa
máquina que não é esta. **Não protege** contra quem já lê o `config.json` da
máquina, porque quem lê o `config.json` tem a senha. Dizer o contrário seria
vender uma garantia que o desenho não dá.

### O que ainda falta

- **Os outros quatro arquivos.** `.reg`, `.ndx`, `.bin` e `.memo` continuam em
  claro. O `.reg` é o caso difícil: ele é de acesso aleatório por slot, e não
  *append-only*, então o nonce não pode sair do offset — reescrever um slot no
  mesmo lugar repetiria o par (chave, nonce). O desenho para ele precisa de um
  contador por slot no próprio slot, e isso é outra rodada.
- **Sem recifragem.** Ver acima.
- **Sem troca de chave.** Mudar a senha no `config.json` faz os volumes
  gravados com a antiga pararem de abrir. Não há `rekey`.

### O que os testes provam, e como

Em `crates/phxsql-store/tests/cifra-dos-diarios.rs` e
`crates/phxsql-server/tests/cifra-pelo-config.rs`:

| o que se prova | como |
|---|---|
| arquivo escrito antes da cifra continua abrindo | grava em claro, liga a cifra, lê os três arquivos e grava mais um evento |
| sem a seção `cifra`, nada muda no disco | confere versão 2 e 64 bytes de cabeçalho byte a byte |
| o dado some do disco | procura o texto claro nos bytes do arquivo, nos três |
| a chave errada e a falta de chave dão erro claro | confere a classe do erro e o texto |
| trocar o cabeçalho de um evento não passa | **conserta o CRC** e mesmo assim a etiqueta cai |
| o nonce nunca se repete | caminha pelo `.log` real e junta (sal, prefixo, offset) num conjunto |
| a replicação continua lendo | lotes de 50 com a marca do lote anterior |
| a cura funciona no volume cifrado | queda sem `sincronizar`, 120 eventos, nada se perde |
| o campo do `config.json` é lido de verdade | `Config::ler` e o `.log` nasce na versão 3 |
