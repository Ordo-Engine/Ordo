# Documentation Deployment Guide

Ordo documentation supports dual deployment:

1. **GitHub Pages**: `https://pama-lee.github.io/Ordo/docs/`
2. **Custom Domain**: `https://docs.ordoengine.com/`

## How It Works

The VitePress configuration (`config.mts`) uses environment variables to determine the base path:

```typescript
const isCustomDomain = process.env.CUSTOM_DOMAIN === 'true';
const BASE_PATH = process.env.DOCS_BASE_PATH || (isCustomDomain ? '/' : '/Ordo/docs/');
```

## Build Commands

### For GitHub Pages (default)

```bash
cd ordo-editor/apps/docs
pnpm build
# Output: .vitepress/dist/ with base path /Ordo/docs/
```

### For Custom Domain

```bash
cd ordo-editor/apps/docs
CUSTOM_DOMAIN=true pnpm build
# Output: .vitepress/dist/ with base path /
```

### Custom Base Path

```bash
cd ordo-editor/apps/docs
DOCS_BASE_PATH=/my-custom-path/ pnpm build
```

## Deployment Options

### Option 1: Vercel

1. Import the repository to Vercel
2. Set the following:
   - **Root Directory**: `ordo-editor/apps/docs`
   - **Build Command**: `pnpm build`
   - **Output Directory**: `.vitepress/dist`
   - **Environment Variable**: `CUSTOM_DOMAIN=true`
3. Add custom domain `docs.ordoengine.com`

### Option 2: Netlify

1. Create a new site from Git
2. Set the following:
   - **Base directory**: `ordo-editor/apps/docs`
   - **Build command**: `CUSTOM_DOMAIN=true pnpm build`
   - **Publish directory**: `ordo-editor/apps/docs/.vitepress/dist`
3. Add custom domain in Domain settings

### Option 3: Cloudflare Pages

1. Connect your repository
2. Set the following:
   - **Root directory**: `ordo-editor/apps/docs`
   - **Build command**: `CUSTOM_DOMAIN=true pnpm build`
   - **Build output directory**: `.vitepress/dist`
   - **Environment variable**: `CUSTOM_DOMAIN=true`
3. Add custom domain in Custom domains

### Option 4: GitHub Actions (Manual)

Use the `deploy-docs-custom.yml` workflow:

1. Go to Actions → "Deploy Docs to Custom Domain"
2. Click "Run workflow"
3. Download the artifact and upload to your hosting provider

## DNS Configuration

For `docs.ordoengine.com`, add a CNAME record:

| Type  | Name | Value                        |
| ----- | ---- | ---------------------------- |
| CNAME | docs | your-hosting-provider-domain |

Example for Vercel:

```
CNAME docs cname.vercel-dns.com
```

## Local Development

```bash
cd ordo-editor/apps/docs
pnpm dev
# Visit http://localhost:5173/Ordo/docs/

# Or with custom domain simulation:
CUSTOM_DOMAIN=true pnpm dev
# Visit http://localhost:5173/
```
