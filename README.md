# Repository Coverage

[Full report](https://htmlpreview.github.io/?https://github.com/andgineer/ibkr-porez/blob/python-coverage-comment-action-data/htmlcov/index.html)

| Name                             |    Stmts |     Miss |   Cover |   Missing |
|--------------------------------- | -------: | -------: | ------: | --------: |
| src/ibkr\_porez/\_\_about\_\_.py |        1 |        0 |    100% |           |
| src/ibkr\_porez/config.py        |       29 |       11 |     62% |22-30, 33-34, 38 |
| src/ibkr\_porez/ibkr.py          |      138 |       36 |     74% |40, 42, 51-53, 57, 62-63, 78, 86-87, 148, 157-167, 174, 177-178, 194, 197-198, 204-209, 236, 245, 254, 257-258, 262-263 |
| src/ibkr\_porez/main.py          |      163 |      140 |     14% |29-55, 62-127, 133-230, 238-283 |
| src/ibkr\_porez/models.py        |       38 |        0 |    100% |           |
| src/ibkr\_porez/nbs.py           |       50 |       11 |     78% |20, 36-40, 67-79, 99 |
| src/ibkr\_porez/report.py        |       43 |       43 |      0% |     1-103 |
| src/ibkr\_porez/storage.py       |      133 |       61 |     54% |31-34, 37-57, 65-82, 117-131, 134, 141-142, 145, 154, 156, 181-182, 202-205, 210-215, 218-224, 227-233 |
| src/ibkr\_porez/tax.py           |       72 |       15 |     79% |26, 32, 56-58, 86-102, 150-151, 154-155 |
| **TOTAL**                        |  **667** |  **317** | **52%** |           |


## Setup coverage badge

Below are examples of the badges you can use in your main branch `README` file.

### Direct image

[![Coverage badge](https://raw.githubusercontent.com/andgineer/ibkr-porez/python-coverage-comment-action-data/badge.svg)](https://htmlpreview.github.io/?https://github.com/andgineer/ibkr-porez/blob/python-coverage-comment-action-data/htmlcov/index.html)

This is the one to use if your repository is private or if you don't want to customize anything.

### [Shields.io](https://shields.io) Json Endpoint

[![Coverage badge](https://img.shields.io/endpoint?url=https://raw.githubusercontent.com/andgineer/ibkr-porez/python-coverage-comment-action-data/endpoint.json)](https://htmlpreview.github.io/?https://github.com/andgineer/ibkr-porez/blob/python-coverage-comment-action-data/htmlcov/index.html)

Using this one will allow you to [customize](https://shields.io/endpoint) the look of your badge.
It won't work with private repositories. It won't be refreshed more than once per five minutes.

### [Shields.io](https://shields.io) Dynamic Badge

[![Coverage badge](https://img.shields.io/badge/dynamic/json?color=brightgreen&label=coverage&query=%24.message&url=https%3A%2F%2Fraw.githubusercontent.com%2Fandgineer%2Fibkr-porez%2Fpython-coverage-comment-action-data%2Fendpoint.json)](https://htmlpreview.github.io/?https://github.com/andgineer/ibkr-porez/blob/python-coverage-comment-action-data/htmlcov/index.html)

This one will always be the same color. It won't work for private repos. I'm not even sure why we included it.

## What is that?

This branch is part of the
[python-coverage-comment-action](https://github.com/marketplace/actions/python-coverage-comment)
GitHub Action. All the files in this branch are automatically generated and may be
overwritten at any moment.