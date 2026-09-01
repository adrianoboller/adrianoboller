# Test the chart rendering with synthetic data
# 30/08 18:06

import json, os
S=os.environ.get('S','/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad')
d={"linhas":1000000,"quando":"teste","maquina":{},
 "durabilidade":{"phxsql":"janela","mysql":"flush=1","sqlite":"WAL"},
 "fases":{
  "inserir":{"phxsql":{"mediana_s":6.34,"min_s":6.1,"max_s":7.2,"rodadas":5},
             "mysql":{"mediana_s":12.0,"min_s":11.2,"max_s":16.6,"rodadas":5},
             "sqlite":{"mediana_s":1.74,"min_s":1.7,"max_s":1.9,"rodadas":5}},
  "buscar":{"phxsql":{"mediana_s":0.031,"min_s":0.03,"max_s":0.033,"rodadas":5},
            "mysql":{"mediana_s":0.09,"min_s":0.08,"max_s":0.11,"rodadas":5},
            "sqlite":{"mediana_s":0.061,"min_s":0.06,"max_s":0.064,"rodadas":5}},
  "atualizar":{"phxsql":{"mediana_s":6.88,"min_s":6.5,"max_s":7.4,"rodadas":5},
               "mysql":{"mediana_s":9.9,"min_s":9.1,"max_s":11.0,"rodadas":5},
               "sqlite":{"mediana_s":10.12,"min_s":9.8,"max_s":10.9,"rodadas":5}},
  "excluir":{"phxsql":{"mediana_s":34.5,"min_s":33.0,"max_s":36.1,"rodadas":5},
             "mysql":{"mediana_s":8.9,"min_s":8.2,"max_s":9.6,"rodadas":5},
             "sqlite":None}},
 "ressalvas":["Teste de renderizacao -- numeros de mentira."]}
json.dump(d, open(f"{S}/um-milhao-teste.json","w"), indent=1)
print("json de teste gravado")
