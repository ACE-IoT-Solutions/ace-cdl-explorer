#!/usr/bin/env python3
"""Adapt pinned Buildings CDL Icon annotations to ACE SVG symbols.

The vendored annotations retain upstream geometry. Only the static icon layer is
rendered: names, external parameter captions and DynamicSelect state are omitted.
Colors, stroke weights and type are supplied by the ACE design system.
See THIRD_PARTY_NOTICES.md for the upstream license.
"""
import argparse
import html
import json
import math
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]
SNAPSHOT = ROOT / 'scripts/data/modelica-block-icons.json'
OUTPUT = ROOT / 'web/src/lib/blockSymbolData.ts'


def split_top(text, delimiter=','):
    parts, start, depth, quoted, escaped = [], 0, 0, False, False
    for i, char in enumerate(text):
        if quoted:
            if escaped:
                escaped = False
            elif char == '\\':
                escaped = True
            elif char == '"':
                quoted = False
        elif char == '"':
            quoted = True
        elif char in '({[':
            depth += 1
        elif char in ')}]':
            depth -= 1
        elif char == delimiter and depth == 0:
            parts.append(text[start:i].strip())
            start = i + 1
    return parts + [text[start:].strip()]


def args(text):
    return dict(part.split('=', 1) for part in split_top(text) if '=' in part)


def number(value):
    return f'{value:.3f}'.rstrip('0').rstrip('.') if value else '0'


def pairs(value):
    # Expressions and DynamicSelect geometry are intentionally excluded.
    if not re.fullmatch(r'[\s{},.\-+\deE]+', value):
        return []
    values = [float(n) for n in re.findall(r'[-+]?(?:\d*\.)?\d+(?:[eE][-+]?\d+)?', value)]
    return list(zip(values[::2], values[1::2]))


