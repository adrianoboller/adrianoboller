# Parecer SEC: fáceis E (560, 550, 557, 556, 547, 549)

Revisor adversário de segurança (papel SEC). Árvore da frente:
`.claude/worktrees/agent-acb89d977173c655b/phxsql`, base `d76cd66` + diff não
comitado. Nada foi compilado no worktree. As provas rodaram no scratchpad
(`scratchpad/sec-faceis-e/`), com um crate de prova que liga o `phxsql-core` e o
`phxsql-sql` **da frente**, um `mysqld` 8.0.46 descartável e o `phxsqld.exe`
0.18.0 sob `wine`. Os processos que subi foram parados pelo PID. Os dados que
criei foram apagados depois de provado que nenhum processo os usava (cwd e fd).

**Revisão: 6 de 6 pedidos cobertos.**

## Veredito: COM CONDIÇÃO

| Pedido | Veredito | Por quê |
|---|---|---|
| 560 | **COM CONDIÇÃO**: integra, mas **não fecha** | 3 formas de diretiva ainda vazam no anel, no `perfil.txt` e no `jobs.json` (medido). A recusa cita o que sobrou (medido) |
| 550 | **LIBERA** | Analisa, não recorta. O `<N bytes>` é do texto do relé, não da senha |
| 556 | **LIBERA** | Não achei injeção: 8 charsets multibyte e 3 modos, medido no `mysqld` |
| 557 | **LIBERA** | Passa pela porta do 464 |
| 547 | **COM CONDIÇÃO**: integra, mas **não fecha** | O irmão, o desafio do protocolo próprio, continua sem teto (medido) |
| 549 | **LIBERA** | Só fecha: antes, o login saía para todos |
| **Portão 3 (anterior à frente)** | **BLOQUEIA A VERSÃO** | Contorno do direito por tabela pela caixa do nome no Windows. Medido sob `wine` |

A frente não introduziu nenhuma exposição. Todas as mudanças dela só apertam.
Barrar a integração deixaria mais vazamentos abertos do que integrar, então a
saída conservadora é esta:

- integrar;
- manter o 560 e o 547 `☐`, com o resto registrado;
- abrir já os pedidos novos abaixo.

---

## Achados, por prioridade

### A1. O direito por tabela se contorna pela caixa do nome (Windows) — ALTA, medido, pedido novo ☐ (bloqueia a versão)

É o buraco que a própria frente deixou dito como «não medido»
(`cognicao_direito-perguntado-pela-chave-da-trava-cai-na-base_20260924_1945.md`).
Foi medido agora.

- **Porta.** O portão 3 casa o nome exato: `servidor.rs:11244` →
  `pode_em` → `permissoes_casando(.., casa_exato)` (`usuarios.rs:811`, `:889`).
  O motor abre `Table::abrir(dir, nome)`, e num sistema de arquivos que não
  distingue caixa (NTFS, APFS) `folha` abre `Folha.reg`.
- **Cenário.** O `config.json` tem `ana: {"bases":{"loja":{"ler":true,"tabelas":{"Folha":{}}}}}`,
  e `ana` faz login.
  - `{"op":"varrer","database":"loja","tabela":"Folha"}` → `ACESSO_NEGADO`.
  - O mesmo com `"tabela":"folha"`, com `"FOLHA"` e com
    `{"op":"sql","texto":"SELECT * FROM folha"}` → **`"salario":"SALARIO-SECRETO-9999"`**.
- **Medido** no `phxsqld.exe` 0.18.0, com SHA-256 igual ao `MANIFESTO`, sob
  `wine`, que emula a busca de arquivo sem caixa do Win32.
  - O portão da base atual é o mesmo casamento exato (lido).
  - No Linux, o mesmo pedido cai em «não existe» (lido): o defeito depende do
    sistema operacional, e só se prova contra ele.
  - O `database` com outra caixa (`RH` contra a regra `rh`) usa o mesmo
    mecanismo. **Não foi medido.**
- **Alcance.** `juntar`, `unir` e `pivotar` conferem pelo mesmo `pode_em`, com o
  nome como veio. Herdam o furo.
- **Teste adverso.**
  - Unitário, que falha hoje: com a regra `Folha:{}` e a base lendo,
    `pode_em("loja","FOLHA",Ler)` e `pode_em("loja","folha",Ler)` têm de ser
    falsos.
  - Prova de SO: a receita do `wine` acima, ou uma corrida no Windows.
