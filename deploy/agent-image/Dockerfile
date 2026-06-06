# Base image for SP1 fleet agents: Claude Code CLI + git + a Node toolchain.
# Network is open at runtime (the container is a host-damage boundary, not a
# network jail), so project dependencies install normally during build/check.
#
# Dev tag: cc-agent:dev. Production pins this by digest (SP1 spec, Section 4).
FROM node:22-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends git ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Claude Code CLI. Pinned loosely for dev; production pins an exact version.
RUN npm install -g @anthropic-ai/claude-code

# Run as the non-root `node` user: claude --dangerously-skip-permissions refuses
# to run as root. The named volume at /work inherits this ownership on first use.
RUN mkdir -p /work && chown -R node:node /work
USER node
WORKDIR /work

# Long-lived no-op so the daemon can `docker exec` discrete steps into it.
CMD ["sleep", "infinity"]
