import { useSearchParams } from "@solidjs/router";
import { createMemo, createResource, createSignal, For, onCleanup, onMount, Show } from "solid-js";
import ChannelList from "~/components/ChannelList";
import ChannelSkeleton from "~/components/ChannelSkeleton";
import { Search, StarOutline } from "~/components/Icons";
import { matches, parseQuery, useChannels } from "~/lib/channels";
import { fetchContinueWatching, fetchFavorites } from "~/lib/api";

export default function Browse() {
  const list = useChannels();
  const [params, setParams] = useSearchParams<{ q?: string; g?: string }>();
  const [search, setSearch] = createSignal<HTMLInputElement>();

  const [favoriteItems, { refetch: refetchFavorites }] = createResource(fetchFavorites);
  const favoriteIds = () => new Set((favoriteItems() ?? []).map(item => item.id));

  onMount(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== "/" || event.metaKey || event.ctrlKey) return;
      const active = document.activeElement?.tagName;
      if (active === "INPUT" || active === "TEXTAREA") return;
      event.preventDefault();
      search()?.focus();
    };
    window.addEventListener("keydown", onKey);
    onCleanup(() => window.removeEventListener("keydown", onKey));
  });

  const all = list.channels;
  const group = () => params.g ?? "Todos";
  const query = () => params.q ?? "";

  const groups = createMemo(() => [
    "Todos",
    ...[...new Set(all().map(channel => channel.group).filter(Boolean))].sort(),
  ] as string[]);

  const counts = createMemo(() => {
    const tally = new Map<string, number>();
    for (const channel of all()) {
      if (!channel.group) continue;
      tally.set(channel.group, (tally.get(channel.group) ?? 0) + 1);
    }
    return tally;
  });

  const filtered = createMemo(() => {
    const terms = parseQuery(query());
    const active = group();
    const favorites = favoriteIds();
    return all().filter(channel => {
      if (active === "favoritos" && !favorites.has(channel.id)) return false;
      if (active !== "Todos" && active !== "favoritos" && channel.group !== active) return false;
      return terms.length ? matches(channel, terms) : true;
    });
  });

  const [recent] = createResource(() => fetchContinueWatching(8));
  const continueWatching = createMemo(() => {
    const byId = new Map(all().map(channel => [channel.id, channel]));
    return (recent() ?? [])
      .filter(entry => entry.kind === "channel")
      .map(entry => byId.get(entry.itemId))
      .filter((channel): channel is NonNullable<typeof channel> => Boolean(channel));
  });

  return (
    <main class="relative z-10 mx-auto flex max-w-6xl flex-col gap-0 md:flex-row">
      <aside class="shrink-0 border-b border-edge px-4 py-4 md:w-56 md:border-b-0 md:border-r md:py-6">
        <ul class="flex gap-2 overflow-x-auto whitespace-nowrap md:block md:space-y-0.5 md:overflow-visible">
          <li class="anim-fade">
            <button
              type="button"
              onClick={() => setParams({ g: "favoritos" })}
              class="press flex w-full items-center gap-2 rounded-sm px-2.5 py-1.5 text-left text-sm text-paper/55 hover:bg-panel/70 hover:text-amber md:px-2"
              classList={{ "bg-amber/10 text-amber": group() === "favoritos" }}
            >
              <StarOutline size={15} class="shrink-0" />
              <span>Favoritos</span>
              <span class="ml-auto font-mono text-[0.7rem] tabular-nums text-paper/30">
                {favoriteIds().size}
              </span>
            </button>
          </li>
          <For each={groups()}>
            {(name, index) => (
              <li class="anim-fade" style={{ "--i": Math.min(index() + 1, 14) }}>
                <button
                  type="button"
                  onClick={() => setParams({ g: name === "Todos" ? undefined : name })}
                  class="press flex w-full items-center gap-2 rounded-sm px-2.5 py-1.5 text-left text-sm text-paper/55 hover:bg-panel/70 hover:text-amber md:px-2"
                  classList={{ "bg-amber/10 text-amber": group() === name }}
                >
                  <span class="truncate">{name}</span>
                  <span class="ml-auto font-mono text-[0.7rem] tabular-nums text-paper/30">
                    {name === "Todos" ? all().length : counts().get(name) ?? 0}
                  </span>
                </button>
              </li>
            )}
          </For>
        </ul>
      </aside>

      <section class="min-w-0 flex-1">
        <div class="sticky top-[57px] z-10 border-b border-edge bg-ink/95 px-4 py-4 backdrop-blur">
          <label class="flex items-center gap-3 rounded-sm border border-edge bg-panel px-3 py-2.5 transition-[border-color,box-shadow] duration-[var(--duration-base)] focus-within:border-amber focus-within:shadow-[0_0_0_3px_rgb(255_209_26/0.12)]">
            <Search size={16} class="shrink-0 text-paper/35" />
            <input
              ref={setSearch}
              type="search"
              value={query()}
              onInput={event => setParams({ q: event.currentTarget.value || undefined })}
              placeholder="Buscar canal por nome"
              class="w-full bg-transparent text-paper outline-none placeholder:text-paper/30"
            />
            <span class="shrink-0 font-mono text-xs tabular-nums text-paper/35">
              {filtered().length}
            </span>
          </label>
        </div>

        <Show when={continueWatching().length && !query()}>
          <div class="border-b border-edge px-4 py-4">
            <h2 class="mb-2 font-display text-sm font-bold tracking-tight text-paper/70">
              Você estava vendo
            </h2>
            <ul class="flex gap-2 overflow-x-auto pb-1">
              <For each={continueWatching()}>
                {(channel, index) => (
                  <li class="anim-reveal" style={{ "--i": index() }}>
                    <a
                      href={`/watch/${channel.id}`}
                      class="press block max-w-44 truncate rounded-sm border border-edge bg-panel/40 px-3 py-2 text-sm text-paper/80 hover:border-amber hover:bg-panel hover:text-amber"
                    >
                      {channel.title}
                    </a>
                  </li>
                )}
              </For>
            </ul>
          </div>
        </Show>

        <Show
          when={list.error()}
          fallback={
            <Show
              when={!list.pending()}
              fallback={<ChannelSkeleton />}
            >
              <ChannelList
                channels={filtered()}
                empty={
                  group() === "favoritos"
                    ? "Sem favoritos ainda. Toque na estrela de um canal para salvá-lo aqui."
                    : "Nenhum canal com esse nome."
                }
                favoriteIds={favoriteIds()}
                onToggle={refetchFavorites}
              />
            </Show>
          }
        >
          <p class="anim-fade p-8 text-sm text-live">
            {list.error()?.message ?? "A lista de canais não carregou."}
          </p>
        </Show>
      </section>
    </main>
  );
}
