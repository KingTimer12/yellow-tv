import { createResource, createSignal, onMount } from "solid-js";
import { fetchCatalog, type CatalogItem } from "~/lib/api";

/**
 * Canal já chega classificado do Rust: `group` vem do `group-title` da lista e o
 * número, do `tvg-chno`. O front só precisa de um índice de busca.
 */
export type Channel = CatalogItem & { search: string };

function normalize(value: string) {
  return value
    .toLowerCase()
    .normalize("NFD")
    .replace(/[̀-ͯ]/g, "");
}

async function load(): Promise<Channel[]> {
  // O backend já usa uma página gigante para canais (CHANNEL_PAGE_SIZE), então
  // uma única chamada cobre qualquer lista real sem repetir o agregado por
  // `group_name`. Se algum dia uma lista extrapolar isso, pagina sequencialmente
  // (nunca em paralelo — cada página roda um GROUP BY completo por trás do mutex).
  const first = await fetchCatalog({ kind: "canais", query: "", group: "", page: 0 });
  const items = [first.items];
  let page = first;
  while ((page.page + 1) * page.pageSize < page.total) {
    page = await fetchCatalog({ kind: "canais", query: "", group: "", page: page.page + 1 });
    items.push(page.items);
  }
  return items
    .flat()
    .sort((a, b) => (a.channelNumber ?? 9999) - (b.channelNumber ?? 9999))
    .map(item => ({ ...item, search: normalize(item.title) }));
}

export function useChannels() {
  const [started, setStarted] = createSignal(false);
  onMount(() => setStarted(true));
  const [data] = createResource(() => (started() ? "channels" : undefined), load);

  return {
    channels: () => data() ?? [],
    pending: () => !data() && !data.error,
    error: () => data.error as Error | undefined,
  };
}

export function matches(channel: Channel, terms: string[]) {
  return terms.every(term => channel.search.includes(term));
}

export function parseQuery(query: string) {
  return normalize(query).split(/\s+/).filter(Boolean);
}

export const formatNumber = (n: number) => String(n).padStart(3, "0");
