# Repository Coverage

[Full report](https://htmlpreview.github.io/?https://github.com/andgineer/ibkr-porez/blob/python-coverage-comment-action-data/htmlcov/index.html)

| Name                                            |    Stmts |     Miss |   Cover |   Missing |
|------------------------------------------------ | -------: | -------: | ------: | --------: |
| src/gui/config\_dialog.py                       |      100 |      100 |      0% |     1-142 |
| src/gui/constants.py                            |        6 |        6 |      0% |      1-11 |
| src/gui/export\_worker.py                       |       18 |       18 |      0% |      1-27 |
| src/gui/import\_dialog.py                       |      148 |      148 |      0% |     1-211 |
| src/gui/import\_worker.py                       |       20 |       20 |      0% |      1-30 |
| src/gui/main.py                                 |       11 |       11 |      0% |      1-17 |
| src/gui/main\_window.py                         |      419 |      419 |      0% |     1-594 |
| src/gui/styles.py                               |        2 |        2 |      0% |       1-3 |
| src/gui/sync\_worker.py                         |       21 |       21 |      0% |      1-28 |
| src/ibkr\_porez/\_\_about\_\_.py                |        1 |        0 |    100% |           |
| src/ibkr\_porez/config.py                       |       42 |        0 |    100% |           |
| src/ibkr\_porez/declaration\_gains\_xml.py      |       98 |        3 |     97% |135-136, 139 |
| src/ibkr\_porez/declaration\_income\_xml.py     |       94 |        1 |     99% |        79 |
| src/ibkr\_porez/declaration\_manager.py         |      116 |       17 |     85% |24-32, 88, 120-124, 132, 136, 189 |
| src/ibkr\_porez/error\_handling.py              |       18 |       17 |      6% |     18-50 |
| src/ibkr\_porez/ibkr\_csv.py                    |       98 |       19 |     81% |39, 46, 63, 73, 81-82, 89-90, 95-96, 125, 128, 132-133, 137-138, 142-143, 152 |
| src/ibkr\_porez/ibkr\_flex\_query.py            |      126 |       27 |     79% |44-48, 52, 60-61, 70, 78-79, 127, 137, 140-141, 158, 161-162, 168-173, 201, 210, 217, 220-221, 225-226 |
| src/ibkr\_porez/logging\_config.py              |       15 |        0 |    100% |           |
| src/ibkr\_porez/main.py                         |      357 |       73 |     80% |53-56, 72-81, 100, 121, 131, 153-154, 174-175, 187-194, 209, 214, 217, 255-261, 273, 286-297, 328-330, 337, 374-375, 403-405, 414-420, 524-527, 547, 556-558, 563, 569-575, 649-651, 732-733, 742-743 |
| src/ibkr\_porez/models.py                       |       78 |        1 |     99% |       162 |
| src/ibkr\_porez/nbs.py                          |       51 |        1 |     98% |        40 |
| src/ibkr\_porez/operation\_config.py            |      116 |        2 |     98% |    21, 99 |
| src/ibkr\_porez/operation\_get.py               |       36 |        0 |    100% |           |
| src/ibkr\_porez/operation\_import.py            |       76 |       13 |     83% |41-54, 72-73, 112, 131 |
| src/ibkr\_porez/operation\_list.py              |       31 |        0 |    100% |           |
| src/ibkr\_porez/operation\_report.py            |      105 |        8 |     92% |46, 98-99, 189-190, 232-234 |
| src/ibkr\_porez/operation\_report\_params.py    |      110 |        6 |     95% |81, 146, 165-166, 169, 188 |
| src/ibkr\_porez/operation\_report\_tables.py    |       21 |        0 |    100% |           |
| src/ibkr\_porez/operation\_show\_declaration.py |       48 |        0 |    100% |           |
| src/ibkr\_porez/operation\_stat.py              |      165 |       38 |     77% |45-50, 73-90, 116, 118, 146-149, 195, 199, 216, 223, 229, 241-242, 363, 383-390 |
| src/ibkr\_porez/operation\_sync.py              |      135 |        4 |     97% |28, 119, 172, 223 |
| src/ibkr\_porez/report\_base.py                 |       18 |        2 |     89% |    34, 54 |
| src/ibkr\_porez/report\_gains.py                |       28 |        0 |    100% |           |
| src/ibkr\_porez/report\_income.py               |      143 |       14 |     90% |92-95, 160-161, 165, 191, 202, 229, 234-235, 239, 408, 437 |
| src/ibkr\_porez/storage.py                      |      319 |       43 |     87% |37, 68-71, 80, 95-96, 143-144, 180-182, 248-249, 273, 282, 298-300, 308, 341-342, 349, 351, 368, 371-372, 377-382, 390, 396-400, 413, 415, 417-418, 475, 486 |
| src/ibkr\_porez/storage\_flex\_queries.py       |      155 |       33 |     79% |125, 149, 159-161, 169-186, 226, 250, 256-257, 261-264, 284, 300, 303-304, 309-312 |
| src/ibkr\_porez/tax.py                          |       71 |       14 |     80% |27, 33, 57-59, 87-103, 151-152, 155-156 |
| src/ibkr\_porez/validation.py                   |       13 |        1 |     92% |        29 |
| **TOTAL**                                       | **3429** | **1082** | **68%** |           |


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