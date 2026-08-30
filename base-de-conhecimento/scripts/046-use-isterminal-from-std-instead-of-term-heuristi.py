# Use IsTerminal from std instead of TERM heuristic
# 27/08 19:06

p='Cargo.toml'
s=open(p).read()
s=s.replace('rust-version = "1.75"','rust-version = "1.70"' if '1.75' in s else 'rust-version = "1.75"')
open(p,'w').write(s)
