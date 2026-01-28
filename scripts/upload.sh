#!/usr/bin/env bash
# Uploads built package (see build.sh) to PyPi repo
rm -rf build/*
rm -rf dist/*
./build.sh
python3 -m twine upload --verbose dist/*