- **Direção, para o DBA decidir.**
  - O portão casa sem caixa, a mesma decisão que o `carga::chave` já tomou para
    a trava. Ou resolve o nome canônico do disco **antes** do portão.
  - Junto: recusar criar tabela ou banco cujo nome só difere de outro pela
    caixa, também no Linux, porque hoje duas tabelas assim dividiriam a mesma
    trava e, com o portão sem caixa, a mesma regra.

### A2. 560 fechado pela metade: três formas de diretiva vazam — ALTA (fere pétrea), medido, fica ☐

A frente reconhece só `SET <nome>(.<nome>)* [=]`, colado no `SET`
(`usuario.rs:311`). É um **segundo analisador** da gramática da diretiva: o
primeiro é o `diretiva::Passo`. E ele só reconhece uma forma. Medido pelas
funções da frente:

| Entrada | Anel (`sem_a_senha_se_mencionada`) | `perfil.txt` (`normalizado`) | Job recusa? |
|---|---|---|---|
| `ALTER SERVER SET max_linhas = 500, alertas.email.senha = abc123` | `… senha = abc123` | `… senha = abc123` | **não** |
| `ALTER SERVER SET 'alertas.email.senha' = abc123` | `… '***' = abc123` | `… ? = abc123` | **não** |
| `ALTER SERVER SET (alertas.email.senha) = abc123` | `… ) = abc123` | `… ) = abc123` | **não** |

- As três são recusadas pelo `diretiva::comando`, mas o Profiler captura
  **antes** da recusa. O job guarda o texto cru no `jobs.json`.
- A primeira é o hábito de quem vem do `SET GLOBAL a=1, b=2`.
- **Teste adverso.** `a_diretiva_sigilosa_torta_nao_sai_crua`: as três
  entradas, `!contains("abc123")` no anel e no `normalizado`, e
  `diretiva_com_segredo == true`. Cai hoje.
- **Direção.** A lei «função e comando vêm do mesmo motor»:
  - a redação pergunta ao **próprio** `diretiva::comando`;
  - se ele analisa, reserializa com o `valor` tapado quando o campo é
    sigiloso;
  - se o texto começa por `ALTER`/`SHOW`, a diretiva **não** analisa e o
    `menciona_segredo` diz sim, sai `<comando invalido, N bytes>`.
  - O que não se analisa vira tamanho, e não é o casador de uma forma que
    decide.

### A3. A recusa da diretiva cita o que sobrou — MÉDIA (fere pétrea), medido, pedido novo ☐

- **Entrada.** `ALTER SERVER SET alertas.email.senha = correct horse battery`,
  uma frase-senha sem aspas.
- **Efeito.** `SQL, coluna 1: sobrou "horse" depois do comando de ALTER`
  (`diretiva.rs:408-419`, pelo `Token::descrever`, que devolve a palavra
  solta crua).
  - O texto vai à resposta.
  - Vai ao `acessos.log`: `erro: resultado.err().to_string()`,
    `servidor.rs:7580/9736/10132…`.
  - Vai ao `erro` do evento do Profiler.
- **O irmão já foi consertado.** A recusa do cadastro diz «sobrou um símbolo
  depois do comando — se era a senha…» sem citar (`usuario.rs:619`). O guarda
  de `lib.rs:276-283` cobre só literal e nome citado, e não a palavra solta.
- **Teste adverso.** Acrescentar à tabela do guarda de `phxsql-sql/src/lib.rs`
  o caso `diretiva::comando("ALTER SERVER SET token = a SEGREDO123")`, com a
  marca na **palavra solta**. Cai hoje.

### A4. 547 fechado pela metade: o desafio do protocolo próprio não tem teto — MÉDIA, medido, fica ☐

- **Mesma pergunta, outro caminho.** `phxsql_core::desafio::prova_de_senha`
  (`desafio.rs:160`) paga as `iteracoes` que o **servidor** manda. Chamam-na:
  - o driver ODBC (`phxsql-odbc/src/conexao.rs:404-426`);
  - a réplica com `senha` em claro (`replica.rs:252-269`);
  - **o DbLink para outro PhxSql, sempre** (`dblink/phx.rs:106`,
    `autenticar(usuario, "", senha)`).
- **Cenário.** Quem está no meio do fio em claro, ou um PhxSql remoto
  malicioso, responde ao `desafio` com `"iteracoes":4294967295`.
- **Medido.** `prova_de_senha(.., 2_100_001, ..)` → `Ok` em 1,42 s, e
  `(.., 21_000_000, ..)` → `Ok` em 14,9 s: dez vezes o teto do SCRAM, aceito.
  O `u32::MAX` dá ~51 min a 0,71 µs por iteração. É conta, não medida.
