#!/usr/bin/env bash
# Interactive tag-driven release, following the ggg release command.
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

if [[ -n "$(git status --porcelain)" ]]; then
  echo "error: working tree is not clean; commit or stash changes first" >&2
  git status --short
  exit 1
fi

# Ignore prereleases when proposing the next stable patch.
latest="$(git tag --sort=-v:refname | awk '/^v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$/ && !found { print; found=1 }')"
if [[ -z "$latest" ]]; then
  proposed="v0.1.0"
  echo "No existing version tags found."
else
  IFS=. read -r major minor patch <<< "${latest#v}"
  proposed="v${major}.${minor}.$((patch + 1))"
  echo "Latest tag: $latest"
fi

echo "Proposed next tag: $proposed"
printf "Version to release [%s]: " "$proposed"
read -r input
version="${input:-$proposed}"
[[ "$version" == v* ]] || version="v${version}"

if [[ ! "$version" =~ ^v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-[0-9A-Za-z.-]+)?$ ]]; then
  echo "error: version must look like v1.2.3 or v1.2.3-rc1 (got $version)" >&2
  exit 1
fi

if git rev-parse -q --verify "refs/tags/$version" >/dev/null; then
  echo "error: tag $version already exists locally" >&2
  exit 1
fi

# A failed remote lookup must stop the release before creating a local tag.
remote_tag="$(git ls-remote --tags origin "refs/tags/$version")"
if [[ -n "$remote_tag" ]]; then
  echo "error: tag $version already exists on origin" >&2
  exit 1
fi

echo
echo "Will create annotated tag:"
echo "  tag:    $version"
echo "  branch: $(git rev-parse --abbrev-ref HEAD)"
echo "  commit: $(git rev-parse --short HEAD)"
echo "  remote: origin"
echo "The release workflow will update Cargo.toml, Cargo.lock and the README example."
printf "Proceed? [y/N] "
read -r confirm
case "$confirm" in
  y|Y|yes|YES) ;;
  *) echo "Aborted."; exit 1 ;;
esac

git tag -a "$version" -m "Release $version"
git push origin "refs/tags/$version"

echo
echo "Tagged and pushed $version"
echo "Release workflow should start on GitHub shortly."
