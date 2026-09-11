# Cognição: no correio E2E, "a senha do outro" é ECDH, não duas senhas juntas

**Descoberta:** 11/09/2026 ~14:45 UTC.

## 1. O que aconteceu

O dono pediu o correio nativo do PhxSql com criptografia fim-a-fim: *"os dados
do e-mail são criptografados entre os 2 usuários; a senha dele com a senha do
outro descriptografa"*, e *"para um mandar para o outro tem que aceitar uma
solicitação de relação de confiança"* — mesma empresa ou empresas diferentes.

Construí um protótipo rodável — `crates/phxsql-core/examples/correio-e2e.rs` —
que prova o modelo só com a `std` e a cripto já conferida contra vetor da casa
(X25519, HKDF, ChaCha20-Poly1305, PBKDF2). `cargo run --example correio-e2e -p
phxsql-core` sai **9 checagens, 9 ok, PROVA VERDE**: enviar sem confiança é
recusado; com confiança sucede; o gravado é ciphertext (77 bytes, o texto claro
não aparece nos bytes); a destinatária decifra com a **própria** senha; senha
errada não abre a privada; um terceiro fora do par não decifra; e o gate de
confiança vale igual entre empresas diferentes.

## 2. O que eu concluí primeiro, e estava errado

Li *"a senha dele com a senha do outro descriptografa"* ao pé da letra: uma
única chave derivada da **combinação das duas senhas** (algo como
`KDF(segredo_A ⊕ segredo_B)`), exigindo as duas presentes para decifrar.

Isso **não funciona para correio**, que é assíncrono: quando A manda, B não
está online, e A nunca pode ter a senha de B (nem B a de A — senha nunca em
texto puro, nem em memória alheia). Se decifrar exigisse as duas senhas juntas,
**ninguém conseguiria ler** a mensagem depois. O diagnóstico plausível caía na
primeira colisão com a restrição real do meio.

## 3. O que a medição disse

O modelo que roda é **ECDH X25519**, e ele realiza exatamente a frase do dono
sem nunca pedir a senha do outro:

- Cada usuário tem uma identidade X25519. A **privada mora cifrada sob a senha**
  (`cifra::chave_de_senha` = PBKDF2 → `selar`). A senha destranca a **própria**
  privada, e só.
- O par se fecha por `x25519::segredo(priv_A, pub_B) == x25519::segredo(priv_B,
  pub_A)` — a simetria do Diffie-Hellman. A chave da mensagem sai desse segredo
  por HKDF, com sal por mensagem.
- Então **cada um decifra com a sua senha (que abre a sua privada) + a chave
  PÚBLICA do outro**. "A senha dele com a [chave do] outro" — a pública, não a
  senha. Provado nos dois sentidos: senha errada não abre a privada (etiqueta do
  AEAD); terceiro fora do par deriva outro segredo e o `abrir` recusa.

## 4. A regra

**No correio fim-a-fim, a senha destranca só a PRÓPRIA chave privada; o par se
fecha por ECDH com a chave PÚBLICA do outro — nunca se pede a senha do outro
para decifrar, porque correio é assíncrono. E nada sai sem relação de confiança
aceita, mesma empresa ou não.**

## 5. Como está guardado hoje, e onde o buraco ficou

Guardado como **prova do modelo em memória**: o exemplo rodável, com o comando
na cabeça do arquivo. Não é o formato em disco.

O buraco, nomeado para não virar promessa:

- **Formato em disco** — caixas, mensagens, relações de confiança e as chaves
  (pública em claro, privada selada) ainda não são tabelas PSCH. Decide-se com
  o dono antes de gravar (papel C, "mudança de formato entra cedo").
- **Protocolo no 8000** — o transporte nativo TCP/IP com usuário e senha ainda
  não existe; o protótipo não abre soquete.
- **Sigilo futuro (forward secrecy)** — esta versão usa identidade X25519
  **estática**: vazou a privada de um lado, vaza todo o histórico daquele par.
  Um ratchet (estilo Signal) compraria isso, e é decisão para depois — medida,
  não embutida caladamente.
- **Revogação de confiança** — solicitar e aceitar existem; retirar a confiança
  (e o que acontece com o histórico já trocado) ainda não.
