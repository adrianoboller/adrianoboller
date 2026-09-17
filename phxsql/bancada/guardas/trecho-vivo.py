#!/usr/bin/env python3
"""A catraca do catalogo envelhecido -- pedido 263.

    python3 bancada/guardas/trecho-vivo.py --catraca
    python3 bancada/guardas/trecho-vivo.py --numeros
    python3 bancada/guardas/trecho-vivo.py --autoteste

# O defeito que motivou

Em 16/09/2026 o `provar-guardas.py` correu inteiro e devolveu **11 guardas
QUEBRADAS** -- nao reprovadas, QUEBRADAS: o `trecho` que a entrada manda
substituir para repor o defeito **nao existe mais** no arquivo, entao a
guarda nao pode nem ser tentada. Guarda que existe e nao guarda e pior que
guarda faltando, porque o catalogo a conta como cobertura.

Medido commit a commit depois: **um unico commit aposentou cinco delas de
uma vez** -- `2fe8658` (12/09, «a conferencia de FK dentro da transacao ve o
pai empilhado»), que mexeu em `table.rs` e `transacao.rs`. Ninguem percebeu
por QUATRO DIAS, e o motivo e o custo: o provador leva cerca de uma hora,
porque repoe o defeito e roda `cargo test` para cada uma das 143 entradas.
Guarda que so se confere em uma hora e guarda que nao se confere.

# As CINCO formas de QUEBRADA, e quais desta regua pega

O `provar-guardas.py` devolve QUEBRADA por cinco motivos diferentes. Esta
regua nasceu (16/09, pedido 263) vendo **uma** delas, e o numero que isso
custou esta medido: a catraca dizia `ok 0` no mesmo dia em que o provador
dizia **1 QUEBRADA** (`trava-sem-guarda-de-reentrancia`, que estoura o prazo
de 420 s). A regua estava certa no que prometia -- e quem lesse o `ok 0` como
INVENTARIO concluiria que o catalogo estava inteiro. Lei que lista menos
casos do que existem protege igual hoje e menos no dia em que alguem usar a
lista como inventario.

O criterio de quem entra e um so: **da para ver sem compilar e sem rodar?**

| forma de QUEBRADA | onde ela nasce no provador | aqui? |
|---|---|---|
| o arquivo/o trecho nao esta mais la | `Arvore.repor`, `quantas == 0` | SIM -- `TETO_TRECHO_MORTO` |
| o trecho aparece DUAS vezes | `Arvore.repor`, `quantas > 1` | SIM -- `TETO_TRECHO_AMBIGUO` |
| o teste nomeado nao existe mais | `julgar`, `sumidos` | SIM -- `TETO_TESTE_MORTO` |
| o teste existe, mas nao no binario | laco principal, `faltando` | SIM -- `TETO_TESTE_FORA_DO_BINARIO` |
| o codigo trocado NAO COMPILA | `julgar`, `desfecho == "nao compilou"` | **NAO** |
| a rodada estourou o prazo | `julgar`, `desfecho == "prazo"` | **NAO** |
| o binario abortou quando nao devia | `julgar`, `desfecho == "aborta"` | **NAO** |

As tres de baixo **ficam de fora, e ficam de fora por definicao**: as tres
so existem depois de `cargo test` compilar o `troca` e RODAR o binario. Ver
o `troca` compilar custa uma compilacao por entrada -- que e exatamente a
hora do provador que esta regua existe para nao esperar --, e ver o prazo e
o aborto custa a rodada inteira. Uma regua de 0,2 s nao pode te-las, e
fingir que as tem seria a mentira que este cabecalho acabou de nomear.

Entao esta regua **diz que nao as tem**, em toda corrida, na linha
«o provador continua dono de: …» que o `--catraca` imprime no fim. Nao e
prosa: e o inventario do que este numero NAO cobre, impresso junto do
numero, para que ninguem precise vir ler este arquivo para saber.

# O que esta regua faz, e o que ela NAO faz

FAZ, em **0,200-0,206 s** (medido, cinco corridas a load ~1,0) e sem
compilar nada: as quatro perguntas de texto puro da tabela acima, mais o
**piso** do catalogo (abaixo). A versao de uma regua so custava
**0,171-0,183 s** no mesmo minuto e na mesma maquina -- duas perguntas a
mais por 0,03 s. **Regua cara e regua que nao se roda**, e esta roda em toda
bateria: o custo foi medido a cada passo, e nao no fim.

NAO FAZ, e isto importa: ela **nao substitui o provador**. Achar o trecho
nao prova que repo-lo derruba o teste -- so o provador prova isso, e
continua sendo ele a autoridade. Esta regua e o aviso barato, que pega a
classe de envelhecimento que custou os quatro dias: o codigo andou e a
entrada ficou para tras.

# O piso: a catraca nao distingue conserto de APAGAMENTO

Buraco medido em 16/09/2026, depois de as oito entradas velhas serem
consertadas: `TETO_TRECHO_MORTO` conta **trecho morto**, nao **guarda
viva**. Apagar as oito entradas do `catalogo.py` teria medido exatamente o
mesmo `0` que conserta-las -- e apagar e o caminho barato. Uma catraca que
premia o apagamento igual ao conserto nao segura o catalogo; segura so a
aparencia dele.

O `PISO_DAS_ENTRADAS` fecha esse lado. Ele nao conta as entradas vivas: ele
conta **vivas + aposentadas escritas**. A diferenca e a armadilha que um
piso rigido teria -- um piso que so olhasse as vivas impediria aposentar uma
guarda cuja logica deixou de existir, e guarda impossivel de aposentar vira
entrada remendada no chute, que e pior que entrada nenhuma.

A saida e a mesma lei da catraca que muda de regua: **a aposentadoria se
ESCREVE.** Quem tira uma entrada poe uma linha em `APOSENTADAS` com o id, a
data e o motivo, e a soma nao se mexe; quem apaga uma entrada em silencio
faz a soma cair, e o piso reprova nomeando quantas sumiram. O piso muda o
preco relativo dos dois caminhos: consertar continua custando ler o codigo,
e apagar passa a custar escrever por que.

E ele sobe junto, pelo mesmo motivo que o teto desce junto: catalogo que
cresceu e piso parado e piso frouxo -- ele voltaria a aceitar o apagamento
das entradas novas. Crescer reprova pedindo o numero novo no MESMO commit,
que e o espelho exato do «DESCEU -- BAIXE O TETO».

# A armadilha que esta medicao ja pagou

A primeira versao varria so `crates/<pacote>/src/` atras dos testes e
acusou **154** nomes mortos. Eram 154 falsos: o teste de integracao mora em
`crates/<pacote>/tests/`, nao em `src/`. Regua que mede um terco da caixa e
anuncia o numero inteiro e a mesma lei do KiB da interface -- **quando um
gerador depende de uma lista, a lista tem de sair do codigo**. Hoje a
varredura e `crates/**/*.rs` inteiro, e o numero medido e zero.

# As catracas

So DESCEM (as de teto) e so SOBEM (a de piso). Nascem no numero MEDIDO do
dia, nunca no desejado:

- `TETO_TRECHO_MORTO = 0` -- **DESCEU de 8 para 0 em 16/09/2026**, no mesmo
  passo em que a divida velha foi paga. As oito entradas nomeadas no pedido
  263 tiveram o ponto de reposicao reencontrado no codigo de hoje e foram
  PROVADAS pelo provador (7 PROVADA + 1 REDUNDANTE -- o
  `recuperar-sem-reindexar`, que declara `espera: "nada muda"` e continua
  sendo pego so pela prova por soquete; 0 nao pegaram, 0 estragaram, 0
  quebradas). Nao se consertou por varredura: cada entrada pediu ler o
  codigo de hoje, achar para onde o ponto de reposicao andou, e provar. De
  oito, cinco tinham so MUDADO DE INDENTACAO (`2fe8658` desaninhou laco e
  extraiu ajudante), uma mudou de ARQUIVO (`d59967a` levou a formula do
  `profiler.rs` para o `rodizio.rs`) e duas ganharam um braco novo no mesmo
  ponto (`7d29f5f` e `2d33c5c`). Entrada consertada no chute produz guarda
  que passa por engano -- pior que a quebrada, e por isso o veredito acima
  e do provador e nao desta regua.
- `TETO_TESTE_MORTO = 0` -- medido depois do conserto de
  `leitura-sem-recuo-para-a-exclusiva`, cuja entrada nomeava
  `so_uma_operacao_usa_a_ficha_compartilhada`, renomeado em `f2b87aa` para
  `so_as_duas_operacoes_medidas_usam_a_ficha_compartilhada`.
- `TETO_TRECHO_AMBIGUO = 0` -- **nasce em 16/09/2026**, medido. E a segunda
  forma de QUEBRADA, e ela e barata: `texto.count(trecho) > 1`. Um trecho
  que casa duas vezes nao e so uma entrada que nao roda -- e uma entrada que
  provaria OUTRA COISA se o executor escolhesse a ocorrencia errada, e por
  isso o provador recusa antes de tentar.
- `TETO_TESTE_FORA_DO_BINARIO = 0` -- **nasce em 16/09/2026**, medido. E o
  `faltando` do laco principal do provador: o nome do teste existe em algum
  lugar de `crates/`, entao o `TETO_TESTE_MORTO` o da por vivo, mas ele nao
  esta no binario que a entrada nomeia (`pacote` + `alvo`) -- e o provador
  nunca o veria. Conta so o que o `TETO_TESTE_MORTO` ja nao contou: as duas
  reguas nao se sobrepoem, senao um nome sumido subiria dois numeros e
  pareceria dois defeitos.

# E a catraca que sobe, porque e piso e nao teto

- `PISO_DAS_ENTRADAS = 170` -- nasceu em 16/09/2026 valendo 143, contado no
  `catalogo.py` daquele dia (143 entradas, 143 ids distintos) mais as
  `APOSENTADAS` (hoje nenhuma). **SUBIU para 150 em 16/09/2026**, no mesmo
  passo em que a frente 245 (O2-O6) escreveu cinco guardas novas -- o teto de
  64 bits que saturava, a saida do direito por coluna, o CHECK que se
  contradiz no `acrescentar_coluna`, o ALTER com regra sem aviso e o upsert
  parcial que viraria mescla. Piso parado com catalogo que cresceu volta a
  aceitar o apagamento das entradas novas, que e o que ele existe para
  **SUBIU de novo para 160 em 16/09/2026**, na frente G-CRIPTO: nove guardas
  novas -- cinco da petrea «criptografia se confere contra vetor oficial»,
  que ate aqui nao tinha entrada NENHUMA neste catalogo, e tres da petrea do
  portao de permissao -- `juntar`, `unir` e `diferencas`, as operacoes que
  escondem a tabela do campo que o portao le, mais o `derivado-sem-portao`,
  que sozinho prova OITO provas de porta dos fundos.
  **SUBIU de novo para 169 em 16/09/2026**, na frente G-SENHA: nove guardas
  novas da petrea «senha nunca em texto puro», que ate aqui tinha SEIS provas
  no `caem` de alguma entrada -- todas as seis do Profiler -- contra 40
  funcoes de teste medidas na arvore que afirmam que um segredo nao aparece
  numa saida. As nove cobrem as quatro saidas que a petrea nomeia (arquivo,
  log, resposta do protocolo e a ficha) mais o `Debug`, que a frase nao nomeia
  e que nenhum teste de `--lib` alcanca. Ele e o UNICO numero desta
  regua que sobe, e sobe porque conta ENTRADAS e nao defeitos: os quatro
  tetos acima continuam em zero, e nenhum deles foi tocado.
  impedir. Ele e a unica coisa nesta regua que reprova o APAGAMENTO; tudo o
  mais aqui reprova o envelhecimento.

# A QUINTA REGUA: a tabela publicada pode ser MENOR que o catalogo

Pedido 269. As quatro de cima olham o **codigo** contra a entrada; esta olha a
**entrada** contra a ULTIMA CORRIDA -- e e' a quinta forma de o catalogo
envelhecer, a unica que nenhuma delas ve. Uma entrada pode ter trecho vivo,
teste vivo, teste no binario certo e mesmo assim **nunca ter sido julgada**:
basta ela ter entrado depois da ultima corrida do provador.

Medido em 16/09/2026: o catalogo tinha 160 entradas e a tabela publicada em
`docs/TESTES.md` dizia «143 guardas». A pagina era honesta sobre a DATA (traz
o `medido em`) e **muda sobre o TAMANHO** -- quem a lesse como inventario a
leria 17 entradas curta.

## Por que o teto conta o ESCONDIDO e nao o buraco

O pedido pedia o buraco cru -- «quantos ids do catalogo nao estao na
`ultima-corrida.json`» --, nascendo em 17 e so descendo. Esse numero foi
medido, e foi medido **duas vezes no mesmo serao**: 17 as 21h e **26** as 23h,
porque uma frente vizinha escreveu nove guardas novas da petrea «senha nunca
em texto puro» nesse intervalo. Nao houve defeito nenhum entre as duas
medicoes: houve trabalho certo.

E' isso que decide a forma. Um teto sobre o buraco cru fica VERMELHO toda vez
que alguem escreve uma guarda nova, e os dois caminhos para reverde-lo sao
rodar o provador inteiro (~3.374 s de mutacao mais a compilacao) ou SUBIR o
teto -- que esta casa proibe. Catraca cujo unico caminho verde custa uma hora
e catraca que se pula, e catraca pulada e catraca frouxa. Pior: ela cobraria
o preco de quem ESCREVE a guarda e nao cobraria nada de quem nao escreve --
o espelho exato da doenca que o `PISO_DAS_ENTRADAS` existe para curar.

O buraco nao e o defeito; o buraco e a consequencia aceita de um provador que
custa uma hora. O defeito e a pagina ficar **muda** sobre ele. Entao o teto
conta as entradas que a ultima corrida nao julgou **e** que a tabela publicada
nao nomeia -- e essa divida se paga em 0,2 s, republicando a mesma corrida
pelo `tabela-no-testes.py`. Sem prova nova, sem data nova, sem numero novo:
so a pagina passando a dizer o proprio tamanho.

O buraco cru continua MEDIDO e IMPRESSO, na linha «a ultima corrida julgou N
de M» que o `--catraca` escreve sempre. Ele nao vira teto; vira inventario, do
mesmo jeito que o «o provador continua dono de:» daqui de cima. Numero que nao
se pode zerar honestamente nao vira catraca -- vira numero visivel.

## Catraca, e nao parada com o motivo -- e onde ela vira parada

O `conferir_o_formato()` e uma parada com o motivo porque forma errada impede
a regua de MEDIR: uma entrada sem `id` nao produz um numero ruim, produz
numero nenhum. Aqui ha um numero, ele e uma divida real (entradas escondidas),
e ele tem um dono e um conserto conhecido -- entao e catraca, e entra no
inventario de catracas do `docs/qa/medir.py` junto das outras quatro, que e
onde esta casa guarda o que segura.

Mas a regua **nao sabe medir** quando falta a fonte: sem `ultima-corrida.json`
nao ha corrida para comparar, e sem as marcas `guardas:inicio/fim` no
`docs/TESTES.md` nao da para saber o que a pagina nomeia. Nesses dois casos
ela reprova com o motivo escrito **e conta o pior caso** -- pagina ilegivel
nomeia zero, corrida ausente julgou zero. Regua que nao sabe tem de dizer que
nao sabe; o que ela nao pode e' devolver `0` e parecer um catalogo inteiro.

- `TETO_NAO_JULGADA_ESCONDIDA = 0` -- **nasceu medido em 26 e DESCEU para 0 no
  mesmo passo** (16/09/2026), como o `TETO_TRECHO_MORTO` nasceu em 8 e desceu
  a 0 quando a divida velha foi paga: aqui o conserto foi o
  `tabela-no-testes.py` republicar a corrida de 15:25 ja nomeando as 26 que
  ela nao julgou -- a mesma medida, a mesma data, so a pagina dizendo o
  proprio tamanho. Enquanto este teto estiver em 0, a tabela publicada pode
  ser menor que o catalogo -- mas nao pode ESCONDER que e'.
"""
import importlib.util
import json
import os
import re
import sys

