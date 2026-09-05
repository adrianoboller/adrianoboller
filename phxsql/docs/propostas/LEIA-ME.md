# Propostas recebidas, e a medição de cada uma antes de virarem plano

Esta pasta guarda receita de fora **como ela chegou**, sem edição, ao lado do
que a medição desta casa disse sobre ela. Existe por causa da lei do
`CLAUDE.md`: *receita de fora se mede contra o nosso gargalo antes de virar
plano* — e porque a medição custa caro e não pode morrer com a sessão.

Regra da pasta: o documento recebido **não se edita**. O que esta casa achou
vai aqui embaixo, com data, e nomeia arquivo e linha.

---

## `conexao-phxsql-1.0.md` — Manual da Conexão Segura PHXSQL

**Recebido em 05/09/2026**, 1.200 linhas. Especifica TLS 1.3, autenticação do
servidor por fingerprint, desafio assinado em Ed25519, chave privada protegida
por senha local, senha de conta opcional, revogação, rotação e trilha de
auditoria. O exemplo de cliente está em **WLanguage** (WINDEV).

**Estado: em análise pelo dono, 05/09/2026.** Nada foi implementado, nada foi
recusado. O que segue é só a medição.

### As sete garantias centrais da §3.1 já estão de pé

Conferido contra o código, não contra a lembrança:

| A proposta pede | O que existe hoje | Onde |
|---|---|---|
| canal cifrado e autenticado | aperto **Noise `NX_25519_ChaChaPoly_SHA256`** | `docs/CIFRA-DO-FIO.md`, `crates/phxsql-core/src/fio.rs` |
| fingerprint confiável, pinning | **pino da estática do servidor**, estilo `known_hosts` | `docs/CIFRA-DO-FIO.md` §1 |
| chave pública do cliente cadastrada | `Usuario.chave_publica: Option<[u8; 32]>` | `crates/phxsql-server/src/usuarios.rs:537` |
| desafio aleatório assinado | Ed25519 (RFC 8032), e ele já corre **dentro** do túnel | `crates/phxsql-core/src/desafio.rs`, `ed25519.rs` |
| chave privada nunca trafega | é assim desde o começo | `docs/SEGURANCA.md` §2 |
| senha só dentro do canal | idem, e há teste que falha se a ficha vazar o hash | pétrea do `CLAUDE.md` |
| comparação em tempo constante | `iguais_em_tempo_constante` | `crates/phxsql-core/src/hash.rs` |

As primitivas foram todas escritas aqui e provadas contra vetor oficial:
FIPS 180-4, RFC 4231, RFC 6070, RFC 8032, RFC 7748, RFC 5869, RFC 8439.

### O que a proposta traz e NÃO existe — e é a parte que mais vale

**Estado de chave.** Medido: a ficha do usuário tem `chave_publica` e `ativo`,
e **não há estado de chave nenhum** — nem revogada, nem suspensa, nem validade,
nem encerramento das sessões abertas por uma chave que acabou de ser revogada.
As §10, §11 e §12 do documento descrevem algo que aqui não existe.

E esta parte **não depende de decidir o transporte**: ela funciona igual sobre
o Noise que já está de pé.

### As quatro divergências, e a primeira não é criptográfica

**1. O gargalo é o CLIENTE, e o documento não diz isso.** O exemplo da §6 está
em WLanguage. O WINDEV tem TLS 1.3 pronto e **não tem Noise** — X25519, HKDF e
ChaCha20-Poly1305 teriam de ser escritos em WLanguage e provados contra vetor
lá. Do lado do servidor é o inverso: o Noise existe e o TLS não. É o argumento
mais forte a favor de TLS que este documento carrega, e ele está implícito.
**Premissa a medir antes de qualquer decisão:** o que o WLanguage realmente
oferece — TLS 1.3 com pinning de fingerprint? Ed25519? Aritmética para X25519?

**2. TLS 1.3 esbarra na pétrea de zero dependências.** Não é como escrever
SHA-256: é máquina de estados, X.509, ASN.1, extensões e cadeia de
certificados. A auditoria do pedido 157 já pôs a exceção na mesa — *abrir
exceção só na camada de rede* — e ela continua **sem decisão do dono**.

**3. «Não desenvolver algoritmos criptográficos próprios» (§3.2).** A regra
está certa e esta casa a cumpre — mas a frase, ao pé da letra, proibiria o
método que produziu o que já está provado. Aqui não se *inventa* algoritmo: lê-
se a norma, reescreve-se e prova-se contra o vetor oficial. Criptografia
caseira é o contrário disso.

**4. Argon2id contra PBKDF2-HMAC-SHA256.** Hoje são 210.000 voltas, provadas
contra vetor. O Argon2id é melhor contra hardware dedicado, e é escrevível aqui
como os outros seis foram. É melhoria real, não bloqueio.

### O limite que já estava escrito, e que a proposta resolveria

A §5 do `docs/CIFRA-DO-FIO.md` diz onde o Noise **não** vale: a interface web.
*«O navegador fala TLS ou fala claro; um aperto Noise em JavaScript seria
teatro, porque o script chega pelo mesmo canal em claro que se quer proteger —
cifra cujo código o atacante entrega não é cifra.»* Hoje a saída honesta ali é
proxy TLS na frente. TLS de verdade fecharia esse buraco junto.

### O que NÃO foi feito

Nenhuma linha de código, nenhuma recusa. A decisão do transporte é do dono, e
ele a tomou como *analisar depois* em 05/09/2026.
