#!/usr/bin/env bash
# Render every Mermaid diagram in docs/diagrams to an SVG next to it.
# Requires @mermaid-js/mermaid-cli (mmdc). If it is not installed the script
# prints how to get it and exits nonzero so CI can treat missing rendering as a
# real gap rather than silently passing.
set -euo pipefail
cd "$(dirname "$0")/.."

if ! command -v mmdc >/dev/null 2>&1; then
  echo "mmdc (mermaid-cli) not found."
  echo "Install: npm install -g @mermaid-js/mermaid-cli"
  exit 1
fi

shopt -s nullglob
for f in docs/diagrams/*.mmd; do
  out="${f%.mmd}.svg"
  echo "rendering $f -> $out"
  mmdc -i "$f" -o "$out" -b transparent
done
echo "done"
