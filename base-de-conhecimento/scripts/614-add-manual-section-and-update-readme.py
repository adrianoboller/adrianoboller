# Add manual section and update README
# 28/08 18:04

import io
p='MANUAL.txt'
s=io.open(p,encoding='utf-8').read()
velho='''OPERACOES

    ping                                     versao, papel, conexoes'''
novo='''AS DUAS EXCLUSOES

    Toda tabela nasce com a coluna de sistema "softdeleted", e por isso excluir
    passou a ser duas coisas diferentes.

    SUAVE (o padrao). Marca a linha. Ela some das listas e continua INTEIRA no
    .reg, com os anexos. "restaurar" desfaz.

        {"op":"excluir","database":"loja","tabela":"clientes","rowid":42,
         "motivo":"pedido de remocao do titular"}

    FISICA (so quando pedida). A linha sai do .reg. Antes disso ela e gravada
    inteira no .trash -- com o CONTEUDO dos anexos, e nao com os ponteiros --,
    e o DISCO CONFIRMA antes de o slot ser liberado.

        {"op":"excluir","database":"loja","tabela":"clientes","rowid":42,
         "fisico":true,"motivo":"duplicidade com o contrato 9"}

    POR QUE O PADRAO E O SUAVE. O caminho irreversivel nao pode ser escolhido
    por omissao. Um cliente que manda "excluir" sem dizer mais nada esta
    pedindo "tira isto da minha lista", e e isso que ele recebe. Numa tabela
    anterior a v4 do esquema -- que nao tem a coluna -- so existe o caminho
    fisico, e ele e usado sem alarde.

    A ORDEM DA GRAVACAO E A GARANTIA. Guardar depois de liberar teria uma
    janela em que a linha nao existe em lugar nenhum, e uma queda dentro dela
    nao tem conserto. Guardar antes tem a janela oposta: a linha aparece nos
    dois lugares, o que se resolve olhando. Entre perder e duplicar, o motor
    duplica.

    A VARREDURA NAO ENXERGA LINHA MARCADA. "visao" escolhe:

        "ativas"     (padrao) so as nao marcadas
        "excluidas"  so as marcadas -- a tela do administrador
        "todas"      tudo que esta no .reg

    MOTIVO OBRIGATORIO. Escolhido na criacao da tabela
    ("motivo_obrigatorio": true). Marcada, o motor RECUSA qualquer exclusao
    sem uma frase escrita, antes de qualquer gravacao. Vale para tabela cujo
    apagamento alguem vai ter de justificar depois; numa tabela de rascunho,
    obrigar so ensina todo mundo a digitar um ponto.

    ESVAZIAR A LIXEIRA NAO TEM VOLTA, e por isso exige motivo mesmo numa tabela
    que nao exige motivo para excluir. O expurgo e registrado no .reason ANTES
    de o dado sair: o motivo tem de sobreviver ao dado.

    NA TELA. O botao Excluir da ficha abre um dialogo com os dois modos e o
    campo do motivo. A grade tem o par "ativas / excluidas", e na visao das
    excluidas cada linha ganha o botao "restaurar". A lixeira e os motivos tem
    tela propria, no menu Tabelas e no botao Lixeira da barra -- e as duas so
    abrem para quem tem "administrar".

OPERACOES

    ping                                     versao, papel, conexoes'''
assert velho in s
s=s.replace(velho,novo,1)
io.open(p,'w',encoding='utf-8').write(s)
print('ok')
