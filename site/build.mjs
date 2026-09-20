// Renders the repository's markdown into colloq.dev.
// The docs live in ../docs and ../rfcs and stay the single source. This only
// wraps them in the site shell, so a doc change never has to be written twice.

import { Marked } from "marked";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const repo = path.resolve(here, "..");
const out = path.join(here, "dist");

const DOCS = [
  { slug: "quickstart", file: null, title: "Quickstart", group: "Start" },
  { slug: "conversations", file: "docs/language.md", title: "The language surface", group: "Start" },
  { slug: "architecture", file: "docs/architecture.md", title: "Architecture", group: "Model" },
  { slug: "design", file: "docs/design.md", title: "Design principles", group: "Model" },
  { slug: "plan", file: "docs/plan.md", title: "Plans and identity", group: "Model" },
  { slug: "wire", file: "docs/wire.md", title: "Colloq Wire", group: "Model" },
  { slug: "runtime", file: "docs/runtime.md", title: "Runtime", group: "Running it" },
  { slug: "two-node", file: "docs/two-node.md", title: "Two-node runbook", group: "Running it" },
  { slug: "substrates", file: "docs/substrates.md", title: "Substrates", group: "Running it" },
  { slug: "benchmark", file: "docs/benchmark.md", title: "Benchmark", group: "Evidence" },
  { slug: "evolution", file: "docs/evolution.md", title: "Governed evolution", group: "Evidence" },
  { slug: "prior-art", file: "docs/prior-art.md", title: "Prior art", group: "Evidence" },
  { slug: "roadmap", file: "docs/roadmap.md", title: "Roadmap", group: "Evidence" },
  { slug: "jev", file: "docs/jev.md", title: "Jev chooser (optional)", group: "Extensions" },
];

const RFCS = [
  { slug: "0001-language-kernel", file: "rfcs/0001-colloq-language-kernel.md", title: "RFC-0001 · Language kernel" },
  { slug: "0002-conversation", file: "rfcs/0002-conversation-is-the-computation.md", title: "RFC-0002 · The conversation is the computation" },
  { slug: "0003-substrates", file: "rfcs/0003-pluggable-substrates.md", title: "RFC-0003 · Pluggable substrates" },
  { slug: "0004-evidence", file: "rfcs/0004-evidence-protocol.md", title: "RFC-0004 · Evidence protocol" },
];

const SCHEMAS = [
  ["colloq-conversation-v0", "Conversation", "The global conversation: roles, messages, choices, loops, cancellation and declared failures. This is the file you write."],
  ["colloq-plan-v0", "Plan", "The compiled artifact: conversation identity, plan identity, one endpoint graph per role and the compact transition table."],
  ["colloq-endpoint-v0", "Endpoint", "One role's projected state machine, as produced by <code>colloq project</code>."],
  ["colloq-session-v0", "Session preface", "The preface two peers exchange before frame zero. A mismatch stops the session there."],
  ["colloq-node-v0", "Node identity", "A local, secret-bearing Iroh identity. Written with mode 0600 and never shared."],
  ["colloq-endpoint-v0-ticket", "Endpoint ticket", "The public direct-address ticket for a persistent endpoint identity.", "colloq-endpoint-v0"],
  ["colloq-authorization-v0", "Authorization", "Which authenticated peer may take which role under which exact plan. No wildcards."],
  ["colloq-graph-v0", "Graph", "The wider graph interchange experiment from RFC-0001."],
  ["colloq-evidence-v0", "Evidence", "A reproducible correctness, security or performance result, tied to one revision."],
  ["colloq-jev-v0", "Jev binding (optional)", "Binds one choice state to a typed model question, with an explicit threshold and escalation branch."],
];

const marked = new Marked({ gfm: true, mangle: false, headerIds: false });

const esc = (s) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");

