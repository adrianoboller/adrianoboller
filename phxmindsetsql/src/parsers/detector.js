const DIALECTS = ['postgresql','mysql','sqlite','sqlserver'];

const rules = {
  postgresql: [
    [/\bLANGUAGE\s+plpgsql\b/gi, 7], [/\bCREATE\s+POLICY\b/gi, 6], [/\bJSONB\b/gi, 4],
    [/\bUUID\b/gi, 2], [/\bRETURNING\b/gi, 2], [/\bILIKE\b/gi, 3], [/::[A-Za-z_][\w$]*/g, 3],
    [/\$[A-Za-z_]*\$/g, 2], [/\bCREATE\s+EXTENSION\b/gi, 5], [/\bSET\s+search_path\b/gi, 5],
  ],
  mysql: [
    [/\bENGINE\s*=\s*(?:InnoDB|MyISAM)\b/gi, 7], [/\bAUTO_INCREMENT\b/gi, 6], [/\bUNSIGNED\b/gi, 4],
    [/\bDELIMITER\b/gi, 5], [/`[^`]+`/g, 2], [/\bTINYINT\s*\(\s*1\s*\)/gi, 3],
  ],
  sqlite: [
    [/\bPRAGMA\b/gi, 7], [/\bAUTOINCREMENT\b/gi, 5], [/\bWITHOUT\s+ROWID\b/gi, 7],
    [/\bSTRICT\s*;/gi, 3], [/\bsqlite_(?:master|schema|sequence)\b/gi, 6],
  ],
  sqlserver: [
    [/\bIDENTITY\s*\(/gi, 6], [/\bNVARCHAR\b/gi, 3], [/\bCLUSTERED\b/gi, 4],
    [/\bCREATE\s+OR\s+ALTER\b/gi, 5], [/\bGO\s*$/gim, 4], [/\[[A-Za-z_][\w$]*\]/g, 1],
    [/\bUNIQUEIDENTIFIER\b/gi, 5], [/\bGETDATE\s*\(/gi, 3],
  ],
};

export function rankDialects(sql='') {
  const text = String(sql);
  const ranked = DIALECTS.map(dialect => {
    let score = 0;
    for (const [re, weight] of rules[dialect]) {
      re.lastIndex = 0;
      let count = 0;
      while (re.exec(text) && count < 12) count++;
      score += count * weight;
    }
    return {dialect, score};
  }).sort((a,b)=>b.score-a.score);

  const genericSql = /\bCREATE\s+TABLE\b/i.test(text) || /\bALTER\s+TABLE\b/i.test(text);
  if (ranked[0].score === 0 && genericSql) ranked[0] = {dialect:'postgresql', score:1};
  return ranked;
}

export function detectDialect(sql='') {
  const ranked = rankDialects(sql);
  const best = ranked[0] || {dialect:'unknown',score:0};
  const second = ranked[1] || {score:0};
  const confidence = best.score <= 0 ? 0 : Math.max(0.35, Math.min(0.99, best.score / Math.max(best.score + second.score, 1)));
  return {dialect: best.score > 0 ? best.dialect : 'unknown', confidence, ranked};
}
