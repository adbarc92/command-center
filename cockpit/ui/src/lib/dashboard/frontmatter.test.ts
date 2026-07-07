import { describe, it, expect } from 'vitest';
import { parseStatusFrontmatter } from './frontmatter';

describe('parseStatusFrontmatter', () => {
  it('parses a valid byte-0 block', () => {
    const m = parseStatusFrontmatter('---\nstage: Build\nreadiness: "85%"\nupdated: "2026-07-06"\n---\n# Title\n');
    expect(m.present).toBe(true);
    expect(m.stage).toBe('Build');
    expect(m.readiness).toBe('85%');
    expect(m.updated).toBe('2026-07-06');
  });

  it('strips a leading UTF-8 BOM', () => {
    const m = parseStatusFrontmatter('﻿---\nstage: Ship\n---\n');
    expect(m.present).toBe(true);
    expect(m.stage).toBe('Ship');
  });

  it('tolerates CRLF fences', () => {
    const m = parseStatusFrontmatter('---\r\nstage: Plan\r\n---\r\n# T\r\n');
    expect(m.stage).toBe('Plan');
  });

  it('is absent when line 1 is not a fence (no false front-matter from a Session-log rule)', () => {
    const m = parseStatusFrontmatter('# STATUS\n\nsome prose\n\n---\n\n## Session 1\n');
    expect(m.present).toBe(false);
    expect(m.stage).toBeUndefined();
  });

  it('coerces an unquoted date to a string (not a Date)', () => {
    const m = parseStatusFrontmatter('---\nstage: Build\nupdated: 2026-07-06\n---\n');
    expect(typeof m.updated).toBe('string');
    expect(m.updated).toBe('2026-07-06');
  });

  it('parses baseBranch and testCmd (Phase-2 fields) when present', () => {
    const m = parseStatusFrontmatter('---\nstage: Build\nbase_branch: "main"\ntest_cmd: "cargo test"\n---\n');
    expect(m.baseBranch).toBe('main');
    expect(m.testCmd).toBe('cargo test');
  });
});
