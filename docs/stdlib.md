# Built-in functions

Calls generally accept both dotted and underscored names, for example `file.read(path)` and `file_read(path)`. User-defined `pub` declarations are exposed through their import alias (for example `models.primary.think(...)`); private declarations remain module-local. The runtime validates argument counts before dispatch; the table below is the supported surface.

| Group | Common calls | Description |
|---|---|---|
| Core | `print(...)`, `len(value)`, `str(value)`, `int(value)`, `float(value)`, `bool(value)` | Output and conversion; `print` accepts any number of values |
| Text | `upper`, `lower`, `trim`, `split`, `join`, `contains`, `replace`, `starts_with`, `ends_with` | String operations |
| Collections | `push`, `push_to`, `pop`, `sort`, `reverse`, `first`, `last`, `slice`, `range`, `keys`, `values`, `has` | Arrays and objects |
| Results | `ok(value)`, `err(value)`, `is_ok(value)`, `is_err(value)` | Result construction and inspection |
| Files | `file.read`, `file.write`, `file.append`, `file.exists`, `file.delete`, `file.copy`, `file.mkdir`, `file.size`, `file.list_dir` | Local file operations |
| HTTP / JSON | `http.get`, `http.post`, `http.put`, `http.delete`, `json.parse`, `json.stringify` | Requests and JSON |
| Environment | `env.get`, `env.set`, `sys.info`, `sys.args`, `sys.env_all` | Process environment |
| Time / math | `time.now_ms`, `time.now_sec`, `time.sleep`, `math.sin`, `math.cos`, `math.random`, `min`, `max`, `abs`, `sqrt`, `pow` | Clock and math |
| Data | `regex.match`, `regex.find`, `regex.find_all`, `regex.replace`, `crypto.sha256`, `crypto.sha512`, `uuid.v4` | Patterns, hashes, IDs |
| Models | `llm.complete(prompt)`, `model.think(prompt)` | OpenAI-compatible chat-completions request; model declarations provide `think`/`complete` |
| Memory | `memory.open(path)`, `memory.add(conversation, role, content)`, `memory.get(conversation)`, `memory.count(conversation)`, `memory.clear(conversation)` | SQLite-backed conversation memory after `open` |

`push_to("messages", item)` updates the **global** array named `messages`; the first argument is its name as a string. It is useful with screen-level UI state. `llm.complete` accepts either a prompt string or an options object; a declared `model` exposes the same request through `model.think(prompt)` or `model.complete(prompt)`. It uses `OPENAI_API_KEY` when no key is passed, defaults to the configured OpenAI-compatible endpoint, applies `limits.timeout`, and makes a blocking network request. The offline chat example does not need an API key.

`permissions` restricts HTTP, model, file, environment, memory, and system calls through the interpreter and blocks `shell.run`. HTTP and model requests do not follow redirects when a permissions block is present. No `permissions` block means the legacy unrestricted mode. The full implementation and argument checks live in `crates/aec-runtime/src/interpreter.rs`, `stdlib.rs`, and `stdlib_extended.rs`.
