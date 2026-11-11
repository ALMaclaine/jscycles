#!/usr/bin/env node
// Fails when a built page links to a page or heading anchor that does not
// exist. External links are not fetched. Run after `astro build`.
import { readFileSync, readdirSync, existsSync } from 'node:fs';
import { join, relative, extname, sep } from 'node:path';
import { fileURLToPath } from 'node:url';

const dist = fileURLToPath(new URL('../dist/', import.meta.url));
const base = '/jscycles/';

function htmlFiles(directory) {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) return htmlFiles(path);
    return entry.name.endsWith('.html') ? [path] : [];
  });
}

const decode = (text) =>
  text.replace(/&amp;/g, '&').replace(/&quot;/g, '"').replace(/&#39;/g, "'").replace(/&lt;/g, '<').replace(/&gt;/g, '>');

const pages = htmlFiles(dist);
const ids = new Map(pages.map((page) => [page, new Set([...readFileSync(page, 'utf8').matchAll(/\sid="([^"]+)"/g)].map((m) => m[1]))]));
const problems = [];
for (const page of pages) {
  const directory = relative(dist, page).split(sep).slice(0, -1).join('/');
  const pageUrl = new URL(`${base}${directory ? `${directory}/` : ''}`, 'https://site.invalid');
  for (const [, raw] of readFileSync(page, 'utf8').matchAll(/href="([^"]+)"/g)) {
    const href = decode(raw);
    if (/^(https?:|mailto:|data:)/.test(href)) continue;
    const url = new URL(href, pageUrl);
    if (url.origin !== pageUrl.origin) continue;
    if (!url.pathname.startsWith(base)) {
      problems.push(`${relative(dist, page)}: ${href} is outside ${base}`);
      continue;
    }
    const path = decodeURIComponent(url.pathname.slice(base.length));
    const target = extname(path) ? join(dist, path) : join(dist, path, 'index.html');
    if (!existsSync(target)) {
      problems.push(`${relative(dist, page)}: ${href} does not exist`);
    } else if (url.hash && target.endsWith('.html') && !ids.get(target)?.has(decodeURIComponent(url.hash.slice(1)))) {
      problems.push(`${relative(dist, page)}: ${href} names a missing anchor`);
    }
  }
}
for (const problem of problems) console.error(problem);
console.log(`check-links: ${pages.length} pages, ${problems.length} broken links`);
process.exit(problems.length ? 1 : 0);