AQUI = os.path.dirname(os.path.abspath(__file__))
RAIZ = os.path.abspath(os.path.join(AQUI, "..", ".."))
EU = os.path.relpath(os.path.abspath(__file__), RAIZ)

TETO_TRECHO_MORTO = 0
TETO_TESTE_MORTO = 0
TETO_TRECHO_AMBIGUO = 0
TETO_TESTE_FORA_DO_BINARIO = 0
PISO_DAS_ENTRADAS = 170

# ------------------------------------------------------------- APOSENTADAS
#
# Cada guarda que SAIU do catalogo de proposito, com o id, a data e o motivo.
# A soma `len(catalogo) + len(APOSENTADAS)` e o que o `PISO_DAS_ENTRADAS`
# cobra -- entao tirar uma entrada sem escrever a linha aqui REPROVA, e
# escrever a linha e a diferenca entre aposentar e apagar.
#
# Aposentar e legitimo, e por isso esta lista existe: guarda cuja logica
# deixou de existir no produto nao tem defeito para repor, e manter a entrada
# so para o numero fechar produziria justamente a entrada remendada no chute
# que esta casa trata como pior que a quebrada.
#
# O que NAO e aposentadoria: entrada que envelheceu. Essa se conserta -- se
# acha para onde o ponto de reposicao andou e se prova com o provador. Quem
# aposenta uma entrada envelhecida escreve aqui um motivo que o proximo
# leitor consegue conferir, e e' por isso que o motivo e obrigatorio.
#
# Formato: {"id": …, "data": "DD/MM/AAAA", "motivo": …}
APOSENTADAS = []


