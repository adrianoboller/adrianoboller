# O que falta no WX Claude Code

Fonte da pagina `docs/o-que-falta.html`, que **nao se edita**:
`python3 docs/dossie/gerar-o-que-falta.py` a gera daqui e conta os estados sozinha.

Estados: `falta` (nada feito), `parcial` (parte existe, e a nota diz qual),
`feito`. Estado que muda so muda **medido** -- a lista do que falta tambem e
palpite ate alguem medir.

<!-- lead: o paragrafo de abertura da pagina; texto de interface, com acento -->

O plugin hoje governa a conversão: questionário, gates, evidências, equipe de
agentes sobre o Help, PMO, esqueleto do destino, procedência e prova. O que
falta é o que transforma governança em fábrica: ler o projeto WX de verdade,
traduzir WLanguage de forma reprodutível, cobrir WEBDEV e Mobile, e provar
igualdade contra o legado rodando.

<!-- fim do lead -->

## 1. Entrada: o projeto WX de verdade

### 1. Arquivos nativos do projeto (.wdp/.wwp/.wpp, .wdw, .wwh, .wdc, .wdg, .wde, .wdr, .wda, .wdk)

- estado: `falta`
- tamanho: 5 · projeto próprio
- por que importa: A extração por PDF perde o que o PDF não mostra: propriedade de controle, ordem de tabulação, evento não impresso, código de componente. É a maior fonte de GAP-* hoje.
- hoje: O plugin lê PDFs exportados pelo IDE (código, interfaces, queries, completo) e o SQL da análise. O G0 classifica isso como FORENSIC.
- construir: Leitor dos formatos nativos, ou ao menos do que o IDE exporta em texto: código por elemento, descrição de janelas/páginas com controles e propriedades, análise com arquivos, chaves e ligações, queries em SQL e em WDR. Quem faz: script que o cliente roda no IDE (WLanguage sobre o próprio projeto, via `EnumElement`/`ElementInfo`) e gera JSON; o plugin passa a ler JSON em vez de PDF.

### 2. Análise HFSQL a partir do .wda

- estado: `falta`
- tamanho: 4 · muito grande
- por que importa: Sem isso, o modelo de dados do destino é adivinhado do SQL e a integridade referencial se perde.
- hoje: Só o script SQL que o cliente exporta à mão.
- construir: Descrição dos arquivos, itens, tipos WX (Texto, Numérico com casas, Data/Hora/Duração, Memo, Binário, Chave automática), chaves únicas/duplicadas/compostas, ligações com integridade e regras de cardinalidade.

### 3. Dados reais do HFSQL (Classic e Client/Server)

- estado: `falta`
- tamanho: 4 · muito grande
- por que importa: Migração de dados é entrega obrigatória de todo cliente; hoje é manual.
- hoje: Dados de amostra anonimizados, por CSV que o cliente prepara.
- construir: Exportador de dados por arquivo (WDCSV ou WLanguage `HExportCSV`/`HExportJSON`), com anonimização por coluna marcada no dicionário, e importador para o banco de destino com verificação de contagem e soma por tabela.

### 4. Inventário de dependências externas

- estado: `falta`
- tamanho: 3 · grande
- por que importa: Cada uma é uma decisão de conversão que hoje só aparece quando o agente tropeça nela.
- hoje: Não há.
- construir: Listar DLLs, componentes externos (.wdk), WDAPI, COM/ActiveX, webservices SOAP/REST consumidos, impressoras e drivers, e-mail, FTP, arquivos de configuração (.ini/registro).

## 2. Semântica WLanguage

### 5. Transpilador determinístico do subconjunto frequente

- estado: `falta`
- tamanho: 5 · projeto próprio
- por que importa: Agente traduz bem, mas não é reprodutível nem barato em milhares de procedures. Transpilador dá o mesmo resultado duas vezes e custa zero token.
- hoje: A conversão é feita pelos agentes, função a função, com o Help 12k como referência e a WL_C# no perfil C#.
- construir: Analisador sintático do WLanguage (expressões, tipos, procedures, classes, estruturas, `FOR EACH`, `SWITCH`, `IF … THEN`, indirection, `..` de propriedades) e gerador para o perfil escolhido, cobrindo o núcleo; o que não cobre vai para o agente com o localizador. Meta medida: cobrir 80% das linhas de um projeto real pelo transpilador.

