#!/usr/bin/env bun

/**
 * update-vod.ts
 *
 * Lê a lista Filmes-Series.m3u8 — a fonte de vídeo sob demanda, que o
 * update-channel.ts descarta de propósito — e escreve o catálogo em data/vod/.
 *
 * A lista tem ~290 mil itens (79 MB) e traz um item por episódio, então a saída
 * é separada para o navegador nunca baixar mais do que a tela precisa:
 *
 *   vod/filmes.json         todos os filmes
 *   vod/series.json         uma entrada por série, sem os episódios
 *   vod/series/<id>.json    os episódios de uma série, por temporada
 *
 * Uso:
 *   bun run scripts/update-vod.ts
 */

const M3U_URL =
  'https://raw.githubusercontent.com/Ramys/Iptv-Brasil-2026/refs/heads/master/Filmes-Series.m3u8';

const DATA_DIR = process.env.YELLOWTV_DATA_DIR ?? `${process.cwd()}/data`;
const OUTPUT_DIR = `${DATA_DIR}/vod`;

const MEDIA_EXTENSIONS = ['.mp4', '.mkv', '.avi', '.m4v'];

interface Entry {
  name: string;
  url: string;
  logo: string;
  group: string;
}

interface Movie {
  id: string;
  title: string;
  year: number | null;
  logo: string;
  group: string;
  url: string;
}

interface Episode {
  season: number;
  episode: number;
  url: string;
}

interface Series {
  id: string;
  title: string;
  logo: string;
  group: string;
  seasons: number[];
  episodeCount: number;
}

/** "Título S01 E02" e "Título S01E02" — o resto é filme. */
const EPISODE_PATTERN = /^(.*?)[\s.-]+s(\d{1,3})\s*e(\d{1,4})$/i;
const YEAR_PATTERN = /\s*\((\d{4})\)\s*$/;

function isMediaFile(url: string): boolean {
  const path = url.toLowerCase().split('?')[0];
  return MEDIA_EXTENSIONS.some(extension => path.endsWith(extension));
}

function slugify(name: string): string {
  return name
    .toLowerCase()
    .normalize('NFD')
    .replace(/[\u0300-\u036f]/g, '')
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-|-$/g, '')
    .substring(0, 80);
}

/** As URLs de pôster vêm com barra dupla ("image.tmdb.org//t/p/…"). */
function cleanLogo(logo: string): string {
  return logo.replace(/([^:])\/{2,}/g, '$1/');
}

function attribute(line: string, name: string): string {
  return line.match(new RegExp(`${name}="([^"]*)"`))?.[1] ?? '';
}

/** Lê o M3U em pedaços: o arquivo tem 79 MB e não cabe confortavelmente em memória como string única. */
async function* readEntries(url: string): AsyncGenerator<Entry> {
  console.log(`📡 Baixando ${url}`);
  const response = await fetch(url);
  if (!response.ok) throw new Error(`HTTP ${response.status} ao buscar o M3U`);
  if (!response.body) throw new Error('resposta sem corpo');

  const decoder = new TextDecoder();
  let buffer = '';
  let pending: Entry | null = null;

  const handle = function* (raw: string): Generator<Entry> {
    const line = raw.trim();
    if (!line) return;

    if (line.startsWith('#EXTINF:')) {
      pending = {
        name: (line.match(/,(.*)$/)?.[1] ?? '').trim() || attribute(line, 'tvg-name'),
        url: '',
        logo: cleanLogo(attribute(line, 'tvg-logo')),
        group: attribute(line, 'group-title') || 'Outros',
      };
      return;
    }

    if (line.startsWith('#')) return;
    if (!pending) return;

    const entry: Entry = { ...pending, url: line };
    pending = null;
    if (entry.name && isMediaFile(entry.url)) yield entry;
  };

  for await (const chunk of response.body) {
    buffer += decoder.decode(chunk, { stream: true });
    const lines = buffer.split('\n');
    buffer = lines.pop() ?? '';
    for (const line of lines) yield* handle(line);
  }
  for (const line of (buffer + decoder.decode()).split('\n')) yield* handle(line);
}

