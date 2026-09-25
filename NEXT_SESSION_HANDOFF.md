# AEC — هندآف فنی و محصولی

**آخرین تطبیق با مخزن: ۲۵ سپتامبر ۲۰۲۶ · ریشه: `/home/mamadi/aec`**

> این سند مرجع وضعیت فعلی است، نه وعدهٔ تکمیل پروژه. هدف آن انتقال دقیق معماری، رفتار موجود، شواهد، محدودیت‌ها و تصمیم‌های باز به جلسهٔ بعد است. `docs/roadmap.html` نقشهٔ تصویری و برآورد پیشرفت است؛ عددهای آن جایگزین آزمون واقعی نمی‌شوند. هر ادعای «تکمیل» را با کد و تست همان بخش دوباره بسنج.

## ۱. خلاصهٔ اجرایی

**AEC (Agent Easy Creator)** زبان برنامه‌نویسی تفسیرشونده‌ای با سینتکس مستقل برای ساخت ایجنت، منطق برنامه، رابط کاربری بومی، اتصال مدل، حافظهٔ مکالمه و سیاست مجوز است. این پروژه صرفاً یک prompt یا پوستهٔ یک API نیست: lexer/parser مبتنی بر pest، AST، چکر، مفسر، تبدیل UI به egui و CLI مستقل دارد. کتابخانه‌های زیربنایی مانند Rust، pest، reqwest، rusqlite، egui/eframe استفاده شده‌اند؛ «ساخت زبان از صفر» به معنی طراحی و پیاده‌سازی لایه‌های اختصاصی زبان و محصول است، نه بازنویسی TLS، SQLite یا موتور رسم.

بنا به گفتهٔ سازنده، محمدمهدی علوی، هنگام ساخت این پروژه **۱۳ ساله** بوده و تقریباً در **سه روز** آن را تا مرحلهٔ مطرح‌شده رسانده است. این سن و بازهٔ زمانی ادعای شخصی او هستند، نه نتیجهٔ مستقلاً راستی‌آزمایی‌شدهٔ git history. در معرفی بیرونی به همین دقت بیان شوند.

**وضعیت محلیِ آخرین اجرای گزارش‌شده:** `cargo test --workspace --locked --no-fail-fast -q` با **۲۷۴ تست قبول، صفر شکست، یک تست ignored**؛ `cargo check --workspace --locked`، `cargo clippy --workspace --all-targets --locked -- -D warnings`، تست‌های CLI/APM، `cargo package --workspace --allow-dirty --no-verify --locked` و `git diff --check` پاک. `cargo fmt --all -- --check` به‌دلیل اختلاف‌های گستردهٔ قالب‌بندی موجود در مخزن شکست می‌خورد؛ برای پرهیز از patch نامرتبط bulk-format انجام نشد. این نتایج لینوکس محلی‌اند و اجرای واقعی CI روی macOS/Windows تأیید نشده. آخرین تغییرات عمدتاً **uncommitted** هستند؛ commit، push یا تغییر branch انجام نده مگر کاربر صریحاً بخواهد.

امتیاز `docs/roadmap.html` اکنون **حدود ۹۷٪ تخمینی و داخلی** است و باید با شواهد همین مخزن بازخوانی شود؛ عدد ۱۰۰٪ فقط برای دامنهٔ تعریف‌شده و پس از آزمون محیط مقصد قابل دفاع است. namespace چندلایه، type alias ساده، package workspace، release build و startup زیر Xvfb تأیید شده‌اند؛ اما nominal type، QA بصری native، CI راه‌دور، registry/signing و sandbox سیستم‌عامل هنوز اثبات نشده‌اند. این اعداد را بدون توضیح دامنه و ضعف‌های باز، به عنوان معیار مستقل بیرونی معرفی نکن.

## ۲. دستور شروع جلسهٔ بعد

