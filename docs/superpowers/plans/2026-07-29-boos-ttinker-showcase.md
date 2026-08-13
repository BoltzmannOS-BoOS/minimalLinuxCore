# BoOS ttinker Showcase Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a production-ready `/boos/` showcase to ttinker.net, link it from the existing Systems Archive, export both static routes, and publish the verified output to the existing Aliyun-hosted site.

**Architecture:** Preserve the existing vinext application. Add one server-rendered App Router route with route-scoped CSS and one optimized ouroboros asset, extend the existing systems data with an internal BoOS entry, and make the static exporter render a declared route list. Keep all BoOS visual rules inside `app/boos/` so the main ttinker homepage remains unchanged.

**Tech Stack:** Next.js 16 App Router, React 19, vinext, TypeScript, route-scoped CSS, Node test runner, static HTML export, nginx/static Aliyun hosting, existing Sites project archive.

## Global Constraints

- Public route is exactly `https://ttinker.net/boos/`; do not create a subdomain or modify DNS.
- Full name is `Boltzmann Operating System`.
- Core principle is `AI is the subject, not the object.`
- Human role is `creator and owner`; native user 0 is `AI subject`.
- Use one major page image: the ouroboros seal.
- Do not ship the discarded Archimedes engine or subsystem plates.
- Preserve the existing vinext architecture, package manager, lockfile, homepage identity, and untracked `research/` directory.
- Add no runtime dependency and no client component to the BoOS route.
- Preserve complete static content under reduced motion and when the image fails.
- The main page receives one Systems Archive entry and no other redesign.
- Aliyun/ttinker is the primary publication; the existing Sites project is only a versioned archive.

---

## File map

- Create `app/boos/page.tsx`: semantic BoOS route, route metadata, repository link, and all authored copy.
- Create `app/boos/boos.css`: route-scoped ochre industrial-print design, responsive layout, focus states, and reduced-motion behavior.
- Create `design-assets/boos/boos-ouroboros-v1.png`: preserved source image.
- Create `public/boos/ouroboros.webp`: optimized production image used by the page.
- Create `public/boos/og.png`: finished social-preview image for the route.
- Modify `app/SystemsArchive.tsx`: add internal BoOS archive entry and avoid external-link behavior for internal routes.
- Modify `scripts/export-static.mjs`: export `/` and `/boos/` to deterministic output paths.
- Modify `tests/rendered-html.test.mjs`: render arbitrary paths and assert the BoOS route and archive entry.
- Create `tests/static-export.test.mjs`: exercise the exporter and verify both route artifacts.
- Modify `package.json`: run all Node test files after the build.

Dependency direction remains `route/page -> route CSS and public asset`; the
homepage archive knows only the `/boos/` URL. Export and tests consume routes
without route code importing deployment concerns.

---

### Task 1: Establish route-level failing tests

**Files:**
- Modify: `tests/rendered-html.test.mjs`

**Interfaces:**
- Consumes: vinext worker `fetch(Request, Env, ExecutionContext)`
- Produces: `render(pathname = "/"): Promise<Response>` test helper

- [ ] **Step 1: Generalize the renderer without changing existing assertions**

Change the helper signature and request URL:

```js
async function render(pathname = "/") {
  const workerUrl = new URL("../dist/server/index.js", import.meta.url);
  workerUrl.searchParams.set("test", `${process.pid}-${Date.now()}-${pathname}`);
  const { default: worker } = await import(workerUrl.href);

  return worker.fetch(
    new Request(new URL(pathname, "https://ttinker.net"), {
      headers: { accept: "text/html" },
    }),
    {
      ASSETS: {
        fetch: async () => new Response("Not found", { status: 404 }),
      },
    },
    {
      waitUntil() {},
      passThroughOnException() {},
    },
  );
}
```

- [ ] **Step 2: Add a failing BoOS route behavior test**

