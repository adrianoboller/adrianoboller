# Cognição — o teto que faltava era o teto que não vinha do motor

**Descoberto em** 23/09/2026, 22:44 · pedido 434 · papel B

## 1. O que aconteceu

Cinco `read_line` liam o soquete sem passar pelo `Canal` de
`crates/phxsql-core/src/fio.rs`, e por isso sem teto:

- `crates/phxsql-server/src/http.rs`, a linha de pedido HTTP — **sem teto
  nenhum**;
- o mesmo arquivo, cada linha de cabeçalho — o `MAX_CABECALHO` conferido
  **depois** de a linha já estar na memória: ele limitava o acumulado, nunca a
  linha;
- `crates/phxsql-server/src/servidor.rs`, a linha da porta 5000 — vinha do
  motor, mas com `TETO_DO_REGISTRO` (128 MiB) **antes do login**;
- `crates/phxsql-odbc/src/conexao.rs::cifrar` — o **terceiro** irmão do aperto
  de mão, que ficou para trás quando o pedido 312 consertou os outros dois.

## 2. O que eu concluí primeiro, e estava errado

**Errado nº 1 — o nível do diagnóstico.** Li o pedido como «falta um teto» e
comecei a escolher um valor para uma constante nova na porta web. O teto
ausente é o **sintoma**; a doença é a leitura não vir do motor. Uma constante
`MAX_LINHA` no `http.rs` teria fechado o buraco de hoje e criado o próximo —
duas decisões do mesmo assunto em dois arquivos, para divergirem depois.
Corrigido: o teto certo era o `MAX_CABECALHO` que **já estava no arquivo**, e a
leitura passa a ser `Canal::Claro::ler_ate`.

**Errado nº 2 — a condição do teto pequeno.** Ia amarrar o teto do anônimo a
`sessao.usuario.is_none()`, só isso. Medido no próprio `despachar`: o portão do
login só morde `if !self.cadastro().vazio()`. Servidor sem usuário nenhum não
exige credencial de ninguém — amarrar só ao `usuario.is_none()` teria apertado
para 64 KiB **todo `inserir` em lote de todo servidor sem cadastro**, que é a
bateria inteira desta casa e todo cliente que nunca criou usuário. O estrago do
pedido 203 com outra roupa.

**Errado nº 3 — como provar a porta web.** A primeira ideia foi contar quantos
bytes o cliente consegue escrever antes de o cano quebrar. Medido em
`/proc/sys/net/ipv4/tcp_rmem`: o buffer de recepção autotune até **32 MiB**
nesta máquina, mais 4 MiB de envio. Contar bytes mediria o kernel, não o
servidor — e a fronteira ficaria a menos de uma ordem de grandeza do ruído.

## 3. O que a medição disse

| medida | número |
|---|---|
| `read_line` do ODBC que eram **produção** | **1 de 4** — os outros 3 estão em `#[cfg(test)]` (módulo na linha 596): são os servidores de mentira, o **outro lado** do fio |
| operações legítimas numa sessão anônima com cadastro | **6** (`ping`, `login`, `desafio`, `quem_sou`, `sair`, `catalogo`); a maior não chega a **1 KiB**, contra 64 KiB de teto |
| pedidos HTTP que o teto de 16 KiB passa a recusar e antes eram aceitos | **zero** — toda linha de um pedido aceito já cabia no acumulado de 16 KiB |
| `http.rs` com o defeito reposto | **262.144 bytes escritos e 3,15 s** sem o servidor largar a conexão |
| a recusa do `Canal` no teto do aperto | dizia **«mais de 0 MiB»** (`65536 / 1 MiB` = 0) e mandava «parta a tabela» dentro de um aperto de mão |
| buffer de recepção TCP desta máquina | autotune até **32 MiB** — por isso a prova é por tempo, não por bytes |

## 4. A regra

**Quando um teto faltar, procure o motor antes de escolher o valor: teto que
nasce fora do motor é a segunda cópia da mesma decisão, e a cópia é a doença.**
E o corolário da medição: **antes de aplicar uma guarda por simetria, conte
quantos dos sítios são mesmo o caminho que ela protege** — aqui eram 1 de 4.

## 5. Como está guardado hoje

- `Servidor::teto_da_linha` e `Servidor::ainda_anonima` — **uma** função para a
  condição «ainda anônima», usada pelo portão do login e pelo teto. Duas cópias
  divergiriam, e a que ficasse para trás seria a porta dos fundos.
- `TETO_DO_APERTO` teve a documentação **alargada** para dizer o que ela sempre
  justificou: qualquer linha lida antes de existir credencial. O nome continua
  mais estreito que o significado — **este é o buraco**, e ele está aqui
  escrito: renomear para `TETO_ANTES_DA_IDENTIDADE` obriga a mexer no `trecho`
  da guarda `aperto-de-mao-sem-teto` (`bancada/guardas/catalogo.py`), e isso é
  do papel G.
- Provas nos dois sentidos: `crates/phxsql-server/tests/teto-da-linha-anonima.rs`
  (4), `crates/phxsql-server/tests/teto-da-linha-http.rs` (4),
  `conexao::testes::a_resposta_do_aperto_acima_do_teto_e_recusada_pelo_limite`,
  `fio::testes::a_recusa_nunca_anuncia_zero_e_nao_da_ordem_que_nao_cabe`.
  **Nenhuma está no catálogo de guardas ainda** — papel G.
- Sítio conhecido e **não** consertado: `email.rs:164`, o `read_line` cru do
  cliente SMTP dos alertas. Registrado em `docs/SEGURANCA.md` §19.7.
