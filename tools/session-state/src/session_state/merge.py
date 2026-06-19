"""merge.py — branch-scoped read model: freshest git facts + latest narrative (Spec §5)."""
from __future__ import annotations


def _newest(records: list[dict]) -> dict | None:
    return max(records, key=lambda r: r.get("ts", ""), default=None)


def resolve_state(timeline: list[dict], scratches: list[dict]) -> dict:
    git_bearing = [r for r in (timeline + scratches) if r.get("git")]
    freshest = _newest(git_bearing)
    rich = _newest([r for r in timeline if r.get("type") == "rich"])

    banner = None
    if freshest and rich and freshest.get("git") and rich.get("git"):
        fg, rg = freshest["git"], rich["git"]
        suppress = fg.get("detached") or fg.get("in_progress")
        if not suppress and fg.get("branch") and rg.get("branch") and fg["branch"] != rg["branch"]:
            wt = f" (worktree {rg['worktree']})" if rg.get("worktree") else ""
            banner = (f"narrative captured on `{rg['branch']}`{wt} — "
                      f"newest activity is on `{fg['branch']}`")
    return {
        "git": freshest["git"] if freshest else None,
        "git_source": freshest,
        "narrative": rich,
        "branch_banner": banner,
    }


def _fmt_git(g: dict | None) -> str:
    if not g:
        return "_no git facts_"
    if g.get("git_unavailable"):
        return "_git binary unavailable_"
    if g.get("detached") or g.get("in_progress"):
        op = g.get("in_progress") or "detached"
        return f"HEAD {g.get('head', '?')} ({op})"
    line = f"branch `{g.get('branch')}` @ {g.get('head', '?')}"
    if g.get("dirty"):
        line += f" — {len(g['dirty'])} changed"
    return line


def render_resume_block(timeline: list[dict], scratches: list[dict]) -> str | None:
    if not timeline and not scratches:
        return None
    st = resolve_state(timeline, scratches)
    lines = ["<session-state>", "Resuming this repo — last known state:", _fmt_git(st["git"])]
    if st["branch_banner"]:
        lines.append(f"⚠ {st['branch_banner']}")
    n = st["narrative"]
    if n:
        if n.get("did"):
            lines.append(f"Last: {n['did']}")
        for item in (n.get("next") or [])[:5]:
            lines.append(f"Next: {item}")
        for item in (n.get("open_threads") or [])[:5]:
            lines.append(f"Open: {item}")
    else:
        lines.append("(no narrative captured yet — run /save-state to record one)")
    lines.append("</session-state>")
    return "\n".join(lines)


def render_latest_md(timeline: list[dict], scratches: list[dict]) -> str:
    st = resolve_state(timeline, scratches)
    out = ["# Latest session state", "", f"**Git:** {_fmt_git(st['git'])}", ""]
    if st["branch_banner"]:
        out += [f"> ⚠ {st['branch_banner']}", ""]
    n = st["narrative"]
    if n:
        out += [f"**Last:** {n.get('did','')}", "", "**Next:**"]
        out += [f"- {i}" for i in (n.get("next") or [])]
        out += ["", "**Open threads:**"]
        out += [f"- {i}" for i in (n.get("open_threads") or [])]
    else:
        out.append("_No narrative captured yet._")
    out += ["", "## Recent history", ""]
    for r in reversed(timeline[-5:]):
        out.append(f"- `{r.get('ts')}` {r.get('type')} ({r.get('source')})")
    return "\n".join(out) + "\n"
