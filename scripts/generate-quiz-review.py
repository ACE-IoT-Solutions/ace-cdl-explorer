#!/usr/bin/env python3
"""Generate the manual-review tables from the quiz's single question bank."""
import argparse
import csv
import html
import io
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

def outputs():
    questions = json.loads((ROOT / 'web/src/data/cdlQuizQuestions.json').read_text())
    fields = ['ID', 'Topic', 'Level', 'Question', 'A', 'B', 'C', 'D', 'Correct answer', 'Explanation', 'Reference', 'Source section / symbol', 'Source basis', 'Review status', 'Reviewer notes']
    rows = []
    for q in questions:
        options = {o['id']: o['text'] for o in q['options']}
        rows.append([q['id'], q['topic'], q['difficulty'], q['prompt'], *[options[k] for k in 'ABCD'], f"{q['correctOptionId']}: {options[q['correctOptionId']]}", q['explanation'], q['reference'], q['referenceSection'], q['referenceBasis'], 'Pending', ''])
    stream = io.StringIO(newline='')
    writer = csv.writer(stream, lineterminator='\n')
    writer.writerow(fields)
    writer.writerows(rows)
    header = ''.join(f'<th scope="col">{html.escape(c)}</th>' for c in fields)
    def cell(value, column):
        escaped = html.escape(str(value), quote=True)
        if fields[column] == 'Reference':
            return f'<td><a href="{escaped}" target="_blank" rel="noreferrer">Open source ↗</a></td>'
        return f'<td>{escaped}</td>'
    body = ''.join('<tr>' + ''.join(cell(value, column) for column, value in enumerate(row)) + '</tr>' for row in rows)
    page = f'''<!doctype html>
<html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>ASHRAE Standard 231 competency — 50-question review table</title>
<style>body{{font:15px/1.5 system-ui;margin:2rem;color:#17202a}}h1{{font-size:2rem}}.table-wrap{{overflow:auto;max-height:80vh;border:1px solid #aaa}}table{{border-collapse:collapse;min-width:2400px}}th,td{{border:1px solid #ccd1d1;padding:.7rem;text-align:left;vertical-align:top;min-width:120px}}th{{position:sticky;top:0;background:#e8f4f2}}td:nth-child(4),td:nth-child(10){{min-width:300px}}tr:nth-child(even){{background:#f5f7f9}}@media print{{.table-wrap{{max-height:none;overflow:visible}}th{{position:static}}}}</style>
<h1>ASHRAE Standard 231 competency: question bank review</h1>
<p>50 original questions about Standard 231 language and elementary-block semantics. Each has one correct answer. Source-clause verification against the published standard remains pending; this table supports ongoing editorial review.</p>
<p>Use <a href="quiz-question-review.csv">the CSV copy</a> to record feedback. Edit <code>web/src/data/cdlQuizQuestions.json</code> to apply approved wording or answer changes, and increment the item's revision when its meaning or answer changes. Regeneration replaces the review tables; retain annotated review copies separately.</p>
<p>Target edition: ANSI/ASHRAE Standard 231-2026. I/O is abstract and typed; neither Brick nor ASHRAE 223 is required for I/O modeling. App behavior, weather, checkpoints, and vendor implementation limits are outside this bank.</p>
<p><a href="https://data.ashrae.org/standard231/">ASHRAE’s resource page</a> identifies Buildings 12.1.0 as the basis of the normative elementary-block definitions. Block references are pinned to that version. OBC language documentation is an informal clarification, and its section numbers are not Standard 231 clause numbers. Modelica references corroborate shared syntax only. The published standard’s full text was not accessed; preview definitions and public resources support this draft. See <a href="quiz-source-audit.json">source retrieval details</a>. These sources support review, not a certification claim.</p>
<div class="table-wrap"><table><caption>Questions, answer choices and answer key</caption><thead><tr>{header}</tr></thead><tbody>{body}</tbody></table></div></html>\n'''
    return {'quiz-question-review.csv': stream.getvalue(), 'quiz-question-review.html': page}

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    for name, content in outputs().items():
        path = ROOT / 'docs' / name
        if args.check:
            if not path.exists() or path.read_text() != content:
                raise SystemExit(f'{path}: stale; run python3 scripts/generate-quiz-review.py')
        else:
            path.write_text(content)

if __name__ == '__main__':
    main()