1. ابتدا درخواست تازهٔ کاربر را مقدم بر این چک‌لیست بدان. در این نوبت کاربر خواست هندآف خوانده شود، کار ادامه پیدا کند و شکاف‌های قابل‌تکمیل به ۱۰۰٪ برسد؛ تغییرات زیر کد و تست شده‌اند، اما commit/push انجام نشده است.
2. `git status --short`، `git diff --stat` و `git diff -- docs/roadmap.html` را پیش از ویرایش بررسی کن. تغییر قبلی کاربر در roadmap حفظ شده است. فایل‌های untracked را پاک نکن.
3. `./scripts/ci-local.sh` را برای اجرای کامل fetch، test، clippy، package، release و smoke استفاده کن. بار اول حدود ۳۰۰–۸۰۰MB اینترنت و چند GB فضای دیسک می‌خواهد؛ اجرای بعدی از cache استفاده می‌کند. `RUN_FMT=1` فقط برای بررسی اختیاری format کل مخزن است. برای APM از `cargo run -q -p aec-cli --bin apm -- init` و سپس `add/remove/install/list/tree` و تست integration مربوط استفاده کن؛ `aec add`/`aec install` و `apm.toml` قدیمی فقط سازگاری migration دارند. مثال آخر باید خروجی بازگشت ۴۲ بدهد؛ رفتار چاپ دقیق CLI را از اجرا ببین.
4. در کارهای مربوط به UI، علاوه بر تست واحد، پنجرهٔ واقعی، ورودی/رویداد/RTL را دستی بررسی کن؛ در محیط قبلی Chrome extension متصل نبود و QA بصری roadmap انجام نشد. `xmllint --html --noout docs/roadmap.html` موفق بود اما جای QA بصری نیست. smoke بی‌صدا و بسته‌بندی را با `timeout 12s xvfb-run -a cargo run -q -p aec-cli -- run examples/theme.aec` و `cargo package --workspace --allow-dirty --no-verify --locked` بازتولید کن.
5. برای تصمیم جدید دربارهٔ سینتکس، ماژول، سطح دسترسی یا API ابتدا تأیید سازنده را بگیر. هر قابلیت تازه نیازمند parser/AST/checker/runtime یا UI، تست و مثال هماهنگ است.

### محدودیت‌های کاری

- فقط در همین پروژه کار کن؛ تغییرات موجود را حفظ کن، `git checkout --`/reset مخرب نزن؛ commit/push/PR خودسرانه نکن.
- فایل‌های backup پارسر (`grammar.pest.backup` و `build_ast.rs.backup`) را restore نکن.
- `cargo fmt --all` کورکورانه اجرا نکن؛ ممکن است تغییرات گستردهٔ نامرتبط بسازد. فقط محدودهٔ تغییر لازم را قالب‌بندی کن.
- پیام‌های تشخیصی و کامنت سورس، شامل کامنت فایل‌های `.aec`، انگلیسی بمانند؛ متن قابل‌نمایش برنامه می‌تواند فارسی باشد.
- منبع ادعا را مشخص کن: اجرای محلی ≠ اجرای CI راه‌دور؛ محافظ مجوز مفسر ≠ sandbox سیستم‌عامل؛ نمونهٔ echo ≠ چت متصل به مدل.

## ۳. نقشهٔ معماری و جریان اجرا

`Cargo.toml` workspace نسخهٔ 0.1.0 و Rust edition 2021 / حداقل اعلام‌شدهٔ 1.75 دارد، با شش crate:

| Crate | مسئولیت | نقطهٔ شروع |
|---|---|---|
| `aec-ast` | Program، عبارت/بیانیه، تعریف agent، UI، theme، component، permissions، span | `crates/aec-ast/src/lib.rs`، `program.rs`، `expr.rs`، `ui.rs` |
| `aec-parser` | دستور زبان pest، ساخت AST و خطای پارس | `grammar.pest`، `build_ast.rs`، `lib.rs` |
| `aec-check` | تشخیص نوع/نام/عملگر، diagnostics با span و منبع آیتم | `checker.rs`، `ty.rs`، `diag.rs` |
| `aec-runtime` | مفسر tree-walking، value، builtinها، شبکه/مدل، حافظه و مجوز | `interpreter.rs`، `stdlib.rs`، `stdlib_extended.rs`، `llm.rs`، `memory.rs`، `permissions.rs` |
| `aec-ui` | تبدیل AST رابط به widget بومی egui/eframe، رویداد، state، style/theme، Vazirmatn | `renderer.rs`، `widgets.rs` |
| `aec-cli` | دستورهای `check`، `ast`، `run`، loader چندفایلی، خطای caret | `main.rs`، `loader.rs`، `render.rs` |

