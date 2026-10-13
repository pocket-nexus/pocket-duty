/* PocketJS's guest driver (vendor/pocketjs/hosts/3ds/src/qjs.c), compiled here as it stands, and one
 * call it does not have: a run of QuickJS's cycle collector.
 *
 * The interface is a Solid app: a screen that is taken down leaves its owners and their computations
 * pointing at each other, which reference counts never free. QuickJS collects them when its heap has
 * grown by half since its last collection, megabytes away for a heap of this size. A change of map
 * takes the play screen down and puts the loading screen up six times, about 59 KB of such cycles a
 * change, on the C heap the map's own sections need. The host collects once a change, while the
 * frame waits for the pack anyway. */
#include "qjs.c" /* (found on the include path: PocketJS's hosts/3ds/src) */

void guest_qjs_collect(void) {
  if (runtime)
    JS_RunGC(runtime);
}
