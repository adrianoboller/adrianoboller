# A bancada de conexões — itens K e L do PDF das 26 perguntas

Existe porque o pedido do dono («exemplo de connection com acesso nativo,
ODBC e OLEDB» e «exemplo de uso DBLINK MULTILINK DATABASE») pede prova contra
o motor vivo, e o que já existia em `bancada/` cobria ODBC e o DbLink
motor-a-motor, mas não a conexão **nativa** crua (soquete + JSON, com o
desafio-resposta calculado à mão, sem `phxsql-cmd`) nem o desenho **hub**, em
que um único PhxSql liga para *mais de um* servidor na mesma sessão de
cliente — o sentido em que o dono usa "MULTILINK" (o nome vem do pacote
proprietário analisado em `docs/MULTILINK.md`, e do HFSQL(R) de onde ele
saiu: uma ligação que fala com VÁRIOS bancos, não com um só).

O que cada script mede: `nativo.py` sobe um `phxsqld` próprio e mostra as
**três formas de login** do `docs/SEGURANCA.md` (desafio-resposta com PBKDF2
+ HMAC calculados em Python puro, Base64 e texto puro), um CRUD completo pela
porta de dados crua, e o **mesmo servidor** respondendo ao console oficial
`phxsqlcmd` — prova que console e "conexão native à mão" são o mesmo
protocolo por dois caminhos. `multilink.py` sobe **três** `phxsqld` (um hub e
dois servidores de origem), cadastra duas ligações de DbLink no hub e prova,
contra o oráculo direto de cada origem, que uma única sessão de cliente com o
hub lê e combina dado de ambos — e mostra o limite real: não há `FROM` de SQL
que atravesse um DbLink, então quem combina os dois lados é o cliente (ou a
tela), nunca uma junção no servidor. O DbLink PhxSql-para-PhxSql "cru" (só um
lado) já tinha bancada própria e continua em `bancada/dblink/prova-phxsql.py`
— não duplicado aqui. O ODBC já tinha bancada própria em `bancada/odbc/`.

Como roda: `python3 bancada/conexoes/nativo.py` e
`python3 bancada/conexoes/multilink.py`, cada um sobe os próprios `phxsqld`
(faixa de portas 6400–6419 desta frente), mata todos pelo PID guardado —
nunca por `pkill` — e apaga o próprio diretório de trabalho em `/tmp` ao
sair. Precisam só de `target/release/phxsqld` e `target/release/phxsqlcmd`
já compilados; não compilam nada sozinhos.
