export const token = (id: number): string => `⟦K${id}⟧`;

export const TOKEN_PATTERN = /⟦K(\d+)⟧/g;

/** Matches an unfinished token at the very end of streamed text. */
export const PARTIAL_TOKEN_AT_END = /⟦(K\d*)?$/;