`aec check file.aec`: فایل ورودی → بارگذاری importها و merge → parse → checker → diagnostics با مسیر و span. `aec ast` AST ترکیب‌شده را نشان می‌دهد. `aec run` اول type-check می‌کند؛ وجود خطای نوع مانع اجراست. در صورت UI و نبود `--cli` پنجرهٔ native باز می‌شود؛ وگرنه تابع ورودی (پیش‌فرض `main`، قابل تغییر با `--entry`) در ترمینال اجرا می‌شود. موتور UI یک integration لایه‌ای با egui است، نه renderer گرافیکی سطح پایین که از صفر نوشته شده باشد.

وابستگی‌های شاخص: pest، thiserror، reqwest blocking/rustls، serde_json، rusqlite bundled، egui/eframe، uuid، regex، md5/sha2/base64. از وجود این‌ها نتیجهٔ پشتیبانی تولیدی تمام سکوها را نگیر.

## ۴. زبان AEC: امکانات و مرزها

- سرآیند `agent Name`؛ تعریف `fn`، پارامتر/نوع/return؛ `let` ثابت و `var` تغییرپذیر، assignment مرکب. کنترل جریان `if/else`، `while`، `for`، `match` و `return`.
- انواع و داده‌ها: `int`، `float`، `string`، `bool`، `bytes`، `uuid`، `timestamp`، `unit`، آرایه و object، `type?`، `Result(type, type)` و نوع تابع `function`. literal `none`، duration مانند `10s`، boolean و string.
- interpolation: عبارت معتبر در `"{expression}"` هنگام اجرا ارزیابی می‌شود؛ آکولاد نامعتبر به عنوان متن می‌ماند. closure مانند `x => x + 1` snapshot متغیرهای محلی زمان ساخت را می‌گیرد.
- `ok(value)`/`err(value)` نتیجه می‌سازند؛ `expression?` `ok` را unwrap یا `err` را از تابع فعلی برمی‌گرداند. منطق Result، پارس `none` و حفظ نوع optional در جلسات قبلی با تست تصحیح شد.
- import رشتهٔ مسیر نسبی مانند `import "./math.aec"`: CLI نسبت به فایل واردکننده resolve می‌کند، بازگشتی می‌خواند، مسیر canonical را برای dedup نگه می‌دارد، cycle، نبود فایل و declaration تکراری را خطا می‌کند، declarations را در namespace واحد ادغام می‌کند و منشأ خطای نوع را به فایل درست برمی‌گرداند. `pub fn/declaration` در alias namespace به‌عنوان export عمومی عمل می‌کند؛ declaration بدون `pub` از alias قابل فراخوانی نیست. import بدون alias برای سازگاری همچنان namespace تخت دارد. فایل‌های واردشده اجازهٔ `permissions`/`limits` ندارند.
- checker بخشی از تحلیل ایستا را انجام می‌دهد، state سراسری UI را می‌شناسد، نوع state/property/binding و رویدادها را بررسی می‌کند، index کردن غیرقابل-index را با `NotIndexable` تشخیص می‌دهد، توابع ناشناخته/arity/named arguments را گزارش می‌کند و `await` را در runtime همگام رد می‌کند؛ `Any` و استنتاج ناقص مانع ادعای «تایپ‌سیستم کامل» می‌شوند. diagnostics parser/checker قطعهٔ سورس و caret دارند.

جزئیات دستور زبان و نمونه‌های صحیح: `docs/language.md`، `examples/language.aec`، `examples/result.aec`، `examples/lambda.aec` و `examples/modules/`. نام builtin `first` ممکن است نام تابع کاربر را shadow کند؛ یک برخورد قبلاً با آرگومان تهی به panic در builtin منتهی شد؛ اگر دوباره ظاهر شد، resolution و اعتبارسنجی آرگومان builtin را با تست بازتولید و رفع کن، نه با تغییر نام تصادفی تابع مثال.

