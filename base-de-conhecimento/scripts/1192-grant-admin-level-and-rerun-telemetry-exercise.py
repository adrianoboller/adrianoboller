# Grant admin level and rerun telemetry exercise
# 29/08 18:47

import json
c = json.load(open("$S/config.json"))
c["usuarios"][0]["nivel"] = "admin"
json.dump(c, open("$S/config.json","w"), indent=2)
print("nivel admin dado ao usuario de teste")
