import { describe, it, expect } from 'vitest';
import { parseRoadmapItems } from './roadmap';

describe('parseRoadmapItems', () => {
  it('parses a tagged heading into an item, resolving the Dispatch body', () => {
    const md = [
      '## Cache timer',
      '<!-- cc-item id=cache-timer status=done tier=t2 lane=workflow -->',
      'Some prose.',
      '',
      '**Dispatch:** Do the thing. Acceptance: it works.',
    ].join('\n');
    const { items } = parseRoadmapItems(md);
    expect(items).toHaveLength(1);
    expect(items[0]).toMatchObject({
      id: 'cache-timer', title: 'Cache timer', status: 'done', tier: 't2',
      lane: 'workflow', dispatchable: true,
    });
    expect(items[0].task).toContain('Do the thing');
  });

  it('IGNORES a cc-item inside a fenced code block', () => {
    const md = [
      '## Docs',
      'How the format works:',
      '```',
      '## Example',
      '<!-- cc-item id=example status=open tier=t1 -->',
      '```',
    ].join('\n');
    expect(parseRoadmapItems(md).items).toHaveLength(0);
  });

  it('defaults tier to t1 and falls back task→title when no Dispatch/prose', () => {
    const md = '## Bare item\n<!-- cc-item id=bare status=open -->\n';
    const { items } = parseRoadmapItems(md);
    expect(items[0].tier).toBe('t1');
    expect(items[0].task).toBe('Bare item');
  });

  it('marks an item missing id as non-dispatchable and warns', () => {
    const md = '## No id\n<!-- cc-item status=open tier=t1 -->\n';
    const { items, warnings } = parseRoadmapItems(md);
    expect(items[0].dispatchable).toBe(false);
    expect(warnings.join(' ')).toMatch(/missing id/i);
  });

  it('marks duplicate ids non-dispatchable and warns', () => {
    const md = [
      '## A', '<!-- cc-item id=dup status=open -->',
      '## B', '<!-- cc-item id=dup status=open -->',
    ].join('\n');
    const { items, warnings } = parseRoadmapItems(md);
    expect(items.every((i) => !i.dispatchable)).toBe(true);
    expect(warnings.join(' ')).toMatch(/duplicate id/i);
  });
});
