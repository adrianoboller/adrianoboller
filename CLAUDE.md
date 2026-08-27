# PhxSql — convenções do projeto

Motor de dados em Rust no modelo de arquivos separados do HFSQL. O código vive
em `phxsql/`. Especificação do formato em `phxsql/docs/FORMATO.md`, roteiro em
`phxsql/docs/PLANO.md`.

## Ao terminar cada rodada de trabalho: atualize o dossiê

O dossiê é a página que o Adriano usa para enxergar o projeto inteiro:

- **URL:** https://claude.ai/code/artifact/5c14044e-0dc5-4832-b015-224ab1e40033
- **Fonte:** `phxsql/docs/dossie/dossie-phxsql.html` (versionado, para que
  qualquer sessão consiga atualizá-lo)

Publique sempre **passando essa URL**, para cair na mesma página em vez de
criar outra. Instruções e as armadilhas de estilo em
`phxsql/docs/dossie/LEIA-ME.md`.

Os números do painel são **medidos, nunca estimados** — já saíram errados uma
vez por arredondamento para cima.

## Regras que não se quebram

**Zero dependências externas.** Só a `std`. Foi o que fez a compilação cruzada
para Windows funcionar de primeira e o que permite `cargo build --offline`.
JSON, CRC-32, SHA-256, HMAC e PBKDF2 são escritos aqui. Se algo parecer exigir
uma crate, primeiro pergunte — não acrescente.

**Criptografia se confere contra vetor oficial.** Nada de "parece certo": os
testes trazem FIPS 180-4, RFC 4231 e os vetores de PBKDF2.

**Senha nunca em texto puro.** Nem em arquivo, nem em log, nem em resposta do
protocolo. Há teste que falha se a ficha de usuário vazar o hash.

**A ordem de digitação é sagrada.** O `.reg` nunca reaproveita slot excluído.
Qualquer proposta que quebre isso precisa ser discutida antes.

**Mudança de formato entra cedo.** Enquanto não há dado em produção, mudar o
formato é barato; depois vira migração. Foi assim com o volume no ponteiro.

## Antes de commitar

```bash
cargo fmt --all
cargo clippy --workspace --all-targets     # tem de dar zero avisos
cargo test --workspace
```

Mexeu no formato em disco? Atualize `docs/FORMATO.md` no mesmo commit.

## Estilo

- Código, comentários, documentação e mensagens de commit em **português**.
- Identificadores e comentários **sem acento** (o texto de interface pode ter).
- Comentário explica **por que**, não o que — o código já diz o que.
- Mensagem de commit conta a decisão e o motivo, não a lista de arquivos.

## Branch

Trabalhe em `claude/capacidades-disponiveis-y6auxh`, em
`adrianoboller/adrianoboller`. Não abra PR sem pedido explícito.
