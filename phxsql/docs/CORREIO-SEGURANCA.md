# CORREIO — segurança: onde o phxmail está, medido e sem enfeite

**Papéis:** F (prova real / segurança), J (comparação com a concorrência),
H (documentação). **Data:** 12/09/2026.

Este documento responde a três perguntas do dono, e responde contra o **código
que existe**, não contra a folha de marca. Onde uma decisão já mora em outro
documento (sigilo futuro no `CORREIO-FORMATO.md`, a cifra do fio no
`SEGURANCA.md §7`), aqui só se **cita** — para não virar uma terceira cópia da
lei que possa divergir dela.

## 0. Antes de comparar: o estado de maturidade

Comparar "nível de segurança" com Gmail, ProtonMail ou Signal é comparar um
**modelo com prova de conceito** contra **produtos implantados e auditados**.
A honestidade começa aqui:

| Peça | Estado | Prova |
|------|--------|-------|
| Primitivas (ChaCha20-Poly1305, X25519, Ed25519, HKDF, PBKDF2, SHA-256/512) | **prontas e conferidas contra vetor** | RFC 8439, 8032, 7748, 8018; FIPS 180-4; RFC 4231 — testes em `phxsql-core` |
| Certificado X.509 Ed25519/X25519 | **pronto, provado contra ferramenta** | `cert-openssl.rs`: OpenSSL 3.0.13 aceita, 14 checagens verdes |
| Modelo E2E ponta a ponta | **protótipo em memória** | `examples/correio-e2e.rs` (prova verde), **não** é servidor implantado |
| Arquivo de chaves `.p12` em disco (fatia 2) | **não existe ainda** | `CORREIO-P12.md §6` |
| Auditoria criptográfica externa | **nunca houve** | — |

Ou seja: a matemática está escrita e conferida; o **produto** de correio ainda
é desenho + protótipo. O que segue avalia o **modelo**, dizendo onde ele já é
código e onde é planta.

## 1. Comparação com a concorrência, por dimensão

Nada de "mais seguro" seco. Segurança se compara por dimensão, e em cada uma o
phxmail cai num grupo conhecido.

| Dimensão | Gmail/Outlook | ProtonMail/Tutanota | Signal | **phxmail (modelo)** |
|----------|---------------|---------------------|--------|----------------------|
| **Conteúdo (corpo/assunto/anexo)** | provedor **lê** | **E2E**, provedor não lê | **E2E** | **E2E** — X25519-ECDH + HKDF + ChaCha20-Poly1305 |
| **Sigilo futuro (forward secrecy)** | — | **não** | **sim** (Double Ratchet) | **não** — X25519 é estática (`CORREIO-FORMATO §`) |
| **Custódia da chave privada** | provedor tem | cifrada sob senha, acesso-zero | no aparelho | **cifrada sob a senha** (PBKDF2), cada um abre a sua |
| **Metadado (de/para/quando/tamanho)** | provedor vê | provedor vê | **selado** (sealed-sender) | servermail **vê em trânsito**, mas **não loga** e o central nunca vê (P2P) |
| **KDF da senha** | — | bcrypt/Argon2 (auditado) | — | **PBKDF2-SHA256, 200 000** — correto, mas **não** é memória-dura |
| **Anti-spam estrutural** | filtro | filtro | número/contato | **relação de confiança**: sem aceite não há contato |
| **Camadas extras opcionais** | — | senha de e-mail | — | **alto segredo** (frase fora de banda) + **Masson** (3ª chave) |
| **Biblioteca de cripto** | auditada | auditada (libsodium etc.) | auditada | **escrita à mão**, conferida contra vetor, **sem auditoria externa** |

**Leitura honesta do quadro:**

- **À frente do Gmail/Outlook** na dimensão que mais importa: o conteúdo é E2E,
  o provedor não lê. Gmail não é E2E.
- **Mesma classe do ProtonMail/Tutanota** no *desenho* de confidencialidade —
  E2E, acesso-zero, chave sob senha, **sem** sigilo futuro (eles também não
  têm). A diferença é **maturidade** (eles são auditados, implantados, têm 2FA,
  recuperação, anos de fogo) e o **KDF** (eles usam KDF memória-dura; o phxmail
  usa PBKDF2, que ASIC/GPU atacam melhor).
- **Atrás do Signal** no protocolo: o Signal tem sigilo futuro e segurança
  pós-comprometimento pelo ratchet. O phxmail não — mas correio assíncrono
  torna o ratchet do Signal difícil, e o ProtonMail também não o tem.

**Onde o phxmail tem algo que os outros não dão de fábrica:** as camadas
**alto segredo** (frase combinada por telefone/SMS) e **Masson** (uma 3ª chave),
encaixadas por dentro do E2E, e a **confiança obrigatória** antes de qualquer
contato (anti-spam/anti-phishing estrutural, provado em `correio-e2e.rs`).

## 2. Existe chance de descriptografia dos e-mails?

**Sim — mas não quebrando a matemática.** E2E não quer dizer "impossível de
abrir"; quer dizer que quem abre é quem tem a chave. Os caminhos **reais**, do
mais provável ao menos:

1. **Senha fraca.** Toda a confidencialidade repousa na senha da conta: a chave
   privada mora cifrada sob PBKDF2(senha, 200 000). Se um atacante pega a chave
   privada cifrada (nó servermail comprometido, ou o `.p12`) **e** a senha é
   fraca, ele quebra fora de linha e decifra. PBKDF2-SHA256 **não é memória-dura**
   — GPU/ASIC atacam bem. → *Mitigação:* frase-senha forte; melhor ainda, migrar
   para KDF memória-dura (Argon2/scrypt), que **hoje não é o que está escrito**.