# ------------------------------------------- A QUINTA REGUA (pedido 269)
#
# Longe do bloco dos quatro tetos de proposito: o valor do `PISO_DAS_ENTRADAS`
# muda toda vez que o catalogo cresce, e duas frentes editando linhas coladas
# e conflito de merge onde nao havia desacordo nenhum. O motivo e a decisao
# de forma estao no fim do cabecalho deste arquivo.
TETO_NAO_JULGADA_ESCONDIDA = 0

CORRIDA = os.path.join(AQUI, "ultima-corrida.json")
TABELA = os.path.join(RAIZ, "docs", "TESTES.md")


_CATALOGO = None
_GERADOR = None
_ESCONDIDAS = None


def catalogo():
    """Le o catalogo pelo proprio modulo, nunca por copia da lista.

    Receita duplicada e receita que diverge: uma segunda lista de guardas
    aqui envelheceria sozinha, e a regua passaria a medir um catalogo que
    nao e o que o provador roda.

    Guardado: o `exec_module` custa o mesmo que ler o arquivo e compilar, e
    esta regua o pedia cinco vezes por corrida."""
    global _CATALOGO
    if _CATALOGO is not None:
        return _CATALOGO
    caminho = os.path.join(AQUI, "catalogo.py")
    spec = importlib.util.spec_from_file_location("catalogo_das_guardas", caminho)
    modulo = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(modulo)
    _CATALOGO = getattr(modulo, "CATALOGO", None) or getattr(modulo, "GUARDAS")
    conferir_o_formato(_CATALOGO)
    return _CATALOGO


