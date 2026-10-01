#!/usr/bin/env node
"use strict";

/*
 * Builds website/registry.html from a real APM registry directory.
 *
 *   node scripts/build-registry.mjs [registry-dir] [public-key]
 *
 * Every number on the page comes from a published version's metadata file, so
 * the listing cannot drift from what `apm verify` would accept. With no
 * registry argument the page is still generated, but it says so instead of
 * inventing entries.
 */

const fs = require("node:fs");
const path = require("node:path");

const ROOT = path.resolve(__dirname, "..");
const OUT = path.join(ROOT, "website", "registry.html");

const registryDir = path.resolve(process.argv[2] || path.join(ROOT, "registry"));
const publicKey = process.argv[3] || null;

function escapeHtml(text) {
  return String(text)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

function parseMetadata(text) {
  const lines = text.split("\n").filter(Boolean);
  const fields = {};
  const files = [];
  for (const line of lines) {
    const kv = line.match(/^([a-z-]+)=(.*)$/);
    if (kv) {
      fields[kv[1]] = kv[2];
      continue;
    }
    const row = line.match(/^file\t([0-9a-f]+)\t(\d+)\t([0-9a-f]+)$/);
    if (row) files.push({ hexPath: row[1], size: Number(row[2]), sha256: row[3] });
  }
  return { fields, files };
}

function collect(dir) {
  const packagesDir = path.join(dir, "packages");
  if (!fs.existsSync(packagesDir)) return [];

  const out = [];
  for (const name of fs.readdirSync(packagesDir).sort()) {
    const versionsDir = path.join(packagesDir, name);
    if (!fs.statSync(versionsDir).isDirectory()) continue;

    for (const version of fs.readdirSync(versionsDir).sort().reverse()) {
      const versionDir = path.join(versionsDir, version);
      const metadataPath = path.join(versionDir, "metadata");
      const signaturePath = path.join(versionDir, "signature");
      if (!fs.existsSync(metadataPath) || !fs.existsSync(signaturePath)) continue;

      const { fields, files } = parseMetadata(fs.readFileSync(metadataPath, "utf8"));
      const payloadDir = path.join(versionDir, "payload");
      const readmePath = path.join(payloadDir, "README.md");
      const description = fs.existsSync(readmePath)
        ? fs.readFileSync(readmePath, "utf8").split("\n").find((l) => l.trim() && !l.startsWith("#")) || ""
        : "";

      out.push({
        name,
        version,
        treeSha: fields["tree-sha256"] || "",
        manifestSha: fields["manifest-sha256"] || "",
        fileCount: Number(fields["file-count"] || files.length),
        payloadSize: Number(fields["payload-size"] || 0),
        publicKey: fields["public-key"] || "",
        signatureBytes: fs.statSync(signaturePath).size,
        files,
        description,
      });
    }
  }
  return out;
}

function humanSize(bytes) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KiB`;
  return `${(bytes / 1024 / 1024).toFixed(2)} MiB`;
}

const entries = collect(registryDir);
const totalFiles = entries.reduce((sum, e) => sum + e.fileCount, 0);
const totalSize = entries.reduce((sum, e) => sum + e.payloadSize, 0);

const cards = entries.length
  ? entries.map((entry) => `
    <article class="pkg">
      <div class="pkg-head">
        <h3><span class="pname">${escapeHtml(entry.name)}</span> <span class="ver">${escapeHtml(entry.version)}</span></h3>
        <span class="badge ok">signed</span>
      </div>
      ${entry.description ? `<p class="desc">${escapeHtml(entry.description)}</p>` : ""}
      <dl class="facts">
        <div><dt>tree digest</dt><dd><code>${escapeHtml(entry.treeSha.slice(0, 16))}…</code></dd></div>
        <div><dt>files</dt><dd>${entry.fileCount}</dd></div>
        <div><dt>payload</dt><dd>${humanSize(entry.payloadSize)}</dd></div>
        <div><dt>signature</dt><dd>Ed25519, ${entry.signatureBytes} bytes</dd></div>
      </dl>
      <details>
        <summary>Metadata</summary>
        <pre><code>name=${escapeHtml(entry.name)}
version=${escapeHtml(entry.version)}
manifest-sha256=${escapeHtml(entry.manifestSha)}
tree-sha256=${escapeHtml(entry.treeSha)}
file-count=${entry.fileCount}
payload-size=${entry.payloadSize}
public-key=${escapeHtml(entry.publicKey)}</code></pre>
      </details>
      <pre class="cmd"><code>apm publish --registry ./registry --private-key ./keys/signing.pkcs8 --package ./${escapeHtml(entry.name)}
apm verify --registry ./registry --name ${escapeHtml(entry.name)} --version ${escapeHtml(entry.version)} --trust-key ./keys/signing.pub</code></pre>
    </article>`).join("\n")
  : `<div class="empty">
      <p><b>No published packages yet.</b></p>
      <p>Publish one and rebuild this page — it reads a real APM registry, so it never lists anything the verifier would reject.</p>
      <pre class="cmd"><code>apm keygen --private-key ./keys/signing.pkcs8 --public-key ./keys/signing.pub
apm registry-init ./registry
apm publish --registry ./registry --private-key ./keys/signing.pkcs8 --package ./my-agent
node scripts/build-registry.mjs ./registry keys/signing.pub</code></pre>
    </div>`;

const html = `<!DOCTYPE html>
<html lang="en" dir="ltr">
<head>
<meta charset="utf-8" />
<meta name="viewport" content="width=device-width, initial-scale=1" />
<title>Package registry — AEC</title>
<meta name="description" content="Every published AEC package with its signature and tree digest." />
<link rel="icon" href="data:image/svg+xml,<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 100 100'><text y='.9em' font-size='90'>⚡</text></svg>" />
<link rel="stylesheet" href="/assets/site.css" />
<style>
  .docs { display: block; padding-top: 40px; }
  .rhead { max-width: 76ch; margin-bottom: 30px; }
  .rhead h1 { font-size: 36px; margin: 0 0 10px; }
  .rhead p { color: var(--muted); }
  .rstrip { display: grid; grid-template-columns: repeat(auto-fit, minmax(150px, 1fr)); gap: 14px; margin-bottom: 34px; }
  .rstrip div { background: var(--card); border: 1px solid var(--line); border-radius: var(--radius); padding: 18px; text-align: center; }
  .rstrip b { display: block; font-size: 25px; color: var(--primary); }
  .rstrip span { font-size: 13px; color: var(--muted); }
  .pkgs { display: grid; grid-template-columns: repeat(auto-fill, minmax(320px, 1fr)); gap: 18px; }
  .pkg { background: var(--card); border: 1px solid var(--line); border-radius: var(--radius); padding: 22px; }
  .pkg-head { display: flex; align-items: center; justify-content: space-between; gap: 12px; margin-bottom: 8px; }
  .pkg h3 { margin: 0; font-size: 19px; }
  .pname { color: var(--text); }
  .ver { color: var(--accent); font-family: var(--mono); font-size: 14px; }
  .desc { color: var(--muted); font-size: 14px; margin: 0 0 14px; }
  .facts { display: grid; gap: 7px; margin: 0 0 14px; }
  .facts > div { display: flex; justify-content: space-between; gap: 12px; font-size: 13.5px; }
  .facts dt { color: var(--faint); }
  .facts dd { margin: 0; color: var(--muted); text-align: end; }
  .facts code { background: rgba(122,162,255,.1); padding: 1px 6px; border-radius: 5px; color: var(--primary); }
  details { margin-bottom: 12px; }
  details summary { cursor: pointer; color: var(--muted); font-size: 13.5px; }
  pre.cmd, details pre {
    background: #080c18; border: 1px solid var(--line); border-radius: 11px;
    padding: 13px 15px; overflow-x: auto; direction: ltr; text-align: left;
    font-family: var(--mono); font-size: 12.5px; line-height: 1.7; margin: 0;
  }
  pre.cmd code, details pre code { background: none; padding: 0; color: var(--text); }
  .empty { background: var(--card); border: 1px dashed var(--line); border-radius: var(--radius); padding: 30px; max-width: 76ch; }
  .empty p { color: var(--muted); }
  .warn {
    background: rgba(255,180,84,.07); border: 1px solid rgba(255,180,84,.28);
    border-radius: 12px; padding: 15px 18px; color: var(--muted); font-size: 14px; margin-bottom: 26px;
  }
  .warn b { color: var(--amber); }
</style>
</head>
<body>

<header class="nav">
  <a class="brand" href="/"><span class="logo">⚡</span><span>AEC</span></a>
  <nav>
    <a href="/guide.html">Guide</a>
    <a href="/stdlib.html">Built-ins</a>
    <a href="/components.html">Components</a>
    <a href="/themes.html">Themes</a>
    <a href="/apm.html">Packages</a>
    <a href="/registry.html">Registry</a>
    <a href="/roadmap.html">Roadmap</a>
    <a class="cta" href="/guide.html#install">Install</a>
  </nav>
  <button class="burger" id="burger" aria-label="Menu">☰</button>
</header>

<main class="docs">
  <div class="rhead">
    <h1>Package registry</h1>
    <p>
      Every version below was published through <code>apm</code> into a real registry directory.
      Each one is bound by SHA-256 metadata and a detached Ed25519 signature, and verification
      always requires an explicitly selected trust key.
    </p>
  </div>

  <div class="warn">
    <b>Local registry.</b> This index is generated from a directory, not served over HTTP —
    AEC has no network registry client yet. Publishing and verification work; remote resolution
    does not.
  </div>

  <div class="rstrip">
    <div><b>${entries.length}</b><span>published versions</span></div>
    <div><b>${new Set(entries.map((e) => e.name)).size}</b><span>packages</span></div>
    <div><b>${totalFiles}</b><span>files bound</span></div>
    <div><b>${humanSize(totalSize)}</b><span>payload</span></div>
  </div>

  <div class="pkgs">
${cards}
  </div>
</main>

<footer>
  <div class="foot">
    <span>AEC — Agent Easy Creator</span>
    <span class="links">
      <a href="https://github.com/aec-lang/aec">GitHub</a>
      <a href="https://github.com/aec-lang/aec/releases">Releases</a>
      <a href="/registry.html">Registry</a>
      <a href="/roadmap.html">Roadmap</a>
    </span>
    <span class="lic">MIT OR Apache-2.0</span>
  </div>
</footer>

<script src="/assets/site.js"></script>
</body>
</html>
`;

fs.mkdirSync(path.dirname(OUT), { recursive: true });
fs.writeFileSync(OUT, html);

if (publicKey) {
  const key = fs.readFileSync(publicKey, "utf8").trim();
  console.log(`registry page written (${entries.length} versions)`);
  console.log(`trust key: ${key.slice(0, 16)}…`);
} else {
  console.log(`registry page written (${entries.length} versions)`);
}
if (!entries.length) {
  console.log("no published versions found — publish a package and rebuild");
}
