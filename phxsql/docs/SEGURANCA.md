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

### 5.2 O desenho da replicação, provado em contêiner — e o campo que mentia

A §7 do [REPLICACAO.md](REPLICACAO.md) desenha um Source que aceita entrada
**só** do IP da Réplica e **não alcança ninguém**. Com processos no mesmo
`127.0.0.1` não há o que trancar: todo mundo se enxerga por construção, e o
desenho fica sendo um desenho. Em contêiner ele vira medição —
`bancada/replicacao/docker/`, estágio (e): uma rede própria, IPs fixos, e um
**intruso** com o `config.json` de réplica vazado (mesmo token, mesmo usuário,
mesmo `senha_hash` — o modelo de ameaça de quem perde um arquivo de
configuração, não o de quem quebra criptografia).

| tranca | eventos que o intruso levou do diário |
|---|---:|
| nenhuma | **200 de 200** |
| `replicas_autorizadas` *(antes do conserto)* | **200 de 200** |
| `replicas_autorizadas` *(hoje)* | 0 |
| `ips_permitidos` | 0 |
| `iptables` da §7 no namespace do source | nem abre a porta |

A segunda linha é o achado: **`replicas_autorizadas` estava no `config.json`,
na §7 e na tela, e nenhuma linha de código o lia**. O conserto está descrito
na §7 do REPLICACAO.md, com as três garantias de sempre — lista vazia é o
comportamento de sempre, portão único, e a pergunta obrigatória sobre quem
*não* tem o campo novo (job e rotina interna chegam com `ip` vazio).

Duas camadas apareceram sem ninguém pedir, e as duas são boas notícias:
**86 linhas** no `acessos.log` com o IP do intruso, e o IP **na lista negra**
do source ao fim da fase — bater na porta recusada dezenas de vezes é
exatamente o que a política da §3 conta. E há uma diferença que só o
`iptables` mostra: `ips_permitidos` **recusa** (o pacote chega e leva um
«não»), o `DROP` do firewall **some** com o pacote — o intruso leva *timeout*,
e nem fica sabendo que há algo ali.

A metade do desenho que nunca tinha sido provada é a de saída: com
`-A OUTPUT -m conntrack --ctstate ESTABLISHED,RELATED -j ACCEPT` seguido de
`-A OUTPUT -j DROP` no namespace do source, **a replicação continua inteira** —
porque quem abre a conexão é sempre a réplica — e o source **não consegue
abrir conexão para ninguém**. Medido, nos dois sentidos.

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

### 10.6 As hipóteses que morreram medidas

Vale escrevê-las com o número, senão voltam: recusa medida é resultado, e é o
que impede a mesma ideia de ser reimplementada por intuição.

**«O campo `erro` vaza credencial.»** Ele vai para a tela e para o `.txt` sem
passar pela redação — só pela redução a uma linha —, e a suspeita era boa: o
texto do erro carrega dado do pedido. Percorridos os caminhos que recebem
credencial, nenhum a devolve no erro. O `login` responde a **mesma** mensagem
para login errado, senha errada e desafio vencido, de propósito (é o que
impede sondar quem existe). O `senha_b64` malformado responde
`base64 invalido: '#'` — **o caractere ofensor, que por definição não pertence
ao alfabeto Base64 e portanto não é pedaço da senha**. E `extrair_hash`
recusa dizendo o *login*, nunca o valor. Redigir o `erro` custaria a
explicação — que é a razão de o profiler existir — e não compraria nada
medível. Fica **não redigido, e reduzido a uma linha**; o dia em que algum
erro passar a ecoar o valor de um campo, isto vira defeito.

**«O custo do Profiler ligado é o disco.»** Parecia óbvio: um `writeln!` mais
um `flush` por evento. Medido, o que dói é o **anel**, não o arquivo — e
dentro do anel não era nem o parse, era a varredura do `terminou`
(§2.3.2 de `DESEMPENHO.md`). `File::flush` da `std` não faz nada, e o `write`
vai para a cache de páginas: são microssegundos contra os 103 µs da varredura.

**«As aspas escapadas dentro de um valor são um vazamento.»** O campo `obs`
com `ele disse "senha":"…" no chat` **aparece inteiro, e tem de aparecer**.
Não é credencial: é o que alguém digitou numa tabela. Tapá-lo seria o erro
simétrico — a tela mentindo sobre o dado, que é a mesma falta do «BLUMENAU»
em maiúscula. Há teste que falha se o `redigir` tapar isso.

