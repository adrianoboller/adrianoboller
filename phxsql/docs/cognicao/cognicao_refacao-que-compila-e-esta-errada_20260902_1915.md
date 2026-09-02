# A refação óbvia compila de primeira e está errada

- **Quando:** 2026-09-02, 19:15 (frente da concorrência)
- **Onde:** `crates/phxsql-server/src/servidor.rs`, o `Mutex<Instancia>`
- **Custo:** zero, porque foi pega antes — e teria sido altíssimo depois

## O que aconteceu

O caminho «óbvio» para a SP000011 era trocar `Mutex<Instancia>` por
`RwLock<Instancia>`: leitores param de esperar leitores, e o compilador cuida
do resto. A medição olhou o tipo antes de aceitar:

```rust
pub struct Instancia { base: PathBuf }   // um campo
```

Todos os métodos são `&self`. **O `Mutex` não protege a `Instancia`** — ela é
imutável. Ele é uma ficha de exclusão global, e o estado real está **no disco**,
alcançado por um `Table` que se abre e se fecha a cada operação.

Consequência: com `RwLock`, dois escritores tomam **guarda de leitura** (porque
`&self` basta), abrem dois `Table` sobre os mesmos arquivos, e **o compilador
não tem o que reclamar**. A troca compila, os testes de unidade passam, e a
corrupção aparece em produção sob carga.

## O que eu concluí primeiro, e estava errado

Que a garantia estava no tipo. Ela está na **convenção**: «quem tem a ficha
mexe no disco». Convenção que o compilador não conhece é convenção que a
refação apaga em silêncio.

## O que a medição disse

**76 seções críticas** tomam a trava; **24** alcançam `fsync` com ela na mão;
**5** rodam código do dono do banco (gatilho `BEFORE`) sem teto de duração;
**0** atravessam a rede.

## A regra

**Antes de trocar a primitiva de sincronização, pergunte o que ela protege de
verdade.** Se o dado protegido não está dentro do tipo travado, o tipo não
carrega a garantia — e o compilador vai aprovar a mudança que a quebra.

## Como está guardado hoje

Escrito em `docs/CONCORRENCIA.md` §2 e no roteiro, como pré-requisito: **o
invariante se escreve antes de qualquer refação da trava**. Não há guarda
executável — não há como um teste reprovar «o `RwLock` está errado» sem que
alguém primeiro escreva o invariante que ele viola. Esse é o próximo passo, e
está registrado como tal.