## ۵. UI بومی، state، component و theme

`ui Main = Screen "..." { ... }` بخشی از دستور زبان است. widgetهای نمونه شامل `Column`/`Row`، `Heading`، `Text`، `Input`، `Button`، `Messages` و `Card` هستند. `@name: type = value` state تعریف می‌کند؛ `Input ... bind value to name` اتصال دوطرفه دارد؛ رویداد `on click -> fn()` کد AEC را اجرا می‌کند. شاخه‌های `if` و تکرار `for` در UI هستند. خروجی desktop بومی است و فونت `crates/aec-ui/assets/fonts/Vazirmatn-Regular.ttf` در build embed می‌شود؛ مثال `theme.aec` متن فارسی/RTL را نشان می‌دهد. **نشان‌دادن نمونه ≠ تأیید کامل ظاهری در همهٔ سکوها.**

فراخوانی `push_to("messages", item)` آرایهٔ global نام‌گذاری‌شده را به‌روزرسانی می‌کند. widget tree در هر frame و پس از event با state جاری بازسازی می‌شود؛ بنابراین `Text`، شرط‌ها و `for`های مبتنی بر state واکنشی هستند. component/loop state با scope و identity جدا نگهداری می‌شود و به parent leak نمی‌کند؛ state محلیِ instance حذف‌شده در rebuild پاک می‌شود. event handler می‌تواند آرگومان داشته باشد. `examples/chatbot.aec` یک تعامل آفلاین واقعیِ echo است، نه پاسخ هوش مصنوعی؛ برای اجرای native از `cargo run -p aec-cli -- run examples/chatbot.aec` استفاده کن.

`component Name { prop ... render { ... } }` reusable است؛ props در والد ارزیابی و به کپی state محلی داده می‌شوند، expansion بازگشتی محافظت می‌شود. `theme Name default` و `theme Child extends Parent`، tokenهایی مثل `theme.color.primary`، و inline style وجود دارند. panel/card/text defaults و precedence inline در `docs/themes.md`؛ مثال `examples/theme.aec`. جزئیات: `docs/components.md` و `docs/themes.md`.

## ۶. Runtime، مدل، داده و حافظه

Builtinهای اصلی در `docs/stdlib.md` دسته‌بندی شده‌اند. این مرجع خلاصه است؛ امضای دقیق، شمار آرگومان‌ها و alias dotted/underscored را از `stdlib.rs` و `stdlib_extended.rs` بررسی کن.

- تبدیل/متن/آرایه/object/Result/زمان/ریاضی/regex/crypto/UUID در runtime وجود دارند.
- `http.get/post/put/delete`، JSON، file operations، environment و system helpers برای برنامه‌ها فراهم شده‌اند.
- `llm.complete(prompt)` یا object option از endpoint سازگار با OpenAI و به‌طور پیش‌فرض `OPENAI_API_KEY` استفاده می‌کند؛ declaration با `model primary { ... }` نیز متدهای `primary.think(prompt)`/`primary.complete(prompt)` را در همین adapter فعال می‌کند. **درخواست شبکه‌ای blocking** است، `limits.timeout` را رعایت می‌کند و response بدون `choices[0].message.content` خطا می‌دهد. هنگام بلاک مجوز، network allowlist قبل از مدل بررسی می‌شود و redirect برای درخواست‌های محدودشده دنبال نمی‌شود. این مسیر یک adapter است، نه عامل autonomous تضمین‌شده یا دمو مدل آزموده در ارائه.
- حافظهٔ گفتگو با SQLite محلی و `memory.open/add/get/count/clear` پیاده شده؛ `examples/memory.aec` نمونهٔ نگهداری بین اجراهاست. `rusqlite` با SQLite bundled است.
- `limits { timeout: 5s }` و `permissions` وجود دارند؛ timeout برای HTTP/LLM اعمال می‌شود و semantics دقیق محدودیت‌ها را از کد و تست بخوان، از نام `timeout` قطع‌کردن همهٔ محاسبات یا isolation کامل استنتاج نکن.

### مدل امنیتی، بدون اغراق

