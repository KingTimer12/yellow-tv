import { invoke } from "@tauri-apps/api/core";

export type CatalogKind = "filmes" | "series" | "canais";
export type OwnerKind = "item" | "episode";

export type Source = {
  id: number;
  url: string;
  label: string;
  kind: "url" | "file";
  enabled: boolean;
  addedAt: number;
  lastSyncAt: number | null;
  itemCount: number;
};

export type ImportReport = {
  source: Source;
  parsed: number;
  discarded: number;
  movies: number;
  series: number;
  channels: number;
};

export type CatalogItem = {
  id: string;
  kind: "movie" | "series" | "channel";
  title: string;
  year: number | null;
  logo: string | null;
  group: string | null;
  seasons: number;
  episodeCount: number;
  channelNumber: number | null;
  /** 0 a 1 */
  percent: number;
  completed: boolean;
};

export type GroupCount = { name: string; count: number };

export type CatalogPage = {
  total: number;
  page: number;
  pageSize: number;
  items: CatalogItem[];
  groups: GroupCount[];
};

export type StreamRef = {
  id: number;
  url: string;
  quality: string | null;
  sourceLabel: string;
  channelNumber: number | null;
};

export type Progress = {
  ownerId: string;
  ownerKind: OwnerKind;
  positionSecs: number;
  durationSecs: number | null;
  completed: boolean;
  updatedAt: number;
};

export type ItemWithRelated = {
  item: CatalogItem;
  streams: StreamRef[];
  progress: Progress | null;
  related: CatalogItem[];
};

export type EpisodeRow = {
  id: string;
  season: number;
  episode: number;
  title: string | null;
  streams: StreamRef[];
  positionSecs: number;
  percent: number;
  completed: boolean;
};

export type SeriesEpisodes = {
  id: string;
  title: string;
  logo: string | null;
  group: string | null;
  episodes: EpisodeRow[];
};

export type EpisodeRef = {
  id: string;
  seriesId: string;
  season: number;
  episode: number;
  title: string | null;
};

export type ContinueEntry = {
  ownerId: string;
  ownerKind: OwnerKind;
  itemId: string;
  kind: "movie" | "series" | "channel";
  title: string;
  logo: string | null;
  season: number | null;
  episode: number | null;
  positionSecs: number;
  durationSecs: number | null;
  percent: number;
};

export type BoardRow = {
  key: string;
  title: string;
  kind: string;
  items: CatalogItem[];
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

export const TMDB_KEY_SETTING = "tmdb_api_key";

export const listSources = () => invoke<Source[]>("list_sources");

export const addSource = (urlOrPath: string, kind: "url" | "file", label?: string) =>
  invoke<ImportReport>("add_source", { urlOrPath, kind, label: label ?? null });

export const syncSource = (id: number) => invoke<ImportReport>("sync_source", { id });

export const removeSource = (id: number) => invoke<void>("remove_source", { id });

export function fetchCatalog(input: {
  kind: CatalogKind;
  query: string;
  group: string;
  page: number;
  unwatchedOnly?: boolean;
}) {
  return invoke<CatalogPage>("catalog_page", {
    kind: input.kind,
    query: input.query,
    group: input.group || null,
    page: input.page,
    unwatchedOnly: input.unwatchedOnly ?? false,
  });
}

export const fetchItem = (kind: CatalogKind, id: string) =>
  invoke<ItemWithRelated>("catalog_item", { kind, id });

export const fetchEpisodes = (id: string) => invoke<SeriesEpisodes>("series_episodes", { id });

export const fetchBoard = () => invoke<BoardRow[]>("board");

export const fetchContinueWatching = (limit = 20) =>
  invoke<ContinueEntry[]>("continue_watching", { limit });

export const reportProgress = (
  ownerId: string,
  ownerKind: OwnerKind,
  position: number,
  duration: number | null,
) => invoke<Progress | null>("set_progress", { ownerId, ownerKind, position, duration });

export const markWatched = (ownerId: string, ownerKind: OwnerKind, completed: boolean) =>
  invoke<void>("mark_watched", { ownerId, ownerKind, completed });

export const fetchNextEpisode = (seriesId: string) =>
  invoke<EpisodeRef | null>("next_episode", { seriesId });

export const toggleFavorite = (ownerId: string) => invoke<boolean>("toggle_favorite", { ownerId });

export const fetchFavorites = () => invoke<CatalogItem[]>("favorites");

export const getSetting = (key: string) => invoke<string | null>("get_setting", { key });

export const setSetting = (key: string, value: string) =>
  invoke<void>("set_setting", { key, value });

export function fetchMeta(input: { kind: CatalogKind; title: string; year?: number | null }) {
  return invoke<Meta>("title_meta", {
    kind: input.kind,
    title: input.title,
    year: input.year ?? null,
  });
}

/**
 * Streams são HTTP puro e a WebView recusa carga insegura, então a reprodução
 * sempre passa pelo proxy local que o Rust sobe em 127.0.0.1.
 */
export const streamUrl = (url: string) => invoke<string>("stream_url", { url });
