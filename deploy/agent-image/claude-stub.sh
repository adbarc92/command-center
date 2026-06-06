#!/bin/sh
# Deterministic `claude` stand-in for $0 driver validation. It inspects the -p
# prompt, performs the file edits a real agent would, and emits a stream-json
# `result` record so the daemon's cost meter has something to parse. Not for
# production — only swapped in via Dockerfile.stub for pipeline testing.

prompt=""
while [ $# -gt 0 ]; do
  case "$1" in
    -p) prompt="$2"; shift 2 ;;
    *) shift ;;
  esac
done

case "$prompt" in
  *"test oracle"*)
    # Oracle: write the test that defines done.
    cat > sum.test.js <<'JS'
const test = require('node:test');
const assert = require('node:assert');
const { sum } = require('./src/index.js');
test('sum adds two numbers', () => assert.strictEqual(sum(2, 3), 5));
JS
    echo "oracle: wrote sum.test.js"
    ;;
  *"Implement the task"*)
    # Builder: implement so the frozen test passes.
    mkdir -p src
    printf 'module.exports.sum = (a, b) => a + b;\n' > src/index.js
    echo "build: wrote src/index.js"
    ;;
  *"Review the current"*)
    echo "review: implementation matches the test; no issues"
    echo "BLOCKERS=0"
    ;;
esac

# stream-json result record (shape from spikes/SPIKE-RESULTS.md)
echo '{"type":"result","subtype":"success","is_error":false,"total_cost_usd":0.01,"usage":{"input_tokens":120,"output_tokens":18}}'
exit 0
