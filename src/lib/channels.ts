import { createResource, createSignal, onMount } from "solid-js";
import { invoke } from "@tauri-apps/api/core";

export type RawChannel = {
  id: string;
  name: string;
  url: string;
  logo: string;
  category: string;
  channelNumber: number;
};

export type Channel = RawChannel & {
  group: string;
  quality: string[];
  /** name with quality tokens removed, for display and search */
  title: string;
  search: string;
};

export const GROUPS = [
  "Todos",
  "Abertos",
  "Esportes",
  "Filmes e séries",
  "Streaming",
  "Infantil",
  "Notícias",
  "Documentários",
  "Variedades",
  "Música",
  "Religiosos",
  "Adultos",
  "Outros",
] as const;

type Rule = [group: string, patterns: RegExp];

// Order matters: first match wins.
const RULES: Rule[] = [
  ["Adultos", /\b(xxx|adult|playboy|sexy|hot ?go|brasileirinhas|venus|privé|erot)/],
  [
    "Esportes",
    /\b(sportv|espn|premiere|combate|ufc|fight|dazn|eurosport|band ?sports|nsports|tnt ?sports|cazé|caze ?tv|sport|futebol|nba|nfl|mlb|nhl|paulistã|brasileirã|libertadores|goat|ppv|f1|fórmula|formula ?1|betnacional|golf|wwe|luta)/,
  ],
  [
    "Streaming",
    /(prime ?video|netflix|disney ?\+|apple ?tv|globoplay|star ?\+|paramount ?\+|\bmax ?\d|hbo ?max|pluto ?tv|watch ?br|clarotv ?\+|telecine ?on|looke|univer ?video)/,
  ],
  [
    "Infantil",
    /\b(cartoon|nick|nickelodeon|disney|gloob|gloobinho|discovery ?kids|boomerang|tooncast|baby ?tv|zoomoo|kids|infantil|junior)/,
  ],
  [
    "Filmes e séries",
    /\b(telecine|hbo|max\b|cinemax|tnt\b|space|tcm|megapix|paramount|universal|warner|axn|fx\b|amc\b|cinecanal|sony|syfy|star ?(channel|hits)|filmes?|cine|movie|série|series|24 ?h)/,
  ],
  [
    "Notícias",
    /\b(globonews|globo ?news|cnn|record ?news|band ?news|bandnews|jovem ?pan|cnbc|bloomberg|news|notícias|noticias|euronews|al ?jazeera)/,
  ],
  [
    "Documentários",
    /\b(discovery|history|h2\b|nat ?geo|national ?geographic|animal ?planet|investigaç|crime|id\b|curta|doc|science|turbo|home ?& ?health|food ?network)/,
  ],
  [
    "Abertos",
    /\b(globo|sbt|record|band\b|rede ?tv|redetv|tv ?cultura|tv ?brasil|gazeta|cnt\b|tv ?aparecida|rit\b|tv ?diário)/,
  ],
  ["Música", /\b(mtv|music|bis\b|vevo|multishow ?music|sertanejo|funk|pagode|rádio|radio)/],
  [
    "Religiosos",
    /\b(canção ?nova|cancao ?nova|rede ?vida|aparecida|gospel|novo ?tempo|boas ?novas|cristã|cristo|católic|catolic|igreja|universal ?tv|iurd|fé\b|padre)/,
  ],
  [
    "Variedades",
    /(\bmultishow|\bgnt|\bviva\b|\bcomedy|e!|\blifetime|a&e|\btlc\b|\bbio\b|\boff\b|woohoo|prime ?box|canal ?brasil|arte ?1|curta!|fashion ?tv|eurochannel|dog ?tv|fish ?tv|\bagro|canal ?do ?boi|canal ?do ?criador)/,
  ],
];

const QUALITY = /\b(4k|8k|uhd|fhd|full ?hd|hd|sd|h265|h264|hevc|alt(ernativ[oa])?\.?\d*)\b/gi;

function normalize(value: string) {
  return value
    .toLowerCase()
    .normalize("NFD")
    .replace(/[\u0300-\u036f]/g, "");
}

export function classify(name: string) {
  const plain = name.toLowerCase();
  for (const [group, patterns] of RULES) if (patterns.test(plain)) return group;
  return "Outros";
}

export function decorate(raw: RawChannel): Channel {
  const quality = [...raw.name.matchAll(QUALITY)].map(m => m[1].toUpperCase().replace(/\s+/g, " "));
  const title = raw.name
    .replace(QUALITY, "")
    .replace(/\s{2,}/g, " ")
    .replace(/[\s|·-]+$/, "")
    .trim();
  return {
    ...raw,
    group: classify(raw.name),
    quality: [...new Set(quality)],
    title: title || raw.name,
    search: normalize(raw.name),
  };
}

async function load(): Promise<Channel[]> {
  const raw = await invoke<RawChannel[]>("channels");
  return raw.map(decorate);
}

/** The channel list comes from Rust, which reads and caches the file on disk. */
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
