"""Run from the repository root:
python3 native/potato-worker/test/fixtures/generate-recall-expected.py

recall-search.py is the original RECALL_SEARCH_CODE, unchanged. Redirect only
its input open() to the corpus; drop score at the output boundary as required
by P1. All matching, ordering, pagination and truncation remain Python's.
"""
import contextlib
import io
import json
from pathlib import Path

root = Path(__file__).parent
corpus = json.loads((root / 'recall-corpus.json').read_text())
code = compile((root / 'recall-search.py').read_text(), 'recall-search.py', 'exec')
results = []
for case in corpus['queries']:
    payload = dict(case['input'], conversations=corpus['conversations'])
    def input_open(path):
        assert path == '/home/user/recall-input.json'
        return io.StringIO(json.dumps(payload, ensure_ascii=False))
    output = io.StringIO()
    with contextlib.redirect_stdout(output):
        exec(code, {'open': input_open})
    result = json.loads(output.getvalue())
    for source in result['sources']:
        del source['score']
    results.append(dict(name=case['name'], result=result))
(root / 'recall-expected.json').write_text(json.dumps(results, ensure_ascii=False, indent=2) + '\n')
print(f'Generated {len(results)} Python reference results')
