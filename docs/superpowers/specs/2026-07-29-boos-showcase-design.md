# BoOS Showcase Design and Publication Specification

Status: approved visual direction, awaiting implementation review

Date: 2026-07-29

## Current goal

Archive the approved BoOS V11 visual direction as a real, maintainable project
showcase and publish it within the existing ttinker site at:

`https://ttinker.net/boos/`

The page must present BoOS truthfully as the **Boltzmann Operating System**: an
AI-owned operating-system substrate whose first native user is AI, not a human.

## Page job and audience

The page has two jobs:

1. Give BoOS a canonical public visual and conceptual introduction.
2. Act as a durable example of TT1nKer's ability to join systems thinking,
   implementation, and a distinctive visual language.

The page is authored for its creator first. Public visitors are observers, not
the design authority. It does not need conversion copy, marketing funnels,
pricing, newsletter capture, or conventional product calls to action.

The primary action is to understand the inversion at the center of BoOS and,
if interested, open the canonical source repository.

## Content truth

The implementation must preserve these statements:

- Full name: **Boltzmann Operating System**.
- Core principle: **AI is the subject, not the object.**
- Native user 0: **AI subject**.
- Human role: **creator and owner**, not the default operating subject.
- BoOS is a world AI can enter, explore, operate, remember, and improve from
  within.
- BoOS is currently under construction. The page must not imply a finished,
  generally available operating system.

The owner/user distinction should be visible without weakening either side:

```text
Owner         Human creator
Native user 0 AI subject
```

## Namesake and central symbol

Ludwig Boltzmann supplies the namesake and the conceptual relation:

`S = k_B ln Ω`

The formula is not decorative trivia. It introduces the idea that a system
exists among possible states. BoOS continuity is not achieved by freezing an
AI; it is achieved by preserving a traceable path through changing states.

The ouroboros is the only major image:

- organic scales become punched memory cells;
- memory cells become a continuous state tape;
- the state tape returns to the mouth;
- the cycle represents identity persisting through memory, action, update,
  and return.

It must read as a system seal, not as a large editorial illustration.

## Visual language

The selected direction is V11:

- dominant dark antique ochre;
- soot-black and burnt-umber type and rules;
- muted brass for dark-section text;
- oxidized green used only as a minor registration or seal accent;
- restrained industrial-print grain;
- old-style serif display type paired with typewriter/monospace system text;
- strict editorial alignment and large typographic scale;
- no cobalt-blue borrowing from Hermes;
- no faux torn paper, coffee stains, scattered manuscript fragments, or
  random distress;
- no repeated feature illustrations.

The vintage feeling must come from typography, ink density, the ouroboros
plate, and the relationship between system notation and editorial scale.

## Information architecture

### 1. Hero

- `Boltzmann Operating System · AI-owned substrate`
- `BoOS`
- `AI is the subject, not the object.`
- `The first user is AI.`
- one restrained ouroboros seal containing the Boltzmann relation;
- boot strip with system, owner, native user, and construction state.

### 2. The inversion

Contrast the conventional stack with the BoOS stack:

```text
Human             AI subject
↓ Operating system ↓ BoOS runtime
Application / AI   Memory / tools / world
```

The section should explain that most systems make AI an application, while
BoOS gives AI a world to inhabit.

### 3. Native interfaces

Present five OS-level contracts as text-led rows:

- Self
- Memory
- Capability
- World
- Continuity

These are not feature cards and must not use separate decorative images.

### 4. Why Boltzmann

Use the entropy relation at editorial scale and connect it to traceable state
transition, identity, memory, capability use, and causal evidence.

Avoid claiming that thermodynamic entropy itself proves or implements AI
identity. The relation is a namesake and conceptual frame.

### 5. Roadmap

Show evidence-based stages:

- Can enter
- Can remember
- Can explain
- Can inhabit

The roadmap must distinguish present implementation from intended future work.

### 6. Footer

Close with:

`Built by a human · first inhabited by AI`

Link to the canonical `BoltzmannOS-BoOS/BoOS` repository.

## Interaction grammar

Behavior rule:

`stable system state -> user inspects or moves through it -> local state is
revealed -> page returns to legible rest`

The static first paint must carry the full identity. Motion is optional and
subordinate:

