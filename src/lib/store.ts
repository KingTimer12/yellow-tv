import { createSignal } from "solid-js";
import { isServer } from "solid-js/web";

function persisted(key: string, limit = Infinity) {
  const read = (): string[] => {
    if (isServer) return [];
    try {
      const stored = JSON.parse(localStorage.getItem(key) ?? "[]");
      return Array.isArray(stored) ? stored.filter(id => typeof id === "string") : [];
    } catch {
      return [];
    }
  };

  // Starts empty so hydration matches the server render; hydrate() fills it after mount.
  const [ids, setIds] = createSignal<string[]>([]);

  /**
   * Takes an updater rather than a value: the setter's previous argument is not a
   * reactive read, so writing from inside an effect can't re-trigger that effect.
   */
  const write = (update: (previous: string[]) => string[]) =>
    setIds(previous => {
      const next = update(previous).slice(0, limit);
      if (!isServer) {
        try {
          localStorage.setItem(key, JSON.stringify(next));
        } catch {
          // storage full or blocked — the list still works for this session
        }
      }
      return next;
    });

  return { ids, write, hydrate: () => setIds(read()) };
}

const favorites = persisted("yellowtv:favorites");
const recents = persisted("yellowtv:recents", 20);

export const favoriteIds = favorites.ids;
export const isFavorite = (id: string) => favoriteIds().includes(id);
export const toggleFavorite = (id: string) =>
  favorites.write(previous =>
    previous.includes(id) ? previous.filter(other => other !== id) : [id, ...previous],
  );

export const recentIds = recents.ids;
export const markWatched = (id: string) =>
  recents.write(previous => [id, ...previous.filter(other => other !== id)]);

/** localStorage is unavailable during SSR, so read it once the client is live. */
export function hydrateStores() {
  favorites.hydrate();
  recents.hydrate();
}
