# Parecer SEC — derivar a chave de cifra do `Uuid::v7()` da coluna

**Papel SEC (revisor adversario), 17/09/2026.** So leitura; nada editado, nada
comitado. Conferido pelo integrador (papel A) no fonte: os quatro achados que
decidem foram remedidos um a um e batem.

**Origem:** o dono, ao decidir o pedido 293, trouxe o fato *«As chaves sao uuid
v7, nao podem ser diferentes na origem destino.»* O fato esta **certo** e foi
medido. A pergunta que SEC respondeu e outra: **derivar o material de cifra desse
UUID e seguro?**

## VEREDITO: NAO ENTRA

Nao e «entra com emenda», e o motivo e que a emenda que salvaria a proposta **nao
e a proposta**: e o envelope da §11.5, que ja esta desenhado e resolve o problema
do dono com **a mesma pre-condicao** (mesma senha nos dois lados) sem nada do que
segue. **A proposta nao compra nada que o envelope nao compre**, e paga a
separacao de chave por arquivo, que e a unica coisa que hoje separa
criptograficamente dois `.reg`.

Se o dono insistir em usar o UUID, a unica forma defensavel: ele entra como o
`purpose` do NIST SP 800-132 §A.2.1 (`S = purpose || rv`), **prefixado** a uma
parte aleatoria de 128 bits, nunca no lugar dela. E isso **continua nao
replicando**, porque o `rv` continua sorteado.

**O fato do dono esta certo, e e por isso que e perigoso.** O `id` da coluna e um
identificador que chega identico na replica. O problema e que ele e identico
**pelo motivo errado para o papel de sal**: e **publico, escolhivel e
previsivel** — as tres propriedades que um sal nao pode ter todas juntas.

---

## A1 (BLOQUEADOR) — o sal passaria a ser ESCOLHIDO PELO CLIENTE

`crates/phxsql-server/src/valores.rs:392-396` aceita um `"id"` vindo do pedido,
de fora, sem conferencia nenhuma:

```rust
let id = c.texto_ou("id", "").trim().to_string();
if !id.is_empty() {
    col = col.com_id(Uuid::de_texto(&id).map_err(...)?);
}
```

E `Uuid::de_texto` (`crates/phxsql-core/src/uuid.rs:274-292`) **nao confere versao
nem variante** — conferido pelo integrador, a varredura por «versao/variante»
naquele trecho volta vazia. Medido pelo SEC:

```
de_texto(00000000-0000-0000-0000-000000000000) OK -> versao 0
de_texto(ffffffff-ffff-ffff-ffff-ffffffffffff) OK -> versao 15
id escolhido de fora, depois de ida e volta pelo bloco de esquema: igual? true
duas tabelas com o MESMO id de coluna marcada (clientes e folha): true
```

**Exploracao:** qualquer sessao com direito de `criar_tabela` manda
`{"op":"criar_tabela","colunas":[{"nome":"cpf","dado_pessoal":"Pessoal","id":"00000000-0000-0000-0000-000000000000"}]}`.
O sal do arquivo vira 16 bytes de zero, e uma unica tabela PBKDF2 pre-computada
sobre o sal nulo — feita **antes** do roubo do disco — quebra toda instalacao que
tenha usado aquele id.

A RFC 8018 §4.1 nomeia o ataque: *«The party decrypting a message … cannot be
sure that a salt supplied by another party has actually been generated at
random.»*

## A2 (BLOQUEADOR) — transplante de linha entre arquivos, medido nos dois sentidos

Hoje o que separa criptograficamente dois `.reg` e **so** o sal por arquivo.
Nenhuma das outras amarracoes carrega identidade de arquivo:

- `aad_do_slot` = `volume | rowid | versao` — `crates/phxsql-store/src/reg.rs:2286-2292`
- `rotulo_da_prova` = `MAGIC_REG | versao | slot_size` — `reg.rs:2253-2259`
- `nonce_de_pedaco(rowid, volume, versao, tempero)` — `cofre.rs:576-583`, e o
  `tempero` viaja **dentro do proprio slot** (`reg.rs:1735-1738`)

