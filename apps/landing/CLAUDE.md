# apps/landing — Diraigent Landing Page

## Overview

Static landing page for Diraigent. Plain HTML/CSS, served by nginx in a container.

## Stack

- **HTML5 + CSS3** — no build step, no JavaScript framework
- **nginx:alpine** — static file server
- **Containerfile** — OCI image for deployment

## Structure

```
apps/landing/
  CLAUDE.md             — this file
  Containerfile         — OCI container build (nginx:alpine)
  nginx.conf            — nginx site config
  public/               — document root (copied into container)
    index.html          — landing page
    assets/
      css/              — stylesheets
      images/           — logos, icons
```

## Conventions

- Semantic HTML, minimal CSS, no JS unless strictly necessary
- Mobile-first responsive design
- Catppuccin Mocha color palette
- Keep pages fast — no external dependencies, no CDN imports
- All assets served locally

## Build & Run

```bash
podman build -t diraigent-landing -f Containerfile .
podman run --rm -p 8090:80 diraigent-landing
```
