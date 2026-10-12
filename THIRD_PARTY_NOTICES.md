# Third-party notices

Estigia's own license is in [`LICENSE`](LICENSE). Parts of this repository are derived from the
MIT-licensed project below, and its notice travels with them.

## gentle-ai

Source: <https://github.com/Gentleman-Programming/gentle-ai>

Estigia files derived from it:

- `src/transport/tier.rs` — the risk-tier classification (`review-tier`), from
  `internal/reviewtransaction/risk.go`.
- `src/transport/tier/content_scan.rs` — the content scan beside the path producers, from `risk.go` and
  `internal/reviewtransaction/risk_dangerous_sink.go`.
- `src/transport/tier/tests.rs` — the tests holding that classification.
- The reviewer definition template that `estigia setup` renders (`src/setup/mod.rs`), adapted from
  `jd-judge-a.md`.

```
MIT License

Copyright (c) 2025 Gentleman Programming

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```
