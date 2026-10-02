# AEC — گزارش وضعیت نهایی (۲ اکتبر ۲۰۲۶)

این گزارش فقط چیزهایی را می‌گوید که **روی همین ماشین یا روی گیت‌هاب اجرا و تأیید شده‌اند**.
هیچ ادعایی بدون شاهد اجرا در این سند نیست.

## ۱. آنچه این جلسه واقعاً انجام شد

### الف) سه شکست واقعی CI که پیدا و رفع شد
این‌ها با خواندن کد پیدا نمی‌شدند؛ فقط با اجرای واقعی روی رانر ظاهر شدند.

| # | شکست | ریشه | رفع |
|---|---|---|---|
| ۱ | `ubuntu` — مرحلهٔ `Linux OS sandbox smoke test` | مسیر `/workspace/...` داده می‌شد که فقط **داخل** mount مربوط به bubblewrap وجود دارد، نه روی میزبان. ضمناً اوبونتو ۲۴.۰۴ محدودیت user-namespace دارد که bwrap لازم دارد. | مسیر میزبان (`$PWD/...`) داده شد؛ `sysctl` محدودیت userns برداشته شد |
| ۲ | `windows` — مرحلهٔ `Check workspace` | `tui_driver.rs` از `libc::termios` بدون گارد پلتفرم استفاده می‌کرد؛ `libc` روی ویندوز `termios` ندارد | `RawMode` زیر `#[cfg(unix)]` با جانشین no-op |
| ۳ | `windows` — مرحلهٔ `Lint workspace` | پارامتر `private` در `write_new_file` فقط زیر `#[cfg(unix)]` استفاده می‌شد → `clippy -D warnings` آن را «unused variable» می‌گرفت | مصرف صریح متغیر در حالت غیر-POSIX |

### ب) شکست مک/ویندوز در تست‌های APM
همهٔ تست‌های `apm`/`package` روی مک و ویندوز می‌شکستند و روی لینوکس پاس می‌شدند.

**ریشه:** `ensure_no_symlink_components` کل مسیر را **از ریشهٔ فایل‌سیستم** می‌پیمود و هر symlink را رد می‌کرد.
- مک: temp زیر `/var` است و `/var` symlink به `/private/var` است → همهٔ fixtureها رد می‌شدند.
- ویندوز: ریشهٔ temp شامل junction است → همان مشکل.

**رفع:** کامپوننت‌های هم‌تراز/بالاتر از temp و cwd **قابل‌اعتماد** شمرده می‌شوند (هم 형태 lexical و هم canonical).
symlink **زیر** یک پیشوند سیستمی همچنان رد می‌شود — چون آنجا جایی است که یک لینک می‌تواند یک read/write را به درخت دیگری منحرف کند.
تست رد symlink در payload دست‌نخورده پاس می‌شود.

### ج) شکاف‌های ابزاری
- clippy **فقط** در `release.yml` اجرا می‌شد، پس خرابی ویندوز تا لحظهٔ tag مخفی ماند → حالا در `ci.yml` روی هر سه پلتفرم اجرا می‌شود.
- سرور لاگ GitHub از این شبکه در دسترس نیست (`gh run view --log` تایم‌اوت می‌دهد).
  راه‌حل: `scripts/ci-diagnose.sh` و `scripts/ci-clippy.sh` که خطاها را به **check-run annotation** تبدیل می‌کنند؛
  این‌ها با `gh api repos/<owner>/<repo>/check-runs/<job id>/annotations` خوانده می‌شوند.

### د) انتشار واقعی
- release عمومی: `https://github.com/aec-lang/aec/releases/tag/v0.1.0`
- فایل‌ها: `aec-linux-x86_64.tar.gz`، `aec-macos-x86_64.tar.gz`، `aec-windows-x86_64.zip`، `SHA256SUMS` و `.asc` برای هر سه.
- کلید انتشار واقعی GPG (Ed25519) ساخته و در secretهای مخزن تنظیم شد: `GPG_RELEASE_KEY`، `GPG_PASSPHRASE`، `GPG_KEY_ID`.
- یک پکیج واقعی APM (`hello-agent@0.1.0`) با `apm publish` منتشر و با `apm verify` تأیید شد و در مخزن commit شد.
- سایت مستندات آنلاین: `https://aec-lang.github.io/aec/`

## ۲. شواهد اجرا

```
$ cargo test --workspace --locked --no-fail-fast
PASSED: 373  FAILED: 0

$ cargo clippy --workspace --all-targets --locked -- -D warnings
Finished `dev` profile ... (clean)

$ gh api repos/aec-lang/aec/actions/runs/<id>/jobs
Check and test (ubuntu-latest):  success
Check and test (macos-latest):   success
Check and test (windows-latest): success

$ ./scripts/sandbox-selftest.sh
linux      verified  bubblewrap boundary ran aec check successfully
macos      untested  requires a macOS host to verify
windows    untested  requires a Windows host to verify

# دانلود عمومی از release، بدون احراز هویت:
$ sha256sum --check SHA256SUMS --ignore-missing
aec-linux-x86_64.tar.gz: OK
$ gpg --verify aec-linux-x86_64.tar.gz.asc aec-linux-x86_64.tar.gz
gpg: Good signature from "AEC Release Signing ..."
$ ./aec-linux-x86_64 --version
aec 0.1.0
```

## ۳. امتیاز وزنی: ۹۷٪

جدول `docs/roadmap.html`: وزن کل **۱۲۲**، امتیاز **۱۱۸.۳۵** → **۹۷٪**.
حساب هر ردیف (`weight × percent = score`) بازبینی و تأیید شد؛ هیچ ردیفی جمع نمی‌زند اشتباه.

## ۴. آنچه واقعاً باز است (و باید باز بماند)

| مورد | چرا باز است |
|---|---|
| سندباکس بومی مک/ویندوز | به میزبان همان سیستم‌عامل‌ها نیاز دارد؛ اسکریپت‌ها نوشته شده‌اند اما روی آن سیستم‌عامل‌ها اجرا نشده‌اند |
| QA بصری پنجرهٔ native روی مک/ویندوز | تست‌ها headless و روی هر سه پلتفرم سبزند، اما بازبینی چشمی پنجرهٔ واقعی روی آن دو انجام نشده |
| registry شبکه‌ای APM | کلاینت HTTP، چرخش کلید و اتصال verification به import رانتایم وجود ندارد |
| فراتر از دامنه | generic inference، runtime ناهمگام، A2A و IDE |

بدون یک دستگاه مک و یک دستگاه ویندوز، عدد ۱۰۰٪ برای «محصول production» ادعای غیرقابل دفاعی است.
هر وقت این دو میزبان موجود بود، با همان اسکریپت‌های آماده (`sandbox-macos.sh`، `sandbox-windows.ps1`،
`sandbox-selftest.sh`) دقیقاً همین تأیید انجام می‌شود و این دو ردیف هم بسته می‌شود.

## ۵. یادداشت امنیتی

کلید خصوصی امضای انتشار **فقط** در secret مخزن GitHub نگهداری می‌شود و در هیچ فایل مخزن یا مسیر محلی نیست
(`.gitignore` شامل `*.pkcs8` است). کپی موقت محلی پس از تنظیم secret پاک شد.