def primitive(item):
    match = re.fullmatch(r'(\w+)\s*\((.*)\)', item, re.S)
    if not match:
        raise ValueError(f'Unknown icon primitive: {item[:80]}')
    kind, raw = match.groups()
    a = {key.strip(): value.strip() for key, value in args(raw).items()}
    if a.get('visible') == 'false':
        return ''
    # PID variants share a label position; show the library default PI only.
    if 'controllerType ==' in a.get('visible', '') and not a['visible'].endswith('.PI)'):
        return ''
    extent = pairs(a.get('extent', ''))
    points = pairs(a.get('points', ''))
    color = a.get('color', a.get('lineColor', ''))
    muted = color in ('{192,192,192}', '{175,175,175}')
    stroke = 'var(--block-symbol-muted)' if muted else 'currentColor'
    fill = 'none'
    if a.get('fillPattern') == 'FillPattern.Solid':
        fill_color = a.get('fillColor', '')
        fill = 'var(--block-symbol-surface)' if fill_color in ('{255,255,255}', '{210,210,210}', '{223,211,169}') else stroke
    attrs = f' fill="{fill}" stroke="{stroke}"'
    if a.get('pattern') == 'LinePattern.None' or a.get('lineColor') == '{255,255,255}':
        attrs += ' stroke-opacity="0"'
    if a.get('pattern') in ('LinePattern.Dash', 'LinePattern.Dot'):
        attrs += ' stroke-dasharray="10 8"'
    origin = pairs('{' + a.get('origin', '') + '}')
    transform = ''
    if origin or a.get('rotation'):
        x, y = origin[0] if origin else (0, 0)
        transform = f' transform="translate({number(x)} {number(-y)}) rotate({number(-float(a.get("rotation", 0)))})"'
    result = ''
    if kind in ('Line', 'Polygon') and points:
        coords = ' '.join(f'{number(x)},{number(-y)}' for x, y in points)
        tag = 'polygon' if kind == 'Polygon' else 'polyline'
        result = f'<{tag} points="{coords}"{attrs}/>'
        if kind == 'Line' and a.get('smooth') == 'Smooth.Bezier' and len(points) > 2:
            path = f'M{number(points[0][0])} {number(-points[0][1])}'
            for i in range(1, len(points)-1):
                x, y = points[i]
                nx, ny = points[i+1]
                end = (nx, ny) if i == len(points)-2 else ((x+nx)/2, (y+ny)/2)
                path += f' Q{number(x)} {number(-y)} {number(end[0])} {number(-end[1])}'
            result = f'<path d="{path}"{attrs}/>'
        if kind == 'Line' and 'Arrow.Filled' in a.get('arrow', '') and len(points) > 1:
            # Arrowheads are local geometry, avoiding duplicate SVG marker IDs.
            for index, other in [(0, 1), (-1, -2)]:
                arrows = a['arrow'].strip('{}').split(',')
                if arrows[0 if index == 0 else -1].strip() != 'Arrow.Filled':
                    continue
                x, y = points[index]
                px, py = points[other]
                angle = math.atan2(y-py, x-px)
                head = [(x, y)] + [(x-14*math.cos(angle)+side*6*math.sin(angle), y-14*math.sin(angle)-side*6*math.cos(angle)) for side in (-1, 1)]
                coords = ' '.join(f'{number(x)},{number(-y)}' for x, y in head)
                result += f'<polygon points="{coords}" fill="{stroke}" stroke="none"/>'
    elif kind in ('Rectangle', 'Ellipse') and len(extent) == 2:
        (x1, y1), (x2, y2) = extent
        if kind == 'Rectangle':
            radius = min(8, float(a.get('radius', '0')))
            result = f'<rect x="{number(min(x1,x2))}" y="{number(-max(y1,y2))}" width="{number(abs(x2-x1))}" height="{number(abs(y2-y1))}" rx="{number(radius)}"{attrs}/>'
        else:
            # DynamicSelect LEDs are state decoration, not live values here.
            if 'DynamicSelect' in a.get('fillColor', ''):
                return ''
            result = f'<ellipse cx="{number((x1+x2)/2)}" cy="{number(-(y1+y2)/2)}" rx="{number(abs(x2-x1)/2)}" ry="{number(abs(y2-y1)/2)}"{attrs}/>'
    elif kind == 'Text' and len(extent) == 2:
        raw_text = a.get('textString', '')
        if not raw_text.startswith('"'):
            return ''
        text = json.loads(raw_text)
        if '%' in text or not text.strip():
            return ''
        (x1, y1), (x2, y2) = extent
        if abs((y1+y2)/2) > 100 or min(x1,x2) < -110 or max(x1,x2) > 110:
            return ''
        lines = text.split('\n')
        size = min(70, abs(y2-y1)*0.85/max(1,len(lines)), abs(x2-x1)*1.7/max(map(len,lines)))
        x, y = (x1+x2)/2, -(y1+y2)/2
        spans = ''.join(f'<tspan x="{number(x)}" dy="{number(0 if i == 0 else size*1.1)}">{html.escape(line)}</tspan>' for i,line in enumerate(lines))
        result = f'<text x="{number(x)}" y="{number(y-(len(lines)-1)*size*0.55)}" font-size="{number(size)}" fill="currentColor" stroke="none" text-anchor="middle" dominant-baseline="central">{spans}</text>'
    elif kind == 'Bitmap' and a.get('fileName') == '"modelica://Buildings/Resources/Images/Controls/OBC/CDL/Reals/int.png"':
        # The upstream bitmap is an integral sign; use the ACE font equivalent.
        result = '<text x="0" y="0" font-size="110" fill="currentColor" stroke="none" text-anchor="middle" dominant-baseline="central">∫</text>'
    elif kind not in ('Line', 'Polygon', 'Rectangle', 'Ellipse', 'Text'):
        raise ValueError(f'Unsupported primitive: {kind}')
    return f'<g{transform}>{result}</g>' if transform and result else result


def render(annotation):
    a = {key.strip(): value.strip() for key,value in args(annotation[annotation.index('(')+1:-1]).items()}
    graphics = a['graphics'].strip()[1:-1]
    result = ''.join(primitive(item) for item in split_top(graphics))
    if not result:
        raise ValueError('Empty symbol')
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    options = parser.parse_args()
    snapshot = json.loads(SNAPSHOT.read_text())
    symbols = {key: render(value) for key,value in sorted(snapshot['sources'].items())}
    output = '// Generated by scripts/generate-block-symbols.py. Do not edit.\n'
    output += '// Adapted from Buildings CDL Icon annotations; see THIRD_PARTY_NOTICES.md.\n'
    output += f'export const blockSymbolRevision = {json.dumps(snapshot["revision"])}\n'
    output += 'export const blockSymbols: Record<string, string> = ' + json.dumps(symbols, ensure_ascii=False, indent=2) + '\n'
    if options.check:
        if not OUTPUT.exists() or OUTPUT.read_text() != output:
            raise SystemExit('Block symbols are stale; run python3 scripts/generate-block-symbols.py')
    else:
        OUTPUT.write_text(output)
    print(f'{len(symbols)} Buildings symbols {"checked" if options.check else "generated"}')


if __name__ == '__main__':
    main()