function rewriteLinks(html) {
  // Repository-relative links become site links, so a reader never leaves the docs.
  return html
    .replace(/href="(?:\.\.\/)?docs\/([a-z0-9-]+)\.md(#[^"]*)?"/g, (_m, slug, hash) => {
      const known = DOCS.find((d) => d.file === `docs/${slug}.md`);
      return `href="/docs/${known ? known.slug : slug}/${hash || ""}"`;
    })
    .replace(/href="(?:\.\.\/)?rfcs\/([0-9]{4})-([a-z0-9-]+)\.md(#[^"]*)?"/g, (_m, num, _rest, hash) => {
      const known = RFCS.find((r) => r.slug.startsWith(num));
      return known ? `href="/rfcs/${known.slug}/${hash || ""}"` : _m;
    })
    .replace(/href="(?:\.\.\/)?spec\/([a-z0-9-]+)\.schema\.json"/g, 'href="/spec/$1.schema.json"')
    .replace(/href="(?:\.\.\/)?(README|CONTRIBUTING)\.md"/g, (_m, name) =>
      name === "README" ? 'href="/"' : 'href="https://github.com/Vantar-AI/colloq/blob/main/CONTRIBUTING.md"',
    )
    .replace(/href="(?:\.\.\/)?(examples|benchmarks|src|scripts)\/([^"]+)"/g,
      'href="https://github.com/Vantar-AI/colloq/blob/main/$1/$2"');
}

function shell({ title, description, body, nav = "", activeTop = "", canonical, wide = false }) {
  const top = [
    ["/docs/", "Docs", "docs"],
    ["/spec/", "Spec", "spec"],
    ["/rfcs/", "RFCs", "rfcs"],
  ]
    .map(([href, label, key]) =>
      `<a href="${href}"${activeTop === key ? ' class="active"' : ""}>${label}</a>`,
    )
    .join("\n        ");

  return `<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>${esc(title)}</title>
    <meta name="description" content="${esc(description)}" />
    <link rel="canonical" href="${canonical}" />
    <link rel="icon" href="/assets/mark.svg" type="image/svg+xml" />
    <link rel="stylesheet" href="/assets/styles.css" />
  </head>
  <body class="${wide ? "app" : "app app-doc"}">
    <a class="skip" href="#main">Skip to content</a>
    <header class="topbar">
      <a class="brand" href="/">
        <svg class="brand-mark" viewBox="0 0 32 32" aria-hidden="true">
          <circle cx="9" cy="10" r="4.4" />
          <circle cx="23" cy="10" r="4.4" />
          <circle cx="16" cy="23" r="4.4" />
          <path d="M9 10h14M9.9 13.7 15.1 20M22.1 13.7 16.9 20" />
        </svg>
        <span>Colloq</span>
      </a>
      <nav class="topnav" aria-label="Primary">
        ${top}
      </nav>
      <a class="button small outlined" href="https://github.com/Vantar-AI/colloq">GitHub</a>
    </header>
    <div class="layout">
      ${nav}
      <main id="main" class="doc">
${body}
      </main>
    </div>
    <footer class="doc-footer">
      <div class="doc-footer-inner">
        <span>Colloq is open source under Apache 2.0.</span>
        <span class="sep">·</span>
        <a href="https://github.com/Vantar-AI/colloq">Source</a>
        <a href="https://github.com/Vantar-AI/colloq/blob/main/CONTRIBUTING.md">Contribute</a>
        <a href="/spec/">Spec</a>
        <span class="legal">Maintained by Vantar Group LLC</span>
      </div>
    </footer>
  </body>
</html>
`;
}

