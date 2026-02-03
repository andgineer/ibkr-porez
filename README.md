# Repository Coverage

[Full report](https://htmlpreview.github.io/?https://github.com/andgineer/ibkr-porez/blob/python-coverage-comment-action-data/htmlcov/index.html)

| Name                                         |    Stmts |     Miss |   Cover |   Missing |
|--------------------------------------------- | -------: | -------: | ------: | --------: |
| src/ibkr\_porez/\_\_about\_\_.py             |        1 |        0 |    100% |           |
| src/ibkr\_porez/config.py                    |       29 |        0 |    100% |           |
| src/ibkr\_porez/declaration\_gains\_xml.py   |       98 |        3 |     97% |148-149, 152 |
| src/ibkr\_porez/declaration\_income\_xml.py  |       94 |        1 |     99% |        82 |
| src/ibkr\_porez/ibkr\_csv.py                 |       98 |       19 |     81% |39, 46, 63, 73, 81-82, 89-90, 95-96, 125, 128, 132-133, 137-138, 142-143, 152 |
| src/ibkr\_porez/ibkr\_flex\_query.py         |      127 |       27 |     79% |43-47, 51, 59-60, 69, 77-78, 139, 151, 154-155, 172, 175-176, 182-187, 215, 224, 233, 236-237, 241-242 |
| src/ibkr\_porez/main.py                      |      136 |       20 |     85% |32-41, 117-118, 130-133, 149-151, 187, 194-196, 217-219, 226 |
| src/ibkr\_porez/models.py                    |       60 |        0 |    100% |           |
| src/ibkr\_porez/nbs.py                       |       51 |        1 |     98% |        40 |
| src/ibkr\_porez/operation\_get.py            |       26 |        0 |    100% |           |
| src/ibkr\_porez/operation\_report.py         |       87 |        8 |     91% |36, 79-80, 161-162, 202-204 |
| src/ibkr\_porez/operation\_report\_params.py |      110 |        6 |     95% |81, 146, 165-166, 169, 188 |
| src/ibkr\_porez/operation\_report\_tables.py |       21 |        0 |    100% |           |
| src/ibkr\_porez/operation\_show.py           |      165 |       38 |     77% |48-53, 76-93, 119, 121, 149-152, 198, 202, 219, 226, 232, 244-245, 362, 382-389 |
| src/ibkr\_porez/report\_gains.py             |       30 |        1 |     97% |        67 |
| src/ibkr\_porez/report\_income.py            |      148 |       16 |     89% |89-92, 157-158, 162, 188, 199, 226, 231-232, 236, 379, 408, 435-436 |
| src/ibkr\_porez/storage.py                   |      346 |      104 |     70% |53, 87-88, 138-139, 176-178, 244-245, 270, 280, 298-300, 308, 354-368, 378-379, 382, 391, 393, 418-419, 438-441, 446-451, 459, 465-469, 475-487, 491-492, 496-513, 521-530, 534-540, 544, 553-563, 567-573, 577-579 |
| src/ibkr\_porez/tax.py                       |       71 |       14 |     80% |26, 32, 56-58, 86-102, 151-152, 155-156 |
| src/ibkr\_porez/validation.py                |       13 |        1 |     92% |        29 |
| **TOTAL**                                    | **1711** |  **259** | **85%** |           |


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