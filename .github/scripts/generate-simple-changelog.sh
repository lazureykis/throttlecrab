#!/bin/bash
set -e

VERSION="$1"
CURRENT_VERSION="$2"
PREVIOUS_TAG="$3"
REPOSITORY="$4"

echo "Generating simple changelog for v$VERSION..."

# Get commit count since previous tag
COMMIT_COUNT=$(git log --oneline "$PREVIOUS_TAG"..HEAD | wc -l)

# Generate basic changelog
cat << EOF
## Changes

• Version bump from $CURRENT_VERSION to $VERSION
• Includes $COMMIT_COUNT commits since previous release

**Full Changelog**: https://github.com/$REPOSITORY/compare/v$CURRENT_VERSION...v$VERSION
EOF