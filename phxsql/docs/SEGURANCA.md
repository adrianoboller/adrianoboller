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
  "tentativas_para_bloqueio": 1,
  "tentativas_ate_bloquear": 5,
  "janela_minutos": 10,
  "bloqueio_minutos": 60,
  "whitelist": ["127.0.0.1", "192.168.50.0/24"],
  "blacklist": "blacklist.json"
}
```

Isto vale para **todo mundo, root incluso**, e é conferido **antes** do token.
Não é permissão de usuário — é o que este servidor não faz por esta porta.

Pedir um comando proibido **bloqueia o IP na hora**. Não há por que dar cinco
chances a quem pediu exatamente aquilo que o arquivo diz que ninguém pede.
`tentativas_para_bloqueio` existe para quem discorda: acima de 1, a operação
continua **recusada desde a primeira** — o que espera a enésima dentro da
janela é só o bloqueio do IP, e a resposta diz a contagem (`tentativa 2 de 3`).
Sem o campo, 1 — o comportamento de sempre. `bloqueio_minutos: 0` bloqueia
até alguém soltar.

### Duas gravidades

| | O que é | O que acontece |
|---|---|---|
| **Grave** | comando proibido, base proibida, travessia de diretório | bloqueia na tentativa `tentativas_para_bloqueio` (padrão: a primeira) |
| **Leve** | token errado, senha errada, IP fora da lista | conta na janela; bloqueia em `tentativas_ate_bloquear` |

Errar a senha uma vez é humano. Errar oito vezes em dois minutos, não.

### Whitelist: quem nunca bloqueia

`whitelist` aceita IP exato e faixa CIDR (`192.168.50.0/24`, `2001:db8::/32`),
e **vence sempre** — inclusive um bloqueio já gravado antes de a regra entrar:
a conferência acontece a cada conexão, então a regra nova vale na próxima, sem
esperar o bloqueio vencer. O que a whitelist **não** dá é poder: o comando
proibido continua recusado, só o IP fica livre. Proteção de acesso não é
licença.

São duas listas, e a união vale: a **fixa**, no `config.json` (muda com o
arquivo, como toda configuração), e a **editável pela tela de Bloqueios**, que
mora no `blacklist.json` — arquivo próprio pelo mesmo motivo do `dblink.json`:
o que muda pela tela não reescreve o config. Regra ilegível é recusada
inteira, sem gravar metade.

### `127.0.0.1` não tem exceção implícita

Decisão deliberada, e o motivo está em duas partes. Primeiro, uma exceção
embutida mudaria o comportamento que já existe — hoje o localhost bloqueia
como qualquer IP, e há teste de soquete que depende disso. Segundo, o operador
local **nunca fica trancado de verdade**: `phxsqld --desbloquear 127.0.0.1`
roda na máquina, mexe no arquivo sem passar pela porta, e o servidor relê
sozinho. Quem quiser a exceção pede por ela: `"whitelist": ["127.0.0.1"]`, que
é o que o exemplo de config sugere.

---

## 4. `blacklist.json`

```json
{
  "atualizado_em": "2026-08-27 19:30:17,323",
  "whitelist": ["203.0.113.50"],
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

Pelo protocolo, `{"op":"bloqueios"}` (que também devolve as duas whitelists e
a política em vigor), `{"op":"desbloquear","ip":"..."}` e
`{"op":"whitelist_salvar","whitelist":[...]}` — todos exigem `administrar`. A
tela Administração → Bloqueios cobre os três: soltar por linha, whitelist
editável e a exportação da §5.1. O campo `whitelist` deste arquivo é a lista
editável pela tela; a fixa mora no `config.json`.

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

### 5.1 Sem root: exportar a lista e aplicar por fora

A alternativa honesta ao firewall embutido: o servidor **entrega o texto**, e
quem tem o privilégio aplica. Pela tela de Bloqueios (botão Gerar) ou pelo
protocolo:

```json
{"op":"bloqueios_exportar","formato":"nftables"}
```

Uma linha por IP **ativo** — bloqueio vencido não sai, senão a exportação
recriaria no firewall o que o servidor já soltou. Quatro formatos:

| formato | cada linha | como aplicar |
|---|---|---|
| `texto` | `203.0.113.9` | o cru, para o seu script |
| `iptables` | `iptables -I INPUT -s 203.0.113.9 -j DROP` | revise e rode: `sh bloqueados.txt` (IPv6 sai como `ip6tables`) |
| `nftables` | `add element inet filter phxsql_bloqueados { 203.0.113.9 }` | `nft -f bloqueados.txt`, com os conjuntos criados antes (abaixo) |
| `fail2ban` | `fail2ban-client set phxsql banip 203.0.113.9` | revise e rode, com a jail `phxsql` existindo |

Os conjuntos que o formato `nftables` espera (uma vez só):

```bash
nft add set inet filter phxsql_bloqueados  '{ type ipv4_addr; }'
nft add set inet filter phxsql_bloqueados6 '{ type ipv6_addr; }'
nft add rule inet filter input ip  saddr @phxsql_bloqueados  drop
nft add rule inet filter input ip6 saddr @phxsql_bloqueados6 drop
```

E para quem prefere que o **fail2ban vigie sozinho**, o `acessos.log` já foi
desenhado para isso: JSON Lines com `ip`, `ok` e `codigo` estruturados. Um
filtro que casa recusa é
`failregex = ^.*"ip":"<HOST>".*"ok":false.*$` — ele não depende do TEXTO do
erro, e isso importa desde que o texto passou a poder mudar de idioma
([MENSAGENS.md](MENSAGENS.md)); o log grava o texto de fábrica justamente para
filtro nenhum quebrar.

### O que os testes do firewall provam, e a prova real

Em `blacklist.rs`, `servidor.rs` (`testes_firewall_e_mensagens`) e no soquete
(`tests/servico.rs`):

| o que se prova | como |
|---|---|
| sem o bloco `seguranca`, nada muda | config sem o bloco: nada proibido, nada conta, `phxsys` não nasce |
| grave bloqueia na primeira, como sempre | política default, texto byte a byte com o de antes |
| `tentativas_para_bloqueio: 3` recusa sempre e bloqueia na terceira | as duas primeiras respondem `tentativa N de 3` e não bloqueiam |
| a PRÓXIMA CONEXÃO do bloqueado é recusada na porta | soquete de verdade, com o erro nomeando desde/até |
| soltar de outro processo devolve a porta | mexe no arquivo como o `--desbloquear` e reconecta |
| whitelist nunca bloqueia, e vence bloqueio já gravado | grave + 100 leves contra IP na lista; bloqueia primeiro, whitelist depois, conexão volta |
| a exportação é uma linha por IP ativo | os quatro formatos, IPv6 no comando v6, vencido fora |

**Prova real, com o defeito reposto:** comentando a conferência de `protegido`
no `violacao_grave`, caíram três testes — `whitelist_nunca_bloqueia`,
`whitelist_recusa_sem_bloquear_e_vence_bloqueio_gravado` e o de soquete
`whitelist_no_soquete_recusa_sem_nunca_bloquear`. Teste novo que não cai com o
defeito reposto é pior que teste que falta; estes caem.

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
- **Sem BLOQUEIO por faixa.** O bloqueio é IP a IP; banir um `/24` inteiro
  exige o firewall (a exportação da §5.1 ajuda). A *whitelist* aceita CIDR —
  proteger uma faixa de administração é seguro por construção, banir uma faixa
  inteira automaticamente não.
- **As tentativas vivem em memória** — as leves e as graves contadas.
  Reiniciar o servidor zera os contadores; os bloqueios já gravados, não.

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

---

## 9. Gravar o `config.json` pela tela

Até a 0.18 as três telas de configuração só liam, e o comentário no código
dizia por quê: *gravar o `config.json` pela porta web significaria que uma
sessão roubada abre o firewall, troca a lista de comandos proibidos e cria um
supervisor*. O raciocínio estava certo sobre **aqueles** campos, e cobrava o
preço em todos os outros — ajustar o teto de linhas ou a hora do backup exigia
editar o arquivo à mão e reiniciar o serviço.

A operação `config_gravar` separa os dois grupos.

### A lista é fechada, e quem a declara é o servidor

`CAMPOS_EDITAVEIS`, em `crates/phxsql-server/src/config.rs`, é a única porta.
Cada entrada diz **o campo**, **o tipo** e **se aplica a quente**. A resposta
de `config` devolve essa lista em `editaveis`, e a tela monta o formulário
dela — não de uma cópia própria, que envelheceria calada no dia em que um
campo novo entrasse. É a mesma regra do catálogo das operações.

**Ficam de fora, e a ausência é decisão:**

| fora da lista | por quê |
|---|---|
| `token` | é a chave da porta; trocá-la pela web é trocar a fechadura |
| `seguranca.*` | comandos e bases proibidos, bloqueio, firewall |
| `root` e `usuarios` | cadastro é credencial |
| `cifra.*` | carrega a senha do cofre |
| `alertas.email.*` | carrega a senha do relé |
| `replicacao.*` | viraria este servidor para outro dono |
| `ips_permitidos`, `web.servidores`, `web.bind`, `base` | política de rede e de disco da mesma família |

### Os três portões da operação

1. **Permissão.** `administrar`, conferido **dentro** da operação. O portão
   geral do `despachar` também barra — a op não tem campo `"tabela"` e cai na
   regra da base vazia —, mas depender disso deixaria a guarda mais importante
   do servidor amarrada a um detalhe de resolução de nome de base. O teste que
   prova o portão próprio chama `executar` direto: pelo `despachar` ele passa
   igual com a conferência removida, e um teste que passa por engano é pior
   que um teste que falta.
2. **Tipo.** Os leitores usam `inteiro_ou`, que **cai no padrão em silêncio**
   quando o tipo não bate. Sem esta conferência, `"max_linhas": "abc"` seria
   gravado e nunca valeria — e ninguém descobriria pela tela.
3. **Validação.** A árvore alterada passa pelo mesmo `validar()` do arranque
   **antes** do `rename`. Valor que não subiria o servidor não entra no
   arquivo.

### O segredo não entra nem sai

A resposta da gravação devolve a configuração inteira já sem segredo — o
mesmo `para_json` que oculta token, senha do relé e senha da cifra. O teste
`o_segredo_nao_entra_nem_sai` cobre os dois sentidos: o campo secreto é
recusado na entrada, e a resposta de uma gravação válida não traz o token.
Pelo navegador, o DOM inteiro foi varrido atrás das quatro cadeias sensíveis:
nenhuma aparece.

### Gravação atômica, e o arquivo de quem escreveu

Escreve num `.tmp` e troca com `rename`, o mesmo que o cadastro do DbLink faz:
um corte de energia no meio deixa o `config.json` **antigo inteiro**, e não um
pela metade — que não subiria. O temporário herda as permissões do original,
porque o arquivo carrega o token e os hashes.

E a troca é **cirúrgica**: `Json::texto_trocar` acha o intervalo daquele valor
no texto e troca só ele. Reserializar a árvore preservava valor, ordem e
comentário — e perdia a **forma**: as linhas em branco entre as seções sumiam
e `["a", "b"]` virava três linhas. Num arquivo escrito à mão isso é devolver o
trabalho de alguém reformatado, e num controle de versão é um diff ilegível.
Campo que ainda não está no arquivo cai no caminho reserializado, porque
inserir texto exigiria adivinhar a indentação de quem escreveu — e adivinhar
errado é o mesmo estrago que reformatar.

### O que aprendemos exercitando a tela

Dois defeitos que ler o código não acharia:

- **O campo de arranque voltava com o valor velho, calado.** Gravar
  `timeout_s` gravava certo, e o redesenho lia a configuração **viva** — quem
  acabou de digitar 90 via 45 de novo, sem nada dizendo que o 90 estava no
  arquivo. Hoje a resposta traz `no_arquivo` com o que está gravado e ainda
  não vale, o campo mostra o gravado, um pino ao lado diz o que vale até
  reiniciar, e a moldura avisa quem chegar depois.
- **O primeiro conserto trouxe um falso positivo junto.**
  `alertas.livre_minimo_percentual` saía da resposta como o **texto**
  `"10.00"`, contra o número `10` do arquivo — e a tela passou a avisar sobre
  uma divergência que não existia. A causa era a resposta mentir sobre o tipo;
  o conserto foi devolver número, e não ensinar a comparação a tolerar texto.
  Resposta que mente sobre o tipo cria trabalho em quem a lê, e o trabalho
  nasce errado.

### Campo sem leitor, e o que ele escondia

Dois campos estavam no `config.json`, no `MANUAL.txt` e na tela desde a
0.13.0 e **nenhuma linha de código os lia** — a mesma armadilha do
`cache_paginas` antes de existir cache:

| campo | leitor que ganhou |
|---|---|
| `recursos.memoria_max_mb` | teto das tabelas residentes, conferido no `memoria_carregar` |
| `recursos.usuarios_max` | teto de logins distintos, conferido no `login` |
| `recursos.threads` e `recursos.cpu_percentual` | teto global de núcleos do trabalho dividido (`paralelo::definir_teto`) |

E o teto de memória **achou um defeito muito pior do que ele mesmo**: para
provar o teto era preciso carregar uma tabela residente, e `memoria_carregar`
tomava a trava global de dados **duas vezes**. `Mutex` da `std` não é
reentrante: a operação travava a si mesma e, com ela, o servidor inteiro —
toda operação de dados passa por aquela trava. Ninguém tinha percebido porque
**não havia teste que chamasse a operação**, e pela tela a chamada
simplesmente nunca voltava.

A lição é a do projeto, por outro caminho: *campo de configuração sem leitor é
pior que campo ausente* — e o leitor que faltava escondia uma operação
quebrada atrás dele. O teste novo carrega a tabela, consulta em memória e
exige que a resposta volte.

## 10. O Profiler: o que ele promete e o que ele cumpre

O Profiler (`crates/phxsql-server/src/profiler.rs`, tela em *Ferramentas →
Profiler*) mostra o texto dos pedidos **como chegaram pelo soquete**, uma
linha depois do `read_line` e uma antes do despacho. É o lugar do servidor
onde uma senha vazaria sem ninguém notar, porque a razão de ele existir é
mostrar texto cru — e o pedido de `login` traz a senha dentro.

Esta seção é a **validação** dele: o que foi provado, como, e o que a prova
achou. Tudo por soquete, contra um servidor de verdade, com uma sentinela
única no lugar da senha e um `grep` no anel **e** no arquivo depois
(`bancada/profiler/sonda.py`).

### 10.1 A redação: analisar cumpre o que recortar não cumpre

Vinte pedidos torcidos, todos com a mesma sentinela dentro. O que passou:

| o pedido chegou assim | o Profiler mostrou |
|---|---|
| `{"op":"login","senha":"SEGREDO"}` | `"senha":"***"` |
| `{ "op" : "login" , "senha" : "SEGREDO" }` | `"senha":"***"` |
| `{"\u0073enha":"SEGREDO"}` — chave escapada | `"senha":"***"` |
| `{"SENHA":"SEGREDO"}` — maiúscula | `"SENHA":"***"` |
| `senha_b64`, `prova`, `token`, `chave`, `assinatura` | `"***"` |
| `{"config":{"usuarios":[{"perfil":{"credenciais":[{"senha":…}]}}]}}` | `"***"` no fundo |
| `inserir_lote` com 200 linhas, senha em cada | 200 × `"***"` |
| `{"op":"login","senha":"SEGREDO"` — sem fechar | `<pedido invalido, 59 bytes>` |
| `senha=SEGREDO` — não é JSON | `<pedido invalido, 25 bytes>` |

Nenhum recorte de texto passa nesta tabela: a chave `\u0073enha` só é
`senha` **depois** do analisador, e o espaço antes dos dois-pontos já derruba
o `find("\"senha\":\"")`. Repondo o defeito — trocando o `redigir` por um
recorte — caem **sete** testes de uma vez, entre eles o
`chave_escapada_em_unicode_tambem_e_senha` e o `a_senha_nunca_aparece` que já
existia antes desta rodada.

E o caso que prova o outro lado: um `inserir` cujo campo `obs` vale
`ele disse "senha":"SEGREDO" no chat`. Isso é **dado**, não credencial, e
continua visível. O recorte erraria aqui para o lado de tapar o que não era
segredo — e tela que tapa dado de verdade manda o operador procurar em outro
lugar o que estava na frente dele.

### 10.2 O que a prova ACHOU, e o conserto de cada um

**Chave com espaço dentro das aspas.** `{"senha ":"SEGREDO"}` aparecia
inteiro. O servidor não lê `"senha "`, então essa chave nunca autenticou
ninguém — mas um cliente desastrado que a mande põe uma senha de verdade no
fio, e o Profiler a mostrava. A comparação passou a ser contra a chave
**aparada**: não se perde nada e fecha a porta.

**JSON válido que não é objeto.** `["op","senha","SEGREDO"]` virava texto
inteiro. A redação é por **nome de campo**, e um topo que não tem campo não
tem nome para tapar. Passou a virar o tamanho em bytes, pela mesma razão que o
malformado já virava: *o que não se analisa não vira texto*. O protocolo só
aceita objeto no topo, então não se perde pedido legítimo nenhum.

**O que continua exposto, e por quê.** Uma senha escrita **dentro do texto de
um SQL** — `{"op":"sql","texto":"… WHERE obs = 'SEGREDO'"}` — aparece, porque
o campo se chama `texto` e o valor é a consulta inteira. É o mesmo que o
Profiler do SQL Server(R) faz, e não há como resolver por nome de campo. Hoje
a camada SQL não tem nenhum comando que carregue credencial, então nada de
verdade viaja por aí; no dia em que tiver, este parágrafo vira um defeito.

### 10.3 O arquivo `.txt`: uma linha do log é UMA linha

O `pedido` sai seguro do `redigir` porque JSON escapa a quebra de linha. Os
outros campos da linha **não passavam por JSON nenhum**: `op`, `database` e
`tabela` vêm do corpo do pedido, e o `erro` carrega texto que o cliente
influencia.

Provado por soquete, num arquivo de verdade. Este pedido:

```json
{"op":"ping\n2000-01-01T00:00:00 9.9.9.9      forjado      ping   -  ok  0ms  0B  {}"}
```

deixou no `.txt` **duas** linhas — a segunda indistinguível de um evento real,
com outro IP, outro usuário e outro horário. Pelo campo `tabela` o mesmo
truque funcionava. Log de monitoração que aceita linha forjada não serve para
investigar nada: quem lê o arquivo depois de um incidente estaria lendo o que
o suspeito escreveu.

O conserto é no **nascimento** do evento, e não na hora de escrever — que é
onde alguém esqueceria de aplicar: todo campo livre entra no evento reduzido a
uma linha, com o caractere de controle **mostrado escapado** (`\n`, `\r`,
`\x00`) em vez de apagado, porque apagar em silêncio esconderia justamente a
tentativa. E com teto de tamanho: um `"op"` de dez mil bytes virava uma linha
de dez mil bytes no arquivo de quem só queria ver o que estava chegando.

### 10.4 O Profiler não era só do administrador — a ficha mentia

A ficha do `op_profiler_ligar` dizia «**Só administrador**» desde o primeiro
dia, e o `da_operacao` de fato pede `Atividade::Administrar` para as quatro
operações. Só que **nenhum pedido do profiler tem campo `"database"`**, então
o portão 3 do `despachar` pergunta «pode administrar a base *vazia*?» — e
`bases: {"*": {administrar: true}}` responde sim para quem é leitor.

É o furo do `juntar`/`unir` com o sinal trocado: lá o portão olhava um campo
que a operação não tinha e ela **escapava**; aqui ele olha um campo vazio e a
regra curinga a **deixa passar**. A telemetria já tinha aprendido isso e ganhou
o `portao_da_telemetria`; o profiler ficou para trás.

Provado por soquete (`bancada/profiler/sonda-permissao.py`), num servidor com
três usuários. O `curioso` — nível leitor, `ler` só em `loja`, `administrar`
na regra `"*"`:

```
ler folha.salarios  ->  acesso negado: curioso nao tem permissao de ler em folha.salarios
profiler_ligar      ->  LIGOU (arquivo escolhido por ele; 485 B escritos)
profiler            ->  adm  inserir  folha.salarios  {"op":"inserir",…,"quanto":987654}
```

Duas coisas de uma vez: ele leu a **linha** de uma tabela que o servidor
acabara de negar a ele, e mandou o servidor **criar e escrever um arquivo** no
caminho que ele escolheu. Somado ao forjar linha da seção anterior, isso era
acrescentar texto escolhido a qualquer arquivo que o processo do servidor
consiga abrir.

A telemetria mostra o **nome** da tabela; o profiler mostra a **linha**. Por
isso o conserto é o mesmo, e mais apertado: `portao_do_profiler` pergunta o
que o portão geral não consegue — **é administrador DESTE servidor?** — nas
quatro operações. E o teste que mais importa é o do comportamento **velho**,
`sem_cadastro_nada_muda`: servidor que sobe com token de serviço e sem
usuários não pode perder o profiler de um dia para o outro.

### 10.5 O que o arquivo faz, e o que ele não faz

Provado contra o sistema operacional, e não contra uma simulação
(`bancada/profiler/sonda-log.py`):

| situação | o que acontece |
|---|---|
| diretório não existe | recusa no `ligar`, com o caminho na mensagem |
| o caminho **é** um diretório | recusa: `Is a directory (os error 21)` |
| sistema de arquivos somente-leitura | recusa no `ligar` — o cabeçalho já falha |
| diretório `0500` **rodando como root** | aceita: o bit de permissão não vale para o uid 0, e testar com ele não prova nada |
| **disco enche depois de ligar** | ver abaixo |
| servidor reinicia | o profiler volta **desligado**; o arquivo sobrevive e religar **continua** nele (`append`) |
| o arquivo cresce | **345 B por pedido, sem rotação e sem teto** |

O disco cheio foi feito de verdade: um `tmpfs` de 64 KB montado só para isto.
Depois de 400 `inserir`, o anel tinha os 400 eventos, o arquivo tinha **223
linhas** e 65.536 B — e a resposta do `profiler` continuava dizendo
`ligado: true, arquivo: …`, sem uma palavra sobre as 177 linhas que foram para
o chão. O `let _ = writeln!(…)` engolia a falha.

Hoje a gravação é contada nos dois sentidos: `gravados_bytes` e
`falhas_de_escrita` saem na resposta, a tela pinta a caixa de **vermelho** e
diz quantas linhas não foram gravadas, e o rodapé do próprio arquivo registra
o número quando o profiler é desligado. Contar não conserta o disco — troca um
log que mente por um log que avisa. O teste usa `/dev/full`, que aceita a
abertura e recusa toda escrita com `ENOSPC`: é o disco cheio sem esperar o
disco encher.

**A rotação continua não existindo**, e agora com número: 345 B por pedido
significa que um profiler esquecido ligado num servidor com 1.000 pedidos por
segundo escreve **1,2 GB por hora**. O anel de memória tem teto desde sempre;
o arquivo não tem. Está anotado nas pendências — o que existe hoje é a tela
mostrando o tamanho, para quem esqueceu ver o arquivo crescendo antes de ele
comer a partição.

E o reinício merece uma frase, porque é uma escolha e não um esquecimento: o
profiler é uma **sessão de observação**, não configuração. Quem o liga e vê o
servidor reiniciar precisa ligá-lo de novo — o arquivo continua lá, com o
cabeçalho novo separando as duas sessões.

### 10.6 O que a tela escondia

Uma letra. A caixa de estado usava `class="aviso bem"`, e a classe verde desta
interface chama-se `bom` — não existe `.bem` no CSS. A caixa «observando
desde…» passou a vida inteira cinza, igual à de «parado». Nenhum teste pega
isso, e ler o código também não: é a lição do vídeo por outro caminho —
*componente novo se abre no navegador e se olha*.
