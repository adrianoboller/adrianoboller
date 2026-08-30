# Write a minimal config for the footprint measurement
# 30/08 15:33

import json
c=json.load(open('exemplos/Config_exemplo_01.json'))
c['bind']='127.0.0.1:6990'
if 'web' in c and isinstance(c['web'],dict): c['web']['bind']='127.0.0.1:6991'
c['base']='$S/dados'.replace('\$S','$S')
print(json.dumps({k:v for k,v in c.items() if k in ('bind','base','web')}, indent=1)[:400])
json.dump(c, open('$S/config.json','w'), indent=1)
