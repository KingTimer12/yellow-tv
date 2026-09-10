import { createResource, For, Show } from "solid-js";
import PosterRow from "~/components/PosterRow";
import { fetchContinueWatching, fetchFavorites, markWatched, removeSource, syncSource } from "~/lib/api";
import { detailHref } from "~/lib/vod";
import { useSources } from "~/lib/sources";
import { A } from "@solidjs/router";
import { Check } from "~/components/Icons";

export default function Biblioteca() {
  const list = useSources();
  const [favorites, { refetch: refetchFavorites }] = createResource(fetchFavorites);
  const [history, { refetch: refetchHistory }] = createResource(() => fetchContinueWatching(40));

  return (
    <main class="relative z-10 mx-auto max-w-7xl space-y-10 px-4 py-6">
      <section>
        <h1 class="anim-reveal font-display text-3xl font-extrabold tracking-[-0.03em] text-paper">
          Biblioteca
        </h1>
      </section>

      <Show when={favorites()?.length}>
        <PosterRow title="Favoritos" items={favorites()!} />
      </Show>

      <section>
        <h2 class="mb-3 font-display text-sm font-bold tracking-tight text-paper/70">Histórico</h2>
        <Show
          when={history.error}
        >
          <p class="text-sm text-live">
            {history.error ? String(history.error) : "O histórico não carregou."}
          </p>
        </Show>
        <Show
          when={!history.error}
        >
          <Show
            when={history()?.length}
            fallback={
              <p class="text-sm text-paper/40">
                <Show when={history.loading} fallback="Nada assistido ainda.">
                  carregando histórico<span class="anim-caret">_</span>
                </Show>
              </p>
            }
          >
            <ul class="divide-y divide-edge/70">
              <For each={history()}>
                {entry => (
                  <li class="flex items-center gap-3 py-2.5">
                    <A
                      href={detailHref({ kind: entry.kind, id: entry.itemId })}
                      class="min-w-0 flex-1 text-sm text-paper/85 hover:text-amber"
                    >
                      {entry.title}
                      <Show when={entry.season !== null}>
                        <span class="ml-2 font-mono text-xs text-paper/40">
                          T{entry.season}E{entry.episode}
                        </span>
                      </Show>
                      <span class="mt-1 block h-[3px] w-40 bg-ink/70">
                        <span
                          class="block h-full bg-amber"
                          style={{ width: `${Math.round(entry.percent * 100)}%` }}
                        />
                      </span>
                    </A>
                    <button
                      type="button"
                      onClick={async () => {
                        await markWatched(entry.ownerId, entry.ownerKind, true);
                        await refetchHistory();
                        await refetchFavorites();
                      }}
                      class="press shrink-0 rounded-sm border border-edge p-2 text-paper/45 hover:border-amber hover:text-amber"
                      title="Marcar como visto"
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

      <section class="border-t border-edge pt-6">
        <h2 class="mb-3 font-display text-sm font-bold tracking-tight text-paper/70">
          Minhas listas
        </h2>
        <Show when={list.error()}>
          <p class="text-sm text-live">
            {list.error() ? String(list.error()) : "As listas não carregaram."}
          </p>
        </Show>
        <Show when={list.pending()}>
          <p class="text-sm text-paper/40">
            carregando listas<span class="anim-caret">_</span>
          </p>
        </Show>
        <Show when={!list.pending() && !list.error() && !list.sources().length}>
          <p class="text-sm text-paper/40">Nenhuma lista adicionada ainda.</p>
        </Show>
        <ul class="space-y-2">
          <For each={list.sources()}>
            {source => (
              <li class="flex items-center gap-3 rounded-sm border border-edge bg-panel/40 px-3 py-2">
                <div class="min-w-0">
                  <p class="truncate text-sm text-paper/90">{source.label}</p>
                  <p class="truncate font-mono text-[0.7rem] text-paper/35">
                    {source.itemCount.toLocaleString("pt-BR")} títulos
                    <Show when={source.lastSyncAt}>
                      {at => (
                        <>
                          {" · "}
                          {new Date(at() * 1000).toLocaleDateString("pt-BR")}
                        </>
                      )}
                    </Show>
                  </p>
                </div>
                <button
                  type="button"
                  onClick={async () => {
                    await syncSource(source.id);
                    await list.refetch();
                  }}
                  class="press ml-auto shrink-0 text-xs text-paper/55 hover:text-amber"
                >
                  Atualizar
                </button>
                <button
                  type="button"
                  onClick={async () => {
                    await removeSource(source.id);
                    await list.refetch();
                  }}
                  class="press shrink-0 text-xs text-paper/40 hover:text-live"
                >
                  Remover
                </button>
              </li>
            )}
          </For>
        </ul>
        <A href="/setup" class="wipe mt-4 inline-block text-sm text-amber hover:underline">
          Adicionar outra lista
        </A>
      </section>
    </main>
  );
}