# As chaves que o catalogo usa hoje, contadas nele e nao lembradas.
# OBRIGATORIAS em toda entrada; o par trecho/troca e o `trocas` sao as duas
# FORMAS alternativas, conferidas separado logo abaixo.
CHAVES_OBRIGATORIAS = {"id", "titulo", "porque", "pacote", "alvo", "caem"}
CHAVES_OPCIONAIS = {
    "seguem",              # os vizinhos que tem de seguir verdes
    "arquivo", "trecho", "troca",   # a forma comum: um trecho so
    "trocas",              # a forma de varias trocas, cada item com o seu par
    "prazo",               # segundos, quando a suite do pacote e lenta
    "espera",              # a espera propria da entrada
    "nota_da_redundancia", # por que esta guarda pode sair REDUNDANTE
}


def conferir_o_formato(guardas):
    """Parada com o motivo quando uma entrada nao tem a FORMA do catalogo.

    Nasceu em 16/09/2026, e nasceu de um erro desta casa: duas entradas
    entraram escritas com `nome`/`defeito`/`pedido`/`petrea` -- chaves que
    ninguem le -- em vez de `id`/`titulo`/`porque`. Esta regua deu **ok nos
    quatro tetos** e no piso, porque ela conferia o CONTEUDO das chaves que
    conhece (`arquivo`, `trecho`, `caem`) e nunca a FORMA da entrada. Quem
    acusou foi o provador, com um `KeyError: 'id'` depois de lancado.

    E o alcance da lei «chave morta e pior que chave faltando»: a chave morta
    aqui nao era da tela nem da traducao, era do proprio catalogo -- e a regua
    que existe para o catalogo nao envelhecer nao olhava para ela.

    Parada, e nao teto: teto conta quantos: aqui nao ha «quantos» aceitavel.
    E o mesmo criterio do `dossie_da_pasta.py`, que para com o motivo quando
    acha zero ou dois dossies em vez de chutar qual atualizar."""
    conhecidas = CHAVES_OBRIGATORIAS | CHAVES_OPCIONAIS
    problemas = []
    for i, g in enumerate(guardas):
        quem = g.get("id") or g.get("nome") or f"entrada #{i}"
        falta = CHAVES_OBRIGATORIAS - set(g)
        if falta:
            problemas.append(f"{quem}: faltam {sorted(falta)}")
        sobra = set(g) - conhecidas
        if sobra:
            problemas.append(f"{quem}: chaves que ninguem le {sorted(sobra)}")
        # as duas formas: ou o par comum, ou a lista de trocas -- nunca nenhuma
        comum = {"arquivo", "trecho", "troca"} <= set(g)
        if not comum and not g.get("trocas"):
            problemas.append(
                f"{quem}: nem o par arquivo/trecho/troca nem `trocas`")
    if problemas:
        raise SystemExit(
            "catalogo.py: entrada fora do formato -- o provador nao carrega "
            "isto.\n   " + "\n   ".join(problemas))


def pares(guarda):
    """Cada (arquivo, trecho) da guarda -- o catalogo tem duas formas.

    A comum traz `arquivo`/`trecho` no topo; a de varias trocas traz uma
    lista em `trocas`, e cada item dela tem o seu proprio par."""
    if guarda.get("trocas"):
        return [(t["arquivo"], t["trecho"]) for t in guarda["trocas"]]
    return [(guarda["arquivo"], guarda["trecho"])]


FUNCAO = re.compile(r"\bfn\s+([A-Za-z_][A-Za-z0-9_]*)\s*\(")
MODULO = re.compile(r"^\s*(?:pub\s+)?mod\s+([a-z_][a-z0-9_]*)\s*;", re.M)


_FONTES = None


def fontes():
    """UMA passagem por `crates/**/*.rs`: por arquivo, as `fn` e os `mod x;`.

    `src/` guarda o teste de modulo e `tests/` o de integracao: varrer so um
    dos dois mede parte da caixa e anuncia o numero inteiro.

    A passagem unica e o ponto, e ela ja foi paga duas vezes. A primeira
    versao guardava o fonte inteiro numa string e corria uma busca por nome
    citado -- mais de quatrocentas buscas varrendo os mesmos megabytes,
    **31,6 s** medidos. A segunda (16/09, esta) acrescentou a pergunta «em
    qual binario esse teste mora?» e, escrita do jeito obvio, relia
    `crates/<pacote>/src/` uma vez por pacote: **0,47 s**. Os tres consertos
    de custo, medidos um a um em vez de no fim:

      0,47 s  relendo o disco por pacote
      0,32 s  uma passagem so, guardada por arquivo (esta funcao)
      0,29 s  o `catalogo.py` guardado e o `achados()` uma vez, nao duas
      0,20 s  o crivo de `mod x;` so nos 57 arquivos de `tests/`, dos 276

    A regua de antes, com duas perguntas a menos, custava 0,18 s."""
    global _FONTES
    if _FONTES is not None:
        return _FONTES
    _FONTES = {}
    for dirpath, dirs, arquivos in os.walk(os.path.join(RAIZ, "crates")):
        dirs[:] = [d for d in dirs if d not in ("target", ".git")]
        for nome in sorted(arquivos):
            if not nome.endswith(".rs"):
                continue
            caminho = os.path.join(dirpath, nome)
            try:
                with open(caminho, encoding="utf-8", errors="replace") as f:
                    texto = f.read()
            except OSError:
                continue
            # O `mod x;` so serve para resolver um alvo `--test`, e alvo de
            # teste mora em `crates/<pacote>/tests/`: 57 arquivos dos 276.
            # Correr o segundo crivo nos 276 custa parede a toa -- medido,
            # 0,29 s contra 0,20 s na mesma maquina e no mesmo minuto.
            mods = (MODULO.findall(texto)
                    if (os.sep + "tests" + os.sep) in caminho else [])
            _FONTES[caminho] = (set(FUNCAO.findall(texto)), mods)
    return _FONTES


