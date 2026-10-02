# Pareceres da rodada «equipe inteira» (02/10/2026)

## Tradutor — entregue
- 13 casca.* ok; 0 correções obrigatórias; 1 opcional (casca.busca en).
- Catálogo: SALAS/APELIDO en melhoráveis (decalque); 1 en==pt legítimo (phone_id). IRC/Twitch usam NICK, XMPP usa APELIDO (nome de env diverge) — registrar.
- en==pt a conferir no L2: plugins.col.nome, perfis.inativo, config.tag_novo.
- ide.js:305 'Helix' cravado no título da aba → isentar com motivo.
- INVENTÁRIO: sem mecanismo de idioma no servidor; 7 frases do canal (canais/mod.rs:480–643) e 24 format! do fluxos.rs em pt cravado, sem acento. Pedido: FABRICA_CANAL + chave agente.idioma.

## DBA (C) — entregue
- MUDAR AGORA (repassado ao engenheiro da onda 2): hash da definição = struct (invalida fluxo gravado a cada campo novo) → hash canônico; formato:u8 no Relatorio; saida+itens duplicados → um só; save() engole erro (fluxos.rs:1361); doc da ordem instável de passos.
- Onda 3: teto de bytes por passo e saída grande em arquivo separado com sha256.
- XMPP: compat ok; e_sala ignora caixa mas PERMITIDOS é exato (xmpp.rs:373 × mod.rs:377) → defeito pequeno; nick de MUC não é identidade → PRODUTO (sobe ao dono); sem MAM: mensagem durante queda não chega → manual.

## SEC — entregue
- ALTO A1 (sala = qualquer ocupante comanda + responde ask_user), A2 (nick forjável), A3 (por_item sem teto/concorrência). MÉDIO M1 (parser de atributo), M2 (alto de memória), M3 (texto não permitido vai à caixa), M4 (laço com nick trocado), M5 (args inteiros de item externo; {{erro}} injeção). BAIXO B1-B3 ok/normalização de JID.
- Ação: A3+M5 → engenheiro onda 2; A1/A2/M1-M4/B3 → nova frente engenheiro XMPP (lançada). Bloqueio do Go até fechar A1/A2/A3.

## Zelador (D) — entregue
- Livre 1.077 → 2.548 MB (df). Apagado com prova de /proc: examples antigos 951, /tmp phx-* 119, uv/pip 400, 153 crates órfãs do registry/src 159 (todas com .crate no cache).
- Premissa «executáveis >20 MB» não se sustentou: 5 executáveis, 43 MB; os 12,9 GB de deps são rlib/rmeta duplicados (9 cópias de libphxclaw_agent ≈ 1,1 GB). Poda deles só em rodada sem cargo vivo.
- Aprendizado: `pgrep cargo` é prova fraca — o cargo vivo só apareceu pelo fd em target/debug/.cargo-lock. → cognição.

## QA (G) — entregue
- Catracas rodadas: leituras soltas 5/5, textos 0/0 (357 chaves), ui_navegacao 64/64 (L2 em andamento), ui_paineis 14/14, ui_config 21/21. Nenhuma catraca sobe.
- Sem guarda: (e) segredo em variável de fluxo — zero testes (→ enviado ao engenheiro onda 2); (f) percentual digitado no SPRINTS × gerador (pendência: guarda nova); (b) PARADA de estado sem teste; SPRINTS.md:7 listava 3 estados — CORRIGIDO (4).
- Régua do dossiê: ui_paineis.json existia e aparecia NÃO MEDIDO (casava a forma da chamada) — CORRIGIDO em numeros.py medidores().
- Roteiros sem json: ui_navegacao, ui_config, textos_fora_da_fabrica, ide_web, pwa_ponte, desktop_e2e (⏸ já registrado; molde qualificar.json).

## Pesquisador (J) — entregue (docs/propostas/sp32-r5-r1-pesquisa.md)
- R5 minimapa: painel no IDE web (~40 Rust + ~150 JS, zero toque no Helix). Hipótese patch morreu: 0 minimap no Helix 25.07.1, issue #2210 sem PR.
- R5 dobra: FORA com número (PR #14593 +6.314 linhas falha em 7 arquivos no 25.07.1; #16305 fechado 22/09). phxclaw.json fica «nao» com motivo.
- R1 RSA: viável na onda seguinte, verificação só, ~380 linhas, vetores Wycheproof 259 + CAVP; bip340.rs não reaproveita (0 funções); Montgomery recusado (~7 ms bit a bit abaixo do RTT).

## Documentação (H) — entregue
- N8N.md §8 motor onda 1; GUIA_DO_OPERADOR.md sala XMPP; COMPARACAO_MOCKUP §5 L1 entregue; TECNOLOGIAS.md NOVO com extrator tools/gerar_tecnologias.py (656 arq / 185.006 linhas; Rust 122.026 src); CHANGELOG v0.70 (v0.68/69 não existem, dito); cognição PENDENTE (doc do módulo × validar).
- Defeito: fluxos.rs:130 «{{entrada}}» não existe → enviado ao engenheiro onda 2.

## Designer (E) L2 — entregue
- 7 arquivos +249/−13; 23 chaves painel.* (357 total); ui_navegacao 64/64; textos 0; qualificar T1/T4 18/18; console 0.
- 3 defeitos exercitando (coluna vazia; família como texto cravado → chip = chave do JSON; captura em meia-transição).
- Pré-existente (W2): tela Tarefas a 1536 corta «Criada em» → pendência.
- Cognições a registrar: captura com tempo fixo mede meio-transição; identificador de família é dado.

## Prova real (F) — entregue
- GRAVE: número digitado no rodapé passa nas duas provas (ui_navegacao só varre telas). Teste com RED→GREEN em scratchpad/prova-f/ui_navegacao_casca.mjs; precisa de data-fonte="host" no #brandVersion (index.html:62).
- XMPP: prova declarada do aviso da sala é falsa (estrofe de teste sem <body>); privada de ocupante sem teste.
- Fluxo: laco_lotes «central» mede veredito, não conteúdo (descarta passadas boas, fluxos.rs:1229-1240); lacunas em juntar/ramo, prazo() menor, falhos×bloqueado.

## Estado ao parar (02/10, ordem do dono «Pare»)
- Subagentes no limite semanal da conta (volta 06/10 20h UTC).
- XMPP conserto SEC pela metade: NÃO COMPILA (ligar.rs:370 sem permitidos/confiar_no_nick; xmpp.rs:866 teste sem tratar Result do recortar). Falta chave CONFIAR_NO_NICK no catálogo e regerar artefatos.
- Onda 2 do fluxo: compila; faltam testes da onda 2 e os itens do DBA/SEC A3/QA.
- Nada disto passou pelo Integrador. Branch de rascunho, não o phxclaw/v070-nativo.
