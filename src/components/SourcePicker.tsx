import { For, Show } from "solid-js";
import type { StreamRef } from "~/lib/api";

/**
 * Metadata e fonte de stream são coisas distintas: quando o mesmo título vem de
 * mais de uma lista, quem escolhe é o usuário.
 */
export default function SourcePicker(props: {
  streams: StreamRef[];
  active: number;
  onPick: (index: number) => void;
}) {
  return (
    <Show when={props.streams.length > 1}>
      <div class="mt-4">
        <p class="font-mono text-[0.7rem] uppercase tracking-wide text-paper/35">fontes</p>
        <ul class="mt-2 flex flex-wrap gap-2">
          <For each={props.streams}>
            {(stream, index) => (
              <li>
                <button
                  type="button"
                  onClick={() => props.onPick(index())}
                  class="press flex items-baseline gap-1.5 rounded-full border border-edge bg-panel/50 px-3 py-1 text-sm text-paper/60 hover:border-amber-deep hover:text-amber"
                  classList={{
                    "border-amber bg-amber/12 text-amber": index() === props.active,
                  }}
                  aria-pressed={index() === props.active}
                >
                  <span>{stream.sourceLabel}</span>
                  <Show when={stream.quality}>
                    <span class="font-mono text-[0.7rem] opacity-55">{stream.quality}</span>
                  </Show>
                </button>
              </li>
            )}
          </For>
        </ul>
      </div>
    </Show>
  );
}
