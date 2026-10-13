// Builds the TDesign icon sprite from the locally installed
// tdesign-icons-vue-next package, so <t-icon> works without reaching
// tdesign.gtimg.com (intranet / offline deployments).
import { readdirSync, readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const PROP_VALUES = {
  'props.strokeColor1': 'currentColor',
  'props.strokeColor2': 'currentColor',
  'props.fillColor1': 'transparent',
  'props.fillColor2': 'transparent',
  'props.filledColor': 'currentColor',
  'props.strokeWidth': '2',
};

function kebab(name) {
  return name.replace(/[A-Z]/g, (c) => `-${c.toLowerCase()}`);
}

function escapeAttr(value) {
  return String(value).replace(/&/g, '&amp;').replace(/"/g, '&quot;').replace(/</g, '&lt;');
}

function renderNode(node) {
  const attrs = Object.entries(node.attrs ?? {})
    // Ids and clip paths point at defs the package doesn't ship.
    .filter(([key]) => key !== 'id' && key !== 'clipPath')
    .map(([key, value]) => `${kebab(key)}="${escapeAttr(PROP_VALUES[value] ?? value)}"`)
    .join(' ');
  const children = (node.children ?? []).map(renderNode).join('');
  return `<${node.tag}${attrs ? ` ${attrs}` : ''}>${children}</${node.tag}>`;
}

/** The icon package that tdesign-vue-next's <t-icon> renders with. */
export function resolveIconPackageDir(fromDir) {
  const require = createRequire(join(fromDir, 'package.json'));
  const tdesignDir = dirname(require.resolve('tdesign-vue-next/package.json'));
  const fromTdesign = createRequire(join(tdesignDir, 'package.json'));
  return dirname(fromTdesign.resolve('tdesign-icons-vue-next/package.json'));
}

/** Returns a script that injects the sprite into the page, like the CDN file does. */
export function buildSpriteScript(iconPackageDir) {
  const componentsDir = join(iconPackageDir, 'esm', 'components');
  const symbols = [];
  for (const file of readdirSync(componentsDir).sort()) {
    if (!file.endsWith('.js')) continue;
    const source = readFileSync(join(componentsDir, file), 'utf8');
    const match = source.match(/var element = (\{[\s\S]*?\n\});/);
    if (!match) continue;
    const element = JSON.parse(match[1]);
    const name = file.slice(0, -'.js'.length);
    const viewBox = element.attrs?.viewBox ?? '0 0 24 24';
    const body = (element.children ?? []).map(renderNode).join('');
    symbols.push(`<symbol id="t-icon-${name}" viewBox="${viewBox}" fill="none">${body}</symbol>`);
  }
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" style="position:absolute;width:0;height:0;overflow:hidden" aria-hidden="true">${symbols.join(
    ''
  )}</svg>`;
  return `(function(){var d=document;function inject(){var w=d.createElement('div');w.innerHTML=${JSON.stringify(
    svg
  )};d.body.insertBefore(w.firstChild,d.body.firstChild);}if(d.body)inject();else d.addEventListener('DOMContentLoaded',inject);})();\n`;
}

const CDN_SPRITE_URL = /https:\/\/tdesign\.gtimg\.com\/icon\/[\d.]+\/fonts\/index\.js/;
const STUDIO_DIR = fileURLToPath(new URL('..', import.meta.url));
const SPRITE_FILE = 'tdesign-icons.js';

/**
 * Vite plugin: serves the sprite from the app itself and points <t-icon> at
 * it instead of the TDesign CDN.
 */
export function localTdesignIcons() {
  let base = '/';
  let sprite = null;
  const getSprite = () => (sprite ??= buildSpriteScript(resolveIconPackageDir(STUDIO_DIR)));
  const rewrite = (code) =>
    code.replace(new RegExp(CDN_SPRITE_URL.source, 'g'), `${base}${SPRITE_FILE}`);
  const isIconModule = (id) => id.includes('tdesign-icons-vue-next') && /\.m?js$/.test(id);

  return {
    name: 'local-tdesign-icons',
    config(userConfig) {
      base = userConfig.base ?? '/';
      // Dev pre-bundles tdesign with esbuild, which skips Vite transforms.
      return {
        optimizeDeps: {
          esbuildOptions: {
            plugins: [
              {
                name: 'local-tdesign-icons',
                setup(build) {
                  build.onLoad({ filter: /tdesign-icons-vue-next.*\.m?js$/ }, (args) => {
                    const code = readFileSync(args.path, 'utf8');
                    if (!CDN_SPRITE_URL.test(code)) return undefined;
                    return { contents: rewrite(code), loader: 'js' };
                  });
                },
              },
            ],
          },
        },
      };
    },
    transform(code, id) {
      if (!isIconModule(id) || !CDN_SPRITE_URL.test(code)) return null;
      return { code: rewrite(code), map: null };
    },
    configureServer(server) {
      server.middlewares.use(`${base}${SPRITE_FILE}`, (_req, res) => {
        res.setHeader('Content-Type', 'application/javascript');
        res.end(getSprite());
      });
    },
    generateBundle() {
      this.emitFile({ type: 'asset', fileName: SPRITE_FILE, source: getSprite() });
    },
  };
}
