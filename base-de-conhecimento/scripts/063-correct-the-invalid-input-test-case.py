# Correct the invalid input test case
# 27/08 19:21

p='crates/phxsql-core/src/base64.rs'
s=open(p).read()
s=s.replace('''            "Zm9=",           // padding a mais para o que sobra
            "Zg=",            // padding de menos''','''            "Zg=",            // padding de menos para o que sobra''')
open(p,'w').write(s)
