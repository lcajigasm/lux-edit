#!/usr/bin/env bash
set -euo pipefail

last_tag="$(git describe --tags --abbrev=0 2>/dev/null || true)"

if [[ -z "${last_tag}" ]]; then
  echo "# Changelog"
  echo
  git log --pretty=format:"- %h %s (%an)" -n 100
  exit 0
fi

echo "# Changelog since ${last_tag}"
echo
git log "${last_tag}..HEAD" --pretty=format:"- %h %s (%an)"
