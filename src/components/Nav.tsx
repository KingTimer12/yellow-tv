import { A, useLocation } from "@solidjs/router";
import { For } from "solid-js";
import { Film, Signal, Stack, StarOutline, Tv } from "./Icons";

const LINKS = [
  { href: "/", label: "Canais", icon: Tv, exact: true },
  { href: "/filmes", label: "Filmes", icon: Film, exact: false },
  { href: "/series", label: "Séries", icon: Stack, exact: false },
] as const;

export default function Nav() {
  const location = useLocation();

  const active = (href: string, exact: boolean) =>
    exact ? location.pathname === href : location.pathname.startsWith(href);

  return (
    <header class="sticky top-0 z-30 border-b border-edge bg-ink/85 backdrop-blur-md">
      <div class="mx-auto flex max-w-7xl items-center gap-6 px-4 py-3">
        <A href="/" class="group flex items-baseline gap-2 press" aria-label="YellowTV, início">
          <span class="font-display text-2xl font-extrabold leading-none tracking-[-0.04em] text-amber transition-[text-shadow] duration-300 group-hover:[text-shadow:0_0_18px_rgb(255_209_26/0.55)]">
            Yellow<span class="text-paper">TV</span>
          </span>
        </A>

        <span class="hidden items-center gap-1.5 font-mono text-[0.7rem] text-paper/35 sm:flex">
          <Signal size={13} class="text-live anim-live-dot" />
          no ar agora
        </span>

        <nav class="ml-auto flex items-center gap-1 text-sm sm:gap-2">
          <For each={LINKS}>
            {link => (
              <A
                href={link.href}
                class="wipe press flex items-center gap-2 rounded-sm px-2.5 py-1.5 text-paper/60 hover:bg-panel/60 hover:text-amber sm:px-3"
                classList={{ "text-amber": active(link.href, link.exact) }}
                data-active={String(active(link.href, link.exact))}
              >
                <link.icon size={16} />
                <span>{link.label}</span>
              </A>
            )}
          </For>
          <A
            href="/?g=favoritos"
            class="press hidden items-center gap-2 rounded-sm px-3 py-1.5 text-paper/50 hover:bg-panel/60 hover:text-amber sm:flex"
          >
            <StarOutline size={16} />
            <span>Favoritos</span>
          </A>
        </nav>
      </div>
    </header>
  );
}
