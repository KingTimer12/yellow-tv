import { A, useParams } from "@solidjs/router";
import { createMemo, createResource, createSignal, For, onMount, Show } from "solid-js";
import Player from "~/components/Player";
import TitleHero from "~/components/TitleHero";
import { Play } from "~/components/Icons";
import { fetchEpisodes, fetchItem, fetchMeta, shortGroup, type Episode, type Series } from "~/lib/vod";

export default function SeriePage() {
  const params = useParams<{ id: string }>();
  const [started, setStarted] = createSignal(false);
  const [season, setSeason] = createSignal<number>();
  const [current, setCurrent] = createSignal<Episode>();
  onMount(() => setStarted(true));

  const [data] = createResource(
    () => (started() ? params.id : undefined),
    id => fetchItem("series", id),
  );
  const [detail] = createResource(() => (started() ? params.id : undefined), fetchEpisodes);

  const series = () => data()?.item as Series | undefined;

  const [meta] = createResource(
    () => {
      const show = series();
      return show ? { kind: "series" as const, title: show.title } : undefined;
    },
    fetchMeta,
  );

  const seasons = createMemo(() => {
    const episodes = detail()?.episodes ?? [];
    return [...new Set(episodes.map(episode => episode.season))].sort((a, b) => a - b);
  });

  const activeSeason = () => season() ?? seasons()[0];

  const episodes = createMemo(() =>
    (detail()?.episodes ?? []).filter(episode => episode.season === activeSeason()),
  );

  return (
    <main class="relative z-10">
      <Show
        when={series()}
        fallback={
          <div class="mx-auto max-w-md px-4 py-24 text-center">
            <p class="font-mono text-sm text-paper/40">
              <Show when={data.error} fallback={<>carregando série<span class="anim-caret">_</span></>}>
                {(data.error as Error).message}
              </Show>
            </p>
            <Show when={data.error}>
              <A href="/series" class="mt-4 inline-block text-sm text-amber hover:underline">
                Voltar para as séries
              </A>
            </Show>
          </div>
        }
      >
        {show => (
          <>
            <Show when={current()}>
              {episode => (
                <div class="anim-reveal mx-auto max-w-6xl px-4 pt-4">
                  <Player
                    src={episode().url}
                    title={`${show().title} T${episode().season} E${episode().episode}`}
                    poster={show().logo}
                  />
                  <p class="mt-2 font-mono text-xs text-amber">
                    T{episode().season} · E{episode().episode}
                  </p>
                </div>
              )}
            </Show>

            <TitleHero
              title={show().title}
              poster={show().logo}
              subtitle={shortGroup(show().group)}
              meta={meta()}
              metaPending={meta.loading}
            >
              <div class="mt-6 flex flex-wrap items-center gap-3">
                <button
                  type="button"
                  onClick={() => setCurrent(detail()?.episodes[0])}
                  disabled={!detail()}
                  class="press flex items-center gap-2 rounded-sm bg-amber px-5 py-3 font-medium text-ink hover:bg-paper hover:shadow-[var(--shadow-glow)] disabled:opacity-40 disabled:hover:bg-amber disabled:hover:shadow-none"
                >
                  <Play size={16} />
                  Assistir do começo
                </button>
                <span class="text-sm text-paper/45">
                  {`${show().seasons.length} ${
                    show().seasons.length > 1 ? "temporadas" : "temporada"
                  } · ${show().episodeCount} episódios`}
                </span>
              </div>
            </TitleHero>

            <section class="mx-auto max-w-6xl px-4 py-8">
              <Show
                when={detail()}
                fallback={
                  <p class="font-mono text-sm text-paper/40">
                    carregando episódios<span class="anim-caret">_</span>
                  </p>
                }
              >
                <Show when={seasons().length > 1}>
                  <div
                    role="tablist"
                    aria-label="Temporadas"
                    class="mb-6 flex gap-1 overflow-x-auto border-b border-edge"
                  >
                    <For each={seasons()}>
                      {number => (
                        <button
                          type="button"
                          role="tab"
                          aria-selected={activeSeason() === number}
                          onClick={() => setSeason(number)}
                          class="press -mb-px shrink-0 border-b-2 px-4 py-2.5 text-sm"
                          classList={{
                            "border-amber text-amber": activeSeason() === number,
                            "border-transparent text-paper/50 hover:text-paper":
                              activeSeason() !== number,
                          }}
                        >
                          Temporada {number}
                        </button>
                      )}
                    </For>
                  </div>
                </Show>

                <ul role="tabpanel">
                  <For each={episodes()}>
                    {(episode, index) => {
                      const active = () =>
                        current()?.url === episode.url;
                      return (
                        <li class="anim-fade" style={{ "--i": Math.min(index(), 16) }}>
                          <button
                            type="button"
                            onClick={() => setCurrent(episode)}
                            class="press rail flex w-full items-center gap-4 border-b border-edge/60 py-3 pl-3.5 pr-2 text-left hover:bg-panel/80"
                            classList={{ "bg-amber/10": active() }}
                            data-current={String(active())}
                          >
                            <span
                              class="w-12 shrink-0 border-r border-edge pr-3 text-right font-mono text-sm tabular-nums text-amber-deep"
                              classList={{ "text-amber": active() }}
                            >
                              {String(episode.episode).padStart(2, "0")}
                            </span>
                            <span class="min-w-0 flex-1 truncate text-sm text-paper/90">
                              Episódio {episode.episode}
                            </span>
                            <span
                              class="shrink-0 font-mono text-xs text-paper/30"
                              classList={{ "text-amber": active() }}
                            >
                              {active() ? "tocando" : "assistir"}
                            </span>
                          </button>
                        </li>
                      );
                    }}
                  </For>
                </ul>
              </Show>
            </section>
          </>
        )}
      </Show>
    </main>
  );
}