function sidebar(active, kind) {
  if (kind === "rfcs") {
    const items = RFCS.map(
      (r) => `<a href="/rfcs/${r.slug}/"${active === r.slug ? ' class="active"' : ""}>${r.title}</a>`,
    ).join("\n          ");
    return `<aside class="docs-sidebar">
        <p class="side-head">RFCs</p>
        <nav class="side-nav">
          ${items}
        </nav>
      </aside>`;
  }
  const groups = [];
  for (const doc of DOCS) {
    let group = groups.find((g) => g.name === doc.group);
    if (!group) groups.push((group = { name: doc.group, items: [] }));
    group.items.push(doc);
  }
  const blocks = groups
    .map(
      (g) => `<p class="side-head">${g.name}</p>
        <nav class="side-nav">
          ${g.items
            .map(
              (d) =>
                `<a href="/docs/${d.slug}/"${active === d.slug ? ' class="active"' : ""}>${d.title}</a>`,
            )
            .join("\n          ")}
        </nav>`,
    )
    .join("\n        ");
  return `<aside class="docs-sidebar">
        <p class="side-head">Docs</p>
        <nav class="side-nav">
          <a href="/docs/"${active === "index" ? ' class="active"' : ""}>Overview</a>
        </nav>
        ${blocks}
      </aside>`;
}

function write(relDir, html) {
  const dir = path.join(out, relDir);
  fs.mkdirSync(dir, { recursive: true });
  fs.writeFileSync(path.join(dir, "index.html"), html);
}

function copyDir(from, to, filter = () => true) {
  fs.mkdirSync(to, { recursive: true });
  for (const entry of fs.readdirSync(from, { withFileTypes: true })) {
    const src = path.join(from, entry.name);
    const dst = path.join(to, entry.name);
    if (entry.isDirectory()) copyDir(src, dst, filter);
    else if (filter(entry.name)) fs.copyFileSync(src, dst);
  }
}

// ---------- build ----------

fs.rmSync(out, { recursive: true, force: true });
fs.mkdirSync(out, { recursive: true });

// Static files: landing page, assets, 404.
for (const file of ["index.html", "404.html", "_headers"]) {
  fs.copyFileSync(path.join(here, "src", file), path.join(out, file));
}
copyDir(path.join(here, "assets"), path.join(out, "assets"));

// Schemas, served from colloq.dev.
const specOut = path.join(out, "spec");
fs.mkdirSync(specOut, { recursive: true });
for (const file of fs.readdirSync(path.join(repo, "spec"))) {
  if (file.endsWith(".schema.json")) {
    fs.copyFileSync(path.join(repo, "spec", file), path.join(specOut, file));
  }
}

