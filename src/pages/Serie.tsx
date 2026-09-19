import { A, useParams } from "@solidjs/router";
import { createMemo, createResource, createSignal, For, onMount, Show } from "solid-js";
import Player from "~/components/Player";
import TitleHero from "~/components/TitleHero";
import { Check, Play } from "~/components/Icons";
import {
  fetchEpisodes,
  fetchItem,
  fetchMeta,
  fetchNextEpisode,
  markWatched,
  type EpisodeRow,
} from "~/lib/api";
import { shortGroup } from "~/lib/vod";

export default function SeriePage() {
  const params = useParams<{ id: string }>();
  const [started, setStarted] = createSignal(false);
  const [season, setSeason] = createSignal<number>();
  const [current, setCurrent] = createSignal<EpisodeRow>();
  const [source, setSource] = createSignal(0);
  onMount(() => setStarted(true));

  const [data] = createResource(
    () => (started() ? params.id : undefined),
    id => fetchItem("series", id),
  );
  const [detail, { refetch: refetchEpisodes }] = createResource(
    () => (started() ? params.id : undefined),
    fetchEpisodes,
  );

  const series = () => data()?.item;

  const [meta] = createResource(
    () => {
      const show = series();
      return show ? { kind: "series" as const, title: show.title } : undefined;
    },
    fetchMeta,
  );

  const seasons = createMemo(() => [
    ...new Set((detail()?.episodes ?? []).map(episode => episode.season)),
  ]);
  const activeSeason = () => season() ?? seasons()[0];

  // Rótulo do próximo episódio na ordem da série. Só existe quando há um
  // próximo: é ele que liga a contagem regressiva no fim do episódio.
  const nextLabel = () => {
    const playing = current();
    if (!playing) return undefined;
    const all = detail()?.episodes ?? [];
    const index = all.findIndex(episode => episode.id === playing.id);
    const next = index >= 0 ? all[index + 1] : undefined;
    return next ? `T${next.season} · E${next.episode}` : undefined;
  };
  const episodes = createMemo(() =>
    (detail()?.episodes ?? []).filter(episode => episode.season === activeSeason()),
  );

  /** O botão primário aponta para o primeiro episódio não concluído. */
  const upNext = createMemo(() => (detail()?.episodes ?? []).find(episode => !episode.completed));

  const play = (episode: EpisodeRow) => {
    setSource(0);
    setCurrent(episode);
  };

  const advance = async () => {
    const show = series();
    if (!show) return;
    await refetchEpisodes();
    const next = await fetchNextEpisode(show.id);
    if (!next) return setCurrent(undefined);
    const row = (detail()?.episodes ?? []).find(episode => episode.id === next.id);
    if (row) {
      setSeason(row.season);
      play(row);
    }
  };

  return (
    <main class="relative z-10">
      <Show
        when={series()}
        fallback={
          <div class="mx-auto max-w-md px-4 py-24 text-center">
            <p class="font-mono text-sm text-paper/40">
              <Show
                when={data.error}
                fallback={<>carregando série<span class="anim-caret">_</span></>}
              >
                {String(data.error)}
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
                  <Show when={episode().streams[source()]}>
                    {stream => (
                      <Player
                        src={stream().url}
                        title={`${show().title} T${episode().season} E${episode().episode}`}
                        poster={show().logo ?? undefined}
                        owner={{ id: episode().id, kind: "episode" }}
                        startAt={episode().completed ? 0 : episode().positionSecs}
                        streams={episode().streams}
                        activeStream={source()}
                        onPickStream={setSource}
                        nextLabel={nextLabel()}
                        onNext={advance}
                      />
                    )}
                  </Show>
                  <p class="mt-2 font-mono text-xs text-amber">
                    T{episode().season} · E{episode().episode}
                  </p>
                </div>
              )}
            </Show>

            <TitleHero
              title={show().title}
              poster={show().logo ?? ""}
              subtitle={shortGroup(show().group)}
              meta={meta()}
              metaPending={meta.loading}
            >
              <div class="mt-6 flex flex-wrap items-center gap-3">
                <button
                  type="button"
                  onClick={() => {
                    const next = upNext();
                    if (!next) return;
                    setSeason(next.season);
                    play(next);
                  }}
                  disabled={!upNext() || !detail()}
                  class="press flex items-center gap-2 rounded-sm bg-amber px-5 py-3 font-medium text-ink hover:bg-paper hover:shadow-[var(--shadow-glow)] disabled:opacity-40"
                >
                  <Play size={16} />
                  <Show when={upNext()} fallback={<>Série concluída</>}>
                    {next =>
                      next().percent > 0
                        ? `Retomar T${next().season} E${next().episode}`
                        : `Assistir T${next().season} E${next().episode}`
                    }
                  </Show>
                </button>
                <span class="text-sm text-paper/45">
                  {`${show().seasons} ${show().seasons > 1 ? "temporadas" : "temporada"} · ${show().episodeCount} episódios`}
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
                  <ul class="mb-4 flex flex-wrap gap-2">
                    <For each={seasons()}>
                      {value => (
                        <li>
                          <button
                            type="button"
                            onClick={() => setSeason(value)}
                            class="press rounded-full border border-edge bg-panel/50 px-3 py-1 text-sm text-paper/60 hover:border-amber-deep hover:text-amber"
                            classList={{
                              "border-amber bg-amber/12 text-amber": activeSeason() === value,
                            }}
                          >
                            T{value}
                          </button>
                        </li>
                      )}
                    </For>
                  </ul>
                </Show>

                <Show
                  when={episodes().length}
                  fallback={
                    <p class="font-mono text-sm text-paper/40">nenhum episódio nesta temporada</p>
                  }
                >
                  <ul class="divide-y divide-edge/70">
                    <For each={episodes()}>
                      {episode => (
                        <li class="flex items-center gap-3 py-2.5">
                          <button
                            type="button"
                            onClick={() => play(episode)}
                            disabled={!episode.streams.length}
                            class="press flex min-w-0 flex-1 items-center gap-3 text-left disabled:opacity-40"
                          >
                            <span class="w-14 shrink-0 font-mono text-xs tabular-nums text-paper/40">
                              T{episode.season}E{episode.episode}
                            </span>
                            <span class="min-w-0 flex-1">
                              <span class="block truncate text-sm text-paper/85">
                                {episode.title ?? `Episódio ${episode.episode}`}
                              </span>
                              <Show when={episode.percent > 0 && !episode.completed}>
                                <span class="mt-1 block h-[3px] w-32 bg-ink/70">
                                  <span
                                    class="block h-full bg-amber"
                                    style={{ width: `${Math.round(episode.percent * 100)}%` }}
                                  />
                                </span>
                              </Show>
                            </span>
                          </button>
                          <button
                            type="button"
                            onClick={async () => {
                              await markWatched(episode.id, "episode", !episode.completed);
                              await refetchEpisodes();
                            }}
                            class="press shrink-0 rounded-sm border border-edge p-2 text-paper/45 hover:border-amber hover:text-amber"
                            classList={{ "border-amber/60 text-amber": episode.completed }}
                            aria-pressed={episode.completed}
                            title={episode.completed ? "Marcar como não visto" : "Marcar como visto"}
                          >
                            <Check size={14} />
                          </button>
                        </li>
                      )}
                    </For>
                  </ul>
                </Show>
              </Show>
            </section>
          </>
        )}
      </Show>
    </main>
  );
}
