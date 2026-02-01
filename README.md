# Repository Coverage

[Full report](https://htmlpreview.github.io/?https://github.com/andgineer/ibkr-porez/blob/python-coverage-comment-action-data/htmlcov/index.html)

| Name                                       |    Stmts |     Miss |   Cover |   Missing |
|------------------------------------------- | -------: | -------: | ------: | --------: |
| src/ibkr\_porez/\_\_about\_\_.py           |        1 |        0 |    100% |           |
| src/ibkr\_porez/config.py                  |       29 |        0 |    100% |           |
| src/ibkr\_porez/declaration\_gains\_xml.py |       98 |        3 |     97% |148-149, 152 |
| src/ibkr\_porez/ibkr\_csv.py               |       98 |       19 |     81% |39, 46, 63, 73, 81-82, 89-90, 95-96, 125, 128, 132-133, 137-138, 142-143, 152 |
| src/ibkr\_porez/ibkr\_flex\_query.py       |      127 |       27 |     79% |43-47, 51, 59-60, 69, 77-78, 139, 151, 154-155, 172, 175-176, 182-187, 215, 224, 233, 236-237, 241-242 |
| src/ibkr\_porez/main.py                    |      317 |       64 |     80% |42-51, 155-159, 177-179, 215, 224-226, 275-301, 320, 322, 326-335, 341-344, 392, 398, 412, 418, 433-434, 527-529, 579-591 |
| src/ibkr\_porez/models.py                  |       40 |        0 |    100% |           |
| src/ibkr\_porez/nbs.py                     |       51 |        2 |     96% |    40, 92 |
| src/ibkr\_porez/report\_gains.py           |       31 |        1 |     97% |        67 |
| src/ibkr\_porez/report\_income.py          |        7 |        2 |     71% |    12, 34 |
| src/ibkr\_porez/report\_params.py          |      109 |       34 |     69% |31-35, 53, 76-80, 90, 92, 101, 106, 138, 144, 147-149, 163-164, 167, 170-186 |
| src/ibkr\_porez/storage.py                 |      277 |       54 |     81% |43, 77-78, 128-129, 166-168, 234-235, 260, 270, 288-290, 298, 344-358, 368-369, 372, 381, 383, 408-409, 428-431, 436-441, 444-450, 453-459 |
| src/ibkr\_porez/tables.py                  |       21 |        0 |    100% |           |
| src/ibkr\_porez/tax.py                     |       71 |       14 |     80% |26, 32, 56-58, 86-102, 151-152, 155-156 |
| src/ibkr\_porez/validation.py              |       23 |       10 |     57% | 24, 33-44 |
| **TOTAL**                                  | **1300** |  **230** | **82%** |           |


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