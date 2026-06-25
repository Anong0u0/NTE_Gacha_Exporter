export type AssetResolveRequest = {
  asset_ref: string;
  kind?: string | null;
};

export type AssetResolveResult = {
  asset_ref: string;
  kind?: string | null;
  url?: string | null;
};
