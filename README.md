# Repository Coverage

[Full report](https://htmlpreview.github.io/?https://github.com/andgineer/ibkr-porez/blob/python-coverage-comment-action-data/htmlcov/index.html)

| Name                                   |    Stmts |     Miss |   Cover |   Missing |
|--------------------------------------- | -------: | -------: | ------: | --------: |
| src/ibkr\_porez/\_\_about\_\_.py       |        1 |        0 |    100% |           |
| src/ibkr\_porez/config.py              |       29 |       11 |     62% |22-30, 33-34, 38 |
| src/ibkr\_porez/ibkr.py                |      141 |       38 |     73% |44, 46, 55-59, 63, 71-72, 88, 96-97, 158, 167-177, 184, 187-188, 205, 208-209, 215-220, 248, 257, 266, 269-270, 274-275 |
| src/ibkr\_porez/main.py                |      347 |      318 |      8% |30-65, 72-139, 146-186, 200-434, 441-584 |
| src/ibkr\_porez/models.py              |       40 |        0 |    100% |           |
| src/ibkr\_porez/nbs.py                 |       51 |       11 |     78% |20, 37-41, 69-81, 101 |
| src/ibkr\_porez/parsers/csv\_parser.py |       98 |       19 |     81% |39, 46, 63, 73, 81-82, 89-90, 95-96, 125, 128, 132-133, 137-138, 142-143, 152 |
| src/ibkr\_porez/report.py              |       98 |        3 |     97% |148-149, 152 |
| src/ibkr\_porez/storage.py             |      277 |       59 |     79% |31-34, 43, 77-78, 128-129, 166-168, 234-235, 260, 270, 288-290, 298, 344-358, 361, 368-369, 372, 381, 383, 408-409, 428-431, 436-441, 444-450, 453-459 |
| src/ibkr\_porez/tax.py                 |       71 |       14 |     80% |26, 32, 56-58, 86-102, 151-152, 155-156 |
| **TOTAL**                              | **1153** |  **473** | **59%** |           |


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