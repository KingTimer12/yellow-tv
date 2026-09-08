import { createEffect, createSignal, For, onCleanup, Show } from "solid-js";
import type { Channel } from "~/lib/channels";
import ChannelRow from "./ChannelRow";

const PAGE = 60;

type ChannelListProps = {
  channels: Channel[];
  currentId?: string;
  empty?: string;
};

/** Renders the list in pages so 3.677 rows never hit the DOM at once. */
export default function ChannelList(props: ChannelListProps) {
  const [shown, setShown] = createSignal(PAGE);
  const [sentinel, setSentinel] = createSignal<HTMLDivElement>();

  createEffect(() => {
    props.channels;
    setShown(PAGE);
  });

  createEffect(() => {
    const target = sentinel();
    if (!target) return;
    const observer = new IntersectionObserver(entries => {
      if (entries.some(entry => entry.isIntersecting)) setShown(n => n + PAGE);
    });
    observer.observe(target);
    onCleanup(() => observer.disconnect());
  });

  const visible = () => props.channels.slice(0, shown());

  return (
    <Show
      when={props.channels.length}
      fallback={
        <div class="anim-reveal p-10 text-center">
          <p class="font-display text-base text-paper/70">
            {props.empty ?? "Nenhum canal com esse nome."}
          </p>
        </div>
      }
    >
      <div>
        <For each={visible()}>
          {channel => <ChannelRow channel={channel} current={channel.id === props.currentId} />}
        </For>
        <Show when={shown() < props.channels.length}>
          <div ref={setSentinel} class="p-6 text-center font-mono text-xs text-paper/30">
            carregando mais canais<span class="anim-caret">_</span>
          </div>
        </Show>
      </div>
    </Show>
  );
}
