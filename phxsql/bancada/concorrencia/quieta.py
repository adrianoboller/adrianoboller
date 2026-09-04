"""A maquina estava quieta o bastante para o numero valer?

    from quieta import Vigia, porta_livre

Por que este arquivo existe
---------------------------
Medicao de concorrencia e a mais facil de sujar e a mais dificil de perceber
suja. Se outra frente compila enquanto o medidor roda, dois clientes disputam
nucleo com `rustc` e a curva achata -- e a curva achatada e EXATAMENTE o
sintoma que se esperava da trava. O numero sai bonito, com casas decimais, e
diz o contrario do que aconteceu.

Nao ha conserto por media, nem por repetir a rodada: repetir num ambiente
barulhento da media de ruido com menos variancia. O unico conserto e o medidor
saber dizer «nao sei», e esta e a peca que faz ele dizer.

O que se mede, e por que sao tres coisas
----------------------------------------
1. **A ocupacao de fundo**, do `/proc/stat`, ANTES e DEPOIS -- e a diferenca
   entre as duas. Uma so nao serve: uma compilacao que comeca no meio da
   rodada deixa as duas pontas parecidas e o meio estragado, entao ha tambem
   uma amostra durante.
2. **Quantas tarefas estao rodaveis**, do `procs_running` do `/proc/stat`.
   Ocupacao alta com uma tarefa so pode ser o proprio medidor; ocupacao alta
   com dez e vizinho.
3. **A curva de controle, no comeco e no fim.** Esta e a que pega o caso que
   as outras duas nao pegam: o `ping` nao toma a trava de dados, entao se ele
   desacelerar entre o comeco e o fim da bateria, quem desacelerou foi a
   MAQUINA. Controle que se move invalida a comparacao mesmo com o
   `/proc/stat` calmo.

A regra que ele impoe
---------------------
Recusar e o comportamento, nao um aviso. Um numero sujo publicado com uma
ressalva ao lado vira, tres documentos adiante, um numero limpo -- a ressalva
nao viaja junto. Aqui o veredito sujo NAO IMPRIME o numero.
"""
import os
import socket
import time

# A faixa reservada a esta frente. Fora dela ha servidor de outra frente, e
# subir por cima do soquete de quem esta medindo estraga as duas medicoes.
FAIXA = (7600, 7699)


def porta_livre(faixa=FAIXA):
    """Uma porta livre DENTRO da faixa desta frente."""
    for p in range(*faixa):
        with socket.socket() as s:
            s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
            try:
                s.bind(("127.0.0.1", p))
                return p
            except OSError:
                continue
    raise SystemExit(f"nenhuma porta livre em {faixa[0]}-{faixa[1]}")


def _stat():
    """(total, ocioso, tarefas rodaveis) do /proc/stat."""
    total = ocioso = rodaveis = 0
    with open("/proc/stat") as f:
        for linha in f:
            if linha.startswith("cpu "):
                campos = [int(x) for x in linha.split()[1:]]
                total, ocioso = sum(campos), campos[3]
            elif linha.startswith("procs_running"):
                rodaveis = int(linha.split()[1])
    return total, ocioso, rodaveis


class Amostra:
    """Ocupacao media da maquina numa janela, e o pico de tarefas rodaveis."""

    def __init__(self, janela=0.5, passos=5):
        t0, i0, _ = _stat()
        pico = 0
        for _ in range(passos):
            time.sleep(janela / passos)
            _, _, r = _stat()
            # O proprio medidor conta como uma: o que interessa e o excedente.
            pico = max(pico, r - 1)
        t1, i1, _ = _stat()
        self.ocupada = 100.0 * (1 - (i1 - i0) / max(1, t1 - t0))
        self.vizinhos = pico

    def __repr__(self):
        return f"{self.ocupada:.0f}% ocupada, ate {self.vizinhos} vizinho(s)"