### 10.7 O que a tela escondia

Uma letra. A caixa de estado usava `class="aviso bem"`, e a classe verde desta
interface chama-se `bom` — não existe `.bem` no CSS. A caixa «observando
desde…» passou a vida inteira cinza, igual à de «parado». Nenhum teste pega
isso, e ler o código também não: é a lição do vídeo por outro caminho —
*componente novo se abre no navegador e se olha*.

---

---

## 11. A cifra de dados: por coluna marcada

A §8 cifrou os **diários**. Os arquivos de **dados** continuaram em claro, e a
própria §8 registrou a dívida: *«`.reg`, `.ndx`, `.bin` e `.memo` continuam em
claro»*. Esta seção paga parte dela — e diz, com todas as letras, qual parte
não paga.

O que entrou: **o valor de uma coluna marcada como dado pessoal
(`DadoPessoal`) vai cifrado no `.reg`, no `.memo`, no `.bin` e no espelho
`.bkp`.** Coluna não marcada continua exatamente como estava.

### 11.1 As quatro saídas, e por que esta

Antes de escrever código, as quatro foram medidas contra o mesmo alvo — uma
tabela de 500 mil linhas, `Str(40)` como coluna sensível, inserção medida em
**10,4 µs por linha** (`--example onde-doi`, 50 mil linhas, 2 índices).

O AEAD que já existe faz **330 MB/s** nesta máquina; um payload de 128 bytes
custa **0,585 µs** para selar e uma página de 4 KiB, **11,7 µs**
(`--example custo-da-cifra`).

| saída | custo por linha | disco a mais | o que ela quebra |
|---|---|---|---|
| **(a) o slot inteiro** | 0,59 µs (5,7% da inserção) | 16 B/linha | nada — mas cifra colunas que ninguém pediu |
| **(b) página do `.ndx`** | **0,23 µs** (2,2%) | 32 B/página | nada; e o número surpreende — ver abaixo |
| **(c) coluna marcada** ✅ | **0,10 µs** (1,0%) | 16 B/linha | o índice sobre a coluna marcada |
| **(d) arquivo inteiro em volumes** | **194 ms para ler UMA linha** | 0 | o O(1); é a saída que não existe |

**(d) morreu com número na mesa.** Um volume de 500 mil linhas com slot de 128
bytes tem 64 MB. A 330 MB/s, decifrá-lo inteiro para ler uma linha custa
**194 ms** — contra 0,6 µs hoje. São **320 000×**. Cifrar o arquivo inteiro é
trocar um banco de dados por um arquivo compactado.

**(b) foi a surpresa, e o registro fica aqui porque a hipótese ingênua estava
550× errada.** A conta óbvia dizia: a inserção toca **10,86 páginas por linha**
(medido, `--example onde-doi`), a 11,7 µs por página são **127 µs por linha** —
doze vezes a inserção inteira, proibitivo. Medido de verdade, o `.ndx` grava
**0,02 página por linha** no arquivo: o cache de páginas com *write-back*
absorve todos os outros toques. A cifra entraria exatamente onde o CRC-32 já
está — em `escrever_pagina` e `ler_pagina` —, e custaria **0,23 µs por linha**.
*Toque de página não é gravação de página*, e a diferença entre as duas é o
cache inteiro.

**(a) e (c) diferem pouco em custo e muito em significado.** (a) cifra o
payload inteiro, inclusive as colunas que ninguém marcou; (c) cifra só o que
foi declarado sensível. O dono escolheu **(c)** — e o que decide não é o
microssegundo, é que a marca LGPD já existe no esquema (PSCH v6), já tem tela,
e já é a declaração de quem sabe o que é sensível. **O motor não adivinha.**

### 11.2 Como funciona

**O texto cifrado cabe no lugar do claro.** O ChaCha20 é cifra de fluxo: o
cifrado tem exatamente o tamanho do claro. Uma coluna `Str(40)` continua com 40
bytes, no mesmo *offset*, e **nenhum offset de coluna se move**. O que não cabe
é a etiqueta — e ela é **uma só para a linha inteira**, cobrindo todas as
faixas marcadas juntas, no fim do slot.

```text
slot cifrado: [cabeçalho 24][payload, com as faixas marcadas cifradas][etiqueta 16]
                             ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
                             mesmos offsets, mesmos tamanhos
```

