# Cache-window pacing (Roadmap 6E)

The Anthropic prompt cache has a **~5-minute TTL** (300s; the ticker counts down
295s, leaving a 5s safety margin). Every uncached re-read of a large session
prefix is billed at the full input rate instead of the ~0.1x cache-read rate —
so *when* work happens relative to that 5-minute window is itself a cost lever.
This is the standing rule that pairs the cache-aware approval timer (item 1)
and the rate-limit auto-retry (item 5) with proactive budget discipline.

## The rule: warm while working, cool while idle, retry within the window

1. **Keep the cache warm during active work.**
   While a task is in flight, keep turns landing **inside the 5-minute window**.
   Each turn that reuses the cached prefix re-warms it for another ~5 minutes at
   the cheap cache-read rate. Don't let a long, silent tool-execution stretch
   (or a slow human review) drift past 5 minutes mid-task — that forces a cold,
   full-price re-read of the whole session on the next turn. When a task is
   **awaiting user approval**, the item-1 ticker makes this visible: respond
   while it's `🟢/🟡`, before `🔴 → ❄️ COLD`.

2. **Let the cache cool when genuinely idle.**
   If there is no real next turn coming — the work is done, you're blocked on an
   external dependency, or you're deliberately parking the session — **do not**
   burn writes keeping a cache warm you won't read. A cache-write costs ~1.25x
   (5-min TTL); pre-warming or re-warming a prefix you won't touch within the TTL
   is pure waste. Let it expire. The break-even is low (about two reads pays back
   one write at the 5-min TTL), but "two reads" only happens if you actually come
   back inside the window.

3. **Retry within the window.**
   When the API returns a transient 429 ("Server is temporarily limiting
   requests"), back off and retry **inside** the remaining cache TTL where
   possible, so the retry is served from the still-warm cache rather than forcing
   a cold re-read. Cap backoff so cumulative wait stays under the ~5-minute TTL;
   if a backoff would push past it, the cache is already lost — there's no premium
   on waiting a bit longer at that point. (This is the explicit coordination
   point with item 5.)

4. **Coordinate `ScheduleWakeup` / work cadence with the TTL.**
   For scheduled or self-paced work, align the wake cadence to the cache window:
   - If the next scheduled step is **< ~5 minutes** away, the cache is still warm
     on wake — schedule the step and let it read from cache.
   - If the next step is **> ~5 minutes** away, the cache will be cold regardless;
     don't schedule an interim "keep-warm" ping just to hold it. Either pull the
     work forward to land inside one window, or accept the cold re-read and batch
     enough work after the wake to amortise it.
   - For **bursty** work with long idle gaps, prefer batching related turns into a
     single warm window over spreading them across several cold ones.

## Why it pairs with items 1 and 5

- **Item 1 (cache-aware approval timer)** is the *human-in-the-loop* face of rule
  1: the live `🔥 → 🟢 → 🟡 → 🔴 → ❄️` countdown plus cost-at-stake tells the user
  exactly how much money the next approval is racing against, so they respond
  before the window closes.
- **Item 5 (rate-limit auto-retry)** is the *machine* face of rule 3: retries are
  scheduled to stay inside the window so a transient 429 doesn't cascade into a
  cold, full-cost re-read.

## Cost basis (for the "is it worth keeping warm?" call)

Using the Command Center's default model (Claude Opus 4.8; see the `claude-api`
skill model table):

| | per 1M input tokens | per token |
|---|---|---|
| Full input (cold re-read) | $5.00 | $0.000005 |
| Cache read (~0.1x) | $0.50 | $0.0000005 |
| Cache write (~1.25x, 5-min TTL) | $6.25 | $0.00000625 |

**Cost at stake when the cache goes cold** = re-reading the cached prefix at full
price instead of the cache-read price:

    at_stake = cached_tokens x ($0.000005 - $0.0000005)
             = cached_tokens x $0.0000045        # = $4.50 per 1M cached tokens

That `$4.50 / 1M` figure is exactly what the item-1 ticker renders as
cost-at-stake (e.g. ~1.28M cached tokens → ~`$5.75`). If the default model
changes, update both the rates here and `COST_PER_TOKEN_AT_STAKE` in
`src/cache_countdown/core.py`.
