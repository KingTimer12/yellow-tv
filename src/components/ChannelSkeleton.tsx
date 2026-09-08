import { For } from "solid-js";

/** Row placeholders matching ChannelRow's height, so the list never jumps. */
export default function ChannelSkeleton(props: { count?: number }) {
  const rows = () => Array.from({ length: props.count ?? 12 }, (_, index) => index);

  return (
    <div aria-hidden="true">
      <For each={rows()}>
        {index => (
          <div
            class="anim-fade flex items-center gap-4 border-b border-edge/60 py-3 pl-3.5 pr-3"
            style={{ "--i": index }}
          >
            <div class="skeleton h-3.5 w-11 shrink-0 rounded-sm" />
            <div
              class="skeleton h-3.5 rounded-sm"
              style={{ width: `${38 + ((index * 7) % 34)}%` }}
            />
            <div class="skeleton ml-auto h-3.5 w-10 shrink-0 rounded-sm" />
          </div>
        )}
      </For>
    </div>
  );
}
