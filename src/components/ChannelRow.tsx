import { A } from "@solidjs/router";
import { createSignal, Show } from "solid-js";
import { formatNumber, type Channel } from "~/lib/channels";
import { toggleFavorite } from "~/lib/api";
import { StarFilled, StarOutline } from "./Icons";

type ChannelRowProps = {
  channel: Channel;
  current?: boolean;
  favorite?: boolean;
  onToggle?: () => void;
};

export default function ChannelRow(props: ChannelRowProps) {
  const channel = () => props.channel;
  const [favorite, setFavorite] = createSignal(props.favorite ?? false);

  return (
    <div
      class="rail group flex items-stretch border-b border-edge/60 transition-colors duration-[var(--duration-fast)]"
      classList={{ "bg-amber/10": props.current }}
      data-current={String(Boolean(props.current))}
    >
      <A
        href={`/watch/${channel().id}`}
        class="press flex min-w-0 flex-1 items-center gap-4 py-2.5 pl-3.5 pr-2 hover:bg-panel/80"
        aria-current={props.current ? "page" : undefined}
      >
        <span
          class="w-14 shrink-0 border-r border-edge pr-3 text-right font-mono text-sm tabular-nums text-amber-deep transition-colors duration-[var(--duration-fast)] group-hover:text-amber"
          classList={{ "text-amber": props.current }}
        >
          {formatNumber(channel().channelNumber ?? 0)}
        </span>
        <span class="min-w-0 flex-1 truncate text-[0.95rem] text-paper transition-transform duration-[var(--duration-base)] ease-[var(--ease-out-soft)] group-hover:translate-x-0.5">
          {channel().title}
        </span>
        <span class="hidden w-36 shrink-0 truncate text-right text-xs text-paper/40 sm:block">
          {channel().group}
        </span>
      </A>
      <button
        type="button"
        onClick={async event => {
          event.preventDefault();
          setFavorite(await toggleFavorite(channel().id));
          props.onToggle?.();
        }}
        class="press grid w-12 shrink-0 place-content-center text-paper/25 hover:text-amber"
        classList={{ "text-amber": favorite() }}
        aria-pressed={favorite()}
        aria-label={`${favorite() ? "Remover" : "Salvar"} ${channel().title} nos favoritos`}
      >
        <Show when={favorite()} fallback={<StarOutline size={19} />} keyed>
          <StarFilled size={19} class="anim-star-pop" />
        </Show>
      </button>
    </div>
  );
}
