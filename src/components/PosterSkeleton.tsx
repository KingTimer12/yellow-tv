import { For } from "solid-js";

/** Placeholder grid that mirrors PosterGrid's geometry, so nothing shifts on load. */
export default function PosterSkeleton(props: { count?: number }) {
  const cells = () => Array.from({ length: props.count ?? 18 }, (_, index) => index);

  return (
    <ul
      class="grid grid-cols-2 gap-x-4 gap-y-7 sm:grid-cols-3 lg:grid-cols-5 xl:grid-cols-6"
      aria-hidden="true"
    >
      <For each={cells()}>
        {index => (
          <li class="anim-fade" style={{ "--i": index }}>
            <div class="skeleton aspect-[2/3] rounded-sm ring-1 ring-edge/60" />
            <div class="skeleton mt-2 h-3.5 w-4/5 rounded-sm" />
            <div class="skeleton mt-1.5 h-2.5 w-2/5 rounded-sm" />
          </li>
        )}
      </For>
    </ul>
  );
}
