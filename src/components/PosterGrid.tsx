import { A } from "@solidjs/router";
import { For, Show } from "solid-js";
import { detailHref, isSeries, shortGroup, type CatalogItem, type CatalogKind } from "~/lib/vod";
import { Play } from "./Icons";

type PosterGridProps = {
  items: CatalogItem[];
  kind: CatalogKind;
};

export default function PosterGrid(props: PosterGridProps) {
  return (
    <ul class="grid grid-cols-2 gap-x-4 gap-y-7 sm:grid-cols-3 lg:grid-cols-5 xl:grid-cols-6">
      <For each={props.items}>
        {(item, index) => (
          <li class="anim-poster" style={{ "--i": Math.min(index(), 18) }}>
            <A href={detailHref(props.kind, item.id)} class="group block">
              <div class="relative aspect-[2/3] overflow-hidden rounded-sm bg-panel ring-1 ring-edge/70 transition-[transform,box-shadow,ring-color] duration-[var(--duration-base)] ease-[var(--ease-out-soft)] group-hover:-translate-y-1 group-hover:shadow-[var(--shadow-glow)] group-focus-visible:-translate-y-1">
                <Show
                  when={item.logo}
                  fallback={
                    <span class="grid h-full place-content-center px-2 text-center text-xs text-paper/30">
                      sem pôster
                    </span>
                  }
                >
                  <img
                    src={item.logo}
                    alt=""
                    loading="lazy"
                    decoding="async"
                    class="h-full w-full object-cover transition-transform duration-[var(--duration-slow)] ease-[var(--ease-out-soft)] group-hover:scale-[1.04]"
                    onError={event => (event.currentTarget.style.visibility = "hidden")}
                  />
                </Show>

                {/* Play affordance fades up from the bottom edge on hover. */}
                <div
                  class="pointer-events-none absolute inset-0 flex items-end justify-start bg-gradient-to-t from-ink/90 via-ink/10 to-transparent p-3 opacity-0 transition-opacity duration-[var(--duration-base)] group-hover:opacity-100"
                  aria-hidden="true"
                >
                  <span class="grid size-9 place-content-center rounded-full bg-amber text-ink shadow-[var(--shadow-lift)]">
                    <Play size={15} />
                  </span>
                </div>

                <Show when={isSeries(item) ? item : undefined} keyed>
                  {series => (
                    <span class="absolute bottom-0 right-0 rounded-tl-sm bg-ink/85 px-1.5 py-0.5 font-mono text-[0.65rem] tabular-nums text-amber backdrop-blur-sm">
                      {series.seasons.length}T · {series.episodeCount}ep
                    </span>
                  )}
                </Show>
              </div>

              <p class="mt-2 line-clamp-2 text-sm text-paper/90 transition-colors duration-[var(--duration-fast)] group-hover:text-amber">
                {item.title}
              </p>
              <p class="text-xs text-paper/35">{shortGroup(item.group)}</p>
            </A>
          </li>
        )}
      </For>
    </ul>
  );
}