O comentario de `reg.rs:2265-2269` diz que o AAD impede «copiar o corpo cifrado da
linha 7 por cima da linha 9, ou de outro volume». Ele **nao** cobre «de outro
arquivo»; quem cobre e a chave por arquivo.

Prova real nos dois sentidos, com as primitivas da casa:

```
HOJE     -- sal sorteado por arquivo; chaves iguais? false
HOJE :   o transplante foi RECUSADO (a etiqueta nao confere)
PROPOSTA -- sal = UUID da coluna; chaves iguais? true
PROPOSTA: o transplante ABRIU -> "Fulano de Tal da Silva"
```

**E o caminho que produz dois arquivos com os mesmos ids e SUPORTADO e
DOCUMENTADO**: `valores.rs:389-391` diz que aceitar um id de fora existe para
recriar uma tabela mantendo a identidade das colunas.

## A3 (BLOQUEADOR) — o sal deixaria de precisar do disco: vaza pelo protocolo

O `id` sai em claro em duas respostas, as duas exigindo apenas **direito de
leitura**:

- `op_esquema` — `crates/phxsql-server/src/servidor.rs:16819`,
  `("id", Json::texto_de(c.id.to_string()))` (conferido pelo integrador)
- a listagem de catalogo — `servidor.rs:11092`, atras de `pode_ver_tabela`

E tambem no hex do bloco que o `posicao` devolve (`replica.rs:302-306`).

Hoje o sal so existe no cabecalho do arquivo (`cofre.rs:427`): quem quer atacar a
senha por dicionario **precisa do arquivo**. Sob a proposta, um usuario de leitura
obtem o sal sem tocar no disco e comeca a pre-computacao **meses antes** do
vazamento do backup. Isso destroi o beneficio 1 da RFC 8018 §4.1: *«An opponent is
thus limited to searching for passwords **after** a password-based operation has
been performed and the salt is known.»* **A proposta inverte o «after».**

## A4 (BLOQUEADOR) — 62 bits aleatorios onde o NIST exige 128

Layout do v7 desta casa (`uuid.rs:260-270`), contado bit a bit e **conferido pelo
integrador no fonte**:

| pedaco | bits | de onde vem |
|---|---|---|
| bytes 0–5 | 48 | relogio em ms — **previsivel** |
| byte 6 nibble baixo + byte 7 | 12 | contador, semeado com 11 bits por ms e depois incrementado (`avancar`, `uuid.rs:138-149`) |
| byte 8, 2 bits altos | 2 | variante — **fixa** (`b[8] = (b[8] & 0x3F) \| 0x80`) |
| resto de 8–15 | 62 | `sortear`, de `/dev/urandom` |

**62 bits garantidamente aleatorios; no maximo 73 se ninguem souber a ordem de
criacao.** Medido: dois v7 consecutivos tem **7 dos 16 bytes identicos**, posicoes
0 a 6.

- **NIST SP 800-132 §5.1**, verbatim: *«The length of the randomly-generated
  portion of the salt **shall** be at least 128 bits.»*
- **RFC 8018 §4.1**: *«It should be at least eight octets (64 bits) long»* — a
  parte garantida (62 bits) nao chega nem ao `should`.
- **NIST SP 800-132, glossario**: *«Salt — A non-secret binary value…»*

O nosso `SAL_LEN = 16` (`cofre.rs:52`, conferido) = **128 bits, no minimo exato do
NIST**. A proposta o cortaria pela metade.

**Agravante de portabilidade:** onde nao ha `/dev/urandom` (Windows), os 62 bits
caem no `misturar` (`uuid.rs:88-104`), cujo proprio comentario declara *«Nao e
criptografico, e nao precisa ser»*. **Sob a proposta ele passaria a precisar ser**,
e e mais fraco que a reserva que o `senha.rs:143-168` ja usa para sal. A proposta
rebaixaria o sal justamente na plataforma para onde a compilacao cruzada existe.

## A5 (GRAVE) — a premissa caduca na primeira evolucao de esquema

