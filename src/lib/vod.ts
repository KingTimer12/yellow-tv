import type { CatalogItem, CatalogKind } from "~/lib/api";

export type { CatalogItem, CatalogKind } from "~/lib/api";

export const isSeries = (item: CatalogItem) => item.kind === "series";

/** "Series | Netflix" lê melhor como "Netflix" quando a seção já diz Séries. */
export const shortGroup = (group: string | null) =>
  (group ?? "").replace(/^(Series|Filmes|Canais)\s*\|\s*/i, "");

export const detailHref = (item: Pick<CatalogItem, "kind" | "id">) =>
  item.kind === "series"
    ? `/serie/${item.id}`
    : item.kind === "channel"
      ? `/watch/${item.id}`
      : `/filme/${item.id}`;

export const kindOf = (item: Pick<CatalogItem, "kind">): CatalogKind =>
  item.kind === "series" ? "series" : item.kind === "channel" ? "canais" : "filmes";

/** "Retomar em 18min" precisa do que falta, não do que já passou. */
export function remainingLabel(positionSecs: number, durationSecs: number | null) {
  if (!durationSecs || durationSecs <= 0) return "Retomar";
  const minutes = Math.max(1, Math.round((durationSecs - positionSecs) / 60));
  return `Retomar · faltam ${minutes}min`;
}
