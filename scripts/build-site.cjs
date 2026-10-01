#!/usr/bin/env node
"use strict";

/*
 * Builds website/ from the Markdown in docs/.
 *
 * Output is fully static: one HTML file per document, a shared stylesheet, and
 * a JSON-free search index that site.js reads at runtime. There is no build
 * dependency to install, which keeps `node scripts/build-site.mjs` working on a
 * clean checkout.
 */

const fs = require("node:fs");
const path = require("node:path");

const ROOT = path.resolve(__dirname, "..");
const DOCS = path.join(ROOT, "docs");
const OUT = path.join(ROOT, "website");

const PAGES = [
  // `src` is relative to the repository root. `textDir` sets the page
  // direction, which must not collide with a path field.
  { src: "README.md", out: "guide.html", title: "Guide", nav: "Guide", order: 1 },
  { src: "docs/language.md", out: "language.html", title: "Language reference", nav: "Language", order: 2 },
  { src: "docs/stdlib.md", out: "stdlib.html", title: "Built-in functions", nav: "Built-ins", order: 3 },
  { src: "docs/components.md", out: "components.html", title: "UI components", nav: "Components", order: 4 },
  { src: "docs/themes.md", out: "themes.html", title: "Themes and styles", nav: "Themes", order: 5 },
  { src: "docs/apm.md", out: "apm.html", title: "Package manager", nav: "Packages", order: 6 },
  { src: "docs/release.md", out: "release.html", title: "Release guide", nav: "Release", order: 7 },
  { src: "docs/AEC_Presentation_FA.md", out: "presentation.html", title: "ارائه (فارسی)", nav: "Presentation", order: 8, lang: "fa", textDir: "rtl" },
];

const SIDEBAR = [
  { group: "Start", links: [
    { href: "/index.html", text: "Home" },
    { href: "/guide.html", text: "Guide" },
    { href: "/language.html", text: "Language" },
  ]},
  { group: "Reference", links: [
    { href: "/stdlib.html", text: "Built-ins" },
    { href: "/components.html", text: "Components" },
    { href: "/themes.html", text: "Themes" },
  ]},
  { group: "Ecosystem", links: [
    { href: "/apm.html", text: "Packages" },
    { href: "/registry.html", text: "Registry" },
  ]},
  { group: "Project", links: [
    { href: "/roadmap.html", text: "Roadmap" },
    { href: "/release.html", text: "Release" },
  ]},
];

/* ---------------- tiny Markdown renderer ---------------- */

// Escapes text, but preserves the inline spans we style ourselves.
function escapeHtml(text) {
  return text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

function inline(text) {
  let out = escapeHtml(text);

  // Inline code first so its contents are not touched by later rules. The
  // sentinel is a run of underscores that cannot occur in an index number, and
  // the stored span is re-escaped on the way out.
  const codeSpans = [];
  out = out.replace(/`([^`]+)`/g, (_, code) => {
    codeSpans.push(code);
    return `@@CODE${codeSpans.length - 1}@@`;
  });

  out = out.replace(/\[([^\]]+)\]\(([^)\s]+)\)/g, (_, label, href) => {
    const external = /^https?:/.test(href);
    const attrs = external ? ' target="_blank" rel="noopener"' : "";
    return `<a href="${href}"${attrs}>${label}</a>`;
  });

  out = out
    .replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>")
    .replace(/(^|[\s(])\*([^*\n]+)\*/g, "$1<em>$2</em>");

  out = out.replace(/@@CODE(\d+)@@/g, (_, index) => `<code>${codeSpans[Number(index)]}</code>`);
  return out;
}

function slug(text) {
  return text
    .toLowerCase()
    .replace(/`/g, "")
    .replace(/[^\p{L}\p{N}]+/gu, "-")
    .replace(/^-+|-+$/g, "");
}