O diario carrega **so** `Inclusao | Alteracao | Exclusao`
(`crates/phxsql-store/src/log.rs:96-100`): **nao ha evento de DDL**. E o
`abrir_para_replicar` copia o bloco **apenas quando a tabela ainda nao existe** na
replica.

Consequencia: **o id e provadamente igual no instante da criacao da tabela na
replica, e nunca depois.** Um `acrescentar_coluna` no source nao replica; se o DBA
o repetir a mao na replica, `Column::new` sorteia id **diferente em cada lado**. O
fato do dono e verdadeiro em t0 e falso de t1 em diante, e o modo de falha e o
silencioso — que e o que o 293 existe para fechar.

## A6 (GRAVE) — o id nao e estavel para esquema `PSCH` v2: muda a cada abertura

`crates/phxsql-core/src/schema.rs:1392`, no ramo de ausencia (conferido pelo
integrador): `(Uuid::v7(), String::new(), String::new(), String::new())`. E
`VERSAO_ESQUEMA_MINIMA = 2` (`schema.rs:52`), entao **o ramo esta vivo**.

Toda tabela gravada na v2 recebe ids **novos a cada desserializacao**. Sob a
proposta o arquivo derivaria chave diferente a cada abertura e **nunca mais
abriria** — e o erro que sairia seria *«a senha de "cifra" nao e a que gravou este
arquivo»* (`cofre.rs:452-456`), mandando o operador trocar senha onde nao ha senha
errada nenhuma.

## A7 (MEDIO) — nao existe conferencia de id de coluna repetido

Nem em `Schema::new`, nem em `criar_tabela`, nem em `acrescentar_coluna`. Duas
colunas da **mesma** tabela podem carregar o mesmo id, e sob derivacao por coluna
compartilhariam chave — e o `aad` de coluna externa (`reg.rs:1698`/`1840`) e so
`coluna.to_le_bytes()`, que deixa de separar se as duas trocarem de posicao.

## A8 (MEDIO) — coluna e a unidade errada, e o custo diz por que

O material e **por arquivo**: `Material::novo()` uma vez por `.reg`
(`reg.rs:276`), `Cabecalho::novo` uma vez por volume. **O sal por arquivo e carga
estrutural, nao decoracao** — `cofre.rs:343-346` e `:612-615` dizem que e ele que
deixa o nonce ser um contador local (offset, rowid, pagina).

E o nonce do `.trash`/`.reason` ja tem como tempero **os quatro ultimos bytes do
UUID v7 do registro** (`cofre.rs:733-736`), que **replica identico**. Sob a
proposta, origem e replica teriam a mesma chave, o mesmo offset **e o mesmo
tempero** para o mesmo registro.

Custo medido (mediana de 5, `phxsql_core::cifra::chave_de_senha`):

| iteracoes | mediana |
|---|---|
| 10.000 (`ITERACOES_MINIMAS`) | 11,0 ms |
| 210.000 (`ITERACOES_PADRAO`) | 237,6 ms |

Uma tabela de 40 colunas marcadas com um PBKDF2 por coluna: **9,50 s** na primeira
abertura; um database com 50 tabelas assim paga **~7,9 min** de CPU. A `DERIVADAS`
(`cofre.rs:158-161`) **nao segura o caminho frio**.

**E a honestidade obriga a dizer que isto NAO e o argumento que mata a
proposta:** o desenho competente deriva a mestra uma vez com PBKDF2 e tira
subchave por coluna com HMAC-SHA256, que a casa tem com os vetores da RFC 4231.
Medido: **1,227 µs por subchave**, 40 colunas = 49,1 µs contra 9,50 s, **~194.000x**.
O custo some. O que mata a proposta e A1–A4.

---

## As perguntas, respondidas

**O sal precisa ser secreto ou so unico?** Nem uma coisa nem outra, exatamente:
**nao precisa ser secreto**, mas **precisa ser gerado por RBG aprovado com ≥128
bits aleatorios** (`shall`). Os dois beneficios que a RFC nomeia sao impedir
pre-computacao e tornar improvavel que a mesma chave saia duas vezes. **A proposta
destroi os dois**: o primeiro por A3, o segundo por A2.