class Vigia:
    """Guarda as pontas da bateria e diz se o que ficou no meio vale.

    `tolerancia_*` sao os unicos numeros escolhidos e nao medidos deste
    arquivo, e por isso ficam a vista e com o motivo escrito:

      * **10 pontos de ocupacao** entre as pontas: abaixo disso a variacao e do
        proprio medidor subindo e derrubando servidor.
      * **15% no controle**: media pelo `ruido-do-controle.py` em 04/09 --
        30 corridas de `ping` puro, 1 cliente, 1s cada. A maquina nao ficou
        parada em nenhum trecho longo o bastante naquele dia (esta arvore
        tinha tres outras frentes ativas): mesmo nas corridas com 0-1 vizinho
        rodavel a dispersao ficou em 15,6% de CV e ate 49,5% de salto --
        MAIOR que este teto, entao a medicao NAO confirma "parada" e NAO
        justifica apertar o numero. A clausula so deixa a catraca DESCER: sem
        base para apertar, ele fica em 15%, que e o que ja estava. O
        `ruido-do-controle.py` refaz esta conta quando alguem tiver uma
        maquina de fato ociosa para medir contra.
      * **2 vizinhos rodaveis**: um `cargo` sozinho ja e um; dois e uma
        compilacao paralela ao lado, e ai a comparacao acabou.
    """

    def __init__(self, tolerancia_ocupacao=10.0, tolerancia_controle=0.15,
                 vizinhos_demais=2):
        self.tolerancia_ocupacao = tolerancia_ocupacao
        self.tolerancia_controle = tolerancia_controle
        self.vizinhos_demais = vizinhos_demais
        self.antes = self.depois = None
        self.controle_antes = self.controle_depois = None
        self.durante = []

    def abrir(self):
        self.antes = Amostra()
        return self

    def durante_a_rodada(self, meus=0):
        """Uma amostra COM a carga rodando -- a que pega o vizinho que comecou
        no meio, quando as duas pontas ficaram parecidas.

        `meus` e quantas tarefas rodaveis sao DO PROPRIO ARNES (os clientes
        mais o servidor). Sem descontar, a primeira corrida deste vigia
        acusava «4 tarefas rodaveis alem do medidor» numa rodada de dois
        clientes -- e as quatro eram os dois clientes, o servidor e o
        amostrador. Instrumento que acusa a si mesmo recusa sempre, e recusar
        sempre nao e mais util que nunca recusar.
        """
        a = Amostra(janela=0.3, passos=3)
        a.meus = meus
        a.vizinhos = max(0, a.vizinhos - meus)
        self.durante.append(a)
        return a

    def fechar(self, assentar=1.5):
        """A ponta de tras, DEPOIS de a maquina assentar.

        Sem a espera, a amostra final pega o rescaldo do proprio arnes -- o
        servidor morrendo, o `/tmp` sendo apagado -- e a bateria se acusa de
        um barulho que ela mesma acabou de fazer.
        """
        time.sleep(assentar)
        self.depois = Amostra()
        return self

    # ------------------------------------------------------------- veredito

    def motivos(self):
        """A lista do que invalida. Vazia = pode publicar."""
        m = []
        if self.antes is None or self.depois is None:
            return ["a bateria nao fechou as duas pontas"]
        d = abs(self.depois.ocupada - self.antes.ocupada)
        if d > self.tolerancia_ocupacao:
            m.append(f"a ocupacao de fundo mudou {d:.0f} pontos entre as pontas "
                     f"({self.antes.ocupada:.0f}% -> {self.depois.ocupada:.0f}%)")
        vizinhos = max([a.vizinhos for a in (self.antes, self.depois)] +
                       [a.vizinhos for a in self.durante] or [0])
        if vizinhos > self.vizinhos_demais:
            m.append(f"ate {vizinhos} tarefas rodaveis alem do medidor: "
                     "ha outra frente trabalhando nesta maquina")
        if self.controle_antes and self.controle_depois:
            var = abs(self.controle_depois - self.controle_antes) / max(
                1e-9, self.controle_antes)
            if var > self.tolerancia_controle:
                m.append(f"a curva de CONTROLE mudou {var * 100:.0f}% entre o "
                         f"comeco e o fim ({self.controle_antes:.0f} -> "
                         f"{self.controle_depois:.0f} op/s): quem desacelerou "
                         "foi a maquina, e o `ping` nem toma a trava")
        return m

    def publicavel(self):
        return not self.motivos()

    def relatar(self):
        print("-- a maquina estava quieta?")
        print(f"   antes:  {self.antes}")
        if self.durante:
            oc = sorted(a.ocupada for a in self.durante)
            viz = max(a.vizinhos for a in self.durante)
            print(f"   durante ({len(self.durante)} amostras): "
                  f"{oc[0]:.0f}% a {oc[-1]:.0f}% ocupada, mediana "
                  f"{oc[len(oc) // 2]:.0f}%, ate {viz} vizinho(s)")
        print(f"   depois: {self.depois}")
        if self.controle_antes and self.controle_depois:
            print(f"   controle: {self.controle_antes:.0f} -> "
                  f"{self.controle_depois:.0f} op/s")
        if self.publicavel():
            print("   VEREDITO: quieta o bastante. Os numeros valem.\n")
        else:
            print("   VEREDITO: NAO. O numero desta bateria nao vale, e por "
                  "isso ele nao sai:")
            for m in self.motivos():
                print(f"     - {m}")
            print()