```js
test("renders the AI-native Boltzmann Operating System showcase", async () => {
  const response = await render("/boos/");
  assert.equal(response.status, 200);
  assert.match(response.headers.get("content-type") ?? "", /^text\/html\b/i);

  const html = await response.text();
  assert.match(html, /<title>BoOS — Boltzmann Operating System<\/title>/i);
  assert.match(html, /Boltzmann Operating System/i);
  assert.match(html, /AI is the subject, not the object/i);
  assert.match(html, /The first user[\s\S]*is[\s\S]*AI/i);
  assert.match(html, /Native user 0/i);
  assert.match(html, /S\s*=\s*k/i);
  assert.match(html, /Can Enter/i);
  assert.match(html, /Can Inhabit/i);
  assert.match(html, /boos-ouroboros/i);
  assert.doesNotMatch(html, /Archimedes|capability governor|memory drum/i);
});
```

- [ ] **Step 3: Add a failing homepage archive assertion**

Inside the existing authored-portfolio test, add:

```js
assert.match(html, /Boltzmann Operating System/);
assert.match(html, /href="\/boos\/"/);
```

- [ ] **Step 4: Run the focused test and verify the intended failure**

Run:

```bash
npm run build
node --test tests/rendered-html.test.mjs
```

Expected: existing homepage checks pass; the BoOS route/archive assertions fail
because `/boos/` and its Systems Archive entry do not exist.

- [ ] **Step 5: Commit the test contract**

```bash
git add tests/rendered-html.test.mjs
git commit -m "test: define BoOS showcase contract"
```

---

### Task 2: Build the static AI-native BoOS route

**Files:**
- Create: `app/boos/page.tsx`
- Create: `app/boos/boos.css`
- Create: `design-assets/boos/boos-ouroboros-v1.png`
- Create: `public/boos/ouroboros.webp`
- Test: `tests/rendered-html.test.mjs`

**Interfaces:**
- Consumes: root layout fonts `--font-geist-sans` and `--font-geist-mono`
- Produces: static GET `/boos/`; public asset `/boos/ouroboros.webp`

- [ ] **Step 1: Preserve and optimize the approved ouroboros**

Copy the approved PNG into `design-assets/boos/boos-ouroboros-v1.png`.
Create a 960-pixel WebP with quality 84 at
`public/boos/ouroboros.webp`. Validate both files:

```bash
file design-assets/boos/boos-ouroboros-v1.png public/boos/ouroboros.webp
```

Expected: one PNG source and one WebP production derivative, both square.

- [ ] **Step 2: Create route metadata and semantic page structure**

`app/boos/page.tsx` must export:

```tsx
import type { Metadata } from "next";
import "./boos.css";

export const metadata: Metadata = {
  title: "BoOS — Boltzmann Operating System",
  description:
    "An AI-owned operating-system substrate whose first native user is AI.",
  alternates: { canonical: "/boos/" },
  openGraph: {
    title: "BoOS — Boltzmann Operating System",
    description: "AI is the subject, not the object.",
    url: "/boos/",
    type: "website",
  },
  twitter: {
    card: "summary_large_image",
    title: "BoOS — Boltzmann Operating System",
    description: "The first native user is AI.",
  },
};
```

The default export must use this landmark order:

```tsx
<main className="boos-page">
  <header className="boos-hero" id="boos-top">{/* identity + seal + boot strip */}</header>
  <section className="boos-inversion">{/* conventional vs BoOS stack */}</section>
  <section className="boos-native" id="boos-native">{/* five interfaces */}</section>
  <section className="boos-boltzmann">{/* formula + namesake explanation */}</section>
  <section className="boos-roadmap" id="boos-roadmap">{/* four evidence stages */}</section>
  <footer className="boos-footer">{/* source + return to ttinker */}</footer>
</main>
```

Use exactly one content image:

```tsx
<img
  className="boos-ouroboros-image"
  src="/boos/ouroboros.webp"
  width="960"
  height="960"
  alt="An ouroboros whose scales become punched memory cells and a continuous state tape"
/>
```

Do not use a client directive or add runtime state.

- [ ] **Step 3: Implement the approved copy**

The first viewport must contain:

```text
Boltzmann Operating System · AI-owned substrate
BoOS
AI is the subject, not the object.
The first user is AI.
System / BoOS 0.1
Owner / Human creator
Native user 0 / AI subject
State / Bootstrapping
```

The five interface rows are `Self`, `Memory`, `Capability`, `World`, and
`Continuity`. The Boltzmann section must include `S = k_B ln Ω` and explicitly
describe the formula as a namesake/conceptual frame, not an implementation
claim. Roadmap stages are `Can Enter`, `Can Remember`, `Can Explain`, and
`Can Inhabit`.