def nomes_de_funcao():
    """Toda `fn` declarada no Rust versionado, num conjunto."""
    achados_ = set()
    for fns, _mods in fontes().values():
        achados_ |= fns
    return achados_


_BINARIOS = {}


def fns_do_binario(pacote, alvo):
    """As `fn` que o binario de teste NOMEADO pela entrada enxerga.

    O provador roda `cargo test -p <pacote> <alvo>` e so ve os testes
    daquele binario: um nome que exista noutro pacote da `QUEBRADA` com
    «teste que o catalogo nomeia e o binario nao tem», e o
    `TETO_TESTE_MORTO` -- que pergunta por `crates/**` inteiro -- nao o
    enxerga.

    `--lib` e `crates/<pacote>/src/**`; `--test <nome>` e
    `crates/<pacote>/tests/<nome>.rs` mais os `mod x;` que ele declara (o
    `mod comum;` dos testes de integracao desta casa), resolvidos como o
    compilador resolve: `x.rs` ou `x/mod.rs`. Nao ha `#[path = …]` nesta
    arvore -- conferido; se um dia houver, esta funcao passa a medir menos
    do que existe e o comentario tem de mudar junto."""
    chave = (pacote, tuple(alvo))
    if chave in _BINARIOS:
        return _BINARIOS[chave]
    lidas = fontes()
    encontradas = set()
    if alvo and alvo[0] == "--lib":
        base = os.path.join(RAIZ, "crates", pacote, "src") + os.sep
        for caminho, (fns, _mods) in lidas.items():
            if caminho.startswith(base):
                encontradas |= fns
    elif len(alvo) >= 2 and alvo[0] == "--test":
        pendentes = [os.path.join(RAIZ, "crates", pacote, "tests",
                                  alvo[1] + ".rs")]
        vistos = set()
        while pendentes:
            caminho = pendentes.pop()
            if caminho in vistos or caminho not in lidas:
                continue
            vistos.add(caminho)
            fns, mods = lidas[caminho]
            encontradas |= fns
            pasta = os.path.dirname(caminho)
            for mod in mods:
                for candidato in (os.path.join(pasta, mod + ".rs"),
                                  os.path.join(pasta, mod, "mod.rs")):
                    if candidato in lidas:
                        pendentes.append(candidato)
    else:
        # Alvo que esta regua nao sabe resolver (`--bin`, `--bins`, …).
        # Devolver o conjunto VAZIO acusaria todos os testes dela como fora
        # do binario -- regua que nao sabe tem de dizer que nao sabe, e nao
        # inventar um veredito. `None` faz o chamador pular a pergunta.
        return None
    _BINARIOS[chave] = encontradas
    return encontradas


def achados():
    """Os quatro achados de texto puro, cada um com a guarda que o nomeia.

    Devolve (trechos mortos, trechos ambiguos, testes mortos, testes fora do
    binario). As duas listas de teste sao DISJUNTAS de proposito: um nome que
    nao existe em lugar nenhum entra so na primeira, senao um renomear
    subiria dois numeros e pareceria dois defeitos."""
    lido = {}

    def ler(rel):
        if rel not in lido:
            caminho = os.path.join(RAIZ, rel)
            try:
                with open(caminho, encoding="utf-8", errors="replace") as f:
                    lido[rel] = f.read()
            except OSError:
                lido[rel] = None
        return lido[rel]

    declaradas = nomes_de_funcao()
    trechos, ambiguos, testes, fora = [], [], [], []
    for g in catalogo():
        gid = g.get("id")
        for arq, trecho in pares(g):
            texto = ler(arq)
            if texto is None:
                trechos.append((gid, arq, "o arquivo nao existe mais"))
                continue
            quantas = texto.count(trecho)
            if quantas == 0:
                trechos.append((gid, arq, "o trecho nao esta mais la"))
            elif quantas > 1:
                ambiguos.append((gid, arq, "o trecho aparece %d vezes -- trocar "
                                           "a errada provaria outra coisa" % quantas))
        no_binario = fns_do_binario(g.get("pacote", ""), g.get("alvo") or [])
        for campo in ("caem", "seguem"):
            for teste in g.get(campo) or []:
                curto = teste.split("::")[-1]
                if curto not in declaradas:
                    testes.append((gid, campo, teste))
                elif no_binario is not None and curto not in no_binario:
                    fora.append((gid, campo, "%s -- nao esta em %s %s"
                                 % (teste, g.get("pacote"),
                                    " ".join(g.get("alvo") or []))))
    return trechos, ambiguos, testes, fora


def gerador():
    """O modulo do `tabela-no-testes.py` -- a regua e a pagina pela mesma
    receita.

    Importa em vez de reescrever a subtracao de conjunto: duas receitas da
    mesma pergunta divergem na primeira vez que uma delas aprender algo, e
    divergem em SILENCIO -- uma diria que a pagina esta inteira enquanto a
    outra publica que nao esta. As marcas do bloco vem pelo mesmo caminho e
    pelo mesmo motivo: quem escreve o bloco e quem o le tem de concordar sobre
    onde ele comeca.

    Custa 1,3 ms medidos (o `catalogo.py` entra uma segunda vez, sob o nome
    que o gerador usa) numa regua de ~190 ms. Regua cara e regua que nao se
    roda, e esta roda em toda bateria -- por isso o custo foi medido antes de
    o import entrar, e nao depois."""
    global _GERADOR
    if _GERADOR is None:
        caminho = os.path.join(AQUI, "tabela-no-testes.py")
        spec = importlib.util.spec_from_file_location("tabela_no_testes", caminho)
        modulo = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(modulo)
        _GERADOR = modulo
    return _GERADOR


