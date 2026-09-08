import { A, useNavigate, useParams } from "@solidjs/router";
import { createEffect, createMemo, onCleanup, onMount, Show } from "solid-js";
import ChannelList from "~/components/ChannelList";
import ChannelSkeleton from "~/components/ChannelSkeleton";
import { ChevronLeft, ChevronRight, StarFilled, StarOutline } from "~/components/Icons";
import Player from "~/components/Player";
import { formatNumber, useChannels } from "~/lib/channels";
import { hydrateStores, isFavorite, markWatched, toggleFavorite } from "~/lib/store";

export default function Watch() {
  const params = useParams<{ id: string }>();
  const navigate = useNavigate();
  const list = useChannels();

  const all = list.channels;
  const channel = createMemo(() => all().find(item => item.id === params.id));
  const neighbours = createMemo(() => {
    const current = channel();
    if (!current) return all();
    const siblings = all().filter(other => other.group === current.group);
    return siblings.length > 1 ? siblings : all();
  });

  const step = (delta: number) => {
    const list = neighbours();
    if (!list.length) return undefined;
    const at = list.findIndex(other => other.id === params.id);
    return list[(at + delta + list.length) % list.length];
  };

  onMount(() => {
    hydrateStores();
    const onKey = (event: KeyboardEvent) => {
      if (event.metaKey || event.ctrlKey || event.altKey) return;
      const next =
        event.key === "ArrowDown" || event.key === "PageDown"
          ? step(1)
          : event.key === "ArrowUp" || event.key === "PageUp"
            ? step(-1)
            : undefined;
      if (!next) return;
      event.preventDefault();
      navigate(`/watch/${next.id}`);
    };
    window.addEventListener("keydown", onKey);
    onCleanup(() => window.removeEventListener("keydown", onKey));
  });

  createEffect(() => {
    if (channel()) markWatched(params.id);
  });

  return (
    <main class="relative z-10 flex flex-col lg:h-[calc(100vh-57px)] lg:flex-row">
      <div class="min-w-0 flex-1">
        <Show
          when={channel()}
          fallback={
            <div class="tuner-sweep relative grid aspect-video place-content-center gap-3 bg-black text-center">
              <p class="font-mono text-sm text-paper/40">
                <Show when={list.pending()} fallback="Canal não encontrado.">
                  sintonizando<span class="anim-caret">_</span>
                </Show>
              </p>
              <Show when={!list.pending()}>
                <A href="/" class="wipe relative z-10 mx-auto text-sm text-amber">
                  Ver todos os canais
                </A>
              </Show>
            </div>
          }
        >
          {current => (
            <>
              <Player src={current().url} title={current().title} poster={current().logo} />
              <div class="anim-reveal flex flex-wrap items-center gap-x-4 gap-y-2 border-b border-edge px-4 py-4">
                <span class="font-mono text-2xl tabular-nums text-amber [text-shadow:0_0_20px_rgb(255_209_26/0.35)]">
                  {formatNumber(current().channelNumber)}
                </span>
                <h1 class="font-display text-2xl font-bold tracking-tight text-paper">
                  {current().title}
                </h1>
                <span class="flex items-center gap-1.5 rounded-full border border-live/40 bg-live/10 px-2 py-0.5 text-xs text-live">
                  <span
                    class="anim-live-dot inline-block size-1.5 rounded-full bg-live"
                    aria-hidden="true"
                  />
                  ao vivo
                </span>
                <span class="text-sm text-paper/45">{current().group}</span>
                <div class="ml-auto flex items-center gap-2">
                  <button
                    type="button"
                    onClick={() => toggleFavorite(current().id)}
                    class="press flex items-center gap-2 rounded-sm border border-edge px-3 py-2 text-sm text-paper/70 hover:border-amber hover:text-amber"
                    classList={{ "border-amber/60 text-amber": isFavorite(current().id) }}
                    aria-pressed={isFavorite(current().id)}
                  >
                    <Show when={isFavorite(current().id)} fallback={<StarOutline size={16} />} keyed>
                      <StarFilled size={16} class="anim-star-pop" />
                    </Show>
                    {isFavorite(current().id) ? "Nos favoritos" : "Salvar canal"}
                  </button>
                  <A
                    href={`/watch/${step(-1)?.id ?? current().id}`}
                    class="press flex items-center gap-1.5 rounded-sm border border-edge px-3 py-2 text-sm text-paper/70 hover:border-amber hover:text-amber"
                  >
                    <ChevronLeft size={16} />
                    Anterior
                  </A>
                  <A
                    href={`/watch/${step(1)?.id ?? current().id}`}
                    class="press flex items-center gap-1.5 rounded-sm border border-edge px-3 py-2 text-sm text-paper/70 hover:border-amber hover:text-amber"
                  >
                    Próximo
                    <ChevronRight size={16} />
                  </A>
                </div>
              </div>
              <p class="px-4 py-3 text-xs text-paper/35">
                Use as setas{" "}
                <kbd class="rounded-sm border border-edge px-1 font-mono">↑</kbd>{" "}
                <kbd class="rounded-sm border border-edge px-1 font-mono">↓</kbd> para trocar de
                canal dentro de {current().group}.
              </p>
            </>
          )}
        </Show>
      </div>

      <aside class="min-w-0 border-t border-edge lg:w-80 lg:shrink-0 lg:overflow-y-auto lg:border-l lg:border-t-0">
        <h2 class="sticky top-0 border-b border-edge bg-ink/95 px-4 py-3 font-display text-sm font-bold tracking-tight text-paper/70 backdrop-blur">
          <Show when={channel()} fallback="Canais">
            {current => <>Mais em {current().group}</>}
          </Show>
        </h2>
        <Show when={!list.pending()} fallback={<ChannelSkeleton count={10} />}>
          <ChannelList channels={neighbours()} currentId={params.id} />
        </Show>
      </aside>
    </main>
  );
}