function renderMarkdown(md) {
  const lines = md.split("\n");
  const html = [];
  const headings = [];
  let i = 0;
  let paragraph = [];
  let inCode = false;
  let codeLang = "";
  let codeBuf = [];
  let listType = null;
  let inTable = false;
  let tableRows = [];

  const flushParagraph = () => {
    if (paragraph.length) {
      html.push(`<p>${inline(paragraph.join(" "))}</p>`);
      paragraph = [];
    }
  };
  const closeList = () => {
    if (listType) {
      html.push(`</${listType}>`);
      listType = null;
    }
  };
  const closeTable = () => {
    if (inTable) {
      const [header, , ...body] = tableRows;
      html.push("<table><thead><tr>");
      header.forEach((cell) => html.push(`<th>${inline(cell)}</th>`));
      html.push("</tr></thead><tbody>");
      body.forEach((row) => {
        html.push("<tr>");
        row.forEach((cell) => html.push(`<td>${inline(cell)}</td>`));
        html.push("</tr>");
      });
      html.push("</tbody></table>");
      inTable = false;
      tableRows = [];
    }
  };
  const closeBlocks = () => {
    flushParagraph();
    closeList();
    closeTable();
  };

  while (i < lines.length) {
    const line = lines[i];

    if (line.startsWith("```")) {
      if (inCode) {
        const cls = codeLang ? ` class="lang-${escapeHtml(codeLang)}"` : "";
        html.push(`<pre><code${cls}>${escapeHtml(codeBuf.join("\n"))}</code></pre>`);
        inCode = false;
        codeBuf = [];
        codeLang = "";
      } else {
        closeBlocks();
        inCode = true;
        codeLang = line.slice(3).trim();
      }
      i++;
      continue;
    }
    if (inCode) {
      codeBuf.push(line);
      i++;
      continue;
    }

    // Table row
    if (line.trim().startsWith("|") && line.trim().endsWith("|")) {
      flushParagraph();
      closeList();
      const cells = line.trim().slice(1, -1).split("|").map((c) => c.trim());
      if (cells.every((c) => /^:?-{2,}:?$/.test(c))) {
        i++;
        continue;
      }
      inTable = true;
      tableRows.push(cells);
      i++;
      continue;
    }
    closeTable();

    // Heading
    const heading = /^(#{1,4})\s+(.*)$/.exec(line);
    if (heading) {
      closeBlocks();
      const level = heading[1].length;
      const text = heading[2].trim();
      const id = slug(text);
      if (level === 2 || level === 3) headings.push({ level, text, id });
      html.push(`<h${level} id="${id}">${inline(text)}</h${level}>`);
      i++;
      continue;
    }

    // Horizontal rule
    if (/^(-{3,}|\*{3,})$/.test(line.trim())) {
      closeBlocks();
      html.push("<hr />");
      i++;
      continue;
    }

    // Blockquote
    if (line.startsWith("> ")) {
      closeBlocks();
      const buf = [];
      while (i < lines.length && lines[i].startsWith("> ")) {
        buf.push(lines[i].slice(2));
        i++;
      }
      html.push(`<blockquote>${renderMarkdown(buf.join("\n")).html}</blockquote>`);
      continue;
    }

    // Lists
    const bullet = /^[-*]\s+(.*)$/.exec(line);
    const numbered = /^\d+\.\s+(.*)$/.exec(line);
    if (bullet || numbered) {
      flushParagraph();
      const want = bullet ? "ul" : "ol";
      if (listType !== want) {
        closeList();
        html.push(`<${want}>`);
        listType = want;
      }
      html.push(`<li>${inline((bullet || numbered)[1])}</li>`);
      i++;
      continue;
    }
    closeList();

    if (line.trim() === "") {
      closeBlocks();
      i++;
      continue;
    }

    paragraph.push(line.trim());
    i++;
  }

  if (inCode && codeBuf.length) {
    html.push(`<pre><code>${escapeHtml(codeBuf.join("\n"))}</code></pre>`);
  }
  closeBlocks();

  return { html: html.join("\n"), headings };
}

/* ---------------- plain text, for the search index ---------------- */

function toPlainText(md) {
  return md
    .replace(/```[\s\S]*?```/g, " ")
    .replace(/`([^`]+)`/g, "$1")
    .replace(/!\[[^\]]*\]\([^)]*\)/g, " ")
    .replace(/\[([^\]]+)\]\([^)]*\)/g, "$1")
    .replace(/[#>*_|~-]/g, " ")
    .replace(/\s+/g, " ")
    .trim();
}

/* ---------------- page shell ---------------- */

function page({ title, description, body, headings, index, lang, dir }) {
  const pageLang = lang || "en";
  const pageDir = dir || "ltr";
  const side = SIDEBAR.map((group) => `
    <div class="side-group">
      <h4>${group.group}</h4>
      <nav>
        ${group.links.map((l) => `<a href="${l.href}">${l.text}</a>`).join("\n        ")}
      </nav>
    </div>`).join("\n");

  const toc = headings.length >= 3 ? `
  <details class="toc" open>
    <summary>On this page</summary>
    <ol>
      ${headings.map((h) => `<li><a href="#${h.id}">${escapeHtml(h.text)}</a></li>`).join("\n      ")}
    </ol>
  </details>` : "";

  return `<!DOCTYPE html>
<html lang="${pageLang}" dir="${pageDir}">
<head>
<meta charset="utf-8" />
<meta name="viewport" content="width=device-width, initial-scale=1" />
<title>${escapeHtml(title)} — AEC</title>
<meta name="description" content="${escapeHtml(description)}" />
<link rel="icon" href="data:image/svg+xml,<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 100 100'><text y='.9em' font-size='90'>⚡</text></svg>" />
<link rel="stylesheet" href="/assets/site.css" />
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
  <aside class="side">
    <div class="search">
      <input id="q" type="search" placeholder="Search the docs…" autocomplete="off" />
      <div class="results" id="results"></div>
    </div>
${side}
  </aside>
  <article class="prose">
${toc}
${body}
  </article>
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

<script>window.AEC_INDEX = ${JSON.stringify(index)};</script>
<script src="/assets/site.js"></script>
</body>
</html>
`;
}

/* ---------------- build ---------------- */

function main() {
  fs.mkdirSync(path.join(OUT, "assets"), { recursive: true });

  // Read every document first: the search index has to be complete before any
  // page is written, because each page embeds the whole index.
  const docs = [];
  const index = [];

  for (const spec of PAGES) {
    const source = path.join(ROOT, spec.src);
    if (!fs.existsSync(source)) {
      console.warn(`skip ${spec.src} (missing)`);
      continue;
    }
    const md = fs.readFileSync(source, "utf8");
    const { html, headings } = renderMarkdown(md);
    const body = html.startsWith("<h1")
      ? html
      : `<h1 id="${slug(spec.title)}">${escapeHtml(spec.title)}</h1>\n${html}`;
    docs.push({ spec, body, headings });

    const plain = toPlainText(md);
    index.push({ url: `/${spec.out}`, anchor: "", title: spec.title, text: plain.slice(0, 1200) });
    for (const heading of headings.filter((h) => h.level === 2)) {
      index.push({
        url: `/${spec.out}`,
        anchor: heading.id,
        title: `${spec.title} › ${heading.text}`,
        text: plain.slice(0, 600),
      });
    }
  }

  for (const { spec, body, headings } of docs) {
    fs.writeFileSync(
      path.join(OUT, spec.out),
      page({
        title: spec.title,
        description: `${spec.title} for the AEC programming language.`,
        body,
        headings,
        index,
        lang: spec.lang,
        dir: spec.textDir,
      })
    );
    console.log(`built ${spec.out} (${headings.length} headings)`);
  }

  // The roadmap is hand-written HTML with a Persian layout, so it is copied
  // rather than regenerated.
  const roadmap = path.join(DOCS, "roadmap.html");
  if (fs.existsSync(roadmap)) {
    fs.copyFileSync(roadmap, path.join(OUT, "roadmap.html"));
    console.log("copied roadmap.html");
  }

  // Persian pages need the bundled font at a URL the site can serve. It is the
  // same file the native renderer embeds, so the website matches the window.
  const font = path.join(ROOT, "crates/aec-ui/assets/fonts/Vazirmatn-Regular.ttf");
  if (fs.existsSync(font)) {
    fs.copyFileSync(font, path.join(OUT, "assets", "Vazirmatn-Regular.ttf"));
    console.log("copied Vazirmatn-Regular.ttf");
  }

  console.log(`\n${docs.length} pages, ${index.length} search entries`);
  console.log("preview:  cd website && python3 -m http.server 8000");
}

main();
