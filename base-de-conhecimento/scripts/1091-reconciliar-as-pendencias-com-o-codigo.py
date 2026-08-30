# Reconciliar as pendencias com o codigo
# 29/08 06:24

import io,re
p='docs/PENDENCIAS.md'
s=io.open(p,encoding='utf-8').read()

novos = {
 6:  ('◐', '**Servidor MCP** | **esqueleto feito, transporte não.** `src/mcp.rs` com `initialize`, `tools/list`, `tools/call` e 9 ferramentas, `docs/MCP.md`. A ponte **não executa nada** — recebe o `despachar`, então o portão continua sendo um. Falta quem leia de stdin'),
 40: ('☑️','**Parar e subir o serviço pela interface**, trocando a porta | **feito, e o impedimento resolvido de verdade**: despertador que conecta no próprio endereço, em vez de *polling* — 100 ms de intervalo poriam 100 ms em toda conexão nova. A porta nova é **presa antes** de a velha ser solta, e o processo não é derrubado, então a web é sempre o caminho de volta. 5 testes por soquete, um deles derrubando a porta de dados e a levantando pela web'),
 51: ('☑️','**Jobs de execução** | `jobs.rs`, cadastro em `jobs.json`, corridas append-only em `jobs.log`, relógio de 30 s e 4 operações, todas exigindo `administrar`. **O job roda com o poder do usuário dele** — e isso obrigou a extrair os portões comuns do `despachar` para uma função só, em vez de copiar a conferência. 8 testes por soquete, incluindo o que prova que um job de `so_le` não cria database'),
 83: ('◐', '**Comandos SQL reconhecem `matriz.estoque` e `filial.estoque`** | o **endereçamento** já funcionava em toda operação. Agora existe a crate `phxsql-sql` — léxico, sintaxe e tradutor de um `SELECT` simples, 44 testes, e `FROM matriz.estoque` fecha o lado SQL do pedido. **Falta ligar ao servidor**: não há `op:"sql"`'),
 86: ('◐', '**Depois testar com PostgreSQL(R) e outros** | **cliente feito, dialeto não.** `src/pg/` com SCRAM-SHA-256 conferido contra o vetor da §3 do RFC 7677, startup, `Query`, `RowDescription`/`DataRow`. `md5` e `password` recusados com a linha a mudar no `pg_hba.conf`. `Motor::conecta()` continua `false` de propósito: as operações do DbLink montam SQL de MySQL(R), e acendê-lo ligaria um botão que falha'),
 101:('◐', '**Cifrar e compactar `.log`, `.trash` e `.reason`** | **cifra feita, integração não; compactação medida e recusada.** `cifra.rs` traz ChaCha20-Poly1305 (RFC 8439) com **todos** os vetores oficiais passando — ChaCha e não AES porque AES portátil usa tabela, e tabela em cache vaza chave por tempo. Já compactar: os três somam 19,84% da tabela e comprimiriam 3–4×, mas **volumes fechados = 0** (cortam a 1 GiB, e o `.log` só fecha o primeiro em ~24 milhões de eventos) — compactar por volume fechado pouparia **exatamente zero byte**. A decisão que falta é o `bytes_por_arquivo` do diário'),
 113:('☑️','**Atacar os 83,5% do `.ndx`** | **medido, e o alvo era outro** — não era localidade de chave, era reler e recalcular CRC-32 da mesma página. Um cache de páginas levou a inserção de **44,4 → 18,5 µs**; o cabeçalho do `.reg` que reserializava o esquema, a 17,0; o do `.log`, a 15,9; o do `.ndx`, que gravava 4 KiB por chave, a 14,5; o CRC slice-by-16, a 13,1; e o **cache write-back**, a **7,5 µs — 2,19× só nesta rodada**. `docs/DESEMPENHO.md` §2 a §4.8'),
 125:('☑️','**Marcar coluna como dado pessoal (LGPD/GDPR)** | PSCH **v6**, três graus (`nao`/`pessoal`/`sensivel`, LGPD art. 5º I e II), com o byte no **fim** do bloco para quem lê v5 parar antes. Op `dados_pessoais` audita a base — e como ela **não tem campo `tabela`** (o furo do `juntar`/`unir`), filtra tabela a tabela por dentro. Não adivinha por nome; devolve quantas colunas ficaram sem classificação. Mais a tela que audita, que diz *que não sabe* quando o esquema não traz a marca'),
 127:('◐', '**Diagrama ER e editor de modelo** | **o diagrama está feito** (`ui/diagrama-er.js`), e a tela declara que o editor é outra rodada. Sete defeitos de layout achados **abrindo no navegador**, nenhum deles visível no código. E um achado que corrige uma meia-verdade desta lista: **`criar_tabela` não declara chave estrangeira** — o `esquema` as reporta e o formato as suporta, mas nenhuma operação do protocolo as cria; hoje só via API Rust. O editor de modelo esbarra nisso primeiro'),
}

linhas = s.split('\n')
for i,l in enumerate(linhas):
    m = re.match(r'^\| (☑️|◐|☐) \| (\d+) \|', l)
    if m and int(m.group(2)) in novos:
        e, texto = novos[int(m.group(2))]
        linhas[i] = f'| {e} | {m.group(2)} | {texto} |'
s = '\n'.join(linhas)

feitos = sum(1 for l in linhas if l.startswith('| ☑️ |'))
parc   = sum(1 for l in linhas if l.startswith('| ◐ |'))
plan   = sum(1 for l in linhas if l.startswith('| ☐ |'))
s = re.sub(r'\*\*\d+ feitos · \d+ parciais · \d+ planejados\*\*, de \d+ pedidos\.',
           f'**{feitos} feitos · {parc} parciais · {plan} planejados**, de {feitos+parc+plan} pedidos.', s)
io.open(p,'w',encoding='utf-8').write(s)
print(f'{feitos} feitos, {parc} parciais, {plan} planejados, de {feitos+parc+plan}')
