import re,os,json,subprocess
MAP={'A1mini':'a1mini.html','A1':'pg-a1.html','P1S':'pg-p1s.html','P2S':'pg-p2s.html',
     'X2D':'pg-x2d.html','H2S':'pg-h2s.html','H2C':'pg-h2c.html','H2D':'pg-h2d.html'}
UA="Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/124.0 Safari/537.36"
cat={}
for mod,f in MAP.items():
    h=open(f,encoding='utf-8',errors='replace').read().replace('\\"','"').replace('\\/','/')
    urls=re.findall(r'"url":"(https://[^"]+)"',re.search(r'"mediaFiles":\[(.*?)\]',h,re.S).group(1))
    seen=[];  os.makedirs(f'img/{mod}',exist_ok=True)
    for u in urls:
        if u in seen: continue
        seen.append(u)
    cat[mod]=seen
    for i,u in enumerate(seen):
        dst=f'img/{mod}/{i:02d}.jpg'
        if not os.path.exists(dst):
            subprocess.run(['curl','-s','-A',UA,'-o',dst,u],check=False)
    print(mod, len(seen))
json.dump(cat,open('catalogo.json','w'),indent=1)
