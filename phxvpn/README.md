# phxvpn

Redes virtuais no estilo **Radmin VPN** — *criar rede* e *entrar na rede* com
nome e senha — sobre **OpenVPN**, com o cadastro de empresa, servidores,
usuários e redes num **PostgreSQL**.

Um binário (`phxvpn`), só `std` + o `phxsql-core` da casa: cliente PostgreSQL
(SCRAM-SHA-256), autoridade certificadora Ed25519, cofre da senha mestre
(XChaCha20-Poly1305) e o servidor HTTP da tela, tudo escrito aqui.

## Em três passos

```bash
cargo build --release
PHXVPN_PG="host=127.0.0.1 port=5432 user=phxvpn password=... dbname=phxvpn" \
  ./target/release/phxvpn painel --dados /var/lib/phxvpn --openvpn
# abra http://127.0.0.1:8470/ -> Instalação -> Criar rede -> baixa o .ovpn
```

Quem entra numa rede pela linha de comando:

```bash
phxvpn entrar --painel http://painel:8470 --usuario ana --rede Matriz --conectar
```

Ou pelo console, no estilo do prompt do MS-DOS (modos Painel, P2P e
Ferramentas; aceita arquivo de lote):

```bash
phxvpncmd /modo:painel /painel:http://painel:8470
phxvpn Painel> LOGIN admin
phxvpn Painel admin> CRIARREDE "Filial Sul" /FINALIDADE:ERP
```

A instalação pede: nome da empresa, finalidade, responsável, e-mail, telefone,
usuário e senha admin, **senha mestre criptográfica**, nome/IP/DNS do servidor
e, opcional, o certificado digital da empresa (PEM).

Um pendrive, token ou impressora de um membro pode ser usado pelos outros
(USB/IP, só dentro da rede):

```bash
phxvpn usb compartilhar 1-1.2 --rede Matriz   # Linux, quem tem o dispositivo
phxvpn usb usar 10.78.0.1 1-1.2               # Linux, ou Windows com usbip-win2
```

Como serviço do sistema (Linux), e os pacotes (`.deb`, `.msi`, `.zip`):

```bash
sudo PHXVPN_PG="host=... password=..." phxvpn servico instalar painel --openvpn
./empacotar.sh        # pacotes/ com SHA256SUMS
```

Desenho, provas, limites e o que falta: [`docs/PHXVPN.md`](docs/PHXVPN.md).
