import CatalogBrowser from "~/components/CatalogBrowser";

export default function Filmes() {
  return (
    <CatalogBrowser kind="filmes" heading="Filmes" placeholder="Buscar filme pelo título" />
  );
}
