import { createResource } from "solid-js";
import { listSources, type Source } from "~/lib/api";

/**
 * As fontes decidem se o app já tem catálogo, então tanto o gate de `/setup`
 * quanto a Biblioteca leem daqui.
 */
export function useSources() {
  const [data, { refetch }] = createResource<Source[]>(listSources);
  return {
    sources: () => data() ?? [],
    pending: () => data() === undefined && !data.error,
    error: () => data.error as Error | undefined,
    refetch,
  };
}