def escondidas():
    """O que a ultima corrida nao julgou, e o que disso a pagina nao nomeia.

    Duas perguntas e nao uma, e a diferenca entre elas e a regua inteira:

    * **nao julgadas** e o buraco -- entradas sem veredito na corrida
      publicada. Ele cresce toda vez que alguem escreve uma guarda nova, e
      isso e trabalho certo. Ele e' IMPRESSO, nunca travado.
    * **escondidas** e a divida -- as nao julgadas que a tabela publicada
      nem nomeia. Essa se paga em 0,2 s republicando a mesma corrida, e e' ela
      que o teto trava.

    Sem a corrida ou sem as marcas da pagina a regua nao sabe medir, e entao
    conta o PIOR CASO e diz por que: corrida ausente julgou zero, pagina
    ilegivel nomeia zero. O que ela nao pode e' devolver `0` calada e parecer
    um catalogo inteiro."""
    global _ESCONDIDAS
    if _ESCONDIDAS is not None:
        return _ESCONDIDAS
    ferramenta = gerador()
    avisos = []
    try:
        with open(CORRIDA, encoding="utf-8") as f:
            corrida = json.load(f)
    except (OSError, ValueError) as e:
        avisos.append("%s nao deu para ler (%s): sem corrida nao ha com que "
                      "comparar, e esta regua conta o pior caso -- NENHUMA "
                      "entrada julgada." % (os.path.relpath(CORRIDA, RAIZ), e))
        corrida = {"guardas": []}
    faltam = ferramenta.nao_julgadas(catalogo(), corrida)
    try:
        with open(TABELA, encoding="utf-8") as f:
            texto = f.read()
    except OSError as e:
        avisos.append("%s nao deu para ler (%s): a regua conta o pior caso -- "
                      "a pagina nomeia NENHUMA."
                      % (os.path.relpath(TABELA, RAIZ), e))
        texto = ""
    if ferramenta.INICIO in texto and ferramenta.FIM in texto:
        bloco = texto.split(ferramenta.INICIO)[1].split(ferramenta.FIM)[0]
    else:
        if texto:
            avisos.append("%s nao tem as marcas %s / %s: nao da para saber o "
                          "que a pagina nomeia, e a regua conta o pior caso."
                          % (os.path.relpath(TABELA, RAIZ), ferramenta.INICIO,
                             ferramenta.FIM))
        bloco = ""
    # O id vai entre crases porque e' assim que a pagina o escreve -- na linha
    # da tabela e na do aviso. Procurar o id solto casaria um id DENTRO de
    # outro, e a regua daria por nomeada uma entrada que ninguem nomeou.
    _ESCONDIDAS = {
        "quando": corrida.get("quando", "?"),
        "julgadas": len(corrida.get("guardas") or []),
        "nao_julgadas": faltam,
        "escondidas": [i for i in faltam if ("`%s`" % i) not in bloco],
        "avisos": avisos,
    }
    return _ESCONDIDAS


def medido(dados=None):
    """Os seis numeros, contados AQUI e nao lidos da saida em texto.

    Catraca que le a propria saida quebra no dia em que alguem melhorar a
    redacao, e quebra em silencio -- a mesma licao do «texto se resolve por
    CHAVE, nunca por comparacao da frase».

    `dados` e o retorno de `achados()` quando o chamador ja o tem: sem ele,
    o `--catraca` varria o catalogo inteiro DUAS vezes (uma para listar, uma
    para contar) e pagava o dobro por um veredito identico."""
    trechos, ambiguos, testes, fora = dados if dados else achados()
    return {
        "TETO_TRECHO_MORTO": len(trechos),
        "TETO_TRECHO_AMBIGUO": len(ambiguos),
        "TETO_TESTE_MORTO": len(testes),
        "TETO_TESTE_FORA_DO_BINARIO": len(fora),
        "TETO_NAO_JULGADA_ESCONDIDA": len(escondidas()["escondidas"]),
        "PISO_DAS_ENTRADAS": len(catalogo()) + len(APOSENTADAS),
    }


# Cada catraca desta regua: nome, valor, o que mede, e de que LADO ela trava.
# `teto` so desce; `piso` so sobe. Os dois reprovam nos dois sentidos, porque
# teto frouxo e piso frouxo sao a mesma doenca vista de dois lados.
AS_CATRACAS = [
    ("TETO_TRECHO_MORTO", TETO_TRECHO_MORTO, "teto",
     "entradas cujo trecho o codigo nao tem mais",
     "guarda que nao pode nem ser tentada nao esta guardando nada -- "
     "ache para onde o ponto de reposicao andou e prove com o provador."),
    ("TETO_TRECHO_AMBIGUO", TETO_TRECHO_AMBIGUO, "teto",
     "entradas cujo trecho casa duas ou mais vezes no arquivo",
     "o executor recusa a entrada antes de tentar: trocar a ocorrencia "
     "errada provaria outra coisa. Alongue o trecho ate ele ser unico."),
    ("TETO_TESTE_MORTO", TETO_TESTE_MORTO, "teto",
     "testes nomeados que nao existem como `fn` em crates/**/*.rs",
     "o catalogo nomeia um teste que o fonte nao tem -- alguem renomeou "
     "o teste e a entrada ficou para tras."),
    ("TETO_TESTE_FORA_DO_BINARIO", TETO_TESTE_FORA_DO_BINARIO, "teto",
     "testes que existem, mas nao no binario que a entrada nomeia",
     "o provador roda so o binario nomeado e nunca veria esse teste -- "
     "corrija o `pacote`/`alvo` da entrada, ou o nome do teste."),
    ("TETO_NAO_JULGADA_ESCONDIDA", TETO_NAO_JULGADA_ESCONDIDA, "teto",
     "entradas que a ultima corrida nao julgou e que a pagina nao nomeia",
     "a tabela do `docs/TESTES.md` esta menor que o catalogo e nao diz que "
     "esta -- quem a lesse como inventario a leria curta. O conserto NAO e' "
     "rodar o provador: e' republicar a MESMA corrida, que ja nomeia o que "
     "ela nao julgou -- `python3 bancada/guardas/tabela-no-testes.py "
     "bancada/guardas/ultima-corrida.json`."),
    ("PISO_DAS_ENTRADAS", PISO_DAS_ENTRADAS, "piso",
     "entradas vivas do catalogo mais as aposentadas escritas",
     "sumiu entrada do catalogo sem aposentadoria escrita. Apagar a entrada "
     "mede o mesmo que consertar, e e mais barato -- por isso este piso "
     "existe. Se a guarda deixou mesmo de existir, escreva a linha em "
     "APOSENTADAS (id, data, motivo) no mesmo commit."),
]