def nucleos():
    """Quantos nucleos ESTE processo pode usar -- nao quantos a maquina tem.

    `os.cpu_count()` conta os da maquina; dentro de contentor com afinidade ou
    cota, o teto real e outro, e um teto errado faz «N acima dos nucleos» ser
    dito na hora errada.
    """
    try:
        return len(os.sched_getaffinity(0))
    except AttributeError:
        return os.cpu_count() or 1


def confira_a_pagina(call, monta_pedido, pedidos=(1, 7, 50)):
    """A guarda que teria pegado o defeito de 04/09: **voltou o que se pediu?**

    Ate 04/09 as quatro bancadas de concorrencia mandavam `{"varrer", ...,
    "limite": 50}`. O `op_varrer` le o campo **`max`**; `limite` nao existe no
    pedido e era ignorado em silencio, entao TODA leitura caia no teto de
    configuracao e devolvia 1.000 linhas. As «variacoes» de tamanho de pagina
    eram a mesma leitura, e nenhuma bancada podia perceber: **as quatro
    mandavam o mesmo campo errado, entao nenhuma discordava de nenhuma**.

    Quem pegou foi um medidor de OUTRA camada (o `--example
    onde-doi-na-leitura`, em processo) discordando do de rede.

    `monta_pedido` e o construtor DA PROPRIA BANCADA -- `monta_pedido(n)`
    devolve o pedido que ela vai medir com pagina de `n` linhas. Passar o
    construtor, e nao um pedido montado aqui, e a diferenca entre a guarda
    conferir o SERVIDOR e conferir a BANCADA: a primeira versao desta funcao
    montava `{"max": n}` por conta propria e teria passado com a bancada
    mandando `limite`, que e exatamente o defeito que ela existe para pegar.

    Recusar campo desconhecido no SERVIDOR seria o conserto errado -- quebraria
    todo cliente que manda um campo a mais, e «guarda nova entra pedida, nao
    imposta». A conferencia certa e do lado de quem mede.

    A tabela precisa ter pelo menos `max(pedidos)` linhas.
    """
    for n in pedidos:
        r = call(dict(monta_pedido(n)), exigir=False)
        linhas = r.get("linhas")
        if linhas is None:
            linhas = r.get("resultado", {}).get("linhas", [])
        if len(linhas) != n:
            raise SystemExit(
                f"A BANCADA NAO ESTA PEDINDO O QUE ACHA QUE PEDE: o pedido "
                f"dela para {n} linha(s) devolveu {len(linhas)}.\n"
                f"  pedido montado: {monta_pedido(n)}\n"
                f"Foi assim que a §13 do docs/CONCORRENCIA.md nasceu errada."
            )
