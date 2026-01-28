#!/usr/bin/env bash
#
# Substitute language and site in mkdocs.yml in _mkdocs.yml
# language should be passed as an argument
#

lang=${1:-en}  # Default to "en" if not provided
echo "Rendering docs for $lang"
sed "s/LANGUAGE/$lang/g" docs/mkdocs.yml > docs/_mkdocs.yml
if [ "$lang" = "en" ]; then
    echo "place English to the root of the site"
    sed -i'' -e "s/SITE_PREFIX//g" docs/_mkdocs.yml
else
    sed -i'' -e "s/SITE_PREFIX/$lang/g" docs/_mkdocs.yml
fi
