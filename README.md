# Repository Coverage

[Full report](https://htmlpreview.github.io/?https://github.com/andgineer/ibkr-porez/blob/python-coverage-comment-action-data/htmlcov/index.html)

| Name                                                |    Stmts |     Miss |   Cover |   Missing |
|---------------------------------------------------- | -------: | -------: | ------: | --------: |
| src/ibkr\_porez/\_\_about\_\_.py                    |        1 |        0 |    100% |           |
| src/ibkr\_porez/config.py                           |       49 |        0 |    100% |           |
| src/ibkr\_porez/declaration\_gains\_xml.py          |       98 |        3 |     97% |135-136, 139 |
| src/ibkr\_porez/declaration\_income\_xml.py         |       94 |        1 |     99% |        79 |
| src/ibkr\_porez/declaration\_manager.py             |      190 |       19 |     90% |30, 43, 56, 61-62, 72-78, 143, 170, 195, 216, 246, 250, 307 |
| src/ibkr\_porez/error\_handling.py                  |       18 |        0 |    100% |           |
| src/ibkr\_porez/gui/app\_icon.py                    |       94 |       75 |     20% |25-40, 44-49, 62-74, 85-118, 122-155, 159-161 |
| src/ibkr\_porez/gui/assessment\_dialog.py           |       46 |       40 |     13% |25-59, 62-77 |
| src/ibkr\_porez/gui/config\_dialog.py               |      114 |        5 |     96% |131, 134, 153-159 |
| src/ibkr\_porez/gui/constants.py                    |        6 |        0 |    100% |           |
| src/ibkr\_porez/gui/declaration\_details\_dialog.py |       20 |       15 |     25% |     16-33 |
| src/ibkr\_porez/gui/export\_worker.py               |       18 |        0 |    100% |           |
| src/ibkr\_porez/gui/import\_dialog.py               |      147 |       37 |     75% |119-126, 142, 160-182, 186-194, 198-200, 204-205, 208-211, 216-218 |
| src/ibkr\_porez/gui/import\_worker.py               |       20 |        0 |    100% |           |
| src/ibkr\_porez/gui/launcher.py                     |       86 |       15 |     83% |47-48, 58, 64-65, 83-90, 107-108 |
| src/ibkr\_porez/gui/main.py                         |       45 |       17 |     62% | 29-46, 58 |
| src/ibkr\_porez/gui/main\_window.py                 |      502 |      194 |     61% |221, 232-240, 253-254, 287-289, 314-315, 354-357, 364-392, 395-424, 427-435, 438-445, 448, 451-452, 459-461, 464, 468-469, 473-474, 478-479, 484, 493, 496-505, 511-514, 524, 529, 537, 561, 593-604, 611, 615-624, 627-659, 663-672, 676-678, 682-685, 694-710, 719-727, 731-746, 750-752 |
| src/ibkr\_porez/gui/styles.py                       |        2 |        0 |    100% |           |
| src/ibkr\_porez/gui/sync\_worker.py                 |       22 |        0 |    100% |           |
| src/ibkr\_porez/ibkr\_csv.py                        |       98 |       19 |     81% |39, 46, 63, 73, 81-82, 89-90, 95-96, 125, 128, 132-133, 137-138, 142-143, 152 |
| src/ibkr\_porez/ibkr\_flex\_query.py                |      126 |       27 |     79% |44-48, 52, 60-61, 70, 78-79, 127, 137, 140-141, 158, 161-162, 168-173, 201, 210, 217, 220-221, 225-226 |
| src/ibkr\_porez/logging\_config.py                  |       15 |        0 |    100% |           |
| src/ibkr\_porez/main.py                             |      360 |       68 |     81% |53-62, 81, 88-89, 91, 112-113, 133-134, 146-153, 168, 173, 176, 207-213, 225, 238-249, 280-282, 289, 352-354, 363-369, 473-476, 496, 505-507, 512, 518-524, 622-624, 747-748, 757-758 |
| src/ibkr\_porez/models.py                           |       91 |        6 |     93% |176, 196-200 |
| src/ibkr\_porez/nbs.py                              |       51 |        1 |     98% |        40 |
| src/ibkr\_porez/operation\_config.py                |      116 |        2 |     98% |    21, 99 |
| src/ibkr\_porez/operation\_get.py                   |       36 |        0 |    100% |           |
| src/ibkr\_porez/operation\_import.py                |       76 |       13 |     83% |41-54, 72-73, 112, 131 |
| src/ibkr\_porez/operation\_list.py                  |       32 |        0 |    100% |           |
| src/ibkr\_porez/operation\_report.py                |      100 |        8 |     92% |38, 90-91, 181-182, 224-226 |
| src/ibkr\_porez/operation\_report\_params.py        |      110 |        6 |     95% |81, 146, 165-166, 169, 188 |
| src/ibkr\_porez/operation\_report\_tables.py        |       21 |        0 |    100% |           |
| src/ibkr\_porez/operation\_show\_declaration.py     |       54 |        0 |    100% |           |
| src/ibkr\_porez/operation\_stat.py                  |      165 |       26 |     84% |45-50, 116, 118, 146-149, 195, 199, 216, 223, 229, 241-242, 363, 383-390 |
| src/ibkr\_porez/operation\_sync.py                  |      155 |        4 |     97% |82, 149, 202, 253 |
| src/ibkr\_porez/report\_base.py                     |       18 |        2 |     89% |    34, 54 |
| src/ibkr\_porez/report\_gains.py                    |       28 |        0 |    100% |           |
| src/ibkr\_porez/report\_income.py                   |      143 |       14 |     90% |92-95, 160-161, 165, 191, 202, 229, 234-235, 239, 408, 437 |
| src/ibkr\_porez/storage.py                          |      317 |       43 |     86% |37, 68-71, 80, 95-96, 143-144, 180-182, 248-249, 273, 282, 298-300, 308, 341-342, 349, 351, 368, 371-372, 377-382, 390, 396-400, 413, 415, 417-418, 475, 486 |
| src/ibkr\_porez/storage\_flex\_queries.py           |      155 |       27 |     83% |125, 149, 159-161, 169-186, 226, 250, 256-257, 261-264, 284, 300 |
| src/ibkr\_porez/tax.py                              |       71 |       14 |     80% |27, 33, 57-59, 87-103, 151-152, 155-156 |
| src/ibkr\_porez/validation.py                       |       13 |        1 |     92% |        29 |
| **TOTAL**                                           | **3923** |  **702** | **82%** |           |


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