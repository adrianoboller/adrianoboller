# Deref para `Path` não basta para `impl AsRef<Path>` — pedido 150

## 1. O que aconteceu

Convertendo os helpers de diretório temporário dos testes de `phxsql-store`
(`fn dir(nome: &str) -> PathBuf`) para devolver um guarda com `Drop`
(`DirTemp`), o plano era trocar **só a definição do helper** e deixar cada
chamada no corpo dos ~370 testes intocada — a maioria escreve `&d`,
`d.join(...)`, `d.display()`, e passa `d` para funções como
`Table::criar(d: impl AsRef<Path>)` e `Instancia::nova(d: impl AsRef<Path>)`.

Implementei `impl Deref<Target = Path> for DirTemp` nos dois guardas
(`tests/comum/mod.rs` e `src/apoio_teste.rs`) e converti dez helpers de
`phxsql-store/src` só com isso — sem rodar `cargo check` ainda, porque o plano
era converter o lote inteiro e checar de uma vez.

## 2. O que eu concluí primeiro, e estava errado

Concluí, e escrevi essa conclusão antes de tocar em qualquer arquivo, que
`Deref<Target = Path>` bastava para qualquer lugar que hoje aceita `&d` como
caminho — "zero mudança de call-site", inclusive em `Table::criar(&d, ...)` e
`Instancia::nova(&base)`.

Estava errado, e percebi ANTES de compilar, não depois: ao chegar no
`catalogo.rs` e ler de novo a assinatura de `Instancia::nova` —
`pub fn nova(base: impl AsRef<Path>) -> Result<Instancia>` —, vi que ela pede
um **bound genérico**, não um `&Path` concreto. Coerção de `Deref` só se
aplica em posições concretas da assinatura (quando a função pede `&Path`
explícito) ou em sites de coerção conhecidos do compilador; ela **não** entra
para satisfazer `T: AsRef<Path>`, porque o compilador escolheria `T =
&DirTemp` e então checaria se `&DirTemp: AsRef<Path>` — e sem um `impl
AsRef<Path> for DirTemp` explícito essa checagem falha, mesmo com o `Deref` no
lugar.

Corrigi acrescentando `impl AsRef<Path> for DirTemp` nos dois guardas antes de
converter o resto — os dez helpers já convertidos (`lixeira.rs`, `trilha.rs`,
`backup.rs`, `blob.rs`, `memoria.rs`, `volume.rs`, `log.rs`, `restaurar.rs`,
`motivo.rs`, `reg.rs`) também chamam `fn criar(impl AsRef<Path>)` em algum
ponto, então teriam falhado do mesmo jeito se eu tivesse compilado antes de
adicionar o `AsRef`. Sorte de leitura, não sorte de teste: o defeito nunca
chegou a existir num commit nem foi provado por um erro real do compilador —
é a lição que a seção 3 mede.

## 3. O que a medição disse

`cargo check -p phxsql-store --tests --lib`, já com os dois `impl` (`Deref` e
`AsRef<Path>`) no lugar, passou de primeira em **22 arquivos** (11 de
`src/cfg(test)`, 11 de `tests/`) que somam **203 testes** convertidos, **sem
tocar uma linha** de call-site — só a definição de cada helper mudou. Isso
prova que os dois impls juntos bastam; não ficou provado, e não afirmo aqui,
que o `Deref` sozinho teria de fato quebrado a compilação, porque nunca deixei
essa versão rodar.

## 4. A regra

Um guarda que substitui `PathBuf` num teste precisa de **dois** impls, não um:
`Deref<Target = Path>` para os poucos lugares que pedem `&Path` concreto, e
`AsRef<Path>` para todo `fn criar(d: impl AsRef<Path>)` — que é a assinatura
mais comum neste código (`Table::criar`, `Instancia::nova`). Antes de assumir
que trocar o tipo de retorno de um helper é mudança "zero call-site", leia a
assinatura de toda função que recebe o valor — um bound genérico (`impl
AsRef<Path>`) não se comporta como um parâmetro concreto (`&Path`) para fins
de coerção de `Deref`.

## 5. Como está guardado hoje

Os dois `impl` (`Deref` e `AsRef<Path>`) estão lado a lado nos dois guardas
deste pedido: `crates/phxsql-store/tests/comum/mod.rs` (testes de integração)
e `crates/phxsql-store/src/apoio_teste.rs` (testes unitários), cada um com o
comentário explicando por que o `Deref` sozinho não bastava. Quem for
converter os helpers de `phxsql-server` (pedido 150, fatia que ainda falta)
herda os dois impls prontos se reusar o mesmo padrão de guarda.