`offset = data_offset + (slot - 1) * slot_size` continua valendo: `slot_size`
cresce 16 bytes, uma vez, e **o endereço continua saindo de uma conta**.

| | onde | quanto custa |
|---|---|---|
| coluna marcada, inline | no próprio payload | 16 B por linha, para a linha toda |
| coluna marcada, `Memo`/`Bin` | no bloco do `.memo`/`.bin` | 40 B por valor (nonce de 24 + etiqueta de 16) |
| cabeçalho do `.reg` | 128 → 192 bytes, versão 5 | 64 B por volume |

**Por que `Memo`/`Bin` não entram na faixa do payload.** O que mora no payload
deles é um **ponteiro de 16 bytes** para o outro arquivo. Cifrar o ponteiro não
esconde nada — o conteúdo está no `.memo`, em claro, ao alcance de quem copiou
o diretório. Seria a aparência da proteção com o conteúdo aberto do lado. O
conteúdo é selado antes de virar bloco, com nonce sorteado de 192 bits à frente
(é para isso que o XChaCha20 existe).

**O nonce, e o que impede repeti-lo.** Repetir o par (chave, nonce) é o único
jeito de quebrar isto sem quebrar a matemática — e um slot do `.reg` é
reescrito **no mesmo lugar** a cada alteração, onde o endereço não serve de
contador. O nonce de 192 bits carrega três coisas:

```text
[rowid u64][volume u32][versão da linha u32][tempero u64 sorteado]
```

- **onde mora** separa dois slots;
- **a versão** separa duas gravações do mesmo slot — é o contador que o formato
  já tinha;
- **os 8 bytes sorteados** ficam de pé quando o contador não fica. Uma gravação
  perdida no cache do sistema antes do `fsync` faz a seguinte repetir a versão;
  com 64 bits sorteados, repetir exige colisão de aniversário. Eles moram nos
  bytes 16..24 do cabeçalho do slot, que **já eram reservados**: custam zero de
  formato.

O teste `regravar_a_mesma_linha_nunca_repete_o_texto_cifrado` grava 200 vezes o
**mesmo** conteúdo e exige 200 textos cifrados diferentes.

**O dado associado amarra o endereço.** A etiqueta cobre
`[volume][rowid][versão]`. Sem isso, quem tem o arquivo e não tem a chave ainda
poderia **embaralhar as linhas**: copiar os bytes do slot 5 sobre o slot 9,
consertar o CRC-32 — que é público — e a linha 9 passaria a devolver o conteúdo
da 5 sem erro nenhum. Cifra sem essa amarração protege o conteúdo e não protege
a tabela. O teste `trocar_o_corpo_de_uma_linha_pela_outra_nao_passa` faz
exatamente esse ataque, com o espelho estragado junto.

O `status` fica **fora** do dado associado, de propósito: excluir regrava só os
24 bytes de cabeçalho e não toca no corpo, e um `status` amarrado faria o corpo
de toda linha excluída parar de abrir.

**O CRC-32 cobre o que está no disco**, e não o texto claro. É o que deixa
`reparar` e o espelho `.bkp` trabalharem **sem a chave** — a mesma escolha da
§8 — e um CRC do claro guardado ao lado do cifrado seria um oráculo de 32 bits
para quem quisesse adivinhar o conteúdo.

### 11.3 O que continua em claro

Toda escolha aqui deixa algo em claro. Esconder isso seria pior que não cifrar.

| continua legível | por quê |
|---|---|
| **o `.ndx` sobre a coluna marcada** | um índice guarda a chave para poder **comparar**. Cifrar a chave destrói a ordem, e sem ordem não há B+tree — seria trocar o índice por uma varredura |
| toda coluna **não** marcada | é a escolha (c): o motor cifra o que foi declarado |
| o **bitmap de nulos** | diz **quais** linhas têm a coluna marcada vazia |
| o `rowid`, a versão da linha, o status do slot | é por eles que se anda no arquivo sem a chave — e é o que faz o reparo funcionar |
| o **esquema**, inclusive o nome da coluna marcada | `porCPF` já conta o que a tabela guarda |
| o **tamanho** de um `Memo` marcado | o bloco tem o comprimento no cabeçalho |
| o **`.ndx` inteiro**, o `.pag`, o catálogo | não entraram nesta rodada |
| **o tráfego** | continua sem TLS (§7). A cifra é do arquivo em repouso |

