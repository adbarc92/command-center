// U3 (spec §3.3): insert/refresh a byte-0 YAML front-matter block in a STATUS.md body so the
// Project Dashboard's `local` source can read the project's canonical stage. Line-based (no YAML
// dependency — the format is controlled and small). Output is BOM-less. Managed keys: stage
// (required), readiness, updated. Any other existing keys (e.g. name, blocked) are preserved.

const FENCE = /^---\r?$/;

function setKey(arr, key, val) {
  if (val === undefined || val === null) return;
  const line = `${key}: ${JSON.stringify(String(val))}`;
  const re = new RegExp(`^${key}:\\s`);
  const idx = arr.findIndex((l) => re.test(l));
  if (idx >= 0) arr[idx] = line;
  else arr.push(line);
}

export function stampStatusFrontmatter(text, fields = {}) {
  const { stage, readiness, updated } = fields;
  if (!stage) throw new Error("stampStatusFrontmatter: stage is required");

  const body = text.charCodeAt(0) === 0xfeff ? text.slice(1) : text;
  const nl = body.includes("\r\n") ? "\r\n" : "\n";
  const lines = body.split(/\r?\n/);

  // Detect an existing byte-0 front-matter block.
  let end = -1;
  if (FENCE.test(lines[0] ?? "")) {
    for (let i = 1; i < lines.length; i++) {
      if (FENCE.test(lines[i])) { end = i; break; }
    }
  }

  if (end !== -1) {
    const inner = lines.slice(1, end);
    setKey(inner, "stage", stage);
    setKey(inner, "readiness", readiness);
    setKey(inner, "updated", updated);
    const rest = lines.slice(end + 1);
    return ["---", ...inner, "---", ...rest].join(nl);
  }

  // No block: build one and prepend. A leading H1 stays below it naturally.
  const inner = [];
  setKey(inner, "stage", stage);
  setKey(inner, "readiness", readiness);
  setKey(inner, "updated", updated);
  return ["---", ...inner, "---", ...lines].join(nl);
}