### 6. Tabela de equivalência de funções por perfil

- estado: `falta`
- tamanho: 4 · muito grande
- por que importa: É o que faz a conversão ser igual entre agentes e entre sessões.
- hoje: Só o perfil C# tem a WL_C# (480 funções). Rust, Go, Python e os outros têm o Help por tema, sem tabela.
- construir: Para cada perfil, uma tabela função WLanguage → equivalente (biblioteca, chamada, diferença de semântica), gerada a partir dos 12k e revisada por perfil; começando pelas 300 mais usadas (medir num projeto real).

### 7. Semântica dos tipos: datas, numéricos, strings

- estado: `falta`
- tamanho: 4 · muito grande
- por que importa: Diferença de arredondamento ou de collation muda o golden master e ninguém vê antes da produção.
- hoje: Referência textual em `wlanguage-semantics.md`.
- construir: Biblioteca de runtime mínima por perfil: data/hora/duração WX (AAAAMMDD, HHMMSSCC), Numérico com casas fixas, Moeda, comparação de string sem acento e sem caixa como o WX, `Truncate`/`Round` iguais, cadeia com tamanho fixo. Com teste contra o comportamento real do WX.

### 8. Comportamento de tela do WX

- estado: `falta`
- tamanho: 3 · grande
- por que importa: É onde o usuário final percebe que «não é igual».
- hoje: A letra F e o Impeccable cuidam do visual; o comportamento é descrito em texto.
- construir: Catálogo de eventos e comportamentos: inicialização de janela, `Modification`, `Exit`, tabulação, `RETURN`/`ESC`, tabela com edição na célula, combos com preenchimento automático, planos, ancoragens, `..Visible`/`..State`, `Timer`, procedures automáticas.

## 3. Cobertura por produto

### 9. WEBDEV

- estado: `falta`
- tamanho: 4 · muito grande
- por que importa: Metade dos clientes WX tem WEBDEV; sem a separação servidor/navegador a conversão sai errada.
- hoje: Tratado como WINDEV com perfil web; nada específico.
- construir: Páginas com código servidor e navegador separados, `AJAXExecute`, sessões, modelos de página, CSS e folhas de estilo do WEBDEV, `Upload`, cookies, SEO, WEBDEV Cluster e o servidor de aplicação. Mapa página → rota + componente.

### 10. WINDEV Mobile

- estado: `falta`
- tamanho: 4 · muito grande
- por que importa: Sem perfil mobile, o cliente mobile não tem para onde converter.
- hoje: Nada específico.
- construir: Janelas mobile, `Looper`, gestos, câmera, GPS, notificações, sincronização com HFSQL C/S, empacotamento Android/iOS. Perfis de destino mobile (Flutter, React Native, Kotlin/Swift) e o processo por peça.

### 11. Relatórios (.wde) e impressão

- estado: `falta`
- tamanho: 4 · muito grande
- por que importa: Relatório fiscal e contábil é onde a igualdade é obrigatória.
- hoje: Rotas RPT-* na matriz; a conversão é descrita.
- construir: Leitor do relatório (bandas, ruptura, totais, código de banda), gerador para o motor do perfil (relatório em HTML/PDF, JasperReports, QuestPDF, Typst), e o comparador de PDF gerado contra o PDF do legado.

### 12. Queries (.wdr) e HFSQL SQL

- estado: `falta`
- tamanho: 3 · grande
- por que importa: Cada query errada é uma tela errada.
- hoje: Extraídas do PDF de queries.
- construir: Tradutor do SQL do HFSQL (funções próprias, `LIKE` sem acento, parâmetros `{p}`, `TOP`, junções da análise) para o SQL do destino, com teste de resultado idêntico sobre os dados de amostra.

### 13. Groupware, multi-idioma, componentes

- estado: `falta`
- tamanho: 3 · grande
- por que importa: Estão em quase todo ERP WX de porte.
- hoje: Não tratados.
- construir: Groupware usuário (perfis e direitos) → papéis do destino; textos multilíngues dos controles → i18n; componentes internos/externos → módulos.

