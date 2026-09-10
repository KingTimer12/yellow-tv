import { useSearchParams } from "@solidjs/router";
import { createMemo, createResource, createSignal, For, onCleanup, onMount, Show } from "solid-js";
import PosterGrid from "~/components/PosterGrid";
import PosterSkeleton from "~/components/PosterSkeleton";
import { fetchCatalog, type CatalogKind } from "~/lib/api";
import { shortGroup } from "~/lib/vod";
import { ChevronLeft, ChevronRight, Search } from "./Icons";

type CatalogBrowserProps = {
  kind: CatalogKind;
  heading: string;
  placeholder: string;
};

export default function CatalogBrowser(props: CatalogBrowserProps) {
  const [params, setParams] = useSearchParams<{ q?: string; g?: string; p?: string; u?: string }>();
  const [search, setSearch] = createSignal<HTMLInputElement>();
  const [started, setStarted] = createSignal(false);

  onMount(() => {
    setStarted(true);
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

  const query = () => params.q ?? "";
  const group = () => params.g ?? "";
  const page = () => Number(params.p ?? 0);
  const unwatchedOnly = () => params.u === "1";

  const [data] = createResource(
    () =>
      started()
        ? {
            kind: props.kind,
            query: query(),
            group: group(),
            page: page(),
            unwatchedOnly: unwatchedOnly(),
          }
        : undefined,
    fetchCatalog,
  );

  const pending = () => !data() && !data.error;
  const pages = createMemo(() => {
    const result = data();
    return result ? Math.ceil(result.total / result.pageSize) : 0;
  });

  const move = (delta: number) => {
    setParams({ p: String(Math.max(0, page() + delta)) });
    window.scrollTo({ top: 0, behavior: "smooth" });
  };

  const chip =
    "press flex items-baseline gap-1.5 rounded-full border border-edge bg-panel/50 px-3 py-1 text-sm text-paper/60 hover:border-amber-deep hover:text-amber";

  return (
    <main class="relative z-10 mx-auto max-w-7xl px-4 py-6">
      <div class="anim-reveal flex flex-wrap items-baseline gap-x-4 gap-y-2">
        <h1 class="font-display text-3xl font-extrabold tracking-[-0.03em] text-paper">
          {props.heading}
        </h1>
        <Show when={data()}>
          {result => (
            <span class="anim-fade font-mono text-xs tabular-nums text-paper/40">
              {result().total.toLocaleString("pt-BR")} títulos
            </span>
          )}
        </Show>
      </div>

      <label
        class="anim-reveal mt-4 flex items-center gap-3 rounded-sm border border-edge bg-panel px-3 py-2.5 transition-[border-color,box-shadow] duration-[var(--duration-base)] focus-within:border-amber focus-within:shadow-[0_0_0_3px_rgb(255_209_26/0.12)]"
        style={{ "--i": 1 }}
      >
        <Search size={16} class="shrink-0 text-paper/35" />
        <input
          ref={setSearch}
          type="search"
          value={query()}
          onInput={event =>
            setParams({ q: event.currentTarget.value || undefined, p: undefined })
          }
          placeholder={props.placeholder}
          class="w-full bg-transparent text-paper outline-none placeholder:text-paper/30"
        />
        <kbd class="hidden shrink-0 rounded-sm border border-edge px-1.5 py-0.5 font-mono text-[0.65rem] text-paper/35 sm:block">
          /
        </kbd>
      </label>

      <Show when={data()}>
        {result => (
          <ul class="mt-4 flex flex-wrap gap-2">
            <li class="anim-fade">
              <button
                type="button"
                onClick={() => setParams({ u: unwatchedOnly() ? undefined : "1", p: undefined })}
                class={chip}
                classList={{ "border-amber bg-amber/12 text-amber": unwatchedOnly() }}
                aria-pressed={unwatchedOnly()}
              >
                Não assistidos
              </button>
            </li>
            <li class="anim-fade">
              <button
                type="button"
                onClick={() => setParams({ g: undefined, p: undefined })}
                class={chip}
                classList={{ "border-amber bg-amber/12 text-amber": !group() }}
              >
                Tudo
              </button>
            </li>
            <For each={result().groups}>
              {(entry, index) => (
                <li class="anim-fade" style={{ "--i": Math.min(index() + 1, 14) }}>
                  <button
                    type="button"
                    onClick={() => setParams({ g: entry.name, p: undefined })}
                    class={chip}
                    classList={{
                      "border-amber bg-amber/12 text-amber": group() === entry.name,
                    }}
                  >
                    <span>{shortGroup(entry.name)}</span>
                    <span class="font-mono text-[0.7rem] tabular-nums opacity-55">
                      {entry.count}
                    </span>
                  </button>
                </li>
              )}
            </For>
          </ul>
        )}
      </Show>

      <div class="mt-6">
        <Show
          when={data.error}
          fallback={
            <Show when={!pending()} fallback={<PosterSkeleton />}>
              <Show
                when={data()!.items.length}
                fallback={
                  <div class="anim-reveal py-16 text-center">
                    <p class="font-display text-lg text-paper/70">Nenhum título com esse nome.</p>
                    <p class="mt-1 text-sm text-paper/40">
                      Tente outro termo ou volte para a categoria "Tudo".
                    </p>
                  </div>
                }
              >
                <PosterGrid items={data()!.items} />
              </Show>
            </Show>
          }
        >
          <p class="anim-fade py-12 text-sm text-live">{(data.error as Error).message}</p>
        </Show>
      </div>

      <Show when={pages() > 1}>
        <nav class="mt-10 flex items-center justify-between border-t border-edge pt-4">
          <button
            type="button"
            onClick={() => move(-1)}
            disabled={page() === 0}
            class="press flex items-center gap-2 rounded-sm border border-edge px-3 py-2 text-sm text-paper/70 hover:border-amber hover:text-amber disabled:opacity-30 disabled:hover:border-edge disabled:hover:text-paper/70"
          >
            <ChevronLeft size={16} />
            Anterior
          </button>
          <span class="font-mono text-xs tabular-nums text-paper/40">
            página {page() + 1} de {pages()}
          </span>
          <button
            type="button"
            onClick={() => move(1)}
            disabled={page() + 1 >= pages()}
            class="press flex items-center gap-2 rounded-sm border border-edge px-3 py-2 text-sm text-paper/70 hover:border-amber hover:text-amber disabled:opacity-30 disabled:hover:border-edge disabled:hover:text-paper/70"
          >
            Próxima
            <ChevronRight size={16} />
          </button>
        </nav>
      </Show>
    </main>
  );
}