- [ ] **Step 4: Implement route-scoped V11 styling**

Every selector in `app/boos/boos.css` must begin with `.boos-` or be nested
under `.boos-page`. Define these route tokens on `.boos-page`:

```css
.boos-page {
  --boos-ochre: #ad7b2c;
  --boos-ochre-light: #c39443;
  --boos-ink: #201912;
  --boos-soot: #17130f;
  --boos-brass: #ddbc70;
  --boos-green: #526d59;
  color: var(--boos-ink);
  background: var(--boos-ochre);
  font-family: "Iowan Old Style", Baskerville, "Times New Roman", serif;
}
```

Required composition:

- hero uses a `minmax(0, 1.42fr) minmax(260px, .58fr)` grid;
- seal width is `min(27vw, 390px)` on wide screens and never exceeds 55% of
  the mobile viewport;
- boot strip is a four-cell dark system row and collapses to two visible cells
  at 540 pixels;
- all later sections are typography-led and contain no background images;
- all body/system copy uses `var(--font-geist-mono), "Courier New", monospace`;
- visible focus uses a two-pixel brass or soot outline;
- at 880 pixels, two-column sections collapse to one;
- at 540 pixels, roadmap stages become one column;
- `@media (prefers-reduced-motion: reduce)` removes transitions and animation;
- no continuous animation is required for the first release.

- [ ] **Step 5: Build and run the focused route test**

Run:

```bash
npm run build
node --test tests/rendered-html.test.mjs
```

Expected: route-specific assertions pass; the homepage archive assertion is
the only remaining failure.

- [ ] **Step 6: Commit the isolated route**

```bash
git add app/boos design-assets/boos public/boos/ouroboros.webp
git commit -m "feat: add AI-native BoOS showcase"
```

---

### Task 3: Add the BoOS Systems Archive entry

**Files:**
- Modify: `app/SystemsArchive.tsx`
- Test: `tests/rendered-html.test.mjs`

**Interfaces:**
- Consumes: internal route `/boos/`; existing `Signal` value `"orbit"`
- Produces: Systems Archive entry `008`; `isExternalHref(href): boolean`

- [ ] **Step 1: Add a small internal/external URL predicate**

Add above `SystemsArchive`:

```ts
function isExternalHref(href: string): boolean {
  return href.startsWith("https://") || href.startsWith("http://");
}
```

- [ ] **Step 2: Add the BoOS record without changing ProjectVisual**

Append:

```ts
{
  index: "008",
  name: "BoOS",
  kind: "AI-NATIVE OPERATING SYSTEM",
  state: "BOOTSTRAPPING",
  description:
    "A Boltzmann-named operating-system substrate whose first native user is AI.",
  stack: "RUST / LINUX / IDENTITY / MEMORY",
  href: "/boos/",
  signal: "orbit",
  accent: "#ad7b2c",
},
```

Reusing the existing orbit signal avoids modifying the large canvas renderer;
the dedicated route owns the ouroboros identity.

- [ ] **Step 3: Render internal links without a new tab**

For both list and inspector links, derive:

```tsx
const external = isExternalHref(item.href);
```

and pass:

```tsx
target={external ? "_blank" : undefined}
rel={external ? "noreferrer" : undefined}
```

Use the same logic for the current `system.href` inspector link. Change the
accessible label from `Open ... repository` to `Open ${item.name}` so the
internal showcase is described truthfully.

- [ ] **Step 4: Build and run the authored-site contract**

Run:

```bash
npm run build
node --test tests/rendered-html.test.mjs
```

Expected: all rendered HTML tests pass.

- [ ] **Step 5: Commit the homepage integration**

```bash
git add app/SystemsArchive.tsx tests/rendered-html.test.mjs
git commit -m "feat: archive BoOS on ttinker"
```

---

### Task 4: Export and verify both static routes

**Files:**
- Modify: `scripts/export-static.mjs`
- Modify: `package.json`
- Create: `tests/static-export.test.mjs`

**Interfaces:**
- Consumes: built vinext worker and `dist/client/`
- Produces: `out/index.html`; `out/boos/index.html`

- [ ] **Step 1: Write the failing static-export test**

Create:

```js
import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { readFile } from "node:fs/promises";
import { promisify } from "node:util";
import test from "node:test";

const execFileAsync = promisify(execFile);

test("exports the homepage and BoOS route", async () => {
  await execFileAsync(process.execPath, ["scripts/export-static.mjs"], {
    cwd: new URL("..", import.meta.url),
  });

  const [home, boos] = await Promise.all([
    readFile(new URL("../out/index.html", import.meta.url), "utf8"),
    readFile(new URL("../out/boos/index.html", import.meta.url), "utf8"),
  ]);

  assert.match(home, /TT1NKER\./);
  assert.match(home, /href="\/boos\/"/);
  assert.match(boos, /Boltzmann Operating System/i);
  assert.match(boos, /The first user/i);
  assert.match(boos, /Native user 0/i);
});
```

- [ ] **Step 2: Run it and verify `out/boos/index.html` is missing**

Run:

```bash
node --test tests/static-export.test.mjs
```

Expected: FAIL reading `out/boos/index.html`.

- [ ] **Step 3: Refactor the exporter around an explicit route table**

Use:

```js
const routes = [
  { pathname: "/", output: new URL("../out/index.html", import.meta.url) },
  { pathname: "/boos/", output: new URL("../out/boos/index.html", import.meta.url) },
];

async function renderRoute(pathname) {
  const response = await worker.fetch(
    new Request(new URL(pathname, "https://ttinker.net"), {
      headers: { accept: "text/html" },
    }),
    { ASSETS: { fetch: async () => new Response("Not found", { status: 404 }) } },
    { waitUntil() {}, passThroughOnException() {} },
  );
  if (!response.ok) {
    throw new Error(`Static render failed for ${pathname} with ${response.status}`);
  }
  return response.text();
}
```

After clearing `out/`, copy `dist/client/` once. For each route, create the
parent directory with `{ recursive: true }` and write the rendered HTML.

- [ ] **Step 4: Make the package test script include every Node test**

Change:

```json
"test": "npm run build && node --test tests/*.test.mjs"
```

- [ ] **Step 5: Run the complete local suite**

Run:

```bash
npm test
npm run lint
npm run export
```

Expected: all commands exit zero; both HTML files exist.

- [ ] **Step 6: Commit the deterministic export**

```bash
git add scripts/export-static.mjs tests/static-export.test.mjs package.json
git commit -m "build: export ttinker and BoOS routes"
```

---

### Task 5: Add and validate the route social preview

**Files:**
- Create: `public/boos/og.png`
- Modify: `app/boos/page.tsx`
- Test: `tests/rendered-html.test.mjs`

**Interfaces:**
- Consumes: final V11 palette, ouroboros, route title, and first-user statement
- Produces: absolute metadata URL `https://ttinker.net/boos/og.png`

- [ ] **Step 1: Generate exactly one finished social card**

Use built-in image generation with this content contract:

```text
Landscape 1200×630 social card for BoOS — Boltzmann Operating System.
Dark antique ochre field, soot-black old-style serif title "BoOS", small
typewriter line "BOLTZMANN OPERATING SYSTEM", engraved ouroboros seal, and
the exact statement "THE FIRST USER IS AI". Restrained industrial-print grain,
high contrast, no blue, no machinery collage, no additional words, no watermark.
```

Inspect spelling, the exact first-user statement, contrast, and crop safety.
Retry once only if text is incorrect or the card is unusable.

- [ ] **Step 2: Save the accepted card and wire absolute metadata**

Save the accepted image to `public/boos/og.png`. Add:

```tsx
openGraph: {
  title: "BoOS — Boltzmann Operating System",
  description: "AI is the subject, not the object.",
  url: "/boos/",
  type: "website",
  images: [{
    url: "/boos/og.png",
    width: 1200,
    height: 630,
    alt: "BoOS — The first user is AI",
  }],
},
twitter: {
  card: "summary_large_image",
  title: "BoOS — Boltzmann Operating System",
  description: "The first native user is AI.",
  images: ["/boos/og.png"],
},
```

The root layout's `metadataBase` resolves these to the incoming production
origin `https://ttinker.net`.

- [ ] **Step 3: Extend the route test**

Add:

```js
assert.match(html, /https:\/\/ttinker\.net\/boos\/og\.png/i);
assert.match(html, /property="og:image"/i);
assert.match(html, /name="twitter:image"/i);
```

- [ ] **Step 4: Rebuild and validate metadata**

Run:

```bash
npm test
```

Expected: all tests pass and rendered BoOS HTML contains absolute Open Graph
and X image URLs.

- [ ] **Step 5: Commit the social surface**

```bash
git add public/boos/og.png app/boos/page.tsx tests/rendered-html.test.mjs
git commit -m "feat: add BoOS social preview"
```

---

### Task 6: Browser verification and release candidate

**Files:**
- Modify only if verification exposes a defect in Task 2-5 files.

**Interfaces:**
- Consumes: production build and static `out/`
- Produces: verified release candidate commit

- [ ] **Step 1: Read the frontend validation references**

Read:

```text
build-verified-frontends/references/browser-validation.md
build-verified-frontends/references/performance-and-accessibility.md
```

- [ ] **Step 2: Start the production-equivalent static preview**

Serve `out/` on an unused localhost port with a retained process. Open
`/boos/` once.

- [ ] **Step 3: Verify representative viewports and inputs**

Check:

- 1440×1000 desktop;
- 390×844 mobile;
- keyboard-only navigation and visible focus;
- reduced motion;
- image disabled or failed;
- return navigation to `/`;
- no horizontal overflow at 320 CSS pixels;
- no console errors;
- no below-the-fold image requests other than the social card, which should not
  be loaded as page content.

Capture one desktop and one mobile screenshot as release evidence outside the
source tree.

- [ ] **Step 4: Run the final local release gate**

Run:

```bash
npm test
npm run lint
npm run export
git diff --check
git status --short
```

Expected: zero failures; only intentional tracked changes; user-owned
`research/` remains untouched and untracked in the original main worktree.

- [ ] **Step 5: Commit any verification fixes**

If verification required changes:

```bash
git add app/boos app/SystemsArchive.tsx scripts/export-static.mjs tests public/boos
git commit -m "fix: harden BoOS showcase release"
```

If no source changed, do not create an empty commit.

---

### Task 7: Publish the verified source and static output

**Files:**
- Preserve: `.openai/hosting.json`
- Deploy: exact release candidate source and `out/`

**Interfaces:**
- Consumes: verified branch-head commit; existing Sites project ID; Aliyun ttinker web root
- Produces: Sites archive version; public `https://ttinker.net/boos/`

- [ ] **Step 1: Merge the verified feature branch into ttinker main**

Use a non-destructive fast-forward or normal merge. Do not stage, move, or
delete `research/`.

- [ ] **Step 2: Publish a versioned Sites archive**

Use the existing project identifier in `.openai/hosting.json`, the validated
build, and the Sites hosting sequence. Preserve the returned deployment URL as
an archive; do not present it as the requested ttinker launch.

- [ ] **Step 3: Re-establish the Aliyun publication path**

Probe the existing `aliyun` alias once. If authentication still closes:

- stop before mutating the server;
- report that the release candidate is ready;
- request the current SSH/deployment path from the user.

If access succeeds, inspect the active `ttinker.net` server block and resolve
its exact document root before copying.

- [ ] **Step 4: Deploy recoverably**

On Aliyun:

1. create a timestamped sibling backup of the existing ttinker document root;
2. upload the complete `out/` tree to a timestamped staging directory;
3. verify `index.html`, `boos/index.html`, and referenced assets in staging;
4. switch the web root or rename directories atomically;
5. run `nginx -t` only if nginx configuration changed (no configuration change
   is expected);
6. request `https://ttinker.net/` and `https://ttinker.net/boos/`;
7. restore the prior directory if either route fails.

Do not delete the backup during this release.

- [ ] **Step 5: Verify public behavior**

Check:

```text
https://ttinker.net/
https://ttinker.net/boos/
https://ttinker.net/boos/ouroboros.webp
https://ttinker.net/boos/og.png
```

Confirm status 200, HTML content type for routes, image content types for
assets, canonical metadata, and working homepage-to-BoOS navigation.

- [ ] **Step 6: Record the release**

Report:

- ttinker public URL;
- Sites archive URL;
- release commit;
- local build/test/lint/export results;
- whether the Aliyun backup remains available;
- any server area not verified.
