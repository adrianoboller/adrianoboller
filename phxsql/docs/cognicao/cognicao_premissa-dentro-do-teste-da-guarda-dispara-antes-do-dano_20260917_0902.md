# A premissa conferida DENTRO do teste da guarda dispara antes do dano

Pedido 316, frente B, 17/09/2026 09:02 UTC.

## 1. O que aconteceu

O pedido 316 pedia a prova de uma garantia que só existia por consequência:
nenhuma amarração do slot cifrado carrega identidade de arquivo — o
`aad_do_slot` é `(volume, rowid, versao)`
(`crates/phxsql-store/src/reg.rs:2286-2292`), o `rotulo_da_prova` é
`(MAGIC_REG, versao, slot_size)` (`:2253-2259`) e o tempero do
`nonce_de_pedaco` viaja **dentro** do slot, portanto viaja junto na cópia. O
que separa dois `.reg` é só o sal sorteado por arquivo em
`cofre::Material::novo()` (`crates/phxsql-store/src/cofre.rs:347-363`).

O teste nasceu com as três premissas conferidas dentro dele, o que parecia
zelo: geometria igual, `versao` do slot igual, e **sais diferentes**. Com o
defeito reposto — sal fixo — o teste caiu, e o relatório disse «2/2 caíram».

Só que ele caiu no `assert_ne!` dos sais, na linha 5 do corpo. O transplante
nunca aconteceu, e a garantia continuou sem prova.

## 2. O que eu concluí primeiro, e estava errado

Que a lei desta casa sobre prova real era «a conferência não pode acontecer
**depois** do dano» — foi assim que ela ficou escrita, do caso em que o
veredito era conferido depois da leitura já ter lido demais. Concluí que pôr a
conferência **antes** era o lado seguro, e que quanto mais premissa medida
dentro do teste, mais forte ele ficava.

É o contrário, e pelo mesmo motivo: o que importa não é o lado, é **se o
assert que dispara é o da guarda**. Conferência antes do dano transforma o
vermelho num vermelho da premissa — o teste cai, o catálogo diz PROVADA, e
ninguém exercitou a porta.

## 3. O que a medição disse

Com o sal fixo em `[0x5A; 16]`, os dois `.reg` derivam a mesma chave.

- **Premissa dentro do teste da guarda** (uma versão só): caiu em
  `assertion left != right failed: os dois .reg nasceram com o MESMO sal`,
  `left` e `right` ambos `[90; 16]`. O transplante não rodou.
- **Premissa em teste próprio** (duas versões): o teste da guarda passou das
  premissas e caiu onde tinha de cair —
  `o slot de OUTRO arquivo abriu em b: [...] saiu Ok(Some([Int(1), Str("Alice de Origem"), Bool(false), UInt(1)]))`.
  A linha 1 da tabela `b` devolveu o conteúdo da tabela `a`, **sem erro
  nenhum**.

O `Ok(Some(...))` com o nome da linha do outro arquivo é a garantia medida. O
`assertion left != right` dos sais não era.

Os outros 15 testes de `cifra-dos-dados.rs` ficaram verdes nos dois casos,
inclusive `trocar_o_corpo_de_uma_linha_pela_outra_nao_passa` (a amarração
*dentro* do arquivo não foi tocada) e
`senha_errada_e_falta_de_senha_param_na_abertura` (sal fixo não é chave fixa).
`provar-guardas.py --so slot-de-outro-reg`: **PROVADA, 2/2 caíram**, 11,2 s.

## 4. A regra

**Premissa se afirma em teste próprio; o teste da guarda só confere o que a
guarda faz.** Quando o defeito reposto derruba os dois, cada vermelho prova
uma metade; quando ele derruba só a premissa, a guarda não foi exercitada e o
relatório mente dizendo PROVADA.

E o crivo que decide, em uma pergunta: *com o defeito reposto, QUAL assert
dispara?* Se não for o da guarda, o teste está medindo outra coisa.

## 5. Como está guardado hoje

- `crates/phxsql-store/tests/cifra-dos-dados.rs`:
  `transplantar_slot_entre_dois_reg_e_recusado` (a guarda) e
  `dois_reg_novos_nascem_com_sais_diferentes` (a premissa), separados, com o
  porquê da separação escrito nos dois.
- `bancada/guardas/catalogo.py`, entrada `slot-de-outro-reg`: os dois em
  `caem`, e o comentário diz qual metade cada um prova.
- O que **não** está guardado: não há conferidor que ache, no repositório
  inteiro, outro teste de guarda com premissa conferida antes do dano. A busca
  por padrão de texto não distingue premissa de verificação — é a mesma recusa
  medida do conferidor genérico de erro cru (8 interpolações, 2 defeitos). O
  buraco fica nomeado aqui.
