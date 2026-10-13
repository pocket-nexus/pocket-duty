// The dev server the export page is opened on (tools/export.ts starts one per export).
//
// It is the reference's own configuration with two additions:
//
//  - the repository's `maps/` may be read (it lies outside `web/`, the server's root);
//  - with DUTY_MAP set, the reference's world modules get another layout in place of their own:
//    `web/src/world/{ground,dressing,index}.js` import `./layout.js`, and here that name resolves to
//    `maps/<DUTY_MAP>/layout.js` (a map in the reference's terms), or, for a place of ours
//    (`maps/<DUTY_MAP>/place.js`), to a street of no length (`./nolayout.js`). The reference's files
//    stay as they are on disk. The page's own modules name the map's file as `@map/layout` or
//    `@map/place`.
import { mergeConfig } from 'vite';
import { existsSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import reference from '../../vite.config.js';

const here = dirname(fileURLToPath(import.meta.url));
const web = resolve(here, '../..');
const repository = resolve(web, '..');
const map = process.env.DUTY_MAP || '';
if (map && !/^[a-z][a-z0-9-]*$/.test(map)) throw new Error(`DUTY_MAP: "${map}" is not a map's name`);
const placeFile = map ? resolve(repository, 'maps', map, 'place.js') : '';
const place = placeFile && existsSync(placeFile) ? placeFile : resolve(here, 'noplace.js');
const layout = place === placeFile ? resolve(here, 'nolayout.js') : map ? resolve(repository, 'maps', map, 'layout.js') : resolve(web, 'src/world/layout.js');
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
        if (source === '@map/place') return place;
        if (map && source === './layout.js' && importer && importer.replace(/\\/g, '/').split('?')[0].startsWith(world)) return layout;
        return null;
      },
    },
  ],
});
