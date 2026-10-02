# AEC v0.2.0 — گزارش وضعیت نهایی (۲ اکتبر ۲۰۲۶)

این سند وضعیت تأییدشدهٔ نسخهٔ **0.2.0** را ثبت می‌کند. هر ادعا با یک فرمان اجراشده و خروجی واقعی همراه است.

## ۱. چه چیزی در این جلسه اضافه شد

| ویژگی | شاهد |
|---|---|
| کلاینت HTTP(S) برای رجیستری APM | `apm install --registry http://127.0.0.1:8795` روی سرور واقعی؛ خروجی `installed 1 dependencies` |
| دانلود جریانی + verify فایل‌به‌فایل payload | هر فایل با SHA-256 و size امضاشده چک می‌شود؛ tree digest و canonical metadata در انتها |
| چرخش کلید (چند trust key) | تست `trust_key_files_accept_multiple_rotation_keys` |
| اتصال verification به runtime | `aec run` قبل از اجرا cache را با `apm.lock` می‌سنجد |
| رفع باگ `commit_staged_package` | تست regression جدید: `commit_staged_package_creates_missing_parent_directories` |

## ۲. شواهد اجرا

### ۲.۱ تست کامل workspace

```
$ cargo test --workspace --locked --offline
PASSED: 376  FAILED: 0
```

### ۲.۲ clippy

```
$ cargo clippy --workspace --all-targets --offline -- -D warnings
Finished `dev` profile [unoptimized + debuginfo] target(s) in 7.63s
```

### ۲.۳ تست end-to-end نصب از رجیستری HTTP

راه‌اندازی سرور، publish یک پکیج، و نصب از URL:

```
$ python3 -m http.server 8795 &
$ curl -sS -o /dev/null -w "%{http_code}" http://127.0.0.1:8795/packages/helper/1.0.0/metadata
200
$ apm install --registry http://127.0.0.1:8795 --trust-key keys/signing.pub
installed 1 dependencies
helper@1.0.0 -> .apm/packages/helper/1.0.0/payload
```

### ۲.۴ تست end-to-end تأیید runtime (T4)

```
$ aec run main.aec --cli     # cache سالم
✅ Type check passed
✅ Program finished!
   Result: 0

$ sed -i 's/return "hi"/return "TAMPERED"/' .apm/packages/helper/1.0.0/payload/src/lib.aec
$ aec run main.aec --cli     # cache دست‌کاری‌شده (محتوای parse-پذیر)
Error: registry package verification failed before execution
Caused by:
    cached package 'helper@1.0.0' was modified after install; run 'apm install' to restore it before running
```

## ۳. وضعیت اهداف

| هدف | وضعیت |
|---|---|
| زبان، runtime، type checker، UI واکنشی | ✅ کامل |
| APM با رجیستری امضاشدهٔ محلی | ✅ کامل |
| APM با رجیستری شبکه‌ای (HTTP/HTTPS) | ✅ کامل |
| چرخش کلید | ✅ کامل |
| verification پکیج import‌شده قبل از اجرا | ✅ کامل |
| sandbox لینوکس | ✅ verified |
| sandbox مک/ویندوز | ⏳ نیازمند میزبان همان سیستم‌عامل |
| QA بصری پنجرهٔ native روی مک/ویندوز | ⏳ نیازمند میزبان همان سیستم‌عامل |
| release v0.2.0 | 🔄 در همین جلسه |

## ۴. آنچه همچنان باز است

- **سندباکس بومی مک/ویندوز:** اسکریپت‌ها آماده‌اند (`scripts/sandbox-macos.sh` و `scripts/sandbox-windows.ps1`) ولی روی میزبان خودشان اجرا نشده‌اند.
- **QA بصری پنجرهٔ native:** تست‌های headless رد می‌شوند؛ ولی ظاهر و interaction فقط با چشم روی مک/ویندوز تأیید می‌شود.
- **خارج از دامنهٔ فعلی:** generic inference، runtime ناهمگام، A2A، IDE.

## ۵. فایل‌های تغییر‌یافته در این جلسه

```
crates/aec-cli/src/package.rs    +736/-... (کلاینت HTTP، key rotation، verify_cached_packages، رفع باگ)
crates/aec-cli/src/main.rs       +21/-...
crates/aec-cli/src/bin/apm.rs    +16/-...
crates/aec-cli/Cargo.toml        +11/-...
Cargo.toml / Cargo.lock           (نسخه 0.2.0 + reqwest workspace)
README.md / docs/apm.md           (مستندسازی رجیستری شبکه‌ای)
CHANGELOG.md                       (نسخه 0.2.0)
NEXT_SESSION_HANDOFF.md            (به‌روزرسانی وضعیت)
```
