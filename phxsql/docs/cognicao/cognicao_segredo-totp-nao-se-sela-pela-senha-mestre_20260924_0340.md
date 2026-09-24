# O segredo TOTP não se sela pela senha mestre: o login vem antes de destrancar

## 1. O que aconteceu

Frente F3 do phxvpn (segundo fator no painel e na conexão OpenVPN). O segredo
TOTP é da família da senha — quem o tem gera o código —, então tinha de ir ao
PostgreSQL selado. O painel já tem um cofre (`phxvpn/src/cofre.rs`): chave
derivada da senha mestre, XChaCha20-Poly1305, e é ele que sela a chave da AC e
a `tls-crypt` de cada rede.

## 2. O que eu concluí primeiro, e estava errado

«Sela no cofre, como tudo o que é segredo aqui — um motor só.» Parecia a
aplicação direta de «função não se duplica».

Não funciona, pela ordem das coisas: o painel liga **trancado**, e o admin
**faz login** para digitar a senha mestre (`/api/destrancar` exige sessão de
admin). Com o segredo do autenticador selado pela mestre, o admin que ligou o
MFA nunca mais entraria num painel recém-reiniciado — o código não se confere
sem abrir o selo, e o selo não abre sem o login que precisa do código. O mesmo
vale para a conexão VPN: o verificador roda como `nobody` e não tem a mestre.

## 3. O que a medição disse

Três hipóteses para a chave do selo, pesadas antes de escrever:

| Hipótese | Custo | Morre por |
|---|---|---|
| a) senha mestre (cofre) | 0 PBKDF2 a mais | trava o admin fora (acima) |
| b) derivada da senha do usuário | **+1 PBKDF2 por login** (~430 ms no custo de produção, a medida do `PHXVPN.md` de 23/09) — mais barata seria um oráculo da senha mais barato que o próprio hash | custo dobrado em todo login e em toda conexão |
| c) arquivo `mfa.chave` 0600 na pasta de dados | 0 | — (fica) |

Venceu a (c): o mesmo motor de selo (`Cofre::de_chave`), outra chave. Quem leva
só o dump do banco não gera código; quem leva o disco do painel já levava a
chave do servidor OpenVPN em claro (0600), então o modelo de ameaça não piorou.

## 4. A regra

**Antes de selar um segredo com uma chave, pergunte QUANDO ele precisa ser
aberto — e se a chave já existe nesse momento.** «Um motor só» vale para o
selo; a chave é outra pergunta.

## 5. Como está guardado hoje

`phxvpn/src/mfa.rs` (comentário do topo) e o teste
`autenticador_cadastro_reuso_e_rede_que_exige` em `tests/postgres_real.rs`,
que confere que o selo no banco não contém o segredo nem em base32 nem em hex.
**Buraco que ficou:** perder o `mfa.chave` desliga todo autenticador (ninguém
confere código); o admin sem MFA zera os dos outros, mas o admin com MFA fica
de fora. Faz parte do backup da pasta de dados — não há ferramenta de
recuperação ainda.