Isto está num teste, e não só aqui:
`o_indice_sobre_a_coluna_marcada_continua_em_claro` **prova o vazamento** —
procura o nome dentro do `.ndx` e exige achá-lo. Se um dia o `.ndx` for
cifrado, o teste cai, e cair é o aviso para apagar esta linha da tabela acima.

> **Um banco que diz «cifrado» e vaza a chave pelo índice está mentindo para o
> usuário.** Uma tabela com coluna marcada e índice sobre ela protege o
> `.reg` copiado, e **não** protege contra quem copiou o `.ndx` junto. Quem
> precisa dos dois deve tirar o índice da coluna sensível.

### 11.4 O modo FrogCript

O **FrogCript** é do Adriano Boller (Wx Soluções). Ele parte o texto pelo
**pulo** — as casas 5, 10, 15… saem —, inverte ou não o extraído conforme a
**direção**, e envolve os dois lados em duas camadas, com a direção escondida
dentro da segunda:

```text
AEAD( AEAD(resto) )  |  AEAD( d + AEAD(extraído) + d )
```

Ele entra como **modo escolhido**, e o padrão do motor continua sendo o AEAD
direto. Três coisas precisam estar ditas sem rodeio e sem desdém.

**Primeira: o que ele acrescenta.** O formato. O pacote tem a forma que o autor
definiu, e um leitor que não conheça a convenção não remonta o texto nem depois
de abrir os dois lados. Mais o salto e o separador personalizáveis (§10 do
documento dele), que quem personaliza passa a tratar como parte do segredo.

**Segunda: o que ele não acrescenta.** Força criptográfica. A frase é do próprio
autor, na §9 do documento dele:

> O pulo 5 e a direção **não são a chave**. A chave é a senha.

A transposição é uma permutação **fixa e pública**: quem tem o texto cifrado só
a vê depois de abrir o AEAD, e quem abriu o AEAD já tem tudo. Duas camadas com
a **mesma** chave também não somam segredo. O que segura o conteúdo é o AEAD e
o tamanho da senha — exatamente como no modo padrão.

**Terceira: o que ele custa.** Medido, não estimado (`--example
custo-da-cifra`, valor de 22 bytes):

| | tempo | tamanho |
|---|---|---|
| AEAD direto | **0,10 µs** | 38 B |
| FrogCript, como está aqui | **2,77 µs** (27×) | 189 B (5×) |
| `frogcript.py` de referência | **1 137 ms** (410 000×) | ~397 B (18×) |

O 1,1 s por valor não é exagero: o `frogcript.py` deriva a chave com
**PBKDF2 de 200 000 iterações a cada uma das quatro selagens**, com sal próprio
— e um PBKDF2 de 210 000 custa **298 ms** medidos nesta máquina. Cifrar uma
tabela de 100 mil linhas com ele levaria **31 horas**. Aqui a chave é derivada
**uma vez por arquivo** e guardada, que é a mesma decisão da §8.

No `.reg`, como o pacote **não cabe** no lugar do texto claro, a faixa marcada
do payload vai a zeros e o pacote inteiro mora no rabo do slot: o custo de
disco é `largura marcada + 167` bytes por linha, contra 16 no modo padrão. Para
um `Str(40)`, são **207 bytes por linha em vez de 16**.

E o separador continua mostrando que existem dois blocos — o autor já diz isso
na §3 dele.

#### O AES: a decisão que não é minha

O FrogCript de referência usa **AES-256-GCM**. O `cifra.rs` desta casa **não
tem AES**, e o cabeçalho dele explica por quê: AES portátil, sem a instrução do
processador, se escreve com tabelas, e tabela em cache vaza a chave pelo tempo
de acesso.

O modo aqui implementado mantém a **estrutura** do FrogCript sobre o
ChaCha20-Poly1305 já conferido contra o RFC 8439. A consequência, escrita e não
escondida:

> **Um pacote produzido aqui NÃO abre no `frogcript.py`, e um pacote produzido
> pelo `frogcript.py` NÃO abre aqui.** A estrutura é a mesma; a cifra de dentro
> não é. **Não há compatibilidade com o que foi cifrado por fora em Python.**

Escrever AES aqui para ganhar essa compatibilidade seria pôr alguns milhares de
linhas novas de código criptográfico no caminho de todo dado pessoal do banco,
para substituir uma cifra que já tem vetor oficial nos testes. **É uma decisão
de peso e é do dono** — este documento a coloca na mesa com o custo, não a toma.

