source "$(dirname "$0")/comum.sh"
titulo "Certificação de release — gerada, não escrita à mão" \
       "cada gate roda de verdade; o que não roda aqui aparece BLOQUEADO com o motivo"
roda "python3 tools/demo/mostra_certificacao.py" 7
nota "7 de 12 obrigatórios passam — os 5 restantes dependem de chave, credenciais ou hardware" 5
