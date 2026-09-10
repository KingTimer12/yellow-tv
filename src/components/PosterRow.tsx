import { For, Show } from "solid-js";
import PosterCard from "~/components/PosterCard";
import type { CatalogItem } from "~/lib/api";

/** Linha horizontal no formato Stremio: o pôster manda, sem sinopse. */
export default function PosterRow(props: { title: string; items: CatalogItem[] }) {
  return (
    <Show when={props.items.length}>
      <section class="anim-reveal">
        <h2 class="mb-3 font-display text-sm font-bold tracking-tight text-paper/70">
          {props.title}
        </h2>
        <ul class="-mx-4 flex snap-x gap-4 overflow-x-auto px-4 pb-2 [scrollbar-width:thin]">
          <For each={props.items}>
            {(item, index) => (
              <li
                class="anim-poster w-[38vw] shrink-0 snap-start sm:w-44 lg:w-48"
                style={{ "--i": Math.min(index(), 12) }}
              >
                <PosterCard item={item} />
              </li>
            )}
          </For>
        </ul>
      </section>
    </Show>
  );
}
