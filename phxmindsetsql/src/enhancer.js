const CDN={
  sigma:'https://cdn.jsdelivr.net/npm/sigma@3.0.3/+esm',
  graphology:'https://cdn.jsdelivr.net/npm/graphology@0.26.0/+esm',
  fa2:'https://cdn.jsdelivr.net/npm/graphology-layout-forceatlas2@0.10.1/+esm',
  elk:'https://cdn.jsdelivr.net/npm/elkjs@0.12.0/+esm'
};

export function createEnhancer(api){
  const rt={Sigma:null,Graph:null,fa2:null,ELK:null,elk:null,sigmaRenderer:null,sigmaGraph:null,loading:null,mode:'fallback'};
  const load=()=>rt.loading ||= Promise.allSettled([
    import(CDN.sigma), import(CDN.graphology), import(CDN.fa2), import(CDN.elk)
  ]).then(results=>{
    const [s,g,f,e]=results;
    if(s.status==='fulfilled') rt.Sigma=s.value.default || s.value.Sigma || s.value;
    if(g.status==='fulfilled') rt.Graph=g.value.default || g.value.Graph || g.value;
    if(f.status==='fulfilled') rt.fa2=f.value.default || f.value;
    if(e.status==='fulfilled') rt.ELK=e.value.default || e.value.ELK || e.value;
    if(rt.ELK) try{rt.elk=new rt.ELK()}catch(_){rt.elk=null}
    rt.mode=(rt.Sigma&&rt.Graph?'WebGL':'Canvas')+' / '+(rt.elk?'ELK':'Fallback');
    api.setEngine(rt.mode);
    return rt;
  }).catch(()=>rt);

  async function renderRelational(){
    if(!rt.elk) return false;
    const root=api.selectedTable()||api.state.model.tables[0]; if(!root) return true;
    const levels=api.bfsLevels(root.name,api.state.depth); let names=[...levels.keys()];
    if(api.state.migration) names=names.filter(n=>api.tableByName(n)?.migration===api.state.migration||n===api.canonical(root.name));
    names=names.slice(0,40);
    const nameSet=new Set(names); const nodes=[];
    for(const n of names){const t=api.tableByName(n);if(!t)continue;nodes.push({id:api.canonical(t.name),width:252,height:56+Math.min(9,t.columns.length)*24+(t.columns.length>9?22:0)});}
    const edges=[]; let edgeId=0;
    for(const fk of api.state.model.relationships){const a=api.canonical(fk.from_table),b=api.canonical(fk.to_table);if(nameSet.has(a)&&nameSet.has(b)&&a!==b)edges.push({id:`e${edgeId++}`,sources:[a],targets:[b]});}
    const graph={id:'root',layoutOptions:{
      'elk.algorithm':'layered','elk.direction':'RIGHT','elk.edgeRouting':'ORTHOGONAL',
      'elk.spacing.nodeNode':'48','elk.layered.spacing.nodeNodeBetweenLayers':'120',
      'elk.layered.nodePlacement.strategy':'BRANDES_KOEPF','elk.layered.crossingMinimization.strategy':'LAYER_SWEEP',
      'elk.padding':'[top=54,left=54,bottom=54,right=54]','elk.aspectRatio':'1.6'
    },children:nodes,edges};
    let laid; try{laid=await rt.elk.layout(graph)}catch(_){return false}
    if(api.state.view!=='relational') return true;
    const nodeLayer=api.$('#relationalNodes'), edgeSvg=api.$('#relationalEdges'), world=api.$('#relationalWorld'); nodeLayer.innerHTML='';edgeSvg.innerHTML='';
    const W=Math.max(laid.width||900,900),H=Math.max(laid.height||600,600); world.style.width=`${W}px`;world.style.height=`${H}px`;edgeSvg.setAttribute('viewBox',`0 0 ${W} ${H}`);edgeSvg.setAttribute('width',W);edgeSvg.setAttribute('height',H);
    const pos=new Map(); for(const n of laid.children||[]) pos.set(n.id,{x:n.x||0,y:n.y||0,w:n.width||252,h:n.height||160});
    const hot=api.canonical(root.name);
    for(const e of laid.edges||[]){const original=edges.find(x=>x.id===e.id);const isHot=original&&(original.sources[0]===hot||original.targets[0]===hot);for(const sec of e.sections||[]){let d=`M${sec.startPoint.x},${sec.startPoint.y}`;for(const b of sec.bendPoints||[])d+=` L${b.x},${b.y}`;d+=` L${sec.endPoint.x},${sec.endPoint.y}`;edgeSvg.append(api.svgEl('path',{d,class:`fk-line elk${isHot?' selected':''}`}));if(api.state.labels){const pts=[sec.startPoint,...(sec.bendPoints||[]),sec.endPoint],mid=pts[Math.floor(pts.length/2)];edgeSvg.append(api.svgEl('text',{x:mid.x+5,y:mid.y-5,class:'cardinality elk-label'},'N → 1'));}}}
    for(const id of names){const t=api.tableByName(id),p=pos.get(api.canonical(id));if(!t||!p)continue;const card=document.createElement('div');card.className='table-card pro'+(api.canonical(t.name)===hot?' selected':'');card.style.left=`${p.x}px`;card.style.top=`${p.y}px`;card.style.width=`${p.w}px`;const fkCols=new Set(t.foreign_keys.flatMap(f=>f.from_columns.map(api.canonical)));const rows=t.columns.slice(0,9).map(c=>`<div class="table-row"><span class="key ${c.primary_key?'pk':fkCols.has(api.canonical(c.name))?'fk':''}">${c.primary_key?'PK':fkCols.has(api.canonical(c.name))?'FK':''}</span><span class="col-name">${api.escapeHtml(c.name)}</span><span class="col-type">${api.escapeHtml(api.truncate(c.data_type,16))}</span></div>`).join('');card.innerHTML=`<div class="table-head"><strong>${api.escapeHtml(api.shortName(t.name))}</strong><span>${api.escapeHtml(t.name.includes('.')?t.name.split('.')[0]:'public')} · ${t.columns.length} campos · ${t.degree||0} links</span></div><div class="table-columns">${rows}${t.columns.length>9?`<div class="more-row">+ ${t.columns.length-9} campos</div>`:''}</div>`;card.addEventListener('click',()=>api.selectTable(t.name,{openInspector:true}));nodeLayer.append(card);}
    api.state.rel.layout={pos,names,W,H,engine:'ELK'};api.applyRelTransform();api.$('#visibleCount').textContent=api.fmt(names.length);api.drawMiniMap();
    if(!api.state.rel.userPositioned) fitRelational(false);
    return true;
  }

  function fitRelational(animated=true){const v=api.$('#relationalView'),l=api.state.rel.layout;if(!l)return false;const r=v.getBoundingClientRect(),margin=70;const z=Math.min((r.width-margin)/(l.W||r.width),(r.height-margin)/(l.H||r.height),1.35);api.state.rel.zoom=Math.max(.2,z);api.state.rel.panX=(r.width-(l.W||r.width)*z)/2;api.state.rel.panY=(r.height-(l.H||r.height)*z)/2;api.applyRelTransform();api.updateZoomLabel();return true;}

  function destroySigma(){if(rt.sigmaRenderer){try{rt.sigmaRenderer.kill()}catch(_){}rt.sigmaRenderer=null;rt.sigmaGraph=null;}rt.sigmaKey=null;const c=api.$('#sigmaContainer');if(c)c.innerHTML='';}

  function renderObsidian(){
    if(!(rt.Sigma&&rt.Graph)) return false;
    const key=`${api.state.model.source}|${api.state.model.stats?.bytes||0}|${api.state.model.stats?.foreign_keys||0}|${api.state.migration??'all'}`;
    const container=api.$('#sigmaContainer'),fallback=api.$('#obsidianCanvas');if(!container)return false;
    if(rt.sigmaRenderer&&rt.sigmaKey===key){container.hidden=false;fallback.hidden=true;rt.sigmaRenderer.refresh();api.$('#visibleCount').textContent=api.fmt(rt.sigmaGraph?.order||0);return true}
    destroySigma(); container.hidden=false;fallback.hidden=true;
    let graph;try{graph=new rt.Graph()}catch(_){return false}
    const tables=api.state.model.tables.filter(t=>!api.state.migration||t.migration===api.state.migration);const allowed=new Set(tables.map(t=>api.canonical(t.name)));const migs=[...new Set(tables.map(t=>t.migration??0))].sort((a,b)=>a-b);const centers=new Map(migs.map((m,i)=>{const a=(i/Math.max(1,migs.length))*Math.PI*2;return[m,{x:Math.cos(a)*20,y:Math.sin(a)*14}]}));
    for(const t of tables){const id=api.canonical(t.name),h=api.hash32(t.name),c=centers.get(t.migration??0)||{x:0,y:0},a=(h%6283)/1000,r=2+((h>>>8)%800)/100;graph.addNode(id,{label:api.shortName(t.name),x:c.x+Math.cos(a)*r,y:c.y+Math.sin(a)*r,size:3+Math.min(9,Math.sqrt((t.degree||0)+1)*1.25),color:api.palette[((t.migration||0)-1+api.palette.length)%api.palette.length],degree:t.degree||0,fullName:t.name});}
    let ei=0;for(const fk of api.state.model.relationships){const a=api.canonical(fk.from_table),b=api.canonical(fk.to_table);if(allowed.has(a)&&allowed.has(b)&&a!==b&&!graph.hasEdge(a,b)){try{graph.addEdgeWithKey(`e${ei++}`,a,b,{size:.45,color:'rgba(105,129,167,.26)'})}catch(_){}}}
    if(rt.fa2&&graph.order<900){try{const settings=rt.fa2.inferSettings?rt.fa2.inferSettings(graph):undefined;rt.fa2.assign(graph,{iterations:Math.min(120,35+Math.round(Math.sqrt(graph.order)*4)),settings:{...(settings||{}),gravity:1.2,scalingRatio:7,strongGravityMode:false}})}catch(_){}}
    const selected=()=>api.canonical(api.state.selected||'');
    try{rt.sigmaRenderer=new rt.Sigma(graph,container,{allowInvalidContainer:true,renderEdgeLabels:false,labelDensity:.1,labelGridCellSize:90,labelRenderedSizeThreshold:8,hideEdgesOnMove:graph.order>450,hideLabelsOnMove:true,zIndex:true,nodeReducer:(node,data)=>{const s=selected(),isSel=node===s;if(!isSel&&s){const adjacent=graph.areNeighbors?.(node,s);if(adjacent===false)return{...data,color:fade(data.color,.26),label:null};}return isSel?{...data,size:(data.size||4)*1.45,color:'#a8b0ff',zIndex:5,forceLabel:true}:{...data,forceLabel:(data.degree||0)>=7};},edgeReducer:(edge,data)=>{const [a,b]=graph.extremities(edge),s=selected();const hot=s&&(a===s||b===s);return hot?{...data,color:'rgba(145,157,255,.72)',size:1.45,zIndex:4}:{...data,color:'rgba(92,117,156,.20)',size:.45};}})}catch(_){fallback.hidden=false;container.hidden=true;return false}
    rt.sigmaGraph=graph;rt.sigmaKey=key;rt.sigmaRenderer.on('clickNode',({node})=>{const d=graph.getNodeAttributes(node);api.selectTable(d.fullName||node,{openInspector:true});rt.sigmaRenderer.refresh();});rt.sigmaRenderer.on('enterNode',()=>container.classList.add('node-hover'));rt.sigmaRenderer.on('leaveNode',()=>container.classList.remove('node-hover'));
    api.$('#visibleCount').textContent=api.fmt(graph.order);api.state.graph._sigma=true;api.drawMiniMap();return true;
  }

  function zoomObsidian(f){if(!rt.sigmaRenderer)return false;const cam=rt.sigmaRenderer.getCamera();const s=cam.getState();cam.animate({...s,ratio:Math.max(.03,Math.min(6,s.ratio/f))},{duration:180});return true;}
  function fitObsidian(){if(!rt.sigmaRenderer)return false;rt.sigmaRenderer.getCamera().animatedReset?.({duration:350})||rt.sigmaRenderer.getCamera().setState({x:.5,y:.5,ratio:1,angle:0});return true;}
  function focusObsidian(){if(!rt.sigmaRenderer||!rt.sigmaGraph)return false;const id=api.canonical(api.state.selected||'');if(!rt.sigmaGraph.hasNode(id))return false;const d=rt.sigmaRenderer.getNodeDisplayData(id);if(!d)return false;try{rt.sigmaRenderer.getCamera().animate({x:d.x,y:d.y,ratio:.18},{duration:420})}catch(_){}return true;}
  function refreshObsidian(){try{rt.sigmaRenderer?.refresh()}catch(_){}}
  function exportObsidian(){if(!rt.sigmaRenderer)return false;const canvases=[...api.$$('#sigmaContainer canvas')];if(!canvases.length)return false;const r=api.$('#sigmaContainer').getBoundingClientRect(),dpr=Math.min(3,devicePixelRatio||1),out=document.createElement('canvas');out.width=Math.max(1,Math.floor(r.width*dpr));out.height=Math.max(1,Math.floor(r.height*dpr));const ctx=out.getContext('2d');ctx.scale(dpr,dpr);ctx.fillStyle=document.body.classList.contains('light')?'#f6f8fc':'#070b13';ctx.fillRect(0,0,r.width,r.height);for(const c of canvases){try{ctx.drawImage(c,0,0,r.width,r.height)}catch(_){}}api.downloadUrl(out.toDataURL('image/png'),'PhxMindSetSQL_Obsidian_WebGL.png');api.toast('Grafo WebGL exportado em PNG HiDPI');return true;}

  return {rt,load,renderRelational,fitRelational,renderObsidian,zoomObsidian,fitObsidian,focusObsidian,refreshObsidian,exportObsidian,destroySigma,hasElk:()=>!!rt.elk,hasSigma:()=>!!(rt.Sigma&&rt.Graph)};
}

function fade(hex,alpha){if(/^#[0-9a-f]{6}$/i.test(hex||'')){const r=parseInt(hex.slice(1,3),16),g=parseInt(hex.slice(3,5),16),b=parseInt(hex.slice(5,7),16);return`rgba(${r},${g},${b},${alpha})`}return`rgba(120,140,170,${alpha})`}