# O que esta regua NAO ve, impresso junto do numero que ela ve.
#
# Nao e prosa de rodape: e o inventario do buraco. O `ok 0` desta catraca ja
# conviveu com `1 QUEBRADA` no provador (16/09/2026, a
# `trava-sem-guarda-de-reentrancia`, que estourou os 420 s), e quem lesse o
# zero como inventario concluiria que o catalogo estava inteiro. As tres so
# existem depois de compilar e rodar -- e uma regua de 0,2 s nao compila.
DO_PROVADOR = [
    ("o codigo com o defeito reposto nao compila",
     "ver isso custa uma compilacao por entrada"),
    ("a rodada estourou o prazo do executor",
     "ver isso custa rodar o binario ate o prazo"),
    ("o binario abortou quando a entrada nao esperava aborto",
     "ver isso custa rodar o binario"),
]


def catraca():
    dados = achados()
    trechos, ambiguos, testes, fora = dados
    agora = medido(dados)
    print("=== a catraca do catalogo envelhecido (pedido 263) ===")
    print(f"   {len(catalogo())} guardas no catalogo"
          + (f" + {len(APOSENTADAS)} aposentada(s) escrita(s)"
             if APOSENTADAS else ""))
    if trechos:
        print("   -- trecho que o codigo nao tem mais:")
        for gid, arq, porque in trechos:
            print(f"      {gid}: {porque} ({arq})")
    if ambiguos:
        print("   -- trecho que casa em mais de um lugar:")
        for gid, arq, porque in ambiguos:
            print(f"      {gid}: {porque} ({arq})")
    if testes:
        print("   -- teste nomeado que nao existe em crates/**/*.rs:")
        for gid, campo, teste in testes:
            print(f"      {gid}: {campo} -> {teste}")
    if fora:
        print("   -- teste que existe, mas nao no binario da entrada:")
        for gid, campo, teste in fora:
            print(f"      {gid}: {campo} -> {teste}")

    # O buraco cru sai IMPRESSO e nao travado: ele cresce quando alguem
    # escreve uma guarda nova, que e trabalho certo. Numero que nao se pode
    # zerar honestamente vira inventario, como o «o provador continua dono
    # de:» la de baixo -- nunca catraca.
    p = escondidas()
    print(f"   a ultima corrida ({p['quando']}) julgou {p['julgadas']} das "
          f"{len(catalogo())} entradas")
    if p["nao_julgadas"]:
        print(f"   -- {len(p['nao_julgadas'])} sem veredito nessa corrida: "
              f"{len(p['nao_julgadas']) - len(p['escondidas'])} nomeadas na "
              f"tabela publicada, {len(p['escondidas'])} escondidas")
    # So as ESCONDIDAS saem uma a uma: a lista existe para dar o que
    # consertar, e a corrida em dia tem 26 nomeadas -- imprimi-las todas
    # afogaria a unica linha que importa no dia em que uma sumir da pagina.
    for gid in p["escondidas"]:
        print(f"      {gid}: a pagina publicada nao a nomeia")

    ruim = 0
    for nome, valor, lado, _mede, recado in AS_CATRACAS:
        atual = agora[nome]
        if lado == "teto":
            acima, abaixo = "SUBIU", "DESCEU -- BAIXE O TETO"
            conserto = (f"Melhorou. Ponha o numero novo em {nome}, no mesmo "
                        "commit -- catraca frouxa nao segura nada.")
            reprova_acima = True
        else:
            acima, abaixo = "CRESCEU -- SUBA O PISO", "ENCOLHEU"
            conserto = (f"Cresceu. Ponha o numero novo em {nome}, no mesmo "
                        "commit -- piso parado volta a aceitar o apagamento "
                        "das entradas novas.")
            reprova_acima = False
        if atual > valor:
            print(f"\n   {acima}  {nome}: {atual} ({lado} {valor})")
            print("   " + (f"Reprovado: {recado}" if reprova_acima else conserto))
            ruim = 1
        elif atual < valor:
            print(f"\n   {abaixo}  {nome}: {atual} ({lado} {valor})")
            print("   " + (conserto if reprova_acima else f"Reprovado: {recado}"))
            ruim = 1
        else:
            print(f"\n   ok  {nome}: {atual} ({lado} {valor})")

    # Uma aposentadoria que "voltou" contaria duas vezes na soma do piso e o
    # deixaria frouxo em silencio -- a mesma entrada de dois lados.
    vivas = {g.get("id") for g in catalogo()}
    voltaram = [a["id"] for a in APOSENTADAS if a["id"] in vivas]
    if voltaram:
        print("\n   APOSENTADA QUE VOLTOU  " + ", ".join(voltaram))
        print("   Reprovado: ela conta dos dois lados e infla o piso. "
              "Tire a linha de APOSENTADAS e baixe o piso, ou renomeie a "
              "entrada viva.")
        ruim = 1

    # Parada com o motivo, e nao teto: sem a corrida ou sem as marcas da
    # pagina nao existe numero para comparar -- existe uma fonte que sumiu.
    # Reprovar com o motivo e' o unico veredito honesto; o `0` que sairia
    # calado seria lido como catalogo inteiro.
    for aviso in p["avisos"]:
        print("\n   NAO DA PARA MEDIR  " + aviso)
        ruim = 1

    print("\n   o provador continua dono de:")
    for o_que, porque in DO_PROVADOR:
        print(f"      {o_que} -- {porque}")
    print("   Esta regua nao compila e nao roda nada: um `ok` aqui NAO diz "
          "que o catalogo esta inteiro.")
    return ruim


