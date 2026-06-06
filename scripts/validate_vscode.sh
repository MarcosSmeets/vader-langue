#!/usr/bin/env bash
cd /mnt/c/Users/marco/Documents/workspace/side_projects/vader/editors/vscode || exit 1
for f in package.json language-configuration.json syntaxes/vader.tmLanguage.json; do
  if python3 -c "import json,sys; json.load(open(sys.argv[1]))" "$f" 2>/dev/null; then
    echo "OK  $f"
  else
    echo "BAD $f"
    python3 -c "import json,sys; json.load(open(sys.argv[1]))" "$f"
  fi
done