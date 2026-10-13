// The dev server the export page is opened on (tools/export.ts starts one per export).
//
// It is the reference's own configuration with two additions:
//
//  - the repository's `maps/` may be read (it lies outside `web/`, the server's root);
//  - with DUTY_MAP set, the reference's world modules get that map's layout in place of their own:
//    `web/src/world/{ground,dressing,index}.js` import `./layout.js`, and here that name resolves to
//    `maps/<DUTY_MAP>/layout.js`. The reference's files stay as they are on disk; its kit builds
//    another plan. The page's own modules name the same file as `@map/layout`.
import { mergeConfig } from 'vite';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import reference from '../../vite.config.js';

const here = dirname(fileURLToPath(import.meta.url));
const web = resolve(here, '../..');
const repository = resolve(web, '..');
const map = process.env.DUTY_MAP || '';
if (map && !/^[a-z][a-z0-9-]*$/.test(map)) throw new Error(`DUTY_MAP: "${map}" is not a map's name`);
const layout = map ? resolve(repository, 'maps', map, 'layout.js') : resolve(web, 'src/world/layout.js');
const world = resolve(web, 'src/world').replace(/\\/g, '/') + '/';

export default mergeConfig(reference, {
  root: web,
  server: { fs: { allow: [repository] } },
  plugins: [
    {
      name: 'pocket-duty-map',
      enforce: 'pre',
      resolveId(source, importer) {
        if (source === '@map/layout') return layout;
        if (map && source === './layout.js' && importer && importer.replace(/\\/g, '/').split('?')[0].startsWith(world)) return layout;
        return null;
      },
    },
  ],
});