Se ela for tomada um dia, o caminho honesto é AES **bitsliced** (tempo
constante, sem tabela), e não a versão de tabela: a de tabela seria trocar «não
tem AES» por «tem um AES que vaza a chave por tempo», que é pior que não ter.

#### Como se liga

```json
"cifra": {
  "ligada": true,
  "senha_env": "PHXSQL_CIFRA",
  "iteracoes": 210000,
  "modo": "frogcript",
  "salto": 5,
  "separador": "|"
}
```

Sem `modo`, **`aead`** — e sem a seção `cifra` inteira, nada muda. O modo vai
gravado **no arquivo**, e não só na configuração: trocar `modo` no
`config.json` numa terça não pode fazer toda tabela gravada antes parar de
abrir com a mensagem errada, mandando procurar corrupção onde só há um
interruptor trocado. O teste é `o_modo_sai_do_arquivo_e_nao_da_configuracao`.

O salto e o separador **não** vão gravados: o autor pede que sejam tratados
como parte do segredo, e segredo não se grava ao lado do dado que ele protege.
Quem os perde perde o dado, como quem perde a senha —
`salto_e_separador_personalizados_viram_parte_do_segredo` prova os dois lados.
A resposta de `config` oculta os dois **quando saem do padrão**, pela mesma
razão: o padrão está publicado, um personalizado é segredo.

### 11.5 A chave

**Chave ao lado do dado protege pouco**, e isso já estava escrito na §8: quem
lê o `config.json` tem a senha. O que a cifra protege é o **arquivo copiado**
— disco levado, *backup* vazado, cópia numa máquina que não é esta.

O que existe hoje:

- a chave sai de **PBKDF2-SHA256** sobre a senha, com **sal por arquivo** (16
  bytes, em claro no cabeçalho — sal não é segredo, o papel dele é impedir que
  a mesma senha derive a mesma chave em dois arquivos);
- **210 000 iterações** por padrão, com piso de 10 000;
- a derivação é **guardada por (sal, iterações)**, porque o servidor abre e
  fecha a tabela a cada pedido e 298 ms por abertura transformaria a cifra numa
  escolha entre proteger e responder;
- **trocar a senha limpa esse cache** — sem isso, um servidor que já tivesse
  aberto o arquivo continuaria aceitando a senha antiga. *(Defeito encontrado
  ao escrever o teste `senha_errada_e_falta_de_senha_param_na_abertura`: ele
  passava com a senha errada.)*
- uma **prova da chave** de 16 bytes no cabeçalho recusa a senha errada **na
  abertura**, e não na primeira leitura de linha — que numa tabela recém-criada
  seria nunca.

**O que ainda não existe, e o desenho para quando existir:**

- **Envelope.** Hoje a chave do arquivo *é* a derivada da senha. O certo é uma
  **chave de tabela sorteada**, guardada no cabeçalho **envelopada** pela chave
  mestra derivada da senha (`AEAD(chave_mestra, chave_da_tabela)`, 32 + 16
  bytes). Com ela, trocar a senha mestra reescreve **48 bytes por arquivo** em
  vez de reescrever a tabela inteira. É a mudança que precisa entrar **cedo**,
  enquanto não há dado em produção — depois vira migração.
- **Rotação.** Com o envelope, `rekey` é: abrir o envelope com a senha velha,
  fechar com a nova, regravar o cabeçalho. Sem o envelope, rotacionar exige
  reescrever e recifrar **toda linha**, e é por isso que hoje **não há
  rotação**: trocar `cifra.senha` faz os arquivos gravados com a antiga
  pararem de abrir, com o erro da prova da chave dizendo exatamente isso.
- **Argon2id (RFC 9106).** O PBKDF2 não resiste a GPU; o Argon2id resiste. Ele
  **não entrou nesta rodada** e o motivo é ordem de risco: mexer no hash de
  senha (`senha.rs`) toca o login de todo mundo, e a regra da casa manda que o
  teste que mais importa seja o do comportamento velho
  (`senha_velha_continua_entrando`). É trabalho de uma frente própria, com o
  vetor do RFC 9106 no teste e o hash antigo continuando a abrir.