**O que um v7 previsivel abre?** No modelo de ameaca que o proprio `cofre.rs:30-34`
declara — o arquivo copiado — a previsibilidade sozinha **nao abre nada novo**, e
o segredo continua sendo a senha. **Mas a proposta muda o modelo de ameaca**: move
o sal para **fora** do arquivo (A3) e para **dentro do alcance do atacante** (A1).

**Unicidade de verdade?** Nao, por cinco caminhos: o cliente escolhe o id (A1);
nada confere repeticao (A7); duas tabelas carregam o mesmo id por workflow
suportado (A2); `acrescentar_coluna` gera id novo em cada lado (A5); e o `PSCH` v2
gera ids novos a cada abertura (A6).

**O que isto NAO resolve?** **Nada do caso 3**, que e o pior. A proposta nao toca
`abrir_externo` (`reg.rs:1827-1830`) nem `decodificar_com_externos`
(`table.rs:4051-4082`): os 63 bytes de texto cifrado gravados como conteudo
continuam acontecendo **exatamente igual**, calados. **A decisao do dono de recusar
no motor continua sendo a unica coisa que fecha o caso 3, e a proposta nao a
substitui.**

## Contra a alternativa dos motores maduros

SEC recomenda **decifrar antes de mandar e o destino recifrar**, por tres motivos
medidos:

1. **Superficie menor.** A faixa **inline** marcada **ja viaja em claro** dentro da
   imagem (`SEGURANCA.md` §12.4: «o nome marcado aparece literalmente dentro dos
   156 bytes da imagem»). Decifrar a externa apenas a torna **consistente** com a
   inline. A proposta do UUID cria categoria nova: sal publico e escolhivel.
2. **O fio, com condicao.** Ele tem o aperto estilo Noise, mas `cifra_fio.exigir`
   **nasce desligada** (`SEGURANCA.md` §7: «cifra pedida e cifra que o atacante
   ativo apaga do pedido»). Entao: **replicacao de coluna marcada so com
   `exigir: true` e pino**, recusada de outro jeito.
3. **O que fecha o buraco de verdade e o envelope da §11.5**: chave de tabela
   **sorteada** (128 bits de RBG, `shall` cumprido), envelopada pela mestra, e a
   **chave envelopada viajando junto do bloco de esquema**. Ele replica **sem
   publicar sal nenhum**, preserva chave distinta por arquivo, mantem o transplante
   de A2 recusado, e ainda entrega a rotacao que hoje nao existe. **Pre-requisito
   identico ao da proposta** — e e por isso que a proposta nao compra nada.

## O que SEC bloqueia, e o que deixa passar

**Bloqueia:** qualquer derivacao de material de cifra a partir do `Uuid` da coluna,
em qualquer alcance, enquanto A1, A3, A4 e A6 estiverem de pe — **e A4 nao se
conserta, e o comprimento do UUID**.

**Deixa passar, como trabalho independente do 293** (valem com ou sem a proposta):

1. `Uuid::de_texto` no caminho de `"id"` de coluna exigir **v7 e variante RFC
   9562**, e recusar o nulo — hoje aceita versao 0 e 15, medido. *(pedido 315)*
2. Conferencia de **id de coluna repetido** dentro da tabela. *(pedido 315)*
3. A guarda `transplantar_slot_entre_dois_reg_e_recusado` entrar no catalogo
   **agora**, afirmando a garantia que hoje existe **por consequencia** e que
   ninguem escreveu. *(pedido 316)*
4. A recusa do motor decidida pelo dono para coluna externa marcada em replicacao
   continua sendo o que fecha o caso 3.

## O que SEC NAO mediu

Nao repos o defeito dentro de um `.reg` de verdade (o transplante foi provado no
nivel da primitiva, com o mesmo nonce e o mesmo AAD que o `reg.rs` monta, nao sobre
dois arquivos reais); nao mediu o caminho de Windows do `misturar`, so o leu; e nao
rodou a suite nem o servidor.
