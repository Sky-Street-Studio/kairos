import type { KairosApi } from "../shared/types";

declare global {
  interface Window {
    kairos: KairosApi;
  }
}
