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
  // Uma página grande basta: nenhuma lista real passa de alguns milhares de canais.
  const first = await fetchCatalog({ kind: "canais", query: "", group: "", page: 0 });
  const pages = Math.ceil(first.total / first.pageSize);
  const rest = await Promise.all(
    Array.from({ length: Math.max(0, pages - 1) }, (_, index) =>
      fetchCatalog({ kind: "canais", query: "", group: "", page: index + 1 }),
    ),
  );
  return [first, ...rest]
    .flatMap(page => page.items)
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