با نبودن بلاک `permissions`، رفتار قبلی builtinها unrestricted است. با وجود بلاک، شبکه/فایل/سیستم بر اساس allowlist و اصل default deny در سطح interpreter کنترل می‌شوند؛ `shell.run`، دسترسی environment، `sys.env_all` و memory پایدار رد می‌شوند تا فرمان/secret/state از مسیرهای جانبی دور زده نشود. بدون بلاک، این مسیرها همچنان ممکن‌اند. LLM و HTTP در حالت محدود redirect را دنبال نمی‌کنند. این کنترل‌ها **درون‌فرایندی و cooperative** هستند: sandbox سیستم‌عامل، جداسازی کد بومی غیرقابل‌اعتماد، اثبات امنیت مسیر فایل، یا سیاست جامع egress/CPU/حافظه ادعا نمی‌شود. قبل از استفاده برای کد نامطمئن به ارزیابی امنیتی مستقل نیاز است. فایل importشده هم سیاست root را جایگزین نمی‌کند. `examples/sandbox.aec` و `crates/aec-runtime/tests/permissions.rs` مرجع تست‌اند.

## ۷. شواهد و اجرای نمونه

| هدف | فرمان | انتظار |
|---|---|---|
| همهٔ تست‌های محلی | `cargo test --workspace --locked --no-fail-fast -q` | در آخرین اجرا: ۲۷۴ قبول، ۰ شکست، ۱ ignored |
| ساخت/بررسی | `cargo check --workspace --locked` و `cargo clippy --workspace --all-targets --locked -- -D warnings` | در آخرین اجرا بدون warning |
| چک نمونهٔ Theme | `cargo run -q -p aec-cli -- check examples/theme.aec` | parse/type-check موفق |
| ماژول چندفایلی | `cargo run -q -p aec-cli -- run examples/modules/main.aec --cli` | مقدار بازگشتی ۴۲ |
| چک چت آفلاین | `cargo run -q -p aec-cli -- check examples/chatbot.aec` | موفق، بدون API key |
| سلامت patch | `git diff --check` | بدون whitespace error |

تست‌های crateهای AST/parser/check/runtime/UI/CLI و integration در `crates/*/tests/` و `src` موجودند. تست ۲۷۴ نشان‌دهندهٔ محدودهٔ محلی است، نه benchmark، کاربر واقعی، امنیت تضمین‌شده یا سازگاری سه‌سکویی. هر آزمون جدید را اول مخصوص لایهٔ دست‌خورده و سپس در کل workspace اجرا کن. `--no-fail-fast` مانع پنهان‌ماندن شکست crateهای بعدی می‌شود.

Workflow در `.github/workflows/ci.yml` برای push/PR با matrix `ubuntu-latest`، `macos-latest`، `windows-latest` و `cargo check/test/build/package/smoke --locked` نوشته شده؛ فایل و `Cargo.lock` در working tree هستند و **هیچ گزارش اجرای remote موفقی در دست نیست**. بسته‌بندی binary و installer/release هنوز در workflow راه‌دور آزموده نشده است.

## ۸. اسناد و فایل‌های کلیدی

- `README.md`: شروع و مرزهای فعلی.
- `docs/roadmap.html`: جدول وزنی، شکاف‌ها و مسیر تا ۱۰۰٪؛ عدد ۹۷٪ فعلی تخمین داخلی است. در ابتدای نوبت تغییراتی از کاربر در این فایل وجود داشت؛ حفظ شده‌اند.
- `docs/language.md`: دستور زبان نمونه و واردکردن فایل.
- `docs/stdlib.md`: دسته‌بندی builtinها و نکتهٔ LLM/permissions.
- `docs/components.md` و `docs/themes.md`: UI reusable و tokenها.
- `examples/language.aec`، `result.aec`، `lambda.aec`، `theme.aec`، `memory.aec`، `sandbox.aec`، `modules/{main,math}.aec`، `chatbot.aec`.
- `COMPONENT_SYSTEM_HANDOFF.md`: یادداشت تاریخی دربارهٔ component، نه مرجع اول وضعیت جاری.
- `docs/AEC_Pitch_2026.pptx`: ارائهٔ مخاطب بیرونی؛ تصویر چشم‌انداز را از قابلیت تأییدشده تفکیک می‌کند.
- `docs/AEC_Presentation_FA.md`: نسخهٔ متنی همین ارائه؛ شمار تست، وضعیت alias/UI/permissions و مرزهای محصول با وضعیت فعلی هماهنگ شد.