## 4. Prova de igualdade

### 14. Golden master executável

- estado: `parcial`
- tamanho: 5 · projeto próprio
- por que importa: Sem baseline executável a igualdade é declarada, não provada.
- hoje: Compara saídas fornecidas em CSV; classe FORENSIC quando não há baseline executável.
- construir: Executor do legado: rodar o EXE/serviço WX com os dados de amostra (na máquina do cliente ou em VM Windows), capturar saída de query, relatório e tela por script WLanguage de automação (`EmulateKey`, `WinRunReport`), e comparar automaticamente com o destino.
- medido: `golden.py comparar` prova igualdade contra baseline em CSV (10/10 no piloto); falta o EXECUTOR do legado que produz o baseline sozinho

### 15. Projeto real de ponta a ponta

- estado: `falta`
- tamanho: 5 · projeto próprio
- por que importa: Tudo acima é hipótese até esse número existir.
- hoje: Nenhum projeto WINDEV real passou pelos gates G1 a G7; o exemplo é sintético.
- construir: Um cliente piloto, com contrato, medido: horas, tokens, GAP-* abertas, defeitos achados em homologação. O resultado vira o `DESEMPENHO.md` do plugin.

### 16. Testes do destino gerados da matriz

- estado: `parcial`
- tamanho: 3 · grande
- por que importa: A matriz já tem o que provar; falta o teste sair dela.
- hoje: Sete pastas de teste vazias no esqueleto.
- construir: Gerador de teste por BR-* (domínio), por QRY-* (resultado da query), por UI-* (e2e com Playwright), a partir da matriz e dos dados de amostra.
- medido: o piloto vertical da 3.36.0 escreveu 13 testes a mao e passou 10/10 no golden; o GERADOR a partir da matriz continua a faltar

## 5. Operação e escala

### 17. Windows: scripts e instalador

- estado: `feito`
- tamanho: ✓ feito
- por que importa: O cliente WX está no Windows.
- hoje: Resolvido na 3.23.0 e 3.25.0. `instalar.ps1` faz os cinco passos e oferece instalar o que falta, pedindo aprovação. Falta a prova real numa máquina Windows.
- construir: PowerShell equivalente ao instalador, ao exportador e ao zelador; ou tudo em Python puro sem shell.
- medido: `instalar.ps1` faz os cinco passos na 3.23.0/3.25.0; falta a prova numa maquina Windows de verdade

### 18. Custo em tokens por projeto

- estado: `parcial`
- tamanho: 3 · grande
- por que importa: É o preço que o cliente pergunta primeiro.
- hoje: Não medido no questionário inteiro; medido só em hooks e RAG.
- construir: Medir num projeto real: questionário, G0, cada gate, por agente e por modelo; publicar por linha convertida. O laudo de tokens existe, falta rodá-lo de ponta a ponta.
- medido: `/wx-claude-code:laudo-tokens` existe e mede fase a fase; falta rodar de ponta a ponta num projeto real

### 19. Questionário: pausar, retomar, revisar

- estado: `falta`
- tamanho: 2 · médio
- por que importa: Ninguém responde setenta itens numa sessão.
- hoje: Não pausa nem retoma; mais de setenta itens.
- construir: Estado por item no `questionario.json`, `retomar` volta ao último pendente, `revisar <item>` reabre um.

### 20. Licença: segunda camada

- estado: `falta`
- tamanho: 3 · grande
- por que importa: Deixado para depois, a pedido.
- hoje: Serial por hook (dissuasão). Servidor adiado por decisão do dono.
- construir: Servir corpus e agentes de um servidor com o serial; revogação; contagem de projetos.

### 21. Estrangulamento com o legado no ar

- estado: `falta`
- tamanho: 4 · muito grande
- por que importa: É a única estratégia que um ERP em produção aceita.
- hoje: Descrito como estratégia; nada executável.
- construir: Ponte de dados HFSQL ↔ destino (sincronização por tabela, de ida e volta, com conflito registrado) e roteamento por módulo.
