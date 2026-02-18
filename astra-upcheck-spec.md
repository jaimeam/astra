# astra-upcheck — HTTP Health Checker

## Purpose

CLI tool that checks whether a list of URLs are up or down, reports status, and optionally writes results to a file.

---

## Core Types

- `HealthStatus` enum — `Up(code: Int)` | `Down(reason: Text)` | `Timeout`
- `Check` record — `{ url: Text, status: HealthStatus }`
- `CheckSummary` record — `{ total: Int, up: Int, down: Int }`

## Functions

| Function | Signature | Effects | Description |
|---|---|---|---|
| `check_url` | `(url: Text) -> Check` | `Net, Console` | GET the URL, return a `Check` with status |
| `check_all` | `(urls: List[Text]) -> List[Check]` | `Net, Console` | Run `check_url` on each URL, print progress |
| `summarize` | `(checks: List[Check]) -> CheckSummary` | *(pure)* | Count up/down/timeout |
| `format_report` | `(checks: List[Check], summary: CheckSummary) -> Text` | *(pure)* | Build a human-readable text report |
| `print_report` | `(report: Text)` | `Console` | Print the report to stdout |
| `save_report` | `(report: Text, path: Text)` | `Fs, Console` | Write report to file, confirm on console |

## Modules

- `module upcheck` — core types, `check_url`, `check_all`, `summarize` (in `upcheck.astra`)
- `module report` — `format_report`, `save_report` (in `report.astra`)
- `module main` — entry point, reads URL list, orchestrates everything (in `main.astra`)

## Input

Hardcoded `List[Text]` of URLs in `main.astra`. (Astra's `Env.args()` exists if you want to get fancy, but a simple list is fine for v1.)

## Output (Console)

```
Checking 4 URLs...

  [UP]   https://example.com (200)
  [DOWN] https://bad.invalid (connection refused)
  [UP]   https://httpbin.org/get (200)
  [DOWN] https://httpbin.org/status/500 (500)

Summary: 2 up, 2 down, 0 timeout (4 total)
```

## Optional: File Output

If a file path is provided, write the same report via `Fs.write()`.

## Contracts

- `check_all` — `requires urls.len() > 0` (don't check an empty list)
- `summarize` — `ensures result.up + result.down == result.total`

## Tests

| Test | What it validates |
|---|---|
| `"summarize all up"` | All checks `Up` -> `down == 0` |
| `"summarize all down"` | All checks `Down` -> `up == 0` |
| `"summarize mixed"` | Mix of statuses -> counts add up |
| `"format_report includes URLs"` | Output text contains each URL |
| `"empty summary"` | `total == 0` edge case (unit test for the pure function) |
| `"check_all requires non-empty"` | Contract violation on empty list |

## Effects Map

```
main        effects(Net, Console, Fs)
 |-- check_all   effects(Net, Console)
 |    \-- check_url   effects(Net, Console)
 |-- summarize       (pure)
 |-- format_report   (pure)
 |-- print_report    effects(Console)
 \-- save_report     effects(Fs, Console)
```
