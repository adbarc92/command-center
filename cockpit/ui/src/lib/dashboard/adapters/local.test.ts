import { describe, it, expect } from 'vitest';
import { localCards, type LocalProjectDoc, type LocalReader } from './local';

const NOW = () => new Date('2026-07-10T00:00:00Z');
const reader = (docs: LocalProjectDoc[]): LocalReader => ({ scan: async () => docs });

describe('localCards', () => {
  it('emits a declared card for a valid marker, fully resolved (no override)', async () => {
    const cards = await localCards(
      reader([{ projectDir: 'D:/proj/alpha', statusText: '---\nstage: Build\nreadiness: "85%"\nupdated: "2026-07-06"\n---\n', isPinned: false }]),
      { now: NOW },
    );
    expect(cards).toHaveLength(1);
    expect(cards[0]).toMatchObject({
      projectId: 'local:D--proj-alpha', source: 'local', stage: 'Build',
      stageSource: 'declared', override: null, conflict: null, health: 'ok',
    });
    expect(cards[0].detail).toContain('85%');
  });

  it('does NOT emit a card for an auto-discovered project with no marker', async () => {
    const cards = await localCards(reader([{ projectDir: 'D:/proj/beta', statusText: '# no frontmatter\n', isPinned: false }]), { now: NOW });
    expect(cards).toHaveLength(0);
  });

  it('emits a degraded card for a PINNED project with no marker', async () => {
    const cards = await localCards(reader([{ projectDir: 'D:/proj/beta', statusText: undefined, isPinned: true }]), { now: NOW });
    expect(cards).toHaveLength(1);
    expect(cards[0]).toMatchObject({ health: 'unknown', stage: 'Idle' });
    expect(cards[0].detail).toMatch(/no STATUS marker/i);
  });

  it('emits a degraded card for an invalid stage value', async () => {
    const cards = await localCards(reader([{ projectDir: 'D:/proj/g', statusText: '---\nstage: Bogus\n---\n', isPinned: false }]), { now: NOW });
    expect(cards[0]).toMatchObject({ health: 'unknown' });
    expect(cards[0].detail).toMatch(/invalid stage: Bogus/);
  });

  it('attaches parsed roadmap items and an item count detail', async () => {
    const cards = await localCards(
      reader([{
        projectDir: 'D:/proj/alpha',
        statusText: '---\nstage: Build\n---\n',
        roadmapText: '## A\n<!-- cc-item id=a status=open tier=t1 -->\n## B\n<!-- cc-item id=b status=done -->\n',
        isPinned: false,
      }]),
      { now: NOW },
    );
    expect(cards[0].dispatch?.items).toHaveLength(2);
    expect(cards[0].detail).toMatch(/2 tagged · 1 open/);
  });

  it('degrades every lane to unknown if the reader throws', async () => {
    const cards = await localCards({ scan: async () => { throw new Error('scan boom'); } }, { now: NOW });
    expect(cards).toHaveLength(1);
    expect(cards[0]).toMatchObject({ source: 'local', health: 'unknown' });
    expect(cards[0].detail).toMatch(/scan boom/);
  });
});
