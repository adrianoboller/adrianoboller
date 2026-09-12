# O CRC do `.reg` e o hash do ledger são camadas diferentes — e a prova real do ledger tem de entrar pela porta que o motor ACEITA

Data da descoberta: 12/09/2026, ~14:52 (o executor B da frente blockchain
descobriu ao desenhar a prova real; eu conferi na integração).

## 1. O que aconteceu

Ao construir o E3 (verificação de cadeia do ledger encadeado), a prova real dos
dois sentidos precisava «adulterar um bloco antigo e ver a verificação pegar».
O caminho óbvio — corromper um byte cru do conteúdo do bloco direto no `.reg` —
**não prova o ledger**: a camada de armazenamento (`t.ler`, `reg.rs:1919`)
confere o **CRC-32 da linha na leitura** e **recusa a linha antes** de o ledger
sequer ver o bloco. São duas camadas de integridade distintas, e o teste ingênuo
mediria a de baixo.

A prova certa entra pela porta que o motor **aceita**: alterar o conteúdo por um
`atualizar` legítimo (que **recalcula** o CRC, então a linha passa na leitura),
deixando só o `hash` do ledger **velho**. É exatamente a ameaça que o hash da
cadeia existe para pegar e o CRC **não** pega — linha regravada, íntegra para o
armazenamento, mas divergente da prova de conteúdo encadeada.

## 2. O que eu concluí primeiro, e estava errado

No prompt do executor, escrevi «corromper 1 byte do conteúdo direto no arquivo →
a verificação PEGA». Errado no mecanismo: quem pega o byte corrompido cru é o
**CRC do `.reg`**, não o ledger — o teste passaria «por engano», atribuindo à
cadeia uma detecção que é da camada de baixo. Teste que passa pelo motivo errado
é o pior tipo (a lição do Profiler: o conserto funcionou por outra causa).

## 3. O que a medição disse

- `reg.rs:1919`: `t.ler` confere o CRC-32 da linha e recusa a corrompida na leitura.
- Os 8 testes do `ledger.rs` separam as duas ameaças: (a) `conteudo_adulterado`
  entra por `atualizar` (CRC válido, hash velho) → a cadeia pega em `Conteudo`;
  (b) conteúdo **e** hash regravados juntos → a cadeia pega a **Ligação** no bloco
  seguinte (`anterior_{n+1} != hash_n`). Cada um falha com a checagem desligada e
  passa com ela ligada — prova real dos dois sentidos, na árvore quente.

## 4. A regra

Quando um recurso novo acrescenta uma camada de integridade sobre outra que já
existe, a prova real dele tem de entrar pela porta que a camada de baixo
**aceita** — senão o teste credita à camada nova uma detecção que é da antiga.
Nomeie qual camada pega o quê antes de escrever o teste.

## 5. Como está guardado hoje

No módulo `crates/phxsql-store/src/ledger.rs` (doc do módulo + os 8 testes),
integrado no commit `540e5cd`. O formato em disco não mudou, então nada foi para
o `FORMATO.md` além de, se couber, uma nota curta apontando para o módulo. É
aprendizado de **processo/mecanismo** (qual camada prova o quê), não reafirmação
de pétrea — por isso vira cognição, não uma terceira cópia de «prova real nos
dois sentidos».
