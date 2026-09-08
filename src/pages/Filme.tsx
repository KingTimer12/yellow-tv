import { A, useParams } from "@solidjs/router";
import { createResource, createSignal, onMount, Show } from "solid-js";
import Player from "~/components/Player";
import PosterGrid from "~/components/PosterGrid";
import TitleHero from "~/components/TitleHero";
import { Play } from "~/components/Icons";
import { fetchItem, fetchMeta, shortGroup, type CatalogItem, type Movie } from "~/lib/vod";

export default function FilmePage() {
  const params = useParams<{ id: string }>();
  const [started, setStarted] = createSignal(false);
  const [playing, setPlaying] = createSignal(false);
  onMount(() => setStarted(true));

  const [data] = createResource(
    () => (started() ? params.id : undefined),
    id => fetchItem("filmes", id),
  );

  const movie = () => data()?.item as Movie | undefined;

  const [meta] = createResource(
    () => {
      const current = movie();
      return current ? { kind: "filmes" as const, title: current.title, year: current.year } : undefined;
    },
    fetchMeta,
  );

  return (
    <main class="relative z-10">
      <Show
        when={movie()}
        fallback={
          <div class="mx-auto max-w-md px-4 py-24 text-center">
            <p class="font-mono text-sm text-paper/40">
              <Show when={data.error} fallback={<>carregando filme<span class="anim-caret">_</span></>}>
                {(data.error as Error).message}
              </Show>
            </p>
            <Show when={data.error}>
              <A href="/filmes" class="mt-4 inline-block text-sm text-amber hover:underline">
                Voltar para os filmes
              </A>
            </Show>
          </div>
        }
      >
        {current => (
          <>
            <Show when={playing()}>
              <div class="anim-reveal mx-auto max-w-6xl px-4 pt-4">
                <Player src={current().url} title={current().title} poster={current().logo} />
              </div>
            </Show>

            <TitleHero
              title={current().title}
              poster={current().logo}
              subtitle={shortGroup(current().group)}
              meta={meta()}
              metaPending={meta.loading}
            >
              <div class="mt-6 flex flex-wrap items-center gap-3">
                <button
                  type="button"
                  onClick={() => setPlaying(true)}
                  class="press flex items-center gap-2 rounded-sm bg-amber px-5 py-3 font-medium text-ink hover:bg-paper hover:shadow-[var(--shadow-glow)]"
                >
                  <Play size={16} />
                  {playing() ? "Tocando" : "Assistir filme"}
                </button>
                <A href="/filmes" class="wipe text-sm text-paper/55 hover:text-amber">
                  Voltar ao catálogo
                </A>
              </div>
            </TitleHero>

            <Show when={data()?.related.length}>
              <section class="mx-auto max-w-6xl px-4 py-8">
                <h2 class="mb-4 font-display text-sm font-bold tracking-tight text-paper/70">
                  Mais em {shortGroup(current().group)}
                </h2>
                <PosterGrid items={data()!.related as CatalogItem[]} kind="filmes" />
              </section>
            </Show>
          </>
        )}
      </Show>
    </main>
  );
}
