import { A, useParams } from "@solidjs/router";
import { createResource, createSignal, onMount, Show } from "solid-js";
import Player from "~/components/Player";
import PosterGrid from "~/components/PosterGrid";
import SourcePicker from "~/components/SourcePicker";
import TitleHero from "~/components/TitleHero";
import { Check, Play, StarOutline } from "~/components/Icons";
import { fetchFavorites, fetchItem, fetchMeta, markWatched, toggleFavorite } from "~/lib/api";
import { remainingLabel, shortGroup } from "~/lib/vod";

export default function FilmePage() {
  const params = useParams<{ id: string }>();
  const [started, setStarted] = createSignal(false);
  const [playing, setPlaying] = createSignal(false);
  const [source, setSource] = createSignal(0);
  const [favoriteOverride, setFavoriteOverride] = createSignal<boolean>();
  onMount(() => setStarted(true));

  const [data, { refetch }] = createResource(
    () => (started() ? params.id : undefined),
    id => fetchItem("filmes", id),
  );

  const movie = () => data()?.item;
  const streams = () => data()?.streams ?? [];
  const progress = () => data()?.progress ?? null;
  const resumeAt = () => {
    const stored = progress();
    return stored && !stored.completed ? stored.positionSecs : 0;
  };

  const [meta] = createResource(
    () => {
      const current = movie();
      return current
        ? { kind: "filmes" as const, title: current.title, year: current.year }
        : undefined;
    },
    fetchMeta,
  );

  // A lista de favoritos serve só para saber o estado inicial; depois do
  // primeiro toggle o signal local já reflete a verdade sem precisar recarregar.
  const [favoritesList] = createResource(() => (started() ? true : undefined), fetchFavorites);
  const favorite = () =>
    favoriteOverride() ?? (favoritesList()?.some(entry => entry.id === movie()?.id) ?? false);

  const primaryLabel = () => {
    if (playing()) return "Tocando";
    const at = resumeAt();
    return at > 0 ? remainingLabel(at, progress()?.durationSecs ?? null) : "Assistir filme";
  };

  return (
    <main class="relative z-10">
      <Show
        when={movie()}
        fallback={
          <div class="mx-auto max-w-md px-4 py-24 text-center">
            <p class="font-mono text-sm text-paper/40">
              <Show
                when={data.error}
                fallback={<>carregando filme<span class="anim-caret">_</span></>}
              >
                {String(data.error)}
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
            <Show when={playing() && streams()[source()]}>
              {stream => (
                <div class="anim-reveal mx-auto max-w-6xl px-4 pt-4">
                  <Player
                    src={stream().url}
                    title={current().title}
                    poster={current().logo ?? undefined}
                    owner={{ id: current().id, kind: "item" }}
                    startAt={resumeAt()}
                    onEnded={() => void refetch()}
                  />
                </div>
              )}
            </Show>

            <TitleHero
              title={current().title}
              poster={current().logo ?? ""}
              subtitle={shortGroup(current().group)}
              meta={meta()}
              metaPending={meta.loading}
            >
              <div class="mt-6 flex flex-wrap items-center gap-3">
                <button
                  type="button"
                  onClick={() => setPlaying(true)}
                  disabled={!streams().length}
                  class="press flex items-center gap-2 rounded-sm bg-amber px-5 py-3 font-medium text-ink hover:bg-paper hover:shadow-[var(--shadow-glow)] disabled:opacity-40"
                >
                  <Play size={16} />
                  {primaryLabel()}
                </button>

                <button
                  type="button"
                  onClick={async () => {
                    await markWatched(current().id, "item", !current().completed);
                    await refetch();
                  }}
                  class="press flex items-center gap-2 rounded-sm border border-edge px-4 py-3 text-sm text-paper/70 hover:border-amber hover:text-amber"
                  aria-pressed={current().completed}
                >
                  <Check size={16} />
                  {current().completed ? "Assistido" : "Marcar como visto"}
                </button>

                <button
                  type="button"
                  onClick={async () => setFavoriteOverride(await toggleFavorite(current().id))}
                  class="press flex items-center gap-2 rounded-sm border border-edge px-4 py-3 text-sm text-paper/70 hover:border-amber hover:text-amber"
                  classList={{ "border-amber/60 text-amber": favorite() }}
                  aria-pressed={favorite()}
                >
                  <StarOutline size={16} />
                  {favorite() ? "Nos favoritos" : "Salvar"}
                </button>

                <A href="/filmes" class="wipe text-sm text-paper/55 hover:text-amber">
                  Voltar ao catálogo
                </A>
              </div>

              <SourcePicker streams={streams()} active={source()} onPick={setSource} />
            </TitleHero>

            <Show when={data()?.related.length}>
              <section class="mx-auto max-w-6xl px-4 py-8">
                <h2 class="mb-4 font-display text-sm font-bold tracking-tight text-paper/70">
                  Mais em {shortGroup(current().group)}
                </h2>
                <PosterGrid items={data()!.related} />
              </section>
            </Show>
          </>
        )}
      </Show>
    </main>
  );
}
