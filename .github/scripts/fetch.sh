#!/usr/bin/env bash
#
# Download a file, and keep trying.
#
# Both workflows pull binaries from GitHub's release downloads — ffmpeg for the
# Windows runners, git-cliff for the notes — and that host fails often enough
# to have taken out a release and a CI run within an hour of each other. A
# transient download should cost a minute, not a build.
#
# Still fails when the file never arrives. Carrying on without it would mean
# shipping a bundle with no ffmpeg in it, or a release with no notes.
#
# Usage: fetch.sh <url> <output>

set -euo pipefail

url="${1:?url}"
out="${2:?output}"

for attempt in 1 2 3 4 5; do
  # One retry layer, this loop, which logs each attempt and backs off between
  # them. curl retrying inside it as well put four transfers behind every one
  # of these, and --max-time bounds a transfer rather than the set of them, so
  # the ceiling was twenty deadlines rather than five.
  if curl --fail --silent --show-error --location \
       --connect-timeout 20 --max-time 600 \
       --output "$out" "$url"
  then
    echo "fetched $out on attempt $attempt"
    exit 0
  fi

  echo "attempt $attempt failed for $url" >&2
  sleep $((attempt * 10))
done

echo "gave up on $url" >&2
exit 1
