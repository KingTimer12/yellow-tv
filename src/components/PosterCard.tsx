import { A } from "@solidjs/router";
import { Show } from "solid-js";
import { detailHref, shortGroup } from "~/lib/vod";
import type { CatalogItem } from "~/lib/api";
import { Check, Play } from "./Icons";

/**
 * Um pôster tem três estados: intocado, em andamento (barra âmbar no rodapé) e
 * concluído (escurecido com um check).
 */
export default function PosterCard(props: { item: CatalogItem }) {
  const item = () => props.item;
  const started = () => item().percent > 0.001 && !item().completed;

  return (
    <A href={detailHref(item())} class="group block">
      <div class="relative aspect-[2/3] overflow-hidden rounded-sm bg-panel ring-1 ring-edge/70 transition-[transform,box-shadow] duration-[var(--duration-base)] ease-[var(--ease-out-soft)] group-hover:-translate-y-1 group-hover:shadow-[var(--shadow-glow)] group-focus-visible:-translate-y-1">
        <Show
          when={item().logo}
          fallback={
            <span class="grid h-full place-content-center px-2 text-center text-xs text-paper/30">
              sem pôster
            </span>
          }
        >
          <img
            src={item().logo!}
            alt=""
            loading="lazy"
            decoding="async"
            class="h-full w-full object-cover transition-transform duration-[var(--duration-slow)] ease-[var(--ease-out-soft)] group-hover:scale-[1.04]"
            classList={{ "opacity-45": item().completed }}
            onError={event => (event.currentTarget.style.visibility = "hidden")}
          />
        </Show>

        <div
          class="pointer-events-none absolute inset-0 flex items-end justify-start bg-gradient-to-t from-ink/90 via-ink/10 to-transparent p-3 opacity-0 transition-opacity duration-[var(--duration-base)] group-hover:opacity-100"
          aria-hidden="true"
        >
          <span class="grid size-9 place-content-center rounded-full bg-amber text-ink shadow-[var(--shadow-lift)]">
            <Play size={15} />
          </span>
        </div>

        <Show when={item().completed}>
          <span
            class="absolute right-2 top-2 grid size-6 place-content-center rounded-full bg-ink/85 text-amber backdrop-blur-sm"
            title="Já assistido"
          >
            <Check size={13} />
          </span>
        </Show>

        {/* O rótulo de progresso ganha do contador de temporadas: saber onde
            você parou vale mais que saber o tamanho da série. */}
        <Show
          when={item().progressLabel}
          fallback={
            <Show when={item().kind === "series" && item().episodeCount > 0}>
              <span class="absolute bottom-0 right-0 rounded-tl-sm bg-ink/85 px-1.5 py-0.5 font-mono text-[0.65rem] tabular-nums text-amber backdrop-blur-sm">
                {item().seasons}T · {item().episodeCount}ep
              </span>
            </Show>
          }
        >
          {label => (
            <span class="absolute bottom-0 right-0 rounded-tl-sm bg-amber px-1.5 py-0.5 font-mono text-[0.65rem] font-bold tabular-nums text-ink">
              {label()}
            </span>
          )}
        </Show>

        <Show when={started()}>
          <span
            class="absolute inset-x-0 bottom-0 h-[3px] bg-ink/70"
            aria-hidden="true"
          >
            <span
              class="block h-full bg-amber"
              style={{ width: `${Math.round(item().percent * 100)}%` }}
            />
          </span>
        </Show>
      </div>

      <p class="mt-2 line-clamp-2 text-sm text-paper/90 transition-colors duration-[var(--duration-fast)] group-hover:text-amber">
        {item().title}
      </p>
      <p class="text-xs text-paper/35">{shortGroup(item().group)}</p>
    </A>
  );
}