**Quando alguém erra a senha**, a prova da chave falha e a abertura para com
`Autorizacao`, nomeando o arquivo e o campo do `config.json`. **Sem** senha, o
erro é outro e diz onde preenchê-la. Nenhum dos dois devolve lixo, e nenhum dos
dois cifra por cima.

### 11.6 Ligar a cifra não cifra o que já existe

Vale para as tabelas criadas **daqui para a frente**, pela mesma razão da §8: o
`.reg` não se reescreve inteiro ao abrir, e não há comando de recifragem. Uma
tabela criada antes continua na versão 4 e continua abrindo — o teste
`tabela_escrita_antes_da_cifra_continua_abrindo` grava sem cifra, liga a cifra,
lê, insere e altera.

**Desligar também não decifra.** Um `.reg` da versão 5 aberto sem a chave para
com erro claro, e não devolve texto cifrado como se fosse texto.

E **desmarcar a coluna não decifra**: um arquivo que diz ter cifrado com um
esquema que não tem mais coluna marcada é recusado, em vez de devolver bytes
cifrados como se fossem o nome do cliente.

**Uma tabela sem coluna marcada nasce em claro mesmo com o cofre ligado.** Não
há o que cifrar, e carimbá-la de cifrada custaria 16 bytes por linha e um
cabeçalho maior para não proteger nada.

### 11.7 O diário, a lixeira e a trilha

**A decisão mais importante desta frente**, e ela não é sobre o `.reg`: *uma
trilha que guarda em claro o que a tabela cifra anula a cifra.*

Onde o motor grava o valor de uma linha, hoje:

| onde | o que vai | como fica |
|---|---|---|
| `.memo`/`.bin` | o conteúdo | **cifrado**, quando a coluna é marcada |
| `.trash` (lixeira) | o payload + o conteúdo dos externos | o externo vai **como está no bloco** — cifrado |
| `.log` (imagem da linha) | idem | idem |
| `.reason` | só o motivo escrito por gente | em claro, e é texto de operador |

O conteúdo externo vai para a lixeira e para o diário **como está no bloco**, e
não decifrado. Decifrar ali poria o texto claro dentro da imagem do diário e
dentro da lixeira — exatamente o que a cifra da coluna existe para impedir.

**O que ainda vai em claro na imagem: a faixa inline.** A imagem da linha
carrega o *payload* como ele foi montado, antes de o `.reg` selá-lo — então uma
coluna marcada **inline** aparece em claro dentro da imagem. Isso é seguro **só
porque o corpo do diário é cifrado pelo cofre** (§8) — e os dois vêm da mesma
seção `cifra`, com a mesma senha: não há como ligar a cifra de coluna sem ligar
o cofre.

Sobra **um** buraco real, e ele fica escrito: um volume de `.log` ou de
`.trash` que **já existia em claro** antes de a cifra ser ligada continua em
claro, e receberá imagens novas em claro. A saída é a mesma da §8 — a proteção
vale para os volumes criados daqui em diante; para o histórico, copiar para uma
tabela nova com a cifra já ligada.

**Se a trilha de auditoria (`.lgpd`) passar a gravar valor-antes e
valor-depois de coluna marcada, ela tem de gravar o texto cifrado ou uma marca
de redação — nunca o claro.** Gravar o claro numa trilha que ninguém cifra
desfaz tudo o que está nesta seção, e desfaz em silêncio.

### 11.8 A replicação de uma coluna cifrada

O conteúdo externo viaja **cifrado** na imagem, então a réplica só o abre se
tiver a **mesma chave** — e a chave sai da senha **mais o sal do arquivo**, que
é sorteado por arquivo. Consequência, dita em vez de descoberta:

> **Replicar uma tabela com coluna `Memo`/`Bin` marcada só funciona entre
> servidores que compartilham a senha da cifra E o sal do arquivo de origem.**
> Sem isso, a réplica recebe bytes que não abre.

É o mesmo limite que o envelope da §10.5 resolveria: com a chave da tabela
sorteada e envelopada, ela pode ser entregue à réplica sem entregar a senha
mestra. Enquanto o envelope não existe, a recomendação é **não replicar tabela
com coluna externa marcada** — e o tráfego continua sem TLS (§7), o que é o
problema maior nessa mesma frase.

### 11.9 O que os testes provam, e a prova real

Em `crates/phxsql-store/tests/cifra-dos-dados.rs`,
`crates/phxsql-store/tests/cifra-modo-frogcript.rs` e nos módulos
`phxsql_core::cifra` e `phxsql_core::frogcript`:

