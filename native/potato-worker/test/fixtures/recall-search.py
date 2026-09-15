import json, unicodedata
from datetime import datetime
p=json.load(open('/home/user/recall-input.json'))
q=unicodedata.normalize('NFKC',p['query']).casefold().strip()
terms=q.split()
rows=[]
for c in p['conversations']:
 for i,m in enumerate(c['messages']):
  if p.get('start') and m['date'] < p['start']: continue
  if p.get('end') and m['date'] >= p['end']: continue
  text=unicodedata.normalize('NFKC',m['text']).casefold()
  score=10 if q and q in text else sum(1 for t in terms if t in text)
  if q and not score: continue
  rows.append(dict(m,conversation=c['id'],title=c['title'],revision=c['revision'],score=score))
rows.sort(key=lambda x:(x['score'],x['date']),reverse=True)
o=p.get('offset',0)
print(json.dumps({'sources':[{**m,'text':m['text'][:4000]} for m in rows[o:o+8]],'more':len(rows)>o+8},ensure_ascii=False))
