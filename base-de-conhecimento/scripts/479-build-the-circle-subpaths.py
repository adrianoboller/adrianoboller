# Build the circle subpaths
# 28/08 15:48

# Um circulo como subcaminho, para poder juntar dois num `d` so.
def circ(cx, cy, r):
    return f"M{cx-r},{cy} a{r},{r} 0 1,0 {2*r},0 a{r},{r} 0 1,0 {-2*r},0 Z"
print("A  ", circ(34,30,23))
print("B  ", circ(58,30,23))
print("AB ", circ(34,30,23) + " " + circ(58,30,23))
print()
print("ui A ", circ(36,29,24))
print("ui B ", circ(60,29,24))