| o que se prova | como |
|---|---|
| tabela escrita antes da cifra continua abrindo | grava em claro, liga a cifra, lê, insere e altera; confere que a versão do arquivo **não** mudou |
| sem a seção `cifra`, nada muda | exige que o nome apareça legível no `.reg`, no `.memo` e no `.ndx` |
| o dado some | procura o texto claro no `.reg`, no `.memo` **e no espelho `.bkp`** |
| a chave errada e a falta de chave param na abertura | confere a classe do erro e o texto |
| embaralhar as linhas não passa | copia o slot 5 sobre o 9 **com o CRC certo**, estraga o espelho junto, e exige erro |
| o nonce nunca se repete | grava 200 vezes o **mesmo** conteúdo e exige 200 textos cifrados diferentes |
| o `.ndx` vaza — de propósito | procura o nome dentro do `.ndx` e exige **achá-lo** |
| o custo do FrogCript é o escrito | subtrai o `slot_size` em claro do cifrado e compara com a conta do módulo |
| o modo sai do arquivo | grava em AEAD, troca o processo para FrogCript, e a tabela antiga continua abrindo |
| salto e separador são segredo | grava com `(7, '#')`, tenta abrir com `(5, '|')`, exige erro que nomeia o separador |
| HChaCha20 e XChaCha20-Poly1305 | **vetores oficiais** do draft-irtf-cfrg-xchacha-03, §2.2.1 e §A.3.1 |
| o pulo do FrogCript | o exemplo da §7 do documento do autor, letra por letra |
| o pulo conta caractere, e não byte | `ADRIANO JOSÉ BOLLER` — o `É` ocupa dois bytes e uma casa |
| o gerador de bytes do processo não repete | 5 000 blocos de 16 bytes, todos distintos e nenhum zerado |

**Prova real, com o defeito reposto:**

| defeito reposto | o que caiu |
|---|---|
| tirar o `aad` do `montar_slot`/`abrir_slot` | `trocar_o_corpo_de_uma_linha_pela_outra_nao_passa` passa a **ler a linha trocada** |
| tirar a `versão` e o `tempero` do nonce | `regravar_a_mesma_linha_nunca_repete_o_texto_cifrado` acha texto cifrado repetido |
| não limpar o cache de derivadas em `definir` | `senha_errada_e_falta_de_senha_param_na_abertura` **abre com a senha errada** — foi assim que o defeito apareceu |
| trocar `chars()` por `bytes()` no `pular` | `o_pulo_conta_caractere_e_nao_byte` devolve texto inválido |
| tirar a `FLAG_FROGCRIPT` da leitura do material | `o_modo_sai_do_arquivo_e_nao_da_configuracao` cai |

### 11.10 Aprendizado, inclusive o infrutífero

**A hipótese que morreu, e o número que a matou.** «Cifrar o `.ndx` custa 127 µs
por linha, doze vezes a inserção inteira» — errado por **550×**. A conta usava
*toques de página* (10,86 por linha) onde o certo era *gravações de página*
(0,02 por linha): o cache com *write-back* absorve o resto. O custo real seria
**0,23 µs por linha**, 2,2% da inserção. A recusa virou aceitação com o número
na mesa — e o `.ndx` só ficou de fora porque a escolha do dono foi por coluna,
não por preço.

**O corolário:** *toque de página não é gravação de página.* É a terceira vez
que este projeto tropeça na mesma pedra por outro caminho (o CRC do `.ndx`, o
cabeçalho por linha, e agora este).

**O que a medição comprou de graça.** O AEAD custa **0,585 µs** para 128 bytes,
dos quais **0,12 µs são a alocação do `Vec`** — 40% do total nesse tamanho, e
quase nada a 4 KiB. Uma API que sele no lugar devolveria ~1% da inserção. Não
entrou: 1% não paga uma API nova no caminho de todo dado sensível, e o número
fica escrito para quem um dia precisar dele.

**O que a leitura do FrogCript comprou.** O documento do autor é honesto na §9,
e a implementação de referência é 410 000× mais cara do que ele imagina — não
pela cifra, e sim por derivar a chave quatro vezes **por valor**. O erro não é
de criptografia, é de onde a derivação mora. Aqui ela mora no arquivo, uma vez,
e é o mesmo motivo pelo qual a §8 guarda a chave derivada.