async function main() {
  console.log('🎬 Atualizando catálogo de filmes e séries...\n');

  const movies: Movie[] = [];
  const series = new Map<string, Series & { episodes: Episode[] }>();
  const seenUrls = new Set<string>();
  let total = 0;
  let duplicates = 0;

  for await (const entry of readEntries(M3U_URL)) {
    total++;
    if (seenUrls.has(entry.url)) {
      duplicates++;
      continue;
    }
    seenUrls.add(entry.url);

    const episodeMatch = entry.name.match(EPISODE_PATTERN);

    if (episodeMatch) {
      const [, rawTitle, rawSeason, rawEpisode] = episodeMatch;
      const title = rawTitle.trim();
      const id = slugify(`${title}-${entry.group}`);
      let show = series.get(id);
      if (!show) {
        show = {
          id,
          title,
          logo: entry.logo,
          group: entry.group,
          seasons: [],
          episodeCount: 0,
          episodes: [],
        };
        series.set(id, show);
      }
      show.episodes.push({
        season: Number(rawSeason),
        episode: Number(rawEpisode),
        url: entry.url,
      });
      continue;
    }

    const yearMatch = entry.name.match(YEAR_PATTERN);
    const title = entry.name.replace(YEAR_PATTERN, '').trim();
    movies.push({
      id: slugify(`${title}-${entry.group}-${movies.length}`),
      title,
      year: yearMatch ? Number(yearMatch[1]) : null,
      logo: entry.logo,
      group: entry.group,
      url: entry.url,
    });

    if (total % 50_000 === 0) console.log(`   ↳ ${total.toLocaleString('pt-BR')} itens lidos...`);
  }

  if (!total) {
    console.error('❌ Nenhum item encontrado! Verifique o formato do M3U.');
    process.exit(1);
  }

  movies.sort((a, b) => a.title.localeCompare(b.title, 'pt-BR'));

  const shows = [...series.values()].sort((a, b) => a.title.localeCompare(b.title, 'pt-BR'));
  let episodeFiles = 0;

  for (const show of shows) {
    show.episodes.sort((a, b) => a.season - b.season || a.episode - b.episode);
    show.seasons = [...new Set(show.episodes.map(episode => episode.season))].sort((a, b) => a - b);
    show.episodeCount = show.episodes.length;

    await Bun.write(
      `${OUTPUT_DIR}/series/${show.id}.json`,
      JSON.stringify({
        id: show.id,
        title: show.title,
        logo: show.logo,
        group: show.group,
        episodes: show.episodes,
      }),
    );
    episodeFiles++;
  }

  const index = shows.map(({ episodes, ...show }) => show);

  const movieBytes = await Bun.write(`${OUTPUT_DIR}/filmes.json`, JSON.stringify(movies));
  const seriesBytes = await Bun.write(`${OUTPUT_DIR}/series.json`, JSON.stringify(index));

  const byGroup: Record<string, number> = {};
  for (const movie of movies) byGroup[movie.group] = (byGroup[movie.group] ?? 0) + 1;
  for (const show of index) byGroup[show.group] = (byGroup[show.group] ?? 0) + 1;

  const kb = (bytes: number) => `${(bytes / 1024).toFixed(0)} KB`;

  console.log(`\n✅ ${total.toLocaleString('pt-BR')} itens lidos (${duplicates} duplicados).`);
  console.log(`   ${movies.length.toLocaleString('pt-BR')} filmes`);
  console.log(
    `   ${index.length.toLocaleString('pt-BR')} séries, ${index
      .reduce((sum, show) => sum + show.episodeCount, 0)
      .toLocaleString('pt-BR')} episódios`,
  );

  console.log('\n📊 Por grupo:');
  Object.entries(byGroup)
    .sort((a, b) => b[1] - a[1])
    .slice(0, 20)
    .forEach(([group, count]) => console.log(`   ${group}: ${count}`));

  console.log(`\n💾 ${OUTPUT_DIR}/filmes.json (${kb(movieBytes)})`);
  console.log(`   ${OUTPUT_DIR}/series.json (${kb(seriesBytes)})`);
  console.log(`   ${OUTPUT_DIR}/series/ (${episodeFiles} arquivos)`);
}

main().catch(err => {
  console.error('❌ Erro:', err);
  process.exit(1);
});
