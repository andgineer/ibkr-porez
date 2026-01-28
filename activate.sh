#!/usr/bin/env bash
#
# "Set-ups or/and activates development environment"
#

VENV_FOLDER=".venv"

PRIMARY_PYTHON_VERSION="3.12"  # sync with .github/workflows/docs.yml&static.yml

RED='\033[1;31m'
GREEN='\033[1;32m'
CYAN='\033[1;36m'
NC='\033[0m' # No Color

if ! (return 0 2>/dev/null) ; then
    # If return is used in the top-level scope of a non-sourced script,
    # an error message is emitted, and the exit code is set to 1
    echo
    echo -e $RED"This script should be sourced like"$NC
    echo "    . ./activate.sh"
    echo
    exit 1
fi


if [[ ! -d ${VENV_FOLDER} ]] ; then
    unset CONDA_PREFIX  # if conda is installed, it will mess with the virtual env

    echo -e $CYAN"Creating virtual environment for python in ${VENV_FOLDER}"$NC
    START_TIME=$(date +%s)

    # Check if the required Python version is installed
    if ! command -v python${PRIMARY_PYTHON_VERSION} &> /dev/null; then
        echo -e $RED"Error: Python ${PRIMARY_PYTHON_VERSION} is not installed."$NC
        echo -e $YELLOW"Please install Python ${PRIMARY_PYTHON_VERSION} before proceeding."$NC
        echo -e $YELLOW"You can download it from https://www.python.org/downloads/"$NC
        return 1
    fi

    if command -v uv &> /dev/null; then
        if uv venv ${VENV_FOLDER} --python=python${PRIMARY_PYTHON_VERSION}; then

            . ${VENV_FOLDER}/bin/activate
            uv sync --frozen
            END_TIME=$(date +%s)
            echo "Environment created in $((END_TIME - $START_TIME)) seconds"
        else
            echo -e $RED"Error creating virtual environment. Please check the output above for more details."$NC
            return 1
        fi
    else

        echo -e $RED"Error: Astral's UV is not installed."$NC
        echo -e $YELLOW"Please install UV from https://github.com/astral-sh/uv before proceeding."$NC

        return 1
    fi
else
    echo -e $CYAN"Activating virtual environment ..."$NC
    . ${VENV_FOLDER}/bin/activate
fi
