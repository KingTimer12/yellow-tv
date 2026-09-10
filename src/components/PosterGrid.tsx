import { For } from "solid-js";
import PosterCard from "~/components/PosterCard";
import type { CatalogItem } from "~/lib/api";

export default function PosterGrid(props: { items: CatalogItem[] }) {
  return (
    <ul class="grid grid-cols-2 gap-x-4 gap-y-7 sm:grid-cols-3 lg:grid-cols-5 xl:grid-cols-6">
      <For each={props.items}>
        {(item, index) => (
          <li class="anim-poster" style={{ "--i": Math.min(index(), 18) }}>
            <PosterCard item={item} />
          </li>
        )}
      </For>
    </ul>
  );
}
