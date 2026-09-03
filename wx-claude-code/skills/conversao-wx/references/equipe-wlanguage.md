# Equipe WLanguage: especialistas por tema do Help

O corpus `Help_WL_12k_Json.zip` tem 45 temas indexados em
`00_indice_de_grupos.json`, e o nome de cada página começa pelo código do tema
(`GG-SS-TT`). A equipe WLanguage é dividida por esses códigos: cada
subagente só consulta a fatia dele, com `--group`, e por isso lê menos páginas,
responde mais rápido (medido: 5,4 s → 0,5 s numa busca restrita ao HFSQL) e
não opina fora do que o Help cobre.

## Os especialistas

| Subagente | Temas (código → páginas) | Cobre |
| --- | --- | --- |
| `wl-hfsql-specialist` | `01-03-01` (101), `01-03-02` (165), `01-03-03` (978), `01-03-04` (8), `10-01-01` (60), `12-01-01` (45) | HFSQL, Big Data, conectores nativos, administração do banco |
| `wl-ui-controls-specialist` | `01-04-02` (2530), `02-03-01` (891), `02-03-02` (17), `02-04-01` (17), `13-01-01` (34) | controles, janelas, páginas, estilos, AAF |
| `wl-communication-specialist` | `01-04-01` (1178), `17-01-01` (13) | e-mail, HTTP, REST, SOAP, soquete, FTP, webservices |
| `wl-standard-functions-specialist` | `01-04-04` (2390), `01-05-01` (733), `01-06-01` (158), `01-02-01` (33), `07-01-01` (112) | funções padrão, propriedades, sintaxe, funções C |
| `wl-mobile-specialist` | `01-04-03` (246), `15-01-01` (25) | Android, iOS, permissões, câmera, GPS, push, lojas |
| `wl-web-specialist` | `01-04-05` (286), `02-05-01` (47), `05-01-01` (165), `05-02-01` (5) | WEBDEV, sessão, navegador × servidor, administrador WEBDEV |
| `wl-errors-specialist` | `01-01-01` (123), `03-01-01` (557) | erros do compilador e do runtime, páginas sem trilha |

Os temas restantes (editores, instalação, ferramentas, tutoriais, licenças)
ficam com o `help-indexer`, que é mecânico e não interpreta.

## Como o `wlanguage-specialist` usa a equipe

1. Recebe um símbolo ou trecho WLanguage com evidência (PDF + página).
2. Classifica o símbolo pelo prefixo e pelo contexto: `H*` → HFSQL; `HTTP*`,
   `Email*`, `Socket*`, `SOAP*`, `REST*` → comunicação; `Control*`, `Window*`,
   `Page*`, `Table*` → UI; `Mobile*`, `Camera*`, `GPS*`, `Notif*` → mobile;
   `Server*`, `Browser*`, `Session*`, `Cookie*` → web; código de erro → erros;
   o resto → funções padrão.
3. Delega ao especialista do tema com a consulta já montada:

```bash
python3 "${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/scripts/query_wlanguage_help.py" \
  --query HReadSeekFirst --group 01-03-03 --version 2025 --limit 5
```

4. O especialista devolve **semântica** (assinatura, parâmetros, retorno,
   efeitos, diferenças por versão e plataforma) com o localizador da página
   (`member` + `member_sha256`) e propõe a equivalência na linguagem de destino
   marcada `equivalente | adaptar | substituir | encapsular`.
5. Símbolo que cai na lacuna ou na quarentena do corpus volta como `GAP-*`,
   nunca como palpite.

## Regras

- O Help é semântica técnica. Regra de negócio vem do código do projeto e do
  responsável de negócio, nunca do Help.
- Uma consulta por símbolo, `--limit` pequeno, e o retorno é trecho com
  localizador, não a página inteira: é aqui que o custo em tokens se controla.
- Símbolo ambíguo entre dois temas é consultado nos dois, e o especialista
  diz qual página respondeu.
- Versão do WX (registrada no fechamento do questionário, `projeto.wx_versao`) entra sempre em `--version`;
  divergência entre versões é achado, não ruído.
