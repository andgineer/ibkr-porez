# Repository Coverage

[Full report](https://htmlpreview.github.io/?https://github.com/andgineer/ibkr-porez/blob/python-coverage-comment-action-data/htmlcov/index.html)

| Name                                            |    Stmts |     Miss |   Cover |   Missing |
|------------------------------------------------ | -------: | -------: | ------: | --------: |
| src/ibkr\_porez/\_\_about\_\_.py                |        1 |        0 |    100% |           |
| src/ibkr\_porez/config.py                       |       29 |        0 |    100% |           |
| src/ibkr\_porez/declaration\_gains\_xml.py      |       98 |        3 |     97% |135-136, 139 |
| src/ibkr\_porez/declaration\_income\_xml.py     |       94 |        1 |     99% |        79 |
| src/ibkr\_porez/error\_handling.py              |       18 |       17 |      6% |     18-50 |
| src/ibkr\_porez/ibkr\_csv.py                    |       98 |       19 |     81% |39, 46, 63, 73, 81-82, 89-90, 95-96, 125, 128, 132-133, 137-138, 142-143, 152 |
| src/ibkr\_porez/ibkr\_flex\_query.py            |      126 |       27 |     79% |44-48, 52, 60-61, 70, 78-79, 127, 137, 140-141, 158, 161-162, 168-173, 201, 210, 217, 220-221, 225-226 |
| src/ibkr\_porez/logging\_config.py              |       15 |        0 |    100% |           |
| src/ibkr\_porez/main.py                         |      230 |       55 |     76% |44-53, 147-148, 160-167, 182, 187, 190, 228-234, 246, 259-270, 303-305, 312, 328-329, 352-362, 445-448, 468, 477, 481, 487-493 |
| src/ibkr\_porez/models.py                       |       63 |        0 |    100% |           |
| src/ibkr\_porez/nbs.py                          |       51 |        1 |     98% |        40 |
| src/ibkr\_porez/operation\_get.py               |       36 |        0 |    100% |           |
| src/ibkr\_porez/operation\_import.py            |       76 |       13 |     83% |41-54, 72-73, 112, 131 |
| src/ibkr\_porez/operation\_report.py            |       92 |        8 |     91% |36, 82-83, 167-168, 208-210 |
| src/ibkr\_porez/operation\_report\_params.py    |      110 |        6 |     95% |81, 146, 165-166, 169, 188 |
| src/ibkr\_porez/operation\_report\_tables.py    |       21 |        0 |    100% |           |
| src/ibkr\_porez/operation\_show.py              |      165 |       38 |     77% |45-50, 73-90, 116, 118, 146-149, 195, 199, 216, 223, 229, 241-242, 359, 379-386 |
| src/ibkr\_porez/operation\_show\_declaration.py |       44 |        0 |    100% |           |
| src/ibkr\_porez/operation\_sync.py              |      108 |        3 |     97% |99, 126, 142 |
| src/ibkr\_porez/report\_base.py                 |       18 |        2 |     89% |    34, 54 |
| src/ibkr\_porez/report\_gains.py                |       28 |        0 |    100% |           |
| src/ibkr\_porez/report\_income.py               |      143 |       14 |     90% |92-95, 160-161, 165, 191, 202, 229, 234-235, 239, 406, 435 |
| src/ibkr\_porez/storage.py                      |      319 |       55 |     83% |37, 68-71, 80, 95-96, 143-144, 180-182, 248-249, 273, 282, 298-300, 308, 341-342, 349, 351, 368, 371-372, 377-382, 390, 396-400, 413, 415, 417-418, 434-435, 438, 457, 475, 484-494 |
| src/ibkr\_porez/storage\_flex\_queries.py       |      155 |       33 |     79% |125, 149, 159-161, 169-186, 226, 250, 256-257, 261-264, 284, 300, 303-304, 309-312 |
| src/ibkr\_porez/tax.py                          |       71 |       14 |     80% |27, 33, 57-59, 87-103, 151-152, 155-156 |
| src/ibkr\_porez/validation.py                   |       13 |        1 |     92% |        29 |
| **TOTAL**                                       | **2222** |  **310** | **86%** |           |


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