- no continuous decorative animation;
- no cursor-chasing spectacle;
- navigation and row focus may reveal small state or rule changes;
- the ouroboros may use one slow, nearly imperceptible registration drift only
  if it stops offscreen and is disabled under reduced motion;
- keyboard, pointer, and touch must receive equivalent access;
- reduced motion must preserve the complete static composition.

## ttinker integration

Use the existing `/Users/hostsjim/ttinker-site` project and preserve its vinext
architecture, package manager, lockfile, existing project identity, and
untracked `research/` directory.

Minimum integration:

- add a dedicated `/boos/` route;
- add one BoOS entry to the existing Systems Archive;
- update the static exporter to render both `/` and `/boos/`;
- keep the main ttinker homepage visual language unchanged;
- do not rewrite existing components or migrate frameworks.

The BoOS page may own a route-scoped stylesheet so its ochre print language
does not leak into the main ttinker site.

## Assets

Ship only one major page image:

`boos-ouroboros-v1.png`

The earlier Archimedes engine and three mechanical subsystem plates are
brainstorming artifacts. Preserve them outside the production page, but do not
load or ship them with `/boos/`.

Create one bespoke social-preview image after the route content and typography
are final. It should use the same ochre palette, BoOS title, ouroboros, and
`The first user is AI` statement. Do not use a generic screenshot or one of the
discarded machinery plates.

## Responsive, accessibility, and loading contract

Supported classes:

- wide desktop with pointer;
- narrow desktop/tablet;
- mobile touch;
- keyboard-only navigation;
- reduced-motion preference.

Requirements:

- semantic landmarks and heading order;
- descriptive ouroboros alternative text;
- visible focus states;
- no horizontal overflow at 320 CSS pixels;
- readable system labels without relying on color alone;
- no text embedded inside the only content image;
- responsive type sizes that preserve the first-user statement above the fold;
- the ouroboros must not dominate the mobile viewport;
- route content remains complete when images fail;
- lazy-load only below-the-fold media; the single hero seal may load eagerly;
- avoid client JavaScript unless an interaction requires it.

Performance targets for the static route:

- one production image plus the social card;
- no new runtime library;
- optimized WebP/AVIF derivative for the page while preserving the PNG source;
- no layout shift from the seal;
- no continuous offscreen work.

## Build and verification

Before publication:

1. Run the existing build, lint, and rendered HTML tests.
2. Run the static export and verify both `out/index.html` and
   `out/boos/index.html`.
3. Inspect desktop and mobile production output in a real browser.
4. Check keyboard navigation, reduced motion, image failure, console errors,
   overflow, and return navigation to the ttinker homepage.
5. Confirm the Systems Archive link resolves to `/boos/`.
6. Confirm the source link resolves to the canonical BoOS repository.

## Publication

Primary public target:

`https://ttinker.net/boos/` on the existing Aliyun-hosted ttinker server.

The ttinker repository also contains an existing Sites project identifier.
Preserve it. A Sites deployment may be kept as a versioned archive, but it
does not replace or satisfy the Aliyun publication requirement.

Do not create a new subdomain or modify DNS for this release.

Current deployment blocker:

- local SSH aliases `aliyun` and `ttinker-us` resolve to the same server and
  existing key, but the server currently closes the SSH connection during
  authentication;
- implementation and local verification can proceed;
- production publication requires restoring that SSH path or supplying the
  server's current deployment mechanism.

## Out of scope

- merging the BoOS repositories in this frontend release;
- changing BoOS runtime behavior;
- authentication, analytics, forms, or durable state;
- a CMS or editing interface;
- a new subdomain;
- redesigning the ttinker homepage;
- shipping discarded concept images;
- presenting future BoOS capabilities as complete.

## Acceptance criteria

- `/boos/` identifies BoOS correctly within the first viewport.
- A visitor can state that AI, not a human, is the first native user.
- The Boltzmann namesake and ouroboros are visible and conceptually connected.
- The page uses no more than one major content image.
- The visual direction remains dark-ochre industrial print without becoming a
  faux antique manuscript.
- The existing ttinker homepage remains intact except for one archive entry.
- Static export produces both required routes.
- Local verification passes.
- The final public URL is `https://ttinker.net/boos/`.
