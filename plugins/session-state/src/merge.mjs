function newest(records) {
  let best = null;
  for (const r of records) if (!best || (r.ts || "") > (best.ts || "")) best = r;
  return best;
}

export function resolveState(timeline, scratches) {
  const gitBearing = [...timeline, ...scratches].filter((r) => r.git);
  const freshest = newest(gitBearing);
  const rich = newest(timeline.filter((r) => r.type === "rich"));
  let branchBanner = null;
  if (freshest && rich && freshest.git && rich.git) {
    const fg = freshest.git, rg = rich.git;
    const suppress = fg.detached || fg.in_progress;
    if (!suppress && fg.branch && rg.branch && fg.branch !== rg.branch) {
      const wt = rg.worktree ? ` (worktree ${rg.worktree})` : "";
      branchBanner = `narrative captured on \`${rg.branch}\`${wt} — newest activity is on \`${fg.branch}\``;
    }
  }
  return { git: freshest ? freshest.git : null, gitSource: freshest, narrative: rich, branchBanner };
}

function fmtGit(g) {
  if (!g) return "_no git facts_";
  if (g.git_unavailable) return "_git binary unavailable_";
  if (g.detached || g.in_progress) return `HEAD ${g.head || "?"} (${g.in_progress || "detached"})`;
  let line = `branch \`${g.branch}\` @ ${g.head || "?"}`;
  if (g.dirty && g.dirty.length) line += ` — ${g.dirty.length} changed`;
  return line;
}

export function renderResumeBlock(timeline, scratches) {
  if (!timeline.length && !scratches.length) return null;
  const st = resolveState(timeline, scratches);
  const lines = ["<session-state>", "Resuming this repo — last known state:", fmtGit(st.git)];
  if (st.branchBanner) lines.push(`⚠ ${st.branchBanner}`);
  const n = st.narrative;
  if (n) {
    if (n.did) lines.push(`Last: ${n.did}`);
    for (const i of (n.next || []).slice(0, 5)) lines.push(`Next: ${i}`);
    for (const i of (n.open_threads || []).slice(0, 5)) lines.push(`Open: ${i}`);
  } else {
    lines.push("(no narrative captured yet — run /save-state to record one)");
  }
  lines.push("</session-state>");
  return lines.join("\n");
}

export function renderLatestMd(timeline, scratches) {
  const st = resolveState(timeline, scratches);
  const out = ["# Latest session state", "", `**Git:** ${fmtGit(st.git)}`, ""];
  if (st.branchBanner) out.push(`> ⚠ ${st.branchBanner}`, "");
  const n = st.narrative;
  if (n) {
    out.push(`**Last:** ${n.did || ""}`, "", "**Next:**", ...(n.next || []).map((i) => `- ${i}`),
      "", "**Open threads:**", ...(n.open_threads || []).map((i) => `- ${i}`));
  } else out.push("_No narrative captured yet._");
  out.push("", "## Recent history", "");
  for (const r of timeline.slice(-5).reverse()) out.push(`- \`${r.ts}\` ${r.type} (${r.source})`);
  return out.join("\n") + "\n";
}
