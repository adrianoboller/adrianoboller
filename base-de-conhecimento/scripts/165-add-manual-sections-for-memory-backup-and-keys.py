# Add manual sections for memory, backup and keys
# 27/08 20:59

import re
s = open('MANUAL.txt').read()

# renumera 10..15 -> 13..18 (abrindo espaco para 10, 11, 12 novos)
for n in range(15, 9, -1):
    s = re.sub(rf"\n{n}\. ([A-Z][^\n]*)\n-{{80}}", lambda m, n=n: f"\n{n+3}. {m.group(1)}\n" + "-"*80, s)

novas = '''
10. TABELA EM MEMORIA (SelectMemory)
--------------------------------------------------------------------------------
O .reg e rapido porque enderecar um registro e aritmetica -- mas ainda e um
seek e um read no disco. Ha tabela que quase nao muda e e lida o tempo todo:
precos, cidades, parametros. Para essas, da para tirar o disco do caminho.

Carregada, a tabela vira um vetor em RAM e a consulta nao toca em arquivo
nenhum. E o modelo de um Redis(R): o dado MORA na memoria.

CARREGAR

    {"token":"...","op":"memoria_carregar",
     "database":"Comercial","tabela":"cadastroClientes"}

    Sem mais nada, ganham mapa de igualdade as colunas que ja sao a primeira
    de algum indice -- quem indexou no disco costuma filtrar pelo mesmo campo
    na memoria. Para escolher: "mapear":["cidade","uf"].

    Responde com linhas, slots, bytes, mapas e carregou_em_ms.

CONSULTAR

    {"token":"...","op":"SelectMemory",
     "database":"Comercial","tabela":"cadastroClientes",
     "onde":[{"coluna":"cidade","op":"=","valor":"Blumenau"},
             {"coluna":"limite","op":">=","valor":"1000.00"}],
     "ordenar":[{"coluna":"limite","desc":true}],
     "colunas":["nome","cidade","limite"],
     "pular":0, "max":20}

    Operadores: =  !=  <  <=  >  >=  contem  comeca  termina  nulo  nao_nulo
    "contem", "comeca" e "termina" nao distinguem maiuscula de minuscula.

    Todos os filtros valem JUNTOS (E, nunca OU).

    A resposta traz "por_mapa" com o nome da coluna cujo mapa evitou a
    varredura, "examinadas" com quantas linhas o motor precisou olhar, e "us"
    com o tempo em microssegundos. Sao os tres numeros de que voce precisa
    para saber se a consulta esta boa.

    selecionar_memoria e o mesmo nome em portugues. Sao a mesma operacao.

LIBERAR E LISTAR

    {"op":"memoria_liberar","database":"...","tabela":"..."}
    {"op":"memoria"}          o que esta residente, com bytes e idade

O QUE VOCE PRECISA SABER

    NADA entra em memoria sozinho. Voce carrega quando quer e libera quando
    quer. Um cache que decide sozinho o que guardar e um cache que um dia
    decide errado no pior momento.

    A copia NAO vive a parte. Toda inclusao, alteracao e exclusao atualiza a
    copia residente no mesmo passo, dentro da mesma trava do disco. Nao ha
    janela em que os dois discordem.

    A ordem de digitacao vale na memoria como vale no arquivo: slot excluido
    vira buraco e o rowid seguinte continua de onde estava.

    A memoria NAO sobrevive ao servidor. Reiniciou, carrega de novo.

    Consultar em memoria pede permissao de LER, nao de administrar: e o mesmo
    dado do disco por outro caminho.

QUANTO ISSO PAGA -- MEDIDO, NAO ESTIMADO

    50.000 linhas, a mesma pergunta pelos dois caminhos:

        varrendo o .reg      55.878 us   (50.000 linhas lidas do disco)
        SelectMemory            641 us   (8.333 examinadas, mapa em cidade)
                                 87x

        carga para a RAM         53 ms   (2.205 KB de valores)

    Refaca na sua maquina:  cargo run --release --example memoria


11. BACKUP
--------------------------------------------------------------------------------
Copiar arquivo e facil. O dificil e saber, seis meses depois, que a copia
presta. Por isso o backup daqui e copia MAIS um backup.json com o SHA-256 de
cada arquivo, e um comando que le tudo de volta e confere.

COM O SERVIDOR PARADO

    phxsql backup <base> <destino>
    phxsql conferir-backup <destino>

    O conferir sai com codigo de erro quando nao bate -- da para por no cron:

        phxsql conferir-backup /backup/phxsql || mandar-email

COM O SERVIDOR NO AR

    {"token":"...","op":"backup","destino":"/backup/phxsql-2026-08-27"}
    {"token":"...","op":"conferir_backup","destino":"/backup/phxsql-2026-08-27"}

    Pede permissao de ADMINISTRAR.

O QUE "CONSISTENTE" QUER DIZER AQUI

    Nao ha transacoes, entao consistente quer dizer uma coisa precisa:
    NENHUMA ESCRITA ACONTECE DURANTE A COPIA. A operacao "backup" segura a
    trava unica de dados do inicio ao fim, e como toda escrita passa por essa
    mesma trava, nao ha registro pela metade.

    E menos do que um snapshot de verdade: uma escrita longa faz o backup
    esperar, e o backup faz a escrita esperar. E o que da para prometer sem
    mentir enquanto nao houver commit.

    Pela linha de comando NAO ha essa garantia -- ela nao segura trava
    nenhuma. Com o servidor no ar, use a operacao.

O QUE A COPIA LEVA

    Tudo debaixo da raiz de dados: os cinco arquivos de cada tabela, os
    volumes numerados e os diretorios de database e de schema.

    O config.json NAO vai junto. Ele tem o token e os hashes de senha, e
    backup de dado costuma ir para lugar diferente de backup de segredo.

O QUE O CONFERIR ACHA

    Arquivo que sumiu, arquivo que mudou (mesmo do mesmo tamanho -- so o
    SHA-256 pega) e arquivo que apareceu sem estar no manifesto.

RESTAURAR

    Nao ha comando: pare o servidor, apague a raiz de dados e copie o
    conteudo do destino de volta. Confira ANTES de apagar qualquer coisa.


12. CHAVE PUBLICA E PRIVADA (SEGUNDO FATOR)
--------------------------------------------------------------------------------
A senha prova que voce SABE alguma coisa. A chave prova que voce TEM alguma
coisa. Sao fatores diferentes, e por isso somam.

E ha uma diferenca que importa: no desafio-resposta, o que esta guardado no
config.json e exatamente a chave usada na prova -- quem le o arquivo consegue
autenticar. Com chave assimetrica nao: a privada nunca esteve no servidor.

GERAR

    phxsqld --gerar-chave

    Sai a privada UMA vez e a linha que vai no config.json. Guarde a privada
    FORA do servidor. Perdeu, nao ha como recuperar: gera-se outra e troca-se
    a publica.

LIGAR PARA UM USUARIO

    {
      "id": 2, "login": "adriano", "senha_hash": "pbkdf2-sha256$...",
      "chave_publica": "88cee8fc...55b7aa3f"
    }

    Quem tem a linha passa a precisar assinar. Quem nao tem continua so com a
    senha. Da para misturar os dois no mesmo servidor.

COMO E O LOGIN

    1. {"op":"desafio","usuario":"adriano"}       -> sal, iteracoes, nonce
    2. prova       = HMAC(pbkdf2(senha,sal), "nonce,nonce_cliente,usuario")
       assinatura  = Ed25519(chave_privada,   "nonce,nonce_cliente,usuario")
    3. {"op":"login","usuario":"adriano","prova":"...",
        "nonce_cliente":"...","assinatura":"<128 hex>"}

    A mensagem assinada e a MESMA da senha. Entao a assinatura tambem vale
    uma vez so, e tambem morre com o nonce.

PELO NAVEGADOR

    O campo "Chave privada" aparece sozinho quando algum usuario exige chave.
    Depende de crypto.subtle com Ed25519, que e recente -- navegador sem
    suporte recebe um aviso claro em vez de uma falha silenciosa.

ALGORITMO

    Ed25519 (RFC 8032), escrito neste projeto, sem dependencia externa,
    conferido contra os quatro vetores oficiais e contra a implementacao de
    referencia da propria RFC.

'''

ancora = "\n13. LOG DE ACESSOS\n"
assert ancora in s, "ancora nao achada"
s = s.replace(ancora, novas + ancora[1:], 1)
open('MANUAL.txt','w').write(s)
