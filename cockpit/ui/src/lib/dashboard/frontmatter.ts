// U1 (spec §3.1): parse a byte-0 YAML front-matter block from STATUS.md.
// Robust to BOM, CRLF, and the Session-log `---` horizontal-rule collision.
import { load } from 'js-yaml';

export interface StatusMarker {
  present: boolean;
  stage?: string;
  readiness?: string;
  updated?: string;
  blocked?: string;
  name?: string;
  baseBranch?: string;
  testCmd?: string;
}

const str = (v: unknown): string | undefined =>
  v == null ? undefined : v instanceof Date ? v.toISOString().slice(0, 10) : String(v);

export function parseStatusFrontmatter(text: string): StatusMarker {
  // Strip a leading UTF-8 BOM so the byte-0 fence check holds on Windows-written files.
  const body = text.charCodeAt(0) === 0xfeff ? text.slice(1) : text;
  const lines = body.split('\n');
  // Front-matter exists only if line 1 is exactly a fence (CRLF-tolerant).
  if (!/^---\r?$/.test(lines[0] ?? '')) return { present: false };
  // Closing fence = the next `---` line; a later `---` is a Session-log rule, not a fence.
  let end = -1;
  for (let i = 1; i < lines.length; i++) {
    if (/^---\r?$/.test(lines[i])) {
      end = i;
      break;
    }
  }
  if (end === -1) return { present: false };

  const yaml = lines.slice(1, end).join('\n');
  let doc: Record<string, unknown>;
  try {
    doc = (load(yaml) as Record<string, unknown>) ?? {};
  } catch {
    return { present: true }; // malformed YAML: block existed but yielded no fields
  }
  return {
    present: true,
    stage: str(doc.stage),
    readiness: str(doc.readiness),
    updated: str(doc.updated),
    blocked: str(doc.blocked),
    name: str(doc.name),
    baseBranch: str(doc.base_branch),
    testCmd: str(doc.test_cmd),
  };
}
