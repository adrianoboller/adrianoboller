# Add JSON to Value translation layer
# 27/08 18:42

p='Cargo.toml'
s=open(p).read()
s=s.replace('''    "crates/phxsql-cli",
]''','''    "crates/phxsql-cli",
    "crates/phxsql-server",
]''')
s=s.replace('''phxsql-store = { path = "crates/phxsql-store" }''','''phxsql-store = { path = "crates/phxsql-store" }
phxsql-server = { path = "crates/phxsql-server" }''')
open(p,'w').write(s)
