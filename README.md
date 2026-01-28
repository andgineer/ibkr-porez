[![Build Status](https://github.com/andgineer/ibkr-porez/workflows/CI/badge.svg)](https://github.com/andgineer/ibkr-porez/actions)
[![Coverage](https://raw.githubusercontent.com/andgineer/ibkr-porez/python-coverage-comment-action-data/badge.svg)](https://htmlpreview.github.io/?https://github.com/andgineer/ibkr-porez/blob/python-coverage-comment-action-data/htmlcov/index.html)
# ibkr-porez

Automated PPDG-3R tax reports for Interactive Brokers. It automatically pulls your data and generates a ready-to-upload XML file with all prices converted to RSD.

# Documentation

[Ibkr Porez](https://andgineer.github.io/ibkr-porez/)



# Developers

Do not forget to run `. ./activate.sh`.

For work it need [uv](https://github.com/astral-sh/uv) installed.

Use [pre-commit](https://pre-commit.com/#install) hooks for code quality:

    pre-commit install

## Allure test report

* [Allure report](https://andgineer.github.io/ibkr-porez/builds/tests/)

# Scripts

Install [invoke](https://docs.pyinvoke.org/en/stable/) preferably with [pipx](https://pypa.github.io/pipx/):

    pipx install invoke

For a list of available scripts run:

    invoke --list

For more information about a script run:

    invoke <script> --help


## Coverage report
* [Codecov](https://app.codecov.io/gh/andgineer/ibkr-porez/tree/main/src%2Fibkr_porez)
* [Coveralls](https://coveralls.io/github/andgineer/ibkr-porez)

> Created with cookiecutter using [template](https://github.com/andgineer/cookiecutter-python-package)