def numeros():
    """Saida de maquina para o `docs/qa/medir.py`.

    O gerador do inventario NAO le a prosa acima, e nao le por decisao:
    `grep` em relatorio e resolver numero por comparacao de FRASE, e no dia
    em que alguem melhorar a redacao o inventario publica o numero de ontem
    sem dizer nada. A chave e estavel; o rotulo e livre."""
    agora = medido()
    for nome, valor, lado, mede, _recado in AS_CATRACAS:
        print(f"catraca:nome={nome};onde={EU};valor={valor};"
              f"medido={agora[nome]};tipo={lado};mede={mede}")
    return 0


def autoteste_da_quinta():
    """Prova real da quinta regua, sem `cargo` e sem provador.

    Cada caso repoe um defeito e confere que a regua o ACUSA, e confere
    tambem os dois silencios que seriam piores que o defeito: dar por nomeada
    uma entrada que a pagina nao nomeia, e devolver `0` quando a fonte sumiu.
    Teste que passa por engano e pior que teste que falta."""
    import tempfile
    global _CATALOGO, _ESCONDIDAS, CORRIDA, TABELA
    guardo = (_CATALOGO, CORRIDA, TABELA)
    falhas = []

    def conferir(nome, cond, detalhe=""):
        print("   %s  %s%s" % ("ok  " if cond else "FALHOU", nome,
                               "" if cond else "  -- " + detalhe))
        if not cond:
            falhas.append(nome)

    def medir(pagina, corrida, cat):
        global _CATALOGO, _ESCONDIDAS, CORRIDA, TABELA
        _CATALOGO, _ESCONDIDAS = cat, None
        caminho = os.path.join(tmp, "TESTES.md")
        if pagina is None:
            caminho = os.path.join(tmp, "nao-existe.md")
        else:
            with open(caminho, "w", encoding="utf-8") as f:
                f.write(pagina)
        TABELA = caminho
        alvo = os.path.join(tmp, "corrida.json")
        if corrida is None:
            alvo = os.path.join(tmp, "nao-existe.json")
        else:
            with open(alvo, "w", encoding="utf-8") as f:
                json.dump(corrida, f)
        CORRIDA = alvo
        return escondidas()

    marcas = gerador().INICIO + "\n%s\n" + gerador().FIM
    cat = [{"id": "g%d" % i} for i in range(5)] + [{"id": "g1-longa"}]
    corrida = {"quando": "hoje", "guardas": [{"id": "g0"}, {"id": "g1"}]}
    try:
        with tempfile.TemporaryDirectory() as tmp:
            # 1. O DEFEITO REPOSTO: a pagina traz so o que a corrida julgou.
            p = medir(marcas % "| `g0` |\n| `g1` |", corrida, cat)
            conferir("pagina muda: as 4 nao julgadas saem ESCONDIDAS",
                     [i for i in p["escondidas"]] == ["g2", "g3", "g4", "g1-longa"],
                     str(p["escondidas"]))
            conferir("e o buraco cru continua medido e impresso",
                     len(p["nao_julgadas"]) == 4 and p["julgadas"] == 2)

            # 2. O CONSERTO: a pagina nomeia as nao julgadas -> divida zero,
            #    com o buraco INTACTO. Os dois numeros nao podem se confundir.
            p = medir(marcas % ("| `g0` |\n| `g1` |\n- `g2`\n- `g3`\n- `g4`\n"
                                "- `g1-longa`"), corrida, cat)
            conferir("pagina que nomeia todas: zero escondidas",
                     p["escondidas"] == [], str(p["escondidas"]))
            conferir("e o buraco NAO zerou junto -- ele nao e' a divida",
                     len(p["nao_julgadas"]) == 4)

            # 3. A ARMADILHA DO SUBSTRING, e ela so pega no sentido certo: a
            #    pagina nomeia `g1-longa` e NAO nomeia `g1`. Procurar o id
            #    solto acharia «g1» DENTRO de «g1-longa» e daria uma entrada
            #    escondida por nomeada -- a regua mentiria a favor da pagina.
            #    A primeira versao deste caso testava o contrario e passava
            #    com o defeito reposto; teste que passa por engano e pior que
            #    teste que falta, e a mutacao foi quem disse isso.
            so_g0 = {"quando": "hoje", "guardas": [{"id": "g0"}]}
            p = medir(marcas % "| `g0` |\n- `g1-longa`\n- `g2`\n- `g3`\n- `g4`",
                      so_g0, cat)
            conferir("id que e' prefixo de outro nao passa por nomeado",
                     p["escondidas"] == ["g1"], str(p["escondidas"]))

            # 4 e 5. Fonte que sumiu: reprova com o motivo e conta o pior caso.
            p = medir(None, corrida, cat)
            conferir("pagina ausente: aviso escrito e pior caso contado",
                     len(p["avisos"]) == 1 and len(p["escondidas"]) == 4,
                     str(p["avisos"]))
            p = medir(marcas % "| `g0` |", None, cat)
            conferir("corrida ausente: aviso escrito e julgadas = 0",
                     len(p["avisos"]) == 1 and p["julgadas"] == 0,
                     str(p["avisos"]))
            p = medir("uma pagina sem marca nenhuma", corrida, cat)
            conferir("pagina sem as marcas: aviso escrito, nada dado por nomeado",
                     len(p["avisos"]) == 1 and len(p["escondidas"]) == 4,
                     str(p["avisos"]))
    finally:
        _CATALOGO, CORRIDA, TABELA = guardo
        _ESCONDIDAS = None

    print("   %s" % ("todos passaram" if not falhas
                     else "FALHOU: " + ", ".join(falhas)))
    return 1 if falhas else 0


def principal():
    if "--autoteste" in sys.argv:
        print("=== autoteste da quinta regua (pedido 269) ===")
        return autoteste_da_quinta()
    if "--catraca" in sys.argv:
        return catraca()
    if "--numeros" in sys.argv:
        return numeros()
    trechos, ambiguos, testes, fora = achados()
    for gid, arq, porque in trechos:
        print(f"trecho-morto   {gid}: {porque} ({arq})")
    for gid, arq, porque in ambiguos:
        print(f"trecho-ambiguo {gid}: {porque} ({arq})")
    for gid, campo, teste in testes:
        print(f"teste-morto    {gid}: {campo} -> {teste}")
    for gid, campo, teste in fora:
        print(f"teste-fora     {gid}: {campo} -> {teste}")
    return 0


if __name__ == "__main__":
    sys.exit(principal())
