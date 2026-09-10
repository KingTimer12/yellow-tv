import { For, Show, type JSX } from "solid-js";
import type { Meta } from "~/lib/api";

type TitleHeroProps = {
  title: string;
  poster: string;
  subtitle: string;
  meta: Meta | undefined;
  metaPending: boolean;
  children?: JSX.Element;
};

const hours = (minutes: number) =>
  minutes >= 60 ? `${Math.floor(minutes / 60)}h ${minutes % 60}min` : `${minutes}min`;

export default function TitleHero(props: TitleHeroProps) {
  const poster = () => props.meta?.poster || props.poster;

  return (
    <header class="relative overflow-hidden border-b border-edge">
      <Show when={props.meta?.backdrop}>
        {backdrop => (
          <div class="absolute inset-0 overflow-hidden" aria-hidden="true">
            <img
              src={backdrop()}
              alt=""
              class="anim-drift h-full w-full object-cover opacity-25"
            />
            <div class="absolute inset-0 bg-gradient-to-t from-ink via-ink/80 to-ink/40" />
            {/* Amber floor light: the broadcast still bleeds into the page. */}
            <div class="absolute inset-x-0 bottom-0 h-24 bg-gradient-to-t from-amber/[0.07] to-transparent" />
          </div>
        )}
      </Show>

      <div class="relative mx-auto flex max-w-6xl flex-col gap-6 px-4 py-8 sm:flex-row">
        <div
          class="anim-poster w-40 shrink-0 self-start overflow-hidden rounded-sm bg-panel ring-1 ring-edge/80 shadow-[var(--shadow-lift)] sm:w-56"
        >
          <Show
            when={poster()}
            fallback={
              <div class="grid aspect-[2/3] place-content-center text-xs text-paper/30">
                sem pôster
              </div>
            }
          >
            <img src={poster()} alt={`Pôster de ${props.title}`} class="w-full" />
          </Show>
        </div>

        <div class="min-w-0 flex-1">
          <h1
            class="anim-reveal font-display text-3xl font-extrabold leading-tight tracking-[-0.03em] text-paper sm:text-4xl"
            style={{ "--i": 1 }}
          >
            {props.title}
          </h1>

          <p
            class="anim-reveal mt-1 flex flex-wrap items-center gap-x-3 gap-y-1 text-sm text-paper/45"
            style={{ "--i": 2 }}
          >
            <span>{props.subtitle}</span>
            <Show when={props.meta?.year}>{year => <span class="tabular-nums">{year()}</span>}</Show>
            <Show when={props.meta?.runtime}>
              {runtime => <span class="tabular-nums">{hours(runtime())}</span>}
            </Show>
            <Show when={props.meta?.rating}>
              {rating => (
                <span class="text-amber tabular-nums">
                  {rating().toFixed(1)}
                  <span class="text-paper/35"> /10 no TMDB</span>
                </span>
              )}
            </Show>
          </p>

          <Show when={props.meta?.genres.length}>
            <ul class="anim-reveal mt-3 flex flex-wrap gap-1.5" style={{ "--i": 3 }}>
              <For each={props.meta!.genres}>
                {genre => (
                  <li class="rounded-full border border-edge bg-panel/60 px-2.5 py-0.5 text-xs text-paper/60">
                    {genre}
                  </li>
                )}
              </For>
            </ul>
          </Show>

          <Show when={props.meta?.tagline}>
            <p class="anim-reveal mt-4 font-display text-lg text-amber-deep" style={{ "--i": 4 }}>
              {props.meta!.tagline}
            </p>
          </Show>

          <Show
            when={props.meta?.overview}
            fallback={
              <p class="mt-4 max-w-prose text-sm text-paper/40">
                <Show
                  when={props.metaPending}
                  fallback={
                    props.meta?.reason
                      ? `Sinopse indisponível: ${props.meta.reason}`
                      : "Sem sinopse para este título."
                  }
                >
                  <span class="font-mono">
                    buscando sinopse<span class="anim-caret">_</span>
                  </span>
                </Show>
              </p>
            }
          >
            <p
              class="anim-reveal mt-4 max-w-prose leading-relaxed text-paper/80"
              style={{ "--i": 5 }}
            >
              {props.meta!.overview}
            </p>
          </Show>

          <Show when={props.meta?.imdbUrl}>
            {href => (
              <a
                href={href()}
                target="_blank"
                rel="noreferrer"
                class="wipe mt-3 inline-block text-sm text-amber"
              >
                Ver no IMDb
              </a>
            )}
          </Show>

          {props.children}
        </div>
      </div>

      <Show when={props.meta?.cast.length}>
        <div class="relative mx-auto max-w-6xl px-4 pb-8">
          <h2 class="mb-3 font-display text-sm font-bold tracking-tight text-paper/70">Elenco</h2>
          <ul class="flex gap-4 overflow-x-auto pb-2">
            <For each={props.meta!.cast}>
              {(person, index) => (
                <li class="anim-poster w-24 shrink-0" style={{ "--i": index() }}>
                  <div class="aspect-[2/3] overflow-hidden rounded-sm bg-panel ring-1 ring-edge/60">
                    <Show
                      when={person.photo}
                      fallback={
                        <span class="grid h-full place-content-center text-[0.65rem] text-paper/25">
                          sem foto
                        </span>
                      }
                    >
                      <img
                        src={person.photo!}
                        alt=""
                        loading="lazy"
                        decoding="async"
                        class="h-full w-full object-cover transition-transform duration-[var(--duration-slow)] ease-[var(--ease-out-soft)] hover:scale-105"
                      />
                    </Show>
                  </div>
                  <p class="mt-1.5 text-xs leading-tight text-paper/85">{person.name}</p>
                  <Show when={person.character}>
                    <p class="text-[0.7rem] leading-tight text-paper/35">{person.character}</p>
                  </Show>
                </li>
              )}
            </For>
          </ul>
        </div>
      </Show>
    </header>
  );
}