- **Teste adverso.**
  - `o_desafio_acima_do_teto_recusa_antes_do_pbkdf2`, com `teto+1`: o `Err`
    tem de vir em menos de 1 s. Cai hoje, porque devolve `Ok` depois de 1,4 s.
  - A prova de soquete: um `TcpListener` falso que responde ao `desafio` com
    `u32::MAX`. O ODBC e o `replica::Cliente` têm de voltar em menos de 1 s.
- **Direção.** O teto vai ao `core`, como o `sigilo` do 560, e é aplicado
  **dentro** do `prova_de_senha`. O `pg::scram::TETO_DE_ITERACOES` e o
  `ITERACOES_MAXIMAS_DO_CADASTRO` passam a ser ele: um motor só.

### A5. O irmão do 556/557 do outro lado do fio: o erro do banco remoto cita o valor — MÉDIA (LGPD), medido e lido, pedido novo ☐

- **Medido no `mysqld` 8.0.46.**
  - `1292 Incorrect date value: '999.888.777-66' for column 'nasc'`.
  - `1366 Incorrect string value: '\xF0\x9F\x98\x80 S...' for column 'nome'`.
- **Lido.** `mysql.rs:435-441` copia a mensagem verbatim. O
  `pg/mod.rs:460-484` junta o `Detail`, e o do PostgreSQL traz
  `Key (cpf)=(…) already exists`.
- **Cenário.** O `dblink_sincronizar` empurra uma coluna local marcada para uma
  coluna remota de tipo ou charset diferente. O valor do titular vai à
  resposta, ao `acessos.log` e ao Profiler.
- **Por que agora.** O 556 fez o texto multilinha e o de mais de 128 bytes
  chegarem ao outro lado, e com isso mais recusas remotas.
- O motor daqui não cita valor na recusa de unicidade
  (`table.rs:4355/4794/5129/6960`). O de lá cita, e nós copiamos.
- **Teste adverso.** Um MySQL falso (`TcpListener`) que responde ao
  `COM_QUERY` com `ERR 1292 … '999.888.777-66' …`. O erro do
  `dblink_sincronizar` sobre a coluna marcada não pode conter `999.888`.
- **Direção.** A recusa remota passa pela porta da coluna quando o empurrão
  toca coluna marcada. Sem saber qual, o texto do servidor remoto vira código
  mais tamanho, como no 550.

### A6. O cabeçalho do `.reg` cifrado não tem teto de iterações, e deriva segurando o `COFRE` global — BAIXA-MÉDIA, lido, ⏸ proposto

- **Onde.** `cofre.rs:1022-1029` tem piso (`ITERACOES_MINIMAS`) e não tem teto.
  O `derivar` (`cofre.rs:268-286`) roda o PBKDF2 **com o `COFRE.lock()`
  preso**.
- **Cenário.** Um `.reg` de backup restaurado ou copiado de fora com
  `u32::MAX` no byte 44. Abrir a tabela custa ~51 min e trava toda derivação
  do processo.
- É a mesma pergunta do `ITERACOES_MAXIMAS_DO_CADASTRO` («arquivo que alguém
  pode ter editado»). Entra no mesmo motor do A4.
- **Teste.** Cabeçalho com `u32::MAX` → `Corrompido` em menos de 1 s.

### A7. A diretiva comentada no job (dispensa da frente) — BAIXA, medido, ⏸ proposto

- `-- ALTER SERVER SET token = abc123\nSELECT 1` e o `/* … */` são aceitos pelo
  job, e o `jobs.json` guarda o texto cru. Medido.
- **A razão da dispensa não se sustenta inteira.** Não é preciso casar as
  letras `TOKEN`: dá para **analisar o corpo do comentário** como SQL e
  perguntar `diretiva_com_segredo` a ele. `-- limpa tokens expirados` não é
  diretiva e passa. O job de limpeza continua.

### A8. Menores — ⏸ proposto

- **`campos` como lista.** `{"op":"config_gravar","campos":[["alertas.email.senha","X"]]}`
  sai com o `X` cru no anel e no `perfil.txt`. O servidor recusa, o Profiler
  captura antes. Lido.
- **`pode_em_sem_caixa` é `pub`** (`usuarios.rs:879`).
  - Ele casa sem caixa, e quem o usar amanhã como portão cria o inverso do A1.
  - No Linux, com duas tabelas que só diferem pela caixa, a resposta depende
    da ordem das regras no `config.json`.
  - Sugestão: `pub(crate)` e o nome dizendo «só para o recado». Morre com a
    direção do A1.
