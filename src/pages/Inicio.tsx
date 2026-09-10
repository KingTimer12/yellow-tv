import { createResource, For, Show } from "solid-js";
import LazyRow from "~/components/LazyRow";
import PosterRow from "~/components/PosterRow";
import PosterSkeleton from "~/components/PosterSkeleton";
import { fetchBoard } from "~/lib/api";

export default function Inicio() {
  const [rows] = createResource(fetchBoard);

  return (
    <main class="relative z-10 mx-auto max-w-7xl space-y-8 px-4 py-6">
      <Show
        when={!rows.loading}
        fallback={
          <div class="space-y-6">
            <PosterSkeleton />
          </div>
        }
      >
        <Show
          when={!rows.error}
          fallback={<p class="anim-fade py-12 text-sm text-live">{String(rows.error)}</p>}
        >
          <Show
            when={rows()?.length}
            fallback={
              <div class="anim-reveal py-16 text-center">
                <p class="font-display text-lg text-paper/70">Catálogo vazio.</p>
                <p class="mt-1 text-sm text-paper/40">
                  Atualize a lista em Biblioteca para trazer os títulos.
                </p>
              </div>
            }
          >
            {/* As duas primeiras fileiras montam de imediato — são as que
                aparecem sem rolar. O resto espera chegar perto da viewport. */}
            <For each={rows()}>
              {(row, index) => (
                <div style={{ "--i": Math.min(index(), 8) }}>
                  <Show
                    when={index() >= 2}
                    fallback={<PosterRow title={row.title} items={row.items} />}
                  >
                    <LazyRow>
                      <PosterRow title={row.title} items={row.items} />
                    </LazyRow>
                  </Show>
                </div>
              )}
            </For>
          </Show>
        </Show>
      </Show>
    </main>
  );
}