// Docs pages.
const quickstart = fs.readFileSync(path.join(here, "src", "quickstart.md"), "utf8");
for (const doc of DOCS) {
  const md = doc.file ? fs.readFileSync(path.join(repo, doc.file), "utf8") : quickstart;
  const body = rewriteLinks(marked.parse(md));
  const firstPara = md.replace(/^#.*$/m, "").trim().split("\n\n")[0].replace(/[#*`\[\]]/g, "").slice(0, 180);
  write(
    `docs/${doc.slug}`,
    shell({
      title: `${doc.title} — Colloq docs`,
      description: firstPara,
      canonical: `https://colloq.dev/docs/${doc.slug}/`,
      body: `<article class="prose">\n${body}\n<p class="edit"><a href="https://github.com/Vantar-AI/colloq/blob/main/${doc.file ?? "site/src/quickstart.md"}">Edit this page on GitHub</a></p>\n</article>`,
      nav: sidebar(doc.slug, "docs"),
      activeTop: "docs",
    }),
  );
}

// Docs index.
const groupList = [...new Set(DOCS.map((d) => d.group))]
  .map((g) => {
    const items = DOCS.filter((d) => d.group === g)
      .map(
        (d) =>
          `<li><a href="/docs/${d.slug}/"><strong>${d.title}</strong></a></li>`,
      )
      .join("\n            ");
    return `<section class="index-group">\n          <h2>${g}</h2>\n          <ul class="index-list">\n            ${items}\n          </ul>\n        </section>`;
  })
  .join("\n        ");

write(
  "docs",
  shell({
    title: "Colloq documentation",
    description:
      "How Colloq works: one typed conversation, projected into a checked endpoint per role, executed over four transports.",
    canonical: "https://colloq.dev/docs/",
    body: `<article class="prose">
        <h1>Documentation</h1>
        <p class="lead">Colloq is one typed conversation that compiles into a checked endpoint for every role. These pages explain the model, the compiled plan, the wire, the runtime and the evidence behind the claims.</p>
        ${groupList}
      </article>`,
    nav: sidebar("index", "docs"),
    activeTop: "docs",
  }),
);

// RFC pages.
for (const rfc of RFCS) {
  const md = fs.readFileSync(path.join(repo, rfc.file), "utf8");
  write(
    `rfcs/${rfc.slug}`,
    shell({
      title: `${rfc.title} — Colloq`,
      description: rfc.title,
      canonical: `https://colloq.dev/rfcs/${rfc.slug}/`,
      body: `<article class="prose">\n${rewriteLinks(marked.parse(md))}\n<p class="edit"><a href="https://github.com/Vantar-AI/colloq/blob/main/${rfc.file}">Edit this page on GitHub</a></p>\n</article>`,
      nav: sidebar(rfc.slug, "rfcs"),
      activeTop: "rfcs",
    }),
  );
}

write(
  "rfcs",
  shell({
    title: "Colloq RFCs",
    description: "The decisions behind Colloq, written down before the code.",
    canonical: "https://colloq.dev/rfcs/",
    body: `<article class="prose">
        <h1>RFCs</h1>
        <p class="lead">Every load-bearing decision is written down before it is built, with the alternatives and the reason for the choice.</p>
        <ul class="index-list">
          ${RFCS.map((r) => `<li><a href="/rfcs/${r.slug}/"><strong>${r.title}</strong></a></li>`).join("\n          ")}
        </ul>
      </article>`,
    nav: sidebar("", "rfcs"),
    activeTop: "rfcs",
  }),
);

// Spec index.
const specRows = SCHEMAS.filter((s) => !s[3])
  .map(
    ([file, name, detail]) => `<tr>
            <td><strong>${name}</strong><br /><a class="mono" href="/spec/${file}.schema.json">${file}.schema.json</a></td>
            <td>${detail}</td>
          </tr>`,
  )
  .join("\n          ");

write(
  "spec",
  shell({
    title: "Colloq spec — machine-readable schemas",
    description:
      "JSON Schemas for the Colloq conversation, plan, endpoint, session preface, identity, authorization, evidence and Jev binding formats.",
    canonical: "https://colloq.dev/spec/",
    body: `<article class="prose">
        <h1>Spec</h1>
        <p class="lead">Every Colloq artifact has a machine-readable schema. These URLs are stable identifiers: a file's <code>$id</code> points here, so a validator can resolve it.</p>
        <p>The JSON representation is an interchange and debugging format. It is not the canonical binary encoding, and it is not stable yet. The Rust checker enforces more than the schema does: it recomputes identities, verifies references and rejects a noncanonical transition table.</p>
        <table class="spec-table">
          <thead><tr><th>Schema</th><th>What it describes</th></tr></thead>
          <tbody>
          ${specRows}
          </tbody>
        </table>
        <h2>Validate a file</h2>
        <pre><code>pipx run check-jsonschema --schemafile \\
  https://colloq.dev/spec/colloq-conversation-v0.schema.json \\
  examples/route.colloqconv.json</code></pre>
      </article>`,
    nav: "",
    activeTop: "spec",
    wide: true,
  }),
);

// Sitemap.
const urls = [
  "/",
  "/docs/",
  ...DOCS.map((d) => `/docs/${d.slug}/`),
  "/rfcs/",
  ...RFCS.map((r) => `/rfcs/${r.slug}/`),
  "/spec/",
];
fs.writeFileSync(
  path.join(out, "sitemap.xml"),
  `<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n${urls
    .map((u) => `  <url><loc>https://colloq.dev${u}</loc></url>`)
    .join("\n")}\n</urlset>\n`,
);
fs.writeFileSync(
  path.join(out, "robots.txt"),
  "User-agent: *\nAllow: /\nSitemap: https://colloq.dev/sitemap.xml\n",
);

console.log(`built ${urls.length} pages + ${fs.readdirSync(specOut).length} schemas`);