- **ſ e K (Kelvin).** `alertas.email.ſenha` sai cru, porque o `campo_sigiloso`
  usa `to_ascii_lowercase`. Não é campo que exista, e o servidor o recusa.
  **Não é defeito.** Fica registrado para que ninguém «conserte» com
  `to_lowercase` só num dos dois lados.

---

## As cinco perguntas

1. **Ainda há caminho que leva segredo cru?** Sim.
   - A2: anel, `perfil.txt` e `jobs.json`.
   - A3: resposta, `acessos.log` e Profiler.
   - A7: `jobs.json`.
   - A8: anel e `perfil.txt` com `campos` em lista.
   - O JSON aninhado (`{"campos":{"alertas":{"email":{"senha":…}}}}`) e a
     chave em maiúscula (`"REST.TOKEN"`, `"Valor"`, `"CAMPO"`) estão cobertos.
     Lido.
   - `ALTER USER … PASSWORD` junto de diretiva, no mesmo roteiro, medido:
     - as duas ordens com a forma reconhecida saem tapadas;
     - `ALTER USER ana PASSWORD 'x'; ALTER SERVER SET max_linhas = 5, token = abc123`
       ainda deixa o `abc123` no anel e no `perfil.txt`. É o A2: a regra da
       senha só vale até o `;`.
2. **Analisa ou recorta?**
   - O `eco_redigido` (550) analisa: código, RFC 3463 e tamanho.
   - O `literal_de_valor` recusa por análise do caractere.
   - A redação da diretiva (560) é análise por símbolos, **mas de uma forma
     só**. Ali está o recorte que sobrou (A2), e o conserto é perguntar ao
     analisador que decide.
3. **O empurrão abre injeção?** **Não achei.** Medido no `mysqld` 8.0.46:
   - 13 valores adversos montados como o `literal_de_valor` monta:
     - o `€` com byte-líder GBK colado na aspa de fecho;
     - `"`, `;DROP`, `*/ /*`, `#` e `\n`/`\r\n`;
     - `U+2028`, `＇`, `ʼ` e `＼` de largura total, e `0x1A`;
     - 300 bytes.
   - Cliente forçado (`--skip-character-set-client-handshake`) para gbk,
     sjis, cp932, big5, gb18030, eucjpms, ujis e euckr, e depois utf8mb4 em
     `ANSI,NO_BACKSLASH_ESCAPES,PIPES_AS_CONCAT`.
   - Em todos: 13 linhas, 387 bytes idênticos, uma instrução, tabela intacta.
   - O porquê: `0x27` não é byte de continuação em nenhum desses charsets.
     Sem `'` nem `\`, o literal não fecha nem escapa.
   - O quadro acima de 16 MiB é recusado (`mysql.rs:370`), então não há
     contrabando de pacote. O lote de 500 com Memo grande agora estoura ali e
     falha inteiro. É limite funcional, não furo.
   - O PostgreSQL não chega ao empurrão: o `SELECT * FROM` com o nome entre
     crases falha antes (lido). Com `standard_conforming_strings`, e sem `\`, não
     haveria escape.
   - O UTF-8 inválido não existe: é `String` de Rust. O byte nulo é recusado.
4. **549.**
   - O `pode_em_sem_caixa` não abre direito de operação. Só decide mostrar o
     login, que antes saía para todos.
   - **O mesmo buraco pela porta principal existe e foi medido: A1.**
5. **550.**
   - O `<N bytes>` é o tamanho do texto **do relé**. Só quando o relé ecoa a
     senha ele dá o tamanho em degraus de 3 caracteres (base64), a quem lê o
     log. É o preço já aceito pela regra «o que não se analisa vira tamanho».
     Não bloqueia.
   - O código estendido vem só do relé, e só na forma `[245].d{1,3}.d{1,3}`.

## Condições para a integração

1. **O 560 e o 547 não viram ☑** nesta integração. Ficam `☐`:
   - o 560 com as três formas do A2, medidas;
   - o 547 com o irmão A4.

   Ou a frente fecha os dois agora pela direção dita, cada um com o teste
   adverso vermelho antes e verde depois.
2. **Três pedidos novos na conta**, abertos no commit de integração:
   - **A1**: `☐`, bloqueia a versão;
   - **A3**: `☐`, fere pétrea;
   - **A5**: `☐`, dado pessoal em log.
3. **A6, A7 e A8 nascem `⏸`**, com a severidade dita, para o juiz confirmar.