2. **Sem sigilo futuro.** Como o X25519 é estático, **um** vazamento de chave
   privada (senha quebrada, chave roubada, aparelho apreendido) abre **todo o
   histórico** com aquele correspondente, não só uma mensagem. O Signal impede
   isso; o phxmail (como PGP e ProtonMail) não.
3. **Aparelho invadido.** E2E protege no fio e no servidor, **não** um cliente
   comprometido. Quem já lê a tela do destinatário lê o e-mail. Nenhum sistema
   E2E resolve isso — e a flag "ler somente" (§3) também não.
4. **A cripto é escrita à mão e não auditada.** Os vetores provam que as
   primitivas **calculam certo**; não provam ausência de canal lateral nem de
   falha de composição. Pontos concretos já visíveis no código: o **Ed25519
   assina em tempo não-constante** (o próprio `x25519.rs` anota isso), e no
   **Windows** o gerador cai numa mistura mais fraca que o `/dev/urandom`. Os
   concorrentes usam bibliotecas que passaram por anos de auditoria e *fuzzing*;
   o phxmail não passou por nenhuma.
5. **Metadado.** Mesmo sem abrir o conteúdo, o servermail vê **quem falou com
   quem, quando, tamanho, prioridade**. Isso é descriptografar o *grafo social*,
   não o texto — e merece ser dito.

**O que NÃO é caminho, dado o código de hoje:**

- O **diretório central** (`phxmail.com.br`) nunca vê o e-mail: no desenho, a
  entrega é P2P e o central só guarda o catálogo de servermail.
- Força bruta **na cripto em si**: ChaCha20-Poly1305 e X25519 na força total são
  inviáveis de quebrar, e os erros que quebrariam sem quebrar a matemática estão
  **projetados para fora** — nonce nunca reusa (sal HKDF por mensagem + CSPRNG
  do SO), etiqueta conferida em **tempo constante** antes de decifrar, X25519 em
  **escada de Montgomery de tempo constante**, ChaCha20 sem tabela.

**Frase de fechamento, sem enfeite:** *o e-mail não se abre quebrando o ChaCha;
abre-se com senha fraca, com uma chave privada vazada (e aí cai tudo, porque não
há sigilo futuro), com o aparelho do destinatário invadido, ou por um defeito na
cripto escrita à mão que auditoria nenhuma ainda procurou.*

## 3. A flag "ler somente" com senha

**Baseline honesto primeiro:** **toda** mensagem já exige a senha do
destinatário para abrir — a chave privada está cifrada sob ela (`ler()` chama
`abrir_privada(senha)` em `correio-e2e.rs`). "Exigir senha para ler" é o padrão,
não uma novidade. O que a flag acrescenta são duas coisas **diferentes**, e é
pétreo separá-las (não se mente sobre o dado, nem sobre a garantia):

**(a) Uma senha a mais, por mensagem — isto é criptográfico e real.** Uma
camada extra, no molde do "alto segredo" que já existe: uma senha que o
remetente combina fora de banda e que o leitor **digita a cada abertura**. Sem
ela, nem a senha da conta abre. Isso protege inclusive contra uma conta cuja
senha vazou. É a parte que **se garante com matemática**.

**(b) "Ler somente" = sem encaminhar/copiar/salvar/imprimir — isto é barreira de
tela, NÃO é garantia.** Dá para esconder os botões de encaminhar/copiar/baixar/
imprimir e nunca gravar o texto claro em disco (coerente com a pétrea da
privacidade: escrever é só memória). **Mas** quem já pode decifrar e **ver** o
texto pode fotografar a tela ou redigitar — e nenhuma tela impede isso. É atrito
honesto, não cadeado. Vender (b) como se fosse (a) seria mentir sobre a garantia.

**Proposta concreta (a decidir com o dono, papéis B + C):**

- Uma flag `ler_somente` na mensagem que:
  1. **exige uma senha-extra por mensagem** (camada como a do alto segredo) —
     digitada a **cada** abertura, sem cache do estado destravado; *cripto real*;
  2. o cliente mostra **só leitura**: sem encaminhar/copiar/baixar/imprimir, e o
     claro vive só em memória; *barreira de tela, rotulada como tal*.
- **Isto mexe no formato da mensagem** (uma flag + o sal da camada extra). Pela
  pétrea "mudança de formato entra cedo", ela deve entrar **junto do congelar do
  PSCH dos 7 registros do correio** — uma das três decisões de formato ainda
  abertas —, não depois de haver e-mail gravado.

## 4. Ver também

- `CORREIO-FORMATO.md` — a decisão sobre **sigilo futuro / ratchet** (aberta).
- `CORREIO-P12.md` — o arquivo de chaves; fatia 1 pronta, fatia 2 a decidir.
- `CORREIO-PRIVACIDADE.md` — purga, sem log, sem rascunho, sem engenharia reversa.
- `SEGURANCA.md §7` — a cifra do fio, e o que ela **não** é.
- `PHXMAIL-vs-SMTP.md` — por que não é SMTP, e a comparação com Signal/Matrix.
- Provas: `cargo run --example correio-e2e -p phxsql-core` (modelo E2E),
  `cargo run --example cert-openssl -p phxsql-core` (X.509 contra o OpenSSL).
