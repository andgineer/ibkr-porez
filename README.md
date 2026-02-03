# Repository Coverage

[Full report](https://htmlpreview.github.io/?https://github.com/andgineer/ibkr-porez/blob/python-coverage-comment-action-data/htmlcov/index.html)

| Name                                         |    Stmts |     Miss |   Cover |   Missing |
|--------------------------------------------- | -------: | -------: | ------: | --------: |
| src/ibkr\_porez/\_\_about\_\_.py             |        1 |        0 |    100% |           |
| src/ibkr\_porez/config.py                    |       29 |        0 |    100% |           |
| src/ibkr\_porez/declaration\_gains\_xml.py   |       98 |        3 |     97% |148-149, 152 |
| src/ibkr\_porez/declaration\_income\_xml.py  |       94 |        1 |     99% |        86 |
| src/ibkr\_porez/ibkr\_csv.py                 |       98 |       19 |     81% |39, 46, 63, 73, 81-82, 89-90, 95-96, 125, 128, 132-133, 137-138, 142-143, 152 |
| src/ibkr\_porez/ibkr\_flex\_query.py         |      127 |       27 |     79% |43-47, 51, 59-60, 69, 77-78, 139, 151, 154-155, 172, 175-176, 182-187, 215, 224, 233, 236-237, 241-242 |
| src/ibkr\_porez/main.py                      |      156 |       34 |     78% |35-44, 120-121, 133-136, 152-154, 190, 197-199, 220-222, 229, 243-277 |
| src/ibkr\_porez/models.py                    |       62 |        0 |    100% |           |
| src/ibkr\_porez/nbs.py                       |       51 |        1 |     98% |        40 |
| src/ibkr\_porez/operation\_get.py            |       26 |        0 |    100% |           |
| src/ibkr\_porez/operation\_report.py         |       92 |        8 |     91% |36, 82-83, 167-168, 208-210 |
| src/ibkr\_porez/operation\_report\_params.py |      110 |        6 |     95% |81, 146, 165-166, 169, 188 |
| src/ibkr\_porez/operation\_report\_tables.py |       21 |        0 |    100% |           |
| src/ibkr\_porez/operation\_show.py           |      165 |       38 |     77% |48-53, 76-93, 119, 121, 149-152, 198, 202, 219, 226, 232, 244-245, 362, 382-389 |
| src/ibkr\_porez/operation\_sync.py           |      104 |        7 |     93% |71-74, 94, 121, 137 |
| src/ibkr\_porez/report\_base.py              |       18 |        2 |     89% |    34, 54 |
| src/ibkr\_porez/report\_gains.py             |       28 |        0 |    100% |           |
| src/ibkr\_porez/report\_income.py            |      143 |       14 |     90% |92-95, 160-161, 165, 191, 202, 229, 234-235, 239, 406, 435 |
| src/ibkr\_porez/storage.py                   |      349 |       66 |     81% |58, 92-93, 143-144, 181-183, 249-250, 275, 285, 303-305, 313, 359-373, 383-384, 387, 396, 398, 423-424, 443-446, 451-456, 464, 470-474, 487, 489, 491-492, 508-509, 512, 531, 545, 549, 558-568 |
| src/ibkr\_porez/tax.py                       |       71 |       14 |     80% |26, 32, 56-58, 86-102, 151-152, 155-156 |
| src/ibkr\_porez/validation.py                |       13 |        1 |     92% |        29 |
| **TOTAL**                                    | **1856** |  **241** | **87%** |           |


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