Tokens use gpt-tokenizer 4.0.0 (o200k_base), a proxy for Claude's tokenizer. Bytes are UTF-8 file sizes.

### Tokens (proxy tokenizer)

| Screen | weft | html | jsx | a2ui | weft/html | weft/jsx | weft/a2ui |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| login | 203 | 189 | 242 | 714 | 107% | 84% | 28% |
| signup | 264 | 277 | 371 | 887 | 95% | 71% | 30% |
| settings | 258 | 273 | 344 | 765 | 95% | 75% | 34% |
| data-table | 285 | 277 | 285 | 795 | 103% | 100% | 36% |
| tabs | 232 | 339 | 342 | 629 | 68% | 68% | 37% |
| confirm-dialog | 171 | 153 | 168 | 532 | 112% | 102% | 32% |
| wizard-step | 190 | 224 | 294 | 540 | 85% | 65% | 35% |
| search-results | 223 | 185 | 225 | 560 | 121% | 99% | 40% |
| todo-list | 220 | 209 | 259 | 622 | 105% | 85% | 35% |
| profile | 169 | 133 | 137 | 599 | 127% | 123% | 28% |
| menu | 148 | 149 | 164 | 615 | 99% | 90% | 24% |
| error-state | 139 | 116 | 134 | 444 | 120% | 104% | 31% |
| **total** | **2502** | **2524** | **2965** | **7702** | **99%** | **84%** | **32%** |

Per-screen weft/baseline ratio (lower is better for Weft):

| Baseline | min | median | max | total |
| --- | ---: | ---: | ---: | ---: |
| html | 68% | 104% | 127% | 99% |
| jsx | 65% | 88% | 123% | 84% |
| a2ui | 24% | 33% | 40% | 32% |

### Tokens with whitespace collapsed (indentation and newlines removed from every format)

| Screen | weft | html | jsx | a2ui | weft/html | weft/jsx | weft/a2ui |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| login | 192 | 175 | 224 | 574 | 110% | 86% | 33% |
| signup | 251 | 256 | 346 | 712 | 98% | 73% | 35% |
| settings | 243 | 247 | 314 | 623 | 98% | 77% | 39% |
| data-table | 263 | 248 | 253 | 649 | 106% | 104% | 41% |
| tabs | 213 | 314 | 314 | 510 | 68% | 68% | 42% |
| confirm-dialog | 161 | 143 | 155 | 435 | 113% | 104% | 37% |
| wizard-step | 181 | 203 | 269 | 435 | 89% | 67% | 42% |
| search-results | 207 | 166 | 203 | 457 | 125% | 102% | 45% |
| todo-list | 206 | 189 | 236 | 504 | 109% | 87% | 41% |
| profile | 161 | 124 | 124 | 488 | 130% | 130% | 33% |
| menu | 141 | 142 | 153 | 503 | 99% | 92% | 28% |
| error-state | 132 | 108 | 122 | 364 | 122% | 108% | 36% |
| **total** | **2351** | **2315** | **2713** | **6254** | **102%** | **87%** | **38%** |

Per-screen weft/baseline ratio (lower is better for Weft):

| Baseline | min | median | max | total |
| --- | ---: | ---: | ---: | ---: |
| html | 68% | 108% | 130% | 102% |
| jsx | 67% | 90% | 130% | 87% |
| a2ui | 28% | 38% | 45% | 38% |

### Bytes

| Screen | weft | html | jsx | a2ui | weft/html | weft/jsx | weft/a2ui |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| login | 692 | 671 | 956 | 3251 | 103% | 72% | 21% |
| signup | 946 | 1039 | 1514 | 4157 | 91% | 62% | 23% |
| settings | 961 | 987 | 1395 | 3421 | 97% | 69% | 28% |
| data-table | 980 | 1005 | 1166 | 3335 | 98% | 84% | 29% |
| tabs | 800 | 1223 | 1357 | 2704 | 65% | 59% | 30% |
| confirm-dialog | 599 | 550 | 671 | 2267 | 109% | 89% | 26% |
| wizard-step | 615 | 733 | 1098 | 2341 | 84% | 56% | 26% |
| search-results | 747 | 638 | 881 | 2324 | 117% | 85% | 32% |
| todo-list | 717 | 716 | 999 | 2651 | 100% | 72% | 27% |
| profile | 531 | 405 | 485 | 2541 | 131% | 109% | 21% |
| menu | 494 | 531 | 646 | 2686 | 93% | 76% | 18% |
| error-state | 476 | 389 | 505 | 1847 | 122% | 94% | 26% |
| **total** | **8558** | **8887** | **11673** | **33525** | **96%** | **73%** | **26%** |

Per-screen weft/baseline ratio (lower is better for Weft):

| Baseline | min | median | max | total |
| --- | ---: | ---: | ---: | ---: |
| html | 65% | 99% | 131% | 96% |
| jsx | 56% | 74% | 109% | 73% |
| a2ui | 18% | 26% | 32% | 26% |

Anthropic token counts: not measured (ANTHROPIC_API_KEY was not set).

Stop-criterion check (as committed): Weft uses 67.5% fewer tokens than A2UI JSON in total; the criterion asks for at least 25% fewer.
Stop-criterion check (whitespace collapsed): Weft uses 62.4% fewer tokens than A2UI JSON in total; the criterion asks for at least 25% fewer.