## ۹. بک‌لاگ واقعی؛ ترتیب پیشنهادی مشروط به خواستهٔ کاربر

1. **۱۸.۶، semantics ماژول:** `pub`، alias چندلایه، duplicate declaration، path resolution، cycle، private/public و type alias پیاده و تست شده‌اند؛ nominal type و registry/signing باز است.
2. **۱۸.۴، state lifecycle:** scope/identity component و loop، rebuild واکنشی، event arguments و cleanup پیاده و تست واحد دارد؛ QA بصری پنجرهٔ واقعی در هر سه سیستم باقی است.
3. **۱۸.۸، checker فاز بعد:** UI/state/property/type/arity/unknown-function/index diagnostics، named/default arguments و type alias اضافه شده‌اند؛ nominal type و استنتاج کامل هنوز `Any`/design باز دارند.
4. **۱۸.۹ و ۱۸.۳، CI و distribution:** workflow سه‌سکویی با build/package/smoke اضافه شده و release/package محلی موفق است؛ گزارش remote، artifact release و امضا هنوز تأیید نشده‌اند.
5. **۱۸.۱۰، docs/demo:** راهنمای زبان، stdlib، component، theme، APM و مثال‌ها به‌روز شده‌اند؛ دموی آنلاین واقعی فقط با کلید و secret-management مناسب باقی است و نمونهٔ chat آفلاین است.
6. **۱۸.۱۱، APM:** binary `apm` با `init`، `add`، `remove`، `install`، `list`، `tree`، manifest/lockfile و تست integration برای path dependency محلی پیاده شده‌اند؛ registry و package signing خارج از دامنهٔ فعلی‌اند.
7. **۱۸.۱۲، فراتر از دامنه:** async/await واقعی، bytecode optimization، A2A، IDE و sandbox OS در چشم‌اندازند، نه قابلیت فعلی؛ اول مسئله و طراحی را اعتبارسنجی کن.

برای تغییر `docs/roadmap.html` هر بار denominator وزن‌ها، صورت، ردیف‌ها، ۳ عدد خلاصه و شمار تست را هماهنگ نگه دار. ۱۰۰٪ فقط پس از تعریف دامنه، تکمیل ردیف‌های الزام‌آور و آزمون محیط هدف قابل دفاع است.

## ۱۰. وضعیت working tree هنگام تحویل

در شروع نوبت قبلی، تغییر `docs/roadmap.html` از قبل موجود بود. تا این هندآف، فایل‌های اصلاح‌شده/افزوده‌شده شامل `NEXT_SESSION_HANDOFF.md`، `docs/roadmap.html`، `crates/aec-cli/src/{main,loader,package,lib}.rs`، `src/bin/apm.rs`، `scripts/ci-local.sh` و `tests/{imports,package_cli}.rs`، `crates/aec-check/src/{checker,diag,ty}.rs` و `tests/check.rs`، چند فایل runtime (`interpreter.rs`، `llm.rs`، `permissions.rs`، `stdlib.rs`، `stdlib_extended.rs`، `value.rs` و تست‌های runtime)، `crates/aec-ui/{Cargo.toml,src/renderer.rs,src/widgets.rs}`، parser/AST visibility و import، type alias، `.github/workflows/ci.yml`، `Cargo.lock`، `README.md`، `docs/apm.md`، راهنماها و نمونه‌های بالا هستند. وضعیت جدید را با `git status --short` بسنج؛ این فهرست snapshot است، نه مجوز پاک‌کردن فایل‌های دیگر.

**دستور اصلی برای عامل بعدی:** درخواست مستقیم کاربر و شواهد مخزن همیشه بر تاریخچهٔ این سند مقدم‌اند؛ ادعای «همه چیز تمام شده» را فقط به‌عنوان framing آرمانی ارائه بفهم، نه گزارش factual پیشرفت.
