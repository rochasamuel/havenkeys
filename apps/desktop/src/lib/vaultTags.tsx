import { createContext, useContext } from "react";
import type { TagCount } from "./tags";

const VaultTags = createContext<TagCount[]>([]);

/** The vault's tags with counts, for the editor's suggestions. */
export const VaultTagsProvider = VaultTags.Provider;
export const useVaultTags = (): TagCount[] => useContext(VaultTags);
