# 495 - hipoteses escritas ANTES de medir (2026-09-24)

H1 detector deterministico no motor, pelo lexico/sintaxe ja existente (tautologia OR 1=1,
   comentario de corte, UNION de sondagem, empilhado, funcao de tempo/sono, leitura de
   catalogo). Registrar por padrao, bloquear so pedido.
   Previsao: detecao alta (>80%) no corpus, FP baixo mas NAO zero no SQL do repo (OR 1=1
   aparece em teste; sys./phxsys. aparece em consulta legitima de admin).
H2 impressao digital (literais normalizados) + lista permitida por usuario/app aprendida
   (MySQL Enterprise Firewall, MaxScale dbfwfilter). Previsao: detecao ~100% de consulta
   nova, FP alto enquanto nao aprendeu (a interface web gera SQL livre - console SQL).
H3 anomalia estatistica por usuario/IP (taxa, volume lido, horario, tabelas). Previsao:
   barato (contador por usuario), mas sem corpus de carga real nao se mede FP -> lacuna.
H4 IA como ANALISTA fora do laco quente: (a) navegador+Claude, (b) Ollama 127.0.0.1,
   (c) nenhuma. Previsao: (a) ja existe o molde; (b) cabe em std (HTTP sem TLS);
   IA como portao morre por latencia (ms-s por consulta contra us).
H5 (minha) parametrizacao/defesa na origem: a propria API do motor (JSON com campos,
   sql.parametros ?) ja torna injecao impossivel para quem usa; o risco e so o SQL cru
   concatenado pelo cliente. Previsao: a maior parte das operacoes (116) nao passa SQL cru.
H6 (minha) detectar pelo resultado da analise (AST) e nao pelo texto: literal-string
   contendo ' OR ... ja foi separado pelo analisador; a tautologia so e tautologia se o
   analisador a ve como expressao. Previsao: detector pos-analise tem FP menor que regex.
H7 (minha) o detector ja existente (215, comando_empilhado) roda so em pedido recusado ->
   nao ve a injecao que deu certo. Previsao: confirmar no codigo.
