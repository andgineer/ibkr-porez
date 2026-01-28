#!/usr/bin/env bash
#
# Build docs for all languages or copy assets for one language.
# Usage:
#   build-docs.sh                    # Build site for all languages in `LANGUAGES`
#   build-docs.sh --copy-assets <lang> # Copy assets only (use before mkdocs serve)
#

LANGUAGES="en ru"

# Parse arguments
if [ "$1" = "--copy-assets" ]; then
    COPY_ASSETS_ONLY=true
    LANGUAGES=${2:-en}
else
    COPY_ASSETS_ONLY=false
fi

for lang in $LANGUAGES; do  # en should be the first language as it clears the root of the site
    scripts/docs-render-config.sh $lang
    if [ $lang != "en" ]; then
        cp -r ./docs/src/en/images/ ./docs/src/$lang/images/ || true
        cp ./docs/src/en/reference.md ./docs/src/$lang/reference.md || true
    fi
    if [ "$COPY_ASSETS_ONLY" != true ]; then
        mkdocs build --dirty --config-file docs/_mkdocs.yml
        rm docs/_mkdocs.yml
    fi
done
