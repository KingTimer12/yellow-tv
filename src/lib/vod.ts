import { invoke } from "@tauri-apps/api/core";

export type Movie = {
  id: string;
  title: string;
  year: number | null;
  logo: string;
  group: string;
  url: string;
};

export type Series = {
  id: string;
  title: string;
  logo: string;
  group: string;
  seasons: number[];
  episodeCount: number;
};

export type CatalogItem = Movie | Series;
export type CatalogKind = "filmes" | "series";

export type CatalogPage = {
  total: number;
  page: number;
  pageSize: number;
  items: CatalogItem[];
  groups: { name: string; count: number }[];
};

export type Episode = { season: number; episode: number; url: string };

export type SeriesEpisodes = {
  id: string;
  title: string;
  logo: string;
  group: string;
  episodes: Episode[];
};

export type Meta = {
  overview: string;
  tagline: string;
  poster: string | null;
  backdrop: string | null;
  rating: number | null;
  votes: number | null;
  year: number | null;
  runtime: number | null;
  genres: string[];
  cast: { name: string; character: string; photo: string | null }[];
  imdbUrl: string | null;
  source: "tmdb" | "none";
  reason?: string;
};

export type DataStatus = {
  dir: string;
  channels: boolean;
  filmes: boolean;
  series: boolean;
};

export const isSeries = (item: CatalogItem): item is Series => "episodeCount" in item;

export function fetchCatalog(input: {
  kind: CatalogKind;
  query: string;
  group: string;
  page: number;
}) {
  return invoke<CatalogPage>("catalog_page", {
    kind: input.kind,
    query: input.query,
    group: input.group || null,
    page: input.page,
  });
}

export function fetchItem(kind: CatalogKind, id: string) {
  return invoke<{ item: CatalogItem; related: CatalogItem[] }>("catalog_item", { kind, id });
}

export function fetchEpisodes(id: string) {
  return invoke<SeriesEpisodes>("series_episodes", { id });
}

export function fetchMeta(input: { kind: CatalogKind; title: string; year?: number | null }) {
  return invoke<Meta>("title_meta", {
    kind: input.kind,
    title: input.title,
    year: input.year ?? null,
  });
}

export function fetchDataStatus() {
  return invoke<DataStatus>("data_status");
}

/**
 * Streams are plain HTTP and the WebView refuses insecure loads, so playback always
 * goes through the local proxy that Rust starts on 127.0.0.1.
 */
export function streamUrl(url: string) {
  return invoke<string>("stream_url", { url });
}

/** "Series | Netflix" reads better as just "Netflix" once the section says Séries. */
export const shortGroup = (group: string) => group.replace(/^(Series|Filmes)\s*\|\s*/i, "");

export const detailHref = (kind: CatalogKind, id: string) =>
  kind === "series" ? `/serie/${id}` : `/filme/${id}`;
